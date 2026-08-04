//! The compilation worker thread — the backend that drives the entire HRW observatory.
//!
//! # Architecture: why a worker thread?
//!
//! egui (the immediate-mode UI framework) calls `update()` every frame at ~60fps.
//! Each call must return in under ~16ms or the UI stutters. But compiling a
//! Modelica model through Rumoca's pipeline (parse → resolve → flatten → structural
//! analysis → index reduction → initialization → events → solve lowering) takes
//! hundreds of milliseconds, and simulation can take seconds. So all compiler and
//! solver work MUST run on a dedicated **worker thread**, communicating with the
//! UI thread via `mpsc` channels (message passing, no shared mutable state).
//!
//! Charter §4.4 / Decision 6: compilation runs on a worker thread, results
//! returned over a channel. The egui `update()` loop never blocks and never
//! calls into the compiler directly. Breakpoints for studying a phase belong
//! here (in `compile`), never in the paint path.
//!
//! # The communication pattern
//!
//! ```text
//! UI thread                         Worker thread
//! ─────────                         ─────────────
//! Worker::send(ToWorker::Compile)  ──►  WorkerState::handle()
//!                                          │ compile(), simulate(), ...
//! rx.try_recv() ◄── FromWorker::Log        │ (streams partial results via `emit`)
//! rx.try_recv() ◄── FromWorker::Progress   │
//! rx.try_recv() ◄── FromWorker::Compiled   │ (final result)
//! ```
//!
//! The UI polls `rx.try_recv()` each frame (non-blocking). The worker calls
//! `ctx.request_repaint()` after sending a result so the UI wakes up even if
//! the user isn't interacting.
//!
//! # The Rumoca Session
//!
//! The worker owns a persistent Rumoca `Session` — an incremental compilation
//! workspace (the same type the LSP uses). Library dependencies (the MSL) are
//! loaded once as **source roots**; thereafter each specimen edit re-resolves
//! incrementally (~0.3s) rather than re-parsing thousands of library files.
//!
//! # JSON serialization strategy
//!
//! For each stage we serialize only the **user model's** IR node (a few KB),
//! never the whole resolved aggregate — resolving with the full MSL loaded
//! produces a ~430MB tree, of which the user's model is a tiny slice.
//! We use `serde_json::Value` (a generic JSON tree) as the interchange format
//! between the worker and UI because not all Rumoca IR types implement `Serialize`,
//! and JSON lets the generic tree-inspector widget render any stage without
//! knowing its Rust type.

// --- Standard library imports ---
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
// `mpsc` = multi-producer, single-consumer channel. Here we use it as
// single-producer (worker) / single-consumer (UI) in each direction.
// `Sender` and `Receiver` are the two halves of a channel; `Sender` is
// `Clone` (multi-producer) but we only have one of each.
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

// --- Rumoca crate imports (the upstream API surface) ---
// These are the crates HRW calls into. When Rumoca upstream changes an API,
// the breakage will show up in these imports and their call sites below.
//
// `PhaseResult` is the key enum: every pipeline invocation returns one of
//   - `Success(CompileResult)` — the full DAE + flat IR
//   - `Failed { phase, error, .. }` — which phase failed and why
//   - `NeedsInner { .. }` — the model needs inner declarations (rare)
// `FailedPhase` names the phase that failed (Flatten, ToDae, etc.).
// `SourceRootKind` tags a source set as durable-external (libraries) vs ephemeral.
use rumoca_compile::compile::{CompilationResult, FailedPhase, PhaseResult, SourceRootKind};
// Library-loading helpers: `parse_source_root_with_cache` parses a directory
// of `.mo` files into a `ParsedSourceRoot`, and `source_root_source_set_key`
// generates a stable key for caching.
use rumoca_compile::source_roots::{parse_source_root_with_cache, source_root_source_set_key};
// `Session` is Rumoca's incremental compilation workspace (the same type the
// LSP server uses). `SessionConfig` configures it (we use defaults).
use rumoca_compile::{Session, SessionConfig};

/// A request from the UI thread to the worker.
///
/// This enum defines every possible command the UI can send. The worker's
/// `handle()` method pattern-matches on it and dispatches to the right handler.
/// Adding a new command means adding a variant here + a match arm in `handle()`.
///
/// These messages cross a thread boundary via `mpsc::Sender::send()`, which
/// requires `Send` — all variants must contain only `Send`-able types (no `Rc`,
/// no raw pointers). `PathBuf`, `String`, and `f64` are all `Send`.
pub enum ToWorker {
    /// Replace the library source roots (reloads them into a fresh session).
    SetLibraries(Vec<PathBuf>),
    /// Run parse → resolve on a specimen file.
    Compile(PathBuf),
    /// Compile a model **already present in a loaded library**, by qualified name
    /// — `Modelica.Electrical.Analog.Examples.CauerLowPassSC`.
    ///
    /// The corpus counterpart of [`Self::Compile`]. Until 2026-08-01 the worker
    /// could do this (`compile_model_by_name`, built for the fidelity sweep) and
    /// **the UI had no way to ask**, which meant a tour could not link to an MSL
    /// model and the 2,626-model corpus was unreachable from the app.
    CompileLibraryModel(String),
    /// Extract an arbitrary class from the resolved tree by qualified name, so
    /// the UI can navigate into a definition a `def_id`/`type_def_id` points at.
    OpenDef(String),
    /// Compile the model, lower it to a `SolveModel`, and run a simulation
    /// to `t_end`, returning the state trajectories to plot. Runs on this worker
    /// thread so the UI never blocks.
    Simulate { path: PathBuf, model: String, t_end: f64 },
    /// Enable or disable Rumoca's internal `tracing` subscriber on this thread.
    SetTracing(bool),
}

/// Simulation output for plotting — one time axis and, per output variable,
/// its trajectory (`data[var][t]`). Deliberately plain (no Rumoca types) so the UI
/// stays decoupled from the solver crates.
///
/// Layout: `times` is the shared x-axis; `names[i]` labels the i-th variable;
/// `data[i][j]` is variable i at time `times[j]`. The first `n_states`
/// variables are true differential states (the ODE unknowns); the rest are
/// algebraic outputs computed from the state at each time step.
///
/// `Clone` so [`FromWorker`] can derive it — see the note there. Every field is
/// owned plain data, so a clone is a deep copy of the trajectories and nothing
/// subtler. `SolverStepRecord` comes from `rumoca_solver` and is `Clone` there.
#[derive(Clone)]
pub struct SimData {
    pub times: Vec<f64>,
    pub names: Vec<String>,
    pub data: Vec<Vec<f64>>,
    /// The first `n_states` names are true states (the rest are algebraics/outputs).
    pub n_states: usize,
    /// True when the model has a **discrete update** — a `reinit` (`f_z`) or a
    /// `when`-clause assignment (`f_m`) — that can jump a variable's value at an
    /// event. Only then does the plot break the polyline at discontinuities. A
    /// bare zero-crossing with no update (BenchActuator has one, yet is smooth)
    /// does *not* count, and a smooth model's coarse-but-steep transients (a stiff
    /// current spike) must never be mistaken for jumps. See [`discontinuity_segments`].
    pub has_discontinuities: bool,
    /// Per-step solver diagnostics (t, h, order) from the BDF integrator.
    pub solver_steps: Vec<rumoca_solver::SolverStepRecord>,
}

/// Split a plotted trajectory into contiguous segments, breaking it where the
/// value **jumps discontinuously** — a state reinitialized at an event (the ball's
/// velocity flips at a bounce). Returns half-open index ranges into `values`;
/// the caller draws one polyline per range so egui never interpolates a *sloped*
/// line across the jump ("discontinuities render as discontinuities").
///
/// # Algorithm
///
/// A break sits between `i-1` and `i` when `|Δ|` exceeds
/// `max(range · 0.08, 6 · median|Δ|)` — a per-series threshold well above the
/// smooth step yet well below any real reinit (for BouncingBall's `v` the two
/// differ ~40x). Caller gates this on [`SimData::has_discontinuities`]; on a uniform
/// output grid alone a jump is indistinguishable from an under-resolved steep transient.
///
/// # Why `Vec<Range<usize>>` and not just "where are the jumps?"
///
/// Returning segments (not break-points) lets the caller iterate directly:
/// `for seg in segments { plot_line(&values[seg]); }` — no off-by-one arithmetic.
pub fn discontinuity_segments(values: &[f64]) -> Vec<std::ops::Range<usize>> {
    let n = values.len();
    if n < 3 {
        return std::iter::once(0..n).collect();
    }
    let diffs: Vec<f64> = (1..n).map(|i| (values[i] - values[i - 1]).abs()).collect();
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &v in values {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let mut sorted = diffs.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    const RANGE_FRACTION: f64 = 0.08;
    const MEDIAN_MULTIPLIER: f64 = 6.0;
    let threshold = ((hi - lo) * RANGE_FRACTION).max(MEDIAN_MULTIPLIER * median);
    // A flat/degenerate series (threshold 0) has no meaningful jumps — one segment.
    if threshold <= f64::EPSILON {
        return std::iter::once(0..n).collect();
    }
    let mut segments = Vec::new();
    let mut start = 0;
    for (k, &d) in diffs.iter().enumerate() {
        if d > threshold {
            segments.push(start..k + 1); // segment ends at the pre-jump sample k
            start = k + 1;
        }
    }
    segments.push(start..n);
    segments
}

/// What a compile was asked to compile.
///
/// The two differ in exactly three places — where the source comes from, what
/// names the model, and whether the document is registered with the session —
/// and are identical in the ten stages after that. Keeping them one function
/// rather than two is what stops the pipeline drifting into two versions, which
/// is the defect the fidelity work found twice on 2026-07-31.
#[derive(Clone, Copy)]
enum CompileTarget<'a> {
    /// A specimen file on disk.
    File(&'a Path),
    /// A model already present in a loaded library, named in full.
    #[allow(dead_code, reason = "constructed by compile_model_by_name; see there")]
    Library(&'a str),
}

/// A resolved compile target: where the source is, and what to call the model.
struct Located {
    uri: String,
    source: String,
    /// `Some` for a library model, whose name the caller supplied in full.
    /// `None` for a specimen, whose model name comes from parsing it.
    qualified: Option<String>,
    /// 1-based line where the requested class is declared, for a library model.
    ///
    /// **From `ClassDef::location.start_line`, so it is exact.** A library file
    /// commonly declares dozens of classes across thousands of lines, and
    /// `Modelica.Electrical.Analog.Basic.Resistor` opens 1,498 lines into
    /// `Basic.mo`. Landing the reader at line 1 would show a package header and
    /// nothing they asked for. Searching the text for `model Resistor` would be
    /// **heuristic name matching**, which `docs/identity-and-provenance.md`
    /// rules out; the compiler already knows the answer.
    decl_line: Option<u32>,
}

/// The declaring file behind a **library model**, for the source view.
///
/// Carried rather than re-read from disk: this is the exact text the session
/// parsed, so the source pane shows **what was compiled** rather than what the
/// path holds now. The two are the same today and the distinction costs nothing
/// to keep right.
#[derive(Clone)]
pub struct LibrarySource {
    /// Document URI of the declaring file — shown, because the pane holds a
    /// whole package file and the reader must know which one.
    pub uri: String,
    /// The file's text, or why it could not be read.
    ///
    /// A `Result` rather than an empty `String` so the pane can **say** what went
    /// wrong. Blank-on-failure would be indistinguishable from the refusal this
    /// replaced, and from a genuinely empty file.
    pub text: Result<String, String>,
    /// 1-based line of the requested class within the text.
    pub decl_line: Option<u32>,
}

/// What actually became of a stage — **three outcomes, not two.**
///
/// This replaces a `note_is_error: bool` that three different constructors set
/// to `true` for three different reasons: "produced nothing", "failed with a
/// structured diagnosis", and **"reported a problem but produced usable IR the
/// pipeline went on to consume"**. That third one is not a failure at all.
/// `Drivetrain` is *structurally singular* on purpose — it is a high-index
/// model, and index reduction fixes it two stages later — so anything counting
/// errors on the old boolean calls a healthy compile a failure.
///
/// That miscount is not hypothetical: it produced a false finding on
/// 2026-07-29 (`docs/ideas.md` #51), which is why `docs/fidelity-plan.md`
/// sequences this split *before* any harness reads outcomes at scale.
///
/// The UI's red/neutral colouring is unchanged — see [`Stage::note_is_error`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    /// The stage produced its IR and Rumoca reported nothing about it. Also the
    /// `Default`, which is the "not yet computed" state.
    #[default]
    Ok,
    /// The stage produced **usable IR** and Rumoca reported something alongside
    /// it — a structurally singular system, typecheck diagnostics, a recovered
    /// parse tree, surplus initial conditions. The pipeline continued.
    ///
    /// Rendered red like a failure, because the user should see it. Counted
    /// separately from one, because it is not one.
    Flagged,
    /// The stage produced no IR of its own. `value` is either `None` or holds
    /// *only* the error payload under `"error"`. The pipeline stopped here.
    Failed,
}

/// One pipeline stage's outcome for the selected model: the serialized IR node
/// (if the stage produced one) plus an optional note (error or status).
///
/// This is the uniform envelope every stage tab in the UI receives. The UI
/// doesn't know which pipeline stage it came from — it just renders:
/// - `value`: the JSON IR tree (if the stage produced one), displayed in the
///   generic tree inspector
/// - `note`: an optional status/error message shown above the tree
/// - `outcome`: [`Outcome`] — drives colour, and lets a census tell a *flagged*
///   stage apart from a *failed* one
///
/// `#[derive(Clone, Default)]` — `Clone` because the progressive-streaming
/// pattern sends clones mid-compile; `Default` gives "not yet computed"
/// (`None`/`None`/`Ok`).
#[derive(Clone, Default)]
pub struct Stage {
    pub value: Option<serde_json::Value>,
    pub note: Option<String>,
    /// Which of the three outcomes this stage reached. Prefer the constructors
    /// below to setting this by hand.
    pub outcome: Outcome,
}

/// Constructors for the possible stage outcomes. `pub(crate)` — only the worker
/// builds stages in production; the UI consumes them read-only, and tests build
/// them through these rather than by struct literal so a new field cannot be
/// forgotten at one site.
impl Stage {
    /// Stage succeeded and produced an IR tree to display.
    pub(crate) fn ok(value: serde_json::Value) -> Self {
        Stage { value: Some(value), note: None, outcome: Outcome::Ok }
    }
    /// Stage failed — no IR, just an error message (rendered red).
    /// `impl Into<String>` accepts both `String` and `&str` — a Rust ergonomic
    /// pattern so callers can pass either without explicit conversion.
    pub(crate) fn err(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), outcome: Outcome::Failed }
    }
    /// A non-error status note for a stage with no IR of its own to show.
    pub(crate) fn info(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), outcome: Outcome::Ok }
    }
    /// A best-effort IR plus an error note — a recovered parse tree, a singular
    /// structural analysis, surplus initial conditions. [`Outcome::Flagged`]:
    /// **the value is real and downstream stages consume it.**
    pub(crate) fn recovered(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), outcome: Outcome::Flagged }
    }
    /// A successful IR plus an informational (non-error) note — e.g. the
    /// index-reduction stage's "already index-1" / "reduced from singular".
    pub(crate) fn ok_with_note(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), outcome: Outcome::Ok }
    }

    /// Serialize a value to JSON and wrap in a successful Stage, or return an
    /// error Stage if serialization fails (instead of silently producing Null).
    fn from_ser<T: serde::Serialize>(v: &T) -> Self {
        match serde_json::to_value(v) {
            Ok(val) => Stage::ok(val),
            Err(e) => Stage::err(format!("serialization failed: {e}")),
        }
    }

    /// Stage failed with structured error data. The error is embedded in the
    /// value JSON under `"error"` so the UI can render a rich summary, and the
    /// note carries a short message for the stage tab label.
    ///
    /// [`Outcome::Failed`], **not** `Flagged: the `value` here is the error
    /// payload rather than IR, so nothing downstream can consume it. It shares
    /// a shape with `recovered` and not a meaning — which is precisely the
    /// conflation this enum exists to end.
    pub(crate) fn err_with_details(error: serde_json::Value, note: impl Into<String>) -> Self {
        Stage {
            value: Some(serde_json::json!({ "error": error })),
            note: Some(note.into()),
            outcome: Outcome::Failed,
        }
    }

    /// Should the note render red? True for both abnormal outcomes.
    ///
    /// **Preserves the old field's behaviour exactly**, so this split changed no
    /// pixel and no control flow: every reader of the former `note_is_error`
    /// field now calls this and sees what it saw before. The three-way truth is
    /// available to whoever wants it via [`Stage::outcome`] — which, for now, is
    /// the fidelity harness rather than the UI.
    pub fn note_is_error(&self) -> bool {
        self.outcome != Outcome::Ok
    }

    /// The structured error payload Rumoca supplied, if this stage carries one.
    ///
    /// Both `Failed` (via `err_with_details`) and `Flagged` (via `recovered`)
    /// may embed one under `"error"`. **F9 asks whether it is there at all**: a
    /// stage that failed with nothing but a formatted string has lost the
    /// spans, labels and counts Rumoca actually reported.
    pub fn error_json(&self) -> Option<&serde_json::Value> {
        self.value.as_ref()?.get("error")
    }
}

/// Serialize to JSON, falling back to a descriptive error string instead of
/// Null. Used inside `json!()` macros where we need a Value, not a Stage.
fn ser_value<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v)
        .unwrap_or_else(|e| serde_json::Value::String(format!("serialization failed: {e}")))
}

/// Which pipeline stage the user is viewing. The Rumoca compiler has discrete
/// phases (Parse, Resolve, etc.), each producing an intermediate representation
/// (IR). This enum tracks which phase is selected. `Simulation` is special —
/// it's not a compiler phase but an on-demand run of the compiled model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageKind {
    Parse,
    Resolve,
    Instantiate,
    Typecheck,
    Flatten,
    /// **DAE construction** — the flat equation list becomes a mathematical
    /// system: variables partitioned into states/algebraics/parameters, and
    /// equations into the MLS Appendix B partitions (`f_x`, `f_z`, `f_m`, `f_c`).
    ///
    /// Added 2026-08-03. It was **built and never shown** — `rumoca-ir-dae` is a
    /// boundary IR like `rumoca-ir-flat`, and HRW simply had no tab for it, so
    /// the leftmost mathematical step of the chain was invisible. Found while
    /// writing `docs/fixture-tours/dae-construction.md`, which had to teach the
    /// step from its neighbours.
    Dae,
    Structural,
    IndexReduction,
    Initialization,
    Events,
    SolveLowering,
    Simulation,
}

impl StageKind {
    pub const ALL: &[StageKind] = &[
        StageKind::Parse, StageKind::Resolve, StageKind::Instantiate,
        StageKind::Typecheck, StageKind::Flatten, StageKind::Dae, StageKind::Structural,
        StageKind::IndexReduction, StageKind::Initialization, StageKind::Events,
        StageKind::SolveLowering, StageKind::Simulation,
    ];

    /// The compilation stages, in pipeline order — [`Self::ALL`] **without
    /// `Simulation`**.
    ///
    /// `Simulation` is in `ALL` because it is a tab, but it is not a compilation stage:
    /// `StageBundle::get()` *panics* on it. So any code that walks stages and asks the
    /// bundle for each one must use this list, not `ALL`. Added 2026-07-29 after
    /// `failure_context` walked `ALL` and hit that panic in three tests — the trap is
    /// easy to fall into and silent until something calls `get`.
    pub const COMPILATION: &[StageKind] = &[
        StageKind::Parse, StageKind::Resolve, StageKind::Instantiate,
        StageKind::Typecheck, StageKind::Flatten, StageKind::Dae, StageKind::Structural,
        StageKind::IndexReduction, StageKind::Initialization, StageKind::Events,
        StageKind::SolveLowering,
    ];

    /// Human-readable name for this stage, matching the tab labels in the UI.
    pub fn name(self) -> &'static str {
        match self {
            StageKind::Parse => "Parse",
            StageKind::Resolve => "Resolve",
            StageKind::Instantiate => "Instantiate",
            StageKind::Typecheck => "Typecheck",
            StageKind::Flatten => "Flatten",
            StageKind::Dae => "DAE",
            StageKind::Structural => "Structural",
            StageKind::IndexReduction => "Index reduction",
            StageKind::Initialization => "Initialization",
            StageKind::Events => "Events",
            StageKind::SolveLowering => "Solve lowering",
            StageKind::Simulation => "Simulation",
        }
    }

    /// The PascalCase slug used in `hrw://stage/<Slug>` links and in the capture.
    ///
    /// **Distinct from [`name`], and that distinction caused a real break.** `name` is a
    /// *display* label for tab text, so it reads "Index reduction" with a space. The
    /// capture emitted `name`, which meant `focus.json` said `"stage": "Index reduction"`
    /// while `from_slug` only accepts `"IndexReduction"` — so two of the eleven stages
    /// were describable in a capture and **unreachable by the link built from it**.
    /// Found 2026-07-29 by the noun/verb parity audit, before it bit anyone.
    ///
    /// This is the canonical inverse of [`from_slug`], and
    /// `every_stage_round_trips_between_capture_and_link` holds them together.
    pub fn slug(self) -> &'static str {
        match self {
            StageKind::Parse => "Parse",
            StageKind::Resolve => "Resolve",
            StageKind::Instantiate => "Instantiate",
            StageKind::Typecheck => "Typecheck",
            StageKind::Flatten => "Flatten",
            StageKind::Dae => "Dae",
            StageKind::Structural => "Structural",
            StageKind::IndexReduction => "IndexReduction",
            StageKind::Initialization => "Initialization",
            StageKind::Events => "Events",
            StageKind::SolveLowering => "SolveLowering",
            StageKind::Simulation => "Simulation",
        }
    }

    /// Parse a PascalCase slug (as used in `hrw://stage/<Slug>` URLs) into a stage kind.
    pub fn from_slug(s: &str) -> Option<Self> {
        match s {
            "Parse" => Some(Self::Parse),
            "Resolve" => Some(Self::Resolve),
            "Instantiate" => Some(Self::Instantiate),
            "Typecheck" => Some(Self::Typecheck),
            "Flatten" => Some(Self::Flatten),
            "Dae" => Some(Self::Dae),
            "Structural" => Some(Self::Structural),
            "IndexReduction" => Some(Self::IndexReduction),
            "Initialization" => Some(Self::Initialization),
            "Events" => Some(Self::Events),
            "SolveLowering" => Some(Self::SolveLowering),
            "Simulation" => Some(Self::Simulation),
            _ => None,
        }
    }
}

/// The ten pipeline-stage results as one bundle, used for progressive streaming.
///
/// During a compile, the worker fills this bundle one stage at a time and sends
/// a clone (`FromWorker::CompileProgress`) after each stage completes. The UI
/// uses these intermediate snapshots to colour tabs green/red as each stage
/// lands — the user sees real-time progress, not an all-or-nothing wait. The
/// finished bundle is unpacked into the final `FromWorker::Compiled`.
///
/// Not-yet-computed stages are `Stage::default()` (neutral — no IR, no error),
/// thanks to `#[derive(Default)]`.
#[derive(Clone, Default)]
pub struct StageBundle {
    pub parse: Stage,
    pub resolve: Stage,
    pub instantiate: Stage,
    pub typecheck: Stage,
    pub flatten: Stage,
    pub dae: Stage,
    pub structural: Stage,
    pub index_reduction: Stage,
    pub initialization: Stage,
    pub events: Stage,
    pub solve_lowering: Stage,
}

impl StageBundle {
    /// Access a stage by kind. Panics on `Simulation` — that variant has no
    /// corresponding stage in the bundle (it's an on-demand run, not a
    /// compilation stage). Callers must handle `Simulation` before calling this.
    pub fn get(&self, kind: StageKind) -> &Stage {
        match kind {
            StageKind::Parse => &self.parse,
            StageKind::Resolve => &self.resolve,
            StageKind::Instantiate => &self.instantiate,
            StageKind::Typecheck => &self.typecheck,
            StageKind::Flatten => &self.flatten,
            StageKind::Dae => &self.dae,
            StageKind::Structural => &self.structural,
            StageKind::IndexReduction => &self.index_reduction,
            StageKind::Initialization => &self.initialization,
            StageKind::Events => &self.events,
            StageKind::SolveLowering => &self.solve_lowering,
            StageKind::Simulation => panic!("Simulation is not a compilation stage — handle it before calling StageBundle::get()"),
        }
    }

    /// All ten stages as (name, optional JSON value) pairs, for
    /// `bridge::write_stages` and similar bulk consumers.
    pub fn as_stage_pairs(&self) -> [(&'static str, Option<&serde_json::Value>); 11] {
        [
            ("parse", self.parse.value.as_ref()),
            ("resolve", self.resolve.value.as_ref()),
            ("instantiate", self.instantiate.value.as_ref()),
            ("typecheck", self.typecheck.value.as_ref()),
            ("flatten", self.flatten.value.as_ref()),
            ("dae", self.dae.value.as_ref()),
            ("structural", self.structural.value.as_ref()),
            ("index_reduction", self.index_reduction.value.as_ref()),
            ("initialization", self.initialization.value.as_ref()),
            ("events", self.events.value.as_ref()),
            ("solve_lowering", self.solve_lowering.value.as_ref()),
        ]
    }
}

/// Resolved identity of a `DefId` referenced in a stage's IR — what an opaque
/// integer like `type_def_id: 27579` actually points at.
///
/// Rumoca's IR uses integer `DefId`s (definition IDs) as interned references
/// to classes and components in the resolved tree. Without resolution, these
/// are just opaque numbers. This struct maps them to human-readable names,
/// kinds, and source locations. A deterministic lookup against the resolved
/// tree (which the worker owns), *not* reasoning: the UI shows it inline and
/// the bridge hands it to Claude so answers follow real pointers instead of
/// narrating faith in a number.
#[derive(Clone)]
pub struct DefInfo {
    /// Qualified name, e.g. "Modelica.Mechanics.Rotational.Components.Inertia".
    pub name: String,
    /// Whether this DefId names a class or a non-class definition.
    pub kind: DefKind,
    /// Class keyword ("model", "block", …) when this DefId names a class.
    pub class_type: Option<String>,
    /// Source location of the class definition (when a class).
    pub file_name: Option<String>,
    pub line: Option<u32>,
}

/// Whether a `DefId` resolves to a class definition or a non-class definition
/// (component, type alias, etc.).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefKind {
    Class,
    Definition,
}

impl DefKind {
    fn as_str(self) -> &'static str {
        match self {
            DefKind::Class => "class",
            DefKind::Definition => "definition",
        }
    }
}

impl DefInfo {
    /// Compact inline label for the tree, e.g. "model Modelica.…Inertia".
    pub fn label(&self) -> String {
        match &self.class_type {
            Some(ct) => format!("{ct} {}", self.name),
            None => self.name.clone(),
        }
    }

    /// JSON form for the bridge focus file (DefInfo doesn't derive Serialize to
    /// avoid taking a direct `serde` dependency).
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "kind": self.kind.as_str(),
            "class_type": self.class_type,
            "file_name": self.file_name,
            "line": self.line,
        })
    }
}

/// A single log entry streamed from the worker to the UI.
///
/// The log view shows these entries in real time — each has a timestamp
/// (seconds since compile/simulate started) so the UI can render a timeline
/// of compilation progress. Entries are streamed via `FromWorker::Log` as
/// they happen, not batched at the end.
#[derive(Clone)]
pub struct LogEntry {
    /// Seconds elapsed since the compile started.
    pub elapsed_secs: f64,
    pub level: LogLevel,
    pub message: String,
}

/// Severity / category of a log entry. The UI uses this to pick colours and
/// icons, and to build the stage-timing timeline (matching `StageStart`/`StageEnd`
/// pairs).
///
/// `Copy` — a single byte, cheaply passed by value (no heap allocation).
/// `PartialEq + Eq` — enables `matches!(entry.level, LogLevel::StageStart)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogLevel {
    /// General informational message (e.g. "compiling BouncingBall.mo").
    Info,
    /// Marks the beginning of a pipeline stage (paired with a `StageEnd`).
    StageStart,
    /// Marks the end of a pipeline stage (includes timing in the message).
    StageEnd,
    /// A warning from the compiler or solver.
    Warn,
    /// An error — compilation or simulation failed at this point.
    Error,
    /// Captured stdout from Rumoca library code (via `OutputCapture`).
    Stdout,
    /// Captured stderr from Rumoca library code (via `OutputCapture`).
    Stderr,
    /// A `tracing` event forwarded from Rumoca's internal instrumentation
    /// (via `TracingForwarder`).
    Trace,
}

impl LogLevel {
    /// A stable, greppable name for this level.
    ///
    /// Used by the crash log (`diagnostics.rs`), which needs a name that will
    /// not change with the UI's colours or icons. `log_view` renders its own
    /// display strings from `level_style`; the two are separate on purpose,
    /// because one is for looking at and one is for searching.
    pub fn label(self) -> &'static str {
        match self {
            LogLevel::Info => "Info",
            LogLevel::StageStart => "StageStart",
            LogLevel::StageEnd => "StageEnd",
            LogLevel::Warn => "Warn",
            LogLevel::Error => "Error",
            LogLevel::Stdout => "Stdout",
            LogLevel::Stderr => "Stderr",
            LogLevel::Trace => "Trace",
        }
    }
}

/// A result from the worker back to the UI thread.
///
/// Like `ToWorker`, this enum crosses a thread boundary (must be `Send`).
/// The UI calls `rx.try_recv()` each frame and pattern-matches on these
/// variants to update its state.
///
/// The variants come in two flavours:
/// - **Streaming** (`Log`, `CompileProgress`) — sent mid-task so the UI
///   updates in real time.
/// - **Final** (`Compiled`, `Simulated`, `Libraries`, `DefTree`) — one per
///   request, signals the task is done.
///
/// **`Clone` is derived for the test suite, and the cost is worth naming.**
/// Every payload here is plain data — IR, frames, an equation sheet — so a clone
/// is a deep copy and nothing more. The app never needs it: results move from
/// the worker thread to the UI once. `test_msl::compile_specimen_shared` does,
/// because it memoises one compile per specimen per process and hands out copies
/// (`docs/ideas.md` #48). Compiling `Drivetrain` six times per run against the
/// MSL is most of the suite's runtime.
/// **`large_enum_variant` is allowed here deliberately, with the reason.**
/// `Compiled` is far bigger than the other variants, and the lint's remedy is to
/// box it. That trades a real cost for a theoretical one: the lint protects
/// against *many* instances each paying the largest variant's size, and there is
/// **one of these in flight at a time** — the worker sends a result, the UI takes
/// it. Boxing would add an allocation on the hot path and force every one of the
/// ~40 `FromWorker::Compiled { .. }` match sites to dereference, for no memory
/// anyone can observe.
///
/// Item-level rather than crate-level on purpose: the judgement is about *this*
/// enum, and a crate-wide allow would silence the lint where it might be right.
#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
pub enum FromWorker {
    /// Outcome of loading libraries: total documents loaded, or an error.
    Libraries(Result<usize, String>),
    /// A log entry streamed during compilation (stage timing, milestones, stderr).
    Log(LogEntry),
    /// Partial compile progress: the stages known so far (rest neutral). Streamed
    /// after each compile chunk so the UI colours tabs as work lands; the compile
    /// is still running (`compiling` stays true) and a final `Compiled` follows.
    CompileProgress { path: PathBuf, stages: StageBundle },
    /// Outcome of compiling a specimen through the pipeline stages.
    Compiled {
        path: PathBuf,
        /// Simple name of the model whose IR the stages show.
        model: Option<String>,
        /// All ten pipeline-stage results bundled together.
        stages: StageBundle,
        /// Resolved identity of every DefId referenced in the model's IR.
        def_index: BTreeMap<u64, DefInfo>,
        /// Pre-formatted equation sheet built from the typed DAE.
        equation_sheet: Option<crate::equation_sheet::EquationSheet>,
        identifier_index: Option<crate::identifier_index::IdentifierIndex>,
        /// Index-reduction animation frames (empty if no reduction occurred).
        index_reduction_frames: Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>,
        /// `pre()`-lowering replay frames (idea #40). Recorded by re-running DAE
        /// construction over the flat model with an observer attached — the pass
        /// runs *inside* construction, so the finished DAE cannot be replayed.
        pre_lowering_frames: Vec<rumoca_phase_dae::PreLoweringFrame>,
        /// The flat model, for a live-debug replay of `pre()` lowering. It is the
        /// last artifact from *before* that pass runs.
        flat: Option<rumoca_ir_flat::Model>,
        /// The raw DAE for live-debug replay of index reduction.
        dae: Option<rumoca_ir_dae::Dae>,
        /// Connection-expansion replay frames (MLS §9). Recorded by re-running
        /// flatten with an observer; empty for a model with no `connect()`.
        connection_frames: Vec<rumoca_phase_flatten::connections::trace::ConnectionFrame>,
        /// The declaring file's source, for a **library model**. `None` for a
        /// specimen, which the UI reads from its own path so live edits show.
        ///
        /// **Sent because the source view is not optional for MSL models**
        /// (Doug, 2026-08-01). The pane used to refuse them, claiming a library
        /// model had "no single source file to show" — which was simply
        /// untrue: the worker reads that very file out of the session in order
        /// to compile it, and then dropped it on the floor.
        library_source: Option<LibrarySource>,
    },
    /// A class opened by navigation: its qualified name and (on success) its
    /// resolved IR plus the DefIds it references, so navigation can continue.
    DefTree {
        name: String,
        result: Result<(serde_json::Value, BTreeMap<u64, DefInfo>), String>,
    },
    /// The outcome of a simulation request — trajectories or an error.
    Simulated {
        path: PathBuf,
        result: Result<SimData, String>,
    },
}

/// Handle held by the UI thread for talking to the worker.
///
/// This is the UI's half of the bidirectional channel pair:
/// - `tx: Sender<ToWorker>` — sends requests TO the worker (fire-and-forget,
///   never blocks)
/// - `rx: Receiver<FromWorker>` — receives results FROM the worker (polled
///   each frame via `try_recv()`)
///
/// The worker thread's half (the `Receiver<ToWorker>` and `Sender<FromWorker>`)
/// are moved into the spawned thread in `Worker::spawn()` and are not accessible
/// from the UI thread. This separation is enforced by Rust's ownership system —
/// you literally cannot access the wrong end.
pub struct Worker {
    pub(crate) tx: Sender<ToWorker>,
    pub(crate) rx: Receiver<FromWorker>,
    /// Set to `true` when a `send()` fails (the worker thread has exited).
    /// The UI can check this to show a "worker died" diagnostic instead of
    /// silently dropping messages.
    pub send_failed: bool,
}

impl Worker {
    /// Spawn the worker thread and return a `Worker` handle for the UI.
    ///
    /// Creates two `mpsc` channels (one for each direction), spawns a named OS
    /// thread, and moves one end of each channel into it. The `ctx` (egui
    /// `Context`) is cloned into the thread so it can call `request_repaint()`
    /// to wake the UI whenever a result is ready.
    ///
    /// `move || { ... }` — this is a *move closure*: it takes ownership of
    /// `rx_req`, `tx_res`, and `ctx`, moving them into the new thread. After
    /// this point the spawning thread cannot access those values — Rust's
    /// ownership system guarantees no data races.
    pub fn spawn(ctx: egui::Context) -> Worker {
        // Two independent channels: requests flow UI→worker, results flow worker→UI.
        let (tx_req, rx_req) = mpsc::channel::<ToWorker>();
        let (tx_res, rx_res) = mpsc::channel::<FromWorker>();

        thread::Builder::new()
            // Named threads show up in debuggers and crash reports — very helpful
            // when debugging a hang or panic on the worker.
            .name("rumoca-worker".to_owned())
            .spawn(move || {
                let mut state = WorkerState::new();
                // `rx_req.recv()` blocks until a message arrives (or the sender
                // is dropped, returning `Err`). This is the worker's event loop:
                // one message at a time, fully serial — no concurrent compilations.
                while let Ok(msg) = rx_req.recv() {
                    // `emit` is a closure the compile/simulate methods call to
                    // stream partial results (log entries, stage progress) back
                    // to the UI mid-task. Each emit also wakes the UI so it
                    // repaints and picks up the message. The `impl Fn(FromWorker)`
                    // parameter type means "any callable that takes a FromWorker" —
                    // closures, function pointers, etc. This is Rust's equivalent
                    // of a callback.
                    let emit = |m: FromWorker| {
                        // `let _ = ...` discards the Result — if the UI is gone
                        // (channel closed), we just drop the message silently
                        // rather than panicking.
                        let _ = tx_res.send(m);
                        ctx.request_repaint();
                    };
                    // `handle()` returns `Some(response)` for request/response
                    // messages, `None` for fire-and-forget (like SetTracing).
                    if let Some(response) = state.handle(msg, &emit) {
                        if tx_res.send(response).is_err() {
                            break; // UI is gone (channel dropped), shut down
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .expect("failed to spawn rumoca-worker thread");

        Worker { tx: tx_req, rx: rx_res, send_failed: false }
    }

    /// Send a request to the worker. Never blocks.
    ///
    /// On failure (worker thread exited), sets `self.send_failed = true` so the
    /// UI can detect the dead worker and show a diagnostic.
    /// `mpsc::Sender::send()` is non-blocking for unbounded channels.
    pub fn send(&mut self, req: ToWorker) {
        if self.tx.send(req).is_err() {
            self.send_failed = true;
            eprintln!("hrw: worker thread has exited — message dropped");
        }
    }
}

/// Worker-thread-owned state: the persistent session and its loaded libraries.
///
/// This struct lives entirely on the worker thread — it is NOT `Send` or `Sync`
/// and never needs to be, because it never crosses a thread boundary. The
/// `Session` inside it is Rumoca's incremental compilation workspace: it
/// persists across compiles so the MSL stays loaded.
///
/// This is intentionally a plain `struct` (not wrapped in `Arc<Mutex<...>>`)
/// because the worker thread is the sole owner. Shared-state concurrency
/// is harder to reason about and unnecessary here.
/// **Public because a scale runner needs it.** `examples/fidelity_msl.rs`
/// compiles MSL models through HRW's own path — which is the thing under test —
/// so it needs the same engine the app uses rather than a second copy. Fields
/// stay private; only `new`, `load_libraries` and `compile_model_by_name` are
/// exposed.
pub struct WorkerState {
    /// The Rumoca incremental compilation session. Persists across compiles —
    /// library source roots (the MSL) are loaded once and reused for every
    /// specimen. Each `compile()` call updates the specimen's document in the
    /// session and re-resolves incrementally.
    session: Session,
    /// URI of the specimen document added by the previous compile, and whether that
    /// compile failed to resolve.
    ///
    /// **Guards against one broken specimen poisoning every later compile.** Name
    /// resolution runs over the *whole session*, not just the requested model, so a
    /// previously-loaded specimen with an unresolved reference makes a perfectly good
    /// model report that other file's error.
    ///
    /// Verified 2026-07-29 with a fresh session and the MSL loaded: `CapacitorLoop`
    /// resolved clean; then `UndefinedRef`; then `CapacitorLoop` again — and the third
    /// compile reported `unresolved component reference: 'missingGain'`, a name that
    /// appears **only** in `UndefinedRef.mo`. Byte-identical error to the second run.
    ///
    /// **`remove_document` does not clear it**, even though
    /// `apply_document_removal_at_revision` calls
    /// `invalidate_resolved_state(CacheInvalidationCause::DocumentRemoval)`. Rebuilding
    /// the session does. The root cause is inside Rumoca's resolved-state cache and is
    /// logged as an upstream issue rather than guessed at; see `docs/ideas.md` #45.
    ///
    /// So the mitigation is the mechanism that was *measured* to work: rebuild the
    /// session, and only after a compile that actually failed to resolve. A clean
    /// specimen cannot poison anything, so the reparse cost is paid exactly when it
    /// buys something.
    last_specimen_uri: Option<String>,
    /// Whether the previous compile failed at resolve — see `last_specimen_uri`.
    last_resolve_failed: bool,
    /// Library roots currently loaded, so a specimen compile knows they're ready.
    libraries: Vec<PathBuf>,
    /// Guard for the thread-local tracing subscriber. This is an RAII guard —
    /// while it exists (`Some`), the `TracingForwarder` subscriber is active on
    /// this thread. When dropped (set to `None`), the subscriber is deactivated.
    /// `tracing::subscriber::set_default()` returns this guard; it's
    /// thread-local (only affects the worker thread), not global.
    tracing_guard: Option<tracing::subscriber::DefaultGuard>,
}

/// Build a logging closure that wraps `emit` with elapsed-time tracking.
/// Both `compile()` and `simulate()` use the same pattern: a local closure
/// that attaches a timestamp to each log entry.
fn make_log<'a>(
    t0: &'a std::time::Instant,
    emit: &'a impl Fn(FromWorker),
) -> impl Fn(LogLevel, String) + 'a {
    move |level, msg| {
        emit(FromWorker::Log(LogEntry {
            elapsed_secs: t0.elapsed().as_secs_f64(),
            level,
            message: msg,
        }));
    }
}

impl Default for WorkerState {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerState {
    pub fn new() -> Self {
        WorkerState {
            session: Session::new(SessionConfig::default()),
            last_specimen_uri: None,
            last_resolve_failed: false,
            libraries: Vec::new(),
            tracing_guard: None,
        }
    }

    /// Dispatch one request. Natural breakpoint site for studying a phase.
    ///
    /// This is the worker's main dispatcher — set a debugger breakpoint here
    /// to intercept any request before it executes. The `match` exhaustively
    /// covers every `ToWorker` variant; if you add a new variant, the compiler
    /// will force you to add a handler here (Rust's exhaustive matching).
    ///
    /// `emit` streams intermediate results (compile-stage progress) ahead of the
    /// returned final result. It's typed as `&impl Fn(FromWorker)` — a reference
    /// to any callable (closure, function pointer) that takes `FromWorker`. The
    /// `&` means we borrow it (don't take ownership); `impl Fn` means the
    /// concrete type is monomorphized at compile time (zero-cost abstraction,
    /// no dynamic dispatch overhead).
    ///
    /// Returns `None` for fire-and-forget messages (SetTracing) that don't
    /// produce a response back to the UI.
    fn handle(&mut self, msg: ToWorker, emit: &impl Fn(FromWorker)) -> Option<FromWorker> {
        match msg {
            ToWorker::SetLibraries(roots) => Some(FromWorker::Libraries(self.load_libraries(roots))),
            ToWorker::Compile(path) => Some(self.compile(&path, emit)),
            ToWorker::CompileLibraryModel(name) => Some(self.compile_model_by_name(&name, emit)),
            ToWorker::OpenDef(name) => Some(self.open_def(&name)),
            ToWorker::Simulate { path, model, t_end } => {
                let result = self.simulate(&path, &model, t_end, emit);
                Some(FromWorker::Simulated { path, result })
            }
            // SetTracing is fire-and-forget: the UI doesn't need a response.
            // `set_default` installs a thread-local tracing subscriber and
            // returns a guard; dropping the guard (setting to `None`)
            // deactivates it. This toggles Rumoca's internal `tracing::debug!`
            // / `tracing::warn!` capture on or off.
            ToWorker::SetTracing(enabled) => {
                if enabled {
                    if self.tracing_guard.is_none() {
                        let subscriber = TracingForwarder;
                        self.tracing_guard =
                            Some(tracing::subscriber::set_default(subscriber));
                    }
                } else {
                    // Dropping the guard deactivates the subscriber. Setting
                    // the field to `None` runs `Drop` on the old `Some` value.
                    self.tracing_guard = None;
                    // **And discard what it already captured.** Turning tracing
                    // off must take effect on the next compile, not eventually.
                    // Without this, events buffered while it was on are still
                    // waiting to be drained, so the first compile after
                    // unchecking reports traces the user has just asked not to
                    // see — the more confusing half of Doug's report, because
                    // the control appears not to work.
                    clear_traces();
                }
                None
            }
        }
    }

    /// Compile the model to its DAE, lower it to a `SolveModel`, and run a
    /// simulation to `t_end` — returning the state trajectories. On this worker
    /// thread; the UI drives it via `ToWorker::Simulate` and never blocks.
    ///
    /// # Three-phase pipeline: Compile → Lower → Integrate
    ///
    /// 1. **Compile** — re-reads the specimen source, runs Rumoca's full pipeline
    ///    (parse → resolve → flatten → structural → ...) to produce a `Dae`
    ///    (Differential-Algebraic Equation system).
    /// 2. **Lower** — transforms the DAE into a `SolveModel` via
    ///    `rumoca_phase_solve::lower_dae_to_solve_model()`. The `SolveModel` is
    ///    the simulator's input: residual programs, variable layout, mass matrix.
    /// 3. **Integrate** — runs the ODE/DAE solver via
    ///    `rumoca_sim::simulate_solve_model()`. The Auto solver picks BDF for
    ///    stiff systems, RK45 otherwise.
    ///
    /// # Why re-compile instead of reusing the `compile()` result?
    ///
    /// The `compile()` method serializes stage results to JSON for the tree
    /// inspector, which loses the typed `Dae` struct. Rather than maintaining a
    /// separate cache of the typed result, we simply re-compile here — it's fast
    /// (hundreds of ms) and keeps the code simpler. The session's incremental
    /// resolution means the MSL isn't re-parsed.
    fn simulate(
        &mut self,
        path: &Path,
        model: &str,
        t_end: f64,
        emit: &impl Fn(FromWorker),
    ) -> Result<SimData, String> {
        use std::time::Instant;
        let t0 = Instant::now();
        let log = make_log(&t0, emit);

        log(LogLevel::Info, format!("simulating {model} to t={t_end}"));

        // --- Phase 1: Compile the model to a DAE ---
        log(LogLevel::StageStart, "Compile (for simulation)".to_owned());
        let t_stage = Instant::now();
        let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
        let uri = path.to_string_lossy().to_string();
        // Remove then re-add the specimen so the session treats it as new —
        // without this, `update_document` sees identical source text and
        // short-circuits, returning cached results (armed breakpoints won't
        // fire and source edits won't take effect on re-simulate).
        self.session.remove_document(&uri);
        self.session.update_document(&uri, &source);
        // Rumoca API: `qualify_model_name` turns a simple name like "BouncingBall"
        // into a fully-qualified name like "BouncingBall" (for top-level models,
        // these are the same, but nested models would differ).
        let qualified = self.session.qualify_model_name(&uri, model);
        // Rumoca API: the main pipeline entry point. It runs parse → resolve →
        // flatten → DAE construction, with error recovery so partial results are
        // available on failure. Returns a `CompileReport` whose `requested_result`
        // is `Option<PhaseResult>`.
        // NOTE the `_uncached_` variant. `compile_model_strict_reachable_with_recovery`
        // consults `CompiledSourceRoot::compile_cache`, an `IndexMap` keyed by model
        // name, and returns the previous `PhaseResult` for any model already compiled
        // in this process. The IR would be identical — but **the phases would not
        // run**, and HRW is an observatory: "watch the compiler work" has to mean the
        // compiler actually worked. Breakpoints in a phase crate then fire exactly
        // once per model per session and never again, which on 2026-07-28 read as a
        // debugger defect and cost four rounds of misdiagnosis (see
        // `docs/architecture.md` § Rumoca's compile cache).
        //
        // The `remove_document`/`update_document` dance above defeats *document*
        // caching; it does not touch this one.
        //
        // Deliberately paying recompile time on every specimen load. Doug, when
        // choosing this: "This project is for learning, not for production
        // performance. Debuggability is of the highest priority."
        let report =
            self.session.compile_model_strict_reachable_uncached_with_recovery(&qualified);
        // Drain any tracing events that Rumoca emitted during compilation.
        drain_traces(&log);
        log(LogLevel::StageEnd, format!("Compile ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
        // Pattern-match on the `PhaseResult` to extract the successful
        // `CompileResult` (which contains the `Dae`), or return an error.
        // The `?`-like early returns use `return Err(...)` because we're in
        // a `match` arm, not a `?`-compatible expression position.
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            Some(PhaseResult::Failed { phase, error, .. }) => {
                log(LogLevel::Error, format!("compile failed at {phase}: {error}"));
                return Err(format!("compile failed at {phase}: {error}"));
            }
            _ => {
                log(LogLevel::Error, "no simulable result".to_owned());
                return Err("the pipeline produced no simulable result for this model".to_owned());
            }
        };

        // --- Phase 2: Lower the DAE to a SolveModel ---
        // Rumoca API: `lower_dae_to_solve_model` transforms the mathematical DAE
        // (equations + variables) into the solver's executable form: residual
        // programs, mass matrix structure, Jacobian sparsity pattern, etc.
        log(LogLevel::StageStart, "Solve lowering".to_owned());
        let t_stage = Instant::now();
        let sm = rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae)
            .map_err(|e| {
                log(LogLevel::Error, format!("solve lowering failed: {e}"));
                format!("solve lowering failed: {e}")
            })?;
        drain_traces(&log);
        log(LogLevel::StageEnd, format!("Solve lowering ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));

        // Check if the model has discrete updates (reinit / when-clause
        // assignments) that cause discontinuous jumps. This flag controls
        // whether the plot breaks its polylines at jumps (via
        // `discontinuity_segments`). A bare zero-crossing without an update
        // does NOT count — the trajectory is still continuous.
        let has_discontinuities =
            !cr.dae.discrete.real_updates.is_empty() || !cr.dae.discrete.valued_updates.is_empty();
        let n_states = cr.dae.variables.states.len();
        let n_eq = cr.dae.continuous.equations.len();
        log(LogLevel::Info, format!("{n_eq} equations, {n_states} states, hybrid={has_discontinuities}"));

        // --- Phase 3: Integrate (run the ODE/DAE solver) ---
        // Rumoca API: `simulate_solve_model` runs the solver (Auto = BDF for
        // stiff / RK45 otherwise) from t=0 to t_end, returning time series.
        // `..Default::default()` fills the remaining `SimOptions` fields
        // (tolerances, max steps, output points) with sensible defaults.
        log(LogLevel::StageStart, "Integration".to_owned());
        let t_stage = Instant::now();
        let opts = rumoca_sim::SimOptions { t_end, ..Default::default() };
        let res = rumoca_sim::simulate_solve_model(&sm, &opts)
            .map_err(|e| {
                drain_traces(&log);
                log(LogLevel::Error, format!("simulation failed: {e}"));
                format!("simulation failed: {e}")
            })?;
        drain_traces(&log);
        log(LogLevel::StageEnd, format!(
            "Integration ({:.1}ms, {} time points)",
            t_stage.elapsed().as_secs_f64() * 1000.0,
            res.times.len(),
        ));

        log(LogLevel::Info, format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0));

        Ok(SimData {
            times: res.times,
            names: res.names,
            data: res.data,
            n_states: res.n_states,
            has_discontinuities,
            solver_steps: res.solver_steps,
        })
    }

    /// Extract a class from the resolved tree by qualified name for navigation.
    ///
    /// When the user clicks a `type_def_id` in the tree inspector, the UI sends
    /// `ToWorker::OpenDef("Modelica.Mechanics.Rotational.Components.Inertia")`.
    /// This method looks up that class in the already-resolved tree and returns
    /// its IR + a fresh `DefIndex` so navigation can continue recursively from
    /// the opened class.
    ///
    /// Rumoca API: `session.resolved()` returns the full resolved `ClassTree`.
    /// `rt.0` is the tree itself (the `.0` accesses the first field of a tuple
    /// struct / tuple). `get_class_by_qualified_name` does a name-based lookup.
    /// The `{e:#}` format uses Rust's "alternate" `Display` which includes the
    /// full error chain (like Python's chained exceptions).
    fn open_def(&mut self, name: &str) -> FromWorker {
        let rt = match self.session.resolved() {
            Ok(rt) => rt,
            Err(e) => return FromWorker::DefTree { name: name.to_owned(), result: Err(format!("{e:#}")) },
        };
        let result = match rt.0.get_class_by_qualified_name(name) {
            Some(class) => {
                let value = ser_value(class);
                // Build a DefId→DefInfo map for all DefIds referenced in this
                // class's IR, so navigation can continue from here.
                let def_index = build_def_index(&rt.0, &value);
                Ok((value, def_index))
            }
            None => Err(format!("`{name}` not found in resolved tree")),
        };
        FromWorker::DefTree { name: name.to_owned(), result }
    }

    /// Rebuild the session from scratch and load each library root as a
    /// durable source set.
    ///
    /// Called once at startup with the MSL paths. Creates a fresh `Session`
    /// (discarding any previously loaded state) and parses each library root
    /// directory into the session. The "durable external" kind means these
    /// source sets persist across specimen compiles — the MSL is parsed once
    /// and reused for every compile, which is why incremental re-resolution
    /// takes ~0.3s instead of re-parsing thousands of library files.
    ///
    /// Rumoca API surface:
    /// - `parse_source_root_with_cache()` — parses a directory tree of `.mo`
    ///   files, with an on-disk cache for speed.
    /// - `source_root_source_set_key()` — generates a stable cache key from
    ///   the path.
    /// - `replace_parsed_source_set()` — loads the parsed documents into the
    ///   session, returning the count of documents loaded.
    /// - `SourceRootKind::DurableExternal` — marks these as library sources
    ///   that outlive any single compile.
    pub fn load_libraries(&mut self, roots: Vec<PathBuf>) -> Result<usize, String> {
        let mut session = Session::new(SessionConfig::default());
        let mut total = 0usize;
        for root in &roots {
            let parsed = parse_source_root_with_cache(root)
                .map_err(|e| format!("{}: {e:#}", root.display()))?;
            let key = source_root_source_set_key(&root.to_string_lossy());
            total += session.replace_parsed_source_set(
                &key,
                SourceRootKind::DurableExternal,
                parsed.documents,
                None,
            );
        }
        self.session = session;
        // A fresh session holds no specimen document and no stale resolved state, so
        // both trackers reset with it.
        self.last_specimen_uri = None;
        self.last_resolve_failed = false;
        self.libraries = roots;
        Ok(total)
    }

    /// Run the full compilation pipeline on a specimen, extracting the user
    /// model's IR at each stage and streaming progress to the UI.
    ///
    /// This is the most complex method in the worker — it orchestrates the
    /// entire Rumoca pipeline and produces the `FromWorker::Compiled` result
    /// that populates all ten stage tabs in the UI.
    ///
    /// # Pipeline stages (in order)
    ///
    /// 1. **Parse** — `rumoca_phase_parse::parse_to_ast()` — source text to AST
    /// 2. **Resolve** — `session.resolved()` — name resolution against the MSL
    /// 3. **Instantiate** — `rumoca_phase_instantiate::instantiate_model()` — class instantiation
    /// 4. **Typecheck** — `rumoca_phase_typecheck::typecheck_instanced()` — type checking
    /// 5. **Flatten** — extracted from the reachable-closure pipeline result
    /// 6. **Structural** — `rumoca_phase_structural::build_structural_report()` — matching + BLT
    /// 7. **Index reduction** — the dummy-derivative funnel
    /// 8. **Initialization** — `build_ic_plan()` — initial-condition solve plan
    /// 9. **Events** — hybrid/event structure extraction
    /// 10. **Solve lowering** — `lower_dae_to_solve_model()` — DAE to SolveModel
    ///
    /// # Progressive streaming pattern
    ///
    /// After each stage, the method sends a `CompileProgress` message with all
    /// stages computed so far (the rest are `Stage::default()` — neutral). This
    /// lets the UI colour tabs green/red as each stage lands, giving real-time
    /// progress feedback during a multi-second compile.
    ///
    /// # Two-phase compilation
    ///
    /// Stages 1-4 (Parse through Typecheck) are run independently with direct
    /// Rumoca API calls. Stages 5-10 come from a single Rumoca pipeline
    /// invocation (`compile_model_strict_reachable_with_recovery`), and the
    /// stage-extraction functions (`flatten_stage`, `structural_stage`, etc.)
    /// pull out individual stages from the combined `PhaseResult`.
    ///
    /// Typecheck is deferred: a clean, model-scoped typecheck needs
    /// instantiation; the pre-instantiation whole-tree typecheck fails
    /// on the full MSL.
    /// Find the document declaring `qualified`, and hand back its source.
    ///
    /// **`ClassDef::location.file_name` is the document URI**, not a bare
    /// basename — verified against `Session::get_document` for models in three
    /// different MSL packages, including one nested 1,498 lines into a
    /// multi-class file. So no path guessing, no scan of 2,553 documents, and no
    /// heuristic name matching (which `docs/identity-and-provenance.md` rules out
    /// anyway).
    fn locate_library_model(&mut self, qualified: &str) -> Result<Located, String> {
        let tree = self
            .session
            .resolved()
            .map_err(|e| format!("cannot resolve the library to find `{qualified}`: {e:#}"))?;
        let class = tree
            .0
            .get_class_by_qualified_name(qualified)
            .ok_or_else(|| format!("`{qualified}` is not a class in the loaded libraries"))?;
        let uri = class.location.file_name.clone();
        // Reported rather than shrugged off: a class whose declaring document the
        // session cannot produce is a broken assumption, not an empty pane.
        let source = self
            .session
            .get_document(&uri)
            .ok_or_else(|| {
                format!("`{qualified}` is declared in `{uri}`, which the session has no document for")
            })?
            .content
            .to_string();
        Ok(Located {
            uri,
            source,
            qualified: Some(qualified.to_owned()),
            decl_line: Some(class.location.start_line),
        })
    }

    /// Compile a **specimen file** — read it from disk, register it with the
    /// session, and take its first class as the model.
    fn compile(&mut self, path: &Path, emit: &impl Fn(FromWorker)) -> FromWorker {
        self.compile_target(CompileTarget::File(path), emit)
    }

    /// Compile a model **already present in a loaded library**, by qualified
    /// name — `Modelica.Electrical.Analog.Basic.Resistor`.
    ///
    /// The entry point Test mode needs to open a row of a report
    /// (`docs/reports.md`), and the one fidelity testing at MSL scale needs:
    /// checking HRW's representation of an MSL model means compiling it
    /// *through HRW's own path*, which is the thing under test.
    ///
    /// # Why this cannot just call [`Self::compile`] with the library file
    ///
    /// Two reasons, and the second is the one that bites. A library file may
    /// declare **many** classes — `Blocks/Continuous.mo` holds
    /// `CriticalDamping` at lines 1498-1620 among others — so "the first class
    /// in the file" is the wrong model. And registering a source-root file as a
    /// workspace document would have the session hold it twice.
    ///
    /// So the document is **located, not added**: `ClassDef::location.file_name`
    /// *is* the document URI, verified against `Session::get_document`.
    pub fn compile_model_by_name(
        &mut self,
        qualified: &str,
        emit: &impl Fn(FromWorker),
    ) -> FromWorker {
        self.compile_target(CompileTarget::Library(qualified), emit)
    }

    fn compile_target(&mut self, target: CompileTarget<'_>, emit: &impl Fn(FromWorker)) -> FromWorker {
        use std::time::Instant;
        let t0 = Instant::now();
        let log = make_log(&t0, emit);

        // **This run shows this run's traces, and no others.**
        //
        // Doug, 2026-08-04: with tracing off, *"detailed rumoca logs are still
        // included in the log view, but for a smaller subset of compiler phases."*
        // Those were **leftovers**. `TRACE_BUFFER` is drained after each Rumoca
        // call, but anything emitted after the last drain of a run stays in it —
        // and the next run's first drain then reports it, whether or not tracing
        // is still on.
        //
        // The same stranding is the other half of Doug's report: traces appearing
        // *"for only a subset of compiler phases"* while tracing is on, because
        // the missing phase's events were not stranded forever — they were
        // arriving one compile late.
        //
        // Discarding here is the structural fix: whatever a future change leaves
        // behind, it can no longer surface under someone else's compile. The
        // matching drains below are what make the events appear in the run that
        // produced them.
        clear_traces();

        // **Say up front which phases cannot speak.** Only when tracing is on,
        // because that is the only time their silence is a question the reader is
        // asking. See `UNINSTRUMENTED_PHASES`.
        if self.tracing_guard.is_some() {
            log(LogLevel::Info, uninstrumented_notice());
        }

        // Start capturing stdout/stderr from Rumoca library calls.
        // Some Rumoca phases print diagnostics via `println!`/`eprintln!` rather
        // than returning them as structured errors. `OutputCapture` intercepts
        // these at the file-descriptor level and forwards them as log entries.
        let mut output_capture = OutputCapture::start();

        // `drain_output` pulls captured stdout/stderr and forwards each line
        // as a log entry. `&dyn Fn(...)` is a *trait object* (dynamic dispatch)
        // — unlike `&impl Fn(...)` (static dispatch), it works across the
        // closure boundary here where the concrete type isn't known.
        let drain_output = |capture: &mut Option<OutputCapture>, log_fn: &dyn Fn(LogLevel, String)| {
            if let Some(cap) = capture.as_mut() {
                let (stdout, stderr) = cap.drain();
                for line in stdout.lines() {
                    if !line.is_empty() {
                        log_fn(LogLevel::Stdout, line.to_owned());
                    }
                }
                for line in stderr.lines() {
                    if !line.is_empty() {
                        log_fn(LogLevel::Stderr, line.to_owned());
                    }
                }
            }
        };

        log(
            LogLevel::Info,
            match target {
                CompileTarget::File(p) => format!(
                    "compiling {}",
                    p.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                ),
                CompileTarget::Library(name) => format!("compiling library model {name}"),
            },
        );

        // Where the source comes from, and what to name the model, are the ONLY
        // things the two targets disagree about. Everything downstream is shared.
        let located = match target {
            CompileTarget::File(path) => std::fs::read_to_string(path)
                .map(|source| Located {
                    uri: path.to_string_lossy().to_string(),
                    source,
                    // Derived from the parse below: a specimen names its own model.
                    qualified: None,
                    // A specimen file opens at its own model; nothing to skip past.
                    decl_line: None,
                })
                .map_err(|e| format!("read error: {e}")),
            CompileTarget::Library(name) => self.locate_library_model(name),
        };
        let Located { uri, source, qualified: given_qualified, decl_line } = match located {
            Ok(l) => l,
            Err(msg) => {
                drop(output_capture.take());
                return FromWorker::Compiled {
                    path: PathBuf::from(match target {
                        CompileTarget::File(p) => p.to_string_lossy().to_string(),
                        CompileTarget::Library(n) => n.to_owned(),
                    }),
                    model: None,
                    stages: StageBundle {
                        parse: Stage::err(msg),
                        ..Default::default()
                    },
                    def_index: BTreeMap::new(),
                    equation_sheet: None,
                    identifier_index: None,
                    index_reduction_frames: Vec::new(),
                    pre_lowering_frames: Vec::new(),
                    connection_frames: Vec::new(),
                    flat: None,
                    dae: None,
                    // Nothing was located, so there is no source to show.
                    library_source: None,
                };
            }
        };
        // **What this compile calls itself, back to the UI.**
        //
        // For a specimen that is the file path. For a **library model it must be
        // the qualified name**, not the document URI — the UI's three staleness
        // checks compare the result's `path` against `App::selected`, which holds
        // the qualified name because a library model has no file of its own (its
        // package file may declare many classes).
        //
        // **This was a real bug on 2026-08-01.** The early-error return above
        // already reported the qualified name; this success path reported the MSL
        // *file* URI, so the two disagreed. Every successful library compile was
        // then discarded as stale: the log showed the work happening, no stage
        // ever landed, and the spinner span forever. The worker was inconsistent
        // with itself, and only the success path was wrong.
        let report_path = match target {
            CompileTarget::File(p) => p.to_path_buf(),
            CompileTarget::Library(n) => PathBuf::from(n),
        };
        // **Read from disk, not from `source`, and this is not an oversight.**
        //
        // Rumoca stores source-root documents with **empty content** —
        // `Document::new(uri, String::new(), SyntaxFile::from_parsed(parsed))` in
        // `session_impl_source_roots.rs`. A library keeps its parsed AST and
        // discards its text, which across 2,553 MSL documents is the right call
        // for a compiler. So `Located::source` is `""` for every library model,
        // and `session.get_document(uri).content` cannot supply the pane.
        //
        // **The URI is a filesystem path** (`collect_modelica_files` walked it),
        // and Rumoca's parsed-artifact cache is keyed on a hash of those files,
        // so a file that changed since parsing would have invalidated the cache.
        // Disk text and parsed text therefore agree by construction.
        //
        // Deliberately **not** fixed by giving `Located::source` the real text:
        // that feeds `parse_to_ast` below, so it would change what the compile
        // does for every library model and invalidate the 2,614/2,626 fidelity
        // measurement. This capture is observation-only.
        let library_source = matches!(target, CompileTarget::Library(_)).then(|| LibrarySource {
            uri: uri.clone(),
            // The error is carried, not swallowed: a blank pane is exactly the
            // ambiguity this whole change exists to remove.
            text: std::fs::read_to_string(&uri).map_err(|e| format!("cannot read {uri}: {e}")),
            decl_line,
        });
        // **The text everything user-facing is resolved against.**
        //
        // `IdentifierIndex::build` and `equation_sheet::build` turn each
        // variable's `source_span` byte offset into a line number by counting
        // newlines in the text they are handed. For a library model `source` is
        // `""` (see above), so every offset collapsed to **line 1** -- the index
        // was not empty, it was *wrong*, and no identifier in an MSL model was
        // clickable anywhere except the first line.
        //
        // Must be the same bytes the pane renders, or a span found here lands on
        // a different line there. Both now come from `library_source`.
        let display_source: &str = library_source
            .as_ref()
            .and_then(|l| l.text.as_ref().ok())
            .map(String::as_str)
            .unwrap_or(&source);

        // =====================================================================
        // Stage 1: PARSE — source text to AST
        // =====================================================================
        // Rumoca API: `parse_to_ast(source, file_name)` parses Modelica source
        // into an AST. Returns `Ok(StoredDefinition)` or `Err` on syntax error.
        // We grab the first class name from `ast.classes` as the model name.
        //
        // **`&uri`, not the basename.** The `file_name` argument is stamped into
        // every `Location` in the resulting AST, and the session parsed these same
        // bytes with the full document URI. Passing a basename made HRW's parse
        // differ from the compiler's own AST for the file — measured 2026-08-01 at
        // **400 of 400** MSL documents, and **0 of 400** once the URI was passed.
        // The bytes and every span already agreed; this one field did not.
        //
        // It is not cosmetic. `bridge::slice_source` resolves a location to a file
        // by trying `file_name` as a path and falling back to the specimen path.
        // A basename is not a path, and for a **library model the fallback is a
        // qualified name**, so pointing at a Parse node emitted no excerpt at all.
        // Worse, `Path::new("Resistor.mo").is_file()` is a *relative* test: run
        // from a directory holding a same-named file, it would have sliced the
        // wrong one and emitted a confident wrong excerpt.
        log(LogLevel::StageStart, "Parse".to_owned());
        let t_stage = Instant::now();
        // **`display_source`, not `source`.** For a library model `source` is `""`
        // -- Rumoca discards source-root text -- so this parsed an empty string
        // and produced `{"classes":{},"within":null}` for **every MSL model**,
        // shown as a *successful* stage. An empty green tab asserts "this model
        // parsed to nothing", which is false, and is indistinguishable from a
        // model that genuinely declares nothing. Worse than an error, which at
        // least points somewhere. Fixed 2026-08-01 at Doug's request, after the
        // source view made the disagreement visible: the pane showed a file full
        // of declarations while the Parse tab claimed it held none.
        //
        // This is display-only for a library model. The pipeline below works from
        // the session's resolved tree, never from this AST, so the stage now
        // reports what Rumoca would parse without changing what it compiles.
        //
        // The **whole declaring file** is parsed, matching what the source view
        // shows. A library file declares many classes and the reader is looking
        // at all of them.
        let (parse, model) = match rumoca_phase_parse::parse_to_ast(display_source, &uri) {
            Ok(ast) => {
                // For a specimen, `ast.classes.keys().next()` is the model: the
                // file declares one. For a **library** model the name was given,
                // and taking the file's first class would be wrong — a library
                // file commonly declares many, and the requested one is rarely
                // the first.
                let model = match &given_qualified {
                    Some(q) => q.rsplit('.').next().map(str::to_owned),
                    None => ast.classes.keys().next().cloned(),
                };
                (Stage::from_ser(&ast), model)
            }
            Err(e) => {
                let msg = format!("{e:#}");
                // **A library model keeps its name through a parse failure.**
                //
                // `None` here stops the pipeline dead: the big `match &model`
                // below yields no stages at all. That was harmless while this
                // parsed `""` and could not fail -- feeding it a real 60 KB file
                // makes failure possible for the first time, and a model that
                // compiles perfectly through the session would have gone blank
                // because a *display* stage could not parse its declaring file.
                //
                // The name was supplied by the caller, so a parse of the file has
                // no bearing on it. Only a specimen, whose name comes from the
                // parse, genuinely loses it.
                let model = given_qualified
                    .as_ref()
                    .and_then(|q| q.rsplit('.').next().map(str::to_owned));
                (Stage::err_with_details(serde_json::json!({
                    "kind": "parse",
                    "message": msg,
                    "guidance": "Check the Modelica source for syntax errors.",
                }), msg), model)
            }
        };
        // After each Rumoca API call, drain any `tracing` events that were
        // buffered by our `TracingForwarder` subscriber.
        drain_traces(&log);
        drain_output(&mut output_capture, &log);
        log(LogLevel::StageEnd, format!("Parse ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
        // Stream the first progress snapshot: Parse is done, everything else
        // is `Stage::default()` (neutral). `..Default::default()` fills the
        // remaining struct fields with their default values — a Rust pattern
        // called "struct update syntax".
        emit(FromWorker::CompileProgress {
            path: report_path.clone(),
            stages: StageBundle { parse: parse.clone(), ..Default::default() },
        });

        // =====================================================================
        // Stage 2: RESOLVE — name resolution against the MSL
        // =====================================================================
        // Resolution merges the specimen's AST with the loaded MSL library,
        // resolving all names (component types, extends clauses, etc.) to their
        // definitions in the class tree.
        log(LogLevel::StageStart, "Resolve".to_owned());
        let t_stage = Instant::now();
        // A previous compile that failed to resolve leaves errors in the session's
        // resolved-state cache that `remove_document` does not clear, so a good model
        // compiled next reports the *broken* one's error. Rebuilding the session is the
        // only mechanism measured to clear it. See `last_specimen_uri` for the
        // reproduction and the upstream note.
        //
        // Guarded on the previous compile having actually failed: a clean specimen
        // poisons nothing, so the MSL reparse is paid only when it buys correctness.
        // **Only a specimen is registered.** A library model's document is
        // already in a durable source root; adding it as a workspace document
        // would have the session hold the same file twice, and removing it later
        // would evict part of the library.
        if given_qualified.is_none() {
            if self.last_resolve_failed && self.last_specimen_uri.as_deref() != Some(uri.as_str()) {
                let roots = self.libraries.clone();
                log(
                    LogLevel::Info,
                    "rebuilding session (previous specimen failed to resolve)".to_owned(),
                );
                if let Err(e) = self.load_libraries(roots) {
                    log(LogLevel::Warn, format!("session rebuild failed: {e}"));
                }
            }
            if let Some(prev) = self.last_specimen_uri.take()
                && prev != uri
            {
                self.session.remove_document(&prev);
            }
            // Remove then re-add the specimen so the session treats it as new — without
            // this, `update_document` sees identical source text and short-circuits,
            // returning cached results (the registration code never re-runs).
            self.session.remove_document(&uri);
            self.session.update_document(&uri, &source);
            self.last_specimen_uri = Some(uri.clone());
        }
        let mut def_index = BTreeMap::new();
        let mut instantiate = Stage::default();
        let mut typecheck = Stage::default();
        let mut connection_frames = Vec::new();
        let resolve = match &model {
            None => Stage::err("parse produced no model to resolve"),
            Some(simple_name) => {
                // A library model was named in full by the caller. Qualifying its
                // simple name against the library file's URI would re-derive it,
                // and a file declaring several packages could re-derive it wrong.
                let qualified = match &given_qualified {
                    Some(q) => q.clone(),
                    None => self.session.qualify_model_name(&uri, simple_name),
                };
                // Rumoca API: `session.resolved()` triggers (or reuses) full
                // name resolution and returns the resolved `ClassTree`.
                match self.session.resolved() {
                    Ok(rt) => {
                        // Extract just this model's class definition from the
                        // full resolved tree (which includes the entire MSL).
                        let stage = extract_class(&rt.0, &qualified);
                        if let Some(v) = &stage.value {
                            // Build the DefId→DefInfo lookup for all DefIds
                            // referenced in this model's IR.
                            def_index = build_def_index(&rt.0, v);
                        }
                        // Stages 3+4 (Instantiate + Typecheck) piggyback on the
                        // resolved tree — they need it to resolve component types
                        // and dimensions.
                        log(LogLevel::StageStart, "Instantiate + Typecheck".to_owned());
                        let t_sub = Instant::now();
                        let (i, t) = instantiate_and_typecheck(&rt.0, &qualified, &source);
                        // Connection expansion (MLS §9) records its own replay.
                        // It re-runs flatten, so it is timed inside this block
                        // rather than pretending to be free.
                        connection_frames = record_connection_frames(&rt.0, &qualified);
                        log(LogLevel::StageEnd, format!("Instantiate + Typecheck ({:.1}ms)", t_sub.elapsed().as_secs_f64() * 1000.0));
                        instantiate = i;
                        typecheck = t;
                        stage
                    }
                    Err(e) => {
                        // Resolution failed. Show the error, but try to show a
                        // best-effort tree from the cache if one exists (e.g.
                        // from a previous successful compile). `resolved_cached()`
                        // returns the last good result without re-resolving.
                        let note = format!("{e:#}");
                        // Model-scoped, structured diagnostics rather than only the
                        // concatenated anyhow chain. `compile_model_diagnostics` returns
                        // real `Diagnostic`s with severities and labels, so the model's
                        // own error can be separated from the library warnings that
                        // otherwise bury it — see `model_diagnostics_to_json`.
                        //
                        // `note` is still emitted verbatim as `message`: never lossy.
                        let diag = model_diagnostics_to_json(
                            &self.session.compile_model_diagnostics(&qualified).diagnostics,
                            &source,
                        );
                        let resolve_err = |note: &str| serde_json::json!({
                            "kind": "resolve",
                            "message": note,
                            "diagnostics": diag,
                            "guidance": "Name resolution binds every reference to a definition.                                 Read `diagnostics.errors` first: those are this model's problems,                                 each with the source location of the reference that failed.                                 `diagnostics.warnings` are library-level and almost never the                                 cause.",
                        });
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(&rt.0, &qualified) {
                                Stage { value: Some(v), .. } => {
                                    def_index = build_def_index(&rt.0, &v);
                                    Stage::recovered(v, note)
                                }
                                _ => Stage::err_with_details(resolve_err(&note), note),
                            },
                            None => Stage::err_with_details(resolve_err(&note), note),
                        }
                    }
                }
            }
        };

        drain_traces(&log);
        drain_output(&mut output_capture, &log);
        log(LogLevel::StageEnd, format!("Resolve ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
        emit(FromWorker::CompileProgress {
            path: report_path.clone(),
            stages: StageBundle {
                parse: parse.clone(),
                resolve: resolve.clone(),
                instantiate: instantiate.clone(),
                typecheck: typecheck.clone(),
                ..Default::default()
            },
        });

        // Remember a resolve failure so the *next* compile rebuilds the session before
        // trusting it — see `last_resolve_failed`. Set here rather than inside the match
        // above so a recovered-from-cache resolve still counts as failed: the session's
        // resolved state is poisoned either way.
        self.last_resolve_failed = resolve.note_is_error();

        // =====================================================================
        // Stages 5-11: Flatten → DAE construction → Solve lowering
        // =====================================================================
        // **Flatten and DAE construction happen inside one Rumoca call**
        // (`compile_model_strict_reachable_with_recovery`); their `*_stage()`
        // functions only *extract* the results from the returned `PhaseResult`.
        // Structural analysis, index reduction, initialization, events and solve
        // lowering are run here, so their timings are real work.
        //
        // That difference is why the call is bracketed separately in the log. It
        // used to be bracketed as a "DAE pipeline" spanning flatten → solve
        // lowering, which named a phase that is not a pipeline and claimed five
        // phases it does not contain (fixed 2026-08-04).
        //
        // Each stage emits a `CompileProgress` so its tab colours in the UI
        // as soon as it's known.
        //
        // The return type is a 6-tuple — Rust's way of returning multiple
        // values without defining a struct. Destructured immediately via
        // `let (flatten, structural, ...) = match ...`.
        let (flatten, dae_stage, structural, index_reduction, initialization, events, solve_lowering, equation_sheet, identifier_index, ir_frames, compiled_dae, pre_frames, compiled_flat) = match &model {
            None => {
                let e = "parse produced no model to compile";
                (Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), None, None, Vec::new(), None, Vec::new(), None)
            }
            Some(simple_name) => {
                // A library model was named in full by the caller. Qualifying its
                // simple name against the library file's URI would re-derive it,
                // and a file declaring several packages could re-derive it wrong.
                let qualified = match &given_qualified {
                    Some(q) => q.clone(),
                    None => self.session.qualify_model_name(&uri, simple_name),
                };

                // **Named for what this call does, not for a phase.** It logged
                // "DAE pipeline (flatten → solve lowering)" until 2026-08-04 —
                // Doug: *"our logs contain a fiction about a DAE pipeline which
                // includes the phases which follow the DAE phase."* Two things
                // were wrong with it: DAE construction is a **phase**, not a
                // pipeline, and the span it claimed reached five phases past it.
                //
                // The bracket is kept because it is the only honest timing in
                // this block: flatten and DAE construction really do happen
                // inside this one call, so the per-stage figures below are
                // *extraction* time for those two and real work for the rest.
                log(
                    LogLevel::StageStart,
                    "Rumoca compile \u{2014} flatten and DAE construction".to_owned(),
                );
                let t_compile = Instant::now();
                // Uncached, for the reason spelled out in `simulate` above: a
                // cached result means the phases did not run, so nothing can be
                // observed happening — breakpoints, tracing, or timing.
                let report = self
                    .session
                    .compile_model_strict_reachable_uncached_with_recovery(&qualified);
                drain_traces(&log);
                drain_output(&mut output_capture, &log);
                // Closed here, where the call returns. The old bracket closed
                // after solve lowering, which is what let it claim to span
                // phases it had nothing to do with.
                log(
                    LogLevel::StageEnd,
                    format!(
                        "Rumoca compile ({:.1}ms)",
                        t_compile.elapsed().as_secs_f64() * 1000.0
                    ),
                );
                // **Say which of the timings below are real.** Flatten and DAE
                // construction ran inside the call that just ended, so their
                // stage entries time an *extraction* — `DAE construction (0.1ms)`
                // would otherwise read as the phase being nearly free when it is
                // part of a second-long call. Same family as the "DAE pipeline"
                // fiction removed 2026-08-04: a log that states a number the
                // reader will misread is not reporting, it is misreporting.
                log(
                    LogLevel::Info,
                    "Flatten and DAE construction ran inside that call \u{2014} their \
                     times below are extraction only; the later stages are real work"
                        .to_owned(),
                );

                let result = report.requested_result.as_ref();

                let eq_sheet = match result {
                    Some(PhaseResult::Success(cr)) => {
                        Some(crate::equation_sheet::build(
                            &cr.dae,
                            Some((&uri, display_source)),
                        ))
                    }
                    _ => None,
                };

                let id_index = match result {
                    Some(PhaseResult::Success(cr)) => {
                        Some(crate::identifier_index::IdentifierIndex::build(
                            &cr.dae, &uri, display_source,
                        ))
                    }
                    _ => None,
                };

                let mut bundle = StageBundle {
                    parse: parse.clone(),
                    resolve: resolve.clone(),
                    instantiate: instantiate.clone(),
                    typecheck: typecheck.clone(),
                    ..Default::default()
                };

                // Macro capturing `log`, `drain_traces`, `bundle`, `emit`,
                // and `path` from the enclosing scope. Each invocation: logs
                // start/end with timing, runs the extraction function, stores
                // the result on the bundle, and emits a progress update.
                macro_rules! run_stage {
                    ($name:expr, $extract:expr, $field:ident) => {{
                        log(LogLevel::StageStart, $name.to_owned());
                        let t = Instant::now();
                        let stage = $extract;
                        drain_traces(&log);
                        log(LogLevel::StageEnd, format!(
                            "{} ({:.1}ms)", $name, t.elapsed().as_secs_f64() * 1000.0
                        ));
                        bundle.$field = stage.clone();
                        emit(FromWorker::CompileProgress {
                            path: report_path.clone(), stages: bundle.clone(),
                        });
                        stage
                    }};
                }

                let flatten = run_stage!("Flatten", flatten_stage(result, &source), flatten);

                // **DAE construction, logged in its true position.** Until
                // 2026-08-04 this stage was built *after* solve lowering and
                // never logged at all — so the log showed the chain jumping
                // Flatten → Structural, with the phase they both depend on
                // missing. Doug found it walking the tour that teaches it.
                //
                // Moved here rather than logged where it stood: logging it in
                // place would have reported DAE construction *finishing after*
                // the five phases that consume its output, which is a second
                // fiction in place of the first.
                let dae = match result {
                    Some(PhaseResult::Success(cr)) => Some(cr.dae.clone()),
                    _ => None,
                };
                // **The DAE stage.** `rumoca-ir-dae` is a boundary IR like
                // `rumoca-ir-flat`, and `Dae` implements `Serialize`, so this is
                // the same one-liner every other stage uses — there was never
                // anything to build, only a tab nobody had added.
                //
                // Its note is the balance, because that is the claim DAE
                // construction makes and the one everything downstream relies
                // on: matching cannot assign one equation per unknown unless the
                // counts agree.
                //
                // **And when there is no DAE, this stage says why itself.**
                // `flatten_stage` has carried the `FailedPhase::ToDae` error
                // since 2026-07-29, which was right when Flatten was the last
                // tab before Structural — it was the only place left to put it.
                // With a DAE tab that attribution became actively misleading:
                // **Flatten succeeded.** The phase that failed is this one, and
                // on 2026-08-03 it was the only stage in the pipeline rendering
                // a blank tab for its own failure while `structural`,
                // `index_reduction`, `initialization`, `events` and
                // `solve_lowering` all correctly read "not reached (ToDae failed
                // earlier)". The stage whose failure it was said the least.
                //
                // Deliberately **additive**: Flatten keeps its copy. Two tabs
                // explaining the same stop is redundant; a learner opening the
                // DAE tab of a model with no DAE and finding nothing is a dead
                // end, and the tour that found this
                // (`docs/fixture-tours/dae-construction.md`) walks exactly that
                // path.
                let dae_stage = run_stage!(
                    "DAE construction",
                    match &dae {
                        Some(d) => {
                            let n_x = d.variables.states.len();
                            let n_y = d.variables.algebraics.len();
                            let n_eq = d.continuous.equations.len();
                            let mut s = Stage::from_ser(d);
                            s.note = Some(format!(
                                "{n_x} state(s), {n_y} algebraic(s), {n_eq} continuous equation(s)",
                            ));
                            s
                        }
                        None => dae_absent_stage(result, &source),
                    },
                    dae
                );

                let structural = run_stage!("Structural analysis", structural_stage(result, &source), structural);
                let (index_reduction, ir_frames) = {
                    log(LogLevel::StageStart, "Index reduction".to_owned());
                    let t = Instant::now();
                    let (stage, frames) = index_reduction_stage(result, &source);
                    drain_traces(&log);
                    log(LogLevel::StageEnd, format!(
                        "Index reduction ({:.1}ms)", t.elapsed().as_secs_f64() * 1000.0
                    ));
                    bundle.index_reduction = stage.clone();
                    emit(FromWorker::CompileProgress {
                        path: report_path.clone(), stages: bundle.clone(),
                    });
                    (stage, frames)
                };
                let initialization = run_stage!("Initialization", initialization_stage(result), initialization);
                let events = run_stage!("Events", events_stage(result), events);
                let solve_lowering = run_stage!("Solve lowering", solve_lowering_stage(result), solve_lowering);

                // `pre()`-lowering replay frames (idea #40).
                //
                // Re-runs **DAE construction** over the flat model rather than
                // the pass alone. The pass runs *inside* construction, so the
                // DAE above already has its `__pre__` slots and no `pre()` calls
                // left — replaying the pass on it would produce nothing. The
                // flat model is the last artifact from before it ran.
                //
                // The rebuilt DAE is discarded: this is purely observation, and
                // the compile's own result is what every other stage shows.
                let (pre_frames, flat) = match result {
                    Some(PhaseResult::Success(cr)) => {
                        let frames = std::cell::RefCell::new(Vec::new());
                        let _ = rumoca_phase_dae::to_dae_with_options_traced(
                            &cr.flat,
                            Default::default(),
                            Some(&|f: &rumoca_phase_dae::PreLoweringFrame| {
                                frames.borrow_mut().push(f.clone());
                            }),
                        );
                        (frames.into_inner(), Some(cr.flat.clone()))
                    }
                    _ => (Vec::new(), None),
                };
                // **The drain this call never had.** `to_dae_with_options_traced`
                // re-runs the whole of DAE construction, so it is one of the
                // noisiest tracing sources in the pipeline — and it was the last
                // Rumoca call in the compile, with nothing after it to drain.
                // Every event it emitted was therefore stranded and reported
                // against the *next* compile, which is exactly the "logs for a
                // subset of phases" and "logs after unchecking" Doug saw.
                drain_traces(&log);

                (flatten, dae_stage, structural, index_reduction, initialization, events, solve_lowering, eq_sheet, id_index, ir_frames, dae, pre_frames, flat)
            }
        };

        // Restore stdout/stderr by dropping the OutputCapture.
        // `output_capture.take()` moves the value out of the `Option`,
        // returning `Some(capture)`, and `drop()` runs its `Drop` impl
        // which restores the original file descriptors via `dup2`.
        drop(output_capture.take());
        log(LogLevel::Info, format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0));

        // Build and return the final `Compiled` message with every stage.
        FromWorker::Compiled {
            path: report_path,
            model,
            stages: StageBundle {
                parse,
                resolve,
                instantiate,
                typecheck,
                flatten,
                dae: dae_stage,
                structural,
                index_reduction,
                initialization,
                events,
                solve_lowering,
            },
            def_index,
            equation_sheet,
            identifier_index,
            index_reduction_frames: ir_frames,
            pre_lowering_frames: pre_frames,
            connection_frames,
            library_source,
            flat: compiled_flat,
            dae: compiled_dae,
        }
    }
}

/// Compile a specimen through every pipeline stage with the given library roots,
/// headlessly — the exact path the worker thread runs, minus the thread/channel.
///
/// Used by `examples/gen_trace` (trace-log generation) and tests, so their output
/// is byte-identical to what the running app produces. Returns the `Compiled`
/// result, or an error if the libraries fail to load.
///
/// The `&|_: FromWorker| {}` is a no-op closure — it receives and ignores all
/// streaming messages (progress, logs) since there's no UI to display them.
/// The `_` means "ignore this parameter" in Rust.
pub fn compile_specimen(specimen: &Path, libraries: Vec<PathBuf>) -> Result<FromWorker, String> {
    let mut state = WorkerState::new();
    state.load_libraries(libraries)?;
    Ok(state.compile(specimen, &|_: FromWorker| {}))
}

/// Simulate a specimen headlessly, returning the trajectory data.
/// Used by `examples/gen_trace` for writing simulation traces.
/// Same as `compile_specimen` — creates a fresh WorkerState, loads libraries,
/// and runs `simulate()` with a no-op emit closure.
pub fn simulate_specimen(
    specimen: &Path,
    model: &str,
    t_end: f64,
    libraries: Vec<PathBuf>,
) -> Result<SimData, String> {
    let mut state = WorkerState::new();
    state.load_libraries(libraries)?;
    state.simulate(specimen, model, t_end, &|_: FromWorker| {})
}

/// Structural analysis of the model's DAE: maximum matching, BLT blocks,
/// and tearing, from `build_structural_report`, plus the raw incidence matrix
/// (equation x unknown bipartite adjacency) from `build_incidence`. Only available
/// on a full Success (the DAE must exist). The report types aren't `Serialize`,
/// so we build JSON manually via `structural_to_json` / `incidence_to_json`.
///
/// # The `PhaseResult` matching pattern
///
/// Every stage-extraction function follows the same pattern: match on
/// `Option<&PhaseResult>` to handle the four possible outcomes:
/// - `Some(Success(cr))` — the pipeline succeeded; extract the stage from `cr`
/// - `Some(Failed { phase, .. })` — an earlier phase failed; show which one
/// - `Some(NeedsInner { .. })` — the model needs inner declarations (rare)
/// - `None` — the pipeline produced no result at all
///
/// This pattern appears in every `*_stage()` function below. The match arms
/// are exhaustive (Rust enforces this), so if Rumoca adds a new `PhaseResult`
/// variant, every extraction function will get a compile error until updated.
///
/// Rumoca API: `build_structural_report(&dae)` runs maximum matching +
/// BLT decomposition. `build_incidence(&dae)` builds the equation x unknown
/// bipartite adjacency matrix (which equations reference which unknowns).
/// If the pipeline result is a non-success variant (failed, needs inner, or
/// absent), return the appropriate placeholder Stage. Returns `None` when
/// the result is `Success` — the caller should handle that case.
fn not_reached_stage(result: Option<&PhaseResult>) -> Option<Stage> {
    match result {
        Some(PhaseResult::Success(_)) => None,
        Some(PhaseResult::Failed { phase, .. }) => {
            Some(Stage::info(format!("not reached ({phase} failed earlier)")))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Some(Stage::info("not reached (model needs inner declarations)"))
        }
        None => Some(Stage::err(
            "the reachable-closure pipeline produced no result for this model",
        )),
    }
}

fn unwrap_success(result: Option<&PhaseResult>) -> &CompilationResult {
    match result {
        Some(PhaseResult::Success(cr)) => cr,
        _ => unreachable!("not_reached_stage handles non-Success"),
    }
}

/// Serialize diagnostics **with their labels resolved to source locations**.
///
/// `rumoca_core::Diagnostic` carries `labels: Vec<Label>`, each a `Span` plus an optional
/// message marking exactly where the error is ("equation assignment here", "add `pure` or
/// `impure` to the function declaration"). Every diagnostic emitter in HRW dropped
/// `labels` until 2026-07-29 — the same species as the dropped structural spans, on the
/// phases a hand-authored model fails in most.
///
/// A label whose span points into a *library* file resolves to `null`: the offsets are
/// into that file, not the specimen, and `span_to_location` refuses rather than inventing
/// a line. That is why MSL warnings carry no location here and the model's own error does.
fn diagnostics_to_json(diags: &[rumoca_core::Diagnostic], source: &str) -> serde_json::Value {
    serde_json::Value::Array(
        diags
            .iter()
            .map(|d| {
                serde_json::json!({
                    "severity": format!("{:?}", d.severity),
                    "code": d.code,
                    "message": d.message,
                    "notes": d.notes,
                    "labels": d.labels.iter().map(|l| serde_json::json!({
                        "message": l.message,
                        "primary": l.primary,
                        "location": span_to_location(source, &l.span),
                    })).collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// Split a model's diagnostics into the signal and the noise.
///
/// **Why this exists.** The resolve stage used to emit `format!("{e:#}")` — about 39
/// semicolon-separated items of which ~38 were MSL deprecation warnings ("external
/// function 'constructor' should declare `pure` or `impure`", "Evaluate=true on 'factor'
/// has no effect"), with the model's actual error *last*. The signal was the final 2% of
/// a 2000-character string.
///
/// **`severity` does the classification, so nothing is guessed.** No pattern-matching on
/// message text: `DiagnosticSeverity::Error` is the model's problem and everything else
/// is context. Measured on `UndefinedRef`: 34 diagnostics in, **one** error out —
/// `ER002 unresolved component reference: 'missingGain'`, with an in-range span.
///
/// Warnings are kept but **deduplicated and counted** rather than listed: the same MSL
/// deprecation repeats dozens of times and repeating it dozens of times helps nobody.
fn model_diagnostics_to_json(
    diags: &[rumoca_core::Diagnostic],
    source: &str,
) -> serde_json::Value {
    let (errors, warnings): (Vec<_>, Vec<_>) = diags
        .iter()
        .cloned()
        .partition(|d| matches!(d.severity, rumoca_core::DiagnosticSeverity::Error));

    let mut seen = std::collections::BTreeMap::<String, usize>::new();
    for w in &warnings {
        *seen.entry(w.message.clone()).or_insert(0) += 1;
    }
    let distinct: Vec<serde_json::Value> = seen
        .into_iter()
        .map(|(message, count)| serde_json::json!({ "message": message, "occurrences": count }))
        .collect();

    serde_json::json!({
        "errors": diagnostics_to_json(&errors, source),
        "warnings": {
            "note": "library-level warnings, deduplicated. Almost always about the MSL rather \
                     than the model, and not the reason the compile failed.",
            "total": warnings.len(),
            "distinct": distinct,
        },
    })
}

/// Structured JSON for a DAE-construction (`ToDae`) failure.
///
/// The balance figures are **parsed out of the message** because `rumoca-compile`
/// stringifies the typed error at its boundary (`error: format!("{error}")` in
/// `compile_support.rs`), so `ToDaeError::Unbalanced { equations, unknowns, balance }`
/// is gone by the time HRW sees it. Preserving the type through that boundary is the
/// better fix and is logged as an upstream candidate; parsing is what is available now.
///
/// **The parse can only ever be absent, never wrong.** Structured fields appear only on
/// an unambiguous match, and `message` is always emitted verbatim, so a Rumoca wording
/// change loses the extras and never invents a number. That discipline was earned the
/// hard way by the `rank_deficiency` bug: a wrong number reads as authoritative.
/// `an_unbalanced_model_reports_its_balance` fails loudly if the wording moves, rather
/// than letting it degrade in silence.
fn dae_construction_error_to_json(
    error: &str,
    error_code: &Option<String>,
    diagnostics: &[rumoca_core::Diagnostic],
    source: &str,
) -> serde_json::Value {
    let mut json = serde_json::json!({
        "kind": "dae_construction",
        "message": error,
        "error_code": error_code,
        "guidance": "DAE construction turns the flat model into equations plus unknowns \
            and checks that the two counts agree (MLS \u{00a7}4.9). An unbalanced model is \
            usually a declared variable with no equation to determine it, or one equation \
            too many. This check runs *before* structural analysis, so a missing equation \
            is reported here rather than as a structural singularity.",
    });
    let obj = json.as_object_mut().expect("built as an object");

    if !diagnostics.is_empty() {
        // Shared helper so labels resolve to source lines here as well.
        obj.insert("diagnostics".to_owned(), diagnostics_to_json(diagnostics, source));
    }

    if let Some((n_eq, n_unk, balance)) = parse_unbalanced(error) {
        obj.insert("n_equations".to_owned(), n_eq.into());
        obj.insert("n_unknowns".to_owned(), n_unk.into());
        obj.insert("balance".to_owned(), balance.into());
        // Which *direction* the imbalance runs is the actionable half, and it is not
        // obvious from a signed number alone.
        obj.insert(
            "reading".to_owned(),
            serde_json::json!(if balance < 0 {
                "fewer equations than unknowns \u{2014} some variable has nothing to determine it"
            } else {
                "more equations than unknowns \u{2014} something is determined twice"
            }),
        );
    }
    json
}

/// Pull `(equations, unknowns, balance)` out of Rumoca's unbalanced-model message.
///
/// Matches `"unbalanced model: {e} equations, {u} unknowns (balance = {b})"`. Returns
/// `None` on any deviation — see the caller on why absent beats wrong.
fn parse_unbalanced(message: &str) -> Option<(usize, usize, i64)> {
    let rest = message.strip_prefix("unbalanced model: ")?;
    let (eq, rest) = rest.split_once(" equations, ")?;
    let (unk, rest) = rest.split_once(" unknowns (balance = ")?;
    let bal = rest.strip_suffix(')')?;
    Some((eq.trim().parse().ok()?, unk.trim().parse().ok()?, bal.trim().parse().ok()?))
}

/// Turn a Rumoca `Span` into a source location Claude can quote back at Doug.
///
/// A span is byte offsets into the specimen source. On its own that is useless in an
/// answer — "unknown `gnd.p.i`" tells Doug nothing about *his* model. With the line
/// number and the line's text it becomes "line 5 of your model, the `gnd`
/// declaration", which is the difference between explaining the compiler and
/// diagnosing the model (ideas #45).
///
/// Byte offsets are converted by counting newlines rather than by character
/// arithmetic, and excerpts use `from_utf8_lossy`, so a specimen containing
/// non-ASCII cannot panic here. An em-dash in a description string caused exactly
/// that class of crash in the lexer on 2026-07-27.
fn span_to_location(source: &str, span: &rumoca_core::Span) -> Option<serde_json::Value> {
    let bytes = source.as_bytes();
    // `BytePos` is a newtype over `usize`; unwrap once here so the arithmetic
    // below reads as ordinary slicing.
    let (start, end) = (span.start.0, span.end.0);
    if start > end || end > bytes.len() {
        // A span from a different source file than this specimen. Nothing to say
        // about it, and inventing a line number would be worse than silence.
        return None;
    }
    let line_start = bytes[..start].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let line_end = bytes[end..].iter().position(|&b| b == b'\n').map_or(bytes.len(), |i| end + i);
    let line = bytes[..start].iter().filter(|&&b| b == b'\n').count() + 1;
    let column = start - line_start + 1;
    Some(serde_json::json!({
        "line": line,
        "column": column,
        "excerpt": String::from_utf8_lossy(&bytes[start..end]),
        "line_text": String::from_utf8_lossy(&bytes[line_start..line_end]).trim_end(),
        "byte_start": start,
        "byte_end": end,
    }))
}

/// Source locations for a singular error's unmatched unknowns, parallel to
/// `unmatched_unknowns`.
///
/// `StructuralError::Singular` has carried `unmatched_unknown_spans` all along — its
/// doc comment says it exists "so the failure is traceable back to source" — and HRW
/// dropped it until 2026-07-29. Emitting it is what lets a failure be explained in
/// terms of the model Doug actually wrote (ideas #45).
///
/// Entries carry `location: null` where an unknown has no source provenance: solver
/// scalars and manufactured variables genuinely have no line, and saying so keeps the
/// array aligned with `unmatched_unknowns` instead of silently shortening it.
fn unmatched_unknown_locations(
    source: &str,
    names: &[String],
    spans: &[Option<rumoca_core::Span>],
) -> serde_json::Value {
    serde_json::Value::Array(
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let loc = spans
                    .get(i)
                    .and_then(|s| s.as_ref())
                    .and_then(|s| span_to_location(source, s));
                serde_json::json!({ "unknown": name, "location": loc })
            })
            .collect(),
    )
}

fn structural_stage(result: Option<&PhaseResult>, source: &str) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    match rumoca_phase_structural::build_structural_report(&cr.dae) {
        Ok(rep) => {
            let inc = rumoca_phase_structural::build_incidence(&cr.dae);
            let mut json = structural_to_json(&rep);
            json.as_object_mut()
                .unwrap()
                .insert("incidence".to_owned(), incidence_to_json(&inc, Some(&cr.dae)));
            Stage::ok(json)
        }
        Err(e) => {
            let inc = rumoca_phase_structural::build_incidence(&cr.dae);
            let (match_eq, _) = rumoca_phase_structural::matching::maximum_matching(
                inc.n_eq, inc.n_var, &inc.eq_unknowns,
            );
            let matching_json = partial_matching_to_json(&inc, &match_eq, &cr.dae);
            let mut json = serde_json::json!({});
            let obj = json.as_object_mut().unwrap();
            obj.insert("incidence".to_owned(), incidence_to_json(&inc, Some(&cr.dae)));
            obj.insert("matching".to_owned(), matching_json);
            obj.insert("error".to_owned(), structural_error_to_json(&e, source));
            let note = match &e {
                rumoca_phase_structural::StructuralError::Singular { .. } => "singular".to_owned(),
                _ => format!("{e}"),
            };
            Stage::recovered(json, note)
        }
    }
}

/// Structural analysis of the DAE **after** index reduction. Runs the
/// dummy-derivative funnel (`index_reduce_for_structural_analysis`) on a copy of
/// the raw DAE, then `build_structural_report` on the result — so a high-index
/// system that `structural_stage` reports singular becomes solvable here. The
/// note says whether reduction was actually needed.
///
/// The two Structural and Index-Reduction tabs show the before/after of index
/// reduction side by side. For an already-index-1 system (like SingleInertia),
/// both tabs show the same thing. For a high-index system (like Drivetrain),
/// the Structural tab shows "singular" while the Index-Reduction tab shows
/// the successfully reduced system.
///
/// `cr.dae.clone()` — we clone the DAE because `index_reduce_for_structural_analysis`
/// mutates it in place, and we don't want to modify the original.
fn index_reduction_stage(
    result: Option<&PhaseResult>,
    source: &str,
) -> (Stage, Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>) {
    if let Some(stage) = not_reached_stage(result) {
        return (stage, Vec::new());
    }
    let cr = unwrap_success(result);
    let raw_ok = rumoca_phase_structural::build_structural_report(&cr.dae).is_ok();
    let before_inc = rumoca_phase_structural::build_incidence(&cr.dae);
    let (before_match_eq, _) = rumoca_phase_structural::matching::maximum_matching(
        before_inc.n_eq, before_inc.n_var, &before_inc.eq_unknowns,
    );
    let mut reduced = cr.dae.clone();
    let (reduction, frames) = index_reduce_for_structural_analysis(&mut reduced);
    match rumoca_phase_structural::build_structural_report(&reduced) {
        Ok(rep) => {
            let inc = rumoca_phase_structural::build_incidence(&reduced);
            let note = if raw_ok {
                "already index-1 — the reduction funnel is a no-op here (same as the Structural tab)"
            } else {
                "index-reduced from a structurally singular (high-index) system — now solvable"
            };
            let mut json = structural_to_json(&rep);
            let obj = json.as_object_mut().expect("structural_to_json returns an object");
            obj.insert("incidence".to_owned(), incidence_to_json(&inc, Some(&reduced)));
            obj.insert("before".to_owned(), before_report_json(
                &before_inc, &before_match_eq, Some(&cr.dae),
            ));
            obj.insert("reduction".to_owned(), reduction.to_json());
            (Stage::ok_with_note(json, note), frames)
        }
        Err(e) => {
            let msg = format!("{e}");
            let mut json = serde_json::json!({});
            let obj = json.as_object_mut().unwrap();
            obj.insert("incidence".to_owned(), incidence_to_json(
                &rumoca_phase_structural::build_incidence(&reduced),
                Some(&reduced),
            ));
            obj.insert("before".to_owned(), before_report_json(
                &before_inc, &before_match_eq, Some(&cr.dae),
            ));
            obj.insert("reduction".to_owned(), reduction.to_json());
            obj.insert("error".to_owned(), structural_error_to_json(&e, source));
            (Stage::recovered(json, format!("still singular after index reduction: {msg}")), frames)
        }
    }
}

/// The initial-condition solve plan — how Rumoca computes a consistent
/// initial state at t=0.
///
/// Rumoca API:
/// - `build_ic_plan(dae, n_states)` — plans how to solve the initial algebraic
///   system. Returns an ordered list of `IcBlock`s: direct symbolic solves
///   (explicit formula), scalar Newton iterations (implicit scalar), torn blocks
///   (a large coupled system reduced by tearing), and coupled LM blocks
///   (Levenberg-Marquardt for fully coupled systems).
/// - `build_ic_relaxation_hint` — names the equations dropped / unknowns pinned
///   when the initial algebraic subsystem is structurally singular.
///
/// The IC types carry `rumoca_core::Expression`, which doesn't implement
/// `Serialize`, so we build JSON manually via `ic_plan_to_json`.
///
/// This stage also computes a "determinacy check" — comparing the number of
/// explicit initial conditions (initial equations + fixed-start states) against
/// the number of states. A surplus means over-determined initialization
/// (conflicting/redundant conditions), which `build_ic_plan` alone doesn't catch.
fn initialization_stage(result: Option<&PhaseResult>) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    let n_x = cr.dae.variables.states.len();
    let n_eq = cr.dae.continuous.equations.len();
    let n_initial_eq = cr.dae.initialization.equations.len();
    let n_fixed_start_states = cr
        .dae
        .variables
        .states
        .values()
        .filter(|v| v.fixed == Some(true))
        .count();
    let explicit = n_initial_eq + n_fixed_start_states;
    let surplus = explicit as i64 - n_x as i64;
    let determinacy = serde_json::json!({
        "states": n_x,
        "initial_equations": n_initial_eq,
        "fixed_start_states": n_fixed_start_states,
        "explicit_initial_conditions": explicit,
        "surplus_over_states": surplus,
        "verdict": if surplus > 0 {
            "over-determined"
        } else {
            "well-posed (remaining states initialize from their start attributes)"
        },
    });
    match rumoca_phase_structural::build_ic_plan(&cr.dae, n_x) {
        Ok(plan) => {
            let hint = rumoca_phase_structural::build_ic_relaxation_hint(&cr.dae, n_x);
            let mut json = ic_plan_to_json(&plan, hint.as_ref(), n_x, n_eq);
            if let Some(obj) = json.as_object_mut() {
                obj.insert("determinacy".to_owned(), determinacy);
            }
            if surplus > 0 {
                Stage::recovered(
                    json,
                    format!(
                        "OVER-DETERMINED initialization: {explicit} explicit initial condition(s) \
                         ({n_initial_eq} initial equation(s) + {n_fixed_start_states} fixed start(s)) \
                         for {n_x} state(s) — {surplus} too many; conflicting / redundant ICs"
                    ),
                )
            } else if plan.is_empty() {
                Stage::ok_with_note(json, "no algebraic initialization subsystem (equations ≤ states)")
            } else {
                Stage::ok(json)
            }
        }
        Err(e) => {
            let msg = format!("{e}");
            let mut error_json = match &e {
                rumoca_phase_structural::StructuralError::Singular {
                    n_equations, n_unknowns, n_matched,
                    unmatched_equations, unmatched_unknowns, ..
                } => serde_json::json!({
                    "kind": "singular",
                    "message": msg,
                    "n_equations": n_equations,
                    "n_unknowns": n_unknowns,
                    "n_matched": n_matched,
                    "rank_deficiency": (*n_equations).max(*n_unknowns) - n_matched,
                    "unmatched_equations": unmatched_equations,
                    "unmatched_unknowns": unmatched_unknowns,
                    "guidance": "The initialization subsystem is structurally singular: the algebraic \
                        equations (beyond the state derivatives) cannot be fully matched to unknowns. \
                        Check for missing or redundant initial equations, and verify that start values \
                        are specified for the right variables.",
                }),
                _ => serde_json::json!({
                    "kind": "initialization",
                    "message": msg,
                    "guidance": "Initialization planning builds the algebraic system that determines \
                        consistent initial conditions.",
                }),
            };
            error_json.as_object_mut().unwrap()
                .insert("determinacy".to_owned(), determinacy.clone());
            let mut json = serde_json::json!({ "error": error_json });
            json.as_object_mut().unwrap().insert("determinacy".to_owned(), determinacy);
            Stage::recovered(json, format!("IC planning failed: {msg}"))
        }
    }
}

/// Convert the IC plan (a slice of `IcBlock` enums) to a JSON value.
///
/// Each `IcBlock` variant maps to a different JSON "kind":
/// - `ScalarDirect` — a variable solved by a closed-form expression
/// - `ScalarNewton` — a variable solved by scalar Newton iteration
/// - `TornBlock` — a coupled system reduced by tearing (pick "tear" variables,
///   solve the rest causally, iterate on the tear variables)
/// - `CoupledLM` — a fully coupled system solved by Levenberg-Marquardt
pub(crate) fn ic_plan_to_json(
    plan: &[rumoca_phase_structural::IcBlock],
    hint: Option<&rumoca_phase_structural::IcRelaxationHint>,
    n_x: usize,
    n_eq: usize,
) -> serde_json::Value {
    use rumoca_phase_structural::IcBlock;
    let blocks: Vec<serde_json::Value> = plan
        .iter()
        .map(|b| match b {
            IcBlock::ScalarDirect { var_name, solution_expr, .. } => serde_json::json!({
                "kind": "scalar_direct",
                "var": var_name,
                "solution": ser_value(solution_expr),
            }),
            IcBlock::ScalarNewton { var_name, eq_idx, .. } => serde_json::json!({
                "kind": "scalar_newton",
                "var": var_name,
                "equation": eq_idx,
            }),
            IcBlock::TornBlock { tear_var_names, causal_sequence, residual_eq_indices, .. } => {
                serde_json::json!({
                    "kind": "torn_block",
                    "tear_vars": tear_var_names,
                    "residual_equations": residual_eq_indices,
                    "causal_steps": causal_sequence.iter().map(|s| serde_json::json!({
                        "var": s.var_name,
                        "equation": s.eq_idx,
                        "newton": s.solution_expr.is_none(),
                    })).collect::<Vec<_>>(),
                })
            }
            IcBlock::CoupledLM { eq_indices, var_names, .. } => serde_json::json!({
                "kind": "coupled_lm",
                "vars": var_names,
                "equations": eq_indices,
            }),
        })
        .collect();
    serde_json::json!({
        "n_states": n_x,
        "n_equations": n_eq,
        "block_count": blocks.len(),
        "blocks": blocks,
        "relaxation_hint": hint.map(|h| serde_json::json!({
            "dropped_equations": h.dropped_eq_global,
            "pinned_unknowns": h.dropped_unknown_names,
        })),
    })
}

/// The DAE's hybrid / event structure — where the equation set changes at
/// discrete events. Read directly from the public `rumoca-ir-dae` partitions:
/// `conditions` (the `f_c` equations + the `relation` expressions that trigger
/// events), `discrete` (the `f_z`/`f_m` update equations lowered from `when`
/// clauses), and `events` (zero-crossing root conditions + scheduled time events).
fn events_stage(result: Option<&PhaseResult>) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    let json = events_to_json(&cr.dae);
    let total = json["summary"]
        .as_object()
        .map(|s| s.values().filter_map(serde_json::Value::as_u64).sum::<u64>())
        .unwrap_or(0);
    if total == 0 {
        Stage::ok_with_note(json, "no events — this model is a smooth (continuous) system")
    } else {
        Stage::ok(json)
    }
}

/// Convert the DAE's hybrid/event structure to JSON. Reads directly from the
/// public `rumoca-ir-dae` partitions:
/// - `conditions` — the `f_c` equations + `relation` expressions that trigger events
/// - `discrete` — the `f_z` (real updates) / `f_m` (valued updates) from `when` clauses
/// - `events` — zero-crossing root conditions + scheduled time events
///
/// The `summary` object counts each category — the Events tab uses this to
/// show "no events" for smooth models vs a detailed breakdown for hybrid ones.
fn events_to_json(dae: &rumoca_ir_dae::Dae) -> serde_json::Value {
    let conditions = &dae.conditions;
    let discrete = &dae.discrete;
    let events = &dae.events;
    serde_json::json!({
        "summary": {
            "condition_equations": conditions.equations.len(),
            "relations": conditions.relations.len(),
            "discrete_real_updates": discrete.real_updates.len(),
            "discrete_valued_updates": discrete.valued_updates.len(),
            "zero_crossing_conditions": events.synthetic_root_conditions.len(),
            "scheduled_time_events": events.scheduled_time_events.len(),
        },
        "conditions": {
            "equations_f_c": ser_value(&conditions.equations),
            "relations": ser_value(&conditions.relations),
        },
        "discrete_updates": {
            "real_updates_f_z": ser_value(&discrete.real_updates),
            "valued_updates_f_m": ser_value(&discrete.valued_updates),
        },
        "events": {
            "zero_crossing_conditions": ser_value(&events.synthetic_root_conditions),
            "scheduled_time_events": ser_value(&events.scheduled_time_events),
        },
    })
}

/// Solve lowering (phase 8) — the DAE lowered to a `SolveModel`, the
/// solvable form the simulator runs (residual programs, variable layout, mass
/// matrix, Jacobian sparsity).
///
/// Unlike the structural/IC stages, `SolveModel` derives `Serialize`, so we
/// can use `serde_json::to_value(&sm)` directly instead of building JSON
/// manually. This is the simplest stage extractor — just serialize and wrap.
///
/// Rumoca API: `lower_dae_to_solve_model(&dae)` transforms the mathematical
/// DAE into the solver's executable form.
fn solve_lowering_stage(result: Option<&PhaseResult>) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    match rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae) {
        Ok(sm) => match serde_json::to_value(&sm) {
            Ok(v) => Stage::ok(v),
            Err(e) => Stage::err(format!("serialize SolveModel: {e}")),
        },
        Err(e) => {
            let msg = format!("{e}");
            let error_json = solve_lower_error_to_json(&e);
            Stage::err_with_details(error_json, format!("solve lowering failed: {msg}"))
        }
    }
}

/// Convert a `StructuralReport` to JSON. The report contains the maximum
/// matching (which equation is paired with which unknown), the BLT blocks
/// (the solve order), and the coupled block count (algebraic loops).
fn structural_to_json(rep: &rumoca_phase_structural::StructuralReport) -> serde_json::Value {
    serde_json::json!({
        "n_equations": rep.n_equations,
        "n_unknowns": rep.n_unknowns,
        "coupled_block_count": rep.coupled_block_count(),
        "matching": rep
            .matching
            .iter()
            .map(|(e, u)| serde_json::json!({ "equation": e, "unknown": u }))
            .collect::<Vec<_>>(),
        "blocks": rep.blocks.iter().map(block_to_json).collect::<Vec<_>>(),
    })
}

/// Convert the incidence matrix (equation x unknown bipartite adjacency) to JSON.
/// Each row says which unknowns appear in that equation — the raw data the
/// spy-plot view renders as dots in a matrix.
///
/// When `equations` is provided (the DAE's continuous equation list), each row
/// also carries an `"equation_text"` field with the pretty-printed Modelica
/// expression (e.g. `der(w) - tau / J`), for human-readable labels.
fn incidence_to_json(
    inc: &rumoca_phase_structural::Incidence,
    dae: Option<&rumoca_ir_dae::Dae>,
) -> serde_json::Value {
    let eq_texts: Vec<String> = dae
        .map(|d| crate::expr_format::equation_labels(&d.continuous.equations))
        .unwrap_or_default();
    let rows: Vec<serde_json::Value> = inc
        .eq_unknowns
        .iter()
        .enumerate()
        .map(|(i, cols)| {
            let mut sorted: Vec<usize> = cols.iter().copied().collect();
            sorted.sort_unstable();
            // **Rumoca's own labelling function**, not a copy of it. Matching
            // lookups in `IncidenceMatrix::from_report` correlate by this exact
            // string, so a reimplementation that drifts silently unmatches
            // everything — which is what happened here (see
            // `partial_matching_to_json`). Without a DAE there is no origin to
            // look up, and the bare ref is what `equation_label` would return
            // anyway.
            let eq_name = match dae {
                Some(d) => rumoca_phase_structural::equation_label(d, &inc.equation_refs[i]),
                None => inc.equation_refs[i].to_string(),
            };
            let mut row = serde_json::json!({
                "equation": eq_name,
                "unknowns": sorted,
            });
            if let Some(text) = eq_texts.get(i) {
                row.as_object_mut()
                    .unwrap()
                    .insert("equation_text".to_owned(), serde_json::Value::String(text.clone()));
            }
            row
        })
        .collect();
    let unknown_names: Vec<String> = inc.unknown_names.iter().map(|u| u.to_string()).collect();
    serde_json::json!({
        "n_eq": inc.n_eq,
        "n_var": inc.n_var,
        "unknown_names": unknown_names,
        "rows": rows,
    })
}

/// Build a self-contained "before" report for the raw (pre-reduction) DAE.
///
/// The raw system is typically structurally singular (high-index), so there's
/// no full structural report (no BLT blocks). We include the incidence matrix
/// and partial matching so the UI can show a Before pane with rank deficiency
/// highlighted. The returned JSON has the same shape as a structural report
/// (`"matching"`, `"incidence"`) so `IncidenceMatrix::from_report` can parse it.
fn before_report_json(
    inc: &rumoca_phase_structural::Incidence,
    match_eq: &[Option<usize>],
    dae: Option<&rumoca_ir_dae::Dae>,
) -> serde_json::Value {
    let matching = match dae {
        Some(d) => partial_matching_to_json(inc, match_eq, d),
        None => serde_json::Value::Array(Vec::new()),
    };
    let n_matched = matching.as_array().map_or(0, Vec::len);
    serde_json::json!({
        "n_equations": inc.n_eq,
        "n_unknowns": inc.n_var,
        "n_matched": n_matched,
        "matching": matching,
        "incidence": incidence_to_json(inc, dae),
    })
}

/// Build a JSON array of matched (equation, unknown) pairs from a partial
/// matching result — the same shape as the structural report's `"matching"`
/// array so `IncidenceMatrix::from_report` can parse it.
/// Labels equations with `rumoca_phase_structural::equation_label`, the same
/// function the successful report uses. It used to emit the **bare**
/// `EquationRef` while `incidence_to_json` emitted the labelled form, so on any
/// model whose equations carry origins the two never matched and the singular
/// incidence view showed *nothing* as matched — `Drivetrain` rendering 0 of 97
/// when Rumoca had matched 93. Found by `docs/fidelity-plan.md` F1 on
/// 2026-07-31; see that file for why re-derivation checks catch this class.
fn partial_matching_to_json(
    inc: &rumoca_phase_structural::Incidence,
    match_eq: &[Option<usize>],
    dae: &rumoca_ir_dae::Dae,
) -> serde_json::Value {
    let pairs: Vec<serde_json::Value> = match_eq
        .iter()
        .enumerate()
        .filter_map(|(eq_idx, var_idx)| {
            var_idx.map(|v| {
                serde_json::json!({
                    "equation": rumoca_phase_structural::equation_label(
                        dae, &inc.equation_refs[eq_idx],
                    ),
                    "unknown": inc.unknown_names[v].to_string(),
                })
            })
        })
        .collect();
    serde_json::Value::Array(pairs)
}

/// Convert a `StructuralError` into structured JSON for UI rendering.
/// Convert a `StructuralError` into structured JSON for the UI and for Claude.
///
/// Takes the specimen `source` so unmatched unknowns can be reported with the line
/// that declares them (ideas #45). It no longer takes the incidence: the only thing
/// that used it was the rank-deficiency computation, which was *wrong* to use it —
/// see the comment on that field.
fn structural_error_to_json(
    e: &rumoca_phase_structural::StructuralError,
    source: &str,
) -> serde_json::Value {
    match e {
        rumoca_phase_structural::StructuralError::Singular {
            n_equations, n_unknowns, n_matched,
            unmatched_equations, unmatched_unknowns, unmatched_unknown_spans,
        } => serde_json::json!({
            "kind": "singular",
            "message": format!("{e}"),
            "n_equations": n_equations,
            "n_unknowns": n_unknowns,
            "n_matched": n_matched,
            // From the **error's own** counts, not from `inc`. Until 2026-07-29 this
            // read `inc.n_eq.max(inc.n_var) - n_matched`, and `index_reduction_stage`
            // passes the *raw* incidence while the error describes the *reduced*
            // system — so `CapacitorLoop` reported a rank deficiency of **7** (14 raw
            // equations minus 7 reduced matches) where the truth is 1. A wrong number
            // is worse than a missing one: it reads as authoritative, and Claude would
            // have repeated it.
            "rank_deficiency": (*n_equations).max(*n_unknowns) - n_matched,
            "unmatched_equations": unmatched_equations,
            "unmatched_unknowns": unmatched_unknowns,
            // The point of ideas #45: where in *Doug's source* each untethered
            // unknown is declared, so a failure can be diagnosed rather than merely
            // described.
            "unmatched_unknown_locations":
                unmatched_unknown_locations(source, unmatched_unknowns, unmatched_unknown_spans),
            "guidance": "The maximum matching could not pair every equation with a unique \
                unknown. Each unmatched unknown is a variable no equation determines; \
                `unmatched_unknown_locations` gives the source line that declares it. \
                If the Index Reduction stage also reports singular, the model is \
                genuinely ill-posed rather than merely high-index.",
        }),
        _ => serde_json::json!({
            "kind": "other",
            "message": format!("{e}"),
            "guidance": "An unexpected error occurred during structural analysis.",
        }),
    }
}

/// Convert an `InstantiateError` into structured JSON for UI rendering.
fn instantiate_error_to_json(e: &rumoca_phase_instantiate::InstantiateError) -> serde_json::Value {
    use rumoca_phase_instantiate::InstantiateError;
    let msg = format!("{e}");
    let mut json = serde_json::json!({
        "kind": "instantiate",
        "message": msg,
    });
    let obj = json.as_object_mut().unwrap();
    match e {
        InstantiateError::ModelNotFound(name)
        | InstantiateError::ModelNotFoundWithSpan { name, .. } => {
            obj.insert("error_code".to_owned(), "EI001".into());
            obj.insert("detail".to_owned(), format!("Model `{name}` could not be found in the loaded libraries.").into());
            obj.insert("guidance".to_owned(), "Check that the model name is spelled correctly, the package is loaded, \
                and the model is exported (not encapsulated).".into());
        }
        InstantiateError::TypeNotFound { name, .. } => {
            obj.insert("error_code".to_owned(), "EI030".into());
            obj.insert("detail".to_owned(), format!("Type `{name}` is referenced but not defined.").into());
            obj.insert("guidance".to_owned(), "Check that the type exists in the loaded libraries and is \
                accessible from the model's scope.".into());
        }
        InstantiateError::InvalidModPath { path, .. } => {
            obj.insert("error_code".to_owned(), "EI002".into());
            obj.insert("detail".to_owned(), format!("Modification path `{path}` does not correspond to a valid element.").into());
            obj.insert("guidance".to_owned(), "Check the component path — it may reference a non-existent \
                sub-component or use an incorrect dotted path.".into());
        }
        InstantiateError::ModTypeMismatch { path, expected, found, .. } => {
            obj.insert("error_code".to_owned(), "EI003".into());
            obj.insert("detail".to_owned(), format!("Modification for `{path}` expects type `{expected}` but found `{found}`.").into());
            obj.insert("guidance".to_owned(), "The modification value type must match the component's declared type.".into());
        }
        InstantiateError::StructuralParamError { name, msg: param_msg, .. } => {
            obj.insert("error_code".to_owned(), "EI004".into());
            obj.insert("detail".to_owned(), format!("Structural parameter `{name}` could not be evaluated: {param_msg}").into());
            obj.insert("guidance".to_owned(), "Structural parameters (like array sizes) must be evaluable at \
                compile time. Check that their values are constant expressions.".into());
        }
        InstantiateError::ArrayDimMismatch { name, expected, found, .. } => {
            obj.insert("error_code".to_owned(), "EI005".into());
            obj.insert("detail".to_owned(), format!("Array `{name}` was declared with dimension {expected} but found {found}.").into());
            obj.insert("guidance".to_owned(), "Array dimensions must agree between the declaration and the \
                modification or binding equation.".into());
        }
        _ => {
            obj.insert("guidance".to_owned(), "Instantiation expands a model's class hierarchy into a flat \
                component tree. Check that all component types are declared and modifications are valid.".into());
        }
    }
    json
}

/// Convert a `SolveModelLowerError` into structured JSON for UI rendering.
fn solve_lower_error_to_json(e: &rumoca_phase_solve::SolveModelLowerError) -> serde_json::Value {
    use rumoca_phase_solve::SolveModelLowerError;
    let msg = format!("{e}");
    match e {
        SolveModelLowerError::Structural { source } => {
            let mut json = serde_json::json!({
                "kind": "singular",
                "message": msg,
                "guidance": "The BLT decomposition during solve lowering encountered a structural \
                    singularity. The reduced system may still have unresolvable dependencies.",
            });
            if let rumoca_phase_structural::StructuralError::Singular {
                n_equations, n_unknowns, n_matched,
                unmatched_equations, unmatched_unknowns, ..
            } = source {
                let obj = json.as_object_mut().unwrap();
                obj.insert("n_equations".to_owned(), (*n_equations).into());
                obj.insert("n_unknowns".to_owned(), (*n_unknowns).into());
                obj.insert("n_matched".to_owned(), (*n_matched).into());
                obj.insert("rank_deficiency".to_owned(), ((*n_equations).max(*n_unknowns) - n_matched).into());
                obj.insert("unmatched_equations".to_owned(), serde_json::json!(unmatched_equations));
                obj.insert("unmatched_unknowns".to_owned(), serde_json::json!(unmatched_unknowns));
            }
            json
        }
        SolveModelLowerError::MassMatrix { row, state_name, reason, .. } => {
            serde_json::json!({
                "kind": "mass_matrix",
                "message": msg,
                "detail": format!("Mass matrix row {row} for state `{state_name}` could not be derived."),
                "reason": reason,
                "state_name": state_name,
                "row": row,
                "guidance": "The mass matrix entry for this state variable could not be \
                    computed. This often indicates a higher-index problem or a \
                    variable that should not be a state.",
            })
        }
        SolveModelLowerError::Evaluation { context, source, .. } => {
            serde_json::json!({
                "kind": "evaluation",
                "message": msg,
                "detail": format!("Failed to evaluate {context}: {source}"),
                "context": context,
                "guidance": "An expression could not be evaluated during solve lowering. \
                    Check for division by zero, undefined variables, or unsupported functions.",
            })
        }
        SolveModelLowerError::Lower(lower_err) => {
            serde_json::json!({
                "kind": "solve_lowering",
                "message": msg,
                "detail": format!("{lower_err}"),
                "guidance": "Solve lowering transforms the DAE into a solver-ready form.",
            })
        }
    }
}

/// Convert a single BLT block to JSON. A "scalar" block is a single
/// equation solved for a single unknown (trivial). A "coupled" block is an
/// algebraic loop — multiple equations that must be solved simultaneously
/// (the orange boxes in the spy-plot). Coupled blocks may have a "tearing"
/// report: the strategy for breaking the loop into a smaller iteration.
fn block_to_json(b: &rumoca_phase_structural::BlockReport) -> serde_json::Value {
    use rumoca_phase_structural::BlockReport;
    match b {
        BlockReport::Scalar { equation, unknown } => serde_json::json!({
            "kind": "scalar",
            "size": 1,
            "equation": equation,
            "unknown": unknown,
        }),
        BlockReport::Coupled { equations, unknowns, tearing } => serde_json::json!({
            "kind": "coupled",
            "size": unknowns.len(),
            "equations": equations,
            "unknowns": unknowns,
            "tearing": tearing.as_ref().map(tearing_to_json),
        }),
    }
}

/// Convert a tearing report to JSON. Tearing reduces a coupled block's
/// iteration size: "tear variables" are guessed, the remaining equations
/// are solved causally (one at a time), and the residual equations check
/// convergence. The causal sequence is the order of the sequential solves.
fn tearing_to_json(t: &rumoca_phase_structural::TearingReport) -> serde_json::Value {
    serde_json::json!({
        "tear_vars": t.tear_vars,
        "residual_equations": t.residual_equations,
        "causal_sequence": t
            .causal_sequence
            .iter()
            .map(|(e, v)| serde_json::json!({ "equation": e, "variable": v }))
            .collect::<Vec<_>>(),
    })
}

/// Record connection expansion (MLS §9) by re-running flatten with an observer.
///
/// The session's own compile has already flattened, without an observer — the
/// frames exist only while the pass runs, so the only way to see them is to run
/// it again. This is the same shape as the `pre()`-lowering replay: a second
/// run of a pure function, paid for deliberately. Doug, on this trade:
/// *"This project is for learning, not for production performance.
/// Debuggability is of the highest priority."*
///
/// The options must match `rumoca_compile`'s own (`flatten_options_for_tree`),
/// or the recorded frames would describe a flatten that never happened.
/// `strict_connection_validation: true` is the one that matters here — it is
/// what makes an incompatible-connector model fail rather than expand.
///
/// Returns an empty vec on any failure: this is an observation extra, and a
/// model that will not instantiate has already reported that on its own tab.
fn record_connection_frames(
    tree: &rumoca_ir_ast::ClassTree,
    model_name: &str,
) -> Vec<rumoca_phase_flatten::connections::trace::ConnectionFrame> {
    use rumoca_phase_flatten::connections::trace::ConnectionFrame;

    let Ok(mut overlay) = rumoca_phase_instantiate::instantiate_model(tree, model_name) else {
        return Vec::new();
    };
    // Typecheck mutates the overlay (it annotates types and dimensions), and
    // flatten reads those annotations — so the overlay must go through it even
    // though its diagnostics are ignored here.
    let _ = rumoca_phase_typecheck::typecheck_instanced(tree, &mut overlay, model_name);

    let frames = std::cell::RefCell::new(Vec::new());
    {
        let sink = |f: &ConnectionFrame| frames.borrow_mut().push(f.clone());
        let options = rumoca_phase_flatten::FlattenOptions {
            strict_connection_validation: true,
            simplify_variable_names: false,
            materialize_structured_families: false,
        };
        let _ = rumoca_phase_flatten::flatten_ref_with_options_traced(
            tree,
            &overlay,
            model_name,
            options,
            Some(&sink),
        );
    }
    frames.into_inner()
}

/// Instantiate the model directly from the resolved tree and serialize the
/// resulting `InstanceOverlay` for the Instantiate tab; then run the instanced
/// typecheck, which enriches the *same* overlay in place (evaluated dimensions,
/// resolved component types), and serialize it again for the Typecheck tab.
/// The cross-stage diff between the two shows exactly what typecheck contributed.
///
/// Rumoca API:
/// - `instantiate_model(tree, name)` — creates an `InstanceOverlay` (the model
///   with all inherited/extended components resolved and enumerated).
/// - `typecheck_instanced(tree, &mut overlay, name)` — enriches the overlay
///   in place with type information (dimensions, component types). The `&mut`
///   means it MODIFIES the overlay — which is why we serialize it BEFORE
///   typecheck (for the Instantiate tab) and AFTER (for the Typecheck tab).
///
/// *(This doc block sat above `record_connection_frames` until 2026-08-01, because that
/// function was inserted between it and its own -- and a doc comment attaches to the NEXT
/// item, so it had been silently documenting the wrong function. Clippy's
/// `doc_lazy_continuation` was pointing at it the whole time.)*
///
/// `source` is the specimen text, so a diagnostic's labels can be reported as line
/// numbers rather than byte offsets (ideas #45).
fn instantiate_and_typecheck(
    tree: &rumoca_ir_ast::ClassTree,
    model_name: &str,
    source: &str,
) -> (Stage, Stage) {
    match rumoca_phase_instantiate::instantiate_model(tree, model_name) {
        Ok(mut overlay) => {
            let instantiate = Stage::from_ser(&overlay);
            let typecheck = match rumoca_phase_typecheck::typecheck_instanced(tree, &mut overlay, model_name) {
                Ok(()) => Stage::from_ser(&overlay),
                Err(diags) => {
                    let mut json = ser_value(&overlay);
                    // Shared helper, so `labels` — the source location of each problem —
                    // survives here as it does everywhere else. This block used to build
                    // its own diagnostic JSON and drop them.
                    let collected: Vec<rumoca_core::Diagnostic> = diags.iter().cloned().collect();
                    let diag_json = diagnostics_to_json(&collected, source);
                    let n = collected.len();
                    json.as_object_mut().unwrap().insert("error".to_owned(), serde_json::json!({
                        "kind": "typecheck",
                        "message": format!("Typecheck reported {n} diagnostic(s)"),
                        "diagnostics": diag_json,
                        "guidance": "Typecheck validates types, dimensions, and units across the \
                            instantiated model. The overlay above is partial — it reflects work \
                            completed before the error.",
                    }));
                    Stage::recovered(json, format!("typecheck: {n} diagnostic(s)"))
                }
            };
            (instantiate, typecheck)
        }
        Err(e) => {
            let msg = format!("{e}");
            let error_json = instantiate_error_to_json(&e);
            (
                Stage::err_with_details(error_json, format!("instantiate failed: {msg}")),
                Stage::info("not reached (instantiate failed)"),
            )
        }
    }
}

/// Extract just the Flatten stage from the reachable-closure pipeline's
/// `PhaseResult` (the flat IR on success, or per-phase status/error).
///
/// The Flatten stage has a richer match than the others because it handles
/// `FailedPhase::Flatten` (the stage itself failed) differently from
/// `FailedPhase::ToDae` (flatten succeeded but the subsequent DAE
/// construction failed) and other earlier failures. It also handles
/// `NeedsInner` (the model references inner declarations that weren't
/// provided — a rare Modelica feature).
/// `source` is the specimen text, so diagnostic labels become line numbers.
fn flatten_stage(result: Option<&PhaseResult>, source: &str) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => match serde_json::to_value(&cr.flat) {
            Ok(v) => Stage::ok(v),
            Err(e) => Stage::err(format!("serialize flat model: {e}")),
        },
        Some(PhaseResult::Failed { phase, error, error_code, diagnostics }) => {
            let msg = if diagnostics.is_empty() {
                error.clone()
            } else {
                format!("{error}  ({} diagnostic(s))", diagnostics.len())
            };
            match phase {
                FailedPhase::Flatten => {
                    // Shared helper, so `labels` — the source location of each problem —
                    // survives. This block used to build its own diagnostic JSON and
                    // drop them.
                    let diag_json = diagnostics_to_json(diagnostics, source);
                    Stage::err_with_details(serde_json::json!({
                        "kind": "flatten",
                        "message": error,
                        "error_code": error_code,
                        "diagnostics": diag_json,
                        "guidance": "Flattening transforms the component hierarchy into flat equations. \
                            Check for unsupported language features, circular definitions, or type mismatches.",
                    }), msg)
                }
                FailedPhase::ToDae => {
                    // Until 2026-07-29 this arm discarded everything and returned a
                    // bare `Stage::info("...DAE construction failed (later arc)")` —
                    // while `error`, `error_code` and `diagnostics` sat in scope,
                    // unused. That made the **most common Modelica authoring error**
                    // (declare a variable, forget its equation) the *least* informative
                    // failure in the pipeline: Rumoca says "unbalanced model: 2
                    // equations, 3 unknowns (balance = -1)" and HRW said nothing.
                    //
                    // Promoted from `info` to a real error too. It *is* one, and
                    // `last_successful_stage` keys on `note_is_error`, so flatten no
                    // longer looks like the furthest good stage when DAE construction
                    // has failed.
                    Stage::err_with_details(
                        dae_construction_error_to_json(error, error_code, diagnostics, source),
                        msg,
                    )
                }
                other => Stage::info(format!("not reached ({other} failed earlier)")),
            }
        }
        Some(PhaseResult::NeedsInner { missing_inners, .. }) => {
            Stage::info(format!("needs inner declaration(s) for: {}", missing_inners.join(", ")))
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
    }
}

/// **Why there is no DAE** — the DAE stage when construction produced nothing.
///
/// Every *downstream* stage already explained its own emptiness ("not reached
/// (ToDae failed earlier)"), and until 2026-08-03 the stage that actually failed
/// rendered a blank tab. The asymmetry was invisible while DAE construction had no
/// tab of its own: `flatten_stage` adopted the `FailedPhase::ToDae` error because
/// Flatten was the last tab before Structural, and that made **the succeeding stage
/// the one that reported the failure.**
///
/// The failure modes are distinguished because they mean different things to a reader:
///
/// - **`ToDae`** — this phase ran and refused. The typed error says why, and for the
///   commonest case (`ToDaeError::Unbalanced`, `rumoca::todae::ED001`) it carries the
///   equation and unknown counts that make the refusal checkable.
/// - **anything earlier** — this phase never ran, so it has nothing of its own to say
///   and names the phase that stopped first instead. Claiming a DAE-construction
///   problem here would blame the wrong phase.
///
/// Found by `docs/fixture-tours/dae-construction.md`, whose counterexample stop opens
/// this exact tab on `UnbalancedShaft`.
fn dae_absent_stage(result: Option<&PhaseResult>, source: &str) -> Stage {
    match result {
        Some(PhaseResult::Failed { phase: FailedPhase::ToDae, error, error_code, diagnostics }) => {
            let msg = if diagnostics.is_empty() {
                error.clone()
            } else {
                format!("{error}  ({} diagnostic(s))", diagnostics.len())
            };
            Stage::err_with_details(
                dae_construction_error_to_json(error, error_code, diagnostics, source),
                msg,
            )
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { missing_inners, .. }) => {
            Stage::info(format!("needs inner declaration(s) for: {}", missing_inners.join(", ")))
        }
        // Success with no DAE cannot happen (the DAE is how success is defined), and
        // `None` is the no-result case Flatten already reports. Neither is worth a
        // second claim from this stage.
        _ => Stage::default(),
    }
}

/// Field names in the IR whose values are `DefId`s (resolved definition ids).
///
/// These are the JSON keys we scan for when building the `DefIndex`. The
/// array is `[&str; 3]` — a fixed-size array of three string slices,
/// allocated at compile time (no heap allocation). `const` means this is
/// inlined wherever it's used (like a C `#define` but type-safe).
const DEF_ID_KEYS: [&str; 3] = ["def_id", "type_def_id", "base_def_id"];

/// True when `key` names a `DefId`-valued field.
pub fn is_def_id_key(key: &str) -> bool {
    DEF_ID_KEYS.contains(&key)
}

/// Collect every DefId appearing under a DefId-named key anywhere in the IR.
///
/// Recursively walks the JSON tree. When it finds an object key that matches
/// one of `DEF_ID_KEYS` and whose value is a u64, it inserts the id into
/// `out`. `BTreeSet` deduplicates automatically (a DefId may appear in
/// multiple places in the IR).
///
/// The `if ... && let Some(n) = ...` syntax is a Rust "let chain" — it
/// combines a boolean check with a pattern-match in one `if` guard. The
/// `let Some(n) = val.as_u64()` part is an *irrefutable pattern* — it
/// succeeds if `as_u64()` returns `Some`, binding the inner value to `n`.
fn collect_def_ids(v: &serde_json::Value, out: &mut BTreeSet<u64>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                if is_def_id_key(k)
                    && let Some(n) = val.as_u64()
                {
                    out.insert(n);
                }
                collect_def_ids(val, out);
            }
        }
        serde_json::Value::Array(arr) => arr.iter().for_each(|val| collect_def_ids(val, out)),
        _ => {}
    }
}

/// Resolve every DefId referenced in `value` against the resolved tree, into a
/// `DefId -> DefInfo` map.
///
/// First builds a reverse lookup `HashMap<u32, &str>` from Rumoca's `def_map`
/// (which maps `DefId -> qualified_name`). Then scans the JSON value for all
/// DefId references, and for each one, looks up the qualified name and
/// determines whether it's a class (with a source location) or just a
/// definition (name only).
///
/// The `BTreeMap<u64, DefInfo>` return type uses `BTreeMap` (sorted by key)
/// rather than `HashMap` for deterministic ordering — important for the UI
/// and for test stability. The key is `u64` (the DefId as a JSON number)
/// rather than Rumoca's `DefId` type, so we keep `rumoca-core` out of our
/// direct dependencies.
fn build_def_index(
    tree: &rumoca_ir_ast::ClassTree,
    value: &serde_json::Value,
) -> BTreeMap<u64, DefInfo> {
    let name_by_id: std::collections::HashMap<u32, &str> =
        tree.def_map.iter().map(|(k, v)| (k.0, v.as_str())).collect();

    let mut ids = BTreeSet::new();
    collect_def_ids(value, &mut ids);

    let mut index = BTreeMap::new();
    for id in ids {
        // Use try_from to avoid silently truncating u64 → u32; if the id
        // doesn't fit in a u32, skip it gracefully rather than wrapping.
        let Some(id32) = u32::try_from(id).ok() else { continue };
        let Some(name) = name_by_id.get(&id32) else { continue };
        let name = (*name).to_owned();
        // A class DefId resolves to a ClassDef (with a location); anything else
        // in def_map (e.g. a component) resolves to a name only.
        let info = match tree.get_class_by_qualified_name(&name) {
            Some(class) => DefInfo {
                name,
                kind: DefKind::Class,
                class_type: Some(class.class_type.as_str().to_owned()),
                file_name: Some(class.location.file_name.clone()),
                line: Some(class.location.start_line),
            },
            None => DefInfo { name, kind: DefKind::Definition, class_type: None, file_name: None, line: None },
        };
        index.insert(id, info);
    }
    index
}

// ==========================================================================
// Tests
// ==========================================================================
// `#[cfg(test)]` means this entire module is only compiled when running
// `cargo test` — it doesn't exist in the release binary. `mod tests` creates
// a child module with access to the parent module's private items (via
// `use super::*`).
/// Test-only helpers shared beyond this module's own tests.
///
/// Lifted out of `mod tests` on 2026-07-31 so `crate::fidelity` can compile
/// specimens through the same MSL-loaded worker. Keeping a second copy would
/// have meant a second ~430MB MSL load in the same test process.
///
/// **A module rather than three `#[cfg(test)]` attributes.** The attribute
/// applies to the item that follows it and nothing else, so the first lift
/// gated only `msl_roots` and left the other two compiling into `--bin hrw`,
/// where neither `OnceLock` nor `msl_roots` existed. `cargo test` could not see
/// it: the test build has everything. A module cannot lose the gate when a
/// fourth helper is added.
#[cfg(test)]
pub(crate) mod test_msl {
    use super::{FromWorker, WorkerState};
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    /// Returns the three MSL (Modelica Standard Library) root paths needed
    /// to compile any specimen that uses standard components.
    pub(crate) fn msl_roots() -> Vec<PathBuf> {
        let base = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/msl");
        vec![
            PathBuf::from(format!("{base}/Modelica 4.1.0")),
            PathBuf::from(format!("{base}/ModelicaServices 4.1.0")),
            PathBuf::from(format!("{base}/Complex.mo")),
        ]
    }

    /// One MSL-loaded worker, built once and shared across the worker tests behind
    /// a mutex. Each test needs the full MSL (~430MB resolved); loading it per-test
    /// OOMs / thrashes when cargo runs them in parallel. So tests lock this shared,
    /// already-loaded worker and run serially against it — MSL is parsed once, and
    /// peak memory stays at a single session.
    ///
    /// # Why `&'static Mutex<WorkerState>`?
    ///
    /// - `&'static` — the returned reference lives for the entire program lifetime
    ///   (it's backed by a `static` variable, not the stack).
    /// - `OnceLock::new()` — creates an uninitialized lock. `get_or_init()` lazily
    ///   initializes it on first access, thread-safely. Subsequent calls return the
    ///   same value without re-running the init closure.
    /// - `Mutex<WorkerState>` — wraps the worker in a mutex so only one test at a
    ///   time can access it. `lock().unwrap()` blocks until the mutex is available.
    ///
    /// # Why tests run serially
    ///
    /// `cargo test` runs test functions in parallel by default. Without the mutex,
    /// multiple tests would try to use the `Session` concurrently (which isn't
    /// thread-safe). The mutex serializes access. The session accumulates each
    /// specimen's document (distinct URIs), which is fine: `compile` qualifies the
    /// requested model by its own URI.
    /// `pub(super)`, deliberately not `pub(crate)`: it hands out `WorkerState`,
    /// which is private to `worker`, so a wider visibility would leak a private
    /// type. `worker::tests` needs it directly for the tests that drive the
    /// worker rather than just read a compile; everything outside `worker` goes
    /// through [`compile_specimen_shared`], which returns only the result. The
    /// narrower door is the one worth opening.
    pub(super) fn shared_worker() -> &'static Mutex<WorkerState> {
        static WORKER: OnceLock<Mutex<WorkerState>> = OnceLock::new();
        WORKER.get_or_init(|| {
            let mut state = WorkerState::new();
            state.load_libraries(msl_roots()).expect("load MSL once for tests");
            Mutex::new(state)
        })
    }

    /// Compile `specimens/<name>.mo` against the shared MSL worker.
    ///
    /// `unwrap_or_else(|e| e.into_inner())` — if a previous test panicked while
    /// holding the mutex, the mutex is "poisoned" (marked as potentially in an
    /// inconsistent state). `into_inner()` recovers from the poison by taking
    /// the inner value anyway — we accept the risk because our WorkerState is
    /// still usable after a panic (it's just a Session, not half-modified data).
    pub(crate) fn compile_specimen_shared(name: &str) -> FromWorker {
        if let Some(hit) = specimen_cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(name)
        {
            return hit.clone();
        }
        // **Compile outside the cache lock.** Holding it across a compile would
        // mean holding two locks in a fixed order for tens of seconds; harmless
        // under `--test-threads=1` but a deadlock waiting for the day that
        // changes. A duplicate compile on a race is wasted work, never wrong.
        let fresh = compile_specimen_uncached(name);
        specimen_cache()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_owned(), fresh.clone());
        fresh
    }

    /// One compile per specimen per test process. See [`compile_specimen_shared`].
    fn specimen_cache() -> &'static Mutex<std::collections::HashMap<String, FromWorker>> {
        static CACHE: OnceLock<Mutex<std::collections::HashMap<String, FromWorker>>> =
            OnceLock::new();
        CACHE.get_or_init(|| Mutex::new(std::collections::HashMap::new()))
    }

    /// Compile a specimen **without** consulting the memo — a genuinely fresh
    /// run through every phase.
    ///
    /// # When a test must use this
    ///
    /// **Any test whose subject is the act of compiling**, rather than the
    /// result. Two kinds exist today:
    ///
    /// - **Cross-compile contamination.**
    ///   `a_broken_specimen_does_not_poison_the_next_compile` exists because a
    ///   failed resolve once leaked into the *next* model's result. A memoised
    ///   answer would never touch the session and the test would pass
    ///   vacuously — proving nothing while looking green, which is worse than
    ///   deleting it.
    /// - **Reproducibility.** `compiling_a_specimen_twice_is_reproducible` is
    ///   the mitigation for what memoisation costs: the second test to ask for
    ///   `Drivetrain` no longer re-verifies that compiling it is deterministic,
    ///   so one test keeps doing exactly that.
    pub(crate) fn compile_specimen_uncached(name: &str) -> FromWorker {
        let path = PathBuf::from(format!("{}/specimens/{name}.mo", env!("CARGO_MANIFEST_DIR")));
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        w.compile(&path, &|_: FromWorker| {})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::test_msl::*;
    // `Mutex` — a mutual exclusion lock. Wrapping `WorkerState` in `Mutex`
    // lets multiple test functions share it safely, but only one can access
    // it at a time (the others block). This is how the tests run serially
    // against a single MSL-loaded session.
    //
    // `OnceLock` — a thread-safe cell that can be written to exactly once.
    // Used here for lazy one-time initialization of the shared worker.
    // `OnceLock` is the thread-safe equivalent of `OnceCell` (or Python's
    // `functools.lru_cache` with `maxsize=1` conceptually).

    /// End-to-end: after resolving `RotationalInertia` against the MSL, the
    /// component *types* (`type_def_id`) must resolve to their MSL classes.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn resolves_def_ids_against_msl() {
        let FromWorker::Compiled { def_index, stages, .. } = compile_specimen_shared("RotationalInertia") else {
            panic!("expected Compiled");
        };
        assert!(stages.resolve.value.is_some(), "resolve failed: {:?}", stages.resolve.note);
        assert!(!def_index.is_empty(), "no DefIds resolved");

        let names: Vec<&str> = def_index.values().map(|d| d.name.as_str()).collect();
        // The three declared component types resolved to their MSL classes.
        for expected in [
            "Mechanics.Rotational.Components.Inertia",
            "Mechanics.Rotational.Sources.Torque",
            "Blocks.Sources.Constant",
        ] {
            assert!(
                def_index.values().any(|d| d.kind == DefKind::Class && d.name.ends_with(expected)),
                "{expected} not resolved as a class; got {names:?}"
            );
        }
    }

    /// Navigation: after compiling the specimen, opening a class the model
    /// points at (the MSL `Inertia`) returns its IR and its own DefId index.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn open_def_extracts_a_navigated_class() {
        let name = "Modelica.Mechanics.Rotational.Components.Inertia";
        let result = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
            w.compile(path, &|_: FromWorker| {}); // register the specimen document
            let FromWorker::DefTree { result, .. } = w.open_def(name) else {
                panic!("expected DefTree");
            };
            result
        };
        let (value, def_index) = result.expect("Inertia class extracted");
        // It's a class body with a name matching Inertia.
        assert_eq!(value["name"]["text"], serde_json::json!("Inertia"));
        // Its own references resolved, so navigation can continue from here.
        assert!(!def_index.is_empty(), "navigated class has no resolved DefIds");
    }

    /// The drivetrain specimen compiles through the whole pipeline (it
    /// crosses electrical → rotational → translational, so this exercises
    /// connector expansion / flow-sum generation across domains).
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn drivetrain_compiles_through_flatten() {
        let FromWorker::Compiled { model, stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        assert_eq!(model.as_deref(), Some("Drivetrain"));
        assert!(
            stages.flatten.value.is_some(),
            "Drivetrain did not flatten: {:?}",
            stages.flatten.note
        );
    }

    /// The structural stage builds a matching + BLT report for an index-1 model.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn structural_report_for_rotational_inertia() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RotationalInertia") else {
            panic!("expected Compiled");
        };
        let v = stages.structural.value.expect("structural report");
        assert!(v["matching"].as_array().is_some_and(|a| !a.is_empty()), "no matching");
        assert!(v["blocks"].as_array().is_some_and(|a| !a.is_empty()), "no BLT blocks");
        // A plain index-1 ODE sorts into scalar blocks only — no algebraic loop.
        assert_eq!(v["coupled_block_count"], serde_json::json!(0), "unexpected coupled block");
    }

    /// The proportional-loop specimen closes an algebraic feedback loop, so
    /// structural analysis MUST report a coupled block (a simultaneous algebraic
    /// SCC) — the case the BLT spy-plot draws as a box. This is the specimen's
    /// whole reason for existing, so guard it.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn proportional_loop_has_a_coupled_block() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("ProportionalLoop") else {
            panic!("expected Compiled");
        };
        let v = stages.structural.value.unwrap_or_else(|| panic!("no structural report: {:?}", stages.structural.note));
        let count = v["coupled_block_count"].as_u64().unwrap_or(0);
        assert!(count >= 1, "expected a coupled algebraic block, got {count}; blocks = {}", v["blocks"]);
        // The coupled block should carry a tearing report (iteration variable(s)).
        let coupled = v["blocks"].as_array().into_iter().flatten()
            .find(|b| b["kind"] == serde_json::json!("coupled"))
            .expect("a coupled block");
        assert!(coupled["size"].as_u64().unwrap_or(0) >= 2, "coupled block must be size >= 2");
    }

    /// Compile a `specimens/<name>.mo` against the MSL and return its structural
    /// report JSON — shared by the block-structure guards below.
    fn structural_report_for(name: &str) -> serde_json::Value {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
            panic!("expected Compiled");
        };
        stages.structural.value.unwrap_or_else(|| panic!("no structural report for {name}: {:?}", stages.structural.note))
    }

    fn block_kinds(v: &serde_json::Value) -> Vec<String> {
        v["blocks"].as_array().into_iter().flatten()
            .filter_map(|b| b["kind"].as_str().map(str::to_owned))
            .collect()
    }

    /// MixedLoop brackets an algebraic loop with scalar solves, so its BLT must
    /// contain BOTH scalar and coupled blocks — the mixed spy-plot case.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn mixed_loop_has_scalar_and_coupled_blocks() {
        let v = structural_report_for("MixedLoop");
        assert_eq!(v["coupled_block_count"], serde_json::json!(1));
        let kinds = block_kinds(&v);
        assert!(
            kinds.iter().any(|k| k == "scalar") && kinds.iter().any(|k| k == "coupled"),
            "expected mixed scalar + coupled blocks, got {kinds:?}"
        );
    }

    /// TwoLoops chains two algebraic loops, so structural analysis must report
    /// TWO coupled blocks (two orange boxes).
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn two_loops_has_two_coupled_blocks() {
        let v = structural_report_for("TwoLoops");
        assert_eq!(v["coupled_block_count"], serde_json::json!(2));
    }

    /// NonlinearLoop is structurally identical to ProportionalLoop (structure is
    /// blind to the nonlinearity) — still one coupled block.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn nonlinear_loop_has_a_coupled_block() {
        let v = structural_report_for("NonlinearLoop");
        assert_eq!(v["coupled_block_count"], serde_json::json!(1));
    }

    /// The `dae_prepare` funnel (mirroring rumoca-sim's internal
    /// `prepare_dae_for_structural_analysis` — the shared prep the simulator and
    /// `--inspect structure` both run) reduces Drivetrain's **singular, high-index**
    /// DAE to a non-singular, structurally analyzable one. This confirms Rumoca can
    /// index-reduce (not blocked-on-upstream) and pins the exact public API the
    /// observatory stage will call. NOTE: HRW mirrors Rumoca's funnel *order*;
    /// re-verify it against `rumoca-sim/src/solve_lowering/structural_lowering.rs`
    /// on a pin bump.
    #[test]
    fn drivetrain_index_reduces_from_singular_to_solvable() {
        let report = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/Drivetrain.mo"));
            let source = std::fs::read_to_string(path).unwrap();
            let uri = path.to_string_lossy().to_string();
            w.session.update_document(&uri, &source);
            let qualified = w.session.qualify_model_name(&uri, "Drivetrain");
            w.session.compile_model_strict_reachable_with_recovery(&qualified)
        };
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            _ => panic!("expected a Success result for Drivetrain"),
        };
        // Before: the raw DAE is structurally singular (high index).
        let before = rumoca_phase_structural::build_structural_report(&cr.dae);
        assert!(before.is_err(), "expected Drivetrain to start singular, got {before:?}");

        // Apply the index-reduction funnel, then re-analyze.
        let mut reduced = cr.dae.clone();
        index_reduce_for_structural_analysis(&mut reduced);
        let after = rumoca_phase_structural::build_structural_report(&reduced);
        assert!(
            after.is_ok(),
            "index reduction should make Drivetrain structurally analyzable, got {after:?}"
        );
    }


    /// Blow-up: a capacitor directly across an ideal source can't be
    /// consistently initialized — its state voltage is pinned to the source. Unlike
    /// Drivetrain, index reduction can NOT rescue it: both Structural and Index
    /// reduction stay singular (an observable initialization blow-up).
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn capacitor_loop_is_singular_and_irreducible() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop") else {
            panic!("expected Compiled");
        };
        assert!(stages.flatten.value.is_some(), "CapacitorLoop should still flatten");
        assert!(stages.structural.note_is_error(), "expected singular Structural");
        assert!(stages.structural.value.as_ref().unwrap().get("error").is_some(),
            "singular Structural should carry error details");
        assert!(stages.index_reduction.note_is_error(),
            "index reduction should NOT rescue a capacitor-across-source loop");
        assert!(stages.index_reduction.value.as_ref().unwrap().get("error").is_some(),
            "irreducible index reduction should carry error details");
    }

    /// The Initialization stage plans a consistent initial state for the RC
    /// circuit — a non-empty IC plan plus the ground-current relaxation hint.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn rc_circuit_has_an_ic_plan() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RcCircuit") else {
            panic!("expected Compiled");
        };
        let v = stages.initialization.value.unwrap_or_else(|| panic!("no IC plan: {:?}", stages.initialization.note));
        assert!(v["block_count"].as_u64().unwrap_or(0) >= 1, "expected a non-empty IC plan");
        assert!(v["relaxation_hint"].is_object(), "expected a relaxation hint (ground-current redundancy)");
        // Well-posed init must NOT be mis-flagged as over-determined (idea #6).
        assert_ne!(v["determinacy"]["verdict"], serde_json::json!("over-determined"));
    }

    /// Idea #6: over-specified initialization is flagged. `OverInitRc` pins the
    /// capacitor voltage twice (`C.v = 0` and `der(C.v) = 0`), so the
    /// Initialization stage reports an over-determined init (surplus > 0) with a
    /// red note — the pure init blow-up `build_ic_plan` alone doesn't catch.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn over_init_rc_is_flagged_over_determined() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("OverInitRc") else {
            panic!("expected Compiled");
        };
        let init = &stages.initialization;
        let v = init.value.as_ref().expect("IC plan");
        assert_eq!(v["determinacy"]["verdict"], serde_json::json!("over-determined"));
        assert!(v["determinacy"]["surplus_over_states"].as_i64().unwrap_or(0) >= 1);
        // `Flagged`, not `Failed` — and the distinction is the point of the enum.
        // The IC plan above is real; Rumoca simply also reported that it is
        // over-determined. Asserting `note_is_error()` here would pass equally for
        // a stage that produced nothing at all.
        assert_eq!(init.outcome, Outcome::Flagged, "over-determined init is flagged, not failed");
    }



    /// HRW can RUN a model, not just inspect it. Lower
    /// `SingleInertia`'s DAE to a `SolveModel` and simulate it, checking the
    /// trajectory is produced AND numerically right: constant torque tau=1 with
    /// J=1 gives der(w)=1, so w(t)=t and w(2) is ~2.
    #[test]
    fn single_inertia_simulates_to_a_correct_trajectory() {
        let report = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo"));
            let src = std::fs::read_to_string(path).unwrap();
            let uri = path.to_string_lossy().to_string();
            w.session.update_document(&uri, &src);
            let q = w.session.qualify_model_name(&uri, "SingleInertia");
            w.session.compile_model_strict_reachable_with_recovery(&q)
        };
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            _ => panic!("expected Success for SingleInertia"),
        };
        let sm = rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae).expect("lower DAE -> SolveModel");
        let opts = rumoca_sim::SimOptions { t_end: 2.0, ..Default::default() };
        let result = rumoca_sim::simulate_solve_model(&sm, &opts).expect("simulate");

        assert!(result.times.last().copied().unwrap_or(0.0) >= 1.99, "should integrate to t_end");
        let w_idx = result.names.iter().position(|n| n == "w").expect("w in outputs");
        assert_eq!(result.data[w_idx].len(), result.times.len(), "trajectory length = time points");
        let w_final = *result.data[w_idx].last().unwrap();
        assert!((w_final - 2.0).abs() < 0.05, "w(2) should be ~2.0 (constant torque), got {w_final}");
    }


    /// The stiff bench actuator (a DC motor spinning up an inertial
    /// load) simulates — the Auto solver (BDF) copes with the ~1000x separation
    /// between the fast winding (L/R ~ 1e-4 s) and the slow rotor (J = 0.05). The
    /// current is driven high and the load spins up.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn bench_actuator_simulates_stiff_spinup() {
        let d = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/BenchActuator.mo"));
            w.simulate(path, "BenchActuator", 0.5, &|_: FromWorker| {})
        }
        .expect("simulate BenchActuator");
        let get = |name: &str| -> f64 {
            let i = d.names.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} in outputs"));
            *d.data[i].last().unwrap()
        };
        assert!(get("L.i") > 5.0, "winding current should be driven high");
        assert!(get("load.w") > 1.0, "the load should spin up");
        // Smooth trajectories: BenchActuator has a bare zero-crossing but no
        // discrete update, so the plot must never break its (coarsely sampled,
        // steep) current spike into false discontinuities.
        assert!(!d.has_discontinuities, "BenchActuator has no discrete updates — all trajectories continuous");
    }

    /// The discontinuity-plotting helper. A smooth ramp is one segment;
    /// a signal with a reinit-style jump splits into two, breaking at the jump so
    /// the plot won't slope a line across it. Calibrated against BouncingBall's `v`
    /// (smooth step ~0.06, bounce jump ~8 — a ~40x separation).
    #[test]
    fn discontinuity_segments_break_at_jumps() {
        // Smooth monotone ramp → a single segment.
        let ramp: Vec<f64> = (0..50).map(|i| f64::from(i) * 0.1).collect();
        assert_eq!(discontinuity_segments(&ramp), vec![0..50]);
        // A falling ramp that flips sign once (like a single bounce) → two segments,
        // split right at the jump.
        let mut v: Vec<f64> = (0..40).map(|i| -f64::from(i) * 0.1).collect(); // 0 → -3.9
        v.extend((0..40).map(|i| 3.0 - f64::from(i) * 0.1)); // jumps -3.9 → +3.0
        let segs = discontinuity_segments(&v);
        assert_eq!(segs.len(), 2, "one jump → two segments, got {segs:?}");
        assert_eq!(segs[0], 0..40, "first segment ends at the pre-jump sample");
        assert_eq!(segs[1], 40..80, "second segment starts at the post-jump sample");
    }

    /// End-to-end: BouncingBall is hybrid, and its velocity trajectory
    /// breaks into several segments (one per bounce) while its height stays one
    /// continuous curve.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn bouncing_ball_velocity_plots_as_discontinuous() {
        let data = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/BouncingBall.mo"));
            w.simulate(path, "BouncingBall", 3.0, &|_: FromWorker| {})
        }
        .expect("simulate BouncingBall");
        assert!(data.has_discontinuities, "BouncingBall reinits v at each bounce");
        let v = &data.data[data.names.iter().position(|n| n == "v").expect("v")];
        let h = &data.data[data.names.iter().position(|n| n == "h").expect("h")];
        assert!(
            discontinuity_segments(v).len() > 1,
            "velocity flips at each bounce → multiple segments"
        );
        assert_eq!(
            discontinuity_segments(h).len(),
            1,
            "height is continuous across bounces → one segment"
        );
    }

    /// The worker's `simulate` path (compile → lower → integrate) runs a
    /// hybrid model — BouncingBall — and returns trajectories. Exercises event
    /// handling in the solver (the ball must stay ~above the floor).
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn worker_simulate_runs_bouncing_ball() {
        let data = {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/BouncingBall.mo"));
            w.simulate(path, "BouncingBall", 3.0, &|_: FromWorker| {})
        }
        .expect("simulate BouncingBall");
        assert!(!data.times.is_empty(), "should produce a trajectory");
        let h_idx = data.names.iter().position(|n| n == "h").expect("h in outputs");
        assert!(
            data.data[h_idx].iter().all(|&h| h > -0.5),
            "the ball should stay ~above the floor (events reflect it)"
        );
    }


    /// The Solve-lowering stage (phase 8) lowers the DAE to a `SolveModel`
    /// (the solvable form the simulator consumes) and renders it.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn single_inertia_lowers_to_a_solve_model() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let v = stages.solve_lowering.value.expect("SolveModel IR");
        assert!(v.get("problem").is_some(), "SolveModel should carry the solve problem");
        assert!(v.get("variable_meta").is_some(), "SolveModel should carry variable metadata");
    }

    /// BouncingBall is a hybrid model — the Events stage reports its
    /// condition (`h <= 0`) + discrete update (the `reinit`). A smooth model
    /// (SingleInertia) reports none.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn bouncing_ball_has_events_smooth_model_has_none() {
        let total_events = |v: &serde_json::Value| -> u64 {
            v["summary"].as_object().into_iter().flatten()
                .filter_map(|(_, x)| x.as_u64()).sum()
        };
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("BouncingBall") else {
            panic!("expected Compiled");
        };
        let v = stages.events.value.expect("events IR");
        assert!(total_events(&v) >= 1, "BouncingBall should have hybrid structure");
        assert!(
            v["discrete_updates"]["real_updates_f_z"].as_array().is_some_and(|a| !a.is_empty()),
            "expected the reinit as a discrete real update"
        );

        let FromWorker::Compiled { stages: smooth_stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        assert_eq!(total_events(&smooth_stages.events.value.expect("events IR")), 0, "SingleInertia is smooth");
    }

    /// The parked hand-built PlanarMechanics library (the four-bar-linkage
    /// prerequisite, deferred until Rumoca's Rust-path reduction handles nonlinear
    /// holonomic constraints — see DECISIONS.md) still parses as a source root, so
    /// it doesn't bit-rot while deferred.
    #[test]
    fn planar_mechanics_library_parses() {
        let roots = vec![PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/lib/PlanarMechanics.mo"))];
        let mut state = WorkerState::new();
        let loaded = state.load_libraries(roots).expect("planar mechanics library should parse");
        assert!(loaded >= 1, "expected the planar mechanics library to load");
    }

    /// For the high-index Drivetrain, the raw `structural` stage is singular
    /// **and still produces IR**, and `index_reduction` then makes it solvable —
    /// the before/after the two tabs show side by side.
    ///
    /// The comment here used to say "singular (no IR)" while the line below
    /// asserted the IR was there. That contradiction is the one
    /// [`Outcome::Flagged`] exists to end.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn drivetrain_index_reduction_stage_recovers_singular() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        assert_eq!(
            stages.structural.outcome, Outcome::Flagged,
            "raw Structural is singular for Drivetrain — flagged, not failed",
        );
        assert!(stages.structural.value.as_ref().unwrap().get("error").is_some(),
            "singular Structural should carry error details");
        let v = stages.index_reduction.value.unwrap_or_else(|| {
            panic!("index reduction should recover Drivetrain: {:?}", stages.index_reduction.note)
        });
        assert!(v["coupled_block_count"].as_u64().is_some(), "reduced report missing block count");
        let red = &v["reduction"];
        assert!(red["funnel_completed"].as_bool() == Some(true), "funnel should complete for Drivetrain");
        let steps = red["steps"].as_array().expect("steps array");
        assert!(!steps.is_empty(), "should have logged funnel steps");
        assert!(red["n_states_before"].as_u64().unwrap() > 0);
    }

    /// **Every constructor maps to exactly one outcome, and `note_is_error()`
    /// still says what the old boolean field said.**
    ///
    /// The second half is what makes this split safe to land: it changed no
    /// colour and no control flow, because every former reader of the field now
    /// calls the method and sees the identical answer. Only code that asks for
    /// [`Stage::outcome`] can tell `Flagged` from `Failed`.
    #[test]
    fn each_constructor_reaches_one_outcome_and_colour_is_unchanged() {
        let v = || serde_json::json!({ "ir": true });
        let cases = [
            (Stage::ok(v()), Outcome::Ok, false),
            (Stage::ok_with_note(v(), "already index-1"), Outcome::Ok, false),
            (Stage::info("not reached"), Outcome::Ok, false),
            (Stage::recovered(v(), "singular"), Outcome::Flagged, true),
            (Stage::err("boom"), Outcome::Failed, true),
            (Stage::err_with_details(serde_json::json!({"kind": "singular"}), "boom"),
             Outcome::Failed, true),
        ];
        for (stage, want, red) in cases {
            assert_eq!(stage.outcome, want, "note: {:?}", stage.note);
            assert_eq!(
                stage.note_is_error(), red,
                "colour must match the pre-split boolean for {want:?}",
            );
        }

        // `recovered` keeps the caller's IR; `err_with_details` replaces it with
        // the error payload. Same JSON *shape*, opposite meaning — the conflation
        // that motivated the enum.
        assert_eq!(Stage::recovered(v(), "n").value.unwrap()["ir"], serde_json::json!(true));
        assert!(Stage::err_with_details(v(), "n").error_json().is_some());
        assert!(Stage::ok(v()).error_json().is_none(), "a clean stage carries no error payload");
    }

    /// **The miscount, pinned.** `Drivetrain` compiles all the way through, yet
    /// two of its stages set the old `note_is_error` flag — so a census counting
    /// that boolean would report a healthy high-index model as broken.
    ///
    /// This is not hypothetical: it produced a false finding on 2026-07-29
    /// (`docs/ideas.md` #51), which is why `docs/fidelity-plan.md` sequences the
    /// three-way split ahead of any harness that counts outcomes at scale.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_healthy_high_index_compile_has_no_failed_stage() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };

        let failed: Vec<_> = StageKind::COMPILATION
            .iter()
            .filter(|&&k| stages.get(k).outcome == Outcome::Failed)
            .map(|&k| (k, stages.get(k).note.clone()))
            .collect();
        assert!(failed.is_empty(), "Drivetrain should reach the end of the pipeline; failed: {failed:?}");

        let flagged: Vec<_> = StageKind::COMPILATION
            .iter()
            .filter(|&&k| stages.get(k).outcome == Outcome::Flagged)
            .collect();
        assert!(
            !flagged.is_empty(),
            "Drivetrain is high-index — at least Structural must be flagged, or this test \
             has stopped guarding anything",
        );

        // And the pipeline really did finish, rather than merely not failing.
        assert!(stages.solve_lowering.value.is_some(), "solve lowering should have produced a model");
    }

    /// A singular Structural stage carries structured error data (equation
    /// and unknown counts, rank deficiency, unmatched names) plus the
    /// incidence matrix and partial matching for UI rendering.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn singular_structural_carries_summary_data() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        let v = stages.structural.value.as_ref().expect("singular Structural should have a value");
        let err = &v["error"];
        assert_eq!(err["kind"].as_str(), Some("singular"));
        assert!(err["n_equations"].as_u64().unwrap() > 0);
        assert!(err["n_unknowns"].as_u64().unwrap() > 0);
        assert!(err["rank_deficiency"].as_u64().unwrap() > 0);
        assert!(!err["unmatched_equations"].as_array().unwrap().is_empty());
        assert!(!err["unmatched_unknowns"].as_array().unwrap().is_empty());
        let inc = &v["incidence"];
        assert!(inc["n_eq"].as_u64().unwrap() > 0);
        let matching = v["matching"].as_array().expect("should have partial matching");
        assert!(!matching.is_empty(), "partial matching should be non-empty");
        let mat = crate::incidence_view::IncidenceMatrix::from_report(v)
            .expect("singular structural report should parse as IncidenceMatrix");
        assert!(mat.n_eq() > 0);
    }

    /// Drivetrain's index-reduction trace produces animation frames — the
    /// constrained-dummy reduction finds multiple demotions, each emitting
    /// BeginState, Differentiated, and Demoted frames.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn drivetrain_index_reduction_produces_trace_frames() {
        let FromWorker::Compiled { index_reduction_frames, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        assert!(!index_reduction_frames.is_empty(),
            "Drivetrain should produce index-reduction animation frames");
        use rumoca_phase_structural::dae_prepare::IndexReductionStep;
        let n_demoted = index_reduction_frames.iter()
            .filter(|f| matches!(&f.step, IndexReductionStep::Demoted { .. }))
            .count();
        assert!(n_demoted >= 4, "expected at least 4 demotions, got {n_demoted}");
        let n_differentiated = index_reduction_frames.iter()
            .filter(|f| matches!(&f.step, IndexReductionStep::Differentiated { .. }))
            .count();
        assert!(n_differentiated >= 4, "expected at least 4 differentiations, got {n_differentiated}");
    }

    /// The trace opens on `Start`, so the animation has a visible "before".
    ///
    /// Without it the replay begins on the first `BeginState` — which announces
    /// an intention and reads as though reduction had already happened — and no
    /// frame anywhere shows the unreduced system.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn index_reduction_trace_opens_on_the_starting_system() {
        let FromWorker::Compiled { index_reduction_frames, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        use rumoca_phase_structural::dae_prepare::IndexReductionStep;
        let first = index_reduction_frames.first().expect("frames");
        let IndexReductionStep::Start { states, equations } = &first.step else {
            panic!("first frame should be Start, got {:?}", first.step);
        };
        assert!(!states.is_empty(), "Drivetrain has states entering reduction");
        assert!(*equations > 0, "Drivetrain has equations entering reduction");
        assert!(first.demoted_so_far.is_empty(),
            "nothing is demoted by the traced passes before they begin");
        // Exactly one — the two traced passes must not each contribute a start.
        let n_start = index_reduction_frames.iter()
            .filter(|f| matches!(&f.step, IndexReductionStep::Start { .. }))
            .count();
        assert_eq!(n_start, 1, "expected a single opening frame, got {n_start}");
    }

    /// The index reduction stage embeds a "before" report with the raw
    /// (pre-reduction) DAE's incidence matrix and partial matching.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn drivetrain_index_reduction_has_before_report() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        let v = stages.index_reduction.value.expect("index reduction should succeed");
        let before = &v["before"];
        assert!(before.is_object(), "missing 'before' sub-object in index reduction JSON");
        let inc = &before["incidence"];
        assert!(inc["n_eq"].as_u64().unwrap() > 0, "before incidence should have equations");
        assert!(inc["n_var"].as_u64().unwrap() > 0, "before incidence should have unknowns");
        let matching = before["matching"].as_array().expect("before should have matching");
        assert!(!matching.is_empty(), "partial matching should be non-empty");
        let n_eq = inc["n_eq"].as_u64().unwrap() as usize;
        assert!(matching.len() < n_eq, "partial matching should be incomplete (singular)");
    }

    /// The "before" report is parseable by `IncidenceMatrix::from_report`,
    /// enabling the Before/After split view on the Index Reduction tab.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn drivetrain_before_report_parseable_as_incidence() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        let v = stages.index_reduction.value.expect("index reduction should succeed");
        let before = &v["before"];
        let mat = crate::incidence_view::IncidenceMatrix::from_report(before)
            .expect("before report should parse into an IncidenceMatrix");
        assert!(mat.n_eq() > 0);
        assert!(mat.n_var() > 0);
        let caption = mat.caption();
        assert!(caption.contains("rank deficiency"), "singular system should show rank deficiency: {caption}");

        // The after incidence must resolve matching (equation names must
        // agree between the structural report's matching array and the
        // incidence rows — both use the labeled form like "f_x[0] (origin)").
        let after_mat = crate::incidence_view::IncidenceMatrix::from_report(&v)
            .expect("after report should parse into an IncidenceMatrix");
        let after_caption = after_mat.caption();
        assert!(after_caption.contains("full rank"),
            "reduced system should be full rank: {after_caption}");
    }

    /// For an already index-1 system, the "before" report still exists (so
    /// the split view code doesn't crash), but the note says "index-1".
    #[test]
    fn single_inertia_index_reduction_note_says_index_1() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let note = stages.index_reduction.note.as_deref().unwrap_or("");
        assert!(!note.contains("singular"), "SingleInertia should not be singular: {note}");
        assert!(note.contains("index-1"), "note should mention index-1: {note}");
        let v = stages.index_reduction.value.expect("index reduction should succeed");
        assert!(v.get("before").is_some(), "before report should exist even for index-1 systems");
    }

    /// MotorWithBrake produces trace frames from the missing-derivative path
    /// (index_reduce_missing_state_derivatives) — 1 EMF demotion.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn motor_with_brake_index_reduction_produces_trace_frames() {
        let FromWorker::Compiled { index_reduction_frames, .. } = compile_specimen_shared("MotorWithBrake") else {
            panic!("expected Compiled");
        };
        assert!(!index_reduction_frames.is_empty(),
            "MotorWithBrake should produce index-reduction animation frames");
    }

    /// A scratch specimen compiles like any other (ideas #42).
    ///
    /// The listing and marking are tested in `app`; this is the half that matters for
    /// answering a question — Claude writes a probe mid-conversation and it goes
    /// through the same pipeline as the curated corpus, with the same IR available.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_scratch_specimen_compiles_end_to_end() {
        let path = std::path::Path::new(crate::bridge::SCRATCH_SPECIMEN_DIR)
            .join("ScratchProbe.mo");
        if !path.exists() {
            return; // no probe written in this checkout
        }
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let FromWorker::Compiled { stages, model, .. } = w.compile(&path, &|_: FromWorker| {})
        else {
            panic!("expected Compiled");
        };
        assert_eq!(model.as_deref(), Some("ScratchProbe"));
        assert!(
            stages.solve_lowering.value.is_some() && !stages.solve_lowering.note_is_error(),
            "a scratch probe reaches the end of the pipeline like any specimen",
        );
        // And its IR is real: one state, from `tau * der(x) = -x`.
        let n_states = stages
            .initialization
            .value
            .as_ref()
            .and_then(|v| v.get("n_states"))
            .and_then(serde_json::Value::as_u64);
        assert_eq!(n_states, Some(1), "the probe has exactly one state");
    }

    /// **HRW's re-derived tearing matches Rumoca's own report.**
    ///
    /// `docs/fidelity-plan.md` F1, and the first test of the question Doug raised: does
    /// HRW represent what Rumoca *decided*, or something of its own?
    ///
    /// The tearing animation does not read the compiler's result — it **re-runs the
    /// algorithm** on each coupled block to produce frames. Until 2026-07-30 nothing
    /// compared the two, so they agreed by assumption. A divergence here would mean an
    /// animation teaching a decision the compiler never made, which is the worst failure
    /// available to a tool whose purpose is explanation.
    ///
    /// The non-vacuity guard is not decoration: a model with no coupled block reports
    /// `[]` and derives `[]` — agreement on nothing. Without the guard a corpus of such
    /// models would pass while testing nothing at all.
    ///
    /// **Compared per tab, against the DAE that tab animates.** The Structural and Index
    /// Reduction tabs describe *different systems* (`App::tearing_dae` re-runs the
    /// reduction funnel for the latter), so comparing one tab's re-derivation against the
    /// other's report tests nothing and fails on models that are singular before
    /// reduction. Singular stages are skipped because `structural_view_available` hides
    /// the tearing view there — a re-derivation the UI never shows is not a
    /// misrepresentation.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn hrw_rederived_tearing_matches_rumocas_report() {
        /// The tear variables Rumoca's report lists, flattened across blocks in
        /// report order — the same order `tear_variable_names` walks.
        fn reported_tears(report: &serde_json::Value) -> Vec<String> {
            report
                .get("blocks")
                .and_then(serde_json::Value::as_array)
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter_map(|b| b.get("tearing")?.get("tear_vars")?.as_array())
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default()
        }

        let mut tabs_with_tears = 0usize;

        for name in F1_MODELS {
            let FromWorker::Compiled { stages, dae, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));

            // (tab, the DAE HRW animates there, the report it shows beside it)
            let mut reduced = dae.clone();
            index_reduce_in_place(&mut reduced);
            let cases: [(&str, &rumoca_ir_dae::Dae, &Stage); 2] = [
                ("Structural", &dae, &stages.structural),
                ("IndexReduction", &reduced, &stages.index_reduction),
            ];

            for (tab, tab_dae, stage) in cases {
                // Singular stages hide the tearing view entirely.
                if stage.outcome != Outcome::Ok {
                    continue;
                }
                let Some(report) = stage.value.as_ref() else { continue };

                let reported = reported_tears(report);
                let derived =
                    crate::tearing_anim::TearingAnimation::record(tab_dae).tear_variable_names();

                assert_eq!(
                    derived, reported,
                    "{name} / {tab}: the tearing animation re-derives a different answer \
                     than the compiler reported — it would be teaching a decision Rumoca \
                     never made",
                );
                if !reported.is_empty() {
                    tabs_with_tears += 1;
                }
            }
        }

        assert!(
            tabs_with_tears >= 4,
            "only {tabs_with_tears} tabs actually tore anything; the rest agreed on an \
             empty list, which tests nothing",
        );
    }

    /// **A library model compiles by name, all the way through.**
    ///
    /// The entry point Test mode needs to open a report row, and the one
    /// fidelity testing at MSL scale needs — checking HRW's representation of an
    /// MSL model means compiling it *through HRW's own path*.
    ///
    /// Deliberately picks a model nested deep inside a **multi-class** file:
    /// `CriticalDamping` sits at lines 1498-1620 of `Blocks/Continuous.mo`. The
    /// specimen path takes "the first class in the file" as the model, which
    /// would silently compile something else entirely here — so this is the case
    /// that proves the by-name path is not the file path in disguise.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_library_model_compiles_by_qualified_name() {
        const NAME: &str = "Modelica.Blocks.Continuous.CriticalDamping";
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let FromWorker::Compiled { model, stages, dae, identifier_index, .. } =
            w.compile_model_by_name(NAME, &|_: FromWorker| {})
        else {
            panic!("expected Compiled");
        };

        assert_eq!(
            model.as_deref(),
            Some("CriticalDamping"),
            "the requested model, not the first class in a file of many",
        );
        let dae = dae.expect("a library model should reach a DAE");
        assert!(
            !dae.continuous.equations.is_empty(),
            "CriticalDamping is a real block; an empty DAE means the wrong class was compiled",
        );

        // Every compilation stage produced something — the by-name path is the
        // whole pipeline, not a shortcut to one phase.
        for kind in StageKind::COMPILATION {
            let stage = stages.get(*kind);
            assert!(
                stage.value.is_some() || stage.note.is_some(),
                "{kind:?} produced neither IR nor a note",
            );
        }
        assert_eq!(stages.parse.outcome, Outcome::Ok, "parse: {:?}", stages.parse.note);
        assert_eq!(stages.flatten.outcome, Outcome::Ok, "flatten: {:?}", stages.flatten.note);

        // Source-linked features work too, which is the half that needs the
        // declaring document rather than merely the name.
        let index = identifier_index.expect("identifier index");
        assert!(
            !index.variables.is_empty(),
            "no identifiers indexed — the library document's source text did not reach the index",
        );
    }

    /// A name that is not a class is refused with a message that says so, rather
    /// than compiling something adjacent or panicking.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn an_unknown_qualified_name_is_refused() {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let FromWorker::Compiled { stages, dae, .. } =
            w.compile_model_by_name("Modelica.Nope.NotAClass", &|_: FromWorker| {})
        else {
            panic!("expected Compiled");
        };
        assert!(dae.is_none(), "nothing should have been compiled");
        let note = stages.parse.note.unwrap_or_default();
        assert!(
            note.contains("not a class in the loaded libraries"),
            "the refusal should name the problem; got {note:?}",
        );
    }

    /// **Compiling a library model does not disturb the session.**
    ///
    /// The by-name path deliberately does not register the document — it is
    /// already in a durable source root. If it did, the session would hold the
    /// file twice and a later removal would evict part of the library. This
    /// checks the observable consequence: a specimen compiled afterwards is
    /// unaffected.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_library_compile_leaves_the_session_usable_for_specimens() {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let before = w.session.document_uris().len();
        let _ = w.compile_model_by_name("Modelica.Blocks.Continuous.CriticalDamping", &|_| {});
        let after = w.session.document_uris().len();
        assert_eq!(after, before, "a library compile must not add or remove documents");

        // And a specimen still compiles against the same session.
        let path = PathBuf::from(format!(
            "{}/specimens/ProportionalLoop.mo",
            env!("CARGO_MANIFEST_DIR"),
        ));
        let FromWorker::Compiled { dae, .. } = w.compile(&path, &|_: FromWorker| {}) else {
            panic!("expected Compiled");
        };
        assert!(dae.is_some(), "a specimen must still compile after a library model did");
    }

    /// The specimens F1 re-derives on. Shared by the three checks so a model
    /// added here is covered by all of them at once.
    #[cfg(test)]
    const F1_MODELS: &[&str] = &[
        "ProportionalLoop", "MixedLoop", "TwoLoops", "NonlinearLoop", "Drivetrain",
        "RcCircuit", "SingleInertia", "CapacitorLoop", "BouncingBall", "MotorWithBrake",
    ];

    /// **HRW's re-derived matching matches Rumoca's own report.**
    ///
    /// `docs/fidelity-plan.md` F1, second of three. The incidence view renders the
    /// matching overlay from the report, but [`MatchingAnimation`] **re-runs Kuhn's
    /// algorithm** on the parsed matrix to produce its frames — so the green circles
    /// the animation walks through could in principle end somewhere the compiler
    /// never went.
    ///
    /// The comparison is exact rather than by size, because a maximum matching is
    /// not unique: two matchings of equal cardinality are equally *valid* and still
    /// mean the animation is narrating a different transversal than the one the
    /// solve order was built from. `match_progress` cannot see that difference,
    /// which is why `final_matching` exists.
    ///
    /// What this really exercises is the **JSON round trip** — report → names →
    /// indices → re-run. Both sides call the same Rumoca function, so a divergence
    /// means the row or column order did not survive it.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn hrw_rederived_matching_matches_rumocas_report() {
        let mut compared = 0usize;

        for name in F1_MODELS {
            let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            let Some(report) = stages.structural.value.as_ref() else { continue };
            let Some(mat) = crate::incidence_view::IncidenceMatrix::from_report(report) else {
                continue;
            };
            if mat.n_eq() == 0 {
                continue;
            }

            let derived = crate::matching_anim::MatchingAnimation::from_incidence(&mat).final_matching();
            let reported = mat.reported_matching();

            assert_eq!(
                derived.len(), reported.len(),
                "{name}: re-derived matching covers {} equations, the report {}",
                derived.len(), reported.len(),
            );
            assert_eq!(
                derived, reported,
                "{name}: the matching animation ends on a different transversal than \
                 Rumoca reported — the overlay and the animation would disagree",
            );
            compared += 1;
        }

        assert!(
            compared >= 5,
            "only {compared} models produced an incidence matrix to compare; F1's matching \
             check is testing almost nothing",
        );
    }

    /// **HRW's re-derived BLT blocks match Rumoca's own report.**
    ///
    /// `docs/fidelity-plan.md` F1, third of three. [`TarjanAnimation`] re-runs
    /// matching *and* Tarjan to build its graph, so it is the furthest-removed
    /// re-derivation in HRW — two algorithms deep from anything the compiler said.
    ///
    /// Compared as a **partition**, not a sequence: Tarjan emits components in
    /// reverse topological order while the report lists them in solve order, so
    /// requiring equal ordering would fail on a difference that means nothing.
    /// Requiring equal *sets* still catches the thing that matters — an equation
    /// placed in the wrong block, which is a different solve order and a different
    /// algebraic loop.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn hrw_rederived_blocks_match_rumocas_report() {
        use std::collections::BTreeSet;

        let mut compared = 0usize;
        let mut saw_a_coupled_block = false;

        for name in F1_MODELS {
            let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            let Some(report) = stages.structural.value.as_ref() else { continue };
            let Some(mat) = crate::incidence_view::IncidenceMatrix::from_report(report) else {
                continue;
            };
            let reported = mat.reported_blocks();
            if reported.is_empty() {
                continue;
            }
            let Some(anim) = crate::tarjan_anim::TarjanAnimation::from_incidence(&mat) else {
                continue;
            };

            let as_sets = |bs: Vec<Vec<usize>>| -> BTreeSet<BTreeSet<usize>> {
                bs.into_iter().map(|b| b.into_iter().collect()).collect()
            };
            if reported.iter().any(|b| b.len() > 1) {
                saw_a_coupled_block = true;
            }

            assert_eq!(
                as_sets(anim.final_sccs()), as_sets(reported),
                "{name}: Tarjan re-derives a different block partition than Rumoca \
                 reported — the animation would show the wrong solve order",
            );
            compared += 1;
        }

        assert!(compared >= 5, "only {compared} models had blocks to compare");
        assert!(
            saw_a_coupled_block,
            "every model compared had only singleton blocks; the partition check never \
             had a chance to be wrong",
        );
    }

    /// A resolve failure names the offending reference **and its line**, with the
    /// library noise separated out.
    ///
    /// Two problems fixed together 2026-07-29:
    ///
    /// 1. `Diagnostic::labels` — the `Span` marking exactly where the error is — was
    ///    dropped by every diagnostic emitter in HRW.
    /// 2. The resolve payload was `format!("{e:#}")`: ~39 semicolon-separated items of
    ///    which ~38 were MSL deprecation warnings, the model's real error last. The
    ///    signal was the final 2% of a 2000-character string.
    ///
    /// The fix uses `compile_model_diagnostics` for structured, model-scoped diagnostics
    /// and partitions them by **severity** — so nothing is pattern-matched out of message
    /// text and no real error can be filtered away by a wording change.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_resolve_failure_names_the_reference_and_its_line() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UndefinedRef")
        else {
            panic!("expected Compiled");
        };
        let err = stages
            .resolve
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .expect("a resolve failure must carry a structured payload");

        let errors = err["diagnostics"]["errors"].as_array().expect("errors array");
        assert_eq!(errors.len(), 1, "one error, not 34 items of library noise: {errors:?}");
        assert_eq!(errors[0]["code"], "ER002");
        assert!(
            errors[0]["message"].as_str().is_some_and(|m| m.contains("missingGain")),
            "{}",
            errors[0]["message"],
        );

        // The label is the point: a line Doug can look at.
        let loc = &errors[0]["labels"][0]["location"];
        assert_eq!(loc["line"], 9, "the reference is on line 9: {loc}");
        assert!(
            loc["line_text"].as_str().is_some_and(|t| t.contains("missingGain")),
            "line_text must be quotable: {loc}",
        );

        // Warnings are kept, deduplicated, and clearly not the cause.
        let warnings = &err["diagnostics"]["warnings"];
        let total = warnings["total"].as_u64().expect("total");
        let distinct = warnings["distinct"].as_array().expect("distinct").len();
        assert!(total > distinct as u64, "{total} warnings collapse to {distinct} distinct");

        // Never lossy: the original concatenated message survives verbatim.
        assert!(err["message"].as_str().is_some_and(|m| m.contains("missingGain")));
    }

    /// A typecheck failure names its line too, through the same shared helper.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_typecheck_failure_names_its_line() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("DimensionMismatch")
        else {
            panic!("expected Compiled");
        };
        let err = stages
            .typecheck
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .expect("a typecheck failure must carry a structured payload");

        let diags = err["diagnostics"].as_array().expect("diagnostics array");
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0]["code"], "ET002");
        assert!(
            diags[0]["message"].as_str().is_some_and(|m| m.contains("dimension mismatch")),
            "{}",
            diags[0]["message"],
        );
        let loc = &diags[0]["labels"][0]["location"];
        assert_eq!(loc["line"], 11, "the offending equation is on line 11: {loc}");
        assert!(loc["line_text"].as_str().is_some_and(|t| t.contains("small = big")), "{loc}");
    }

    /// A library compile reports the **qualified name** as its identity.
    ///
    /// **This is the bug that made every MSL model appear to hang.** The UI's
    /// three staleness checks compare a result's `path` against `App::selected`,
    /// which for a library model holds the qualified name. The worker's
    /// early-error return already reported that; the success path reported the
    /// MSL *file* URI instead. So a successful compile never matched, every
    /// result was discarded as stale, and the UI showed a log full of work with
    /// no stages and a spinner that never stopped.
    ///
    /// **The two returns disagreeing is the defect**, so this asserts they agree.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_library_compile_identifies_itself_by_qualified_name() {
        const NAME: &str = "Modelica.Electrical.Analog.Basic.Resistor";
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let FromWorker::Compiled { path, .. } = w.compile_model_by_name(NAME, &|_| {}) else {
            panic!("expected Compiled");
        };
        assert_eq!(
            path,
            std::path::PathBuf::from(NAME),
            "a library compile must report the qualified name it was asked for. Reporting              the document URI instead makes every result look stale to the UI, which is              indistinguishable from a compile that never finishes",
        );
    }

    /// A library compile **carries the source of the file that declares the model**.
    ///
    /// The source view refused MSL models outright until 2026-08-01, on the stated
    /// grounds that a library model had "no single source file to show". That was
    /// never true — `locate_library_model` reads exactly that file out of the
    /// session *in order to compile it*, then dropped it. Doug: *"The modelica
    /// source view for an MSL model should be just as functional as for an HRW
    /// specimen."*
    ///
    /// Checks the two things the pane cannot work without, and would otherwise fail
    /// at silently: **non-empty text**, and a **declaration line inside it**.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_library_compile_carries_the_declaring_file_source() {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let out = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});
        let FromWorker::Compiled { library_source, .. } = out else {
            panic!("expected Compiled");
        };
        let lib = library_source.expect(
            "a library compile must carry its declaring file: the source view has no other \
             way to get it, and without it the pane renders empty",
        );
        let text = lib.text.clone().expect("the declaring file must be readable");
        assert!(
            !text.trim().is_empty(),
            "empty source would render as a blank pane \u{2014} indistinguishable from the \
             refusal this replaced",
        );
        assert!(
            lib.uri.ends_with(".mo"),
            "the URI names the declaring document, and it is shown to the reader: {}",
            lib.uri,
        );

        // **The declaration line must land inside the file.** `Resistor` opens
        // roughly 1,500 lines into `Basic.mo`, so a reader dropped at line 1 sees a
        // package header and none of what they asked for. An out-of-range line
        // scrolls nowhere and looks like the scroll is broken.
        let lines = text.lines().count() as u32;
        let decl = lib.decl_line.expect("a located class has a start line");
        assert!(
            decl >= 1 && decl <= lines,
            "declaration line {decl} is outside the {lines}-line file it indexes",
        );
        let decl_text = text.lines().nth(decl as usize - 1).unwrap_or("");
        assert!(
            decl_text.contains("Resistor"),
            "line {decl} should be Resistor\u{2019}s declaration, found: {decl_text:?}",
        );
    }

    /// An MSL model's identifiers are indexed **on the lines they occupy**.
    ///
    /// Doug, 2026-08-01: *"Identifiers in the modelica source view of an MSL
    /// model do not seem to be clickable to cause following."*
    ///
    /// The index and the source pane must agree about where a variable is.
    /// `IdentifierIndex::build` counts newlines in the text it is handed to turn
    /// a `source_span` byte offset into a line, and it was handed `""` for every
    /// library model — so **every variable landed on line 1**. The index was
    /// populated, which is why nothing looked broken; it was simply pointing at
    /// the wrong lines, and `clickable_spans` found nothing to underline on the
    /// lines a reader was actually looking at.
    ///
    /// **Line 1 is the tell**, so that is what this asserts against: a real
    /// index over a multi-thousand-line library file cannot have everything on
    /// its first line.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn an_msl_model_indexes_identifiers_on_their_own_lines() {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        let out = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});
        let FromWorker::Compiled { identifier_index, library_source, .. } = out else {
            panic!("expected Compiled");
        };
        let idx = identifier_index.expect("a successful library compile builds an index");
        assert!(
            !idx.variables.is_empty(),
            "an index with no variables makes nothing clickable at all",
        );

        let text = library_source
            .expect("carries its source")
            .text
            .expect("readable");
        let total_lines = text.lines().count() as u32;

        // **Every line must be inside the file the pane renders.** A span
        // resolved against different text can land past the end, where it
        // silently matches nothing.
        for (name, v) in &idx.variables {
            assert!(
                v.source_line >= 1 && v.source_line <= total_lines,
                "{name} is indexed at line {} of a {total_lines}-line file",
                v.source_line,
            );
        }

        // The defect's signature: everything collapsed onto line 1.
        let on_line_1 = idx.variables.values().filter(|v| v.source_line == 1).count();
        assert!(
            on_line_1 < idx.variables.len(),
            "all {} variables are indexed on line 1, which means the index was built \
             against text that is not what the pane shows — the exact defect that \
             made MSL identifiers unclickable",
            idx.variables.len(),
        );
    }

    /// **The compiler's byte offsets and the bytes on screen are the same bytes.**
    ///
    /// Doug asked whether displaying MSL source is a hack, and whether spans
    /// agree between the source view and the stage trees. This is that question
    /// made checkable.
    ///
    /// The pane's text does **not** come from the compiler: Rumoca discards
    /// source-root text (`Document::new(uri, String::new(), ..)`), so HRW re-reads
    /// the declaring file from disk. That leaves two paths to what ought to be one
    /// string, and **agreement becomes a property to maintain rather than a
    /// structural guarantee.** Rumoca's parsed-artifact cache is keyed on a
    /// `blake3` hash of every file's bytes, recomputed on each load, so a file
    /// edited behind the cache invalidates it -- but that is a chain of reasoning,
    /// and this is a measurement.
    ///
    /// **Slicing is the sharp end.** `CriticalDamping` lives ~62,000 bytes into
    /// `Continuous.mo`; if the two texts differed by a single byte anywhere before
    /// it, the slice would land on unrelated characters. Nothing would crash, and
    /// the pane would underline confident nonsense.
    ///
    /// Three files, deliberately: two where the model is the whole file, and one
    /// multi-class file deep enough that a drifting offset could not stay hidden.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn compiler_spans_address_the_text_the_pane_shows() {
        let mut checked = 0usize;
        for name in [
            "Modelica.Electrical.Analog.Basic.Resistor",
            "Modelica.Mechanics.Rotational.Components.Inertia",
            "Modelica.Blocks.Continuous.CriticalDamping",
        ] {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let out = w.compile_model_by_name(name, &|_| {});
            let FromWorker::Compiled { identifier_index, library_source, .. } = out else {
                panic!("{name}: expected Compiled");
            };
            let idx = identifier_index.expect("index");
            let text = library_source.expect("source").text.expect("readable");

            for (var, v) in &idx.variables {
                let leaf = var.rsplit('.').next().unwrap_or(var);
                let (s, e) = v.source_byte_range;

                // **In range, and on a character boundary.** A slice that is
                // merely in range can still be nonsense; `get` returning None on
                // a non-boundary is itself a disagreement signal.
                let slice = text.get(s..e).unwrap_or_else(|| {
                    panic!(
                        "{name}: {var} spans {s}..{e}, which is not a valid slice of the \
                         {}-byte file the pane renders",
                        text.len(),
                    )
                });
                assert!(
                    slice.contains(leaf),
                    "{name}: {var} spans {s}..{e}, which reads {slice:?} -- the compiler's \
                     offsets do not address the text on screen, so every underline and \
                     blamed line in this file points somewhere arbitrary",
                );

                // And the line the index reports must hold it too, since that,
                // not the byte range, is what places the underline.
                let line = text.lines().nth(v.source_line as usize - 1).unwrap_or("");
                assert!(
                    line.contains(leaf),
                    "{name}: {var} is indexed at line {}, which reads {line:?}",
                    v.source_line,
                );
                checked += 1;
            }
        }

        // **Non-vacuity.** Every assertion above lives inside a loop that an empty
        // index would skip entirely, leaving the test green while checking nothing.
        assert!(
            checked >= 10,
            "only {checked} variables checked -- too few to have exercised anything",
        );
    }

    /// The Parse stage of an MSL model **holds the classes its file declares**.
    ///
    /// It used to hold `{"classes":{},"within":null}` for every library model,
    /// coloured as a success, because it parsed the empty string Rumoca keeps in
    /// place of source-root text. **An empty green tab asserts "this model parsed
    /// to nothing"** -- false, and indistinguishable from a model that genuinely
    /// declares nothing. The source view made the contradiction visible: a pane
    /// full of declarations beside a tab claiming the file held none.
    ///
    /// Asserts the requested class is **among** the classes parsed, not that it is
    /// the only one: a library file declares many, and the reader is looking at
    /// all of them in the source view.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn an_msl_models_parse_stage_holds_its_declaring_file() {
        for (qualified, leaf) in [
            ("Modelica.Electrical.Analog.Basic.Resistor", "Resistor"),
            // A multi-class file: `Continuous.mo` declares CriticalDamping among
            // dozens, ~62 KB in. If only the first class survived, this fails.
            ("Modelica.Blocks.Continuous.CriticalDamping", "CriticalDamping"),
        ] {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let FromWorker::Compiled { stages, model, .. } =
                w.compile_model_by_name(qualified, &|_| {})
            else {
                panic!("{qualified}: expected Compiled");
            };
            let value = stages
                .parse
                .value
                .as_ref()
                .unwrap_or_else(|| panic!("{qualified}: the Parse stage produced no value"));
            let classes = value
                .get("classes")
                .and_then(|c| c.as_object())
                .unwrap_or_else(|| panic!("{qualified}: parse value has no classes map"));

            assert!(
                !classes.is_empty(),
                "{qualified}: the Parse stage is empty and reports success, which claims \
                 the file declares nothing while the source view shows it declaring plenty",
            );
            // **The AST is a tree, not a flat list.** `Continuous.mo` declares a
            // *package* `Continuous` holding CriticalDamping among dozens, so a
            // top-level lookup finds only the package. Descending is the point:
            // it proves the whole file was parsed, not just its outer shell.
            fn declares(value: &serde_json::Value, leaf: &str) -> bool {
                match value.get("classes").and_then(|c| c.as_object()) {
                    Some(map) => {
                        map.contains_key(leaf) || map.values().any(|v| declares(v, leaf))
                    }
                    None => false,
                }
            }
            assert!(
                declares(value, leaf),
                "{qualified}: parsed {} top-level classes, none of which declares {leaf}                  anywhere beneath it: {:?}",
                classes.len(),
                classes.keys().take(8).collect::<Vec<_>>(),
            );
            assert_eq!(
                model.as_deref(),
                Some(leaf),
                "{qualified}: the model name must survive, since the caller supplied it",
            );
        }
    }

    /// **HRW's parse of a library file is the compiler's own AST, byte for byte.**
    ///
    /// This is the guard on the whole "second source" question Doug asked: HRW
    /// re-reads the declaring file from disk because Rumoca discards source-root
    /// text, so there are two paths to what ought to be one artifact. If they can
    /// diverge, the Parse tab shows something the compiler never saw.
    ///
    /// **They already agreed on bytes and spans and differed on one field.**
    /// `parse_to_ast`'s `file_name` argument is stamped into every `Location`, and
    /// passing a basename where the session used the full URI made **400 of 400**
    /// MSL documents differ. Passing `&uri` makes it **0 of 400**. Nothing about
    /// that is self-evident, which is why it is measured rather than assumed.
    ///
    /// Serialised comparison rather than structural: it is the serialised form
    /// that reaches the stage tree, so it is the form whose agreement matters.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn hrw_reparse_of_a_library_file_matches_the_sessions_own_ast() {
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        // Any compile populates the session with the MSL documents.
        let _ = w.compile_model_by_name("Modelica.Electrical.Analog.Basic.Resistor", &|_| {});

        let uris: Vec<String> = w
            .session
            .document_uris()
            .into_iter()
            .map(str::to_owned)
            .collect();

        let mut compared = 0usize;
        // A sample, not the whole 2,553: this runs in the pre-commit suite, and
        // the property is uniform -- a divergence in how HRW calls `parse_to_ast`
        // would show in the first handful, not only in the tail.
        for uri in uris.iter().take(120) {
            let Some(doc) = w.session.get_document(uri) else { continue };
            let Some(session_ast) = doc.parsed() else { continue };
            let Ok(text) = std::fs::read_to_string(uri) else { continue };
            let Ok(mine) = rumoca_phase_parse::parse_to_ast(&text, uri) else {
                panic!("{uri}: HRW cannot parse a file the session parsed");
            };
            assert_eq!(
                serde_json::to_string(&mine).unwrap_or_default(),
                serde_json::to_string(session_ast).unwrap_or_default(),
                "{uri}: HRW's re-parse differs from the AST the session holds, so the \
                 Parse tab would show something the compiler never saw",
            );
            compared += 1;
        }

        // **Non-vacuity.** Every `continue` above is a silent skip, and a session
        // that produced no readable documents would leave this green.
        assert!(
            compared >= 50,
            "only {compared} documents compared -- too few to have exercised the property",
        );
    }





    /// A **specimen** compile carries no library source, and must not.
    ///
    /// The pane reads a specimen from its own path so live edits show; seeding the
    /// cache from the compile would silently freeze the text at whatever was last
    /// compiled, and an edited file that keeps rendering its old contents is a far
    /// worse failure than the one being fixed.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_specimen_compile_carries_no_library_source() {
        let FromWorker::Compiled { library_source, .. } = compile_specimen_shared("RcCircuit") else {
            panic!("expected Compiled");
        };
        assert!(
            library_source.is_none(),
            "a specimen\u{2019}s pane must keep reading from disk, or edits stop showing",
        );
    }

    /// **A broken specimen must not poison the next compile.**
    ///
    /// Found 2026-07-29 by auditing the front-end failure payloads. Name resolution runs
    /// over the *whole session*, not just the requested model, and a specimen that failed
    /// to resolve leaves errors in the session's resolved-state cache. So loading a broken
    /// model and then a good one made the good one report **the other file's error** --
    /// which would have Claude diagnosing the wrong model entirely, the priority-1
    /// failure in `docs/tech-debt.md`.
    ///
    /// `remove_document` does *not* clear it, despite
    /// `apply_document_removal_at_revision` calling `invalidate_resolved_state`.
    /// Rebuilding the session does; that is the mitigation, guarded on the previous
    /// compile having actually failed so the reparse is paid only when it buys something.
    /// The root cause is inside Rumoca's cache and is logged as an upstream issue.
    ///
    /// Uses a **fresh** `WorkerState` rather than the shared one, so this cannot pass or
    /// fail because of what other tests happened to compile first.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_broken_specimen_does_not_poison_the_next_compile() {
        let mut w = WorkerState::new();
        w.load_libraries(msl_roots()).expect("load MSL");
        let dir = format!("{}/specimens", env!("CARGO_MANIFEST_DIR"));
        let resolve_note = |w: &mut WorkerState, name: &str| -> (bool, String) {
            let path = PathBuf::from(format!("{dir}/{name}.mo"));
            match w.compile(&path, &|_: FromWorker| {}) {
                FromWorker::Compiled { stages, .. } => {
                    let st = stages.get(StageKind::Resolve);
                    (st.note_is_error(), st.note.clone().unwrap_or_default())
                }
                _ => panic!("expected Compiled for {name}"),
            }
        };

        let (failed, _) = resolve_note(&mut w, "CapacitorLoop");
        assert!(!failed, "CapacitorLoop resolves cleanly on its own");

        let (failed, note) = resolve_note(&mut w, "UndefinedRef");
        assert!(failed, "UndefinedRef references an undeclared name");
        assert!(note.contains("missingGain"), "and says which one: {note}");

        // The moment of truth: the same good specimen, compiled after the broken one.
        let (failed, note) = resolve_note(&mut w, "CapacitorLoop");
        assert!(
            !failed,
            "a good model must not inherit the previous specimen's failure: {note}",
        );
        assert!(
            !note.contains("missingGain"),
            "`missingGain` appears only in UndefinedRef.mo; leaking it here would have \
             Claude diagnosing the wrong file: {note}",
        );
    }

    /// A memoised compile equals a fresh one, stage for stage.
    ///
    /// **This is the price of `docs/ideas.md` #48, paid deliberately.** Memoising
    /// specimens took the full suite from 375s to about 100s, but it *weakens* the
    /// suite: before, the second test to ask for `Drivetrain` re-verified that
    /// compiling it produced the same thing. Now it gets a copy of the first answer,
    /// so nothing checks reproducibility, and a compiler that had become
    /// non-deterministic would sail through a green run.
    ///
    /// So one test keeps doing what the others stopped doing. It compares every
    /// compilation stage's **emitted JSON** rather than a summary, because that tree
    /// is what HRW renders, what the bridge publishes and what the fidelity checks
    /// read -- a difference invisible there is invisible everywhere that matters.
    ///
    /// **Two back-to-back uncached compiles, not memo-versus-fresh.** The first
    /// version compared the memo against a fresh compile and failed on Resolve in
    /// the full suite while passing alone — because those two compiles happen at
    /// *different points in the session's life*, and the shared session accumulates
    /// every specimen the suite has touched. That difference is a property of the
    /// session, not non-determinism, so the comparison could never have been stable.
    /// Compiling twice in a row holds the session constant and isolates the property
    /// actually at issue. *(The session-dependence itself is logged in
    /// `docs/tech-debt.md`; it is adjacent to upstream issue 1.)*
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn compiling_a_specimen_twice_is_reproducible() {
        let memoised = compile_specimen_uncached("Drivetrain");
        let fresh = compile_specimen_uncached("Drivetrain");

        let (FromWorker::Compiled { stages: a, def_index: da, .. }, FromWorker::Compiled { stages: b, def_index: db, .. }) =
            (&memoised, &fresh)
        else {
            panic!("expected Compiled from both");
        };

        for kind in StageKind::COMPILATION {
            let (sa, sb) = (a.get(*kind), b.get(*kind));
            assert_eq!(
                sa.outcome, sb.outcome,
                "{} outcome differs between a memoised and a fresh compile",
                kind.name(),
            );
            assert_eq!(
                sa.value.is_some(), sb.value.is_some(),
                "{} presence differs between a memoised and a fresh compile",
                kind.name(),
            );
            if sa.value != sb.value {
                panic!(
                    "{} emits different JSON on a fresh compile — memoisation is hiding \
                     non-determinism, which is exactly what this test exists to catch",
                    kind.name(),
                );
            }
        }
        assert_eq!(da.len(), db.len(), "def_index size differs");

        // Non-vacuity: comparing two empty pipelines proves nothing.
        assert!(
            StageKind::COMPILATION.iter().filter(|k| a.get(**k).value.is_some()).count() >= 8,
            "expected a substantially compiled Drivetrain; got mostly empty stages",
        );
    }

    /// An unbalanced model reports its balance, not just "DAE construction failed".
    ///
    /// #45 step 2. Until 2026-07-29 this failure path returned a bare informational
    /// note while `error`, `error_code` and `diagnostics` sat in scope unused — making
    /// the **most common Modelica authoring error** (declare a variable, forget its
    /// equation) the least informative failure in the pipeline.
    ///
    /// **This test is also the tripwire for the message-format parse.** The structured
    /// counts are recovered from Rumoca's display string, because `rumoca-compile`
    /// stringifies the typed `ToDaeError::Unbalanced` at its boundary. If that wording
    /// changes, this fails loudly instead of the fields silently disappearing.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn an_unbalanced_model_reports_its_balance() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UnbalancedShaft")
        else {
            panic!("expected Compiled");
        };

        let flatten = &stages.flatten;
        assert!(flatten.note_is_error(), "a failed DAE construction is an error, not an info note");
        let err = flatten
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .expect("the failure must carry a structured payload");

        assert_eq!(err["kind"], "dae_construction");
        assert_eq!(err["error_code"], "rumoca::todae::ED001");
        // 2 equations for 3 unknowns: `tau` is declared and never determined.
        assert_eq!(err["n_equations"], 2, "parsed from the message: {}", err["message"]);
        assert_eq!(err["n_unknowns"], 3, "parsed from the message: {}", err["message"]);
        assert_eq!(err["balance"], -1);
        assert!(
            err["reading"].as_str().is_some_and(|r| r.contains("nothing to determine it")),
            "the direction of the imbalance is the actionable half: {}",
            err["reading"],
        );
    }

    /// **The stage that failed must not be the quietest one.**
    ///
    /// On `UnbalancedShaft` every stage downstream of DAE construction said "not
    /// reached (ToDae failed earlier)", and the DAE tab — the phase that actually
    /// refused — rendered blank. The attribution was a leftover: `flatten_stage`
    /// adopted the `FailedPhase::ToDae` error in 2026-07-29 because Flatten was then
    /// the last tab before Structural, so **the succeeding stage reported the
    /// failure and the failing stage reported nothing.**
    ///
    /// Found by walking `docs/fixture-tours/dae-construction.md`, whose
    /// counterexample stop opens this tab expecting an explanation — the pane-is-a-
    /// reporter rule reaching a pane that was already shipping.
    ///
    /// **Checks the property, not the message**: any stage that produced no IR must
    /// say something, and the one that failed must say at least as much as the ones
    /// that merely never ran.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn the_dae_stage_explains_its_own_absence() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UnbalancedShaft")
        else {
            panic!("expected Compiled");
        };

        let dae = &stages.dae;
        assert!(
            dae.note_is_error(),
            "DAE construction refused; its own tab must record that as an error, not silence",
        );
        let err = dae
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .expect("the DAE tab must carry the structured payload of its own failure");
        assert_eq!(err["kind"], "dae_construction");
        assert_eq!(err["error_code"], "rumoca::todae::ED001");
        assert_eq!(err["n_equations"], 2);
        assert_eq!(err["n_unknowns"], 3);
        assert_eq!(err["balance"], -1);

        // The property. Every stage with no IR explains itself, and the DAE — the
        // one that failed — is not the silent member of that set.
        for &kind in StageKind::COMPILATION {
            let s = stages.get(kind);
            if s.value.is_some() {
                continue;
            }
            assert!(
                s.note.is_some(),
                "{} produced no IR and gave no reason — an empty pane with no note is \
                 indistinguishable from a pane that is still loading",
                kind.name(),
            );
        }

        // Non-vacuity: this specimen must actually fail where the test assumes.
        assert!(
            stages.structural.note.as_deref().is_some_and(|n| n.contains("ToDae")),
            "UnbalancedShaft must still fail in ToDae, or this test is checking nothing",
        );
    }

    /// The balance parse yields nothing rather than something wrong.
    #[test]
    fn the_balance_parse_is_absent_rather_than_wrong() {
        assert_eq!(
            parse_unbalanced("unbalanced model: 2 equations, 3 unknowns (balance = -1)"),
            Some((2, 3, -1)),
        );
        // Any deviation returns None, so a reworded message loses the structured
        // fields and never invents them. A wrong number reads as authoritative — the
        // lesson of the `rank_deficiency` bug.
        assert!(parse_unbalanced("internal todae error: something else").is_none());
        assert!(parse_unbalanced("unbalanced model: two equations, 3 unknowns (balance = -1)").is_none());
        assert!(parse_unbalanced("unbalanced model: 2 equations, 3 unknowns balance = -1").is_none());
    }

    /// A structural failure is reported **in terms of Doug's source** (ideas #45).
    ///
    /// This is the whole diagnostic claim: "unknown `gnd.p.i`" tells Doug nothing
    /// about the model he wrote, while "line 9, `connect(src.n, gnd.p);`" is a
    /// diagnosis. `StructuralError::Singular` has carried `unmatched_unknown_spans`
    /// all along; HRW dropped it until 2026-07-29.
    ///
    /// `CapacitorLoop` is the specimen for this because it fails structurally **and
    /// stays failed** after index reduction — a capacitor straight across an ideal
    /// source is genuinely ill-posed, not merely high-index. `MotorWithBrake` and
    /// `Drivetrain` are also singular but get rescued, so neither is a diagnostic case.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_structural_failure_names_the_source_line() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop")
        else {
            panic!("expected Compiled");
        };

        for (label, stage) in
            [("structural", &stages.structural), ("index_reduction", &stages.index_reduction)]
        {
            let err = stage
                .value
                .as_ref()
                .and_then(|v| v.get("error"))
                .unwrap_or_else(|| panic!("{label} should be singular for CapacitorLoop"));

            let locs = err
                .get("unmatched_unknown_locations")
                .and_then(|v| v.as_array())
                .unwrap_or_else(|| panic!("{label} must carry unmatched_unknown_locations"));
            assert_eq!(locs.len(), 1, "{label}: one unmatched unknown");

            let entry = &locs[0];
            assert_eq!(entry["unknown"], "gnd.p.i", "{label}");
            let loc = &entry["location"];
            assert!(!loc.is_null(), "{label}: the unknown must have a source location");
            assert_eq!(loc["line"], 9, "{label}: gnd.p.i traces to the ground connect()");
            assert!(
                loc["line_text"].as_str().is_some_and(|t| t.contains("connect(src.n, gnd.p)")),
                "{label}: line_text must be quotable back at Doug: {loc:?}",
            );
        }
    }

    /// Rank deficiency comes from the **error's own** counts, not from whatever
    /// incidence the caller happened to pass.
    ///
    /// Regression test for a wrong number found 2026-07-29. The field used to read
    /// `inc.n_eq.max(inc.n_var) - n_matched`, and `index_reduction_stage` passes the
    /// *raw* incidence while its error describes the *reduced* system — so
    /// `CapacitorLoop` reported a deficiency of **7** (14 raw equations minus 7
    /// reduced matches) where the truth is 1.
    ///
    /// A wrong number is worse than a missing one: it reads as authoritative, and
    /// Claude would have quoted it straight into an answer.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn rank_deficiency_is_consistent_with_its_own_counts() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop")
        else {
            panic!("expected Compiled");
        };
        for (label, stage) in
            [("structural", &stages.structural), ("index_reduction", &stages.index_reduction)]
        {
            let err = stage.value.as_ref().and_then(|v| v.get("error")).expect(label);
            let n_eq = err["n_equations"].as_u64().expect("n_equations");
            let n_var = err["n_unknowns"].as_u64().expect("n_unknowns");
            let n_matched = err["n_matched"].as_u64().expect("n_matched");
            let deficiency = err["rank_deficiency"].as_u64().expect("rank_deficiency");
            assert_eq!(
                deficiency,
                n_eq.max(n_var) - n_matched,
                "{label}: deficiency must follow from the counts beside it",
            );
            assert_eq!(deficiency, 1, "{label}: CapacitorLoop is one short, before and after");
        }
    }

    /// A **singular** structural report still produces a matching animation, and
    /// that animation ends on the failure (ideas #44).
    ///
    /// This is the claim the #44 fix rests on. Until 2026-07-29 the `Matching ▶`
    /// sub-tab was hidden whenever the Structural stage was singular, so the one
    /// view that shows *why* a rank deficiency exists was unreachable exactly when
    /// it mattered. Nothing had to be built to fix it — the trace already emits
    /// `MatchingStep::EquationFailed` and the view already paints the failed row —
    /// but nothing tested it either, which is how it stayed hidden.
    ///
    /// Guards two regressions: `from_report` learning to bail on a report that
    /// carries an `error`, and the trace stopping short instead of recording the
    /// give-up.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_singular_report_still_animates_and_ends_on_the_failure() {
        use rumoca_phase_structural::matching::MatchingStep;

        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("MotorWithBrake")
        else {
            panic!("expected Compiled");
        };
        let report = stages.structural.value.as_ref().expect("a structural report");
        assert!(
            report.get("error").is_some(),
            "MotorWithBrake's raw structural stage is expected to be singular",
        );

        let mat = crate::incidence_view::IncidenceMatrix::from_report(report)
            .expect("a singular report still carries an incidence matrix");
        let anim = crate::matching_anim::MatchingAnimation::from_incidence(&mat);

        let failures = anim.failed_equations();
        assert_eq!(
            failures.len(),
            1,
            "a deficiency of 1 means exactly one equation gives up: {failures:?}",
        );

        let (matched, total) = anim.match_progress();
        assert_eq!((matched, total), (47, 48), "47 of 48 matched");

        // The give-up must be *recorded*, not merely implied by the count.
        assert!(
            anim.steps().iter().any(|s| matches!(s, MatchingStep::EquationFailed(_))),
            "the trace must record the equation it gave up on",
        );
    }

    /// The connection-expansion replay reaches HRW with real frames (MLS §9).
    ///
    /// End to end through the worker for the same reason the `pre()` test is:
    /// the interesting part is *where the frames come from*. The session's own
    /// compile flattens without an observer, so `record_connection_frames` has
    /// to re-run instantiate + typecheck + flatten to see anything. Get that
    /// sequence wrong — skip the typecheck, use different `FlattenOptions` —
    /// and the result is silently zero frames, or frames describing a flatten
    /// that never happened. A unit test on the animation type cannot catch it.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn the_connection_replay_reaches_hrw_with_real_frames() {
        use rumoca_phase_flatten::connections::trace::ConnectionStep;

        let FromWorker::Compiled { connection_frames, .. } =
            compile_specimen_shared("RcCircuit")
        else {
            panic!("expected Compiled");
        };
        assert!(
            !connection_frames.is_empty(),
            "RcCircuit wires four components together with connect()",
        );

        // Bookends, so a truncated trace is not mistaken for a short model.
        assert!(
            matches!(
                connection_frames.first().map(|f| &f.step),
                Some(ConnectionStep::Start { .. }),
            ),
            "{:?}",
            connection_frames.first(),
        );
        let Some(ConnectionStep::Complete { sets, equations_added }) =
            connection_frames.last().map(|f| f.step.clone())
        else {
            panic!("last frame must be Complete: {:?}", connection_frames.last());
        };
        assert!(sets > 0, "an RC circuit has connection sets");
        assert!(equations_added > 0, "and they produce equations");

        // The asymmetry must be present in a real model, not just in the unit
        // test's hand-built frames: some potential set yields more than one
        // equation, and every flow set yields exactly one.
        let generated: Vec<(&str, usize, usize)> = connection_frames
            .iter()
            .filter_map(|f| match &f.step {
                ConnectionStep::EquationsGenerated { kind, set_size, equations_added } => {
                    Some((*kind, *set_size, *equations_added))
                }
                _ => None,
            })
            .collect();
        assert!(
            generated.iter().any(|(k, n, e)| *k == "potential" && *n > 2 && *e == n - 1),
            "a potential set of n must yield n-1 equalities: {generated:?}",
        );
        assert!(
            generated.iter().filter(|(k, ..)| *k == "flow").all(|(_, _, e)| *e == 1),
            "every flow set yields exactly one sum-to-zero equation: {generated:?}",
        );

        // The running total must land on what Complete reported.
        assert_eq!(
            connection_frames.last().unwrap().equations_so_far,
            equations_added,
            "the running count and the Complete frame must agree",
        );
    }

    /// The `pre()` lowering replay reaches HRW with real frames (idea #40).
    ///
    /// End to end through the worker, because the interesting part is *where the
    /// frames come from*: the pass runs inside DAE construction, so the compiled
    /// DAE is already past it and the worker has to re-run construction over
    /// `cr.flat` to see anything. A unit test on the animation type would not
    /// have caught getting that wrong — it would just have shown zero frames.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn the_pre_lowering_replay_reaches_hrw_with_real_frames() {
        use rumoca_phase_dae::PreLoweringStep;

        let FromWorker::Compiled { pre_lowering_frames, flat, .. } =
            compile_specimen_shared("MotorWithBrake")
        else {
            panic!("expected Compiled");
        };
        assert!(flat.is_some(), "the flat model must be carried for live replay");
        assert!(!pre_lowering_frames.is_empty(), "MotorWithBrake uses pre() via its when-equation");

        let named: Vec<(String, String)> = pre_lowering_frames
            .iter()
            .filter_map(|f| match &f.step {
                PreLoweringStep::Named { base, slot } => Some((base.clone(), slot.clone())),
                _ => None,
            })
            .collect();
        assert!(
            named.iter().any(|(b, s)| b == "overSpeed" && s == "__pre__.overSpeed"),
            "the slot the Events IR references must be seen being named: {named:?}",
        );

        // The pass runs twice per compile, and the second run creates nothing.
        // That was *mis*-stated as the opposite until the instrumentation showed
        // otherwise, so it is pinned here rather than left to memory.
        let completions: Vec<usize> = pre_lowering_frames
            .iter()
            .filter_map(|f| match &f.step {
                PreLoweringStep::Complete { slots_created } => Some(*slots_created),
                _ => None,
            })
            .collect();
        assert_eq!(completions.len(), 2, "the pass runs twice per compile: {completions:?}");
        assert!(completions[0] > 0, "the first pass creates the slots");
        assert_eq!(completions[1], 0, "the second finds nothing left to lower");
    }

    /// Following an identifier must survive **real** IR, whatever is in it.
    ///
    /// Regression for a crash on the simplest possible action: open
    /// MotorWithBrake, click `overSpeed` in the source. Following walks every
    /// stage's IR and lexes each code-bearing string, and MotorWithBrake's
    /// structural note contains an em dash. The lexer stepped one *byte* over
    /// non-ASCII, so a token boundary landed inside that character and slicing
    /// it panicked — see `modelica_lex::bare_non_ascii_lexes_on_character_boundaries`.
    ///
    /// The synthetic tests could not have caught it: they lex Modelica, which
    /// is ASCII. Only prose written by the compiler reaches the lexer with an
    /// em dash in it, and only because following searches IR strings.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn following_an_identifier_walks_every_stage_without_panicking() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("MotorWithBrake") else {
            panic!("expected Compiled");
        };
        let pairs = stages.as_stage_pairs();
        for name in ["overSpeed", "__pre__.overSpeed", "emf.phi", "der(emf.phi)"] {
            let t = crate::bridge::Tracking {
                seq: 1,
                name,
                declared_line: None,
                declaring_class: None,
                stage_values: &pairs,
            };
            crate::bridge::summarize_tracking(&t);
        }

        let t = crate::bridge::Tracking {
            seq: 1,
            name: "overSpeed",
            declared_line: None,
            declaring_class: None,
            stage_values: &pairs,
        };
        let (mentions, stage_count) = crate::bridge::summarize_tracking(&t);
        assert!(mentions > 0, "overSpeed is declared in MotorWithBrake");
        assert!(stage_count > 1, "it should survive past a single stage");
    }

    // -----------------------------------------------------------------------
    // Full-pipeline regression guards: every specimen compiles through ALL
    // expected stages. These are the most rebase-sensitive tests — if an
    // upstream Rumoca change breaks a phase or renames an API, at least one
    // of these will catch it.
    // -----------------------------------------------------------------------

    /// Every specimen that should compile through solve lowering does so
    /// (all stages produce IR). The three known exceptions are tested separately:
    /// CapacitorLoop (structurally singular, irreducible) and OverInitRc
    /// (over-determined init) still produce partial pipelines.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn all_healthy_specimens_compile_through_solve_lowering() {
        let healthy = [
            "SingleInertia", "RotationalInertia", "ProportionalLoop", "NonlinearLoop",
            "MixedLoop", "TwoLoops", "Drivetrain", "RcCircuit", "BouncingBall", "BenchActuator",
        ];
        for name in healthy {
            let FromWorker::Compiled {
                model, stages, ..
            } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            assert!(model.is_some(), "{name}: model name not extracted");
            assert!(stages.parse.value.is_some(), "{name}: parse failed: {:?}", stages.parse.note);
            assert!(stages.resolve.value.is_some(), "{name}: resolve failed: {:?}", stages.resolve.note);
            assert!(stages.instantiate.value.is_some(), "{name}: instantiate failed: {:?}", stages.instantiate.note);
            assert!(stages.typecheck.value.is_some(), "{name}: typecheck failed: {:?}", stages.typecheck.note);
            assert!(stages.flatten.value.is_some(), "{name}: flatten failed: {:?}", stages.flatten.note);
            assert!(stages.index_reduction.value.is_some(), "{name}: index reduction failed: {:?}", stages.index_reduction.note);
            assert!(stages.events.value.is_some(), "{name}: events failed: {:?}", stages.events.note);
            assert!(stages.solve_lowering.value.is_some(), "{name}: solve lowering failed: {:?}", stages.solve_lowering.note);
        }
    }

    /// Every specimen that compiles through solve lowering also simulates
    /// successfully — the end-to-end path from source to trajectories.
    /// RcCircuit is excluded: it compiles but the BDF solver hits a step-size
    /// floor (stiff RC with the default tolerances).
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn all_healthy_specimens_simulate() {
        let healthy = [
            "SingleInertia", "RotationalInertia", "ProportionalLoop", "NonlinearLoop",
            "MixedLoop", "TwoLoops", "Drivetrain", "BouncingBall", "BenchActuator",
        ];
        for name in healthy {
            let data = {
                let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
                let path = PathBuf::from(format!("{}/specimens/{name}.mo", env!("CARGO_MANIFEST_DIR")));
                w.simulate(&path, name, 1.0, &|_: FromWorker| {})
            };
            let data = data.unwrap_or_else(|e| panic!("{name}: simulate failed: {e}"));
            assert!(!data.times.is_empty(), "{name}: no time points");
            assert!(!data.names.is_empty(), "{name}: no output variables");
            assert_eq!(data.data.len(), data.names.len(), "{name}: data/names length mismatch");
        }
    }

    /// The headless `compile_specimen` path (used by gen_trace) produces the
    /// same result as compiling through the shared worker.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn compile_specimen_headless_matches_worker() {
        let result = compile_specimen(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo")),
            msl_roots(),
        )
        .expect("compile_specimen");
        let FromWorker::Compiled { model, stages, .. } = result else {
            panic!("expected Compiled");
        };
        assert_eq!(model.as_deref(), Some("SingleInertia"));
        assert!(stages.parse.value.is_some());
        assert!(stages.resolve.value.is_some());
        assert!(stages.flatten.value.is_some());
        assert!(stages.solve_lowering.value.is_some());
    }

    /// The headless `simulate_specimen` path (used by gen_trace) runs and
    /// produces trajectories.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn simulate_specimen_headless_produces_trajectories() {
        let data = simulate_specimen(
            Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/SingleInertia.mo")),
            "SingleInertia",
            2.0,
            msl_roots(),
        )
        .expect("simulate_specimen");
        assert!(!data.times.is_empty());
        assert!(data.names.iter().any(|n| n == "w"), "expected 'w' in output names");
    }

    // -----------------------------------------------------------------------
    // Stage-specific content guards: verify that key JSON fields are present
    // in each stage's IR. These catch Rumoca IR renames or restructurings.
    // -----------------------------------------------------------------------

    /// The Flatten stage IR for a simple model has the expected top-level
    /// structure: variables, equations, and the flat model fields.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn flatten_ir_has_expected_structure() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let v = stages.flatten.value.expect("flatten IR");
        assert!(v.get("variables").is_some(), "flat IR should have 'variables'");
        assert!(v.get("equations").is_some(), "flat IR should have 'equations'");
    }

    /// The Events stage IR has the expected summary structure.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn events_ir_has_expected_summary_keys() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("BouncingBall") else {
            panic!("expected Compiled");
        };
        let v = stages.events.value.expect("events IR");
        let summary = v["summary"].as_object().expect("summary object");
        for key in ["condition_equations", "relations", "discrete_real_updates",
                     "discrete_valued_updates", "zero_crossing_conditions", "scheduled_time_events"] {
            assert!(summary.contains_key(key), "events summary missing key: {key}");
        }
    }

    /// The Solve-lowering IR has the expected top-level fields.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn solve_lowering_ir_has_expected_fields() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let v = stages.solve_lowering.value.expect("solve lowering IR");
        assert!(v.get("problem").is_some(), "SolveModel should have 'problem'");
        assert!(v.get("variable_meta").is_some(), "SolveModel should have 'variable_meta'");
    }

    /// The Structural stage IR has matching, blocks, and incidence matrix.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn structural_ir_has_incidence_matrix() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("ProportionalLoop") else {
            panic!("expected Compiled");
        };
        let v = stages.structural.value.expect("structural IR");
        assert!(v["matching"].as_array().is_some_and(|a| !a.is_empty()), "missing matching");
        assert!(v["blocks"].as_array().is_some_and(|a| !a.is_empty()), "missing blocks");
        let inc = &v["incidence"];
        assert!(inc["unknown_names"].as_array().is_some(), "incidence missing unknown_names");
        assert!(inc["rows"].as_array().is_some(), "incidence missing rows");
        assert!(inc["n_eq"].as_u64().is_some(), "incidence missing n_eq");
    }

    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn structural_incidence_has_equation_text_labels() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("SingleInertia") else {
            panic!("expected Compiled");
        };
        let v = stages.structural.value.expect("structural IR");
        let rows = v["incidence"]["rows"].as_array().expect("incidence rows");
        for row in rows {
            let text = row.get("equation_text").and_then(|v| v.as_str());
            assert!(text.is_some(), "row missing equation_text: {row}");
            let text = text.unwrap();
            assert!(!text.is_empty(), "equation_text should not be empty");
            assert!(!text.starts_with("f_x["), "equation_text should be readable, not an index label: {text}");
        }
    }

    /// The Index-reduction stage IR includes the reduction report.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn index_reduction_ir_has_reduction_report() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        let v = stages.index_reduction.value.expect("index reduction IR");
        let red = &v["reduction"];
        assert!(red.is_object(), "should have a reduction report");
        assert!(red.get("steps").is_some(), "reduction should have steps");
        assert!(red.get("n_states_before").is_some(), "reduction should have n_states_before");
        assert!(red.get("n_states_after").is_some(), "reduction should have n_states_after");
        assert!(red.get("funnel_completed").is_some(), "reduction should have funnel_completed");
    }

    /// The Initialization stage IR includes the determinacy check.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn initialization_ir_has_determinacy() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("RcCircuit") else {
            panic!("expected Compiled");
        };
        let v = stages.initialization.value.expect("initialization IR");
        let det = &v["determinacy"];
        assert!(det.is_object(), "should have a determinacy section");
        for key in ["states", "initial_equations", "fixed_start_states",
                     "explicit_initial_conditions", "surplus_over_states", "verdict"] {
            assert!(det.get(key).is_some(), "determinacy missing key: {key}");
        }
    }

    // -----------------------------------------------------------------------
    // Utility function guards
    // -----------------------------------------------------------------------

    /// `is_def_id_key` recognizes the three DefId field names.
    #[test]
    fn is_def_id_key_recognizes_all_three() {
        assert!(is_def_id_key("def_id"));
        assert!(is_def_id_key("type_def_id"));
        assert!(is_def_id_key("base_def_id"));
        assert!(!is_def_id_key("id"));
        assert!(!is_def_id_key("def_id_extra"));
        assert!(!is_def_id_key(""));
    }

    /// `discontinuity_segments` handles edge cases.
    #[test]
    fn discontinuity_segments_edge_cases() {
        assert_eq!(discontinuity_segments(&[]).len(), 1); // degenerate: one empty segment
        assert_eq!(discontinuity_segments(&[1.0]), vec![0..1]);
        assert_eq!(discontinuity_segments(&[1.0, 1.0, 1.0]), vec![0..3]);
    }

    /// Compilation produces log entries with the expected stage structure.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn compilation_emits_log_entries() {
        let logs = std::sync::Mutex::new(Vec::new());
        {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = PathBuf::from(format!("{}/specimens/SingleInertia.mo", env!("CARGO_MANIFEST_DIR")));
            w.compile(&path, &|msg: FromWorker| {
                if let FromWorker::Log(entry) = msg {
                    logs.lock().unwrap().push(entry);
                }
            });
        }
        let logs = logs.into_inner().unwrap();
        let stage_starts: Vec<&str> = logs.iter()
            .filter(|e| matches!(e.level, LogLevel::StageStart))
            .map(|e| e.message.as_str())
            .collect();
        let stage_ends: Vec<&str> = logs.iter()
            .filter(|e| matches!(e.level, LogLevel::StageEnd))
            .map(|e| e.message.split(" (").next().unwrap_or(""))
            .collect();
        assert!(stage_starts.contains(&"Parse"), "missing Parse stage start");
        assert!(stage_starts.contains(&"Resolve"), "missing Resolve stage start");
        assert!(stage_starts.contains(&"Flatten"), "missing Flatten stage start");
        assert!(stage_starts.contains(&"Solve lowering"), "missing Solve lowering stage start");
        assert_eq!(stage_starts.len(), stage_ends.len(), "every stage start should have a matching end");
        assert!(logs.iter().any(|e| matches!(e.level, LogLevel::Info)), "should have at least one info entry");

        // **DAE construction is logged, and in its true position.**
        //
        // Doug, 2026-08-04: *"our logs do not report the begin or end of that DAE
        // phase. Worse, our logs contain a fiction about a DAE pipeline which
        // includes the phases which follow the DAE phase."* The stage had a tab, a
        // trace file and a tour, and the log skipped straight from Flatten to
        // Structural — while a bracket labelled "DAE pipeline" claimed to span five
        // phases that come *after* DAE construction.
        //
        // **Order is asserted, not just presence.** Logging it where it used to be
        // built would have reported DAE construction finishing after the phases that
        // consume its output — a second fiction in place of the first.
        let pos = |name: &str| {
            stage_starts.iter().position(|s| *s == name).unwrap_or_else(|| {
                panic!("no `{name}` stage start in {stage_starts:?}")
            })
        };
        assert!(
            pos("Flatten") < pos("DAE construction"),
            "DAE construction must be logged after Flatten: {stage_starts:?}",
        );
        assert!(
            pos("DAE construction") < pos("Structural analysis"),
            "and before the phases that consume the DAE: {stage_starts:?}",
        );
        assert!(
            stage_ends.contains(&"DAE construction"),
            "a phase that starts must also be reported as ending: {stage_ends:?}",
        );

        // **The fiction stays gone.** Named as a substring so a revival under any
        // wording ("DAE pipeline (flatten -> ...)") is caught.
        for e in &logs {
            assert!(
                !e.message.contains("DAE pipeline"),
                "the DAE is a phase, not a pipeline, and the old bracket claimed a \
                 span reaching five phases past it: {:?}",
                e.message,
            );
        }
    }

    /// **A compile never reports another run's traces.**
    ///
    /// Doug, 2026-08-04: with the tracing checkbox *off*, *"detailed rumoca logs are
    /// still included in the log view, but for a smaller subset of compiler phases"* —
    /// and with it on, logs appeared *"for only a subset of compiler phases."* One
    /// cause: `TRACE_BUFFER` is drained after each Rumoca call, but
    /// `to_dae_with_options_traced` ran last with **no drain after it**, so every
    /// event it emitted was stranded and reported against the *following* compile.
    /// The run that produced them was missing them; the run that showed them had not
    /// asked for them.
    ///
    /// **Checks the property, not the one call.** A stranded event is planted
    /// directly, so any future undrained Rumoca call is caught by the same test
    /// rather than needing its own.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_compile_never_reports_another_runs_traces() {
        const STALE: &str = "STRANDED BY A PREVIOUS RUN";

        // Exactly what an undrained Rumoca call leaves behind. Same thread as the
        // compile below, so this is the buffer that compile will see.
        TRACE_BUFFER.with(|b| {
            b.borrow_mut().push((tracing::Level::DEBUG, STALE.to_owned()))
        });

        let logs = std::sync::Mutex::new(Vec::new());
        {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = PathBuf::from(format!(
                "{}/specimens/SingleInertia.mo",
                env!("CARGO_MANIFEST_DIR")
            ));
            w.compile(&path, &|msg: FromWorker| {
                if let FromWorker::Log(entry) = msg {
                    logs.lock().unwrap().push(entry);
                }
            });
        }
        let logs = logs.into_inner().unwrap();

        // Non-vacuity: the compile must actually have logged, or "no stale entry"
        // is true for the uninteresting reason.
        assert!(
            logs.iter().any(|e| matches!(e.level, LogLevel::StageEnd)),
            "the compile produced no stage entries, so this proves nothing",
        );
        assert!(
            !logs.iter().any(|e| e.message.contains(STALE)),
            "a compile reported a trace event stranded before it began \u{2014} which is \
             how tracing appeared to stay on after being switched off",
        );
        // And it must not still be waiting to ambush the next one.
        assert!(
            TRACE_BUFFER.with(|b| b.borrow().is_empty()),
            "the buffer must be empty when a compile ends, or the next run inherits it",
        );
    }

    /// **A compile leaves nothing in the buffer — with tracing actually on.**
    ///
    /// The companion to `a_compile_never_reports_another_runs_traces`, and the half
    /// that needs tracing *enabled* to mean anything: with it off, Rumoca emits no
    /// events and an empty buffer proves nothing.
    ///
    /// This is what catches a Rumoca call added without a `drain_traces` after it.
    /// `to_dae_with_options_traced` was exactly that — a full re-run of DAE
    /// construction, the last call in the compile, with nothing to drain it — so its
    /// output arrived one compile late for as long as the feature existed.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_compile_with_tracing_on_leaves_nothing_behind() {
        let logs = std::sync::Mutex::new(Vec::new());
        let left_behind;
        let sink = |msg: FromWorker| {
            if let FromWorker::Log(entry) = msg {
                logs.lock().unwrap().push(entry);
            }
        };
        {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            w.handle(ToWorker::SetTracing(true), &sink);
            let path = PathBuf::from(format!(
                "{}/specimens/SingleInertia.mo",
                env!("CARGO_MANIFEST_DIR")
            ));
            w.compile(&path, &sink);
            // **Sampled here, before the toggle.** `SetTracing(false)` clears the
            // buffer, so asserting after it would assert the cleanup rather than the
            // compile — the first version of this test did exactly that and passed
            // with the drain removed.
            left_behind = TRACE_BUFFER.with(|b| b.borrow().len());
            // Restore, or every later test on this shared worker runs traced.
            w.handle(ToWorker::SetTracing(false), &sink);
        }
        let logs = logs.into_inner().unwrap();

        // **Non-vacuity, and it is the whole point here**: tracing must have
        // produced something, or "nothing left behind" is trivially true.
        assert!(
            logs.iter().any(|e| matches!(e.level, LogLevel::Trace)),
            "tracing was on and no trace entries were reported \u{2014} this test cannot \
             detect a missing drain unless Rumoca is actually emitting",
        );
        assert_eq!(
            left_behind, 0,
            "a Rumoca call emitted {left_behind} trace event(s) that no drain \
             collected; they would surface under the NEXT compile instead of this one",
        );
    }

    /// **The "no instrumentation" claim is still true.**
    ///
    /// `UNINSTRUMENTED_PHASES` tells the reader that silence from these phases means
    /// *unwired*, not *quiet*. That is a claim of **absence**, and this project's
    /// standing rule is that a claim of absence rots unnoticed unless something
    /// fails when it stops being true — acting on a wrong positive means going to
    /// look and finding nothing, while acting on a wrong negative means **not
    /// looking**.
    ///
    /// So this counts tracing call sites in each named crate. Instrument one of them
    /// upstream and this fails until the entry is removed, which is the only way the
    /// notice can stay honest across a rebase.
    ///
    /// **Both directions.** A listed crate must have none, *and* a crate known to be
    /// instrumented must not be listed — otherwise an over-broad list would silence
    /// real output in the reader's mind, which is the same defect pointed the other
    /// way.
    #[test]
    fn the_uninstrumented_phase_list_matches_the_crates() {
        // `tracing::debug!` and friends, plus the bare form after a `use tracing::…`.
        fn tracing_sites(crate_dir: &Path) -> usize {
            fn walk(dir: &Path, out: &mut usize) {
                let Ok(entries) = std::fs::read_dir(dir) else { return };
                for e in entries.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, out);
                    } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                        let Ok(src) = std::fs::read_to_string(&p) else { continue };
                        // Only `tracing`'s macros count: the parser's generated code
                        // uses `parol_runtime::log::trace`, which HRW never sees, and
                        // treating that as instrumentation would make the notice lie
                        // in the more damaging direction.
                        let uses_tracing = src.contains("use tracing")
                            || src.contains("tracing::");
                        for m in ["tracing::debug!", "tracing::info!", "tracing::warn!",
                                  "tracing::trace!", "tracing::error!"] {
                            *out += src.matches(m).count();
                        }
                        // **Crate-local trace macros count too.** Counting only
                        // `tracing::` undercounted `rumoca-phase-structural` by
                        // 27 — it wraps every call in `structural_trace!`, so the
                        // naive count read the *best* instrumented phase as the
                        // worst. A test that undercounts here would let a genuinely
                        // instrumented crate stay on the silent list.
                        *out += src.matches("_trace!(").count();
                        if uses_tracing && !src.contains("parol_runtime::log") {
                            for m in ["\n    debug!(", "\n    info!(", "\n    warn!(",
                                      "\n    trace!("] {
                                *out += src.matches(m).count();
                            }
                        }
                    }
                }
            }
            let mut n = 0;
            walk(&crate_dir.join("src"), &mut n);
            n
        }

        let crates = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("hrw/ has a parent")
            .join("crates");
        assert!(crates.is_dir(), "the Rumoca crates must be beside hrw/: {crates:?}");

        for (phase, krate, why) in UNINSTRUMENTED_PHASES {
            match krate {
                // Backed by a real crate: it must genuinely emit nothing.
                Some(krate) => {
                    let dir = crates.join(krate);
                    assert!(
                        dir.is_dir(),
                        "{phase} names {krate}, which is not a crate. An absent \
                         directory greps as zero call sites, so a wrong crate name \
                         makes this whole notice a self-confirming fiction \u{2014} \
                         which is exactly how the first draft claimed two phases were \
                         uninstrumented when they were not phases at all.",
                    );
                    let n = tracing_sites(&dir);
                    assert_eq!(
                        n, 0,
                        "{phase} is listed as silent ({why}), but {krate} now has {n} \
                         tracing call site(s). The log is telling readers to expect \
                         silence from a phase that speaks \u{2014} remove the entry.",
                    );
                }
                // **Claimed to render DAE data without calling Rumoca.** Checked by
                // reading the stage function: if it names any `rumoca_phase_*`, a
                // real algorithm runs and the tab can emit traces after all.
                //
                // This check exists because the first version only asked whether a
                // crate named `rumoca-phase-<tab>` existed — which is *no evidence at
                // all*, and duly passed for Initialization, whose
                // `initialization_stage` calls
                // `rumoca_phase_structural::build_ic_plan` on its eleventh line.
                //
                // **Bounded, and honestly so**: it reads the stage function's own
                // body, so a Rumoca call hidden inside a helper would escape it. That
                // is a weaker guarantee than the crate-count branch above and is
                // stated rather than implied.
                None => {
                    let worker = std::fs::read_to_string(
                        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"),
                    )
                    .expect("worker.rs must be readable");
                    let needle = format!("fn {}_stage(", phase.to_lowercase());
                    let start = worker.find(&needle).unwrap_or_else(|| {
                        panic!("{phase} claims to be HRW-derived but has no {needle}")
                    });
                    let body_end = worker[start..]
                        .find("\nfn ")
                        .map(|e| start + e)
                        .unwrap_or(worker.len());
                    let body = &worker[start..body_end];
                    assert!(
                        !body.contains("rumoca_phase_"),
                        "{phase} is described as rendering DAE data ({why}), but \
                         {needle} calls into a rumoca_phase_* crate \u{2014} a real \
                         algorithm runs, so the tab can emit tracing and must not be \
                         listed as permanently silent",
                    );
                }
            }
        }

        // The other direction: a crate that *is* instrumented must not be listed.
        for krate in ["rumoca-phase-flatten", "rumoca-phase-dae"] {
            assert!(
                tracing_sites(&crates.join(krate)) > 0,
                "{krate} was expected to be instrumented; if that changed, this test's \
                 own premise is stale and the notice needs rechecking",
            );
            assert!(
                !UNINSTRUMENTED_PHASES.iter().any(|(_, k, _)| *k == Some(krate)),
                "{krate} emits tracing but is listed as silent",
            );
        }

        // And the notice actually names them, rather than being an empty sentence.
        let notice = uninstrumented_notice();
        for (phase, _, _) in UNINSTRUMENTED_PHASES {
            assert!(notice.contains(phase), "the notice must name {phase}: {notice}");
        }
    }

    /// Simulation also emits log entries with timing.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn simulation_emits_log_entries() {
        let logs = std::sync::Mutex::new(Vec::new());
        {
            let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
            let path = PathBuf::from(format!("{}/specimens/SingleInertia.mo", env!("CARGO_MANIFEST_DIR")));
            w.simulate(&path, "SingleInertia", 1.0, &|msg: FromWorker| {
                if let FromWorker::Log(entry) = msg {
                    logs.lock().unwrap().push(entry);
                }
            }).expect("simulate");
        }
        let logs = logs.into_inner().unwrap();
        let stage_starts: Vec<&str> = logs.iter()
            .filter(|e| matches!(e.level, LogLevel::StageStart))
            .map(|e| e.message.as_str())
            .collect();
        assert!(stage_starts.contains(&"Compile (for simulation)"), "missing compile stage");
        assert!(stage_starts.contains(&"Solve lowering"), "missing solve lowering stage");
        assert!(stage_starts.contains(&"Integration"), "missing integration stage");
    }

    // -----------------------------------------------------------------------
    // Error-path tests (TD-14): verify that the worker reports errors
    // correctly when given bad inputs, rather than panicking.
    // -----------------------------------------------------------------------

    /// Compiling a nonexistent file reports a parse-stage error (file read
    /// failure) instead of panicking.
    #[test]
    fn compile_nonexistent_file_reports_error() {
        let mut w = WorkerState::new();
        let path = PathBuf::from("/tmp/hrw_test_nonexistent_file_that_does_not_exist.mo");
        let result = w.compile(&path, &|_: FromWorker| {});
        let FromWorker::Compiled { stages, .. } = result else {
            panic!("expected Compiled");
        };
        assert!(stages.parse.note_is_error(), "parse stage should flag an error for a missing file");
        assert!(
            stages.parse.note.as_deref().unwrap_or("").contains("read error"),
            "parse note should mention a read error, got: {:?}",
            stages.parse.note
        );
    }

    /// Compiling a file with invalid Modelica syntax reports a parse-stage
    /// error (the parser rejects the input).
    #[test]
    fn compile_invalid_syntax_reports_parse_error() {
        let tmp_dir = PathBuf::from(concat!(
            "/tmp/claude-1000/-home-dougdew-dev-rumoca/",
            "0033dab5-98a0-4f7a-8241-a545c97992aa/scratchpad"
        ));
        std::fs::create_dir_all(&tmp_dir).expect("create scratchpad dir");
        let bad_file = tmp_dir.join("bad_syntax.mo");
        std::fs::write(&bad_file, "not valid modelica {").expect("write temp file");

        let mut w = WorkerState::new();
        let result = w.compile(&bad_file, &|_: FromWorker| {});
        let FromWorker::Compiled { stages, .. } = result else {
            panic!("expected Compiled");
        };
        assert!(
            stages.parse.note_is_error(),
            "parse stage should flag an error for invalid syntax"
        );
        assert!(
            stages.parse.note.is_some(),
            "parse stage should carry an error message"
        );
    }

    /// Calling `open_def` on a fresh worker (no compilation, no resolved tree)
    /// returns a `DefTree` with `result: Err(...)` instead of panicking.
    #[test]
    fn open_def_without_resolved_tree_reports_error() {
        let mut w = WorkerState::new();
        let result = w.open_def("SomeName");
        let FromWorker::DefTree { result, .. } = result else {
            panic!("expected DefTree");
        };
        assert!(result.is_err(), "open_def on a fresh worker should return Err");
    }

    /// `extract_class` with a name that doesn't exist in the tree returns a
    /// `Stage` with `note_is_error == true`.
    #[test]
    fn extract_class_missing_name_reports_error() {
        let empty_tree = rumoca_ir_ast::ClassTree::default();
        let stage = extract_class(&empty_tree, "NonExistent.Model.Name");
        assert!(stage.note_is_error(), "extract_class should flag an error for a missing name");
        assert!(stage.value.is_none(), "extract_class should produce no value for a missing name");
        assert!(
            stage.note.as_deref().unwrap_or("").contains("not found"),
            "error note should mention 'not found', got: {:?}",
            stage.note
        );
    }

    #[test]
    fn stage_kind_all_is_exhaustive() {
        assert_eq!(
            StageKind::ALL.len(),
            12,
            "StageKind::ALL should list every variant (currently 12: 11 pipeline stages \
             plus Simulation). Adding one means wiring it into every per-stage system — \
             stage-diff highlight, stage-file publishing and the notebook trace — which is \
             why this count is asserted rather than derived from the enum."
        );
        // Every name is non-empty and unique.
        let names: Vec<&str> = StageKind::ALL.iter().map(|s| s.name()).collect();
        for name in &names {
            assert!(!name.is_empty());
        }
        let unique: std::collections::HashSet<&&str> = names.iter().collect();
        assert_eq!(unique.len(), names.len(), "duplicate stage names in ALL");
    }

    #[test]
    fn simulate_nonexistent_file_reports_error() {
        let mut w = WorkerState::new();
        let path = PathBuf::from("/tmp/hrw_test_sim_nonexistent.mo");
        let result = w.simulate(&path, "Model", 1.0, &|_: FromWorker| {});
        assert!(result.is_err(), "simulate of a missing file should return Err");
    }

    #[test]
    fn simulate_invalid_syntax_reports_error() {
        let tmp_dir = PathBuf::from(concat!(
            "/tmp/claude-1000/-home-dougdew-dev-rumoca/",
            "0033dab5-98a0-4f7a-8241-a545c97992aa/scratchpad"
        ));
        std::fs::create_dir_all(&tmp_dir).expect("create scratchpad dir");
        let bad_file = tmp_dir.join("sim_bad_syntax.mo");
        std::fs::write(&bad_file, "not valid modelica {").expect("write temp file");
        let mut w = WorkerState::new();
        let result = w.simulate(&bad_file, "Model", 1.0, &|_: FromWorker| {});
        assert!(result.is_err(), "simulate of invalid syntax should return Err");
    }

    #[test]
    fn compile_emits_progress_messages() {
        let specimen = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        ));
        let mut w = WorkerState::new();
        let progress = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let p = std::sync::Arc::clone(&progress);
        let _final = w.compile(&specimen, &move |msg: FromWorker| {
            if let FromWorker::CompileProgress { .. } = &msg {
                p.lock().unwrap().push(msg);
            }
        });
        let msgs = progress.lock().unwrap();
        assert!(!msgs.is_empty(), "compile should emit at least one CompileProgress");
    }

    #[test]
    fn compile_produces_equation_sheet_for_healthy_specimen() {
        let specimen = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        ));
        let mut w = WorkerState::new();
        let result = w.compile(&specimen, &|_: FromWorker| {});
        let FromWorker::Compiled { equation_sheet, .. } = result else {
            panic!("expected Compiled");
        };
        let sheet = equation_sheet.expect("equation_sheet should be Some");
        assert!(sheet.n_equations > 0, "should have at least one equation");
    }

    #[test]
    fn compile_produces_identifier_index_for_healthy_specimen() {
        let specimen = PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/specimens/SingleInertia.mo"
        ));
        let mut w = WorkerState::new();
        let result = w.compile(&specimen, &|_: FromWorker| {});
        let FromWorker::Compiled { identifier_index, .. } = result else {
            panic!("expected Compiled");
        };
        let idx = identifier_index.expect("identifier_index should be Some");
        assert!(!idx.variables.is_empty(), "should have indexed at least one variable");
        let has_state = idx.variables.values().any(|v| v.kind == "state");
        assert!(has_state, "SingleInertia should have at least one state variable");
    }

    // -- OutputCapture tests --------------------------------------------------
    //
    // These tests use raw `libc::write` instead of `print!`/`eprint!` because
    // cargo test intercepts Rust's print macros at the stdlib level — above
    // the fd layer — via an internal `set_output_capture` mechanism. Data
    // written through `print!` goes into cargo's per-test capture buffer and
    // never reaches fd 1 (the pipe). Since `OutputCapture` operates at the fd
    // level (`dup2`), its tests must also write at the fd level to exercise
    // the actual capture path.
    //
    // In production this isn't an issue: Rumoca's C-level `printf` and Rust
    // `tracing` output write directly to fd 1/2, bypassing Rust's BufWriter.

    unsafe fn write_to_fd(fd: i32, data: &[u8]) {
        let mut offset = 0;
        while offset < data.len() {
            let remaining = data.len() - offset;
            // libc::write count is size_t on unix, c_uint on Windows.
            #[cfg(unix)]
            let count = remaining;
            #[cfg(windows)]
            let count = remaining as libc::c_uint;
            let n = unsafe {
                libc::write(fd, data[offset..].as_ptr().cast(), count)
            };
            if n <= 0 { break; }
            offset += n as usize;
        }
    }

    #[test]
    fn output_capture_round_trip() {
        let mut cap = OutputCapture::start().expect("start capture");
        unsafe {
            write_to_fd(1, b"hello stdout");
            write_to_fd(2, b"hello stderr");
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (out, err) = cap.drain();
        drop(cap);
        assert!(out.contains("hello stdout"), "stdout missing: {out:?}");
        assert!(err.contains("hello stderr"), "stderr missing: {err:?}");
    }

    // Regression test for the pipe-buffer deadlock. Three implementations
    // existed; this test distinguishes all three:
    //
    // 1. Post-hoc drain (original): drain() runs after the API call returns.
    //    A 128 KB write exceeds the 64 KB pipe buffer, write() blocks waiting
    //    for a reader, but drain() can't run until write() returns — deadlock.
    //    This test would hang forever.
    //
    // 2. O_NONBLOCK on the write side (partial fix): write() returns EAGAIN
    //    instead of blocking, preventing the deadlock — but the excess bytes
    //    are silently dropped, and Rust's println! panics on EAGAIN. This test
    //    would pass but assert_eq would fail (out.len() < 128 KB).
    //
    // 3. Concurrent reader threads (current fix): reader threads continuously
    //    drain the pipe into a mutex buffer, so the pipe never fills. write()
    //    stays blocking, all bytes are captured, no data loss.
    //    This test passes with all 128 KB captured.
    #[test]
    fn output_capture_handles_large_write_without_deadlock() {
        let mut cap = OutputCapture::start().expect("start capture");
        let big = vec![b'x'; 128 * 1024];
        unsafe { write_to_fd(1, &big); }
        std::thread::sleep(std::time::Duration::from_millis(100));
        let (out, _) = cap.drain();
        drop(cap);
        assert_eq!(out.len(), 128 * 1024, "should capture all 128 KB");
    }

    /// `StageBundle::as_stage_pairs` names must stay in sync with
    /// `bridge::STAGE_FILE_NAMES` — a rename or reorder in one but not the
    /// other silently breaks stage-file publishing.
    #[test]
    fn stage_pairs_names_match_stage_file_names() {
        use crate::bridge::STAGE_FILE_NAMES;

        let bundle = StageBundle::default();
        let pair_names: Vec<String> = bundle
            .as_stage_pairs()
            .iter()
            .map(|(name, _)| format!("{name}.json"))
            .collect();
        let file_names: Vec<&str> = STAGE_FILE_NAMES.to_vec();

        assert_eq!(
            pair_names, file_names,
            "StageBundle::as_stage_pairs() names diverged from STAGE_FILE_NAMES"
        );
    }
}

/// Record of what the index-reduction funnel did, step by step.
///
/// This is HRW's observability layer over the index-reduction process: it
/// records which states were demoted, which equations were manufactured by
/// differentiation, and which variables were eliminated — the narrative the
/// Index Reduction tab tells.
struct ReductionReport {
    /// State variable names present in the raw DAE before reduction.
    states_before: Vec<String>,
    /// State variable names remaining after reduction.
    states_after: Vec<String>,
    /// Per-step log: (step name, outcome description).
    steps: Vec<(&'static str, String)>,
    /// Equations manufactured by differentiation (origin contains
    /// `"index_reduction:d_dt_for_"`), with the state they were created for.
    differentiated_rows: Vec<(String, String)>,
    /// Trivial-elimination substitutions (variable → replacement expression).
    eliminations: Vec<(String, String)>,
    /// The step at which the funnel stopped (if it bailed early on error).
    stopped_at: Option<&'static str>,
}

/// Apply Rumoca's index-reduction / dummy-derivative funnel to a DAE in place,
/// returning a structured report of what each step did.
///
/// # What is index reduction?
///
/// A DAE (Differential-Algebraic Equation system) has an "index" — roughly,
/// the number of times you need to differentiate constraint equations to
/// reduce the system to an ODE. High-index systems (index > 1) are
/// structurally singular: the maximum matching can't pair every equation
/// with a unique unknown. The "dummy derivative" method repeatedly
/// differentiates constraints and demotes states to algebraic variables
/// until the system is index-1 (solvable).
///
/// # The funnel steps (in order)
///
/// Each step targets a specific pattern of redundant/constrained states:
/// 1. `demote_exact_alias_component_states` — states that are exact copies
///    of other states (e.g. connector aliasing)
/// 2. `demote_direct_assigned_states` — states directly assigned by equations
/// 3. `reduce_constrained_dummy_derivatives` — dummy derivatives from prior
///    index reduction that are now constrained
/// 4. `index_reduce_missing_state_derivatives` — states whose derivatives
///    don't appear in any equation
/// 5. `demote_states_without_assignable_derivative_rows` — states whose
///    derivative rows can't be assigned
/// 6. `eliminate_derivative_aliases` — simplify derivative alias chains
/// 7. `demote_states_without_retained_derivative_rows` — final demotion pass
/// 8. `expand_compound_derivatives` — expand `der(der(x))` etc.
/// 9. `substitute_standalone_state_derivatives_in_non_ode_rows` — substitute
///    standalone `der(x)` in algebraic equations
/// 10. `eliminate_trivial` — trivial substitutions (x = expr)
///
/// HRW mirrors Rumoca's funnel *order*; re-verify it against
/// `rumoca-sim/src/solve_lowering/structural_lowering.rs` on a Rumoca pin bump
/// (see `docs/updating-rumoca.md`).
///
/// # Rumoca API surface
///
/// All funnel steps come from `rumoca_phase_structural::dae_prepare` (aliased
/// as `dp`). They mutate the DAE in place (`&mut Dae`) and return either
/// `Result<usize, Error>` (count of states demoted) or `Result<(), Error>`.
/// Index-reduce a DAE in place, discarding the report.
///
/// The Structural and Index Reduction tabs describe *different systems*: the
/// raw DAE and the reduced one. A view that reconstructs compiler state from a
/// DAE (the tearing replay) therefore needs the reduced DAE when it is showing
/// the Index Reduction tab. Re-running the funnel is cheap and pure, which
/// beats caching a second DAE that the two tabs would have to keep in sync.
/// `pub` rather than `pub(crate)` so `examples/survey_msl.rs` runs the **same**
/// funnel HRW does. A second copy in the survey would drift from this one, and
/// the funnel's step ORDER mirrors rumoca-sim's internals — `docs/updating-rumoca.md`
/// step 3 exists because a reordering upstream is invisible to the compiler.
pub fn index_reduce_in_place(dae: &mut rumoca_ir_dae::Dae) {
    let _ = index_reduce_for_structural_analysis(dae);
}

fn index_reduce_for_structural_analysis(
    dae: &mut rumoca_ir_dae::Dae,
) -> (ReductionReport, Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>) {
    use rumoca_phase_structural::dae_prepare as dp;
    use rumoca_phase_structural::eliminate;

    let states_before: Vec<String> = dae.variables.states.keys().map(|k| k.to_string()).collect();
    let mut steps: Vec<(&str, String)> = Vec::new();
    let mut stopped_at: Option<&str> = None;
    let mut ir_frames = Vec::new();
    let mut demoted_so_far = Vec::new();

    macro_rules! run_step {
        ($name:expr, $call:expr, $outcome:expr) => {
            match $call {
                Ok(v) => steps.push(($name, $outcome(v))),
                Err(e) => {
                    steps.push(($name, format!("stopped: {e}")));
                    stopped_at = Some($name);
                    return (finish_report(dae, states_before, steps, stopped_at), ir_frames);
                }
            }
        };
    }

    run_step!("demote_exact_alias_component_states",
        dp::demote_exact_alias_component_states(dae), |n| format!("{n} demoted"));

    run_step!("demote_direct_assigned_states",
        dp::demote_direct_assigned_states(dae), |n| format!("{n} demoted"));

    // Opening frame: the system as the traced reduction begins. Note the two
    // demotion steps above are untraced, so this is the animation's baseline
    // rather than the raw DAE — `IndexReductionStep::Start` documents that.
    dp::emit_index_reduction_start(&mut ir_frames, None, dae, &demoted_so_far);

    match dp::reduce_constrained_dummy_derivatives_with_trace(
        dae, None, &mut ir_frames, &mut demoted_so_far,
    ) {
        Ok(n) => steps.push(("reduce_constrained_dummy_derivatives", format!("{n} demoted"))),
        Err(e) => {
            steps.push(("reduce_constrained_dummy_derivatives", format!("stopped: {e}")));
            stopped_at = Some("reduce_constrained_dummy_derivatives");
            return (finish_report(dae, states_before, steps, stopped_at), ir_frames);
        }
    }

    let round_offset = ir_frames.iter()
        .filter_map(|f| match &f.step {
            dp::IndexReductionStep::RoundComplete { round, .. } => Some(*round + 1),
            _ => None,
        })
        .max()
        .unwrap_or(0);
    match dp::index_reduce_missing_state_derivatives_with_trace(
        dae, None, &mut ir_frames, &demoted_so_far, round_offset,
    ) {
        Ok(n) => steps.push(("index_reduce_missing_state_derivatives", format!("{n} demoted"))),
        Err(e) => {
            steps.push(("index_reduce_missing_state_derivatives", format!("stopped: {e}")));
            stopped_at = Some("index_reduce_missing_state_derivatives");
            return (finish_report(dae, states_before, steps, stopped_at), ir_frames);
        }
    }

    let n_unassignable = dp::demote_states_without_assignable_derivative_rows(dae);
    steps.push(("demote_states_without_assignable_derivative_rows", format!("{n_unassignable} demoted")));

    run_step!("eliminate_derivative_aliases",
        dp::eliminate_derivative_aliases(dae), |()| "ok".to_owned());

    run_step!("demote_states_without_retained_derivative_rows",
        dp::demote_states_without_retained_derivative_rows(dae),
        |(no_der_ref, unassignable)| format!("{no_der_ref} no-derivative-ref + {unassignable} unassignable demoted"));

    dp::expand_compound_derivatives(dae);
    steps.push(("expand_compound_derivatives", "ok".to_owned()));

    let n_subst = dp::substitute_standalone_state_derivatives_in_non_ode_rows(dae);
    steps.push(("substitute_standalone_state_derivatives_in_non_ode_rows", format!("{n_subst} substituted")));

    let mut eliminations = Vec::new();
    if let Ok(elim) = eliminate::eliminate_trivial(dae) {
        steps.push(("eliminate_trivial", format!("{} eliminated", elim.n_eliminated)));
        for sub in &elim.substitutions {
            let expr_json = serde_json::to_string(&sub.expr).unwrap_or_default();
            eliminations.push((sub.var_name.to_string(), expr_json));
        }
        let _ = eliminate::apply_elimination_substitutions_to_dae(dae, &elim.substitutions);
    } else {
        steps.push(("eliminate_trivial", "failed (system may still be singular)".to_owned()));
    }

    (finish_report(dae, states_before, steps, stopped_at)
        .with_eliminations(eliminations), ir_frames)
}

/// Build a `ReductionReport` from the post-reduction DAE state. Called at
/// every exit point from `index_reduce_for_structural_analysis` (both
/// early-bail and normal completion). Scans the DAE's equations for
/// differentiated rows (manufactured by the index-reduction process) by
/// looking for the `"index_reduction:d_dt_for_"` marker in equation origins.
fn finish_report(
    dae: &rumoca_ir_dae::Dae,
    states_before: Vec<String>,
    steps: Vec<(&'static str, String)>,
    stopped_at: Option<&'static str>,
) -> ReductionReport {
    let states_after: Vec<String> = dae.variables.states.keys().map(|k| k.to_string()).collect();
    const DIFF_ROW_MARKER: &str = "index_reduction:d_dt_for_";
    let differentiated_rows: Vec<(String, String)> = dae
        .continuous
        .equations
        .iter()
        .filter_map(|eq| {
            eq.origin.find(DIFF_ROW_MARKER).map(|pos| {
                let state = eq.origin[pos + DIFF_ROW_MARKER.len()..].to_owned();
                (eq.origin.clone(), state)
            })
        })
        .collect();
    ReductionReport {
        states_before,
        states_after,
        steps,
        differentiated_rows,
        eliminations: Vec::new(),
        stopped_at,
    }
}

impl ReductionReport {
    /// Builder method: attach trivial-elimination substitutions to the report.
    /// `mut self` takes ownership (not `&mut self`) — this is the "builder
    /// pattern" where you chain `.with_eliminations(...)` on the return value.
    fn with_eliminations(mut self, eliminations: Vec<(String, String)>) -> Self {
        self.eliminations = eliminations;
        self
    }

    /// Serialize the report to JSON for the Index Reduction tab.
    /// Computes `demoted_states` as the set difference (before - after).
    fn to_json(&self) -> serde_json::Value {
        // Which states were demoted? Filter states_before for those NOT in
        // states_after. This is O(n*m) but both lists are tiny (< 20 states).
        let demoted: Vec<&String> = self
            .states_before
            .iter()
            .filter(|s| !self.states_after.contains(s))
            .collect();
        serde_json::json!({
            "states_before": self.states_before,
            "states_after": self.states_after,
            "demoted_states": demoted,
            "n_states_before": self.states_before.len(),
            "n_states_after": self.states_after.len(),
            "steps": self.steps.iter().map(|(name, outcome)| {
                serde_json::json!({ "step": name, "outcome": outcome })
            }).collect::<Vec<_>>(),
            "differentiated_rows": self.differentiated_rows.iter().map(|(origin, state)| {
                serde_json::json!({ "equation_origin": origin, "for_state": state })
            }).collect::<Vec<_>>(),
            "eliminations": self.eliminations.iter().map(|(var, expr)| {
                serde_json::json!({ "variable": var, "replacement": expr })
            }).collect::<Vec<_>>(),
            "stopped_at": self.stopped_at,
            "funnel_completed": self.stopped_at.is_none(),
        })
    }
}

/// Serialize a single class from a class tree by its qualified name.
///
/// This is how we extract "just the user's model" from the huge resolved tree
/// (which includes the entire MSL). `get_class_by_qualified_name` does a
/// name-based lookup; `serde_json::to_value` serializes the Rust struct to a
/// generic JSON `Value` (the format the tree inspector displays).
fn extract_class(tree: &rumoca_ir_ast::ClassTree, qualified_name: &str) -> Stage {
    match tree.get_class_by_qualified_name(qualified_name) {
        Some(class) => Stage::from_ser(class),
        None => Stage::err(format!("`{qualified_name}` not found in resolved tree")),
    }
}

/// Captures stdout and stderr at the file-descriptor level so that `println!`
/// and `eprintln!` output from Rumoca library calls is intercepted and forwarded
/// as log entries.
///
/// # Why fd-level capture? Why not just redirect `std::io::stdout()`?
///
/// Rust's `std::io::stdout()` is a wrapper around file descriptor 1. But C
/// libraries (and Rust code that calls `libc::write` directly) write to the
/// raw file descriptor, bypassing the Rust wrapper. Since Rumoca links C
/// libraries (LLVM, etc.), we need to capture at the fd level to catch ALL
/// output.
///
/// # How the pipe/dup2 trick works
///
/// 1. Create a pipe: `pipe()` returns two fds — `[read_end, write_end]`
/// 2. Save the original stdout fd: `dup(1)` → `old_stdout`
/// 3. Replace stdout with the pipe's write end: `dup2(write_end, 1)`
/// 4. Now anything written to fd 1 (stdout) goes into the pipe
/// 5. Read from the pipe's read end to get the captured output
/// 6. On `Drop`, restore the original: `dup2(old_stdout, 1)`
///
/// # Cross-platform support
///
/// `dup`, `dup2`, and `close` have identical signatures on unix and Windows
/// (libc maps the Windows CRT `_dup`/`_dup2`/`_close` to the same names).
/// Two operations differ: `pipe()` takes extra arguments on Windows (buffer
/// size + binary mode flag), and converting a raw fd to a `std::fs::File`
/// uses `FromRawFd` on unix vs `get_osfhandle` + `FromRawHandle` on Windows.
/// These are abstracted in `create_pipe()` and `file_from_raw_fd()`.
///
/// # Safety
///
/// The `unsafe` blocks are required because `libc::dup2()` etc. are foreign
/// function calls (FFI) that Rust can't verify for memory safety. The
/// invariants we maintain: we always restore the original fds on Drop, we
/// close all fds we open, and we never use a closed fd.
struct OutputCapture {
    /// Saved copy of the original stdout (fd 1), restored on Drop.
    old_stdout: i32,
    /// Saved copy of the original stderr (fd 2), restored on Drop.
    old_stderr: i32,
    /// Accumulated stdout bytes, filled continuously by a reader thread.
    stdout_buf: Arc<Mutex<Vec<u8>>>,
    /// Accumulated stderr bytes, filled continuously by a reader thread.
    stderr_buf: Arc<Mutex<Vec<u8>>>,
    /// Reader thread handles — joined on Drop to ensure clean shutdown.
    readers: Option<(thread::JoinHandle<()>, thread::JoinHandle<()>)>,
}

#[cfg(unix)]
unsafe fn create_pipe(fds: &mut [i32; 2]) -> bool {
    unsafe { libc::pipe(fds.as_mut_ptr()) == 0 }
}

#[cfg(windows)]
unsafe fn create_pipe(fds: &mut [i32; 2]) -> bool {
    // _O_BINARY = 0x8000: no CR/LF translation.
    unsafe { libc::pipe(fds.as_mut_ptr(), 65536, 0x8000) == 0 }
}

#[cfg(unix)]
unsafe fn file_from_raw_fd(fd: i32) -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    unsafe { std::fs::File::from_raw_fd(fd) }
}

#[cfg(windows)]
unsafe fn file_from_raw_fd(fd: i32) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
    unsafe {
        let handle = libc::get_osfhandle(fd) as std::os::windows::io::RawHandle;
        std::fs::File::from_raw_handle(handle)
    }
}

impl OutputCapture {
    /// Set up the pipe/dup2 capture. Returns `None` if any system call fails
    /// (rather than panicking — output capture is best-effort, not critical).
    ///
    /// Spawns two reader threads that continuously drain the pipe read ends
    /// into shared buffers. This prevents the 64KB pipe-buffer deadlock: no
    /// matter how much a Rumoca phase writes, the readers keep the buffer
    /// from filling. The write side stays in normal blocking mode so
    /// `println!`/`eprintln!` never see `EAGAIN`.
    fn start() -> Option<Self> {
        unsafe {
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::Write::flush(&mut std::io::stderr());

            let mut out_fds = [0i32; 2];
            let mut err_fds = [0i32; 2];
            if !create_pipe(&mut out_fds) {
                return None;
            }
            if !create_pipe(&mut err_fds) {
                libc::close(out_fds[0]);
                libc::close(out_fds[1]);
                return None;
            }

            let old_stdout = libc::dup(1);
            let old_stderr = libc::dup(2);
            if old_stdout < 0 || old_stderr < 0 {
                if old_stdout >= 0 { libc::close(old_stdout); }
                if old_stderr >= 0 { libc::close(old_stderr); }
                libc::close(out_fds[0]); libc::close(out_fds[1]);
                libc::close(err_fds[0]); libc::close(err_fds[1]);
                return None;
            }

            if libc::dup2(out_fds[1], 1) < 0 || libc::dup2(err_fds[1], 2) < 0 {
                libc::dup2(old_stdout, 1);
                libc::dup2(old_stderr, 2);
                libc::close(old_stdout); libc::close(old_stderr);
                libc::close(out_fds[0]); libc::close(out_fds[1]);
                libc::close(err_fds[0]); libc::close(err_fds[1]);
                return None;
            }
            // Close the original write ends — fd 1 and fd 2 are the only
            // writers now (via dup2). The reader threads will see EOF when
            // Drop restores the original fds and closes the pipe write ends.
            libc::close(out_fds[1]);
            libc::close(err_fds[1]);

            let stdout_buf = Arc::new(Mutex::new(Vec::new()));
            let stderr_buf = Arc::new(Mutex::new(Vec::new()));

            let out_reader = Self::spawn_reader(
                file_from_raw_fd(out_fds[0]),
                Arc::clone(&stdout_buf),
            );
            let err_reader = Self::spawn_reader(
                file_from_raw_fd(err_fds[0]),
                Arc::clone(&stderr_buf),
            );

            Some(OutputCapture {
                old_stdout,
                old_stderr,
                stdout_buf,
                stderr_buf,
                readers: Some((out_reader, err_reader)),
            })
        }
    }

    fn spawn_reader(mut file: std::fs::File, buf: Arc<Mutex<Vec<u8>>>) -> thread::JoinHandle<()> {
        thread::Builder::new()
            .name("output-capture-reader".to_owned())
            .spawn(move || {
                use std::io::Read;
                let mut chunk = [0u8; 4096];
                loop {
                    match file.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            if let Ok(mut locked) = buf.lock() {
                                locked.extend_from_slice(&chunk[..n]);
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
            })
            .expect("spawn output-capture reader")
    }

    /// Take any output that has accumulated since the last drain.
    fn drain(&mut self) -> (String, String) {
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let _ = std::io::Write::flush(&mut std::io::stderr());
        let take = |buf: &Mutex<Vec<u8>>| -> String {
            let bytes = std::mem::take(&mut *buf.lock().unwrap());
            String::from_utf8_lossy(&bytes).into_owned()
        };
        (take(&self.stdout_buf), take(&self.stderr_buf))
    }
}

/// RAII cleanup: restore the original stdout/stderr fds, then join the
/// reader threads. Restoring the fds closes the pipe write ends (fd 1/2
/// revert to the saved originals), which makes the reader threads see EOF.
impl Drop for OutputCapture {
    fn drop(&mut self) {
        unsafe {
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::Write::flush(&mut std::io::stderr());
            libc::dup2(self.old_stdout, 1);
            libc::dup2(self.old_stderr, 2);
            libc::close(self.old_stdout);
            libc::close(self.old_stderr);
        }
        if let Some((r1, r2)) = self.readers.take() {
            let _ = r1.join();
            let _ = r2.join();
        }
    }
}

// ==========================================================================
// Tracing subscriber — captures Rumoca's internal `tracing` events
// ==========================================================================
//
// Rumoca uses the `tracing` crate for internal logging (`tracing::debug!`,
// `tracing::warn!`, etc.). By default, these events go nowhere (no subscriber
// is installed). HRW installs a custom subscriber (`TracingForwarder`) that
// buffers events into a thread-local `Vec`, which `compile()` drains after
// each pipeline stage and forwards as `FromWorker::Log` entries.
//
// Why thread-local? Because the subscriber is installed per-thread via
// `tracing::subscriber::set_default()` (not `set_global_default()`).
// This means only the worker thread captures tracing events — the UI thread
// is unaffected. And the buffer is thread-local (`thread_local!`), so
// draining it from the worker thread is safe without any locking.
//
// Why a custom subscriber instead of `tracing_subscriber::fmt`? Because we
// need to capture events as structured data (level + message) for the log
// view, not just print them to stderr. A standard subscriber would write to
// stderr, which we'd then have to re-capture via `OutputCapture` — adding
// a round-trip through the fd-level pipe. Direct capture is simpler.
// ==========================================================================

// `RefCell` provides interior mutability — it lets us mutate the `Vec` even
// though `thread_local!` gives us a shared reference (`&`). `RefCell`
// enforces borrowing rules at runtime (panics on double-borrow) instead of
// compile time. This is safe here because we only access the buffer in
// `drain_traces` (which borrows mutably) and `TracingForwarder::event`
// (which also borrows mutably), and these never overlap (they run on the
// same thread, sequentially).
use std::cell::RefCell;

// `thread_local!` creates a variable that has a separate instance per thread.
// Each thread gets its own `Vec` — no synchronization needed. The `const { ... }`
// syntax is a const initializer (the Vec is created at zero cost, without
// calling a function at runtime).
thread_local! {
    static TRACE_BUFFER: RefCell<Vec<(tracing::Level, String)>> = const { RefCell::new(Vec::new()) };
}

/// Drain all buffered tracing events and forward them as log entries.
///
/// Called after each Rumoca API call in `compile()` and `simulate()`.
/// `TRACE_BUFFER.with(|buf| ...)` accesses this thread's buffer.
/// `buf.borrow_mut().drain(..)` takes all elements out of the Vec (leaving
/// it empty) and iterates over them. The `&dyn Fn(...)` parameter is a
/// trait object — dynamic dispatch (vs `&impl Fn` which is static dispatch).
/// Used here because `drain_traces` is called from multiple contexts.
/// **Phases that cannot produce tracing output, and why.**
///
/// Turning tracing on and watching a phase stay silent is ambiguous: it means
/// either *nothing notable happened* or *nobody instrumented this*, and those call
/// for opposite responses. The log could not tell them apart, so a reader learning
/// the compiler would take an unwired phase for a quiet one — **the wrong-negative
/// shape this project treats as the error nobody catches**, because acting on it
/// means not looking.
///
/// Doug, 2026-08-04, on relying on the log as a student of Rumoca: *"logging must
/// always be accurate."* Stating the absence is what makes silence readable.
///
/// **Kept true by `the_uninstrumented_phase_list_matches_the_crates`**, which counts
/// tracing call sites in each crate. Instrument one of these upstream and the test
/// fails until the entry is removed — so this cannot rot into a stale claim, which
/// is the standing rule for any assertion that something does not exist.
/// **Two different reasons for silence**, and they are not the same finding.
///
/// The first draft of this list got it wrong in a way worth recording: it claimed
/// `rumoca-phase-initialization` and `rumoca-phase-events` had no tracing calls.
/// **Those crates do not exist.** The zero came from grepping a directory that was
/// not there — an absence of evidence read as evidence of absence, in the very list
/// written to stop that mistake. The test below caught it before it shipped.
const UNINSTRUMENTED_PHASES: &[(&str, Option<&str>, &str)] = &[
    (
        "Parse",
        Some("rumoca-phase-parse"),
        "its generated parser traces through the `log` crate \
         (`parol_runtime::log::trace`), which HRW's `tracing` subscriber does not capture",
    ),
    // Typecheck was here until 2026-08-04, when `rumoca-phase-typecheck` gained
    // tracing — including its three previously silent early returns. **The test
    // below is what removed it**: it failed with "now has 8 tracing call site(s)"
    // the moment the crate changed, which is the whole point of pinning a claim of
    // absence to something that can notice.
    // **Events is a reading, not a computation.** `events_stage` renders
    // `dae.conditions`, `dae.discrete` and `dae.events` — data Rumoca produced
    // during DAE construction — and calls no Rumoca function of its own. So no
    // Rumoca event can be emitted under this tab, and instrumenting upstream will
    // never make it speak. The data is real; only the *phase* is not.
    //
    // **Initialization was listed here and should not have been.** It calls
    // `rumoca_phase_structural::build_ic_plan`, a real algorithm in an instrumented
    // crate — the claim was written after reading the first ten lines of
    // `initialization_stage` and stopping before the call. The
    // `calls-no-Rumoca-function` check below exists because of that.
    (
        "Events",
        None,
        "HRW renders this tab from `dae.events`; no Rumoca function is called",
    ),
];

/// The one-line notice a traced compile opens with.
///
/// Separated from the emitting so it can be asserted without running a compile.
fn uninstrumented_notice() -> String {
    let names: Vec<&str> = UNINSTRUMENTED_PHASES.iter().map(|(p, _, _)| *p).collect();
    format!(
        "tracing on \u{2014} these phases have no Rumoca instrumentation and will be \
         silent regardless of what they do: {}. Silence from any other phase means it \
         had nothing to say.",
        names.join(", "),
    )
}

/// Discard buffered tracing events without reporting them.
///
/// Called at the start of every compile and when tracing is switched off. Both
/// are the same guarantee stated twice: **a log entry must belong to the run it
/// appears under.** `drain_traces` empties the buffer *into the log*, which is
/// wrong for events that belong to a previous run or to a setting the user has
/// since turned off — those must be dropped, not relocated.
fn clear_traces() {
    TRACE_BUFFER.with(|buf| buf.borrow_mut().clear());
}

fn drain_traces(log_fn: &dyn Fn(LogLevel, String)) {
    TRACE_BUFFER.with(|buf| {
        for (level, msg) in buf.borrow_mut().drain(..) {
            let ll = match level {
                tracing::Level::ERROR => LogLevel::Error,
                tracing::Level::WARN => LogLevel::Warn,
                _ => LogLevel::Trace,
            };
            log_fn(ll, msg);
        }
    });
}

/// A minimal `tracing::Subscriber` that captures events into the thread-local
/// `TRACE_BUFFER` instead of printing them.
///
/// The `tracing::Subscriber` trait requires implementing several methods.
/// Most are no-ops here (we don't use spans, just events). The key method
/// is `event()`, which formats each tracing event into a string and pushes
/// it to the buffer.
///
/// This is a unit struct (no fields) — it carries no state. All state lives
/// in the `thread_local!` buffer.
struct TracingForwarder;

impl tracing::Subscriber for TracingForwarder {
    /// Filter: only capture events at DEBUG level or above (DEBUG, INFO, WARN,
    /// ERROR). TRACE-level events are too noisy. The `<=` comparison works
    /// because `tracing::Level` orders from most severe (ERROR) to least (TRACE).
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        *metadata.level() <= tracing::Level::DEBUG
    }

    /// Required by the trait but unused — we don't track spans (structured
    /// scopes). Returns a dummy span ID. Spans would be useful for tracking
    /// "inside phase X" context, but we don't need that yet.
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    // No-op trait methods — required but unused since we only capture events.
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}

    /// The core method: called for every `tracing::debug!`, `tracing::warn!`,
    /// etc. event on this thread. Formats the event into a string like
    /// `[rumoca_phase_structural] matching found 12 pairs` and pushes it to
    /// the thread-local buffer.
    ///
    /// The `Visitor` struct implements `tracing::field::Visit` — the visitor
    /// pattern that `tracing` uses to iterate over an event's fields. The
    /// "message" field gets special treatment (no key prefix); other fields
    /// are formatted as `key=value`. This inner struct definition is legal
    /// in Rust — you can define types inside a function body, scoped to
    /// that function.
    fn event(&self, event: &tracing::Event<'_>) {
        use std::fmt::Write;
        let meta = event.metadata();
        let mut msg = String::new();
        // `target` is usually the Rust module path, e.g. "rumoca_phase_structural".
        let target = meta.target();
        let _ = write!(msg, "[{target}] ");

        struct Visitor<'a>(&'a mut String);
        impl tracing::field::Visit for Visitor<'_> {
            fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
                use std::fmt::Write;
                if field.name() == "message" {
                    let _ = write!(self.0, "{value:?}");
                } else {
                    let _ = write!(self.0, " {}={:?}", field.name(), value);
                }
            }
            fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
                use std::fmt::Write;
                if field.name() == "message" {
                    let _ = write!(self.0, "{value}");
                } else {
                    let _ = write!(self.0, " {}={value}", field.name());
                }
            }
        }
        event.record(&mut Visitor(&mut msg));

        // Push to the thread-local buffer. `*meta.level()` dereferences the
        // level (it's `Copy`, so this is cheap). The buffer will be drained
        // by `drain_traces()` after the current API call completes.
        TRACE_BUFFER.with(|buf| buf.borrow_mut().push((*meta.level(), msg)));
    }

    // Span enter/exit — no-ops since we don't use span tracking.
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}
