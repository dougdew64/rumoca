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

/// One pipeline stage's outcome for the selected model: the serialized IR node
/// (if the stage produced one) plus an optional note (error or status).
///
/// This is the uniform envelope every stage tab in the UI receives. The UI
/// doesn't know which pipeline stage it came from — it just renders:
/// - `value`: the JSON IR tree (if the stage produced one), displayed in the
///   generic tree inspector
/// - `note`: an optional status/error message shown above the tree
/// - `note_is_error`: controls colour (red = error, neutral = info)
///
/// `#[derive(Clone, Default)]` — `Clone` because the progressive-streaming
/// pattern sends clones mid-compile; `Default` gives "not yet computed"
/// (all `None`/`false`).
#[derive(Clone, Default)]
pub struct Stage {
    pub value: Option<serde_json::Value>,
    pub note: Option<String>,
    /// True when `note` is an error (rendered red); false = an informational
    /// status like "succeeded" or "not reached" (rendered neutral).
    pub note_is_error: bool,
}

/// Constructors for the four possible stage outcomes. These are `pub(crate)` —
/// only the worker builds stages; the UI consumes them read-only.
impl Stage {
    /// Stage succeeded and produced an IR tree to display.
    fn ok(value: serde_json::Value) -> Self {
        Stage { value: Some(value), note: None, note_is_error: false }
    }
    /// Stage failed — no IR, just an error message (rendered red).
    /// `impl Into<String>` accepts both `String` and `&str` — a Rust ergonomic
    /// pattern so callers can pass either without explicit conversion.
    fn err(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), note_is_error: true }
    }
    /// A non-error status note for a stage with no IR of its own to show.
    fn info(note: impl Into<String>) -> Self {
        Stage { value: None, note: Some(note.into()), note_is_error: false }
    }
    /// A best-effort IR plus an error note (e.g. resolve recovered a partial tree).
    fn recovered(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), note_is_error: true }
    }
    /// A successful IR plus an informational (non-error) note — e.g. the
    /// index-reduction stage's "already index-1" / "reduced from singular".
    fn ok_with_note(value: serde_json::Value, note: impl Into<String>) -> Self {
        Stage { value: Some(value), note: Some(note.into()), note_is_error: false }
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
    fn err_with_details(error: serde_json::Value, note: impl Into<String>) -> Self {
        let json = serde_json::json!({ "error": error });
        Stage::recovered(json, note)
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
        StageKind::Typecheck, StageKind::Flatten, StageKind::Structural,
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
        StageKind::Typecheck, StageKind::Flatten, StageKind::Structural,
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
            StageKind::Structural => "Structural",
            StageKind::IndexReduction => "Index reduction",
            StageKind::Initialization => "Initialization",
            StageKind::Events => "Events",
            StageKind::SolveLowering => "Solve lowering",
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
    pub fn as_stage_pairs(&self) -> [(&'static str, Option<&serde_json::Value>); 10] {
        [
            ("parse", self.parse.value.as_ref()),
            ("resolve", self.resolve.value.as_ref()),
            ("instantiate", self.instantiate.value.as_ref()),
            ("typecheck", self.typecheck.value.as_ref()),
            ("flatten", self.flatten.value.as_ref()),
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
struct WorkerState {
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

impl WorkerState {
    fn new() -> Self {
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
    fn load_libraries(&mut self, roots: Vec<PathBuf>) -> Result<usize, String> {
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
    fn compile(&mut self, path: &Path, emit: &impl Fn(FromWorker)) -> FromWorker {
        use std::time::Instant;
        let t0 = Instant::now();
        let path_owned = path.to_owned();
        let log = make_log(&t0, emit);

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

        log(LogLevel::Info, format!("compiling {}", path.file_name().and_then(|n| n.to_str()).unwrap_or("?")));

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                drop(output_capture.take());
                return FromWorker::Compiled {
                    path: path_owned.clone(),
                    model: None,
                    stages: StageBundle {
                        parse: Stage::err(format!("read error: {e}")),
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
                };
            }
        };
        let uri = path.to_string_lossy().to_string();
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("buffer.mo");

        // =====================================================================
        // Stage 1: PARSE — source text to AST
        // =====================================================================
        // Rumoca API: `parse_to_ast(source, file_name)` parses Modelica source
        // into an AST. Returns `Ok(StoredDefinition)` or `Err` on syntax error.
        // We grab the first class name from `ast.classes` as the model name.
        log(LogLevel::StageStart, "Parse".to_owned());
        let t_stage = Instant::now();
        let (parse, model) = match rumoca_phase_parse::parse_to_ast(&source, file_name) {
            Ok(ast) => {
                // `ast.classes` is a BTreeMap<String, ClassDef>. `.keys().next()`
                // gets the first (alphabetically) class name. For a specimen file
                // with one model, this is the model we want.
                let model = ast.classes.keys().next().cloned();
                (Stage::from_ser(&ast), model)
            }
            Err(e) => {
                let msg = format!("{e:#}");
                (Stage::err_with_details(serde_json::json!({
                    "kind": "parse",
                    "message": msg,
                    "guidance": "Check the Modelica source for syntax errors.",
                }), msg), None)
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
            path: path_owned.clone(),
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
        let mut def_index = BTreeMap::new();
        let mut instantiate = Stage::default();
        let mut typecheck = Stage::default();
        let mut connection_frames = Vec::new();
        let resolve = match &model {
            None => Stage::err("parse produced no model to resolve"),
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);
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
            path: path_owned.clone(),
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
        self.last_resolve_failed = resolve.note_is_error;

        // =====================================================================
        // Stages 5-10: DAE pipeline (Flatten → Solve lowering)
        // =====================================================================
        // These stages come from a single Rumoca pipeline invocation:
        // `compile_model_strict_reachable_with_recovery`. This one call runs
        // flatten → DAE construction internally. Then we extract individual
        // stage results from the returned `PhaseResult` using the
        // `*_stage()` functions, and run structural analysis / index
        // reduction / initialization / events / solve lowering ourselves.
        //
        // Each stage emits a `CompileProgress` so its tab colours in the UI
        // as soon as it's known.
        //
        // The return type is a 6-tuple — Rust's way of returning multiple
        // values without defining a struct. Destructured immediately via
        // `let (flatten, structural, ...) = match ...`.
        let (flatten, structural, index_reduction, initialization, events, solve_lowering, equation_sheet, identifier_index, ir_frames, compiled_dae, pre_frames, compiled_flat) = match &model {
            None => {
                let e = "parse produced no model to compile";
                (Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), None, None, Vec::new(), None, Vec::new(), None)
            }
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);

                log(LogLevel::StageStart, "DAE pipeline (flatten → solve lowering)".to_owned());
                let t_pipeline = Instant::now();
                // Uncached, for the reason spelled out in `simulate` above: a
                // cached result means the phases did not run, so nothing can be
                // observed happening — breakpoints, tracing, or timing.
                let report = self
                    .session
                    .compile_model_strict_reachable_uncached_with_recovery(&qualified);
                drain_traces(&log);
                drain_output(&mut output_capture, &log);

                let result = report.requested_result.as_ref();

                let eq_sheet = match result {
                    Some(PhaseResult::Success(cr)) => {
                        Some(crate::equation_sheet::build(
                            &cr.dae,
                            Some((&uri, &source)),
                        ))
                    }
                    _ => None,
                };

                let id_index = match result {
                    Some(PhaseResult::Success(cr)) => {
                        Some(crate::identifier_index::IdentifierIndex::build(
                            &cr.dae, &uri, &source,
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
                            path: path_owned.clone(), stages: bundle.clone(),
                        });
                        stage
                    }};
                }

                let flatten = run_stage!("Flatten", flatten_stage(result, &source), flatten);
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
                        path: path_owned.clone(), stages: bundle.clone(),
                    });
                    (stage, frames)
                };
                let initialization = run_stage!("Initialization", initialization_stage(result), initialization);
                let events = run_stage!("Events", events_stage(result), events);
                let solve_lowering = run_stage!("Solve lowering", solve_lowering_stage(result), solve_lowering);

                log(LogLevel::StageEnd, format!("DAE pipeline ({:.1}ms)", t_pipeline.elapsed().as_secs_f64() * 1000.0));

                let dae = match result {
                    Some(PhaseResult::Success(cr)) => Some(cr.dae.clone()),
                    _ => None,
                };

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

                (flatten, structural, index_reduction, initialization, events, solve_lowering, eq_sheet, id_index, ir_frames, dae, pre_frames, flat)
            }
        };

        // Restore stdout/stderr by dropping the OutputCapture.
        // `output_capture.take()` moves the value out of the `Option`,
        // returning `Some(capture)`, and `drop()` runs its `Drop` impl
        // which restores the original file descriptors via `dup2`.
        drop(output_capture.take());
        log(LogLevel::Info, format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0));

        // Build and return the final `Compiled` message with all ten stages.
        FromWorker::Compiled {
            path: path_owned,
            model,
            stages: StageBundle {
                parse,
                resolve,
                instantiate,
                typecheck,
                flatten,
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
                .insert("incidence".to_owned(), incidence_to_json(&inc, Some(&cr.dae.continuous.equations)));
            Stage::ok(json)
        }
        Err(e) => {
            let inc = rumoca_phase_structural::build_incidence(&cr.dae);
            let (match_eq, _) = rumoca_phase_structural::matching::maximum_matching(
                inc.n_eq, inc.n_var, &inc.eq_unknowns,
            );
            let matching_json = partial_matching_to_json(&inc, &match_eq);
            let mut json = serde_json::json!({});
            let obj = json.as_object_mut().unwrap();
            obj.insert("incidence".to_owned(), incidence_to_json(&inc, Some(&cr.dae.continuous.equations)));
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
            obj.insert("incidence".to_owned(), incidence_to_json(&inc, Some(&reduced.continuous.equations)));
            obj.insert("before".to_owned(), before_report_json(
                &before_inc, &before_match_eq, Some(&cr.dae.continuous.equations),
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
                Some(&reduced.continuous.equations),
            ));
            obj.insert("before".to_owned(), before_report_json(
                &before_inc, &before_match_eq, Some(&cr.dae.continuous.equations),
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
    equations: Option<&[rumoca_ir_dae::Equation]>,
) -> serde_json::Value {
    let eq_texts: Vec<String> = equations
        .map(|eqs| crate::expr_format::equation_labels(eqs))
        .unwrap_or_default();
    let rows: Vec<serde_json::Value> = inc
        .eq_unknowns
        .iter()
        .enumerate()
        .map(|(i, cols)| {
            let mut sorted: Vec<usize> = cols.iter().copied().collect();
            sorted.sort_unstable();
            // Use the same labeled name as `structural_to_json` so matching
            // lookups in `IncidenceMatrix::from_report` resolve correctly.
            let eq_name = match equations.and_then(|eqs| eqs.get(inc.equation_refs[i].0)) {
                Some(eq) if !eq.origin.trim().is_empty() => {
                    format!("{} ({})", inc.equation_refs[i], eq.origin.trim())
                }
                _ => inc.equation_refs[i].to_string(),
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
    equations: Option<&[rumoca_ir_dae::Equation]>,
) -> serde_json::Value {
    let matching: Vec<serde_json::Value> = match_eq
        .iter()
        .enumerate()
        .filter_map(|(eq_idx, var_idx)| {
            var_idx.map(|v| {
                serde_json::json!({
                    "equation": inc.equation_refs[eq_idx].to_string(),
                    "unknown": inc.unknown_names[v].to_string(),
                })
            })
        })
        .collect();
    let n_matched = matching.len();
    serde_json::json!({
        "n_equations": inc.n_eq,
        "n_unknowns": inc.n_var,
        "n_matched": n_matched,
        "matching": matching,
        "incidence": incidence_to_json(inc, equations),
    })
}

/// Build a JSON array of matched (equation, unknown) pairs from a partial
/// matching result — the same shape as the structural report's `"matching"`
/// array so `IncidenceMatrix::from_report` can parse it.
fn partial_matching_to_json(
    inc: &rumoca_phase_structural::Incidence,
    match_eq: &[Option<usize>],
) -> serde_json::Value {
    let pairs: Vec<serde_json::Value> = match_eq
        .iter()
        .enumerate()
        .filter_map(|(eq_idx, var_idx)| {
            var_idx.map(|v| {
                serde_json::json!({
                    "equation": inc.equation_refs[eq_idx].to_string(),
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
#[cfg(test)]
mod tests {
    use super::*;
    // `Mutex` — a mutual exclusion lock. Wrapping `WorkerState` in `Mutex`
    // lets multiple test functions share it safely, but only one can access
    // it at a time (the others block). This is how the tests run serially
    // against a single MSL-loaded session.
    //
    // `OnceLock` — a thread-safe cell that can be written to exactly once.
    // Used here for lazy one-time initialization of the shared worker.
    // `OnceLock` is the thread-safe equivalent of `OnceCell` (or Python's
    // `functools.lru_cache` with `maxsize=1` conceptually).
    use std::sync::{Mutex, OnceLock};

    /// Returns the three MSL (Modelica Standard Library) root paths needed
    /// to compile any specimen that uses standard components.
    fn msl_roots() -> Vec<PathBuf> {
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
    fn shared_worker() -> &'static Mutex<WorkerState> {
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
    fn compile_specimen_shared(name: &str) -> FromWorker {
        let path = PathBuf::from(format!("{}/specimens/{name}.mo", env!("CARGO_MANIFEST_DIR")));
        let mut w = shared_worker().lock().unwrap_or_else(|e| e.into_inner());
        w.compile(&path, &|_: FromWorker| {})
    }

    /// End-to-end: after resolving `RotationalInertia` against the MSL, the
    /// component *types* (`type_def_id`) must resolve to their MSL classes.
    #[test]
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
    fn two_loops_has_two_coupled_blocks() {
        let v = structural_report_for("TwoLoops");
        assert_eq!(v["coupled_block_count"], serde_json::json!(2));
    }

    /// NonlinearLoop is structurally identical to ProportionalLoop (structure is
    /// blind to the nonlinearity) — still one coupled block.
    #[test]
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
    fn capacitor_loop_is_singular_and_irreducible() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("CapacitorLoop") else {
            panic!("expected Compiled");
        };
        assert!(stages.flatten.value.is_some(), "CapacitorLoop should still flatten");
        assert!(stages.structural.note_is_error, "expected singular Structural");
        assert!(stages.structural.value.as_ref().unwrap().get("error").is_some(),
            "singular Structural should carry error details");
        assert!(stages.index_reduction.note_is_error,
            "index reduction should NOT rescue a capacitor-across-source loop");
        assert!(stages.index_reduction.value.as_ref().unwrap().get("error").is_some(),
            "irreducible index reduction should carry error details");
    }

    /// The Initialization stage plans a consistent initial state for the RC
    /// circuit — a non-empty IC plan plus the ground-current relaxation hint.
    #[test]
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
    fn over_init_rc_is_flagged_over_determined() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("OverInitRc") else {
            panic!("expected Compiled");
        };
        let v = stages.initialization.value.expect("IC plan");
        assert_eq!(v["determinacy"]["verdict"], serde_json::json!("over-determined"));
        assert!(v["determinacy"]["surplus_over_states"].as_i64().unwrap_or(0) >= 1);
        assert!(stages.initialization.note_is_error, "over-determined init should be flagged red");
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
    /// (no IR), but the `index_reduction` stage recovers a solvable report — the
    /// before/after the two tabs show side by side.
    #[test]
    fn drivetrain_index_reduction_stage_recovers_singular() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("Drivetrain") else {
            panic!("expected Compiled");
        };
        assert!(stages.structural.note_is_error, "raw Structural should be singular for Drivetrain");
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

    /// A singular Structural stage carries structured error data (equation
    /// and unknown counts, rank deficiency, unmatched names) plus the
    /// incidence matrix and partial matching for UI rendering.
    #[test]
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
        assert!(err["unmatched_equations"].as_array().unwrap().len() > 0);
        assert!(err["unmatched_unknowns"].as_array().unwrap().len() > 0);
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
            stages.solve_lowering.value.is_some() && !stages.solve_lowering.note_is_error,
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
    fn a_broken_specimen_does_not_poison_the_next_compile() {
        let mut w = WorkerState::new();
        w.load_libraries(msl_roots()).expect("load MSL");
        let dir = format!("{}/specimens", env!("CARGO_MANIFEST_DIR"));
        let resolve_note = |w: &mut WorkerState, name: &str| -> (bool, String) {
            let path = PathBuf::from(format!("{dir}/{name}.mo"));
            match w.compile(&path, &|_: FromWorker| {}) {
                FromWorker::Compiled { stages, .. } => {
                    let st = stages.get(StageKind::Resolve);
                    (st.note_is_error, st.note.clone().unwrap_or_default())
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
    fn an_unbalanced_model_reports_its_balance() {
        let FromWorker::Compiled { stages, .. } = compile_specimen_shared("UnbalancedShaft")
        else {
            panic!("expected Compiled");
        };

        let flatten = &stages.flatten;
        assert!(flatten.note_is_error, "a failed DAE construction is an error, not an info note");
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
    }

    /// Simulation also emits log entries with timing.
    #[test]
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
        assert!(stages.parse.note_is_error, "parse stage should flag an error for a missing file");
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
            stages.parse.note_is_error,
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
        assert!(stage.note_is_error, "extract_class should flag an error for a missing name");
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
            11,
            "StageKind::ALL should list every variant (currently 11 stages)"
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
pub(crate) fn index_reduce_in_place(dae: &mut rumoca_ir_dae::Dae) {
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
