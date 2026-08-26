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
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
// `mpsc` = multi-producer, single-consumer channel. Here we use it as
// single-producer (worker) / single-consumer (UI) in each direction.
// `Sender` and `Receiver` are the two halves of a channel; `Sender` is
// `Clone` (multi-producer) but we only have one of each.
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
// `StoredDefinition` is one parsed Modelica file, as the source-root cache hands
// it back -- see `parsed_source_root`.
use rumoca_ir_ast::StoredDefinition;
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
// of `.mo` files into a `ParsedSourceRoot`, `source_root_input_cache_key`
// fingerprints that directory *without* parsing it, and
// `source_root_source_set_key` generates a stable key for caching.
use rumoca_compile::source_roots::{
    parse_source_root_with_cache, source_root_input_cache_key, source_root_source_set_key,
};
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
    ///
    /// `is_library` says which kind of thing `path` names, and it is a **flag rather
    /// than a sniff** for the same reason `App::selected_is_library` is: for a
    /// library model `path` holds the *qualified name*
    /// (`Modelica.Blocks.Continuous.SecondOrder`), which is not a file and which a
    /// dot-counting heuristic cannot reliably tell from a filename containing a dot.
    ///
    /// **Added 2026-08-04.** Until then this message had no such field and `simulate`
    /// began with `read_to_string(path)`, so pressing Run on any MSL model produced
    /// *"read error: The system cannot find the file specified. (os error 2)"* — the
    /// compile path had gained `CompileLibraryModel` and the simulate path never got
    /// its counterpart. Reported by Doug on `Modelica.Blocks.Continuous.SecondOrder`.
    Simulate {
        path: PathBuf,
        model: String,
        t_end: f64,
        is_library: bool,
    },
    /// Enable or disable Rumoca's internal `tracing` subscriber on this thread.
    SetTracing(bool),
    /// **Step connection expansion under the debugger, on this thread.**
    ///
    /// # Why this one is a worker command when no other live debug is
    ///
    /// Every other live-stepped view spawns an algorithm thread from the *UI* thread
    /// with copied data — matching gets an `IncidenceMatrix`, `pre()` lowering gets a
    /// flat model. Connection expansion cannot: it runs inside
    /// `compile_model_strict_reachable_*`, which needs the resolved `ClassTree` (the
    /// whole MSL) and the session that owns it. Shipping that to the UI thread to arm
    /// a breakpoint was the blocker recorded as `docs/ideas.md` #9.
    ///
    /// The session already lives here, so the work comes to the data rather than the
    /// reverse.
    ///
    /// # And the pass being stepped is the compilation
    ///
    /// The others *re-run* their algorithm so the debugger has something to stop
    /// inside — a second execution that has to be argued equivalent to the first.
    /// This installs `connections::trace::start_live` around a real compile, so the
    /// frames and the breakpoint both come from the run that actually happens.
    ///
    /// `trace` carries the producer half of the channel the UI is already draining;
    /// `done` tells the animation the session ended, since a live `Playback` cannot
    /// know that from an empty channel.
    LiveDebugConnections {
        path: PathBuf,
        /// Simple model name, so the worker need not re-parse to qualify it.
        model: String,
        trace: rumoca_phase_structural::live_trace::LiveTrace<
            rumoca_phase_flatten::connections::trace::ConnectionFrame,
        >,
        done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    },
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

impl SimData {
    /// Variables carrying a non-finite value, with how many samples are affected.
    ///
    /// # Why this exists: a successful simulation can contain an infinity
    ///
    /// **Measured 2026-08-25, after two probes that failed to reproduce it.** The
    /// solver's finiteness guards live in `rumoca-solver`'s *projection* path, so
    /// they catch a non-finite value at initialization and in constrained systems —
    /// both earlier probes died there. But an ordinary algebraic **output** that goes
    /// singular mid-run is not watched by anything: the integrator's error control
    /// follows states, and an output is not a state. A three-equation model whose
    /// output divides by zero at `t = 0.5` returned **`Ok`** with one infinity in its
    /// series.
    ///
    /// **Until this existed, HRW plotted that silently.** `hrw/src/` contained no
    /// finiteness check of any kind, so the pane drew an infinity, `egui_plot`'s
    /// auto-bounds did whatever they do with one, and nothing said why the picture
    /// looked wrong. That is the absence rule pointed at the plot: a trajectory HRW
    /// cannot faithfully draw must be *reported*, not quietly rendered.
    ///
    /// # Why it is data rather than a check inside the painter
    ///
    /// `CLAUDE.md` requires computation to move **out** of the paint path into
    /// checkable data when these files are touched — the painter is one of the two
    /// surfaces `egui_kittest` cannot reach, and Doug edits it. A `Vec` a test can
    /// read is the difference between a guard that is verified and one that is hoped
    /// for.
    pub fn non_finite_series(&self) -> Vec<(String, usize)> {
        self.names
            .iter()
            .zip(self.data.iter())
            .filter_map(|(name, series)| {
                let n = series.iter().filter(|v| !v.is_finite()).count();
                (n > 0).then(|| (name.clone(), n))
            })
            .collect()
    }
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
    ///
    /// *(The `#[allow(dead_code)]` here was removed 2026-08-04: this variant now has
    /// a second constructor in `simulate`, which is the fix for simulation never
    /// having worked on a corpus model.)*
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

/// **Where a pane's content came from** — the data half of Charter Decision 7.
///
/// A stage tab is a claim: *this is what the compiler has for this phase*. Until
/// 2026-08-04 nothing recorded whether that was true, so a pane that HRW had
/// computed itself was indistinguishable from one the compiler produced — which is
/// how the BLT tabs came to render a decomposition for a system the compiler had
/// refused to decompose.
///
/// **The line is drawn at "is this content a function of THIS RUN's compiler
/// output?"**, not at "did HRW do any arithmetic". Selecting fields, reshaping them
/// into JSON, and computing a summary from compiler-produced counts are all
/// [`Compiler`](Self::Compiler): the facts are the compiler's and HRW is presenting
/// them. What makes content [`Hrw`](Self::Hrw) is HRW **executing an algorithm the
/// compiler also runs**, or synthesising a structure the compiler never emitted.
/// That is the crisp version of the distinction, and it is crisp because both
/// removed fictions land unambiguously on the far side of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Provenance {
    /// No content — `value` is `None`, so there is nothing that could be misread.
    /// A stage stating an absence lives here, which is the correct place for it.
    #[default]
    Empty,
    /// Every value shown is a function of an artifact the compiler produced on this
    /// run. HRW selected, reshaped and summarised; it added no facts of its own.
    Compiler,
    /// Contains content HRW produced that the compiler did not — a re-executed
    /// algorithm, or a structure synthesised to fill a pane.
    ///
    /// **No production stage is allowed to be this today**, and
    /// `no_stage_shows_content_hrw_invented` fails if one becomes it. The variant
    /// exists so that a future pane which genuinely needs derived content has a way
    /// to say so — and so that saying so is a deliberate act with a test failure
    /// attached, rather than the silent default it used to be. Build one with
    /// [`Stage::computed`], which makes you write down why.
    Hrw,
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
    /// Where [`value`](Self::value) came from. Set by the constructors; see
    /// [`Provenance`].
    pub provenance: Provenance,
}

/// Constructors for the possible stage outcomes. `pub(crate)` — only the worker
/// builds stages in production; the UI consumes them read-only, and tests build
/// them through these rather than by struct literal so a new field cannot be
/// forgotten at one site.
impl Stage {
    /// Stage succeeded and produced an IR tree to display.
    pub(crate) fn ok(value: serde_json::Value) -> Self {
        Stage {
            value: Some(value),
            note: None,
            outcome: Outcome::Ok,
            provenance: Provenance::Compiler,
        }
    }

    /// **A pane whose content HRW produced, with a written reason.**
    ///
    /// The only way to build a [`Provenance::Hrw`] stage, and deliberately more
    /// expensive to call than [`ok`](Self::ok): it demands a `why` that the UI shows,
    /// so a reader is never looking at HRW's own work believing it to be the
    /// compiler's. **Unused in production**, and
    /// `no_stage_shows_content_hrw_invented` fails if that changes without the same
    /// commit dealing with the display.
    ///
    /// This exists because the alternative to a supported path is not "nobody does
    /// it" — it is somebody calling `ok` with synthesised JSON, which is exactly what
    /// happened to the BLT tabs. **The friction belongs here, not on honesty.**
    /// `expect` rather than `allow`, deliberately: the moment anything calls this the
    /// lint stops firing, the expectation goes unfulfilled, and the compiler says so.
    /// **The scaffolding removes itself when the case it anticipates arrives**, which
    /// is the difference between a kept-for-later API and a quietly dead one.
    ///
    /// **And it fired within the day.** The expectation was unconditional until
    /// `fidelity::check_f10`'s must-fire test began constructing an `Hrw` stage to
    /// prove F10 can fail — so it is now scoped to non-test builds, where the
    /// function genuinely remains unused. The mechanism reported its own change of
    /// circumstance rather than going quietly stale, which is the whole point of
    /// preferring `expect` here.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "\
        no production pane derives its own content; kept as the only legal way to do \
        so, and exercised by F10's must-fire test - see the type docs"
        )
    )]
    pub(crate) fn computed(value: serde_json::Value, why: impl Into<String>) -> Self {
        Stage {
            value: Some(value),
            note: Some(why.into()),
            outcome: Outcome::Flagged,
            provenance: Provenance::Hrw,
        }
    }
    /// Stage failed — no IR, just an error message (rendered red).
    /// `impl Into<String>` accepts both `String` and `&str` — a Rust ergonomic
    /// pattern so callers can pass either without explicit conversion.
    pub(crate) fn err(note: impl Into<String>) -> Self {
        Stage {
            value: None,
            note: Some(note.into()),
            outcome: Outcome::Failed,
            provenance: Provenance::Empty,
        }
    }
    /// A non-error status note for a stage with no IR of its own to show.
    ///
    /// **This is where "absence is stated, never filled" lives.** A phase that did not
    /// run reaches the user through here, carrying `None` — so there is no content to
    /// be misread as the compiler's, and `a_phase_that_did_not_run_shows_nothing`
    /// checks that as a class rather than one tab at a time.
    pub(crate) fn info(note: impl Into<String>) -> Self {
        Stage {
            value: None,
            note: Some(note.into()),
            outcome: Outcome::Ok,
            provenance: Provenance::Empty,
        }
    }
    /// A best-effort IR plus an error note — a recovered parse tree, a singular
    /// structural analysis, surplus initial conditions. [`Outcome::Flagged`]:
    /// **the value is real and downstream stages consume it.**
    pub(crate) fn recovered(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage {
            value: Some(value),
            note: Some(note.into()),
            outcome: Outcome::Flagged,
            provenance: Provenance::Compiler,
        }
    }
    /// A successful IR plus an informational (non-error) note — e.g. the
    /// index-reduction stage's "already index-1" / "reduced from singular".
    pub(crate) fn ok_with_note(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage {
            value: Some(value),
            note: Some(note.into()),
            outcome: Outcome::Ok,
            provenance: Provenance::Compiler,
        }
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
            // The payload is the compiler's diagnostics, reshaped for display —
            // HRW selected and wrapped, it did not invent a failure.
            provenance: Provenance::Compiler,
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
        StageKind::Parse,
        StageKind::Resolve,
        StageKind::Instantiate,
        StageKind::Typecheck,
        StageKind::Flatten,
        StageKind::Dae,
        StageKind::Structural,
        StageKind::IndexReduction,
        StageKind::Initialization,
        StageKind::Events,
        StageKind::SolveLowering,
        StageKind::Simulation,
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
        StageKind::Parse,
        StageKind::Resolve,
        StageKind::Instantiate,
        StageKind::Typecheck,
        StageKind::Flatten,
        StageKind::Dae,
        StageKind::Structural,
        StageKind::IndexReduction,
        StageKind::Initialization,
        StageKind::Events,
        StageKind::SolveLowering,
    ];

    /// The key this stage carries in the specimen notebook — the trace file's stem
    /// (`index_reduction.json`) and its entry in `manifest.json`'s `stages` map.
    ///
    /// **Derived from [`slug`] rather than hand-listed, which is the whole point.**
    /// `examples/gen_trace.rs` used to carry its own `const STAGES: [&str; 11]`, a
    /// second roster nothing held to the first. When `Dae` was added to the pipeline
    /// that list was not updated, so the notebook silently described an eleven-stage
    /// compiler for weeks — found 2026-08-15, with **7 of 21** manifests listing `dae`
    /// and the rest not. A snake_case transform of the canonical slug cannot fall
    /// behind the enum.
    ///
    /// Returns an owned `String` because the transform is computed; the allocation is
    /// paid once per stage per specimen, in a generator and a test.
    #[must_use]
    pub fn notebook_key(self) -> String {
        let slug = self.slug();
        let mut out = String::with_capacity(slug.len() + 2);
        for (i, c) in slug.char_indices() {
            if c.is_ascii_uppercase() && i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        }
        out
    }

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

    /// The name this phase carries in the **log**, which is not always its tab name.
    ///
    /// A third naming, alongside [`name`](Self::name) and [`slug`](Self::slug), and it
    /// exists for the same reason as the second: the log says *"DAE construction"* and
    /// *"Structural analysis"* where the tabs say *"DAE"* and *"Structural"*, because a
    /// bracket names an **activity** while a tab names an **artifact**.
    ///
    /// **Why this is a function and not eleven string literals** *(2026-08-04)*. The
    /// bracket names used to be free-form literals at the emit sites, connected to
    /// nothing — which is precisely how the log came to contain a bracket called
    /// **"DAE pipeline"**, a phase that does not exist, invented to give five real
    /// phases a tidy parent. A name that must come from a `StageKind` cannot be
    /// invented without inventing a phase.
    pub fn log_name(self) -> &'static str {
        match self {
            StageKind::Dae => "DAE construction",
            StageKind::Structural => "Structural analysis",
            other => other.name(),
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
            StageKind::Simulation => panic!(
                "Simulation is not a compilation stage — handle it before calling StageBundle::get()"
            ),
        }
    }

    /// Mutable access by kind, **for tests that need to construct a specific
    /// bundle state**.
    ///
    /// Added 2026-08-04 for `f10_catches_each_way_a_pane_can_mislead`, which has to
    /// build the three states F10 rejects — including a *silent blank* pane, which
    /// production reaches only mid-compile and which no constructor produces on
    /// purpose. Without this the check could only be exercised through a real compile,
    /// and a check about honesty that cannot be made to fail is the exact shape of the
    /// problem it was written for.
    ///
    /// **Un-gated from `#[cfg(test)]` on 2026-08-25** for `panicked_compile`, which
    /// must put the same note on every stage. The alternative — a struct literal with
    /// `..Default::default()` — would leave a newly added stage silently blank on a
    /// panic, and `CLAUDE.md` requires new stages to be wired into every per-stage
    /// system. Driving the roster keeps that automatic.
    pub fn get_mut(&mut self, kind: StageKind) -> &mut Stage {
        match kind {
            StageKind::Parse => &mut self.parse,
            StageKind::Resolve => &mut self.resolve,
            StageKind::Instantiate => &mut self.instantiate,
            StageKind::Typecheck => &mut self.typecheck,
            StageKind::Flatten => &mut self.flatten,
            StageKind::Dae => &mut self.dae,
            StageKind::Structural => &mut self.structural,
            StageKind::IndexReduction => &mut self.index_reduction,
            StageKind::Initialization => &mut self.initialization,
            StageKind::Events => &mut self.events,
            StageKind::SolveLowering => &mut self.solve_lowering,
            StageKind::Simulation => {
                panic!("Simulation is not a compilation stage")
            }
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
    /// How deeply this entry is nested inside open phase brackets.
    ///
    /// Doug, 2026-08-04: *"if some important activities such as connection
    /// computation are happening within phases, then the log should reflect that
    /// fact. If necessary, log line indenting can be used to emphasize that some
    /// major log lines are contained within the scope of other major log lines."*
    ///
    /// A flat list of brackets cannot say *contained in*. It could only say
    /// *adjacent to*, which is how HRW's own replays came to look like phases —
    /// there was no level at which to put something that happens **inside** the
    /// compile rather than beside it.
    ///
    /// Maintained by [`make_log`] from the `StageStart`/`StageEnd` stream itself, so
    /// no call site passes it and none can get it wrong.
    pub depth: u8,
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
        /// Every step of the matching search, **from the run that produced the
        /// blocks above** — not a re-derivation performed when the tab opens.
        matching_frames: Vec<rumoca_phase_structural::matching::MatchingFrame>,
        /// The SCC search, from the same run.
        tarjan_frames: Vec<rumoca_phase_structural::tarjan::TarjanFrame>,
        /// Tearing decisions, one segment per coupled block.
        tearing_frames: Vec<Vec<rumoca_phase_structural::tearing::TearingFrame>>,
        /// The same three searches over the **index-reduced** system, which is a
        /// different DAE — see `StructuralFrames`.
        reduced_frames: StructuralFrames,
        /// `pre()`-lowering frames (idea #40), **captured during the compile above**
        /// by `rumoca_phase_dae`'s capture scope. The pass runs *inside* DAE
        /// construction, so the finished DAE cannot be re-walked to recover them —
        /// which is why the scope exists rather than a second pass.
        ///
        /// *(Comment corrected 2026-08-04: it read "replay frames … recorded by
        /// re-running DAE construction", describing the mechanism this replaced
        /// earlier the same day.)*
        pre_lowering_frames: Vec<rumoca_phase_dae::PreLoweringFrame>,
        /// The flat model, for a live-debug replay of `pre()` lowering. It is the
        /// last artifact from *before* that pass runs.
        flat: Option<rumoca_ir_flat::Model>,
        /// The raw DAE for live-debug replay of index reduction.
        dae: Option<rumoca_ir_dae::Dae>,
        /// Connection-expansion frames (MLS §9), **captured during the compile
        /// above**; empty for a model with no `connect()`. *(Comment corrected
        /// 2026-08-04: it read "replay frames … recorded by re-running flatten".
        /// That re-run was the fiction Doug named first — "the connection replay is
        /// a fiction, invented for logging" — and it is gone.)*
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

/// What a request must be answered with if handling it panics.
///
/// **Decided from `&ToWorker` before `handle` consumes it**, which is the whole
/// reason this type exists: after the panic the message is gone, and an unanswered
/// request is a hang.
///
/// Exhaustive in [`panic_reply`], so a new `ToWorker` variant cannot be added without
/// deciding how a panic in it reaches the user — the same compile-time discipline
/// `expects_a_response` uses for the transport contract.
enum PanicReply {
    Compiled(PathBuf),
    Libraries,
    DefTree(String),
    Simulated(PathBuf),
    /// Fire-and-forget. The flag, when present, still has to be signalled: a
    /// `LiveDebugConnections` that panics without setting `done` leaves the live
    /// `Playback` waiting for a session that already ended.
    Silent(Option<Arc<std::sync::atomic::AtomicBool>>),
}

/// How to answer `msg` if handling it panics.
fn panic_reply(msg: &ToWorker) -> PanicReply {
    match msg {
        ToWorker::SetLibraries(_) => PanicReply::Libraries,
        ToWorker::Compile(p) => PanicReply::Compiled(p.clone()),
        ToWorker::CompileLibraryModel(name) => PanicReply::Compiled(PathBuf::from(name)),
        ToWorker::OpenDef(name) => PanicReply::DefTree(name.clone()),
        ToWorker::Simulate { path, .. } => PanicReply::Simulated(path.clone()),
        ToWorker::SetTracing(_) => PanicReply::Silent(None),
        ToWorker::LiveDebugConnections { done, .. } => PanicReply::Silent(Some(Arc::clone(done))),
    }
}

/// Run `f`, turning a panic into its message.
///
/// **`AssertUnwindSafe` is load-bearing and is only sound because of what the caller
/// does next.** Rust demands [`std::panic::UnwindSafe`] here precisely to make you
/// consider state a panic may have left half-mutated, and `&mut WorkerState` is not
/// unwind-safe: its `Session` could be mid-mutation. The assertion is honest only
/// because the caller **discards that state instead of reusing it** — see the loop in
/// [`Worker::spawn`]. Reusing a panicked session would risk IR that is subtly wrong
/// rather than absent, which this project ranks as worse than a hang.
fn guard<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|payload| {
        // A panic payload is `Box<dyn Any>`; `panic!("literal")` boxes a `&str` and
        // `panic!("{x}")` boxes a `String`. Anything else is a payload we cannot read.
        if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "panicked with a payload HRW cannot read".to_owned()
        }
    })
}

/// The compile response for a compile that panicked.
///
/// # Why every stage carries the same note, and none is `Failed`
///
/// **HRW does not know which phase panicked.** `Outcome::Failed` is defined as *"the
/// pipeline stopped here"*, so attributing it to any particular stage would invent
/// exactly the control-flow claim finding C20 removed from four other sites — and
/// putting it on Parse, the obvious shortcut, is the most wrong of all, since Parse is
/// the one stage that demonstrably did run.
///
/// So every stage states its own absence with [`Stage::info`], which asserts nothing
/// about where the pipeline stopped. The panic text itself reaches the log through the
/// default panic hook and `OutputCapture`, which is where an unattributable fact
/// belongs — `CLAUDE.md`'s *"a log line describes what happened"*.
fn panicked_compile(path: PathBuf, note: &str) -> FromWorker {
    let mut stages = StageBundle::default();
    for kind in StageKind::COMPILATION {
        *stages.get_mut(*kind) = Stage::info(note.to_owned());
    }
    FromWorker::Compiled {
        path,
        model: None,
        stages,
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    }
}

/// Turn a caught panic into the answer its request is owed.
fn panic_response(reply: PanicReply, note: &str) -> Option<FromWorker> {
    match reply {
        PanicReply::Compiled(path) => Some(panicked_compile(path, note)),
        PanicReply::Libraries => Some(FromWorker::Libraries(Err(note.to_owned()))),
        PanicReply::DefTree(name) => Some(FromWorker::DefTree {
            name,
            result: Err(note.to_owned()),
        }),
        PanicReply::Simulated(path) => Some(FromWorker::Simulated {
            path,
            result: Err(note.to_owned()),
        }),
        PanicReply::Silent(done) => {
            if let Some(flag) = done {
                flag.store(true, std::sync::atomic::Ordering::SeqCst);
            }
            None
        }
    }
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
                    // **A panicking phase must not take the worker with it — 2026-08-25.**
                    //
                    // Until then there was no `catch_unwind` anywhere in this file, so
                    // a panic in any Rumoca phase unwound this thread and killed it.
                    // The request that caused it was never answered, the UI kept
                    // polling a channel nothing would ever write to, and the only
                    // signal was `send_failed` on the *next* send — so if Doug clicked
                    // nothing, nothing told him. A dead compiler and a slow one looked
                    // identical.
                    //
                    // **The state is REBUILT, not reused, and that is the point of the
                    // design rather than a detail.** Catching a panic leaves `Session`
                    // in whatever half-mutated condition the unwind produced; carrying
                    // it into the next compile risks IR that is subtly wrong rather
                    // than absent, which this project ranks as worse than a hang. So
                    // the panic is reported and the session is thrown away. The cost
                    // is one MSL reload on the next compile; the alternative is a
                    // fiction.
                    let reply = panic_reply(&msg);
                    // **Started here so a panic can be timed — 2026-08-26.** This entry
                    // used to hardcode `elapsed_secs: 0.0`, reporting a panic forty
                    // seconds into a compile as having happened at t=0. Every other log
                    // line's elapsed time is real, measured from the compile's own start
                    // inside `compile_target`; this one is emitted *outside* that scope,
                    // where no such clock exists, so it invented one. Found by night 5's
                    // sweep of the log machinery and left for Doug, because a log line's
                    // time is a claim.
                    //
                    // Dispatch is a few microseconds, so time since the message was
                    // dequeued is the compile's elapsed time for every purpose a reader
                    // has. **`depth: 0` stays**, and is not the same kind of guess: this
                    // is a worker-level event emitted outside every bracket, so top
                    // level is where it belongs. The brackets the panicking compile left
                    // open stay open, which is what actually happened.
                    let dequeued = std::time::Instant::now();
                    let handled = guard(|| state.handle(msg, &emit));
                    let response = match handled {
                        Ok(r) => r,
                        Err(note) => {
                            let note = format!("the compiler panicked: {note}");
                            emit(FromWorker::Log(LogEntry {
                                elapsed_secs: dequeued.elapsed().as_secs_f64(),
                                level: LogLevel::Error,
                                message: note.clone(),
                                depth: 0,
                            }));
                            state = WorkerState::new();
                            panic_response(reply, &note)
                        }
                    };
                    if let Some(response) = response {
                        if tx_res.send(response).is_err() {
                            break; // UI is gone (channel dropped), shut down
                        }
                        ctx.request_repaint();
                    }
                }
            })
            .expect("failed to spawn rumoca-worker thread");

        Worker {
            tx: tx_req,
            rx: rx_res,
            send_failed: false,
        }
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

/// Every `.mo` file of one source root, parsed, as `(uri, ast)` pairs — the shape
/// `parse_source_root_with_cache` returns and `replace_parsed_source_set` consumes.
type ParsedDocuments = Vec<(String, StoredDefinition)>;

/// The parsed documents of one library source root, reusing a previous parse of
/// the **same bytes** rather than reloading it.
///
/// # The problem this solves
///
/// `parse_source_root_with_cache` is an *on-disk* cache. Every call re-walks the
/// root, re-hashes every file's bytes, deserializes every parsed AST back out of
/// the artifact cache and re-validates the package layout. For the MSL that is
/// **2,553 files and ~4.0 s, and the test suite pays it ten times** — measured
/// 2026-08-21, `docs/ideas.md` #48, where the ten loads are 36 s of the gate.
///
/// Nothing in those ten loads had changed on disk between them. The disk cache
/// cannot know that: it is keyed on the inputs, so it correctly recomputes the key
/// each time, and the key is the cheap part.
///
/// # What it costs, measured rather than assumed
///
/// | | cost |
/// |---|---|
/// | full load | ~4.0 s — collect 0.42, hash 0.81, deserialize 2.18, validate 0.57 |
/// | memo hit | ~1.5 s — collect 0.42, hash 0.81, clone 0.30 |
///
/// So a hit saves ~2.5 s, and the **clone is affordable**: cloning all 2,553
/// documents costs ~0.30 s, against the ~2.75 s of deserialize-plus-validate it
/// avoids. That was the number the design turned on — `replace_parsed_source_set`
/// consumes the documents by value, so a memo *must* hand out a copy, and had the
/// copy cost what the deserialize costs there would have been nothing here to win.
///
/// # Why this cannot serve a stale parse
///
/// The memo is keyed on [`source_root_input_cache_key`] — **the same fingerprint
/// the artifact cache itself uses**, over the root's layout and every file's bytes.
/// A hit therefore means the disk cache would also have hit, on the same key, and
/// returned the same documents. Editing any library file changes the key and the
/// root is parsed again.
///
/// **Paying 1.2 s per load to verify that is the deliberate trade.** Keying on the
/// path alone, or on file mtimes, would save a further ~1.2 s per load and would be
/// a *guess* that nothing changed. Accuracy outranks performance here (`CLAUDE.md`),
/// and the guess is worth about 10 s across the whole gate.
///
/// # What it does NOT change
///
/// **Only the parse is shared. Every caller still gets its own `Session`.**
/// `load_libraries` builds a fresh `Session` on every call and always did; a test
/// that wants a virgin session still gets one, holding no specimen document and no
/// resolved state. That distinction is load-bearing — the notebook traces are
/// *defined* as the virgin-session value (`CLAUDE.md`, *Running things*), and this
/// memo leaves that untouched because parsing the same bytes twice yields the same
/// AST both times.
fn parsed_source_root(root: &Path) -> Result<ParsedDocuments, String> {
    let parse = |root: &Path| {
        parse_source_root_with_cache(root)
            .map(|parsed| parsed.documents)
            .map_err(|e| format!("{}: {e:#}", root.display()))
    };
    if !MEMO_ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
        return parse(root);
    }
    let fingerprint =
        source_root_input_cache_key(root).map_err(|e| format!("{}: {e:#}", root.display()))?;
    memoised_by_fingerprint(source_root_memo(), root, &fingerprint, parse)
}

fn source_root_memo() -> &'static Mutex<HashMap<PathBuf, (String, ParsedDocuments)>> {
    static MEMO: OnceLock<Mutex<HashMap<PathBuf, (String, ParsedDocuments)>>> = OnceLock::new();
    MEMO.get_or_init(|| Mutex::new(HashMap::new()))
}

static MEMO_ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Turn [`parsed_source_root`]'s memo **off for this process** and drop whatever it
/// is already holding, trading its speed back for the memory it retains.
///
/// # This is a memory bound, not a correctness escape
///
/// The memo is neither risky to keep nor risky to drop: it is keyed on the artifact
/// cache's own content fingerprint, so it can only ever serve a parse of the exact
/// bytes on disk. Disabling it changes nothing any caller sees — every load simply
/// pays the full cost, exactly as it did before the memo existed.
///
/// What it costs is **~326 MB of retained working set for the process lifetime** —
/// measured 2026-08-21 over four MSL loads: peak 522 MB without the memo against
/// 848 MB with it, at per-load times of ~3.9 s and ~1.5 s.
///
/// # Why disabling, rather than clearing between loads
///
/// **Clearing does not work, and the reason is worth stating because it is not
/// obvious.** Clearing *before* a load reclaims nothing, because the load
/// immediately re-populates the memo. Clearing *after* one is worse: a memoised load
/// briefly holds **two** copies of the documents — [`memoised_by_fingerprint`]
/// stores a clone and hands back the original — so a caller that memoises and then
/// clears pays the peak and keeps none of the benefit. Only never storing in the
/// first place actually bounds the process.
///
/// # Who should call it
///
/// **One caller, and only for the whole run.** `examples/fidelity_msl` rebuilds its
/// `WorkerState` every N models *specifically* to bound memory (`CLAUDE.md`,
/// *Running things*: "only process exit is a memory bound"), runs under a 3 GB
/// free-RAM watchdog, and has already lost models to memory limits. It loads the MSL
/// **rarely**, so the memo would win it ~2.5 s per rebuild while costing 326 MB
/// continuously — the opposite trade from the test suite and the app, which load
/// often and win ~48 s.
///
/// Call it once at startup. It is deliberately not reversible: a run that has
/// decided memory matters more than speed should not have that silently undone.
pub fn disable_parsed_source_root_memo() {
    MEMO_ENABLED.store(false, std::sync::atomic::Ordering::Relaxed);
    source_root_memo()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// The memo's bookkeeping, with the parse passed in.
///
/// **Split out from [`parsed_source_root`] so it can be tested without parsing
/// anything**, and that is not a stylistic preference — it was forced by a
/// measurement. The obvious test ("edit a file, demand a different answer") writes
/// bytes no artifact cache has seen, and **a cache miss costs ~21 s in
/// `maybe_prune_cache_after_write` no matter how small the file is** — measured
/// 2026-08-21 on a five-line model whose actual parse was 2 ms. A must-fire test
/// for a cache must be able to *miss*, so testing this against the real parser
/// would have put a 40 s test into a suite this whole item exists to shorten.
///
/// What is left here is the part HRW actually wrote: compare the fingerprint,
/// serve the stored value only on an exact match, re-parse otherwise. The real
/// wiring is covered separately by
/// `a_memoised_source_root_parse_returns_the_same_documents`, which is a hit and
/// costs 0.19 s.
fn memoised_by_fingerprint<T: Clone>(
    memo: &Mutex<HashMap<PathBuf, (String, T)>>,
    root: &Path,
    fingerprint: &str,
    parse: impl FnOnce(&Path) -> Result<T, String>,
) -> Result<T, String> {
    // **The lock is not held across the parse.** A parse takes seconds, and holding
    // this across one would serialise every caller behind the first. Two callers
    // racing a cold root both parse and both insert the same value, which is wasted
    // work and never a wrong answer.
    if let Some((stored, value)) = memo
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(root)
        && stored == fingerprint
    {
        return Ok(value.clone());
    }
    let value = parse(root)?;
    memo.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(root.to_path_buf(), (fingerprint.to_owned(), value.clone()));
    Ok(value)
}

/// Build a logging closure that wraps `emit` with elapsed-time tracking.
/// Both `compile()` and `simulate()` use the same pattern: a local closure
/// that attaches a timestamp to each log entry.
fn make_log<'a>(
    t0: &'a std::time::Instant,
    emit: &'a impl Fn(FromWorker),
) -> impl Fn(LogLevel, String) + 'a {
    // **Nesting is derived, not declared.** Every entry between a `StageStart` and
    // its `StageEnd` is inside that phase by definition, so the depth follows from
    // the stream and no caller can mis-state it. A `depth` argument on `log` would
    // be one more thing to keep true by hand, and the thing this whole discussion is
    // about is a log that stopped being true by hand.
    let depth = std::cell::Cell::new(0u8);
    move |level, msg| {
        // **A bracket must name something that exists — checked at every emit.**
        //
        // Placed here rather than only in a test because a test compiles *one*
        // specimen: it would catch an invented bracket on `SingleInertia` and miss
        // one that only appears on a model that fails at flatten, or is singular, or
        // has no `connect()`. This fires on **every bracket of every compile**, and
        // Doug runs a dev build, so it fires in his session rather than in CI.
        //
        // `debug_assert!` deliberately: a release build should not abort a compile
        // over a log defect, and the sweep runs release-ish profiles where the cost
        // of the check on every line is not worth paying. See
        // `bracket_names_a_real_phase` for what "exists" means and why "DAE pipeline"
        // did not.
        debug_assert!(
            !matches!(level, LogLevel::StageStart | LogLevel::StageEnd)
                || bracket_names_a_real_phase(&msg),
            "log bracket {msg:?} names no compiler phase and is on no allow-list. \
             A bracket claims that the named thing ran and that everything nested \
             inside belongs to it; an invented name makes both claims uncheckable, \
             which is what \"DAE pipeline\" did until 2026-08-04",
        );
        // `StageEnd` un-nests *before* it prints, so a phase's closing line sits at
        // the same indent as its opening line rather than one level in.
        if level == LogLevel::StageEnd {
            depth.set(depth.get().saturating_sub(1));
        }
        emit(FromWorker::Log(LogEntry {
            elapsed_secs: t0.elapsed().as_secs_f64(),
            level,
            message: msg,
            depth: depth.get(),
        }));
        if level == LogLevel::StageStart {
            depth.set(depth.get().saturating_add(1));
        }
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
            ToWorker::SetLibraries(roots) => {
                Some(FromWorker::Libraries(self.load_libraries(roots)))
            }
            ToWorker::Compile(path) => Some(self.compile(&path, emit)),
            ToWorker::LiveDebugConnections {
                path,
                model,
                trace,
                done,
            } => {
                self.live_debug_connections(&path, &model, &trace);
                // Signalled whatever happened, including a failed compile: a live
                // `Playback` that never learns the session ended leaves its controls
                // disabled for good, which is `docs/ideas.md` #74's defect in a new
                // place.
                done.store(true, std::sync::atomic::Ordering::Release);
                None
            }
            ToWorker::CompileLibraryModel(name) => Some(self.compile_model_by_name(&name, emit)),
            ToWorker::OpenDef(name) => Some(self.open_def(&name)),
            ToWorker::Simulate {
                path,
                model,
                t_end,
                is_library,
            } => {
                // `path` carries a qualified name when `is_library`; the lossy
                // conversion is exact for one, because that is where it came from.
                let name = path.to_string_lossy().to_string();
                let target = if is_library {
                    CompileTarget::Library(&name)
                } else {
                    CompileTarget::File(&path)
                };
                let result = self.simulate(target, &model, t_end, emit);
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
                        self.tracing_guard = Some(tracing::subscriber::set_default(subscriber));
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
        target: CompileTarget<'_>,
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
        // **Located exactly as `compile_target` locates it**, through the same
        // `locate_library_model`, rather than a second resolution path that could
        // disagree with the one the stage tabs used moments earlier.
        let Located {
            uri,
            source,
            qualified: given_qualified,
            ..
        } = match target {
            CompileTarget::File(p) => std::fs::read_to_string(p)
                .map(|source| Located {
                    uri: p.to_string_lossy().to_string(),
                    source,
                    qualified: None,
                    decl_line: None,
                })
                .map_err(|e| format!("read error: {e}"))?,
            CompileTarget::Library(name) => self.locate_library_model(name)?,
        };

        // **Only a specimen is registered as a document**, mirroring the guard in
        // `compile_target`: a library model's file already lives in a durable source
        // root, so adding it as a workspace document would have the session hold the
        // same file twice and removing it later would evict part of the library.
        //
        // For a specimen the remove/re-add is load-bearing — without it
        // `update_document` sees identical source text and short-circuits, returning
        // cached results, so armed breakpoints never fire and a source edit does not
        // take effect on re-simulate.
        if given_qualified.is_none() {
            self.session.remove_document(&uri);
            self.session.update_document(&uri, &source);
        }
        // Rumoca API: `qualify_model_name` turns a simple name like "BouncingBall"
        // into a fully-qualified name (for top-level models these are the same;
        // nested models differ). A **library** model was already named in full by the
        // caller, and re-deriving it from the declaring file's URI would be wrong for
        // a file declaring several classes — `Blocks/Continuous.mo` holds
        // `SecondOrder` among others, which is the same reason
        // `compile_model_by_name` exists.
        let qualified =
            given_qualified.unwrap_or_else(|| self.session.qualify_model_name(&uri, model));
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
        let report = self
            .session
            .compile_model_strict_reachable_uncached_with_recovery(&qualified);
        // Drain any tracing events that Rumoca emitted during compilation.
        drain_traces(&log);
        // **Closes with the name it opened with.** It read `"Compile (…ms)"` against a
        // `"Compile (for simulation)"` start until 2026-08-04, found by the
        // emit-time bracket check the moment it was added — on the *simulation* path,
        // which the log tests never walk because they compile and stop. A reader
        // scanning for where the compile ended was looking for a name that was never
        // printed, and the balance check saw one start and one end and was content.
        log(
            LogLevel::StageEnd,
            format!(
                "Compile (for simulation) ({:.1}ms)",
                t_stage.elapsed().as_secs_f64() * 1000.0,
            ),
        );
        // Pattern-match on the `PhaseResult` to extract the successful
        // `CompileResult` (which contains the `Dae`), or return an error.
        // The `?`-like early returns use `return Err(...)` because we're in
        // a `match` arm, not a `?`-compatible expression position.
        let cr = match report.requested_result.as_ref() {
            Some(PhaseResult::Success(cr)) => cr,
            Some(PhaseResult::Failed { phase, error, .. }) => {
                log(
                    LogLevel::Error,
                    format!("compile failed at {phase}: {error}"),
                );
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
        let sm = rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae).map_err(|e| {
            log(LogLevel::Error, format!("solve lowering failed: {e}"));
            format!("solve lowering failed: {e}")
        })?;
        drain_traces(&log);
        log(
            LogLevel::StageEnd,
            format!(
                "Solve lowering ({:.1}ms)",
                t_stage.elapsed().as_secs_f64() * 1000.0
            ),
        );

        // Check if the model has discrete updates (reinit / when-clause
        // assignments) that cause discontinuous jumps. This flag controls
        // whether the plot breaks its polylines at jumps (via
        // `discontinuity_segments`). A bare zero-crossing without an update
        // does NOT count — the trajectory is still continuous.
        let has_discontinuities =
            !cr.dae.discrete.real_updates.is_empty() || !cr.dae.discrete.valued_updates.is_empty();
        let n_states = cr.dae.variables.states.len();
        let n_eq = cr.dae.continuous.equations.len();
        log(
            LogLevel::Info,
            format!("{n_eq} equations, {n_states} states, hybrid={has_discontinuities}"),
        );

        // --- Phase 3: Integrate (run the ODE/DAE solver) ---
        // Rumoca API: `simulate_solve_model` runs the solver (Auto = BDF for
        // stiff / RK45 otherwise) from t=0 to t_end, returning time series.
        // `..Default::default()` fills the remaining `SimOptions` fields
        // (tolerances, max steps, output points) with sensible defaults.
        log(LogLevel::StageStart, "Integration".to_owned());
        let t_stage = Instant::now();
        let opts = rumoca_sim::SimOptions {
            t_end,
            ..Default::default()
        };
        // **On a failure that smells non-finite, name the variable — 2026-08-25.**
        //
        // `rumoca`'s CLI calls `simulate_with_diagnostics_auto_nan_trace`, which
        // re-runs with NaN tracing so the offending model variable is reported,
        // *"turning an opaque 'step size too small' into an actionable diagnostic"*.
        // HRW called the plain entry point and got the opaque half — so the learning
        // instrument had the worse diagnostic of the two. This is that pattern, not
        // that function: the diagnostics wrappers take a `&Dae` and HRW simulates a
        // lowered `SolveModel`, but `nan_trace` is publicly re-exported and the retry
        // is four lines.
        //
        // **The capture is what makes it useful.** `nan_trace` reports through
        // `eprintln!`, and unlike `compile_target` this function holds no
        // `OutputCapture` — so without one the trace lands in a terminal Doug is not
        // watching. It is started only for the retry, which happens only on a failure
        // already headed for the error path.
        let res = match rumoca_sim::simulate_solve_model(&sm, &opts) {
            Ok(res) => res,
            Err(e) => {
                let msg = format!("{e}");
                if !rumoca_sim::nan_trace::nan_trace_enabled()
                    && rumoca_sim::nan_trace::error_suggests_nonfinite(&msg)
                {
                    log(
                        LogLevel::Info,
                        "the error suggests a non-finite value \u{2014} re-running with \
                         NaN tracing to name the variable"
                            .to_owned(),
                    );
                    let mut capture = OutputCapture::start();
                    rumoca_sim::nan_trace::set_nan_trace(true);
                    let _ = rumoca_sim::simulate_solve_model(&sm, &opts);
                    rumoca_sim::nan_trace::set_nan_trace(false);
                    match capture.as_mut() {
                        Some(cap) => {
                            let (out, err) = cap.drain();
                            for line in out.lines().chain(err.lines()) {
                                if !line.is_empty() {
                                    log(LogLevel::Stderr, line.to_owned());
                                }
                            }
                        }
                        // Stating the absence rather than losing the trace quietly:
                        // without a capture the report went somewhere Doug cannot see,
                        // and a silent nothing here reads as "the trace found nothing".
                        None => log(
                            LogLevel::Info,
                            "stderr could not be captured, so the NaN trace went to the \
                             terminal rather than this log"
                                .to_owned(),
                        ),
                    }
                    drop(capture);
                }
                drain_traces(&log);
                log(LogLevel::Error, format!("simulation failed: {msg}"));
                return Err(format!("simulation failed: {msg}"));
            }
        };
        drain_traces(&log);
        log(
            LogLevel::StageEnd,
            format!(
                "Integration ({:.1}ms, {} time points)",
                t_stage.elapsed().as_secs_f64() * 1000.0,
                res.times.len(),
            ),
        );

        log(
            LogLevel::Info,
            format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0),
        );

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
            Err(e) => {
                return FromWorker::DefTree {
                    name: name.to_owned(),
                    result: Err(format!("{e:#}")),
                };
            }
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
        FromWorker::DefTree {
            name: name.to_owned(),
            result,
        }
    }

    /// The URIs of the `specimens/` documents **the session itself** currently holds.
    ///
    /// **Exists for `the_session_holds_at_most_one_specimen_document`**, which turns
    /// a doc-comment claim that had gone stale into a checked one.
    ///
    /// **Asks `Session::document_uris`, deliberately — NOT `last_specimen_uri`.**
    /// The first draft returned the tracker, which is circular: the tracker is
    /// HRW's *belief* about what is registered, so a test reading it would pass
    /// even if the session held every specimen ever compiled. The claim is about
    /// the session, so the session has to be the one asked.
    #[cfg(test)]
    fn specimen_document_uris(&self) -> Vec<String> {
        self.session
            .document_uris()
            .into_iter()
            .filter(|uri| uri.replace('\\', "/").contains("/specimens/"))
            .map(ToOwned::to_owned)
            .collect()
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
    ///
    /// **The parse is memoised per process — see [`parsed_source_root`].** A fresh
    /// `Session` is still built here every time, so nothing about what this returns
    /// changes; only the cost of getting the same documents twice does.
    pub fn load_libraries(&mut self, roots: Vec<PathBuf>) -> Result<usize, String> {
        let mut session = Session::new(SessionConfig::default());
        let mut total = 0usize;
        for root in &roots {
            let documents = parsed_source_root(root)?;
            let key = source_root_source_set_key(&root.to_string_lossy());
            total += session.replace_parsed_source_set(
                &key,
                SourceRootKind::DurableExternal,
                documents,
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
                format!(
                    "`{qualified}` is declared in `{uri}`, which the session has no document for"
                )
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

    /// Run a real compile with a **live** connection sink installed, so a debugger
    /// stopped on the live-trace anchor is stopped inside connection expansion.
    ///
    /// # What the reader is standing in when it stops
    ///
    /// `trace::start_live` hands each frame to a closure that calls
    /// `LiveTrace::push`, and `push` calls `live_trace_breakpoint`. So the stack at
    /// the stop is:
    ///
    /// ```text
    /// compile_model_strict_reachable_uncached_with_recovery
    ///   └ … flatten …
    ///       └ generate_connection_set_equations   <- Rumoca's algorithm
    ///           └ connections::trace::emit
    ///               └ (this closure)
    ///                   └ LiveTrace::push
    ///                       └ live_trace_breakpoint   <- the anchor
    /// ```
    ///
    /// Walking *up* from the anchor lands in Rumoca's own code with Rumoca's own
    /// locals — which is the point, and why `rumoca-phase-flatten` was given
    /// `opt-level = 0` before anyone tried it.
    ///
    /// # No re-run, unlike every other live-stepped view
    ///
    /// The others execute their algorithm a second time so there is something to
    /// step. This steps **the compilation**, because the live sink is ambient and the
    /// session lives on this thread.
    ///
    /// # The scope is closed on every path
    ///
    /// A sink left installed keeps firing on later compiles — the reader would hit a
    /// breakpoint during an ordinary specimen load with no session in flight. So
    /// `end_live` runs whether the compile succeeded, failed, or produced no model.
    fn live_debug_connections(
        &mut self,
        path: &Path,
        model: &str,
        trace: &rumoca_phase_structural::live_trace::LiveTrace<
            rumoca_phase_flatten::connections::trace::ConnectionFrame,
        >,
    ) {
        let Ok(source) = std::fs::read_to_string(path) else {
            return;
        };
        let uri = path.to_string_lossy().to_string();
        // Same reason as `compile`: an unchanged document short-circuits and the
        // phase code never re-executes, so nothing would be emitted and nothing
        // would stop.
        self.session.remove_document(&uri);
        self.session.update_document(&uri, &source);
        let qualified = self.session.qualify_model_name(&uri, model);

        // Gate the start, so the reader can arrive before the first frame does.
        // Identical in purpose to `LiveTrace::wait_for_debugger` on the spawned
        // threads: without it the pass can run to completion between the click and
        // the debugger attaching, and the session looks like it never happened.
        trace.wait_for_debugger();

        let sink = trace.clone();
        rumoca_phase_flatten::connections::trace::start_live(Box::new(
            move |frame: &rumoca_phase_flatten::connections::trace::ConnectionFrame| {
                sink.push(frame.clone());
            },
        ));
        let _ = self
            .session
            .compile_model_strict_reachable_uncached_with_recovery(&qualified);
        rumoca_phase_flatten::connections::trace::end_live();
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

    fn compile_target(
        &mut self,
        target: CompileTarget<'_>,
        emit: &impl Fn(FromWorker),
    ) -> FromWorker {
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
        let drain_output = |capture: &mut Option<OutputCapture>,
                            log_fn: &dyn Fn(LogLevel, String)| {
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
        let Located {
            uri,
            source,
            qualified: given_qualified,
            decl_line,
        } = match located {
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
                    matching_frames: Vec::new(),
                    tarjan_frames: Vec::new(),
                    tearing_frames: Vec::new(),
                    reduced_frames: StructuralFrames::default(),
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
                (
                    Stage::err_with_details(
                        serde_json::json!({
                            "kind": "parse",
                            "message": msg,
                            "guidance": "Check the Modelica source for syntax errors.",
                        }),
                        msg,
                    ),
                    model,
                )
            }
        };
        // After each Rumoca API call, drain any `tracing` events that were
        // buffered by our `TracingForwarder` subscriber.
        drain_traces(&log);
        drain_output(&mut output_capture, &log);
        log(
            LogLevel::StageEnd,
            format!("Parse ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0),
        );
        // Stream the first progress snapshot: Parse is done, everything else
        // is `Stage::default()` (neutral). `..Default::default()` fills the
        // remaining struct fields with their default values — a Rust pattern
        // called "struct update syntax".
        emit(FromWorker::CompileProgress {
            path: report_path.clone(),
            stages: StageBundle {
                parse: parse.clone(),
                ..Default::default()
            },
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
        // **Seeded with the not-reached note rather than left blank — 2026-08-25.**
        // These two are filled during typecheck when there is a model. When parse
        // produced none, nothing ever wrote to them and both rendered a **wholly blank
        // tab** — every other stage in that compile said why it was empty and these
        // two said nothing at all. Third instance of the same defect found the same
        // day, after `dae_absent_stage`'s `None` arm and Flatten's adopted DAE error.
        // Starting from the honest note means a stage can only *improve* on it.
        let unreached = Stage::info(not_reached_note("parse produced no model to compile"));
        let mut instantiate = unreached.clone();
        let mut typecheck = unreached;
        let mut connection_frames = Vec::new();
        let resolve = match &model {
            // `info`, not `err`: parse is where the pipeline stopped, and it already
            // says so. See `no_result_note` for why the outcome class is a claim.
            None => Stage::info(not_reached_note("parse produced no model to resolve")),
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
                // **`tree()`, not `resolved()`.** `resolved()` ends with a deep
                // clone of the whole resolved tree — measured at ~487ms on this
                // workspace, over 38,855 defs and 6,521 classes, which was the
                // single largest cost in a compile. `tree()` returns `&ClassTree`
                // from the same cached `Arc` and was already public; HRW was
                // simply calling the copying one. Everything below only reads.
                // **Resolve in the mode the compile will use.**
                //
                // `tree()` builds `ResolveBuildMode::Standard`; a strict compile
                // builds `StrictCompileRecovery`. Deliberately different trees,
                // cached separately — so HRW resolved the whole library twice per
                // compile (~374ms; two `resolve timing summary` traces, def_count
                // 38855 each). `strict_compile_resolved()` builds the one the
                // compile is about to want, so the compile below finds it cached.
                //
                // **Recovery succeeds past errors `Standard` treats as fatal**, so
                // `Ok` no longer means the model resolved. The diagnostics carry
                // that now — see `resolve_diagnostics_indicate_failure`, whose
                // predicate was measured rather than assumed.
                let strict = self.session.strict_compile_resolved();
                let strict = match strict {
                    Ok((rt, diags)) if resolve_diagnostics_indicate_failure(&diags) => {
                        // Recovered a tree, but this model did not resolve. Take the
                        // failure path, exactly as `tree()`'s `Err` used to.
                        let _ = rt;
                        Err(anyhow::anyhow!(
                            "Resolve errors: {}",
                            diags
                                .iter()
                                .filter(|d| d.severity == rumoca_core::DiagnosticSeverity::Error)
                                .map(|d| d.message.clone())
                                .collect::<Vec<_>>()
                                .join("; "),
                        ))
                    }
                    Ok((rt, _)) => Ok(rt),
                    Err(e) => Err(e),
                };
                match strict.as_deref() {
                    Ok(rt) => {
                        let rt = &rt.0;
                        // Extract just this model's class definition from the
                        // full resolved tree (which includes the entire MSL).
                        let stage = extract_class(rt, &qualified);
                        if let Some(v) = &stage.value {
                            // Build the DefId→DefInfo lookup for all DefIds
                            // referenced in this model's IR.
                            def_index = build_def_index(rt, v);
                        }
                        // **Resolve ends here**, before the phases that merely use
                        // its output. Its bracket used to close after Instantiate,
                        // Typecheck and the connection replay had all run inside it,
                        // so it reported 1428ms on SingleInertia of which 505ms
                        // belonged to three other things — and their traces drained
                        // under Resolve's name.
                        drain_traces(&log);
                        log(
                            LogLevel::StageEnd,
                            format!(
                                "Resolve ({:.1}ms)",
                                t_stage.elapsed().as_secs_f64() * 1000.0
                            ),
                        );

                        // **Instantiate and Typecheck are not run here any more**
                        // (2026-08-04). HRW used to run them itself for their two
                        // tabs, and the session's compile below ran them *again* on
                        // its way to the flat model — so the tabs showed one
                        // execution and everything downstream came from another.
                        // Their artifacts now come from the compile, through
                        // `rumoca_compile::observe`, and are turned into stages
                        // alongside Flatten and DAE construction.
                        // **The reset that used to sit here is gone — 2026-08-25.** It
                        // assigned `Stage::default()` to both, a leftover from when HRW
                        // ran them itself. The compile below writes them on every path
                        // it reaches, so the assignment was **dead** — except on a path
                        // that never reaches it, where its only effect was to replace
                        // the honest "not reached" seed at their declaration with a
                        // blank tab. It could only ever destroy information.
                        //
                        // **The connection replay is gone** (2026-08-04). Connection
                        // frames now arrive from the compile below, through
                        // `rumoca-phase-flatten`'s capture scope. Nothing happens
                        // here at all, which is the accurate amount of work for a
                        // step the chain does not have.
                        stage
                    }
                    Err(e) => {
                        // Resolve ends here too — and nothing downstream runs, so
                        // this is the last bracket of the compile.
                        drain_traces(&log);
                        log(
                            LogLevel::StageEnd,
                            format!(
                                "Resolve ({:.1}ms) \u{2014} failed",
                                t_stage.elapsed().as_secs_f64() * 1000.0
                            ),
                        );
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
                            &self
                                .session
                                .compile_model_diagnostics(&qualified)
                                .diagnostics,
                            &source,
                        );
                        let resolve_err = |note: &str| {
                            serde_json::json!({
                                "kind": "resolve",
                                "message": note,
                                "diagnostics": diag,
                                "guidance": "Name resolution binds every reference to a definition.                                 Read `diagnostics.errors` first: those are this model's problems,                                 each with the source location of the reference that failed.                                 `diagnostics.warnings` are library-level and almost never the                                 cause.",
                            })
                        };
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(&rt.0, &qualified) {
                                Stage {
                                    value: Some(mut v), ..
                                } => {
                                    def_index = build_def_index(&rt.0, &v);
                                    // **Both the recovered tree and the error.**
                                    //
                                    // This branch used to return `Stage::recovered(v,
                                    // note)`, dropping the structured diagnostics —
                                    // and it was almost never taken, because a failed
                                    // `Standard` resolve usually left no cached tree
                                    // to recover from. Under `StrictCompileRecovery`
                                    // it is the *normal* branch: recovery succeeds, so
                                    // the class extracts fine, so the diagnostics
                                    // vanished and the Resolve tab went quiet on a
                                    // real failure.
                                    //
                                    // Caught by `a_resolve_failure_names_the_reference_and_its_line`
                                    // — the second of the two tests that defeated the
                                    // first attempt at this change.
                                    if let Some(obj) = v.as_object_mut() {
                                        obj.insert("error".to_owned(), resolve_err(&note));
                                    }
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

        // Resolve's own bracket closed inside the match above, at the point
        // resolution actually finished. What is left here is a sweep for anything
        // the later phases emitted after their own drains — kept so nothing is
        // stranded, but no longer attributed to Resolve.
        drain_traces(&log);
        drain_output(&mut output_capture, &log);
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
        let (
            flatten,
            dae_stage,
            structural,
            index_reduction,
            initialization,
            events,
            solve_lowering,
            equation_sheet,
            identifier_index,
            ir_frames,
            compiled_dae,
            pre_frames,
            compiled_flat,
            structural_frames,
            reduced_frames,
        ) = match &model {
            None => {
                // **Seven `Stage::err`s until 2026-08-25, which is seven claims that
                // the pipeline stopped at seven different phases.** It stopped once,
                // at parse, and the parse tab already carries that error with its
                // payload. These seven never ran; `not_reached_note` is the wording
                // every other skipped stage uses, and routing through it also brings
                // them under `a_stage_that_says_it_never_ran_shows_no_ir`, which
                // matches on "not reached" and could not see them before.
                let e = not_reached_note("parse produced no model to compile");
                (
                    Stage::info(e.clone()),
                    Stage::info(e.clone()),
                    Stage::info(e.clone()),
                    Stage::info(e.clone()),
                    Stage::info(e.clone()),
                    Stage::info(e.clone()),
                    Stage::info(e),
                    None,
                    None,
                    Vec::new(),
                    None,
                    Vec::new(),
                    None,
                    StructuralFrames::default(),
                    StructuralFrames::default(),
                )
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
                // **Named for everything it runs, not just what HRW wants from it.**
                // The traces inside this bracket include resolve, instantiate and
                // typecheck, because `compile_model_strict_reachable_uncached_with_recovery`
                // re-runs the whole pipeline — HRW ran those phases itself earlier to
                // populate their tabs, and this call does them again on its way to the
                // flat model and the DAE. Calling it "flatten and DAE construction"
                // described what HRW *takes* from the call rather than what the call
                // *does*, which is the same species of inaccuracy as the replay
                // brackets.
                log(
                    LogLevel::StageStart,
                    "Rumoca compile \u{2014} full pipeline; HRW takes the flat model and DAE"
                        .to_owned(),
                );
                let t_compile = Instant::now();

                // **Capture the animation data from the run that actually happens.**
                //
                // Doug, 2026-08-04: *"I very much want to measure the compilation as
                // it actually happened rather than make use of replays … our ability
                // to play animations is tremendously valuable and I want to preserve
                // that. But I want to capture the data for those animations during
                // the actual compilation."*
                //
                // These scopes are the Rumoca-side addition that makes that possible
                // (`rumoca-phase-flatten`/`-dae`, 2026-08-04). Both replays are gone:
                // the Connections and pre()-lowering views now show the compilation
                // below, not a second one configured to look like it.
                rumoca_phase_flatten::connections::trace::start_capture();
                rumoca_phase_dae::start_pre_lowering_capture();
                // And the two overlays, so the Instantiate and Typecheck tabs show
                // this compile rather than a separate one HRW ran for them.
                rumoca_compile::observe::start_typed_model_capture();
                // Uncached, for the reason spelled out in `simulate` above: a
                // cached result means the phases did not run, so nothing can be
                // observed happening — breakpoints, tracing, or timing.
                let report = self
                    .session
                    .compile_model_strict_reachable_uncached_with_recovery(&qualified);
                // Taken immediately, before anything else can run: `take_capture`
                // closes the scope, so this both collects the frames and guarantees
                // no later work can add to them.
                connection_frames = rumoca_phase_flatten::connections::trace::take_capture();
                let pre_frames = rumoca_phase_dae::take_pre_lowering_capture();
                let typed = rumoca_compile::observe::take_typed_model_capture();

                // **Route each trace to the phase that emitted it.**
                //
                // Doug, 2026-08-04: *"I still do not see rumoca trace log lines for
                // phases such as Instantiate which are contained within the rumoca
                // compile phase."* They were being emitted — as one block here,
                // because `drain_traces` empties the buffer to wherever the log
                // currently is, and all four phases run inside this one call.
                //
                // Explaining that (the notice below, 2026-08-04) did not answer the
                // ask: he wants the lines under the phase, not a sentence about where
                // they went. **Splitting them was declined earlier on the grounds that
                // it would "present an interleaving that did not happen", and that
                // objection was weaker than it sounded** — the compile runs these
                // phases *sequentially*, so partitioning by target keeps each phase's
                // events in their emitted order and merely cuts the block at the
                // boundaries it already had.
                //
                // What survives of the objection, and is honoured: a line is filed
                // only under the phase **it names itself**. `rumoca_eval_flat` runs
                // during flatten but says so nowhere, so it stays in this bracket
                // rather than being attributed by proximity.
                let traces = take_traces();
                let (instantiate_traces, traces) =
                    attribute_traces(traces, "rumoca_phase_instantiate");
                let (typecheck_traces, traces) = attribute_traces(traces, "rumoca_phase_typecheck");
                let (flatten_traces, traces) = attribute_traces(traces, "rumoca_phase_flatten");
                let (dae_traces, unattributed) = attribute_traces(traces, "rumoca_phase_dae");
                let n_attributed = instantiate_traces.len()
                    + typecheck_traces.len()
                    + flatten_traces.len()
                    + dae_traces.len();
                // Emitted here because nothing claimed them. This is the honest
                // remainder, not a leftover to be tidied away.
                for (level, msg) in unattributed {
                    log(level, msg);
                }
                let replay = |traces: CapturedTraces| {
                    for (level, msg) in traces {
                        log(level, msg);
                    }
                };
                drain_output(&mut output_capture, &log);
                // **The call's own duration, captured where the call returns.**
                //
                // The bracket does *not* close here. It stays open across the four
                // extractions below, so Instantiate, Typecheck, Flatten and DAE
                // construction render nested inside it — which is the truth: those
                // phases ran inside this call, and the entries below are HRW reading
                // out what they produced. Doug, 2026-08-04, asking for exactly this:
                // *"let's figure out how to show the log lines for Instantiate,
                // Typecheck, Flatten and DAE construction nested within the log lines
                // for Rumoca compile."*
                //
                // Keeping the number from *here* is what makes the nesting honest.
                // A bracket that simply closed later would report the call plus the
                // extractions as one figure, quietly inflating the compile by the cost
                // of looking at it. Both numbers are reported instead.
                let compile_call_ms = t_compile.elapsed().as_secs_f64() * 1000.0;
                // **Say which of the timings below are real.** Flatten and DAE
                // construction ran inside the call that just ended, so their
                // stage entries time an *extraction* — `DAE construction (0.1ms)`
                // would otherwise read as the phase being nearly free when it is
                // part of a second-long call. Same family as the "DAE pipeline"
                // fiction removed 2026-08-04: a log that states a number the
                // reader will misread is not reporting, it is misreporting.
                // **Says where the trace output is, not only where the work was.**
                //
                // Doug, 2026-08-04: *"when I check the Rumoca tracing checkbox, some
                // phases show rumoca trace log lines and some phases do not."*
                // Measured on `SingleInertia`: **all 44 traces sit in this bracket**,
                // and Instantiate, Typecheck, Flatten and DAE construction show none.
                //
                // That is correct — those phases ran here, so their events were
                // emitted and drained here, and the entries below emit nothing
                // because reading a captured artifact is not running a phase. But the
                // previous wording said only that the *work* happened inside the
                // call, which left the empty brackets looking like missing
                // instrumentation. **The reader was left to infer the one thing the
                // sentence existed to tell them.**
                //
                // **Superseded 2026-08-04 (same day):** the notice used to say the
                // traces were "the block above". They now sit under the phase that
                // named itself, so the sentence describes the routing and what is
                // left behind by it.
                log(
                    LogLevel::Info,
                    format!(
                        "Instantiate, Typecheck, Flatten and DAE construction all ran \
                         inside that call \u{2014} their entries below time HRW reading \
                         out what each produced, not the phase itself. With tracing on, \
                         {n_attributed} trace line(s) from that call are filed under the \
                         phase each one names; lines above name a helper crate rather \
                         than a phase, so they stay here rather than be attributed by \
                         proximity. Structural onward is real work and traces under its \
                         own name."
                    ),
                );

                // **Instantiate and Typecheck, extracted from the compile above.**
                //
                // Bracketed here rather than before it, because here is where HRW
                // does the work — turning a captured overlay into a stage. The
                // phases themselves ran inside the call, which the Info line below
                // says. Timing them where they *appear* rather than where they
                // *ran* was the last piece of the same inaccuracy.
                let t_inst = Instant::now();
                log(LogLevel::StageStart, "Instantiate".to_owned());
                replay(instantiate_traces);
                instantiate = match &typed.instantiated {
                    Some(o) => Stage::from_ser(o),
                    None => Stage::info(not_reached_note("the compile stopped before instantiate")),
                };
                log(
                    LogLevel::StageEnd,
                    format!(
                        "Instantiate ({:.1}ms)",
                        t_inst.elapsed().as_secs_f64() * 1000.0
                    ),
                );

                let t_tc = Instant::now();
                log(LogLevel::StageStart, "Typecheck".to_owned());
                replay(typecheck_traces);
                typecheck = match (&typed.typechecked, typed.typecheck_diagnostics.is_empty()) {
                    (Some(o), _) => Stage::from_ser(o),
                    // Typecheck failed: the diagnostics are the artifact, and the
                    // instantiated overlay is the last good state to show beside them.
                    (None, false) => {
                        let mut json = typed
                            .instantiated
                            .as_ref()
                            .map(ser_value)
                            .unwrap_or_else(|| serde_json::json!({}));
                        let n = typed.typecheck_diagnostics.len();
                        let diag_json = diagnostics_to_json(&typed.typecheck_diagnostics, &source);
                        json.as_object_mut().unwrap().insert("error".to_owned(), serde_json::json!({
                            "kind": "typecheck",
                            "message": format!("Typecheck reported {n} diagnostic(s)"),
                            "diagnostics": diag_json,
                            "guidance": "Typecheck validates types, dimensions, and units across \
                                the instantiated model. The overlay above is partial \u{2014} it \
                                reflects work completed before the error.",
                        }));
                        Stage::recovered(json, format!("typecheck: {n} diagnostic(s)"))
                    }
                    (None, true) => {
                        Stage::info(not_reached_note("the compile stopped before typecheck"))
                    }
                };
                log(
                    LogLevel::StageEnd,
                    format!("Typecheck ({:.1}ms)", t_tc.elapsed().as_secs_f64() * 1000.0),
                );

                let result = report.requested_result.as_ref();

                let eq_sheet = match result {
                    Some(PhaseResult::Success(cr)) => {
                        let mut sheet =
                            crate::equation_sheet::build(&cr.dae, Some((&uri, display_source)));
                        // **Filled here because this is where both halves exist.**
                        // `build` sees only the DAE; the flat model is on the compile
                        // result beside it, and the source is already resolved for
                        // both. Computing it later in the app would mean re-deriving
                        // the source text a second time.
                        sheet.flat_node_lines = crate::equation_sheet::flat_node_lines(
                            &cr.flat,
                            Some((&uri, display_source)),
                        );
                        Some(sheet)
                    }
                    _ => None,
                };

                let id_index = match result {
                    Some(PhaseResult::Success(cr)) => {
                        Some(crate::identifier_index::IdentifierIndex::build(
                            &cr.dae,
                            &uri,
                            display_source,
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
                //
                // The four-argument form takes traces **already captured** from the
                // compile call above and replays them inside the bracket. Flatten and
                // DAE construction need it because they ran in that call; every stage
                // after them runs here and drains its own, so they pass nothing.
                macro_rules! run_stage {
                    ($name:expr, $extract:expr, $field:ident) => {
                        run_stage!($name, $extract, $field, Vec::new())
                    };
                    ($name:expr, $extract:expr, $field:ident, $captured:expr) => {{
                        log(LogLevel::StageStart, $name.to_owned());
                        // Before the clock starts: these were emitted during the
                        // compile call, so charging them to the extraction would be a
                        // second small fiction of the kind this bracket exists to end.
                        replay($captured);
                        let t = Instant::now();
                        let stage = $extract;
                        drain_traces(&log);
                        log(
                            LogLevel::StageEnd,
                            format!("{} ({:.1}ms)", $name, t.elapsed().as_secs_f64() * 1000.0),
                        );
                        bundle.$field = stage.clone();
                        emit(FromWorker::CompileProgress {
                            path: report_path.clone(),
                            stages: bundle.clone(),
                        });
                        stage
                    }};
                }

                let flatten = run_stage!(
                    "Flatten",
                    flatten_stage(result, &source),
                    flatten,
                    flatten_traces
                );

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
                    dae,
                    dae_traces
                );

                // **Close the compile bracket here**, after the four extractions it
                // contains. Two numbers, because they answer different questions:
                // what the compile cost, and what looking at it cost. Reporting only
                // their sum would let the price of observation hide inside the price
                // of compiling.
                log(
                    LogLevel::StageEnd,
                    format!(
                        "Rumoca compile ({compile_call_ms:.1}ms; +{:.1}ms reading its \
                         artifacts into views)",
                        t_compile.elapsed().as_secs_f64() * 1000.0 - compile_call_ms,
                    ),
                );

                // Structural onward is HRW's own work on the DAE, not a reading of
                // the compile's output — so it sits outside the bracket.
                let (structural, structural_frames) = {
                    log(LogLevel::StageStart, "Structural analysis".to_owned());
                    let t = Instant::now();
                    let (stage, frames) = structural_stage(result, &source);
                    drain_traces(&log);
                    log(
                        LogLevel::StageEnd,
                        format!(
                            "Structural analysis ({:.1}ms)",
                            t.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                    bundle.structural = stage.clone();
                    emit(FromWorker::CompileProgress {
                        path: report_path.clone(),
                        stages: bundle.clone(),
                    });
                    (stage, frames)
                };
                let (index_reduction, ir_frames, reduced_frames) = {
                    log(LogLevel::StageStart, "Index reduction".to_owned());
                    let t = Instant::now();
                    let (stage, frames, reduced) = index_reduction_stage(result, &source);
                    drain_traces(&log);
                    log(
                        LogLevel::StageEnd,
                        format!(
                            "Index reduction ({:.1}ms)",
                            t.elapsed().as_secs_f64() * 1000.0
                        ),
                    );
                    bundle.index_reduction = stage.clone();
                    emit(FromWorker::CompileProgress {
                        path: report_path.clone(),
                        stages: bundle.clone(),
                    });
                    (stage, frames, reduced)
                };
                let initialization = run_stage!(
                    "Initialization",
                    initialization_stage(result),
                    initialization
                );
                let events = run_stage!("Events", events_stage(result), events);
                let solve_lowering = run_stage!(
                    "Solve lowering",
                    solve_lowering_stage(result),
                    solve_lowering
                );

                // **The pre()-lowering replay is gone** (2026-08-04, idea #40's
                // frames kept, its replay removed). It re-ran the whole of DAE
                // construction over the flat model to capture the pass's frames;
                // they now come from the compile that produced the DAE every other
                // stage shows, through `rumoca-phase-dae`'s capture scope. The
                // flat model is still cloned, because the Flatten view needs it.
                let flat = match result {
                    Some(PhaseResult::Success(cr)) => Some(cr.flat.clone()),
                    _ => None,
                };

                (
                    flatten,
                    dae_stage,
                    structural,
                    index_reduction,
                    initialization,
                    events,
                    solve_lowering,
                    eq_sheet,
                    id_index,
                    ir_frames,
                    dae,
                    pre_frames,
                    flat,
                    structural_frames,
                    reduced_frames,
                )
            }
        };

        // Restore stdout/stderr by dropping the OutputCapture.
        // `output_capture.take()` moves the value out of the `Option`,
        // returning `Some(capture)`, and `drop()` runs its `Drop` impl
        // which restores the original file descriptors via `dup2`.
        drop(output_capture.take());
        log(
            LogLevel::Info,
            format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0),
        );

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
            matching_frames: structural_frames.matching,
            tarjan_frames: structural_frames.tarjan,
            tearing_frames: structural_frames.tearing,
            reduced_frames,
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
    state.simulate(
        CompileTarget::File(specimen),
        model,
        t_end,
        &|_: FromWorker| {},
    )
}

/// Simulate a model from a **loaded library** headlessly, by qualified name.
///
/// The counterpart of [`compile_specimen`]/[`simulate_specimen`] for the corpus, and
/// the headless half of the fix for Doug's 2026-08-04 report that pressing Run on
/// `Modelica.Blocks.Continuous.SecondOrder` produced *"read error: The system cannot
/// find the file specified"*. It exists so the library path is **testable without a
/// UI**, which is why that path went unexercised long enough for the gap to survive.
pub fn simulate_library_model(
    qualified: &str,
    t_end: f64,
    libraries: Vec<PathBuf>,
) -> Result<SimData, String> {
    let mut state = WorkerState::new();
    state.load_libraries(libraries)?;
    // The simple name is the last segment; `locate_library_model` supplies the
    // qualified one, so this is only what the log calls the model.
    let simple = qualified.rsplit('.').next().unwrap_or(qualified).to_owned();
    state.simulate(
        CompileTarget::Library(qualified),
        &simple,
        t_end,
        &|_: FromWorker| {},
    )
}

/// If the pipeline result is a non-success variant (failed, needs inner, or
/// absent), return the appropriate placeholder Stage. Returns `None` when
/// the result is `Success` — the caller should handle that case.
fn not_reached_stage(result: Option<&PhaseResult>) -> Option<Stage> {
    match result {
        Some(PhaseResult::Success(_)) => None,
        Some(PhaseResult::Failed { phase, .. }) => Some(Stage::info(not_reached_note(&format!(
            "{phase} failed earlier"
        )))),
        Some(PhaseResult::NeedsInner { .. }) => Some(Stage::info(not_reached_note(
            "model needs inner declarations",
        ))),
        // `info`, not `err`: see `no_result_note` for why the outcome class here is
        // a claim this stage is not entitled to make.
        None => Some(Stage::info(no_result_note())),
    }
}

/// **"This stage never ran, and here is why"** — the one place that sentence is worded.
///
/// # Why it is a function and not five string literals
///
/// It was five, and three of them rebuilt this exact text by hand:
/// [`not_reached_stage`], `flatten_stage` and `dae_absent_stage`, plus two inline
/// variants naming a different stopping point. They agreed **by coincidence**, and one
/// thing already depended on them agreeing:
/// `fidelity::tests::a_stage_that_says_it_never_ran_shows_no_ir` finds a stage that
/// never ran by matching **`"not reached"`**. Reword the helper and three copies keep
/// the old text; reword a copy and the checker silently stops seeing that stage. A
/// guard whose premise is maintained by hand in five places is a guard with a slow leak.
///
/// # Why the distinction it carries is worth single-sourcing
///
/// *Never ran* and *ran and found none* are different facts, and the note is the only
/// thing that separates them. *"Index reduction: nothing to reduce"* is a claim about
/// the model; *"Index reduction: not reached"* is a claim about the pipeline, and a
/// reader given the first when the second is true learns something false about their
/// own model. Five spellings of one claim is how that distinction erodes.
///
/// **The `because` is free text on purpose.** The stopping point differs — a failed
/// phase, a compile that stopped before instantiate — and those are genuinely different
/// facts. What must not differ is the words that say *it did not run*.
fn not_reached_note(because: &str) -> String {
    format!("not reached ({because})")
}

/// **The pipeline returned nothing at all** — worded once, for the same reason.
///
/// Distinct from [`not_reached_note`]: that one says a phase was skipped because
/// something earlier stopped, this one says the reachable-closure pipeline produced no
/// result to skip *from*. `a_stage_that_says_it_never_ran_shows_no_ir` matches on
/// `"produced no result"`, so this string is load-bearing too.
///
/// # Why its callers use `Stage::info` and not `Stage::err` (finding C20)
///
/// **[`Outcome`] is a claim about control flow, not a severity.** Its own
/// documentation says `Failed` means *"the pipeline stopped here"* and `Flagged`
/// means *"the pipeline continued"*. Until 2026-08-25 both callers used
/// [`Stage::err`], so a model whose report carries no result for the requested
/// model — `MissingComponentClass`, `UndefinedRef`, `UnclosedModel` — painted
/// **six** stage tabs `Failed`. The pipeline stops once. At most one of those six
/// could be true, and none was: the neutral notes on Instantiate and Typecheck
/// locate the stop *before instantiate*.
///
/// **Only the outcome class changed; the wording did not.** That is the ordering of
/// the charter's principles doing real work. Consistency, applied first, would have
/// unified these two messages into one — destroying a true distinction while leaving
/// all six false control-flow claims in place. Accuracy points at the class and
/// leaves the words alone.
///
/// **The `None` is Rumoca's, not HRW's** — it comes from
/// `report.requested_result`, so "produced no result for this model" is a fact about
/// the compiler's output. There is no argument that red is warranted to flag an
/// HRW-side malfunction, which is the reading that would have justified `err`.
fn no_result_note() -> &'static str {
    "the reachable-closure pipeline produced no result for this model"
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
fn model_diagnostics_to_json(diags: &[rumoca_core::Diagnostic], source: &str) -> serde_json::Value {
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
        obj.insert(
            "diagnostics".to_owned(),
            diagnostics_to_json(diagnostics, source),
        );
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
    Some((
        eq.trim().parse().ok()?,
        unk.trim().parse().ok()?,
        bal.trim().parse().ok()?,
    ))
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
    let line_start = bytes[..start]
        .iter()
        .rposition(|&b| b == b'\n')
        .map_or(0, |i| i + 1);
    let line_end = bytes[end..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(bytes.len(), |i| end + i);
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

/// Frames captured from one structural run, for the three animations it feeds.
///
/// **A compile produces two of these**, over two different systems: the raw DAE
/// (the Structural tab) and the index-reduced one (the Index Reduction tab). Keeping
/// them apart is not tidiness — on `Drivetrain` the two differ by 97 equations
/// against 20, so frames from one would address rows the other does not have.
#[derive(Default, Clone)]
pub struct StructuralFrames {
    pub matching: Vec<rumoca_phase_structural::matching::MatchingFrame>,
    pub tarjan: Vec<rumoca_phase_structural::tarjan::TarjanFrame>,
    /// Tearing decisions, **one segment per coupled block**, in the order the
    /// blocks were torn — a flat list would splice several loops into one replay.
    pub tearing: Vec<Vec<rumoca_phase_structural::tearing::TearingFrame>>,
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
/// This pattern appears in every `*_stage()` function. The match arms
/// are exhaustive (Rust enforces this), so if Rumoca adds a new `PhaseResult`
/// variant, every extraction function will get a compile error until updated.
///
/// Rumoca API: `build_structural_report(&dae)` runs maximum matching +
/// BLT decomposition. `build_incidence(&dae)` builds the equation x unknown
/// bipartite adjacency matrix (which equations reference which unknowns).
fn structural_stage(result: Option<&PhaseResult>, source: &str) -> (Stage, StructuralFrames) {
    let empty = || StructuralFrames {
        matching: Vec::new(),
        tarjan: Vec::new(),
        tearing: Vec::new(),
    };
    if let Some(stage) = not_reached_stage(result) {
        return (stage, empty());
    }
    let cr = unwrap_success(result);

    // **The animation's frames come from this run.** Until 2026-08-04 the matching
    // animation re-ran `maximum_matching` on the incidence matrix when its tab was
    // opened — deterministic, so it agreed, but it agreed by luck of the algorithm
    // and described an execution that produced nothing. The frames now come from
    // the matching that produced the blocks below.
    //
    // Opened around the *singular* path too, deliberately: a model that fails to
    // match is the one whose search is most worth watching, and
    // `build_structural_report` runs matching before it decides to fail.
    rumoca_phase_structural::matching::start_capture();
    rumoca_phase_structural::tarjan::start_capture();
    rumoca_phase_structural::tearing::start_capture();
    let report = rumoca_phase_structural::build_structural_report(&cr.dae);
    let frames = StructuralFrames {
        matching: rumoca_phase_structural::matching::take_capture(),
        tarjan: rumoca_phase_structural::tarjan::take_capture(),
        tearing: rumoca_phase_structural::tearing::take_capture(),
    };
    let stage = match report {
        Ok(rep) => {
            let inc = rumoca_phase_structural::build_incidence(&cr.dae);
            let mut json = structural_to_json(&rep);
            json.as_object_mut().unwrap().insert(
                "incidence".to_owned(),
                incidence_to_json(&inc, Some(&cr.dae)),
            );
            Stage::ok(json)
        }
        Err(e) => {
            let inc = rumoca_phase_structural::build_incidence(&cr.dae);
            let (match_eq, _) = rumoca_phase_structural::matching::maximum_matching(
                inc.n_eq,
                inc.n_var,
                &inc.eq_unknowns,
            );
            let matching_json = partial_matching_to_json(&inc, &match_eq, &cr.dae);
            let mut json = serde_json::json!({});
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "incidence".to_owned(),
                incidence_to_json(&inc, Some(&cr.dae)),
            );
            obj.insert("matching".to_owned(), matching_json);
            obj.insert("error".to_owned(), structural_error_to_json(&e, source));
            let note = match &e {
                rumoca_phase_structural::StructuralError::Singular { .. } => "singular".to_owned(),
                _ => format!("{e}"),
            };
            Stage::recovered(json, note)
        }
    };
    (stage, frames)
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
) -> (
    Stage,
    Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>,
    StructuralFrames,
) {
    if let Some(stage) = not_reached_stage(result) {
        return (stage, Vec::new(), StructuralFrames::default());
    }
    let cr = unwrap_success(result);
    let raw_ok = rumoca_phase_structural::build_structural_report(&cr.dae).is_ok();
    let before_inc = rumoca_phase_structural::build_incidence(&cr.dae);
    let (before_match_eq, _) = rumoca_phase_structural::matching::maximum_matching(
        before_inc.n_eq,
        before_inc.n_var,
        &before_inc.eq_unknowns,
    );
    let mut reduced = cr.dae.clone();
    let (reduction, frames) = index_reduce_for_structural_analysis(&mut reduced);

    // **Capture the reduced system's own structural run.**
    //
    // This tab shows matching, Tarjan and tearing over the *reduced* DAE — a
    // different system from the Structural tab's, by 97 equations against 20 on
    // `Drivetrain`. Its animations were the last place still re-deriving, and the
    // reason the captured-frames constructors needed a fallback at all: there was
    // simply no capture for this system to offer them.
    //
    // Opened **here**, after the `raw_ok` probe above, and not before: that probe
    // runs a full `build_structural_report` on the *raw* DAE, and tearing's capture
    // appends a segment per loop rather than overwriting — so an earlier scope would
    // splice the raw system's loops into this one's.
    rumoca_phase_structural::matching::start_capture();
    rumoca_phase_structural::tarjan::start_capture();
    rumoca_phase_structural::tearing::start_capture();
    let report = rumoca_phase_structural::build_structural_report(&reduced);
    let reduced_frames = StructuralFrames {
        matching: rumoca_phase_structural::matching::take_capture(),
        tarjan: rumoca_phase_structural::tarjan::take_capture(),
        tearing: rumoca_phase_structural::tearing::take_capture(),
    };
    match report {
        Ok(rep) => {
            let inc = rumoca_phase_structural::build_incidence(&reduced);
            let note = if raw_ok {
                "already index-1 — the reduction funnel is a no-op here (same as the Structural tab)"
            } else {
                "index-reduced from a structurally singular (high-index) system — now solvable"
            };
            let mut json = structural_to_json(&rep);
            let obj = json
                .as_object_mut()
                .expect("structural_to_json returns an object");
            obj.insert(
                "incidence".to_owned(),
                incidence_to_json(&inc, Some(&reduced)),
            );
            obj.insert(
                "before".to_owned(),
                before_report_json(&before_inc, &before_match_eq, Some(&cr.dae)),
            );
            obj.insert("reduction".to_owned(), reduction.to_json());
            (Stage::ok_with_note(json, note), frames, reduced_frames)
        }
        Err(e) => {
            let msg = format!("{e}");
            let mut json = serde_json::json!({});
            let obj = json.as_object_mut().unwrap();
            obj.insert(
                "incidence".to_owned(),
                incidence_to_json(
                    &rumoca_phase_structural::build_incidence(&reduced),
                    Some(&reduced),
                ),
            );
            obj.insert(
                "before".to_owned(),
                before_report_json(&before_inc, &before_match_eq, Some(&cr.dae)),
            );
            obj.insert("reduction".to_owned(), reduction.to_json());
            obj.insert("error".to_owned(), structural_error_to_json(&e, source));
            (
                Stage::recovered(json, format!("still singular after index reduction: {msg}")),
                frames,
                reduced_frames,
            )
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
                Stage::ok_with_note(
                    json,
                    "no algebraic initialization subsystem (equations ≤ states)",
                )
            } else {
                Stage::ok(json)
            }
        }
        Err(e) => {
            let msg = format!("{e}");
            let mut error_json = match &e {
                rumoca_phase_structural::StructuralError::Singular {
                    n_equations,
                    n_unknowns,
                    n_matched,
                    unmatched_equations,
                    unmatched_unknowns,
                    ..
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
            error_json
                .as_object_mut()
                .unwrap()
                .insert("determinacy".to_owned(), determinacy.clone());
            let mut json = serde_json::json!({ "error": error_json });
            json.as_object_mut()
                .unwrap()
                .insert("determinacy".to_owned(), determinacy);
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
            IcBlock::ScalarDirect {
                var_name,
                solution_expr,
                ..
            } => serde_json::json!({
                "kind": "scalar_direct",
                "var": var_name,
                "solution": ser_value(solution_expr),
            }),
            IcBlock::ScalarNewton {
                var_name, eq_idx, ..
            } => serde_json::json!({
                "kind": "scalar_newton",
                "var": var_name,
                "equation": eq_idx,
            }),
            IcBlock::TornBlock {
                tear_var_names,
                causal_sequence,
                residual_eq_indices,
                ..
            } => {
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
            IcBlock::CoupledLM {
                eq_indices,
                var_names,
                ..
            } => serde_json::json!({
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
    // **"Smooth" is a claim about the model, so it is only made when it was read.**
    //
    // Found by the 2026-08-04 sweep. This was `.unwrap_or(0)` followed by
    // `if total == 0 { "no events — this model is a smooth (continuous)
    // system" }` — so a summary that could not be read as an object produced the
    // number zero, and zero produced a **positive physical assertion about Doug's
    // model**. A model full of `when` clauses would have been labelled smooth, and
    // nothing on screen would have hinted that the label came from a failure to read
    // rather than from the compiler.
    //
    // The three cases are now distinct: countable and zero (smooth), countable and
    // non-zero (events), and *not countable* (say so). See the accuracy rule at the
    // top of `CLAUDE.md` — absence is stated, never filled.
    // Counted before the `match` so the borrow of `json` ends here — the arms below
    // move it into a `Stage`.
    // **The entries have to be countable too, not just the summary.**
    //
    // The first version of this fix (earlier the same day) replaced `.unwrap_or(0)`
    // on the *object* but left `filter_map(as_u64)` on its *values* — so a summary
    // that was present but held a non-numeric count still summed to zero, and zero
    // still produced the smoothness claim. The sweep's own audit of `filter_map`
    // sites caught it, three commits after the fix that was supposed to close this.
    let counted: Option<(u64, usize, usize)> = json["summary"].as_object().map(|s| {
        let mut total: u64 = 0;
        let mut uncountable = 0usize;
        for v in s.values() {
            match v.as_u64() {
                Some(n) => total += n,
                None => uncountable += 1,
            }
        }
        (total, uncountable, s.len())
    });

    match counted {
        Some((_, uncountable, n)) if uncountable > 0 => Stage::recovered(
            json,
            format!(
                "{uncountable} of {n} event counts could not be read, so HRW cannot \
                 say whether this model has events \u{2014} the tree below is the raw \
                 event IR",
            ),
        ),
        Some((0, _, _)) => Stage::ok_with_note(
            json,
            "no events \u{2014} this model is a smooth (continuous) system",
        ),
        Some(_) => Stage::ok(json),
        None => Stage::recovered(
            json,
            "the event summary could not be read, so HRW cannot say whether this \
             model has events \u{2014} the tree below is the raw event IR",
        ),
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
            .map(|(e, u)| serde_json::json!({
                "equation": e,
                // The cross-pane identity, so pointing at a matching row resolves to
                // the same object as pointing at an incidence cell. Derived rather
                // than carried, because `StructuralReport::matching` keeps labels
                // only — see `equation_id_from_label`.
                "id": crate::equation_id_from_label(e),
                "unknown": u,
            }))
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
                // **The bare reference, as the cross-pane identity.**
                //
                // `equation` above is `equation_label`'s output — `"f_x[4] (equation
                // from R)"` — and matching/BLT lookups correlate by that exact string,
                // so it cannot change. But the equation *sheet* names the same equation
                // `"f_x[4]"`, so a reader pointing at an incidence cell and asking
                // "why is **this** equation…" produced an id that matched nothing in
                // the other pane. Doug hit that on 2026-08-15 the first time he used
                // deixis for real.
                //
                // Emitted here rather than recovered by splitting the label, because
                // the reference **is** the identity and the label is a decoration of
                // it. Publishing the writer's own value beats parsing the writer's
                // output — the split worked, and `identity-and-provenance.md` is right
                // that it should not have to.
                "id": inc.equation_refs[i].to_string(),
                "unknowns": sorted,
            });
            if let Some(text) = eq_texts.get(i) {
                row.as_object_mut().unwrap().insert(
                    "equation_text".to_owned(),
                    serde_json::Value::String(text.clone()),
                );
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
                    // The AUTHORITATIVE identity, not a derivation: the reference
                    // Rumoca kept is in hand here, unlike in `structural_to_json`
                    // where `StructuralReport` has already discarded it. Without
                    // this, pointing at a row of the index-reduction Before pane
                    // resolved to nothing — the pane names 321 equations across the
                    // notebook and could identify none of them.
                    "id": inc.equation_refs[eq_idx].to_string(),
                    "unknown": inc.unknown_names[v].to_string(),
                })
            })
        })
        .collect();
    serde_json::Value::Array(pairs)
}

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
            n_equations,
            n_unknowns,
            n_matched,
            unmatched_equations,
            unmatched_unknowns,
            unmatched_unknown_spans,
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
                n_equations,
                n_unknowns,
                n_matched,
                unmatched_equations,
                unmatched_unknowns,
                ..
            } = source
            {
                let obj = json.as_object_mut().unwrap();
                obj.insert("n_equations".to_owned(), (*n_equations).into());
                obj.insert("n_unknowns".to_owned(), (*n_unknowns).into());
                obj.insert("n_matched".to_owned(), (*n_matched).into());
                obj.insert(
                    "rank_deficiency".to_owned(),
                    ((*n_equations).max(*n_unknowns) - n_matched).into(),
                );
                obj.insert(
                    "unmatched_equations".to_owned(),
                    serde_json::json!(unmatched_equations),
                );
                obj.insert(
                    "unmatched_unknowns".to_owned(),
                    serde_json::json!(unmatched_unknowns),
                );
            }
            json
        }
        SolveModelLowerError::MassMatrix {
            row,
            state_name,
            reason,
            ..
        } => {
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
        SolveModelLowerError::Evaluation {
            context, source, ..
        } => {
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
            // **The spy-plot's cross-pane identity.** Clicking a block captures this
            // node, and without an `id` the capture named the equation differently
            // from every other pane — the spy-plot being one of the two surfaces
            // nothing but a capture can reach.
            "id": crate::equation_id_from_label(equation),
            "unknown": unknown,
        }),
        BlockReport::Coupled {
            equations,
            unknowns,
            tearing,
        } => serde_json::json!({
            "kind": "coupled",
            "size": unknowns.len(),
            "equations": equations,
            // Plural, matching `equations`: a coupled block is several equations
            // solved together, and "this block" resolves to all of them.
            "ids": equations
                .iter()
                .map(|e| crate::equation_id_from_label(e))
                .collect::<Vec<_>>(),
            "unknowns": unknowns,
            "tearing": tearing.as_ref().map(tearing_to_json),
        }),
    }
}

/// The tearing report, with an identity beside every equation it names.
///
/// Tearing reduces a coupled block's iteration size: "tear variables" are
/// guessed, the remaining equations are solved causally (one at a time), and
/// the residual equations check convergence. The causal sequence is the order
/// of the sequential solves.
///
/// **`TearingReport` keeps labels only**, like the rest of `StructuralReport`, so
/// these ids are derived by [`crate::equation_id_from_label`] and checked against the
/// authoritative incidence ids by
/// `doc_citations::an_equation_id_names_the_same_equation_in_every_pane`.
///
/// This was the **fifth** writer found to name equations without identifying them,
/// and it is the one that showed manual enumeration was not converging: its labels
/// sit in a bare string array (`residual_equations`) with no field name of their own,
/// so the scan that found the first four could not see it at all.
/// `lib::unidentified_equation_labels` now walks for them instead.
fn tearing_to_json(t: &rumoca_phase_structural::TearingReport) -> serde_json::Value {
    serde_json::json!({
        "tear_vars": t.tear_vars,
        "residual_equations": t.residual_equations,
        // Parallel to `residual_equations`, one id per entry, same order. Named
        // explicitly rather than `ids` because this object also lists `tear_vars`,
        // and a bare `ids` here would not say which list it identifies.
        "residual_equation_ids": t
            .residual_equations
            .iter()
            .map(|e| crate::equation_id_from_label(e))
            .collect::<Vec<_>>(),
        "causal_sequence": t
            .causal_sequence
            .iter()
            .map(|(e, v)| serde_json::json!({
                "equation": e,
                "id": crate::equation_id_from_label(e),
                "variable": v,
            }))
            .collect::<Vec<_>>(),
    })
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
        Some(PhaseResult::Failed {
            phase,
            error,
            error_code,
            diagnostics,
        }) => {
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
                    Stage::err_with_details(
                        serde_json::json!({
                            "kind": "flatten",
                            "message": error,
                            "error_code": error_code,
                            "diagnostics": diag_json,
                            "guidance": "Flattening transforms the component hierarchy into flat equations. \
                                Check for unsupported language features, circular definitions, or type mismatches.",
                        }),
                        msg,
                    )
                }
                FailedPhase::ToDae => {
                    // **Flatten stops adopting DAE construction's failure — 2026-08-25.**
                    //
                    // The history, because it explains why this arm existed at all.
                    // Until 2026-07-29 it discarded everything and returned a bare
                    // `Stage::info("...DAE construction failed (later arc)")` while
                    // `error`, `error_code` and `diagnostics` sat in scope, unused —
                    // which made the **most common Modelica authoring error** (declare
                    // a variable, forget its equation) the *least* informative failure
                    // in the pipeline. So it was promoted to a real error carrying the
                    // payload, and at the time that was right: **DAE construction had
                    // no tab of its own**, Flatten was the last tab before Structural,
                    // and somebody had to report it.
                    //
                    // `dae_absent_stage` gave DAE its own tab on 2026-08-03 and now
                    // reports this exact error itself — but this arm was never removed,
                    // so **both stages rendered the same payload** and
                    // `OverDeterminedShaft`/`UnbalancedShaft` read `OOOOXX.....`: two
                    // stages each claiming, per `Outcome::Failed`'s own definition,
                    // that *the pipeline stopped here*. It stops once, and it stopped
                    // at DAE construction. Found by the corpus outcome matrix.
                    //
                    // **Flatten is not failing here — it succeeded.** Reaching ToDae
                    // requires it. What is true is that `PhaseResult::Failed` carries
                    // no flat model, so there is nothing to show; that is a different
                    // fact from failing, and a different one again from not running.
                    //
                    // `last_successful_stage` is unaffected: its `ok` predicate is
                    // `value.is_some() && !note_is_error()`, and `info` carries no
                    // value, so Flatten does not become the furthest good stage. The
                    // 2026-07-29 comment named only the `note_is_error` half.
                    Stage::info(
                        "flatten completed \u{2014} the compile failed at DAE \
                         construction, whose result carries no flat model. The DAE \
                         tab has the error.",
                    )
                }
                other => Stage::info(not_reached_note(&format!("{other} failed earlier"))),
            }
        }
        Some(PhaseResult::NeedsInner { missing_inners, .. }) => Stage::info(format!(
            "needs inner declaration(s) for: {}",
            missing_inners.join(", ")
        )),
        // `info`, not `err` — same reason as `not_reached_stage`'s None arm.
        None => Stage::info(no_result_note()),
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
        Some(PhaseResult::Failed {
            phase: FailedPhase::ToDae,
            error,
            error_code,
            diagnostics,
        }) => {
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
            Stage::info(not_reached_note(&format!("{phase} failed earlier")))
        }
        Some(PhaseResult::NeedsInner { missing_inners, .. }) => Stage::info(format!(
            "needs inner declaration(s) for: {}",
            missing_inners.join(", ")
        )),
        // **`None` says what every other stage says — corrected 2026-08-25.** This
        // fell through to `Stage::default()`, a wholly blank tab, on the reasoning
        // that *"`None` is the no-result case Flatten already reports"*. That is the
        // adoption argument this function's own doc comment above was written to
        // reject, in the same file, about the same two stages: **absence is stated,
        // never filled, and a NEIGHBOURING tab stating it does not satisfy that.** Six
        // sibling stages carry this note for this exact condition and DAE was the only
        // silent one — found while checking whether the not-reached tail was contiguous.
        None => Stage::info(no_result_note()),
        // Success with no DAE cannot happen — the DAE is how success is defined.
        Some(PhaseResult::Success(_)) => Stage::default(),
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
    let name_by_id: std::collections::HashMap<u32, &str> = tree
        .def_map
        .iter()
        .map(|(k, v)| (k.0, v.as_str()))
        .collect();

    let mut ids = BTreeSet::new();
    collect_def_ids(value, &mut ids);

    let mut index = BTreeMap::new();
    for id in ids {
        // Use try_from to avoid silently truncating u64 → u32; if the id
        // doesn't fit in a u32, skip it gracefully rather than wrapping.
        let Some(id32) = u32::try_from(id).ok() else {
            continue;
        };
        let Some(name) = name_by_id.get(&id32) else {
            continue;
        };
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
            None => DefInfo {
                name,
                kind: DefKind::Definition,
                class_type: None,
                file_name: None,
                line: None,
            },
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
pub(crate) mod test_msl;

#[cfg(test)]
mod tests;

/// One funnel step, with the system's shape on either side of it.
///
/// **The shapes are the part that was missing.** The pane used to show a step's name
/// and an outcome string, and an outcome of `"ok"` says a step *ran* while saying
/// nothing about what it *did*. Ten steps reporting "ok" with one of them changing the
/// counts is a different picture from ten that all moved something, and the pane could
/// not tell those apart — which is how a funnel that did nothing at all
/// (`CartesianPendulum`) looked the same as one doing quiet work.
struct StepRow {
    name: &'static str,
    outcome: String,
    states_before: usize,
    states_after: usize,
    equations_before: usize,
    equations_after: usize,
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
    /// Per-step log, one row per funnel step.
    steps: Vec<StepRow>,
    /// Equations manufactured by differentiation (origin contains
    /// `"index_reduction:d_dt_for_"`), with the state they were created for.
    differentiated_rows: Vec<(String, String)>,
    /// Trivial-elimination substitutions (variable → replacement expression).
    eliminations: Vec<(String, String)>,
    /// The step at which the funnel stopped (if it bailed early on error).
    stopped_at: Option<&'static str>,
    /// **How many differentiations the funnel actually performed**, counted from the
    /// frames captured on this run.
    ///
    /// Distinct from [`Self::differentiated_rows`], which scans the FINAL DAE for
    /// surviving origin markers and is therefore a count of *survivors*. The two
    /// disagree whenever a later step removes a differentiated row, which on this
    /// corpus is always: `Drivetrain` differentiates six times and retains none.
    ///
    /// **The gap between them taught a tour the opposite of the truth for its whole
    /// existence** (`DECISIONS.md`, 2026-08-17). Publishing both is what makes the
    /// pane unable to repeat that: a reader seeing an empty list beside "6 performed"
    /// asks the right question instead of concluding zero.
    n_differentiations: usize,
}

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
fn index_reduce_for_structural_analysis(
    dae: &mut rumoca_ir_dae::Dae,
) -> (
    ReductionReport,
    Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>,
) {
    use rumoca_phase_structural::dae_prepare as dp;
    use rumoca_phase_structural::eliminate;

    let states_before: Vec<String> = dae.variables.states.keys().map(|k| k.to_string()).collect();

    // **The compile's own options.** `scalarize` defaults to true and the simulation
    // path takes the default, so the real funnel scalarizes first — a step HRW's
    // mirror never had. Passing anything else here would put the tab back to
    // describing a funnel the compiler does not run.
    let opts = rumoca_sim::SimOptions::default();

    // Both levels, from one run. `RefCell` because the two observers are `Fn`
    // closures called from inside the funnel, and the frames they collect outlive
    // each call.
    let steps: std::cell::RefCell<Vec<StepRow>> = std::cell::RefCell::new(Vec::new());
    let ir_frames: std::cell::RefCell<Vec<dp::IndexReductionFrame>> =
        std::cell::RefCell::new(Vec::new());

    let outcome = {
        let on_step = |f: &rumoca_sim::FunnelStepFrame| {
            let text = match &f.outcome {
                rumoca_sim::FunnelStepOutcome::Demoted(n) => format!("{n} demoted"),
                rumoca_sim::FunnelStepOutcome::Rewrote(n) => format!("{n} rewritten"),
                rumoca_sim::FunnelStepOutcome::Completed => "ok".to_owned(),
                rumoca_sim::FunnelStepOutcome::Failed(why) => format!("stopped: {why}"),
            };
            // **The sizes either side, not just the outcome text.** An outcome of
            // "ok" says a step ran and nothing about what it did; the pair of
            // shapes says whether the system actually changed under it. Ten steps
            // reporting "ok" and one changing the counts is a very different
            // picture from ten steps that all moved something, and until now the
            // pane could not tell those apart.
            steps.borrow_mut().push(StepRow {
                name: f.step,
                outcome: text,
                states_before: f.states_before,
                states_after: f.states_after,
                equations_before: f.equations_before,
                equations_after: f.equations_after,
            });
        };
        let on_frame = |f: &dp::IndexReductionFrame| ir_frames.borrow_mut().push(f.clone());
        rumoca_sim::prepare_dae_for_structural_analysis_fully_observed(
            dae,
            &opts,
            Some(&on_step),
            Some(&on_frame),
        )
    };

    let mut steps = steps.into_inner();
    let mut ir_frames = ir_frames.into_inner();

    // A failing funnel names the step it stopped at, which the observer already
    // reported — so the report's `stopped_at` is read off the frames rather than
    // tracked separately.
    let mut stopped_at: Option<&'static str> = None;
    if outcome.is_err() {
        stopped_at = steps
            .iter()
            .rev()
            .find(|row| row.outcome.starts_with("stopped:"))
            .map(|row| row.name);
        let n_differentiations = count_differentiations(&ir_frames);
        return (
            finish_report(dae, states_before, steps, stopped_at, n_differentiations),
            ir_frames,
        );
    }
    // Nothing below can set it; kept so the shape of the report is unchanged.
    let _ = &mut stopped_at;
    let _ = &mut ir_frames;

    // **`eliminate_trivial` is NOT part of the preparation funnel**, and this is where
    // the mirror was most misleading: it listed the step as though it were, so the
    // tab attributed 77 eliminations to index reduction. In `rumoca-sim` it belongs
    // to the next phase (`structural.eliminate_trivial`). HRW applies it here because
    // the Index Reduction tab shows the system the *later* views are computed over —
    // but it is now visibly a separate act rather than step 10 of a funnel.
    let mut eliminations = Vec::new();
    // Its own before/after, measured here rather than reported by the funnel — this
    // step is not in the funnel, so nothing upstream can supply them.
    let (states_before_elim, equations_before_elim) =
        (dae.variables.states.len(), dae.continuous.equations.len());
    if let Ok(elim) = eliminate::eliminate_trivial(dae) {
        for sub in &elim.substitutions {
            let expr_json = serde_json::to_string(&sub.expr).unwrap_or_default();
            eliminations.push((sub.var_name.to_string(), expr_json));
        }
        let _ = eliminate::apply_elimination_substitutions_to_dae(dae, &elim.substitutions);
        steps.push(StepRow {
            name: "eliminate_trivial",
            outcome: format!("{} eliminated", elim.n_eliminated),
            states_before: states_before_elim,
            states_after: dae.variables.states.len(),
            equations_before: equations_before_elim,
            equations_after: dae.continuous.equations.len(),
        });
    } else {
        steps.push(StepRow {
            name: "eliminate_trivial",
            outcome: "failed (system may still be singular)".to_owned(),
            states_before: states_before_elim,
            states_after: dae.variables.states.len(),
            equations_before: equations_before_elim,
            equations_after: dae.continuous.equations.len(),
        });
    }

    let n_differentiations = count_differentiations(&ir_frames);
    (
        finish_report(dae, states_before, steps, stopped_at, n_differentiations)
            .with_eliminations(eliminations),
        ir_frames,
    )
}

/// Count the differentiations the funnel actually performed, from its own frames.
///
/// **The one number the Index Reduction stage could not previously report.** Its
/// `differentiated_rows` scans the *final* DAE for surviving origin markers, so it
/// reports zero whenever a later step removes them — which on this corpus is always.
/// A tour read that zero as *"the compiler did not differentiate"* and taught the
/// opposite of the truth for its whole existence.
fn count_differentiations(
    frames: &[rumoca_phase_structural::dae_prepare::IndexReductionFrame],
) -> usize {
    use rumoca_phase_structural::dae_prepare::IndexReductionStep;
    frames
        .iter()
        .filter(|f| matches!(&f.step, IndexReductionStep::Differentiated { .. }))
        .count()
}

/// Build a `ReductionReport` from the post-reduction DAE state. Called at
/// every exit point from `index_reduce_for_structural_analysis` (both
/// early-bail and normal completion). Scans the DAE's equations for
/// differentiated rows (manufactured by the index-reduction process) by
/// looking for the `"index_reduction:d_dt_for_"` marker in equation origins.
fn finish_report(
    dae: &rumoca_ir_dae::Dae,
    states_before: Vec<String>,
    steps: Vec<StepRow>,
    stopped_at: Option<&'static str>,
    n_differentiations: usize,
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
        n_differentiations,
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
            // **The shapes travel with the step**, so a reader can see which steps
            // moved the system rather than only that each one ran.
            "steps": self.steps.iter().map(|row| {
                serde_json::json!({
                    "step": row.name,
                    "outcome": row.outcome,
                    "states_before": row.states_before,
                    "states_after": row.states_after,
                    "equations_before": row.equations_before,
                    "equations_after": row.equations_after,
                })
            }).collect::<Vec<_>>(),
            "differentiated_rows": self.differentiated_rows.iter().map(|(origin, state)| {
                serde_json::json!({ "equation_origin": origin, "for_state": state })
            }).collect::<Vec<_>>(),
            "eliminations": self.eliminations.iter().map(|(var, expr)| {
                serde_json::json!({ "variable": var, "replacement": expr })
            }).collect::<Vec<_>>(),
            "n_differentiations": self.n_differentiations,
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
                if old_stdout >= 0 {
                    libc::close(old_stdout);
                }
                if old_stderr >= 0 {
                    libc::close(old_stderr);
                }
                libc::close(out_fds[0]);
                libc::close(out_fds[1]);
                libc::close(err_fds[0]);
                libc::close(err_fds[1]);
                return None;
            }

            if libc::dup2(out_fds[1], 1) < 0 || libc::dup2(err_fds[1], 2) < 0 {
                libc::dup2(old_stdout, 1);
                libc::dup2(old_stderr, 2);
                libc::close(old_stdout);
                libc::close(old_stderr);
                libc::close(out_fds[0]);
                libc::close(out_fds[1]);
                libc::close(err_fds[0]);
                libc::close(err_fds[1]);
                return None;
            }
            // Close the original write ends — fd 1 and fd 2 are the only
            // writers now (via dup2). The reader threads will see EOF when
            // Drop restores the original fds and closes the pipe write ends.
            libc::close(out_fds[1]);
            libc::close(err_fds[1]);

            let stdout_buf = Arc::new(Mutex::new(Vec::new()));
            let stderr_buf = Arc::new(Mutex::new(Vec::new()));

            let out_reader =
                Self::spawn_reader(file_from_raw_fd(out_fds[0]), Arc::clone(&stdout_buf));
            let err_reader =
                Self::spawn_reader(file_from_raw_fd(err_fds[0]), Arc::clone(&stderr_buf));

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

/// **Did resolution fail for this compile?**
///
/// `Session::tree()` answered this with its `Err`. `Session::strict_compile_resolved()`
/// cannot: it builds under `ResolveBuildMode::StrictCompileRecovery`, which **succeeds
/// past errors that `Standard` treats as fatal** — that is what recovery means. So the
/// signal has to come from the diagnostics it returns alongside the tree.
///
/// # Why error-severity, and not "any diagnostic"
///
/// Measured 2026-08-04 on two specimens, which is the only reason this is not a guess:
///
/// | specimen | `tree()` | strict diagnostics | of which errors |
/// |---|---|---|---|
/// | `SingleInertia` (good) | `Ok` | **33** | **0** |
/// | `UndefinedRef` (broken) | `Err` | 34 | **1** |
///
/// **A good model carries 33 library-wide diagnostics.** Treating any diagnostic as
/// failure would fail every model in the workspace; error severity separates them
/// exactly.
///
/// # Why not `compile_model_diagnostics(..).global_resolution_failure`
///
/// It matches on this pair, and it is purpose-named — but the same measurement showed
/// that call returns *the same 34 diagnostics*, not a model-scoped subset, so it is not
/// the finer instrument it appears to be. It also costs a semantic-diagnostics query
/// where this costs nothing.
///
/// **This preserves `tree()`'s behaviour rather than improving on it.** A library-wide
/// *error* fails the model here, exactly as it did before — replacing a signal should
/// change nothing, and any improvement belongs in its own change with its own evidence.
fn resolve_diagnostics_indicate_failure(diags: &rumoca_core::Diagnostics) -> bool {
    diags
        .iter()
        .any(|d| d.severity == rumoca_core::DiagnosticSeverity::Error)
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

/// Take the buffered trace events instead of logging them, so the caller can
/// decide **which phase each one belongs under**.
///
/// Doug, 2026-08-04: *"I still do not see rumoca trace log lines for phases such as
/// Instantiate which are contained within the rumoca compile phase."* They were
/// there — as one undifferentiated block under `Rumoca compile`, because that is the
/// single call all four phases run inside and `drain_traces` empties the buffer to
/// wherever the log happens to be.
///
/// Every event carries its emitting target (`[rumoca_phase_instantiate::connections]`),
/// so the block can be split by **what each line says about itself** rather than by
/// guesswork. See `attribute_traces`.
/// **Brackets that are legitimately not a compiler phase.** Exhaustive, and each
/// entry carries why it is allowed.
///
/// - `Rumoca compile` — a real *call*, not a phase. It brackets the four phases that
///   run inside `compile_model_strict_reachable_uncached_with_recovery`, and its
///   number is that call's own duration. Naming the call is a fact about what HRW
///   did; naming a *phase* that spans those four would be the "DAE pipeline" fiction
///   again, which is why this list is short and argued rather than open.
/// - `Compile (for simulation)` — the simulate path's own call. Distinct from the
///   above because it is a *different* compile, and a reader watching a simulation
///   needs to see that a compile happened rather than infer it.
/// - `Integration` — the solver run. Genuinely not a compiler phase at all;
///   `StageKind::Simulation` exists but is not in `COMPILATION`, and the log names
///   the *activity* (integrating) rather than the artifact (a simulation).
const NON_PHASE_BRACKETS: &[&str] = &["Rumoca compile", "Compile (for simulation)", "Integration"];

/// **Does this `StageStart`/`StageEnd` message name something that actually exists?**
///
/// A bracket is a claim: *this named thing ran, and what is nested inside it belongs
/// to that thing*. Until 2026-08-04 the log carried a bracket called **"DAE
/// pipeline"** — five real phases given an invented parent because it read tidily.
/// Doug found it walking the tour that teaches DAE construction, and named the class:
/// *"logging is supposed to accurately describe what actually happened."*
///
/// Accepts a **prefix** match, because brackets carry qualifiers a reader needs — a
/// timing on close (`"Flatten (0.2ms)"`) and, for the compile, a description
/// (`"Rumoca compile — full pipeline; …"`). The *name* is what must be real;
/// what follows it is commentary.
/// Returns the canonical name, so the caller can also check that brackets **pair**.
///
/// Longest match wins. Nothing in the current set is a prefix of another, but a future
/// `"Flatten"`/`"Flatten connections"` pair would silently mis-pair under first-match,
/// and a mis-paired bracket is a nesting claim that is wrong — the same class of
/// falsehood as an invented name.
pub(crate) fn bracket_phase_name(msg: &str) -> Option<&'static str> {
    StageKind::COMPILATION
        .iter()
        .map(|k| k.log_name())
        .chain(NON_PHASE_BRACKETS.iter().copied())
        .filter(|known| msg.starts_with(known))
        .max_by_key(|known| known.len())
}

pub(crate) fn bracket_names_a_real_phase(msg: &str) -> bool {
    bracket_phase_name(msg).is_some()
}

type CapturedTraces = Vec<(LogLevel, String)>;

fn take_traces() -> CapturedTraces {
    TRACE_BUFFER.with(|buf| {
        buf.borrow_mut()
            .drain(..)
            .map(|(level, msg)| {
                let ll = match level {
                    tracing::Level::ERROR => LogLevel::Error,
                    tracing::Level::WARN => LogLevel::Warn,
                    _ => LogLevel::Trace,
                };
                (ll, msg)
            })
            .collect()
    })
}

/// Split traces into the ones a phase claims and the ones nobody does.
///
/// Returns `(mine, rest)` for the given target prefix. **Only an exact target match
/// counts.** `rumoca_eval_flat::phase_constant` is emitted *during* flatten but names
/// a different crate, so it stays in `rest` rather than being filed under Flatten —
/// attributing it would be inference, and this project has a standing rule against
/// letting a substring decide identity (`docs/identity-and-provenance.md`).
fn attribute_traces(traces: CapturedTraces, target: &str) -> (CapturedTraces, CapturedTraces) {
    let marker = format!("[{target}");
    traces
        .into_iter()
        .partition(|(_, msg)| msg.starts_with(&marker))
}

/// Drain all buffered tracing events and forward them as log entries.
///
/// Called after each Rumoca API call in `compile()` and `simulate()`.
/// `TRACE_BUFFER.with(|buf| ...)` accesses this thread's buffer.
/// `buf.borrow_mut().drain(..)` takes all elements out of the Vec (leaving
/// it empty) and iterates over them. The `&dyn Fn(...)` parameter is a
/// trait object — dynamic dispatch (vs `&impl Fn` which is static dispatch).
/// Used here because `drain_traces` is called from multiple contexts.
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
