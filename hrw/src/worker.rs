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
use rumoca_compile::compile::{FailedPhase, PhaseResult, SourceRootKind};
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
    let threshold = ((hi - lo) * 0.08).max(6.0 * median);
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
}

/// Which pipeline stage the user is viewing. The Rumoca compiler has discrete
/// phases (Parse, Resolve, etc.), each producing an intermediate representation
/// (IR). This enum tracks which phase is selected. `Simulation` is special —
/// it's not a compiler phase but an on-demand run of the compiled model.
#[derive(Clone, Copy, PartialEq, Eq)]
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
    /// "class" (a class definition) or "definition" (component/other non-class).
    pub kind: &'static str,
    /// Class keyword ("model", "block", …) when this DefId names a class.
    pub class_type: Option<String>,
    /// Source location of the class definition (when a class).
    pub file_name: Option<String>,
    pub line: Option<u32>,
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
            "kind": self.kind,
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
#[derive(Clone, Copy, PartialEq, Eq)]
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
    tx: Sender<ToWorker>,
    pub rx: Receiver<FromWorker>,
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

        Worker { tx: tx_req, rx: rx_res }
    }

    /// Send a request to the worker. Never blocks.
    ///
    /// `let _ = ...` ignores send errors (worker thread already exited).
    /// `mpsc::Sender::send()` is non-blocking for unbounded channels.
    pub fn send(&self, req: ToWorker) {
        let _ = self.tx.send(req);
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
    /// Library roots currently loaded, so a specimen compile knows they're ready.
    libraries: Vec<PathBuf>,
    /// Guard for the thread-local tracing subscriber. This is an RAII guard —
    /// while it exists (`Some`), the `TracingForwarder` subscriber is active on
    /// this thread. When dropped (set to `None`), the subscriber is deactivated.
    /// `tracing::subscriber::set_default()` returns this guard; it's
    /// thread-local (only affects the worker thread), not global.
    tracing_guard: Option<tracing::subscriber::DefaultGuard>,
}

impl WorkerState {
    fn new() -> Self {
        WorkerState {
            session: Session::new(SessionConfig::default()),
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

        // `log` is a local closure that wraps `emit` with timing. Every log
        // call computes `elapsed_secs` from the same `t0` baseline, so all
        // entries in this simulate() call share a consistent timeline.
        let log = |level: LogLevel, msg: String| {
            emit(FromWorker::Log(LogEntry {
                elapsed_secs: t0.elapsed().as_secs_f64(),
                level,
                message: msg,
            }));
        };

        log(LogLevel::Info, format!("simulating {model} to t={t_end}"));

        // --- Phase 1: Compile the model to a DAE ---
        log(LogLevel::StageStart, "Compile (for simulation)".to_owned());
        let t_stage = Instant::now();
        let source = std::fs::read_to_string(path).map_err(|e| format!("read error: {e}"))?;
        let uri = path.to_string_lossy().to_string();
        // Update the session's document cache with the (possibly edited) source.
        // Rumoca API: `update_document` registers/updates a single file in the session.
        self.session.update_document(&uri, &source);
        // Rumoca API: `qualify_model_name` turns a simple name like "BouncingBall"
        // into a fully-qualified name like "BouncingBall" (for top-level models,
        // these are the same, but nested models would differ).
        let qualified = self.session.qualify_model_name(&uri, model);
        // Rumoca API: `compile_model_strict_reachable_with_recovery` is the main
        // pipeline entry point. It runs parse → resolve → flatten → DAE construction,
        // with error recovery so partial results are available on failure.
        // Returns a `CompileReport` whose `requested_result` is `Option<PhaseResult>`.
        let report = self.session.compile_model_strict_reachable_with_recovery(&qualified);
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
                let value = serde_json::to_value(class).unwrap_or_default();
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

        // Local log helper — same pattern as in `simulate()`.
        let log = |level: LogLevel, msg: String| {
            emit(FromWorker::Log(LogEntry {
                elapsed_secs: t0.elapsed().as_secs_f64(),
                level,
                message: msg,
            }));
        };

        // Start capturing stdout/stderr from Rumoca library calls (unix only).
        // Some Rumoca phases print diagnostics via `println!`/`eprintln!` rather
        // than returning them as structured errors. `OutputCapture` intercepts
        // these at the file-descriptor level and forwards them as log entries.
        // `#[cfg(unix)]` means this line is only compiled on unix — the struct
        // uses unix-specific `dup2()` / `pipe()` system calls.
        #[cfg(unix)]
        let mut output_capture = OutputCapture::start();

        // `drain_output` pulls captured stdout/stderr and forwards each line
        // as a log entry. The `#[allow(unused)]` on `capture` suppresses a
        // warning on non-unix platforms where the `#[cfg(unix)]` body is absent.
        // `&dyn Fn(...)` is a *trait object* (dynamic dispatch) — unlike
        // `&impl Fn(...)` (static dispatch), it works across the closure
        // boundary here where the concrete type isn't known.
        let drain_output = |#[allow(unused)] capture: &mut Option<OutputCapture>, log_fn: &dyn Fn(LogLevel, String)| {
            #[cfg(unix)]
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
                #[cfg(unix)]
                drop(output_capture.take());
                return FromWorker::Compiled {
                    path: path.to_owned(),
                    model: None,
                    stages: StageBundle {
                        parse: Stage::err(format!("read error: {e}")),
                        ..Default::default()
                    },
                    def_index: BTreeMap::new(),
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
                let value = serde_json::to_value(&ast).unwrap_or_default();
                (Stage::ok(value), model)
            }
            Err(e) => (Stage::err(format!("{e:#}")), None),
        };
        // After each Rumoca API call, drain any `tracing` events that were
        // buffered by our `TracingForwarder` subscriber.
        drain_traces(&log);
        #[cfg(unix)]
        drain_output(&mut output_capture, &log);
        log(LogLevel::StageEnd, format!("Parse ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
        // Stream the first progress snapshot: Parse is done, everything else
        // is `Stage::default()` (neutral). `..Default::default()` fills the
        // remaining struct fields with their default values — a Rust pattern
        // called "struct update syntax".
        emit(FromWorker::CompileProgress {
            path: path.to_owned(),
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
        // Register the specimen source in the session so resolution can find it.
        self.session.update_document(&uri, &source);
        let mut def_index = BTreeMap::new();
        let mut instantiate = Stage::default();
        let mut typecheck = Stage::default();
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
                        let (i, t) = instantiate_and_typecheck(&rt.0, &qualified);
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
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(&rt.0, &qualified) {
                                Stage { value: Some(v), .. } => {
                                    def_index = build_def_index(&rt.0, &v);
                                    Stage::recovered(v, note)
                                }
                                _ => Stage::err(note),
                            },
                            None => Stage::err(note),
                        }
                    }
                }
            }
        };

        drain_traces(&log);
        #[cfg(unix)]
        drain_output(&mut output_capture, &log);
        log(LogLevel::StageEnd, format!("Resolve ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
        emit(FromWorker::CompileProgress {
            path: path.to_owned(),
            stages: StageBundle {
                parse: parse.clone(),
                resolve: resolve.clone(),
                instantiate: instantiate.clone(),
                typecheck: typecheck.clone(),
                ..Default::default()
            },
        });

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
        let (flatten, structural, index_reduction, initialization, events, solve_lowering) = match &model {
            None => {
                let e = "parse produced no model to compile";
                (Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e), Stage::err(e))
            }
            Some(simple_name) => {
                let qualified = self.session.qualify_model_name(&uri, simple_name);

                log(LogLevel::StageStart, "DAE pipeline (flatten → solve lowering)".to_owned());
                let t_pipeline = Instant::now();
                let report = self.session.compile_model_strict_reachable_with_recovery(&qualified);
                drain_traces(&log);
                #[cfg(unix)]
                drain_output(&mut output_capture, &log);

                let result = report.requested_result.as_ref();

                let mut bundle = StageBundle {
                    parse: parse.clone(),
                    resolve: resolve.clone(),
                    instantiate: instantiate.clone(),
                    typecheck: typecheck.clone(),
                    ..Default::default()
                };

                log(LogLevel::StageStart, "Flatten".to_owned());
                let t_stage = Instant::now();
                let flatten = flatten_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Flatten ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.flatten = flatten.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle.clone() });

                log(LogLevel::StageStart, "Structural analysis".to_owned());
                let t_stage = Instant::now();
                let structural = structural_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Structural analysis ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.structural = structural.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle.clone() });

                log(LogLevel::StageStart, "Index reduction".to_owned());
                let t_stage = Instant::now();
                let index_reduction = index_reduction_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Index reduction ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.index_reduction = index_reduction.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle.clone() });

                log(LogLevel::StageStart, "Initialization".to_owned());
                let t_stage = Instant::now();
                let initialization = initialization_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Initialization ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.initialization = initialization.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle.clone() });

                log(LogLevel::StageStart, "Events".to_owned());
                let t_stage = Instant::now();
                let events = events_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Events ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.events = events.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle.clone() });

                log(LogLevel::StageStart, "Solve lowering".to_owned());
                let t_stage = Instant::now();
                let solve_lowering = solve_lowering_stage(result);
                drain_traces(&log);
                log(LogLevel::StageEnd, format!("Solve lowering ({:.1}ms)", t_stage.elapsed().as_secs_f64() * 1000.0));
                bundle.solve_lowering = solve_lowering.clone();
                emit(FromWorker::CompileProgress { path: path.to_owned(), stages: bundle });

                log(LogLevel::StageEnd, format!("DAE pipeline ({:.1}ms)", t_pipeline.elapsed().as_secs_f64() * 1000.0));

                (flatten, structural, index_reduction, initialization, events, solve_lowering)
            }
        };

        // Restore stdout/stderr by dropping the OutputCapture (unix only).
        // `output_capture.take()` moves the value out of the `Option`,
        // returning `Some(capture)`, and `drop()` runs its `Drop` impl
        // which restores the original file descriptors via `dup2`.
        #[cfg(unix)]
        drop(output_capture.take());
        log(LogLevel::Info, format!("done ({:.1}ms total)", t0.elapsed().as_secs_f64() * 1000.0));

        // Build and return the final `Compiled` message with all ten stages.
        FromWorker::Compiled {
            path: path.to_owned(),
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
fn structural_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            match rumoca_phase_structural::build_structural_report(&cr.dae) {
                Ok(rep) => {
                    let inc = rumoca_phase_structural::build_incidence(&cr.dae);
                    let mut json = structural_to_json(&rep);
                    json.as_object_mut()
                        .unwrap()
                        .insert("incidence".to_owned(), incidence_to_json(&inc));
                    Stage::ok(json)
                }
                Err(e) => Stage::err(format!("structural analysis failed: {e}")),
            }
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
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
fn index_reduction_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => {
            let raw_ok = rumoca_phase_structural::build_structural_report(&cr.dae).is_ok();
            let mut reduced = cr.dae.clone();
            let reduction = index_reduce_for_structural_analysis(&mut reduced);
            match rumoca_phase_structural::build_structural_report(&reduced) {
                Ok(rep) => {
                    let inc = rumoca_phase_structural::build_incidence(&reduced);
                    let note = if raw_ok {
                        "already index-1 — the reduction funnel is a no-op here (same as the Structural tab)"
                    } else {
                        "index-reduced from a structurally singular (high-index) system — now solvable"
                    };
                    let mut json = structural_to_json(&rep);
                    let obj = json.as_object_mut().unwrap();
                    obj.insert("incidence".to_owned(), incidence_to_json(&inc));
                    obj.insert("reduction".to_owned(), reduction.to_json());
                    Stage::ok_with_note(json, note)
                }
                Err(e) => Stage::err(format!("still singular after index reduction: {e}")),
            }
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
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
    match result {
        Some(PhaseResult::Success(cr)) => {
            let n_x = cr.dae.variables.states.len();
            let n_eq = cr.dae.continuous.equations.len();
            // Determinacy of the *user* initialization (idea #6): the explicit
            // initial conditions (initial equations + fixed-start states) vs the
            // states. A surplus means an OVER-determined init (conflicting /
            // redundant conditions — a blow-up `build_ic_plan` alone doesn't catch,
            // since it plans the algebraic subsystem, not the user's init).
            // Under-determination isn't flagged: remaining states initialize from
            // their `start` attributes (default init), so a negative count is normal.
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
                        // Over-determined: still show the plan, but flag it red.
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
                Err(e) => Stage::err(format!("IC planning failed: {e}")),
            }
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
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
fn ic_plan_to_json(
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
                "solution": serde_json::to_value(solution_expr).unwrap_or_default(),
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
    match result {
        Some(PhaseResult::Success(cr)) => {
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
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
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
            "equations_f_c": serde_json::to_value(&conditions.equations).unwrap_or_default(),
            "relations": serde_json::to_value(&conditions.relations).unwrap_or_default(),
        },
        "discrete_updates": {
            "real_updates_f_z": serde_json::to_value(&discrete.real_updates).unwrap_or_default(),
            "valued_updates_f_m": serde_json::to_value(&discrete.valued_updates).unwrap_or_default(),
        },
        "events": {
            "zero_crossing_conditions": serde_json::to_value(&events.synthetic_root_conditions).unwrap_or_default(),
            "scheduled_time_events": serde_json::to_value(&events.scheduled_time_events).unwrap_or_default(),
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
    match result {
        Some(PhaseResult::Success(cr)) => {
            match rumoca_phase_solve::lower_dae_to_solve_model(&cr.dae) {
                Ok(sm) => match serde_json::to_value(&sm) {
                    Ok(v) => Stage::ok(v),
                    Err(e) => Stage::err(format!("serialize SolveModel: {e}")),
                },
                Err(e) => Stage::err(format!("solve lowering failed: {e}")),
            }
        }
        Some(PhaseResult::Failed { phase, .. }) => {
            Stage::info(format!("not reached ({phase} failed earlier)"))
        }
        Some(PhaseResult::NeedsInner { .. }) => {
            Stage::info("not reached (model needs inner declarations)")
        }
        None => Stage::err("the reachable-closure pipeline produced no result for this model"),
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
fn incidence_to_json(inc: &rumoca_phase_structural::Incidence) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = inc
        .eq_unknowns
        .iter()
        .enumerate()
        .map(|(i, cols)| {
            let mut sorted: Vec<usize> = cols.iter().copied().collect();
            sorted.sort_unstable();
            serde_json::json!({
                "equation": inc.equation_refs[i].to_string(),
                "unknowns": sorted,
            })
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
fn instantiate_and_typecheck(tree: &rumoca_ir_ast::ClassTree, model_name: &str) -> (Stage, Stage) {
    match rumoca_phase_instantiate::instantiate_model(tree, model_name) {
        Ok(mut overlay) => {
            let instantiate = Stage::ok(serde_json::to_value(&overlay).unwrap_or_default());
            let typecheck = match rumoca_phase_typecheck::typecheck_instanced(tree, &mut overlay, model_name) {
                Ok(()) => Stage::ok(serde_json::to_value(&overlay).unwrap_or_default()),
                // Best-effort: still show the (partially) enriched overlay + the note.
                Err(_diags) => Stage::recovered(
                    serde_json::to_value(&overlay).unwrap_or_default(),
                    "instanced typecheck reported diagnostics",
                ),
            };
            (instantiate, typecheck)
        }
        Err(e) => (
            Stage::err(format!("instantiate failed: {e}")),
            Stage::info("not reached (instantiate failed)"),
        ),
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
fn flatten_stage(result: Option<&PhaseResult>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => match serde_json::to_value(&cr.flat) {
            Ok(v) => Stage::ok(v),
            Err(e) => Stage::err(format!("serialize flat model: {e}")),
        },
        Some(PhaseResult::Failed { phase, error, diagnostics, .. }) => {
            let msg = if diagnostics.is_empty() {
                error.clone()
            } else {
                format!("{error}  ({} diagnostic(s))", diagnostics.len())
            };
            match phase {
                FailedPhase::Flatten => Stage::err(msg),
                FailedPhase::ToDae => {
                    Stage::info("flatten succeeded; DAE construction failed (later arc)")
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
        let Some(name) = name_by_id.get(&(id as u32)) else { continue };
        let name = (*name).to_owned();
        // A class DefId resolves to a ClassDef (with a location); anything else
        // in def_map (e.g. a component) resolves to a name only.
        let info = match tree.get_class_by_qualified_name(&name) {
            Some(class) => DefInfo {
                name,
                kind: "class",
                class_type: Some(class.class_type.as_str().to_owned()),
                file_name: Some(class.location.file_name.clone()),
                line: Some(class.location.start_line),
            },
            None => DefInfo { name, kind: "definition", class_type: None, file_name: None, line: None },
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
                def_index.values().any(|d| d.kind == "class" && d.name.ends_with(expected)),
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
        assert!(stages.structural.value.is_none() && stages.structural.note_is_error, "expected singular Structural");
        assert!(
            stages.index_reduction.value.is_none() && stages.index_reduction.note_is_error,
            "index reduction should NOT rescue a capacitor-across-source loop"
        );
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
        assert!(stages.structural.value.is_none(), "raw Structural should be singular for Drivetrain");
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
fn index_reduce_for_structural_analysis(dae: &mut rumoca_ir_dae::Dae) -> ReductionReport {
    use rumoca_phase_structural::dae_prepare as dp;
    use rumoca_phase_structural::eliminate;

    // Snapshot the state names before reduction so we can diff them later
    // (the "demoted_states" in the report are states_before - states_after).
    let states_before: Vec<String> = dae.variables.states.keys().map(|k| k.to_string()).collect();
    let mut steps: Vec<(&str, String)> = Vec::new();
    let mut stopped_at: Option<&str> = None;

    // `macro_rules!` defines a Rust macro — a compile-time code template.
    // `run_step!` wraps a funnel step call, logging its outcome and stopping
    // the funnel early on error. This avoids repeating the same match/log/bail
    // pattern for each step. Macros in Rust are hygienic (they don't pollute
    // the caller's namespace) and are expanded at compile time (zero runtime
    // cost). `$name:expr` and `$call:expr` are the macro's parameters —
    // any Rust expression.
    macro_rules! run_step {
        ($name:expr, $call:expr) => {
            match $call {
                Ok(n) => steps.push(($name, format!("{n} demoted"))),
                Err(e) => {
                    steps.push(($name, format!("stopped: {e}")));
                    stopped_at = Some($name);
                }
            }
        };
    }

    // Same as `run_step!` but for steps that return `Result<(), Error>`
    // (no count to report).
    macro_rules! run_step_unit {
        ($name:expr, $call:expr) => {
            match $call {
                Ok(()) => steps.push(($name, "ok".to_owned())),
                Err(e) => {
                    steps.push(($name, format!("stopped: {e}")));
                    stopped_at = Some($name);
                }
            }
        };
    }

    run_step!("demote_exact_alias_component_states", dp::demote_exact_alias_component_states(dae));
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

    run_step!("demote_direct_assigned_states", dp::demote_direct_assigned_states(dae));
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

    run_step!("reduce_constrained_dummy_derivatives", dp::reduce_constrained_dummy_derivatives(dae));
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

    run_step!("index_reduce_missing_state_derivatives", dp::index_reduce_missing_state_derivatives(dae));
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

    let n_unassignable = dp::demote_states_without_assignable_derivative_rows(dae);
    steps.push(("demote_states_without_assignable_derivative_rows", format!("{n_unassignable} demoted")));

    run_step_unit!("eliminate_derivative_aliases", dp::eliminate_derivative_aliases(dae));
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

    match dp::demote_states_without_retained_derivative_rows(dae) {
        Ok((no_der_ref, unassignable)) => steps.push((
            "demote_states_without_retained_derivative_rows",
            format!("{no_der_ref} no-derivative-ref + {unassignable} unassignable demoted"),
        )),
        Err(e) => {
            steps.push(("demote_states_without_retained_derivative_rows", format!("stopped: {e}")));
            stopped_at = Some("demote_states_without_retained_derivative_rows");
        }
    }
    if stopped_at.is_some() { return finish_report(dae, states_before, steps, stopped_at); }

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

    finish_report(dae, states_before, steps, stopped_at)
        .with_eliminations(eliminations)
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
    let differentiated_rows: Vec<(String, String)> = dae
        .continuous
        .equations
        .iter()
        .filter_map(|eq| {
            let marker = "index_reduction:d_dt_for_";
            eq.origin.find(marker).map(|pos| {
                let state = eq.origin[pos + marker.len()..].to_owned();
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
        Some(class) => Stage::ok(serde_json::to_value(class).unwrap_or_default()),
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
/// # Why `#[cfg(unix)]`?
///
/// `dup()`, `dup2()`, and `pipe()` are POSIX system calls — they exist on
/// Linux and macOS but not Windows. `#[cfg(unix)]` is conditional compilation:
/// this entire struct and its `impl` blocks are only compiled on unix targets.
/// On Windows, stdout/stderr capture is simply skipped (the `#[cfg(unix)]`
/// guards at the call sites compile to nothing).
///
/// # Safety
///
/// The `unsafe` blocks are required because `libc::dup2()` etc. are foreign
/// function calls (FFI) that Rust can't verify for memory safety. The
/// invariants we maintain: we always restore the original fds on Drop, we
/// close all fds we open, and we never use a closed fd.
#[cfg(unix)]
struct OutputCapture {
    /// Saved copy of the original stdout (fd 1), restored on Drop.
    old_stdout: i32,
    /// Saved copy of the original stderr (fd 2), restored on Drop.
    old_stderr: i32,
    /// Read end of the stdout pipe — `drain()` reads captured output from here.
    stdout_read: std::fs::File,
    /// Read end of the stderr pipe — `drain()` reads captured output from here.
    stderr_read: std::fs::File,
}

#[cfg(unix)]
impl OutputCapture {
    /// Set up the pipe/dup2 capture. Returns `None` if any system call fails
    /// (rather than panicking — output capture is best-effort, not critical).
    ///
    /// `unsafe` is required for all `libc::*` calls (they're C FFI). The
    /// `FromRawFd` trait converts a raw file descriptor (i32) into a Rust
    /// `File` object that will close the fd when dropped.
    fn start() -> Option<Self> {
        use std::os::unix::io::FromRawFd;

        unsafe {
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::Write::flush(&mut std::io::stderr());

            let mut out_fds = [0i32; 2];
            let mut err_fds = [0i32; 2];
            if libc::pipe(out_fds.as_mut_ptr()) != 0 {
                return None;
            }
            if libc::pipe(err_fds.as_mut_ptr()) != 0 {
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
            libc::close(out_fds[1]);
            libc::close(err_fds[1]);

            Some(OutputCapture {
                old_stdout,
                old_stderr,
                stdout_read: std::fs::File::from_raw_fd(out_fds[0]),
                stderr_read: std::fs::File::from_raw_fd(err_fds[0]),
            })
        }
    }

    /// Read any output that has accumulated in the pipes since the last drain.
    /// Returns (stdout_text, stderr_text). Called after each Rumoca API call
    /// to forward captured output as log entries.
    fn drain(&mut self) -> (String, String) {
        let _ = std::io::Write::flush(&mut std::io::stdout());
        let _ = std::io::Write::flush(&mut std::io::stderr());
        (Self::drain_pipe(&mut self.stdout_read), Self::drain_pipe(&mut self.stderr_read))
    }

    /// Read all available data from one pipe without blocking.
    ///
    /// The trick: temporarily set the fd to non-blocking mode (`O_NONBLOCK`),
    /// read everything available, then restore blocking mode. Without
    /// `O_NONBLOCK`, `read_to_end()` would block forever waiting for more data
    /// (the write end of the pipe is still open — it's fd 1 or 2).
    fn drain_pipe(file: &mut std::fs::File) -> String {
        use std::io::Read;
        use std::os::unix::io::AsRawFd;
        let fd = file.as_raw_fd();
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }
        let mut buf = Vec::new();
        let _ = file.read_to_end(&mut buf);
        unsafe {
            let flags = libc::fcntl(fd, libc::F_GETFL);
            libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK);
        }
        String::from_utf8_lossy(&buf).into_owned()
    }
}

/// RAII cleanup: when `OutputCapture` is dropped (goes out of scope or is
/// explicitly dropped), restore the original stdout/stderr file descriptors.
/// Without this, the process's stdout/stderr would remain redirected to the
/// (now-closed) pipes, and all subsequent output would be lost.
///
/// `Drop` is Rust's equivalent of a C++ destructor or Python's `__del__` —
/// but unlike Python, it's guaranteed to run (no GC timing issues).
#[cfg(unix)]
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
