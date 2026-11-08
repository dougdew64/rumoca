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
    /// **the UI had no way to ask**, which meant a lab could not link to an MSL
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
        /// **A tolerance chosen in the UI, overriding whatever the model declares.**
        ///
        /// `None` means *do not override* — use the model's `experiment(Tolerance)` if it has
        /// one, else the solver default. See [`resolve_tolerance`], which owns the precedence
        /// and is where the reasoning lives.
        tolerance: Option<f64>,
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
        trace:
            crate::live_trace::LiveTrace<rumoca_phase_flatten::connections::trace::ConnectionFrame>,
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
/// subtler. (It named `rumoca_solver::SolverStepRecord`, gone since v0.10.0; the
/// step rows are HRW's own `plot_labels::StepRow`.)
#[derive(Clone)]
pub struct SimData {
    pub times: Vec<f64>,
    pub names: Vec<String>,
    pub data: Vec<Vec<f64>>,
    /// How many scalars the integrator advances.
    ///
    /// **This is a count, not a position — corrected 2026-11-04.** It used to be read
    /// as *"the first `n_states` names are the true states"*, which held while states
    /// led the name list and stopped holding on v0.10.4. `CartesianPendulum` lowers to
    /// **one** `State` declaration — `$state_coordinates`, a 2-vector chart on the
    /// constraint manifold — with `x`, `y`, `vx`, `vy` and `lambda` all `Algebraic`
    /// and reconstructed from it. The name list still begins `x, y`, so the positional
    /// rule labelled two algebraics as the model's states, in the pane Doug reads
    /// plots from. Use [`is_state`](Self::is_state) instead.
    pub n_states: usize,
    /// Rumoca's own per-variable answer to *is this a state*, aligned with
    /// [`names`](Self::names).
    ///
    /// **Read, never derived.** `SimVariableMeta::is_state` is published per variable
    /// by the solver; inferring it from an ordering is the same class of mistake as
    /// renaming a field — HRW asserting something about Rumoca that Rumoca already
    /// states. Empty only if the solver returned no metadata, which the accessor
    /// treats as "unknown" rather than as "false".
    pub is_state: Vec<bool>,
    /// For a generated reduced-selection state scalar, the source scalar it equals.
    ///
    /// `None` for every ordinary variable. On a model that selects a chart this is how
    /// a reader learns that `$state_coordinates[1]` *is* some declared variable, which
    /// is the whole content of the selection — and it comes from
    /// `SimVariableMeta::state_coordinate`, whose doc is *"the source scalar a
    /// generated reduced-selection state scalar equals"*.
    pub state_coordinate: Vec<Option<String>>,
    /// True when the model has a **discrete update** — a `reinit` (`f_z`) or a
    /// `when`-clause assignment (`f_m`) — that can jump a variable's value at an
    /// event. Only then does the plot break the polyline at discontinuities. A
    /// bare zero-crossing with no update (BenchActuator has one, yet is smooth)
    /// does *not* count, and a smooth model's coarse-but-steep transients (a stiff
    /// current spike) must never be mistaken for jumps. See [`discontinuity_segments`].
    pub has_discontinuities: bool,
    /// Per-step solver diagnostics (t, h, order) from the integrator.
    ///
    /// **Filled from the integrator's own accepted steps** since the 2026-11-03
    /// re-model — [`solver_steps_from_integrator`], fed by this fork's observation-only
    /// step records in `rumoca-sim` / `rumoca-solver`. Empty when the backend that ran
    /// reports no steps, and the Simulation pane says which kind of empty it is.
    ///
    /// *(Until the 2026-11-07 sweep this said "always empty on v0.10.0" and called the
    /// re-instrumentation still owed. It was empty from the rebase until 11-03: the
    /// old `rumoca_solver::SolverStepRecord` had been HRW's own instrumentation, and
    /// 0.10.0's `SimResult` carried no per-step record.)*
    ///
    /// The step-size-on-a-log-axis and order plots it feeds were built for a specific
    /// reported failure — Doug, 2026-09-25, could not verify a single one of lecture 1's
    /// solver claims because egui_plot's three-place default rendered a 1e-4 step as
    /// `0.000`.
    pub solver_steps: Vec<crate::plot_labels::StepRow>,
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

    /// This run's trajectories, as the bridge and the notebook both publish them.
    ///
    /// # Why one function rather than two
    ///
    /// This body lived in `examples/gen_trace.rs`, which put the committed
    /// notebook baseline inside an example the library cannot reach — so the
    /// live bridge had no simulation file at all, and **the plot pane was the
    /// only stage whose data Claude could not read.** On 2026-09-04 that cost
    /// two diagnoses of a flat-trajectory defect: the first was wrong, and the
    /// second had to rest on Doug squinting at a plot and reporting *"C.v is
    /// flat at 5, I think"*. A hedged reading of the one artifact under
    /// suspicion is the worst possible evidence, and it was the only kind
    /// available.
    ///
    /// Sharing the writer is what makes the comparison that matters — a live
    /// run against the committed baseline — a comparison of like with like. Two
    /// writers of the same summary can disagree, and a disagreement between
    /// them would look exactly like the compiler drifting.
    ///
    /// # Why a summary and not the full series
    ///
    /// Per variable: `initial`, `final`, `min` and `max`. That is not a
    /// description of the trajectory, it is four of its values — enough to
    /// answer *"did this move, and from where to where"*, which is the question
    /// a flat plot raises. The full series is 501 × N floats and answers a
    /// question nobody has yet asked; add it when one does, rather than
    /// speculating about shape now.
    ///
    /// `t_end` is the requested horizon and is passed in because it belongs to
    /// the *request*, not the result: `t_final` below is how far the solver
    /// actually got, and the two differing is a finding rather than an error.
    pub fn to_bridge_json(&self, t_end: f64) -> serde_json::Value {
        let variables: Vec<serde_json::Value> = self
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let series = &self.data[i];
                serde_json::json!({
                    "name": name,
                    // **Rumoca's answer, not a position.** See `SimData::is_state`:
                    // the positional rule called `CartesianPendulum`'s `x` and `y`
                    // states on v0.10.4, when the only state it lowers is a 2-vector
                    // chart and both of those are reconstructed algebraics. Falls
                    // back to the old rule only when the solver returned no metadata
                    // at all, so a missing answer cannot silently read as `false`.
                    "is_state": self
                        .is_state
                        .get(i)
                        .copied()
                        .unwrap_or(i < self.n_states),
                    // Present only on a generated reduced-selection state scalar,
                    // where it names the source scalar that coordinate equals.
                    "state_coordinate": self
                        .state_coordinate
                        .get(i)
                        .and_then(Option::as_deref),
                    "initial": series.first().copied().unwrap_or(f64::NAN),
                    "final": series.last().copied().unwrap_or(f64::NAN),
                    "min": series.iter().copied().fold(f64::INFINITY, f64::min),
                    "max": series.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                })
            })
            .collect();

        serde_json::json!({
            "t_end": t_end,
            "n_time_points": self.times.len(),
            "n_variables": self.names.len(),
            "n_states": self.n_states,
            "has_discontinuities": self.has_discontinuities,
            "t_start": self.times.first().copied().unwrap_or(0.0),
            "t_final": self.times.last().copied().unwrap_or(0.0),
            "variables": variables,
        })
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
/// **The UI caught up on 2026-11-05, and the gap had lasted since the split.** This doc
/// said *"the UI's red/neutral colouring is unchanged"*, which was true and read as
/// harmless: `Stage::note_is_error` is `outcome != Ok`, so [`Outcome::Flagged`] — the
/// outcome this enum exists to distinguish — painted exactly the red the split was
/// introduced to stop meaning. Doug reported it on `CartesianPendulum`.
/// `stage_tabs::tab_label` now branches on the outcome itself, and the remaining UI
/// readers followed on 2026-11-07 (see [`Stage::note_is_error`]).
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
    /// Rendered amber, not red — the user should see it, and it is not a stop.
    /// Counted separately from a failure, because it is not one. (This said
    /// *"rendered red like a failure"* until 2026-11-07, a year-old truth about the
    /// UI that the 11-05 tab fix had already falsified.)
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
    /// Human-readable equations for nodes of [`value`](Self::value), keyed by node path.
    ///
    /// **Rendered from the TYPED IR while the worker still holds it**, which is the whole
    /// safety property — see `equation_text`. Empty for every stage that has nothing
    /// renderable, and empty is a report: no tooltip line rather than a blank one.
    ///
    /// Carried on `Stage` rather than beside it so it cannot drift from the `value` it
    /// describes: the two are built together and travel together.
    pub rendered: crate::equation_text::Rendered,
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
            rendered: crate::equation_text::Rendered::default(),
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
            rendered: crate::equation_text::Rendered::default(),
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
            rendered: crate::equation_text::Rendered::default(),
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
            rendered: crate::equation_text::Rendered::default(),
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
            rendered: crate::equation_text::Rendered::default(),
        }
    }
    // `ok_with_note` — a successful IR plus an informational, non-error note —
    // is deleted rather than kept behind an `#[expect(dead_code)]`. Its only
    // callers were the index-reduction, Events and Initialization stages, all
    // disabled for this rebase, and `dead_code = deny` is the signal saying so
    // rather than a nuisance to suppress.
    //
    // **The re-models all landed without it** (index reduction uses `Stage::ok` and
    // sets `.note`). If a stage ever does want it, re-add it rather than reach for
    // `recovered`. `recovered` claims `Outcome::Flagged`, which
    // says the compiler flagged a problem; "already index-1" is a successful
    // outcome that merely has something to say. `Outcome` is a claim
    // (`CLAUDE.md`), so the distinction is not cosmetic.

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
            rendered: crate::equation_text::Rendered::default(),
        }
    }

    /// Did Rumoca report something about this stage? True for **both** abnormal
    /// outcomes — it does not say whether the pipeline stopped.
    ///
    /// It preserved the old boolean field's behaviour exactly when the outcome split
    /// landed, and that was the defect: every reader went on treating a flag as a
    /// failure. **The UI's readers moved to [`Stage::outcome`] on 2026-11-05 and
    /// 2026-11-07** (the tab, the note colour, the pipeline context, the action trail,
    /// the landing stage). Two keep this predicate deliberately —
    /// `App::central_panel_ui`'s `has_error_summary`, which also requires an `error`
    /// payload, and `artifact_pane` — and **a new caller should ask which question it
    /// means before reaching for this one.**
    pub fn note_is_error(&self) -> bool {
        self.outcome != Outcome::Ok
    }

    /// Whether this stage says **it never ran**, as opposed to ran and retained nothing.
    ///
    /// # Why `value.is_none()` is not this question, measured
    ///
    /// They look interchangeable and are not, and
    /// `the_not_reached_tail_is_contiguous_and_agrees_on_its_cause` already carries the
    /// counter-example: `OverDeterminedShaft` reads `OOOO.X.....`, where **Flatten is
    /// empty *before* DAE's failure** because no flat model was retained — it ran.
    /// `CartesianPendulum` is the other direction: Initialization produces no IR and
    /// says *"no initial-value pins: structural preparation did not admit this
    /// system"*, which is a claim about the model, while Events and Solve lowering
    /// after it both produce IR and the model simulates.
    ///
    /// **So a missing value is ambiguous and the note is not** — exactly the
    /// distinction [`not_reached_note`] exists to protect. `App::failure_context` built
    /// its `downstream_not_reached` list from `value.is_none()` until 2026-11-05, and so
    /// told Claude that `CartesianPendulum`'s Initialization was never reached.
    pub fn says_not_reached(&self) -> bool {
        self.note
            .as_deref()
            .is_some_and(|n| n.starts_with(NOT_REACHED_PREFIX))
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
    /// writing `docs/fixture-labs/dae-construction.md`, which had to teach the
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
    /// `StageBundle::get()` *panics* on it. So any code that runs stages and asks the
    /// bundle for each one must use this list, not `ALL`. Added 2026-07-29 after
    /// `failure_context` run `ALL` and hit that panic in three tests — the trap is
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

    /// The file this stage's IR is written to, under `.hrw-bridge/stages/` and under a
    /// specimen's `trace/`.
    ///
    /// **`Simulation` returned `None` until 2026-09-04, on the reasoning that it is "a
    /// plot rather than IR".** That distinction is real and was still the wrong line to
    /// draw: it made the plot the one pane whose data Claude cannot read, and the flat
    /// trajectories in `docs/upstream-issues.md` had to be diagnosed from a guess vector
    /// and Doug's own hedged reading of the screen. The notebook had written
    /// `simulation.json` all along, so the asymmetry was between the two *destinations*,
    /// not between IR and plots. `None` now means no such stage exists, and no stage
    /// returns it.
    ///
    /// **It is written on a different trigger from the other eleven**, which is the one
    /// hazard worth naming here: those are published by a compile, this by a *simulate*.
    /// So it is kept in step with `App::sim_data` at a single site
    /// (`App::set_sim_data`) rather than written wherever a result arrives — a
    /// `simulation.json` outliving the run that produced it would be indistinguishable
    /// from a current one.
    ///
    /// **Spelled out rather than derived from [`slug`].** A PascalCase-to-snake_case
    /// conversion happens to produce all twelve of these correctly today, which is
    /// exactly what would make a thirteenth stage's mismatch silent.
    /// `every_stage_file_name_is_in_the_canonical_list` pins these against
    /// `bridge::STAGE_FILE_NAMES` in both directions.
    ///
    /// **The `Option` now always returns `Some`, and stays anyway.** Callers already
    /// branch on it, and it is the seam a thirteenth stage would use to decline a file.
    pub fn stage_file_name(self) -> Option<&'static str> {
        Some(match self {
            StageKind::Parse => "parse.json",
            StageKind::Resolve => "resolve.json",
            StageKind::Instantiate => "instantiate.json",
            StageKind::Typecheck => "typecheck.json",
            StageKind::Flatten => "flatten.json",
            StageKind::Dae => "dae.json",
            StageKind::Structural => "structural.json",
            StageKind::IndexReduction => "index_reduction.json",
            StageKind::Initialization => "initialization.json",
            StageKind::Events => "events.json",
            StageKind::SolveLowering => "solve_lowering.json",
            StageKind::Simulation => "simulation.json",
        })
    }

    /// The crate owning the Rust type this stage's pane renders, for field help.
    ///
    /// # Why field help needs this
    ///
    /// `field_help.json` was one flat `field name -> doc` map, so a name documented in
    /// **any** harvested crate supplied the tooltip for that name in **every** stage.
    /// Hovering `names` under Solve lowering's `solver_maps` produced
    /// *"All names imported from the package"* — `rumoca-ir-ast`'s doc for an **import
    /// clause**. 66 field names are documented in two or more IR crates.
    ///
    /// # Each arm is evidence, not a guess
    ///
    /// - **Parse/Resolve/Instantiate/Typecheck** publish AST — `parse.json` is a
    ///   `StoredDefinition`.
    /// - **Flatten** publishes the flat model.
    /// - **Dae** and **Events** — both read `rumoca_ir_dae::Dae` (Events through
    ///   [`events_from_condition_algebra`]'s `DaeView`).
    /// - **SolveLowering** — `solve_lowering_stage` serialises the `SolveProblem` that
    ///   `rumoca_phase_solve::lower_solve_problem` returns, and that type lives in
    ///   `rumoca-ir-solve`. *(It named `lower_dae_to_solve_model` and `events_to_json`,
    ///   both gone in v0.10.0, until the 2026-11-07 sweep.)*
    /// - **Structural/IndexReduction** — `structural_to_json` takes
    ///   `&rumoca_phase_structural::StructuralReport`. **A phase crate, not an IR crate**,
    ///   which is why the mapping could not be assumed from the stage list.
    /// - **Initialization** is `None` **and gets its tooltips from
    ///   [`field_help::routes`](crate::field_help::routes) instead** (2026-11-08). Its
    ///   value is assembled by `initialization_from_initial_pins`, not serialised from
    ///   one type, so a crate-level preference was the wrong shape: the pins' fields live
    ///   on `InitialValuePin`, and the crate's longest doc for `scalar` is `PinTerm`'s.
    /// - **Simulation** is a plot, not IR.
    ///
    /// `every_stage_ir_crate_is_harvested_or_none` holds this against
    /// `field_help::IR_CRATES`.
    pub fn ir_crate(self) -> Option<&'static str> {
        Some(match self {
            StageKind::Parse
            | StageKind::Resolve
            | StageKind::Instantiate
            | StageKind::Typecheck => "rumoca-ir-ast",
            StageKind::Flatten => "rumoca-ir-flat",
            StageKind::Dae | StageKind::Events => "rumoca-ir-dae",
            StageKind::SolveLowering => "rumoca-ir-solve",
            StageKind::Structural | StageKind::IndexReduction => "rumoca-phase-structural",
            StageKind::Initialization | StageKind::Simulation => return None,
        })
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
        /// Every step of the matching search, **from the run that produced the
        /// blocks above** — not a re-derivation performed when the tab opens.
        ///
        /// On 0.10.0 that is true by construction rather than by care:
        /// `inspect_structural_report` returns the report and these steps from
        /// one call, through one observer threaded into the real search. There
        /// is no second traced implementation that could drift.
        matching_frames: Vec<rumoca_phase_structural::MatchingRecord>,
        /// The SCC search, from the same run, **one segment per traversal**.
        ///
        /// Segmented because node indices do not mean the same thing across
        /// traversals: `build_blt_blocks` may condense compact equation
        /// families out of the node set and run over a renumbered graph. Each
        /// segment opens with the `Start` carrying its `equation_of_node` map.
        tarjan_frames: Vec<Vec<rumoca_phase_structural::TarjanRecord>>,
        /// Tearing decisions, **one segment per coupled block**, opening with
        /// the `BlockStart` that declares the block's ordinal.
        ///
        /// A block holds up to two passes inside its segment, since the
        /// planner tries both tear priorities and keeps the smaller set; the
        /// `PlanSelected` step names the one adopted and the other describes a
        /// plan the compiler discarded. Segmenting by *pass* instead would cut
        /// a block in two, which is what every consumer addresses.
        tearing_frames: Vec<Vec<rumoca_phase_structural::TearingRecord>>,
        // **The index-reduced system's own three searches are gone with the
        // funnel.** They came from re-running reduction on a cloned DAE, which
        // 0.10.0 makes impossible (`Dae` is immutable and not `Clone`) and
        // unnecessary (`inspect_prepare_for_solve` observes the real one). The
        // re-model did not restore them: that seam records rounds, not the searches
        // inside them, so the Index Reduction tab offers no frame views. See
        // `index_reduction_stage`.
        /// Every MLS §3.7.5 history-operator site, and what each one lowered to.
        ///
        /// **Sites rather than the old frames, because the subject changed.** On
        /// 0.9.20 `lower_pre_operator(&mut Dae)` was a pass and
        /// `PreLoweringStep` named its rewrites; the field held one step per
        /// rewrite. 0.10.0 has no such pass — `pre()` is lowered inside
        /// `construction::expression`'s builtin dispatch — so a *step of a pass*
        /// is not a thing that exists to carry.
        ///
        /// What exists instead is the result of that lowering, recoverable from
        /// the finished DAE by provenance: see
        /// [`pre_lowering_from_construction`], which also records why this
        /// needed no instrumentation after both the module doc and
        /// `docs/tech-debt.md` predicted it would.
        ///
        /// Empty for a model using no history operator, which is most of them
        /// and is a report rather than a gap.
        pre_lowering_sites: Vec<crate::pre_lowering_anim::PreLoweringSite>,
        /// The flat model, the last artifact from before DAE construction runs.
        flat: Option<rumoca_ir_flat::Model>,
        /// The raw DAE, for the views that read it after the compile.
        ///
        /// **`Arc` because 0.10.0's `Dae` is not `Clone`**, and this message is.
        /// A `Dae` owns its arenas and its `SourceMap`; upstream made it
        /// immutable and valid-by-construction, and declining `Clone` is part of
        /// that — there is no legitimate reason to duplicate one. Sharing it is
        /// what the consumers actually wanted anyway: every reader goes through
        /// `inspect`, which borrows.
        ///
        /// *(The old comment said "for live-debug replay of index reduction".
        /// That consumer is gone — see `index_reduction_stage` — but the field
        /// is not: `app::tearing_dae` and the simulation path both still read
        /// it.)*
        dae: Option<std::sync::Arc<rumoca_ir_dae::Dae>>,
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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        // The downcast moved to `crate::panic_message` on 2026-11-06, when both corpus
        // runners needed the same thing and three copies were one edit away.
        .map_err(|payload| crate::panic_message(payload.as_ref()))
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
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        pre_lowering_sites: Vec::new(),
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
    /// URI of the specimen document added by the previous compile, so it can be dropped
    /// before the next one is registered.
    ///
    /// **This is the whole fix for one broken specimen poisoning every later compile**,
    /// and it is one line. Name resolution runs over the *whole session*, not just the
    /// requested model, so a specimen left behind with an unresolved reference makes a
    /// perfectly good model report that other file's error.
    ///
    /// Observed 2026-07-29 with the MSL loaded: `CapacitorLoop` clean, then
    /// `UndefinedRef`, then `CapacitorLoop` again — and the third compile reported
    /// `unresolved component reference: 'missingGain'`, a name appearing **only** in
    /// `UndefinedRef.mo`.
    ///
    /// # It was read as a Rumoca cache bug for three weeks, and was not
    ///
    /// The diagnosis was that a stale failure survived `remove_document`, logged as
    /// `docs/upstream-issues.md` #1, and mitigated by rebuilding the entire session —
    /// reloading every MSL root, at 1.95 s a time. **Both were wrong.** HRW was not
    /// removing the previous document at all, so the broken one was still in the
    /// session and Rumoca was reporting it correctly.
    ///
    /// The removal below landed 2026-08-21 for unrelated reasons and silently fixed it.
    /// The rebuild ran for five more days clearing state nothing created, because
    /// `a_broken_specimen_does_not_poison_the_next_compile` passed either way: **a
    /// mitigation and a fix produce identical green.** Withdrawn and removed 2026-08-26;
    /// `examples/repro_stale_resolve` is the evidence and would catch a recurrence.
    last_specimen_uri: Option<String>,
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
/// `parse_source_root_with_cache` is an *on-disk* cache. Every call re-runs the
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
                tolerance,
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
                let result = self.simulate(target, &model, t_end, tolerance, emit);
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
    /// 2. **Lower** — transforms the DAE into a `SolveProblem` via
    ///    `rumoca_phase_solve::lower_solve_problem()`, for the Solve lowering stage
    ///    and its counts.
    /// 3. **Integrate** — runs the solver via
    ///    `rumoca_sim::simulate_with_optional_step_records()`, which takes the DAE
    ///    itself and reports the integrator's accepted steps when the backend that
    ///    ran has them. *(Steps 2 and 3 named `lower_dae_to_solve_model` and
    ///    `simulate_solve_model`, gone in v0.10.0, until the 2026-11-07 sweep.)*
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
        tolerance_override: Option<f64>,
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
        // which the log tests never run because they compile and stop. A reader
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

        // --- Phase 2: Lower the DAE to a solve problem ---
        // Rumoca API: `lower_solve_problem` transforms the mathematical DAE
        // (equations + variables) into the solver's executable form: residual
        // programs, mass matrix structure, Jacobian sparsity pattern, etc.
        //
        // **Lowered here only to report it.** Simulation no longer consumes the
        // result: `simulate_with_diagnostics` takes the `Dae` directly on
        // 0.10.0, so this stage exists to be *shown*, and the simulate call
        // below does its own lowering internally.
        log(LogLevel::StageStart, "Solve lowering".to_owned());
        let t_stage = Instant::now();
        // The result is discarded: this call is here to **run** solve
        // lowering, so a failure is reported under its own stage and the
        // timing below measures it. The Solve Lowering tab is filled
        // separately by `solve_lowering_stage`, and simulation lowers
        // internally, so nothing downstream needs this value.
        drop(
            rumoca_phase_solve::lower_solve_problem(&cr.dae).map_err(|e| {
                log(LogLevel::Error, format!("solve lowering failed: {e}"));
                format!("solve lowering failed: {e}")
            })?,
        );
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
        let counts = crate::dae_counts::DaeCounts::of(&cr.dae);
        let has_discontinuities = counts.has_discontinuities();
        let n_states = counts.states;
        // Scalar rows: `continuous_equations` misses structured families.
        let n_eq = counts.continuous_scalar_rows;
        log(
            LogLevel::Info,
            format!("{n_eq} equations, {n_states} states, hybrid={has_discontinuities}"),
        );

        // --- Phase 3: Integrate (run the ODE/DAE solver) ---
        // Rumoca API: `simulate_solve_model` runs the solver (Auto = BDF for
        // stiff / RK45 otherwise) from t=0 to t_end, returning time series.
        // `..Default::default()` fills the remaining `SimOptions` fields
        // (max steps, output points) with sensible defaults.
        //
        // **The model's `experiment(Tolerance = …)` is honored — added 2026-09-24.**
        //
        // It was not, and that was found while opening the specimen ladder's rung 1
        // (`docs/specimen-ladder.md`). `SingleInertia`'s `phi` carries a constant
        // 9.8e-9 offset; the check on whether it comes from the order-1 startup step
        // is *"tighten the tolerance and watch it shrink"* — and HRW returned
        // byte-identical results at 1e-6, 1e-9 and 1e-12, because this struct was
        // built as `SimOptions { t_end, ..Default::default() }` and never read the
        // annotation. `rumoca-compile` extracts it (`experiment.rs`) and surfaces it
        // on the compile result, Rumoca's own CLI honors it (`--atol`/`--rtol` are
        // documented as *overriding* it), and three other in-tree consumers read it.
        // HRW was the only one that dropped it.
        //
        // **This changes no committed result.** `SimOptions::default()` is
        // `rtol = atol = 1e-6`, and all fourteen specimens that declare a tolerance
        // declare exactly `1e-6` — so the corpus matrix, the notebook traces and the
        // fidelity numbers are untouched. What changes is that an *authored* value
        // now takes effect, which is what a simulation lab station needs.
        //
        // **`t_end` is deliberately NOT taken from `experiment(StopTime)`**, nor is
        // `Interval` or `Solver`. `t_end` is HRW's own control — the caller passes it
        // from the UI — and letting an annotation win would make the Run button lie.
        // Sampling and solver choice would churn every committed trace for no
        // teaching gain. Tolerance is the one that was blocking a station.
        //
        // The filter matches the convention of the three in-tree readers
        // (`plot_compare.rs`, `rumoca-worker`, `rumoca-bind-wasm`): a tolerance is
        // used only when finite and strictly positive, and it sets *both*
        // tolerances, because `experiment` declares one number and the solver wants
        // two.
        log(LogLevel::StageStart, "Integration".to_owned());
        let t_stage = Instant::now();
        let authored_tolerance = cr
            .experiment_tolerance
            .filter(|value| value.is_finite() && *value > 0.0);
        let mut opts = rumoca_sim::SimOptions {
            t_end,
            ..Default::default()
        };
        let (tolerance, source) =
            resolve_tolerance(tolerance_override, authored_tolerance, opts.rtol);
        opts.rtol = tolerance;
        opts.atol = tolerance;
        // Say so in the log: an observatory that silently obeys a setting teaches less than one
        // that reports which number it used **and where the number came from**.
        log(
            LogLevel::Info,
            format!("tolerance {tolerance:e} {}", source.describe()),
        );
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
        // **`simulate_with_optional_step_records`, so the diagnostics plots have
        // data again** (2026-11-03, re-modelled after the v0.10.0 rebase disabled
        // them). Same dispatch as `simulate_with_diagnostics`: `Auto` resolves
        // through the same selector, so asking for the steps never changes which
        // integrator runs, and `suite_core::step_observation` pins that an observed
        // run is bit-identical to an unobserved one.
        //
        // `None` is the backend saying it has no steps to report — the rk45 path
        // never enters the ME session — and is kept distinct from `Some(empty)`,
        // which would claim a solve that took no steps.
        let (res, solver_steps) =
            match rumoca_sim::simulate_with_optional_step_records(&cr.dae, &opts) {
                Ok(pair) => pair,
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
                        let _ = rumoca_sim::simulate_with_diagnostics(&cr.dae, &opts);
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

        // **Rumoca's own per-variable facts, carried rather than re-derived.** The
        // metadata is aligned with `names` by position in `SimResult`; it is lifted
        // into two parallel vectors so `SimData` keeps no borrow and the UI thread
        // needs no solver type. `source_name()` is the solver's own rendering of a
        // coordinate — `y`, or `der(y)` for a formal derivative — so HRW invents no
        // notation for it.
        let is_state: Vec<bool> = res.variable_meta.iter().map(|m| m.is_state).collect();
        let state_coordinate: Vec<Option<String>> = res
            .variable_meta
            .iter()
            .map(|m| m.state_coordinate.as_ref().map(|c| c.source_name()))
            .collect();

        Ok(SimData {
            times: res.times,
            names: res.names,
            data: res.data,
            n_states: res.n_states,
            is_state,
            state_coordinate,
            has_discontinuities,
            solver_steps: solver_steps_from_integrator(solver_steps.as_deref()),
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
    /// Rumoca API: `strict_compile_resolved()` returns the planning tree the
    /// compile itself uses, and `tree()` borrows the `ClassTree` out of it.
    /// `get_class_by_qualified_name` does a name-based lookup. The `{e:#}` format
    /// uses Rust's "alternate" `Display`, which includes the full error chain
    /// (like Python's chained exceptions).
    ///
    /// **Recovery mode, not `session.resolved()`** — changed 2026-11-03 for the same
    /// reason as [`Self::locate_library_model`]. `resolved()` builds
    /// `ResolveBuildMode::Standard`, which treats the 0.10.0 MSL's ~75 resolve
    /// errors as fatal, so **navigating to any library class failed**: the test
    /// asking for `Modelica.Mechanics.Rotational.Components.Inertia` got back the
    /// whole stringified diagnostic chain instead of a class. Clicking a
    /// `type_def_id` in the tree inspector would have done the same thing.
    ///
    /// Navigation is also the case where refusing is least defensible: the reader
    /// has asked to look at a declaration, and whether some other file in the
    /// library has an error has nothing to do with whether this class can be shown.
    fn open_def(&mut self, name: &str) -> FromWorker {
        let rt = match self.session.strict_compile_resolved() {
            Ok((rt, _)) => rt,
            Err(e) => {
                return FromWorker::DefTree {
                    name: name.to_owned(),
                    result: Err(format!("{e:#}")),
                };
            }
        };
        let result = match rt.tree().get_class_by_qualified_name(name) {
            Some(class) => {
                let value = ser_value(class);
                // Build a DefId→DefInfo map for all DefIds referenced in this
                // class's IR, so navigation can continue from here.
                let def_index = build_def_index(
                    rt.tree(),
                    &value,
                    &source_names(&self.session.document_uris()),
                );
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
        // A fresh session holds no specimen document, so the tracker resets with it.
        self.last_specimen_uri = None;
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
    /// 7. **Index reduction** — `rumoca_phase_structural::inspect_prepare_for_solve()` — the reduction search
    /// 8. **Initialization** — the same preparation's initial-value pins
    /// 9. **Events** — the DAE's condition algebra, read through `DaeView::conditions` and its siblings
    /// 10. **Solve lowering** — `rumoca_phase_solve::lower_solve_problem()` — DAE to solve problem
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
        // **Recovery mode, not `resolved()`, and v0.10.0 forces the choice.**
        //
        // `Session::resolved()` builds `ResolveBuildMode::Standard`, which treats as
        // fatal exactly the errors `StrictCompileRecovery` tolerates. The 0.10.0 MSL
        // emits about 75 resolve errors over the `Modelica.Mechanics` friction state
        // machines, so `Standard` fails on the library as a whole — and **every
        // library-model lookup failed with it**, whatever the model. `CriticalDamping`
        // lives in `Modelica/Blocks/Continuous.mo` and has nothing to do with
        // friction, but it was unreachable all the same.
        //
        // Recovery is also the mode the compile below actually runs in, so this is
        // the tree the lookup should have consulted regardless: locating a class in
        // one tree and compiling it in another is how the two could disagree about
        // where it is declared. And it is cheaper — `resolved()` ends in a deep clone
        // of the whole resolved tree (~487ms here, 38,855 defs), where this borrows
        // the `Arc` the session already holds.
        //
        // **The diagnostics are deliberately dropped here.** This function locates a
        // class; whether the model *resolves* is the Resolve stage's claim to make,
        // and it makes it per model — see `resolve_diagnostics_indicate_failure`. A
        // lookup that refused on a library-wide error would be the same attribution
        // bug one layer down.
        let (tree, _) = self
            .session
            .strict_compile_resolved()
            .map_err(|e| format!("cannot resolve the library to find `{qualified}`: {e:#}"))?;
        let class = tree
            .tree()
            .get_class_by_qualified_name(qualified)
            .ok_or_else(|| format!("`{qualified}` is not a class in the loaded libraries"))?;
        // **`Location` names a `SourceId` on 0.10.0, not a file.** The id is
        // derived from the URI by `source_id_for_name`, so hashing the session's
        // own document URIs recovers the name by identity rather than by
        // matching text. A class whose declaring document the session does not
        // hold falls through to the same error the missing-document case below
        // reports, which is the honest outcome either way.
        let uri = source_names(&self.session.document_uris())
            .get(&class.location.source)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "`{qualified}` is declared in a document the session does not hold \
                     (source id {:?})",
                    class.location.source,
                )
            })?;
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
        trace: &crate::live_trace::LiveTrace<
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
                    matching_frames: Vec::new(),
                    tarjan_frames: Vec::new(),
                    tearing_frames: Vec::new(),
                    pre_lowering_sites: Vec::new(),
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
        // **The URI is a filesystem path** (`collect_modelica_files` run it),
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
        // **The session rebuild that used to sit here is gone — 2026-08-26.**
        //
        // It rebuilt the whole `Session` and reloaded every MSL root whenever the
        // previous compile had failed to resolve, on the belief that a stale failure
        // survived `remove_document` — `upstream-issues.md` #1, now withdrawn.
        //
        // **There was no stale failure.** HRW was not removing the previous specimen's
        // document at all, so the broken one was still in the session; a `Session`
        // resolves every document it holds, found the unresolved reference, and
        // reported it correctly. The fix is the removal immediately below, added
        // 2026-08-21 for unrelated reasons. `examples/repro_stale_resolve` shows both
        // shapes: with the removal it is clean in four configurations, without it the
        // symptom appears at once.
        //
        // **Nothing noticed for five days**, because `a_broken_specimen_does_not_poison
        // _the_next_compile` passed on both sides of that change — via the workaround
        // before, and because the defect was gone after. A mitigation and a fix produce
        // identical green.
        //
        // **Only a specimen is registered.** A library model's document is
        // already in a durable source root; adding it as a workspace document
        // would have the session hold the same file twice, and removing it later
        // would evict part of the library.
        if given_qualified.is_none() {
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
                // Built before the `match` below borrows the session's tree, so
                // the URI index and that borrow do not overlap.
                let names = source_names(&self.session.document_uris());
                let strict = self.session.strict_compile_resolved();
                let strict = match strict {
                    Ok((rt, diags)) if resolve_diagnostics_indicate_failure(&diags, &uri) => {
                        // Recovered a tree, but this model did not resolve. Take the
                        // failure path, exactly as `tree()`'s `Err` used to.
                        let _ = rt;
                        // **The note lists this model's errors only.** It used to join
                        // every error in the session, so a one-line mistake in a
                        // specimen arrived buried in ~75 MSL diagnostics about friction
                        // state machines the reader never mentioned.
                        Err(anyhow::anyhow!(
                            "Resolve errors: {}",
                            model_resolve_errors(&diags, &uri)
                                .iter()
                                .map(|d| d.message.as_str())
                                .collect::<Vec<_>>()
                                .join("; "),
                        ))
                    }
                    Ok((rt, _)) => Ok(rt),
                    Err(e) => Err(e),
                };
                match strict.as_ref() {
                    Ok(strict) => {
                        // `StrictCompileTree::tree()` borrows the `Arc` the
                        // session already holds, so this is the whole resolved
                        // tree without a copy of it. On 0.9.20 the same thing
                        // was spelled `Arc<ResolvedTree>::inner()`.
                        let rt = strict.tree();
                        // Extract just this model's class definition from the
                        // full resolved tree (which includes the entire MSL).
                        let stage = extract_class(rt, &qualified);
                        if let Some(v) = &stage.value {
                            // Build the DefId→DefInfo lookup for all DefIds
                            // referenced in this model's IR.
                            def_index = build_def_index(rt, v, &names);
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
                        // Kept whole: its `source_map` lets a library label resolve
                        // in its own file (see `diagnostics_to_json`).
                        let model_diags = self.session.compile_model_diagnostics(&qualified);
                        let diag = model_diagnostics_to_json(
                            &model_diags.diagnostics,
                            model_diags.source_map.as_ref(),
                            &source,
                            &uri,
                        );
                        let resolve_err = |note: &str| {
                            serde_json::json!({
                                "kind": "resolve",
                                "message": note,
                                "diagnostics": diag,
                                "guidance": "Name resolution binds every reference to a definition. \
                                 Read `diagnostics.errors` first: those are this model's problems, \
                                 each with the source location of the reference that failed. \
                                 `diagnostics.warnings` are library-level and almost never the \
                                 cause.",
                            })
                        };
                        match self.session.resolved_cached() {
                            Some(rt) => match extract_class(rt.inner(), &qualified) {
                                Stage {
                                    value: Some(mut v), ..
                                } => {
                                    def_index = build_def_index(
                                        rt.inner(),
                                        &v,
                                        &source_names(&self.session.document_uris()),
                                    );
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
            compiled_dae,
            compiled_flat,
            structural_frames,
            pre_lowering_sites,
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
                    // equation_sheet, identifier_index, compiled_dae,
                    // compiled_flat — nothing compiled, so there is nothing to
                    // carry.
                    None,
                    None,
                    None,
                    None,
                    // **One `StructuralFrames`, not two.** This arm still had
                    // the 0.9.20 shape: two frame sets (raw and index-reduced)
                    // plus two `Vec`s for frame kinds that no longer exist. It
                    // only compiled because nothing had re-counted it since the
                    // reduced system's frames were removed.
                    StructuralFrames::default(),
                    // No DAE, so no history-operator site. Empty here is the
                    // same answer a smooth model gives, and the Events-stage
                    // outcome is what says the pipeline stopped at parse.
                    Vec::new(),
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
                        let diag_json = diagnostics_to_json(
                            &typed.typecheck_diagnostics,
                            SpanFile::specimen(&uri, &source),
                            // No map on this path; see `diagnostics_to_json`.
                            None,
                        );
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
                    flatten_stage(result, SpanFile::specimen(&uri, &source)),
                    flatten,
                    flatten_traces
                );

                // **DAE construction, logged in its true position.** Until
                // 2026-08-04 this stage was built *after* solve lowering and
                // never logged at all — so the log showed the chain jumping
                // Flatten → Structural, with the phase they both depend on
                // missing. Doug found it running the lab that teaches it.
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
                // end, and the lab that found this
                // (`docs/fixture-labs/dae-construction.md`) runs exactly that
                // path.
                let dae_stage = run_stage!(
                    "DAE construction",
                    match &dae {
                        Some(d) => {
                            let counts = crate::dae_counts::DaeCounts::of(d);
                            let n_x = counts.states;
                            let n_y = counts.algebraics;
                            let n_eq = counts.continuous_equations;
                            let mut s = Stage::from_ser(d);
                            // A `Dae` owns its source map, and that map serializes
                            // every registered file's full text — the whole vendored
                            // MSL, 15 MiB per trace and absolute home paths in a
                            // public repository. Swap it for an index of the files
                            // this DAE's spans actually name.
                            if let Some(value) = s.value.as_mut() {
                                replace_source_map_with_a_portable_index(value, d.source_map());
                            }
                            // Rumoca's arena names, plus the structured families the
                            // `continuous_equations` arena does not hold — without them
                            // `ArrayFamily` read "0 continuous equation(s)" for a model
                            // with eight (2026-11-07). Unchanged when there are none.
                            let families = match counts.continuous_families {
                                0 => String::new(),
                                n_f => format!(
                                    " and {n_f} structured famil{} \u{2014} {} scalar row(s)",
                                    if n_f == 1 { "y" } else { "ies" },
                                    counts.continuous_scalar_rows,
                                ),
                            };
                            s.note = Some(format!(
                                "{n_x} state(s), {n_y} algebraic(s), {n_eq} continuous \
                                 equation(s){families}",
                            ));
                            // Built from the same `d` the tree will render, one line
                            // apart, so the two cannot describe different IR.
                            s.rendered = crate::equation_text::for_dae(d);
                            s
                        }
                        None => dae_absent_stage(result, SpanFile::specimen(&uri, &source)),
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
                    let (stage, frames) = structural_stage(result);
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
                // **One preparation serves both stages**, which is what 0.10.0 makes
                // possible: `inspect_prepare_for_solve` reduces the system and
                // returns both the prepared result and the report of how it got
                // there. Running it twice would be two reductions of one compile,
                // and nothing would make them agree — so it runs here, outside both
                // `run_stage!` brackets, and each stage reads the same observation.
                //
                // Outside the brackets rather than inside the first, because
                // charging the whole cost to Index reduction and nothing to
                // Initialization would be a timing claim neither stage can support.
                let observed_reduction = match result {
                    Some(PhaseResult::Success(cr)) => {
                        let (prepared, report) =
                            rumoca_phase_structural::inspect_prepare_for_solve(&cr.dae);
                        Some(ObservedReduction { prepared, report })
                    }
                    _ => None,
                };
                // `mut` because solve lowering, built below, can overturn this stage's
                // outcome — see `reconcile_reduction_with_lowering`, which is called as
                // soon as both exist.
                let mut index_reduction = run_stage!(
                    "Index reduction",
                    index_reduction_stage(
                        result,
                        observed_reduction.as_ref(),
                        structural.value.as_ref()
                    ),
                    index_reduction
                );
                let initialization = run_stage!(
                    "Initialization",
                    initialization_stage(result, observed_reduction.as_ref()),
                    initialization
                );
                let events = run_stage!("Events", events_stage(result), events);
                // **Read from the DAE every stage above shows, not from a second
                // construction run.** The provenance identifying a history
                // operator's synthesised nodes survives into the artifact, so
                // this is an observation of the compile rather than a replay of
                // it — see `pre_lowering_from_construction`, which records how
                // that was measured.
                let pre_lowering_sites = match result {
                    Some(PhaseResult::Success(cr)) => cr
                        .dae
                        .inspect(|view| pre_lowering_from_construction(view, cr.dae.source_map())),
                    _ => Vec::new(),
                };
                let solve_lowering = run_stage!(
                    "Solve lowering",
                    solve_lowering_stage(result),
                    solve_lowering
                );
                // **Both stages exist for the first time here**, which is the earliest
                // point this judgement can be made without re-running a phase or
                // reordering the two. See `reconcile_reduction_with_lowering`.
                reconcile_reduction_with_lowering(&mut index_reduction, &solve_lowering);

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
                    dae,
                    flat,
                    structural_frames,
                    pre_lowering_sites,
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
            matching_frames: structural_frames.matching,
            tarjan_frames: structural_frames.tarjan,
            tearing_frames: structural_frames.tearing,
            pre_lowering_sites,
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

/// Where the tolerance a run used actually came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToleranceSource {
    /// Chosen in HRW's Simulation pane, overriding whatever the model says.
    Chosen,
    /// Read from the model's `experiment(Tolerance = …)`.
    Annotation,
    /// Neither was supplied; the solver's own default applies.
    SolverDefault,
}

impl ToleranceSource {
    /// The phrase the log appends after the number.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Chosen => "chosen in HRW (overriding the model)",
            Self::Annotation => "from the model's experiment annotation",
            Self::SolverDefault => "(solver default — the model declares none)",
        }
    }
}

/// Which tolerance a run uses, and where it came from.
///
/// # The precedence, and why it is this way round
///
/// **Chosen in HRW beats the model's annotation, which beats the solver default.** That ordering
/// is not invented here: Rumoca's own CLI documents `--atol`/`--rtol` as *overriding* the model's
/// `experiment(Tolerance=…)`, so HRW's control behaves the way the compiler's own front end
/// already does. A reader who learns one has learned the other.
///
/// # Why the source is returned rather than just the number
///
/// **A tolerance with no provenance is the shape of defect this pane keeps producing.** The plot
/// tooltips showed correct numbers rounded into uselessness; the field named `h` meant a different
/// step than the one it sat on. Here the hazard is subtler still: three plausible sources agree
/// most of the time, so a run silently using the wrong one looks exactly like a run using the
/// right one. Doug, 2026-09-25, could not tell which tolerance produced the trajectory in front of
/// him, because nothing said.
///
/// **A non-finite or non-positive override or annotation is ignored rather than honoured** — it
/// would poison the solver's error control, and "the number I typed did nothing" is a better
/// failure than a silent non-convergence.
pub fn resolve_tolerance(
    chosen: Option<f64>,
    annotated: Option<f64>,
    solver_default: f64,
) -> (f64, ToleranceSource) {
    let usable = |v: Option<f64>| v.filter(|x| x.is_finite() && *x > 0.0);
    if let Some(t) = usable(chosen) {
        (t, ToleranceSource::Chosen)
    } else if let Some(t) = usable(annotated) {
        (t, ToleranceSource::Annotation)
    } else {
        (solver_default, ToleranceSource::SolverDefault)
    }
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
    // `None`: the headless path never overrides, so a specimen's own annotation (or the solver
    // default) decides, exactly as it did before HRW gained a control for it.
    state.simulate(
        CompileTarget::File(specimen),
        model,
        t_end,
        None,
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
        // As in `simulate_specimen`: headless runs never override.
        None,
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
    format!("{NOT_REACHED_PREFIX}{because})")
}

/// The words that mean *it did not run*, shared by the writer above and
/// [`Stage::says_not_reached`] which reads them back.
///
/// **The reading side was unsingle-sourced until 2026-11-05**, which is the same slow
/// leak [`not_reached_note`] was built to stop, one direction over:
/// `fidelity::tests::a_stage_that_says_it_never_ran_shows_no_ir` matched `"not reached"`
/// by hand, and `App::failure_context` did not test for it at all — it used
/// `value.is_none()`, which is a **different fact**. See [`Stage::says_not_reached`].
const NOT_REACHED_PREFIX: &str = "not reached (";

/// One plot row per accepted step the integrator took.
///
/// **This is the symbol `docs/tech-debt.md`'s `unbuilt:` tag recruited**, so
/// `doc_citations::claims_of_absence_are_still_true` fails once this exists and
/// forces that row to be retired — which is the mechanism working rather than a
/// coincidence of naming.
///
/// `None` means the backend that ran reports no steps: the rk45 path never
/// enters the ME session, so there is nothing to observe. That is kept distinct
/// from an empty slice, which would be a claim about a solve that took no
/// steps. Both map to an empty plot, and the Simulation pane says which it is.
///
/// # What each column is, and why `h` is computed here
///
/// `t` is the time the step **reached**, so a reader comparing the diagnostics
/// plot against the trajectory finds a step where the trajectory has a value.
/// `h` is the interval it covered, derived from the two endpoints the solver
/// reported rather than read from a third field — a stored size could disagree
/// with the interval it describes, and `MeStepRecord::step_size` exists so that
/// the subtraction happens once.
fn solver_steps_from_integrator(
    records: Option<&[rumoca_solver::fmi_me::step_observation::MeStepRecord]>,
) -> Vec<crate::plot_labels::StepRow> {
    records
        .unwrap_or_default()
        .iter()
        .map(|step| crate::plot_labels::StepRow {
            t: step.accepted_time,
            h: step.step_size(),
            order: step.order as usize,
        })
        .collect()
}

/// The sentence fragment a stage disabled for a re-model puts in its note.
///
/// Matched on rather than reconstructed, so a test can find such stages by what they
/// *say* instead of being told which they are — see [`AWAITING_REMODEL`], whose
/// roster is checked against this in both directions.
///
/// **Nothing produces this string any more.** `awaiting_remodel_note` was its only
/// writer and went with the last re-model on 2026-11-04. The string stays because it
/// is the wording the **next** rebase's disabled stage must use, and because
/// [`AWAITING_REMODEL`]'s roster test is what makes a new one visible — a disabled
/// stage that invents its own wording would be invisible to that check.
///
/// **`#[cfg(test)]`, because production has no use for it**, and a production reader
/// would be a second place for the list to drift.
#[cfg(test)]
pub(crate) const AWAITING_REMODEL_MARKER: &str = "HRW cannot display it yet";

/// **The stages Rumoca runs and HRW cannot yet show. Empty since 2026-11-04.**
///
/// All five re-models the v0.10.0 rebase recruited have landed. **The mechanism is
/// kept and stays armed**, because an empty roster is not a dead check — it is the
/// complementary claim, and a stronger one:
///
/// - `the_remodel_roster_matches_what_the_pipeline_shows` now asserts that **no
///   stage claims [`AWAITING_REMODEL_MARKER`]**, so a future rebase cannot disable a
///   stage without rostering it here.
/// - `the_tests_blocked_on_a_remodel_are_still_blocked` now asserts that **no test is
///   still `#[ignore]`d for a re-model**. That is what forced the eight blocked tests
///   to be individually restored, re-keyed or retired in the commit that emptied
///   this, rather than left switched off behind a green gate.
/// - [`crate::fidelity`] filters its stage sweep by this list, so emptying it put
///   **every** stage back under fidelity's invariants.
///
/// Deleting it instead would have cost about twenty doc comments their citation of
/// it — and those are *history*, true statements about the day a stage left this
/// list. A symbol naming something in the past does not move when the present
/// changes.
///
/// # Why a roster exists at all
///
/// Three things need to agree: the code that emits the notes, the tests that cover
/// the old IR shape (now `#[ignore]`d, because their bodies are the specification
/// the re-model has to satisfy), and the documents that claim the absence. **A list
/// nothing checks would rot in the usual direction** — a stage comes back, and the
/// tests that would have proved it correct stay switched off, silently.
///
/// So `worker::tests::the_remodel_roster_matches_what_the_pipeline_shows` checks
/// this against real compiled output **in both directions**, and
/// `the_tests_blocked_on_a_remodel_are_still_blocked` fails the moment a stage
/// leaves this list while its tests are still ignored — which is what turns
/// re-enabling a stage into a list of work rather than a thing to remember.
/// **`#[cfg(test)]` for the same reason as [`AWAITING_REMODEL_MARKER`]**: nothing
/// in production branches on this list, and a production reader would be one more
/// copy to keep true. Its whole job is to let two tests disagree with reality
/// loudly.
#[cfg(test)]
/// *Initialization left this list on 2026-11-03*, re-modelled onto
/// `initial_pins` — see [`initialization_from_initial_pins`]. Its three blocked
/// tests were restored in the same commit, which
/// `worker::tests::the_tests_blocked_on_a_remodel_are_still_blocked` required
/// rather than trusted.
///
/// *Events left it the same day*, re-modelled onto the condition algebra — see
/// [`events_from_condition_algebra`]. **Two of its three blocked tests were
/// restored and the third was retired**, which is the outcome worth noting here:
/// `events_ir_has_expected_summary_keys` asserted six 0.9.20 key names and
/// `every_events_rendering_lands_on_a_real_node` resolved a side map the re-model
/// does not build, so neither body was still a specification. **A blocked test's
/// body is the specification only while the data model it describes is the one
/// being re-modelled onto** — restoring one past that point would pin 0.9.20's
/// shape, which is the opposite of what the roster is for.
///
/// *Index reduction left it on 2026-11-04*, re-modelled onto the observed reduction
/// — see [`index_reduction_from_observation`] — and emptied it. Of its eight blocked
/// tests, **four were restored, one was re-keyed to the opposite claim, and three
/// were retired**; the re-keyed one is the finding, because 0.10.0's holonomic lane
/// *does* now rescue `CapacitorLoop`, so a body asserting that reduction cannot was
/// no longer a specification but a false premise.
pub(crate) const AWAITING_REMODEL: &[StageKind] = &[];

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
/// # A library label resolves to `null`, and that is now true rather than claimed
///
/// This comment used to say so already: *"the offsets are into that file, not the
/// specimen, and `span_to_location` refuses rather than inventing a line."* **It did
/// not refuse.** Its only guard was `end > bytes.len()`, so a library span whose byte
/// range happened to fit inside the specimen resolved against the wrong file and
/// reported a line with full confidence. The same mistake was *live* next door, in
/// `unmatched_unknown_locations`; [`span_location`] records what it published.
///
/// Byte offsets cannot say which file they are offsets *into*, so the only way to
/// refuse is to be told, which is what [`SpanFile`] is for. This path is reached from
/// `PhaseResult::Failed`, where there is no `Dae` and therefore no
/// `Dae::source_map` — so it is told the **specimen** and refuses everything else.
/// That is weaker than the IR sites, which resolve a library span in its own file,
/// and it is the honest version of what this comment always said.
///
/// `a_library_label_has_no_location` is the check, with
/// `the_span_resolver_refuses_a_foreign_span` on the guard itself.
///
/// # `map`: a library label resolves in ITS OWN file when the caller has the files
///
/// *2026-11-07, the post-rebase sweep's open item 3.* The Resolve path calls
/// `compile_model_diagnostics`, whose `ModelDiagnostics` carries the compile's
/// `source_map` — and HRW read `.diagnostics` off it and dropped the map, so a library
/// label there was `null` although its file was in hand. It now passes `Some(map)`, and
/// a span whose id is not the specimen's is looked up in it through
/// [`SpanFile::from_map`], which is identity, never a guess. **The three
/// `PhaseResult::Failed` callers pass `None`**: they hold no map, and the only one
/// reachable is `resolved_cached()`, which clones the whole resolved tree, MSL
/// included — too much to pay on every failure for a secondary label. They keep the
/// refusal. (The item was logged as *"resolves its span against the specimen"*; it did
/// not — the 11-04 identity guard refuses — so what was missing was a location, not a
/// wrong one.) `a_library_label_resolves_in_its_own_file_when_the_map_is_given` is the
/// check.
fn diagnostics_to_json(
    diags: &[rumoca_core::Diagnostic],
    specimen: SpanFile<'_>,
    map: Option<&rumoca_core::SourceMap>,
) -> serde_json::Value {
    // The specimen first, then the compile's own map; never the specimen's text for a
    // span that names another file.
    let locate = |span: &rumoca_core::Span| {
        if span.source == specimen.id {
            span_to_location(specimen, span)
        } else {
            span_to_location(SpanFile::from_map(map?, span.source)?, span)
        }
    };
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
                        "location": locate(&l.span),
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
/// **`errors` is split by source as of 2026-11-03, and the old shape made its own
/// `guidance` false.**
///
/// This function already separated library *warnings* from the model's problems,
/// on the premise that errors are always the model's — true on 0.9.20, where the
/// MSL emitted none. v0.10.0's MSL emits about 75 resolve errors, so `errors`
/// carried them, while the Resolve stage's `guidance` told the reader *"those are
/// this model's problems"*. **`UndefinedRef` reported its one real mistake behind
/// 75 messages about friction state machines**, and the text beside them said to
/// read those first.
///
/// So the library's errors move to `library_errors`, deduplicated and counted the
/// way the warnings already are — **kept, not dropped**, because this JSON is read
/// by Claude and an error the compiler reported is evidence even when it is not
/// this model's. The split is by [`diagnostic_source`], which is identity.
fn model_diagnostics_to_json(
    diags: &[rumoca_core::Diagnostic],
    map: Option<&rumoca_core::SourceMap>,
    source: &str,
    uri: &str,
) -> serde_json::Value {
    let own = rumoca_core::source_id_for_name(uri);

    let (all_errors, warnings): (Vec<_>, Vec<_>) = diags
        .iter()
        .cloned()
        .partition(|d| matches!(d.severity, rumoca_core::DiagnosticSeverity::Error));
    let (errors, library_errors): (Vec<_>, Vec<_>) = all_errors
        .into_iter()
        .partition(|d| diagnostic_source(d) == Some(own));

    /// Collapse repeats to one row with a count: the same MSL complaint arrives
    /// dozens of times and repeating it dozens of times helps nobody.
    fn distinct(of: &[rumoca_core::Diagnostic]) -> Vec<serde_json::Value> {
        let mut seen = std::collections::BTreeMap::<String, usize>::new();
        for d in of {
            *seen.entry(d.message.clone()).or_insert(0) += 1;
        }
        seen.into_iter()
            .map(|(message, count)| serde_json::json!({ "message": message, "occurrences": count }))
            .collect()
    }

    serde_json::json!({
        "errors": diagnostics_to_json(&errors, SpanFile::specimen(uri, source), map),
        "library_errors": {
            "note": "errors the compiler reported against LIBRARY sources, deduplicated. \
                     Not this model's problems and not why this compile failed \u{2014} \
                     v0.10.0 rejects some MSL code that 0.9.20 accepted, which is a \
                     question for upstream rather than a fault in the model above.",
            "total": library_errors.len(),
            "distinct": distinct(&library_errors),
        },
        "warnings": {
            "note": "library-level warnings, deduplicated. Almost always about the MSL rather \
                     than the model, and not the reason the compile failed.",
            "total": warnings.len(),
            "distinct": distinct(&warnings),
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
    specimen: SpanFile<'_>,
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
            diagnostics_to_json(diagnostics, specimen, None),
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
/// Matches `"unbalanced model: {e} equations, {u} unknowns (balance = {b})"`, and
/// tolerates anything Rumoca appends after the closing parenthesis. Returns `None`
/// on any other deviation — see the caller on why absent beats wrong.
///
/// # The suffix stopped being the end of the message on v0.10.0
///
/// This read the balance with `strip_suffix(')')`, which requires the parenthesis to
/// be the **last character**. v0.10.0 continues past it with a role breakdown:
///
/// ```text
/// unbalanced model: 2 equations, 3 unknowns (balance = -1); unknowns[state=2
/// algebraic=1 output=0 discrete_real=0 discrete_value=0] equations[continuous=2
/// discrete_real=0 b1c_definition=0]
/// ```
///
/// So the parse declined on every unbalanced model and the pane lost `n_equations`,
/// `n_unknowns`, `balance` and the `reading` sentence. **It failed in the designed
/// direction** — absent, never wrong — which is why this is a one-line correction
/// rather than an incident.
///
/// **The breakdown is deliberately not parsed.** `message` is emitted verbatim, so
/// the counts are already there for a reader, and a second parse is a second thing
/// to rot. It is also the better information: on a failed DAE construction there is
/// no `Dae` for `dae_counts` to count, so this sentence is the only account of what
/// the unknowns *were*.
fn parse_unbalanced(message: &str) -> Option<(usize, usize, i64)> {
    let rest = message.strip_prefix("unbalanced model: ")?;
    let (eq, rest) = rest.split_once(" equations, ")?;
    let (unk, rest) = rest.split_once(" unknowns (balance = ")?;
    let (bal, _after) = rest.split_once(')')?;
    Some((
        eq.trim().parse().ok()?,
        unk.trim().parse().ok()?,
        bal.trim().parse().ok()?,
    ))
}

/// One source file, named by the id Rumoca addresses it with.
///
/// **A span may only be resolved against the file its own `SourceId` names**, and
/// carrying the id beside the text is what makes that checkable instead of assumed.
/// Before 2026-11-04 the resolver took a bare `&str` and sliced whatever it was
/// handed; [`span_location`] records what that cost.
#[derive(Clone, Copy)]
struct SpanFile<'a> {
    id: rumoca_core::SourceId,
    /// The name the compile registered, published so a line number says which file
    /// it counts lines in.
    name: &'a str,
    text: &'a str,
}

/// A source name fit to **publish**: workspace-relative, forward slashes.
///
/// Rumoca registers each source under the path HRW handed it, so a name read back out
/// of a `SourceMap` is an absolute path on *this* machine — `C:\Users\…\rumoca\hrw/
/// vendor/msl\Modelica 4.1.0\…`. Two separate reasons it cannot be published that way:
///
/// - **This repository is public**, and an absolute path under a home directory
///   carries the account name of whoever ran the compile into every committed trace.
/// - **A committed trace has to mean the same thing on another machine**, and a
///   checker that reads the named file cannot find it under someone else's prefix.
///
/// A path *outside* the workspace — a scratch specimen under the system temp
/// directory — keeps only its last two components. Those locate the file for a reader
/// who already has it (Claude wrote it) while leaving the parent chain, which is where
/// an account name sits, off the record.
fn portable_source_name(name: &str) -> String {
    let normalized = name.replace('\\', "/");
    // The workspace root is the parent of this crate's directory. Baked in at compile
    // time, and never itself published.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.to_string_lossy().replace('\\', "/"));
    if let Some(root) = root {
        // Windows paths compare case-insensitively, and the two halves of a name can
        // disagree in case when they were joined by different code.
        let (lower, root_lower) = (normalized.to_lowercase(), root.to_lowercase());
        if let Some(rest) = lower.strip_prefix(&root_lower) {
            // Slice the original by the matched length so the published spelling is
            // the real one rather than the lowercased comparison key.
            let kept = &normalized[normalized.len() - rest.len()..];
            return kept.trim_start_matches('/').to_owned();
        }
    }
    let components: Vec<&str> = normalized.rsplit('/').take(2).collect();
    components.into_iter().rev().collect::<Vec<_>>().join("/")
}

/// Collect every `SourceId` a serialized span names, anywhere in the IR.
///
/// `rumoca_core::Span` is `{source, start, end}` and its field names are its
/// own, so **every serialized span has a `source` key** whatever struct contains
/// it. That is what makes the result complete with respect to the published
/// document rather than a guess about where spans live — the same reasoning
/// [`collect_def_ids`] rests on, one key instead of three.
fn collect_span_sources(v: &serde_json::Value, out: &mut BTreeSet<u64>) {
    match v {
        serde_json::Value::Object(map) => {
            for (k, val) in map {
                if k == "source"
                    && let Some(n) = val.as_u64()
                {
                    out.insert(n);
                }
                collect_span_sources(val, out);
            }
        }
        serde_json::Value::Array(arr) => arr.iter().for_each(|val| collect_span_sources(val, out)),
        _ => {}
    }
}

/// Replace a serialized `Dae`'s source map with an index of the files its spans
/// name, carrying no source text and no absolute path.
///
/// # What it is replacing, and why that had to change
///
/// A `Dae` owns its [`rumoca_core::SourceMap`], and that map's `Serialize` emits
/// `files: Vec<(SourceId, String, Arc<str>)>` — **every registered file's full
/// text.** HRW registers the whole vendored MSL as one source root, so every
/// `dae.json` carried the entire library: measured 2026-11-04 across the
/// notebook, **2556 files and 15.25 MiB of embedded text per trace, 342.6 MiB
/// over 24 files, of which 320.0 MiB (93.4 %) was that text.** The names were
/// absolute paths under a home directory, 2556 of them per file, and **this
/// repository is public.**
///
/// Doug ruled on 2026-11-04: drop the text, publish names relative to the
/// workspace. Stripping the text alone is upstream's own practice —
/// `rumoca-worker` maps its map through
/// [`rumoca_core::SourceMap::without_source_contents`] before sending it — but
/// that keeps the names, which is the half that carries the account name.
///
/// # Why this is not Rumoca's wire shape with the names edited
///
/// Deserializing a `SourceMap` runs `checked_source_indexes`, which **requires
/// `source_id_for_name(name) == id`** (SPEC_0029 §3a) — a source id *is* a hash
/// of its name. So a portable name beside the real id is a triple Rumoca would
/// reject, and recomputing the id to match would detach it from every span in
/// the document. Either way the wire shape would be a lie, so this publishes an
/// **HRW-shaped node that says so in its own text** rather than a Rumoca-shaped
/// one that does not round-trip.
///
/// # Why the referenced subset rather than the whole registry
///
/// The node exists so a reader can turn a span's `source` into a file. Measured
/// on the committed corpus: **2556 files registered, 1 to 28 referenced**, and
/// **nothing unresolvable** — `SingleInertia` names exactly one file, its own.
/// Publishing all 2556 would restate the MSL's directory listing 22 times and
/// explain no span that this does not. `registered_files` keeps the count that
/// is genuinely about the compile.
///
/// # The silent failure this returns `false` for
///
/// `source_map` is a key name `Dae`'s `Serialize` owns, not HRW's. If upstream
/// renames it, this finds nothing, replaces nothing, and **15 MiB of library
/// text returns to every trace with nothing on screen looking different** — the
/// project's usual shape of defect. `the_dae_stage_indexes_its_sources_and_publishes_no_source_text`
/// fails by name on that.
fn replace_source_map_with_a_portable_index(
    value: &mut serde_json::Value,
    map: &rumoca_core::SourceMap,
) -> bool {
    if value.get("source_map").is_none() {
        return false;
    }
    // Scan before replacing. Rumoca's node holds its entries as arrays, so it
    // contributes no `source` key of its own and cannot seed the set with ids
    // nothing points at.
    let mut ids = BTreeSet::new();
    collect_span_sources(value, &mut ids);
    let files: Vec<serde_json::Value> = ids
        .iter()
        .filter_map(|&id| {
            let id = rumoca_core::SourceId(id);
            let (name, text) = map.get_source(id)?;
            Some(serde_json::json!({
                "source_id": id.0,
                "file": portable_source_name(name),
                "content_bytes": text.len(),
            }))
        })
        .collect();
    value["source_map"] = serde_json::json!({
        "hrw_note": "Published by HRW, not by Rumoca's Serialize. Rumoca's node \
             carries every registered file's full text; this one carries the \
             id-to-file index for the files this DAE's spans name, and no text. \
             Names are workspace-relative, so source_id_for_name(file) does NOT \
             equal source_id the way Rumoca's own node requires (SPEC_0029 §3a) \
             — resolve a span by its source_id, never by re-deriving one from \
             the name.",
        "registered_files": map.source_ids().len(),
        "files": files,
    });
    true
}

impl<'a> SpanFile<'a> {
    /// The specimen, identified the way the session registers it.
    ///
    /// `source_id_for_name` is a pure function of the name — the same derivation
    /// `model_resolve_errors` uses to tell this model's diagnostics from the
    /// library's — so no map is needed and the id cannot go stale.
    fn specimen(uri: &'a str, text: &'a str) -> Self {
        Self {
            id: rumoca_core::source_id_for_name(uri),
            name: uri,
            text,
        }
    }

    /// The file an id names, from the compile's own map.
    fn from_map(map: &'a rumoca_core::SourceMap, id: rumoca_core::SourceId) -> Option<Self> {
        let (name, text) = map.get_source(id)?;
        Some(Self { id, name, text })
    }
}

/// Turn a Rumoca `Span` into a source location Claude can quote back at Doug.
///
/// A span is byte offsets into one file. On its own that is useless in an answer —
/// "unknown `gnd.p.i`" tells Doug nothing about *his* model. With the file, the line
/// number and the line's text it becomes "line 5 of your model, the `gnd`
/// declaration", which is the difference between explaining the compiler and
/// diagnosing the model (ideas #45).
///
/// **`None` for a span belonging to another file**, which is the property this
/// function's callers were documented as having and did not have until 2026-11-04.
/// Byte offsets alone cannot say which file they are offsets *into*, so the only way
/// to refuse is to be told, and `SpanFile` is that telling.
///
/// Byte offsets are converted by counting newlines rather than by character
/// arithmetic, and excerpts use `from_utf8_lossy`, so a source containing
/// non-ASCII cannot panic here. An em-dash in a description string caused exactly
/// that class of crash in the lexer on 2026-07-27.
fn span_to_location(file: SpanFile<'_>, span: &rumoca_core::Span) -> Option<serde_json::Value> {
    if span.source != file.id {
        return None;
    }
    let bytes = file.text.as_bytes();
    // `BytePos` is a newtype over `usize`; unwrap once here so the arithmetic
    // below reads as ordinary slicing.
    let (start, end) = (span.start.0, span.end.0);
    if start > end || end > bytes.len() {
        // The span names this file but does not fit inside it, so the two
        // disagree. That is a Rumoca-side inconsistency rather than a foreign
        // span — the id was already checked — and inventing a line number for
        // it would be worse than silence.
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
        // **First, because it qualifies everything after it.** A line number with
        // no file is the unstated assumption that produced the wrong answers.
        // Normalized, because the raw name is this machine's absolute path — see
        // `portable_source_name`.
        "file": portable_source_name(file.name),
        "line": line,
        "column": column,
        "excerpt": String::from_utf8_lossy(&bytes[start..end]),
        "line_text": String::from_utf8_lossy(&bytes[line_start..line_end]).trim_end(),
        "byte_start": start,
        "byte_end": end,
    }))
}

/// Resolve a span **in the file its own `SourceId` names**, through Rumoca's map.
///
/// # The defect this replaces, which was live and confident
///
/// [`span_to_location`] takes one source text and slices it. Its own doc comment
/// claimed the safe property — *"a label whose span points into a library file
/// resolves to `null` … `span_to_location` refuses rather than inventing a
/// line"* — and that claim was **false**: the only guard is `end >
/// bytes.len()`, so a span into an MSL file whose byte range happens to fit
/// inside a short specimen resolves happily against the wrong file.
///
/// It was not hypothetical. Measured 2026-11-04, `Drivetrain`'s four unmatched
/// unknowns were published as:
///
/// ```text
///   emf.w         line 8   "// joints, closed-loop linkages — belong to the …"
///   shaft.a       line 10  "// --- Electrical: a DC source drives the motor …"
///   load.a        line 6   "// rotational flanges (tau), and translational …"
///   wall.flange.f line 2   "// purpose: Ideal gears → high-index, structurally …"
/// ```
///
/// Every one is a **comment in `Drivetrain.mo`**, and every one is wrong: the
/// truth is `SI.AngularVelocity w`, `SI.AngularAcceleration a`,
/// `SI.Acceleration a` and `SI.Force f`, in four different MSL connector and
/// component files. `CapacitorLoop` named `connect(src.n, gnd.p);` for a span
/// that is `SI.Current i` in `Modelica.Electrical.Analog.Interfaces.Pin`. The
/// answer was not merely vague — it pointed Doug at his own comments and told
/// him a declaration was there.
///
/// # Why a map lookup is the fix rather than a guard
///
/// Refusing a foreign span would make the old claim true, and would have been
/// honest; but it would also blank the locations for exactly the models that
/// need them, since most of this corpus declares its unknowns inside the MSL.
/// `Dae::source_map` is already public and `SourceMap::get_source` returns
/// `(name, text)` for any id the compile registered, so the *right* file is one
/// call away and the excerpt comes out of it. Nothing is guessed and no new
/// Rumoca API was needed.
///
/// `file` is therefore part of the published location: a line number means
/// nothing until the reader knows which file it counts lines in, and that was
/// the unstated assumption the old form smuggled in.
fn span_location(
    source_map: &rumoca_core::SourceMap,
    span: &rumoca_core::Span,
) -> Option<serde_json::Value> {
    span_to_location(SpanFile::from_map(source_map, span.source)?, span)
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
///
/// **Resolved through Rumoca's source map since 2026-11-04**, because an unmatched
/// unknown is usually declared in the *library*, not the specimen — `emf.w` is a
/// field of an MSL component — and slicing the specimen for its span produced a
/// confident wrong line for every unknown of every MSL-backed model in the corpus.
/// [`span_location`] records the measurement.
fn unmatched_unknown_locations(
    source_map: &rumoca_core::SourceMap,
    names: &[String],
    // **Every unmatched unknown now HAS a span.** 0.10.0 reports
    // `Vec<Span>` where 0.9.20 reported `Vec<Option<Span>>`, because the error
    // is built from the incidence, which carries a span per unknown. The
    // positional `get(i)` stays — a shorter span list than name list would
    // still leave later unknowns unlocated rather than mispaired — but the
    // inner `Option` is gone, so a `None` location now means only "the compile
    // registered no file under this span's id", never "no span was recorded".
    spans: &[rumoca_core::Span],
) -> serde_json::Value {
    serde_json::Value::Array(
        names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let loc = spans.get(i).and_then(|s| span_location(source_map, s));
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
    pub matching: Vec<rumoca_phase_structural::MatchingRecord>,
    /// Tarjan steps, **one segment per traversal**, because node indices mean
    /// nothing across traversals. `build_blt_blocks` may condense compact
    /// equation families out of the node set and run over a renumbered graph,
    /// so each segment's opening `Start` carries the `equation_of_node` map
    /// that makes its indices readable — see `tarjan_runs()`.
    pub tarjan: Vec<Vec<rumoca_phase_structural::TarjanRecord>>,
    /// Tearing decisions, **one segment per coupled block**, in BLT order - a
    /// flat list would splice several loops into one replay.
    ///
    /// Each segment opens with the `BlockStart` declaring its ordinal, which
    /// is what lets a consumer match it against the report's coupled blocks
    /// rather than counting. Inside, a block holds up to two passes under
    /// different tear priorities, closed by the `PlanSelected` naming the one
    /// adopted.
    pub tearing: Vec<Vec<rumoca_phase_structural::TearingRecord>>,
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
///
/// **No `source` parameter since 2026-11-04.** The unmatched-unknown locations it
/// publishes are resolved through `cr.dae.source_map()` now, because most of them are
/// declared in the MSL rather than the specimen; see [`span_location`].
fn structural_stage(result: Option<&PhaseResult>) -> (Stage, StructuralFrames) {
    let empty = || StructuralFrames {
        matching: Vec::new(),
        tarjan: Vec::new(),
        tearing: Vec::new(),
    };
    if let Some(stage) = not_reached_stage(result) {
        return (stage, empty());
    }
    let cr = unwrap_success(result);

    // **The animation's steps come from this run, and from only this run.**
    // Until 2026-08-04 the matching animation re-ran `maximum_matching` when
    // its tab was opened — deterministic, so it agreed, but it agreed by luck
    // of the algorithm and described an execution that produced nothing.
    //
    // `inspect_structural_report` is what makes that structurally impossible
    // rather than merely true today: it returns the report and the steps from
    // one call, over one observer threaded through the real searches. There is
    // no second implementation that could drift, and nothing here chooses a
    // "traced" code path.
    //
    // Observed around the *singular* path too, deliberately: a model that
    // fails to match is the one whose search is most worth watching, and the
    // matching that proved it singular is already recorded by the time the
    // error comes back.
    let (report, trace) = cr
        .dae
        .inspect(rumoca_phase_structural::inspect_structural_report);
    let frames = StructuralFrames {
        matching: trace.matching().cloned().collect(),
        tarjan: trace
            .tarjan_runs()
            .into_iter()
            .map(|run| run.into_iter().cloned().collect())
            .collect(),
        // **`tearing_blocks`, not `tearing_passes`.** Every consumer addresses
        // a *block* — `TearingAnimation::from_captured` matches each segment's
        // opening `BlockStart` ordinal against the report's coupled blocks, and
        // the view draws block-local indices against that block's rows.
        //
        // Splitting by pass produced two segments for one coupled block: the
        // `BlockStart` alone, then the pass. `from_captured` then refused, and
        // `a_real_algebraic_loop_is_traced_end_to_end` caught it — a producer
        // and a consumer disagreeing about the unit, which is the exact
        // mismatch `BlockStart` was added to make impossible.
        tearing: trace
            .tearing_blocks()
            .into_iter()
            .map(|block| block.into_iter().cloned().collect())
            .collect(),
    };
    let stage = cr.dae.inspect(|view| {
        // Built once and shared by both arms. `build_incidence` can itself
        // fail; when it does the incidence and matching keys are *absent*
        // rather than empty, because an empty matrix is a claim that the
        // system has no entries, which is a different statement from "the
        // matrix could not be built".
        let incidence = rumoca_phase_structural::build_incidence(view).ok();
        let mut json = match &report {
            Ok(rep) => structural_to_json(rep),
            Err(_) => serde_json::json!({}),
        };
        let obj = json
            .as_object_mut()
            .expect("both arms above construct a JSON object");
        if let Some(inc) = incidence.as_ref() {
            obj.insert("incidence".to_owned(), incidence_to_json(inc, Some(view)));
        }
        match &report {
            Ok(_) => Stage::ok(json),
            Err(e) => {
                // **The partial matching is read from the trace, not
                // re-derived.** The old code re-ran `maximum_matching` here
                // purely to have something to draw, which described an
                // execution that produced nothing — and `maximum_matching` is
                // not public on 0.10.0 anyway. The last recorded step carries
                // the matching that actually failed.
                if let Some(inc) = incidence.as_ref()
                    && let Some(last) = trace.matching().last()
                {
                    obj.insert(
                        "matching".to_owned(),
                        partial_matching_to_json(inc, &last.match_eq, view),
                    );
                }
                obj.insert(
                    "error".to_owned(),
                    structural_error_to_json(e, cr.dae.source_map()),
                );
                let note = match e {
                    rumoca_phase_structural::StructuralError::Singular { .. } => {
                        "singular".to_owned()
                    }
                    _ => format!("{e}"),
                };
                Stage::recovered(json, note)
            }
        }
    });
    (stage, frames)
}

/// **The one reduction this compile performed**, with the record of how it got there.
///
/// Index reduction and Initialization are the same act of preparation seen from two
/// ends: [`rumoca_phase_structural::inspect_prepare_for_solve`] reduces the system
/// *and* returns the initial-value pins of what it admitted. Calling it twice would
/// be two reductions of one compile, and nothing would make them agree — so it is
/// called once and both stages read this.
///
/// That sharing was predicted rather than discovered: `initialization_stage` carried
/// the note *"shares its preparation with nothing yet … at that point one call should
/// serve both"* from the day it landed, 2026-11-03.
struct ObservedReduction<'a> {
    /// What preparation admitted, or why it did not.
    prepared:
        Result<rumoca_phase_structural::PreparedDae<'a>, rumoca_phase_structural::StructuralError>,
    /// Every round, candidate, attempt and refusal the run observed, in order.
    report: rumoca_phase_structural::ReductionReport,
}

/// Index reduction, from the report of the run the compiler actually performed.
///
/// **Re-modelled 2026-11-04**, the last of the five the v0.10.0 rebase recruited.
/// [`index_reduction_from_observation`] carries the model; this function is the
/// stage wrapper and the outcome judgement.
///
/// # What it replaced, kept because the shape is the lesson
///
/// 0.9.20 cloned the DAE, re-ran a **ten-step dummy-derivative funnel** over the
/// copy (`index_reduce_for_structural_analysis`), then ran `build_structural_report`
/// on the result so the tab could show the reduced system beside the raw one.
///
/// Two independent things make that unportable, and either alone is fatal: **`Dae`
/// is immutable and no longer `Clone`**, so there is nothing to clone and nothing to
/// mutate; and **the funnel is not the algorithm any more** — `dae_prepare` does not
/// exist, and reduction is two fixed-point lanes driven by a candidate / attempt /
/// selection protocol.
///
/// **It was also a re-run presented as the compile**, which this repository treats as
/// a fiction whatever the word on the pane says. On 0.9.20 that may have been the
/// only way to see reduction at all; 0.10.0 removes the excuse, because
/// `inspect_prepare_for_solve` observes the reduction the compiler performed and
/// returns an owned `ReductionReport` of every round, candidate, attempt and
/// rejection. So this is not a translation — **it stops HRW re-deriving this phase.**
///
/// # What does not come back, and why that is stated rather than stubbed
///
/// The **frame-consuming sub-views are unavailable on this tab** — matching, Tarjan
/// and tearing animations. The reduced system's *report* is observable; its *search*
/// is not, because `inspect_prepare_for_solve` records the reduction's rounds and not
/// the matching passes inside each rebuild.
///
/// That is a deliberate refusal rather than an omission, because the existing guard
/// would not have caught the mistake: `TearingAnimation::from_captured` compares
/// `blocks.len()` against `segments.len()` and ordinals that run `0..n` in both
/// systems, so publishing a structural-report-shaped payload here would let the
/// **raw** system's recorded frames pair silently with **reduced** block names. A
/// plausible animation of a search that never happened is worse than no animation.
fn index_reduction_stage(
    result: Option<&PhaseResult>,
    observed: Option<&ObservedReduction<'_>>,
    // The Structural stage's published report, which is this stage's `before`.
    structural: Option<&serde_json::Value>,
) -> Stage {
    // Still answers the earlier-failure question, so a model that stopped at
    // parse reports *that* here rather than this message. The distinction
    // matters: one says the compiler never got here, the other says it did and
    // HRW cannot show the result.
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    let Some(observed) = observed else {
        // Reached only if the caller sequenced this stage before the shared
        // preparation ran. `the_reduction_stage_reads_the_compiles_own_run`
        // fails rather than letting that read as a property of the model.
        return Stage::info(
            "no reduction was observed for this compile, which is an HRW sequencing \
             fault rather than a fact about the model"
                .to_owned(),
        );
    };
    let json = index_reduction_from_observation(observed, cr, structural, cr.dae.source_map());
    match &observed.prepared {
        Ok(_) => {
            let mut stage = Stage::ok(json);
            stage.note = Some(reduction_success_note(&stage.value));
            stage
        }
        // **`recovered`, not `err`** — the same judgement `structural_stage` makes
        // for a singular system. Rumoca flags the failure and the compile continues,
        // so a stage claiming the pipeline stopped here would be the invented
        // control-flow claim C20 removed from four sites. `Outcome::Failed` is a
        // statement about control flow and at most one stage per compile may make it.
        Err(error) => Stage::recovered(json, reduction_failure_note(error)),
    }
}

/// The stage-tab note for a reduction that admitted the system.
///
/// **Read back out of the payload rather than computed beside it**, so the label and
/// the pane cannot disagree about what happened — the defect shape
/// `the_events_summary_uses_rumocas_own_names` and the `zero_crossing_conditions`
/// episode both come from two expressions of one fact.
///
/// `index-1` appears for `borrowed` because that is precisely what it means: the raw
/// system sorted and normalization left it alone. `sorted` with zero rounds does
/// **not** get that wording, because something was rebuilt.
fn reduction_success_note(value: &Option<serde_json::Value>) -> String {
    let Some(v) = value else {
        return "reduction observed".to_owned();
    };
    let kind = v["outcome"]["kind"].as_str().unwrap_or("unreported");
    let rounds = v["rounds"].as_u64().unwrap_or(0);
    let (before, after) = (
        v["before"]["n_states"].as_u64(),
        v["after"]["n_states"].as_u64(),
    );
    match (kind, rounds) {
        ("borrowed", _) => "already index-1: no reduction needed".to_owned(),
        ("sorted", 0) => "normalized without a reduction round (derivative aliases)".to_owned(),
        ("sorted", n) => match (before, after) {
            (Some(b), Some(a)) if b != a => {
                format!("sorted after {n} round(s): {b} state(s) \u{2192} {a}")
            }
            // **Not "0 states demoted".** `CapacitorLoop` sorts with its counts
            // unchanged, so the honest line names the manifold constraint instead of
            // reporting a difference of zero as though nothing had happened.
            _ => {
                let manifold = v["after"]["n_manifold_constraints"].as_u64().unwrap_or(0);
                format!(
                    "sorted after {n} round(s): state count unchanged, \
                     {manifold} manifold constraint(s)"
                )
            }
        },
        (other, _) => format!("reduction reported {other}"),
    }
}

/// The short note for a reduction that did not admit the system.
///
/// Separate from the payload so the tab label and the pane cannot disagree about
/// what happened.
fn reduction_failure_note(error: &rumoca_phase_structural::StructuralError) -> String {
    match error {
        rumoca_phase_structural::StructuralError::Singular { .. } => {
            "still singular after reduction".to_owned()
        }
        other => format!("{other}"),
    }
}

/// Reconcile the Index Reduction stage with what solve lowering actually produced.
///
/// # The problem: one phase's refusal is not the compiler's verdict
///
/// `inspect_prepare_for_solve` declines the canonical index-3 pendulum — its Holonomic
/// lane finds the rod constraint, attempts it, and reports `would_create_vacuous_residual`,
/// because differentiating `x² + y² = L²` and substituting it back leaves `0 = 0`. The
/// compiler then lowers and simulates the model anyway, on **2** coordinates, because
/// v0.10.4 added `rumoca-phase-solve`'s state selection, which picks independent
/// coordinates on the constraint manifold.
///
/// So [`index_reduction_stage`] cannot settle this stage's outcome from `observed.prepared`
/// alone: `Err` there means *this entry point declined*, not *the index stayed high*. Left
/// alone it published [`Outcome::Flagged`] with the note *"still singular after reduction"*,
/// and `stage_tabs::tab_label` painted the tab in `error_fg_color` — about a model whose rod
/// constraint holds to 4.6e-12 over two seconds.
///
/// # Why this is a second function and not a line in the banner
///
/// `report_sub_view::reduction_banner` was fixed on 2026-11-04 and **this was not**, so for a
/// day the pane read *Reduced by state selection* underneath a red tab. Doug reported the tab;
/// nothing else could have, because no test compares a label's colour against the pane beneath
/// it. Both now derive from the one `state_scalar_count` in the Solve Lowering payload, which
/// is the only way two expressions of one fact stay agreed.
///
/// # Why a post-pass rather than a branch inside the stage
///
/// Index reduction is built **before** solve lowering, and deliberately: the preparation both
/// reduction stages share runs outside either `run_stage!` bracket so neither is charged the
/// other's time. Reordering to let the stage read the lowered layout would trade a false
/// outcome for a false timing claim. So the judgement waits until both stages exist.
///
/// # The note keeps the word `singular`, and that is load-bearing
///
/// `App::note_says_singular` decides sub-view availability by searching the note for
/// `"singular"`, and this stage's *payload* is still the refusal report — Summary is where its
/// rounds and residue live, so it must stay offered.
/// `the_reconciled_note_still_offers_the_summary_view` pins that coupling rather than leaving
/// it to be rediscovered.
fn reconcile_reduction_with_lowering(index_reduction: &mut Stage, solve_lowering: &Stage) {
    // Only a reduction that reported `singular` is in question. Read from the payload
    // rather than from the note, because the note is prose and the payload is Rumoca's
    // own word — the same reason `reduction_success_note` reads back out of the value.
    let reported_singular = index_reduction
        .value
        .as_ref()
        .and_then(|v| v["outcome"]["kind"].as_str())
        == Some("singular");
    if !reported_singular {
        return;
    }
    // **The count the real lowering produced.** `rumoca_phase_solve::integrated_state_names`
    // answers the same question by running state selection a second time, which would be a
    // re-execution presented as the compile — the fiction this repository has removed twice.
    let Some(states) = solve_lowering
        .value
        .as_ref()
        .and_then(|v| v["solve_layout"]["state_scalar_count"].as_u64())
    else {
        // Solve lowering published no layout, so nothing carried the model and the
        // reduction's own refusal is the whole story. Measured across the corpus:
        // `IncompatibleConnect` and `TwiceDefined` land here and must stay flagged.
        return;
    };
    index_reduction.outcome = Outcome::Ok;
    index_reduction.note = Some(format!(
        "this reducer's search stayed singular; solve lowering selected {states} \
         independent coordinate(s) on the constraint manifold and lowered the model"
    ));
}

/// **Index reduction as Rumoca performed it**, from the report of the one run.
///
/// This is the symbol `docs/tech-debt.md`'s `unbuilt:` tag recruited, so
/// `doc_citations::claims_of_absence_are_still_true` fails now that it exists and
/// forces that row to be retired.
///
/// # The model, which is not the funnel under new names
///
/// 0.9.20 published a **ten-step funnel**: a fixed pipeline of named steps, each with
/// a before/after state count, which HRW re-ran over a cloned DAE. 0.10.0 reduction is
/// a **search**, and the published shape is its log:
///
/// - **Two lanes, `Direct` and `Holonomic`**, alternating as a fixed point. A round
///   states the lane, the round number, the current `residue`, and what is still
///   unmatched on both sides.
/// - **Candidates are discovered per proof boundary** — `DirectAdmissible`,
///   `DirectConditional`, `Holonomic` — and `discovered` is how many that boundary
///   admitted.
/// - **An attempt rebuilds and re-sorts the whole system**, so its `outcome` is a
///   measurement and not a prediction: `Sorted`, `Reduced`, `Held`, `Raised`, or one
///   of four refusals.
/// - **`Held` is progress.** Measured on `Drivetrain`, round 3 selects a candidate
///   whose residue does not move, and round 4 then succeeds — because the system
///   changed even though the number did not. A funnel step cannot express that, and
///   a reader shown only the numbers would call round 3 wasted.
///
/// `residue` is **both deficits summed**: `(n_equations - n_matched) + (n_unknowns -
/// n_matched)`, so a square system's residue is twice its rank deficiency. Published
/// as Rumoca's own number, with that definition stated in the payload rather than
/// halved into something friendlier.
///
/// # The ordinals are arena-local, and that is why no name is attached to them
///
/// `state_ordinal`, `rhs_ordinal`, `variable_ordinal` and the holonomic ordinals
/// index the **rebuilt system of that round**, not the raw DAE. Resolving them
/// against `cr.dae` would produce a confident wrong name from round 2 onward, which
/// is the defect class `span_location` was fixed for on the same day this was
/// written. **The span is the stable identity**, so each identity publishes
/// `declared_at` and leaves the ordinal an unnamed number — and the payload says so
/// in `ordinals_note`, so a reader cannot take the silence for an oversight.
///
/// # Before and after are observations, not re-runs
///
/// **`before` is the Structural stage's own published report**, taken as it stands
/// and given the three counts. Not a second analysis of the raw system: that is
/// exactly the duplication the shared `inspect_prepare_for_solve` call removed one
/// screen above, and a `build_incidence` here would be a second copy of a matrix
/// that already has a publisher. It carries the partial matching and the singular
/// `error` with it, which is what lets the before/after split view show a rank
/// deficiency beside a full rank.
///
/// `after` comes from
/// [`rumoca_phase_structural::PreparedDae::structural_report`], which reports the
/// matching and BLT **issued for the reduced DAE that Solve consumes** — so the two
/// cannot drift. Its `None` arm returns a 0/0 report, which is indistinguishable
/// from a real analysis of an empty system, so this branches on
/// `system.structural` itself and omits the key instead.
///
/// # Counts alone do not show reduction, measured
///
/// `CapacitorLoop` is admitted through the holonomic lane with **1 state, 13
/// algebraics and 14 equations before and after** — identical — plus one manifold
/// constraint. So a pane reporting only the counts would say nothing happened, and
/// `n_manifold_constraints` plus the search log are what make the change visible.
/// `Drivetrain` is the opposite case: 9 states to 5. Both are the same phase.
fn index_reduction_from_observation(
    observed: &ObservedReduction<'_>,
    cr: &CompilationResult,
    // The Structural stage's published value, reused rather than recomputed.
    structural: Option<&serde_json::Value>,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    let records = &observed.report.records;
    let rounds = records
        .iter()
        .filter(|r| matches!(r, rumoca_phase_structural::ReductionRecord::Round { .. }))
        .count();
    let search: Vec<serde_json::Value> = records
        .iter()
        .map(|record| reduction_record_to_json(record, source_map))
        .collect();

    let counts_before = crate::dae_counts::DaeCounts::of(&cr.dae);
    // The Structural stage's report, with the counts added. Cloned rather than
    // rebuilt: one origin for the raw system's incidence and matching.
    let mut before = structural.cloned().unwrap_or_else(|| serde_json::json!({}));
    if let Some(obj) = before.as_object_mut() {
        obj.insert(
            "n_states".to_owned(),
            serde_json::json!(counts_before.states),
        );
        obj.insert(
            "n_algebraics".to_owned(),
            serde_json::json!(counts_before.algebraics),
        );
        // **Scalar rows, the incidence's `n_eq`** — not `continuous_equations`, which
        // counts residual entries and misses structured families: `ArrayFamily` read
        // 0 here beside an 8-row incidence until 2026-11-07, overwriting the
        // Structural report's own correct 8. See `DaeCounts::continuous_scalar_rows`.
        obj.insert(
            "n_equations".to_owned(),
            serde_json::json!(counts_before.continuous_scalar_rows),
        );
    }

    let after = match &observed.prepared {
        Ok(prepared) => {
            let reduced = prepared.as_dae();
            let counts = crate::dae_counts::DaeCounts::of(reduced);
            // **The same shape as `before`**, which is what lets one reader —
            // `incidence_view::IncidenceMatrix::from_report` — parse either side.
            // Nesting the report under an `after.structural` key instead would put
            // `matching` one level below where `before` keeps it, and the split
            // view's two halves would need two parsers.
            //
            // Absent rather than empty when there is no analysis:
            // `PreparedDae::structural_report`'s own `None` arm returns a 0/0 report,
            // which reads as a real analysis of an empty system.
            let mut obj = prepared.inspect(|system| {
                if system.structural.is_some() {
                    structural_to_json(&prepared.structural_report())
                } else {
                    serde_json::json!({})
                }
            });
            if let Some(map) = obj.as_object_mut() {
                map.insert("n_states".to_owned(), serde_json::json!(counts.states));
                map.insert(
                    "n_algebraics".to_owned(),
                    serde_json::json!(counts.algebraics),
                );
                // Scalar rows, as `before` — see the note there.
                map.insert(
                    "n_equations".to_owned(),
                    serde_json::json!(counts.continuous_scalar_rows),
                );
            }
            prepared.inspect(|system| {
                if let Ok(inc) = rumoca_phase_structural::build_incidence(system.view) {
                    obj["incidence"] = incidence_to_json(&inc, Some(system.view));
                }
                // **The number that makes `CapacitorLoop` legible.** Its counts do
                // not move; this is what reduction added.
                obj["n_manifold_constraints"] = serde_json::json!(system.manifold.len());
            });
            obj
        }
        // **No `after` at all**, rather than the raw system relabelled. A failed
        // reduction produced no admitted system, and showing the input twice would
        // read as "reduction changed nothing".
        Err(_) => serde_json::Value::Null,
    };

    serde_json::json!({
        "rounds": rounds,
        "n_records": records.len(),
        "outcome": reduction_outcome_summary(observed, rounds, source_map),
        "lanes": reduction_lanes_used(records),
        "residue_definition":
            "(n_equations - n_matched) + (n_unknowns - n_matched): both deficits \
             summed, so a square system's residue is twice its rank deficiency",
        "ordinals_note":
            "state_ordinal, rhs_ordinal, variable_ordinal and the holonomic ordinals \
             index the rebuilt system of their own round, not the raw DAE. They are \
             deliberately unnamed; resolve a candidate through declared_at, which is \
             a span and stable across rebuilds",
        "before": before,
        "after": after,
        "search": search,
        "stalled": observed.report.stalled.as_ref().map(|snapshot| {
            serde_json::json!({
                "error": format!("{}", snapshot.error()),
                "observed_records": snapshot.observed_records(),
                "n_manifold_constraints": snapshot.manifold_expression_ordinals().count(),
            })
        }),
    })
}

/// The one word for how reduction ended, taken from the `Stopped` record.
///
/// Read from the record rather than from `prepared.is_ok()`, because the two answer
/// different questions. Both `Borrowed` and `Sorted` are successes, and they mean
/// different things.
///
/// # `Sorted` is overloaded, and the round count is what disambiguates it
///
/// **`Sorted` with zero rounds is not "reduced until it sorted".** No round ran.
/// `borrowed_with_observer` is reached when the *raw* system's structural analysis
/// already succeeded; it then applies `derivative_aliases::normalize` and reports
/// `Borrowed` if the system came back unchanged and **`Sorted` if normalization
/// rebuilt it**. The same `Sorted` is also emitted from inside the reduction loop
/// when a round sorts the system, so the word alone cannot tell a reader which
/// happened — hence `rounds` decides the sentence this publishes.
///
/// That makes `Sorted` with zero rounds the observable signature of derivative-alias
/// normalization, which is otherwise a `pub(super)` pass. `StabilizedPendulum` is the
/// measured case: 0 rounds, and its system goes from 1 algebraic and 5 equations to
/// **3 algebraics and 7 equations** with its 4 states unchanged. The extension is
/// visible in `before`/`after` without any instrumentation; what is not visible is
/// *which* coordinates received auxiliary aliases.
fn reduction_outcome_summary(
    observed: &ObservedReduction<'_>,
    rounds: usize,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    use rumoca_phase_structural::{ReductionRecord, ReductionStop};
    let stop = observed.report.records.iter().rev().find_map(|record| {
        if let ReductionRecord::Stopped { outcome } = record {
            Some(outcome)
        } else {
            None
        }
    });
    match stop {
        Some(ReductionStop::Borrowed) => serde_json::json!({
            "kind": "borrowed",
            "admitted": true,
            "rounds": rounds,
            "meaning": "the raw system sorted and normalization left it unchanged: \
                        already index-1, nothing rebuilt",
        }),
        Some(ReductionStop::Sorted) if rounds == 0 => serde_json::json!({
            "kind": "sorted",
            "admitted": true,
            "rounds": 0,
            // Not "reduced until it sorted": no round ran. See this function's doc.
            "meaning": "the raw system sorted, and derivative-alias normalization \
                        rebuilt it: no reduction round ran, so the before/after \
                        difference is the normalization's",
        }),
        Some(ReductionStop::Sorted) => serde_json::json!({
            "kind": "sorted",
            "admitted": true,
            "rounds": rounds,
            "meaning": "reduction ran until the system sorted",
        }),
        Some(ReductionStop::Singular { residue }) => serde_json::json!({
            "kind": "singular",
            "admitted": false,
            "residue": residue,
            "meaning": "no candidate in either lane could reduce the residue further",
        }),
        Some(ReductionStop::Failure { error }) => serde_json::json!({
            "kind": "failure",
            "admitted": false,
            "error": format!("{error}"),
        }),
        Some(ReductionStop::DiscardsInitial { variable, span }) => serde_json::json!({
            "kind": "discards_initial",
            "admitted": false,
            "variable": variable,
            "declared_at": span_location(source_map, span),
        }),
        // **Absent, not guessed.** Every terminating path emits `Stopped`; if one
        // does not, the honest report is that nothing said how this ended.
        None => serde_json::json!({
            "kind": "unreported",
            "admitted": observed.prepared.is_ok(),
            "meaning": "the run emitted no Stopped record, so how it ended is unknown",
        }),
    }
}

/// Which lanes the run actually entered, in first-seen order.
fn reduction_lanes_used(records: &[rumoca_phase_structural::ReductionRecord]) -> Vec<&'static str> {
    use rumoca_phase_structural::ReductionRecord;
    let mut lanes: Vec<&'static str> = Vec::new();
    for record in records {
        let lane = match record {
            ReductionRecord::Round { lane, .. }
            | ReductionRecord::Candidates { lane, .. }
            | ReductionRecord::Attempt { lane, .. }
            | ReductionRecord::Selected { lane, .. }
            | ReductionRecord::RetriedPristine { lane } => reduction_lane_name(*lane),
            // **Neither carries a lane, for different reasons.** `Stopped` ends the
            // search rather than belonging to a lane, and an alias class belongs to a
            // *quotient scope* — `Source` or `FormalDerivatives` — which is a different
            // axis from the Direct/Holonomic lanes this function reports.
            ReductionRecord::Stopped { .. } | ReductionRecord::AliasClassUnchanged { .. } => {
                continue;
            }
        };
        if !lanes.contains(&lane) {
            lanes.push(lane);
        }
    }
    lanes
}

/// Rumoca's own lane name, lowercased for JSON.
fn reduction_lane_name(lane: rumoca_phase_structural::ReductionLane) -> &'static str {
    match lane {
        rumoca_phase_structural::ReductionLane::Direct => "direct",
        rumoca_phase_structural::ReductionLane::Holonomic => "holonomic",
    }
}

/// Rumoca's own quotient-scope name, lowercased for JSON.
///
/// Which application of the alias quotient a class was considered under:
/// `Source` is every eligible edge before state selection, `FormalDerivatives`
/// only edges with a formal-derivative endpoint, after that construction.
fn quotient_scope_name(scope: &rumoca_phase_structural::AliasQuotientScope) -> &'static str {
    match scope {
        rumoca_phase_structural::AliasQuotientScope::Source => "source",
        rumoca_phase_structural::AliasQuotientScope::FormalDerivatives => "formal_derivatives",
    }
}

/// Rumoca's own refusal name, lowercased for JSON.
///
/// Why one alias class was left as it was. These are **refusals, not failures**:
/// a class the quotient declines to collapse is a correct outcome, and the whole
/// reduction may still succeed.
fn alias_refusal_name(reason: &rumoca_phase_structural::AliasRefusal) -> &'static str {
    match reason {
        rumoca_phase_structural::AliasRefusal::Cycle => "cycle",
        rumoca_phase_structural::AliasRefusal::SeveralAnchors => "several_anchors",
        rumoca_phase_structural::AliasRefusal::AnchorIsNotState => "anchor_is_not_state",
    }
}

/// One record of the search, as the pane and the stepper read it.
///
/// `kind` names the record variant so a reader never has to infer which it is from
/// which keys are present — the failure mode of a flattened union.
pub(crate) fn reduction_record_to_json(
    record: &rumoca_phase_structural::ReductionRecord,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    use rumoca_phase_structural::ReductionRecord;
    match record {
        ReductionRecord::Round {
            lane,
            round,
            residue,
            unmatched_equations,
            unmatched_unknowns,
        } => serde_json::json!({
            "kind": "round",
            "lane": reduction_lane_name(*lane),
            "round": round,
            "residue": residue,
            "unmatched_equations": unmatched_equations,
            // **The bare ids beside the labels**, the same way
            // `structural_error_to_json` does it and for the same reason: v0.10.0
            // decorates these strings — `"f_x[90] (SI.Angle phi)"` — and a decorated
            // label is not an identity, so pointing at one resolves to nothing.
            // Rumoca's own string is published unchanged; the id goes next to it.
            //
            // **Derived from the label, never recomputed.** A second derivation could
            // disagree with the string it is supposed to identify, and an id naming a
            // different equation than the label beside it is worse than no id.
            //
            // Found by `tests_equation_identity::every_committed_trace_identifies_every_equation_it_names`
            // walking the published trees — 35 labels across the notebook, on the
            // first gate after this stage started publishing. Nobody checked this
            // site; the data enumerated it.
            "unmatched_equation_ids": unmatched_equations
                .iter()
                .map(|l| crate::equation_id_from_label(l))
                .collect::<Vec<_>>(),
            "unmatched_unknowns": unmatched_unknowns
                .iter()
                .map(|u| serde_json::json!({
                    "name": u.name,
                    // Rumoca's own classification, not HRW's reading of the name.
                    "kind": match u.kind {
                        rumoca_phase_structural::UnmatchedKind::Derivative => "derivative",
                        rumoca_phase_structural::UnmatchedKind::Algebraic => "algebraic",
                        rumoca_phase_structural::UnmatchedKind::Other => "other",
                    },
                }))
                .collect::<Vec<_>>(),
        }),
        ReductionRecord::Candidates {
            lane,
            group,
            discovered,
        } => serde_json::json!({
            "kind": "candidates",
            "lane": reduction_lane_name(*lane),
            "group": match group {
                rumoca_phase_structural::ReductionCandidateGroup::DirectAdmissible =>
                    "direct_admissible",
                rumoca_phase_structural::ReductionCandidateGroup::DirectConditional =>
                    "direct_conditional",
                rumoca_phase_structural::ReductionCandidateGroup::Holonomic => "holonomic",
            },
            "discovered": discovered,
        }),
        ReductionRecord::Attempt {
            lane,
            identity,
            outcome,
        } => serde_json::json!({
            "kind": "attempt",
            "lane": reduction_lane_name(*lane),
            "identity": reduction_identity_to_json(identity, source_map),
            "outcome": reduction_attempt_outcome_to_json(outcome, source_map),
        }),
        ReductionRecord::Selected {
            lane,
            identity,
            residue_before,
            residue_after,
        } => serde_json::json!({
            "kind": "selected",
            "lane": reduction_lane_name(*lane),
            "identity": reduction_identity_to_json(identity, source_map),
            "residue_before": residue_before,
            // `null` when the attempt sorted the system: there is no residue after,
            // because there is no unmatched remainder. Zero would be a measurement.
            "residue_after": residue_after,
        }),
        ReductionRecord::RetriedPristine { lane } => serde_json::json!({
            "kind": "retried_pristine",
            "lane": reduction_lane_name(*lane),
            "meaning": "the lane restarted from the unmodified system",
        }),
        ReductionRecord::Stopped { outcome } => {
            let mut obj = serde_json::json!({ "kind": "stopped" });
            obj["outcome"] = reduction_stop_to_json(outcome, source_map);
            obj
        }
        // **New in 0.10.4, and it is not a lane record.** The alias quotient
        // considered one class and left it as it was. `members` are arena-local
        // variable ordinals, published bare the way every other ordinal here is:
        // they identify a variable only within this compile's arena.
        ReductionRecord::AliasClassUnchanged {
            scope,
            members,
            reason,
        } => serde_json::json!({
            "kind": "alias_class_unchanged",
            "scope": quotient_scope_name(scope),
            "members": members,
            "reason": alias_refusal_name(reason),
            "meaning": "the alias quotient declined to collapse this class",
        }),
    }
}

/// One attempted candidate's identity: its form, its arena-local ordinals, and the
/// span that identifies it stably.
fn reduction_identity_to_json(
    identity: &rumoca_phase_structural::ReductionIdentity,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    use rumoca_phase_structural::ReductionIdentity;
    match identity {
        ReductionIdentity::Direct {
            state_ordinal,
            rhs_ordinal,
            provenance_span,
        } => serde_json::json!({
            "form": "direct",
            "state_ordinal": state_ordinal,
            "rhs_ordinal": rhs_ordinal,
            "declared_at": span_location(source_map, provenance_span),
        }),
        ReductionIdentity::DerivativeDefinition {
            state_ordinal,
            rhs_ordinal,
            provenance_span,
        } => serde_json::json!({
            "form": "derivative_definition",
            "state_ordinal": state_ordinal,
            "rhs_ordinal": rhs_ordinal,
            "declared_at": span_location(source_map, provenance_span),
        }),
        ReductionIdentity::AuxiliaryState {
            state_ordinal,
            variable_ordinal,
            provenance_span,
        } => serde_json::json!({
            "form": "auxiliary_state",
            "state_ordinal": state_ordinal,
            "variable_ordinal": variable_ordinal,
            "declared_at": span_location(source_map, provenance_span),
        }),
        ReductionIdentity::Holonomic {
            owner_ordinal,
            body_ordinal,
            component_scalar,
            residual_ordinal,
            owner_span,
            anchored_state_ordinals,
        } => serde_json::json!({
            "form": "holonomic",
            "owner_ordinal": owner_ordinal,
            "body_ordinal": body_ordinal,
            "component_scalar": component_scalar,
            "residual_ordinal": residual_ordinal,
            "declared_at": span_location(source_map, owner_span),
            "anchored_state_ordinals": anchored_state_ordinals,
        }),
    }
}

/// What rebuilding and re-sorting the system with this candidate measured.
fn reduction_attempt_outcome_to_json(
    outcome: &rumoca_phase_structural::ReductionOutcome,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    use rumoca_phase_structural::ReductionOutcome;
    match outcome {
        ReductionOutcome::Sorted => serde_json::json!({ "kind": "sorted" }),
        ReductionOutcome::Reduced { residue } => serde_json::json!({
            "kind": "reduced",
            "residue": residue,
        }),
        // **Accepted in the Direct lane, and it is progress.** See
        // `index_reduction_from_observation` for the Drivetrain round that proves it.
        ReductionOutcome::Held { residue } => serde_json::json!({
            "kind": "held",
            "residue": residue,
            "meaning": "the residue did not move, but the system changed",
        }),
        ReductionOutcome::Raised { residue } => serde_json::json!({
            "kind": "raised",
            "residue": residue,
        }),
        // **New in 0.10.4, and it is a refusal to even try.** A structural lower
        // bound proved the residue cannot fall below `bound`, so the candidate was
        // never reconstructed. `bound` is at least the current residue, which is why
        // this is published as a bound rather than as a residue: it is a statement
        // about what no reconstruction could achieve, not a measurement of one.
        ReductionOutcome::CannotReduce { bound } => serde_json::json!({
            "kind": "cannot_reduce",
            "bound": bound,
            "meaning": "a structural lower bound proves the residue cannot fall below this",
        }),
        ReductionOutcome::NonSingularFailure { error } => serde_json::json!({
            "kind": "non_singular_failure",
            "error": format!("{error}"),
        }),
        ReductionOutcome::WouldDiscardInitial { variable, span } => serde_json::json!({
            "kind": "would_discard_initial",
            "variable": variable,
            "declared_at": span_location(source_map, span),
        }),
        ReductionOutcome::WouldInvalidateManifold => serde_json::json!({
            "kind": "would_invalidate_manifold",
        }),
        ReductionOutcome::WouldCreateVacuousResidual => serde_json::json!({
            "kind": "would_create_vacuous_residual",
        }),
    }
}

/// How the run ended, as its own record states it.
fn reduction_stop_to_json(
    outcome: &rumoca_phase_structural::ReductionStop,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    use rumoca_phase_structural::ReductionStop;
    match outcome {
        ReductionStop::Borrowed => serde_json::json!({ "kind": "borrowed" }),
        ReductionStop::Sorted => serde_json::json!({ "kind": "sorted" }),
        ReductionStop::Failure { error } => serde_json::json!({
            "kind": "failure",
            "error": format!("{error}"),
        }),
        ReductionStop::Singular { residue } => serde_json::json!({
            "kind": "singular",
            "residue": residue,
        }),
        ReductionStop::DiscardsInitial { variable, span } => serde_json::json!({
            "kind": "discards_initial",
            "variable": variable,
            "declared_at": span_location(source_map, span),
        }),
    }
}

/// Initialization, from Rumoca's own initial-value pins.
///
/// **Re-modelled 2026-11-03.** This heading read *"disabled for the v0.10.0 rebase,
/// awaiting a re-model onto `initial_pins`"* for a day after the re-model landed, which
/// is a false claim in the one place a reader looks first. The account below is history.
///
/// # What this did
///
/// It published `ic_plan_to_json(&build_ic_plan(..))` — an ordered **solve
/// sequence** of `IcBlock`s: direct symbolic solves, scalar Newton steps, torn
/// blocks, and coupled Levenberg-Marquardt blocks — plus a determinacy check
/// comparing explicit initial conditions against the state count.
///
/// # Why it is a re-model
///
/// `rumoca_phase_structural::ic_plan` is gone, with `IcBlock`, `build_ic_plan`
/// and `build_ic_relaxation_hint`. 0.10.0 expresses initialization as
/// `initial_pins::{InitialValuePin, InitialValueRole, PinTerm}` — a different
/// formulation, described in its own docs as *"a runtime that seeds one value
/// per state and then projects the algebraic unknowns"*. That is not the same
/// object under a new name, so the pane cannot be re-keyed onto it.
///
/// The determinacy check needs rebuilding too, and **gains precision**. It
/// counted states whose `fixed` attribute was `Some(true)` over a partition
/// that no longer exists. `VariableView::fixed_uniform()` and `fixed_scalar()`
/// replace it, and they are richer: MLS §4.8 lets `fixed` differ per array
/// element, which a single `Option<bool>` could not express — so the old count
/// had to treat a partly-fixed array as all-or-nothing.
///
/// # This is the stage whose defect motivated the rebase
///
/// `docs/upstream-issues.md` carries the initialization defect where several
/// models take the value that zeroes a state's derivative instead of its
/// `start`, with System Modeler's adjudication. **0.10.0 fixes it** — measured
/// 2026-09-19: `ConnRc` goes from 0 of 16 series moving to 10 of 16, `RcCircuit`
/// from 0 of 24 to 15 of 24, both landing on `C.v(1) = 4.999773` and matching
/// the closed form. So this pane is the one most worth re-modelling: it now has
/// something correct to show.
///
/// `docs/tech-debt.md` carries the `unbuilt:` tag naming
/// `worker::initialization_from_initial_pins`.
///
/// Takes no `source`: each pin's `declared_at` resolves through the compile's own
/// source map, which is the only way to locate a `start` written in a library file.
fn initialization_stage(
    result: Option<&PhaseResult>,
    observed: Option<&ObservedReduction<'_>>,
) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    // **The same preparation Index reduction reports on**, since 2026-11-04. It was
    // its own `prepare_for_solve` call until then, which was a second reduction of
    // one compile — two runs that happened to agree because the function is pure,
    // not because anything held them together.
    let Some(observed) = observed else {
        return Stage::info(
            "no preparation was observed for this compile, which is an HRW sequencing \
             fault rather than a fact about the model"
                .to_owned(),
        );
    };
    match &observed.prepared {
        Ok(prepared) => Stage::ok(
            prepared
                .inspect(|system| initialization_from_initial_pins(system, cr.dae.source_map())),
        ),
        // **Not `Failed`, and that is the one-`Failed`-per-compile rule.**
        // Preparation fails on a system structural analysis already reported as
        // singular, and Structural says so with the evidence. A second stage
        // claiming the pipeline stopped here would be the invented control-flow
        // claim C20 removed from four sites.
        Err(error) => Stage::info(format!(
            "no initial-value pins: structural preparation did not admit this system \
             ({error}). The Structural tab reports why \u{2014} initialization has \
             nothing to plan until a system is admitted."
        )),
    }
}

/// What Rumoca will do at the initialization instant, from its own pins.
///
/// **This is the symbol `docs/tech-debt.md`'s `unbuilt:` tag recruited**, so
/// `claims_of_absence_are_still_true` fails once it exists and forces that row
/// to be retired.
///
/// # The model, which is not the old one under new names
///
/// 0.9.20 published an ordered **solve sequence** of `IcBlock`s. 0.10.0 states
/// initialization as a set of [`InitialValuePin`](rumoca_phase_structural::InitialValuePin)s,
/// each an MLS 3.6 §8.6 initial equation rewritten onto the coordinate it is
/// about, with a role deciding how the runtime enforces it:
///
/// - **`Definition`** — this equation *determines* a state's initial value, and
///   stays an equation in the joint initialization solve.
/// - **`Check`** — it stays a residual, so the initialization instant answers it
///   with numbers rather than this phase guessing. A restated value about an
///   already-seeded state is a check, and so is a pin about an algebraic or
///   output coordinate with no unique state to receive a definition.
///
/// That distinction is the thing worth showing, and the old pane could not: a
/// sequence of solve blocks says *how* the values are computed, while the roles
/// say **which stated values are load-bearing and which are consistency
/// conditions the model must satisfy.** `OverInitRc` is the specimen that makes
/// it visible, and it now fails at simulation with an MLS §8.6 message for
/// exactly this reason.
///
/// # Why `provenance` is `Compiler` even though HRW made the call
///
/// `prepare_for_solve` is Rumoca's function over the compile's own immutable
/// `Dae`, and HRW invents no value — the same relationship
/// [`structural_stage`] has to `inspect_structural_report`, which also publishes
/// with [`Stage::ok`]. `Stage::computed` is for content HRW *synthesises*, which
/// this is not.
///
/// **The second preparation cannot disagree with the compile's**, which is the
/// part the old funnel could not claim. Solve lowering prepares internally and
/// returns nothing, so this is the only available route; but `Dae` is immutable
/// and `prepare_for_solve` takes `&Dae`, so the input is the same object and the
/// function is pure. The old pane cloned a DAE and ran steps in an order HRW
/// maintained by hand.
///
/// **It shares Index reduction's preparation** (since 2026-11-04): one
/// `inspect_prepare_for_solve` per compile serves both stages — see
/// [`initialization_stage`]. (This said "shares its preparation with nothing yet"
/// until the 2026-11-07 sweep.)
fn initialization_from_initial_pins(
    system: rumoca_phase_structural::PreparedSystem<'_, '_>,
    // The compile's own map, not the specimen text: a `start` attribute is often
    // written on an MSL component (`Inertia load(J = 0.05, phi(start = 0))` puts the
    // value in the specimen, but a library default puts it in the library), and
    // slicing the specimen for a library span invents a line — see [`span_location`].
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    let view = system.view;
    let name_of = |index: u32| -> String {
        view.variable_id(index as usize)
            .and_then(|id| view.variable(id))
            .map_or_else(|| format!("<variable {index}>"), |v| v.name().to_string())
    };

    let pins: Vec<serde_json::Value> = system
        .pins
        .iter()
        .map(|pin| {
            // The stated value is a sum of signed terms, so it is rendered as
            // one: `a - b`, not a list a reader has to add up.
            let mut value = String::new();
            for (position, term) in pin.value.iter().enumerate() {
                if position == 0 {
                    if term.negated {
                        value.push('-');
                    }
                } else {
                    value.push_str(if term.negated { " - " } else { " + " });
                }
                match view.expression_id(term.expression as usize) {
                    Some(expression) => {
                        value.push_str(&crate::dae_expr_format::format_expr(view, expression));
                    }
                    None => value.push_str("<unresolved expression>"),
                }
            }
            serde_json::json!({
                "role": match pin.role {
                    rumoca_phase_structural::InitialValueRole::Definition => "definition",
                    rumoca_phase_structural::InitialValueRole::Check => "check",
                },
                "coordinate": name_of(pin.coordinate),
                // Reported for every pin, not only aggregates: a reader
                // comparing two pins on one array needs to see which element
                // each is about, and omitting it on scalars would make the
                // field look optional rather than always meaningful.
                "scalar": pin.scalar,
                "value": value,
                // Where the value was read from, which is not always the
                // coordinate it ends up about: a value stated on one member of
                // a proved equality class is transferred onto the state that
                // class determines.
                "source": name_of(pin.source),
                "source_scalar": pin.source_scalar,
                "declared_at": span_location(source_map, &pin.provenance.span()),
            })
        })
        .collect();

    // **The determinacy summary answers one question: which states' initial
    // values are DETERMINED, and which are only guessed?**
    //
    // That is the reader's question and the old pane's determinacy check was
    // aimed at it, by comparing explicit initial conditions against the state
    // count. Three things make it a different computation on 0.10.0:
    //
    // 1. **A state's value can be determined without the state being `fixed`.**
    //    Measured on a two-line probe: `Real y(start = 2.0, fixed = true)` with
    //    `y = x` produces a `Definition` pin **on `x`**, sourced from `y`. So
    //    counting `fixed` states alone reports zero determined and is wrong —
    //    the first version of this function did exactly that.
    // 2. **`fixed` is per element** (MLS §4.8.6), so this counts scalars rather
    //    than declarations; a single `Option<bool>` could not express a partly
    //    fixed array at all.
    // 3. **A bare `start` is a guess, not an initial condition.** It seeds the
    //    solve and the solve may move it. Naming those states is the useful
    //    half: each one is a state whose reported initial value came out of the
    //    initialization solve rather than out of the model, which is precisely
    //    the blast radius of the initialization defect `upstream-issues.md`
    //    carries.
    let defined: std::collections::BTreeSet<(u32, u32)> = system
        .pins
        .iter()
        .filter(|pin| {
            matches!(
                pin.role,
                rumoca_phase_structural::InitialValueRole::Definition
            )
        })
        .map(|pin| (pin.coordinate, pin.scalar))
        .collect();

    let mut state_scalars = 0usize;
    let mut determined = 0usize;
    let mut partly_fixed: Vec<String> = Vec::new();
    let mut only_guessed: Vec<String> = Vec::new();
    for (id, variable) in view.variables() {
        if variable.role() != rumoca_ir_dae::VariableRole::State {
            continue;
        }
        let count = variable.scalar_count();
        state_scalars += count;
        let mut fixed_here = 0usize;
        for scalar in 0..count {
            let own_fixed = variable.fixed_scalar(scalar) == Some(true);
            if own_fixed {
                fixed_here += 1;
            }
            if own_fixed || defined.contains(&(id.index(), scalar as u32)) {
                determined += 1;
            } else {
                // Named per scalar for an aggregate, so a reader can tell a
                // wholly guessed array from one with a guessed element.
                only_guessed.push(if count > 1 {
                    format!("{}[{}]", variable.name(), scalar + 1)
                } else {
                    variable.name().to_string()
                });
            }
        }
        if fixed_here > 0 && fixed_here < count {
            partly_fixed.push(variable.name().to_string());
        }
    }

    serde_json::json!({
        "determinacy": {
            "state_scalars": state_scalars,
            "determined": determined,
            "only_guessed": only_guessed,
            "definitions": defined.len(),
            "checks": system.pins.len() - defined.len(),
            // Named per element, because this is the case the old single
            // `Option<bool>` could not express at all.
            "partly_fixed_states": partly_fixed,
        },
        "pins": pins,
    })
}

/// Events, from Rumoca's condition algebra.
///
/// **Re-modelled 2026-11-03**; this heading claimed otherwise for a day afterwards. The
/// account below is history.
///
/// # Why this is a re-model and not a port
///
/// HRW read four flat collections — `conditions.equations` (`f_c`),
/// `conditions.relations`, `discrete.real_updates`/`valued_updates`
/// (`f_z`/`f_m`), and `events.synthetic_root_conditions` — counted them into a
/// summary, and rendered each as an equation.
///
/// **On 0.10.0 a condition is not an equation.** It is a
/// [`dae::ConditionOperation`]: `Initial`, `Always`, `Relation(id)`,
/// `Discrete(expr)`, `Clock(id)`, `Not`, `And`, `Or`, `AnyRise` — a boolean
/// algebra over relations and clocks. And a root is no longer a bare expression
/// but `{relation, activation}`. Rendering an algebra term as an equation would
/// be a category error, and HRW's `synthetic_root_conditions` key maps onto
/// nothing here one-to-one.
///
/// # The re-model gains a distinction HRW never showed
///
/// `AnyRise` exists because MLS §8.3.5.1 activates a vector `when` on
/// `edge(b1) or … or edge(bn)`, which is **not** the edge of the disjunction:
/// `when {u, not u}` has an element rising at every switch of `u`, while
/// `u or not u` is a tautology that never rises at all. 0.10.0 keeps those
/// apart in the IR. That is worth a lecture, and it is only available to show
/// because upstream made the semantics explicit.
///
/// # Nothing blocked it but the ruling
///
/// Every arena is publicly readable, with a single-entry accessor *and* an
/// ordered walk: `conditions()`, `relations()`, `roots()`,
/// `structured_roots()`, `time_events()`, `event_actions()`, `clocks()`,
/// `previous_values()`, `terminals()`, each documented with its MLS section. No
/// instrumentation was required — it waited only because a re-model done while
/// nothing compiles gets no reader (`DECISIONS.md`, 2026-11-02). It landed as
/// [`events_from_condition_algebra`]. *(Past tense since the 2026-11-07 sweep.)*
///
/// # One defect this retires on the way out
///
/// The old summary published Rumoca's `synthetic_root_conditions` under the key
/// `zero_crossing_conditions`, which reads as a claim about *all* zero
/// crossings — so `BouncingBall` showed **0**, a bouncing ball apparently
/// detecting no contact. Every value was correct for three weeks; the defect
/// lived entirely in a renamed field. The re-model publishes Rumoca's own names,
/// and `the_events_summary_uses_rumocas_own_names` is the check that will hold
/// it to that.
fn events_stage(result: Option<&PhaseResult>) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    Stage::ok(cr.dae.inspect(events_from_condition_algebra))
}

/// Render one condition as the algebra term it is.
///
/// **Rumoca's own operator names, which is the rule the old pane broke.**
/// `anyRise` is spelled `anyRise` rather than expanded into MLS §8.3.5.1's
/// `edge(b1) or … or edge(bn)`: the expansion is more informative and is *HRW's
/// phrasing*, and the last time this pane paraphrased a name it published
/// `synthetic_root_conditions` as `zero_crossing_conditions` and showed a
/// bouncing ball detecting no contact for three weeks. The explanation belongs
/// in the prose beside the value, never in place of it.
///
/// **Why `anyRise` must not render as `or`.** They are different activations:
/// §8.3.5.1 gives one `Boolean bi` per element and fires on
/// `edge(b1) or … or edge(bn)`, which is not the edge of the disjunction. So
/// `when {u, not u}` has an element rising at every switch of `u`, while
/// `u or not u` is a tautology that never rises at all. Collapsing the two
/// would make those two models look identical in the pane.
///
/// A `Relation` renders as its expression, because that is what a reader is
/// looking for — `h <= 0`, not `r[0]`. The id is published beside it in the
/// `relations` list, so a reader can still tell two identical-looking relations
/// apart.
///
/// `depth` bounds the recursion. The arena is acyclic by construction, so this
/// is not a cycle guard but a refusal to recurse without a stated limit: a
/// deeply nested condition reads as truncated rather than overflowing a stack,
/// which is the same choice `matching.rs` made when it went iterative.
fn render_condition<'dae>(
    view: rumoca_ir_dae::DaeView<'dae>,
    id: rumoca_ir_dae::ConditionId<'dae>,
    depth: u8,
) -> String {
    use rumoca_ir_dae::ConditionOperation as Op;

    if depth == 0 {
        return "<condition nested deeper than this pane renders>".to_owned();
    }
    let Some(condition) = view.condition(id) else {
        return format!("<condition {} not in the arena>", id.index());
    };
    let nested = |inner| render_condition(view, inner, depth - 1);
    match condition.operation() {
        Op::Initial => "initial".to_owned(),
        Op::Always => "always".to_owned(),
        Op::Relation(relation) => view.relation(relation).map_or_else(
            || format!("<relation {}>", relation.index()),
            |r| crate::dae_expr_format::format_expr(view, r.expression()),
        ),
        Op::Discrete(expression) => crate::dae_expr_format::format_expr(view, expression),
        Op::Clock(clock) => format!("clock({})", clock.index()),
        Op::Not(inner) => format!("not ({})", nested(inner)),
        Op::And(left, right) => format!("({} and {})", nested(left), nested(right)),
        Op::Or(left, right) => format!("({} or {})", nested(left), nested(right)),
        Op::AnyRise(left, right) => format!("anyRise({}, {})", nested(left), nested(right)),
    }
}

/// The Events pane, from Rumoca's condition algebra.
///
/// **This is the symbol `docs/tech-debt.md`'s `unbuilt:` tag recruited**, so
/// `claims_of_absence_are_still_true` fails once it exists.
///
/// # What changed, and why nothing could be re-keyed
///
/// 0.9.20 read four flat collections — `conditions.equations`,
/// `conditions.relations`, `discrete.real_updates`/`valued_updates`, and
/// `events.synthetic_root_conditions` — and rendered each as an **equation**.
/// On 0.10.0 a condition is not an equation: it is a
/// [`rumoca_ir_dae::ConditionOperation`], a boolean algebra over relations and clocks, and
/// a root is a relation paired with the condition that activates it. Rendering
/// an algebra term as an equation is a category error.
///
/// # Every count is Rumoca's own arena length, under Rumoca's own name
///
/// The summary keys are the accessor names, and
/// `the_events_summary_uses_rumocas_own_names` holds them there. The defect this
/// retires was not a wrong number — every value was correct for three weeks —
/// it was `synthetic_root_conditions` published as `zero_crossing_conditions`,
/// a name that claims something larger. **A renamed field is a claim**, and the
/// cheapest defence is not to make one.
fn events_from_condition_algebra(view: rumoca_ir_dae::DaeView<'_>) -> serde_json::Value {
    /// How deep a condition term renders before it is truncated.
    const CONDITION_DEPTH: u8 = 24;

    let conditions: Vec<serde_json::Value> = view
        .conditions()
        .map(|(id, _)| {
            serde_json::json!({
                "id": format!("c[{}]", id.index()),
                "term": render_condition(view, id, CONDITION_DEPTH),
            })
        })
        .collect();

    let relations: Vec<serde_json::Value> = view
        .relations()
        .map(|(id, relation)| {
            serde_json::json!({
                "id": format!("r[{}]", id.index()),
                "expression": crate::dae_expr_format::format_expr(view, relation.expression()),
            })
        })
        .collect();

    // A root is the pair, which is the whole point: the relation Rumoca
    // monitors for a sign change, and the condition that has to hold for the
    // crossing to matter.
    let roots: Vec<serde_json::Value> = view
        .roots()
        .map(|(_id, root)| {
            serde_json::json!({
                    "relation": format!("r[{}]", root.relation().index()),
                    "expression": view.relation(root.relation()).map_or_else(
                        || "<unresolved relation>".to_owned(),
                        |r| crate::dae_expr_format::format_expr(view, r.expression()),
                    ),
                "activation": render_condition(view, root.activation(), CONDITION_DEPTH),
            })
        })
        .collect();

    let event_actions: Vec<serde_json::Value> = view
        .event_actions()
        .map(|(_id, action)| {
            use rumoca_ir_dae::EventActionOperation as Action;
            let (kind, detail) = match action.operation() {
                Action::Assert { message, .. } => {
                    ("assert", crate::dae_expr_format::format_expr(view, message))
                }
                Action::Terminate { message } => (
                    "terminate",
                    crate::dae_expr_format::format_expr(view, message),
                ),
                // **New in 0.10.4. A warning-level assertion (MLS 8.3.7), and it
                // never aborts.** `condition` is a third thing beside this action's
                // trigger and guard, and it is the one that decides whether the
                // warning fires: the action reports while the guard is active and
                // `condition` is *false*. Publishing only the message would drop it,
                // and nothing else in this object carries it.
                Action::Warning { message, condition } => (
                    "warning",
                    format!(
                        "{} unless {}",
                        crate::dae_expr_format::format_expr(view, message),
                        crate::dae_expr_format::format_expr(view, condition),
                    ),
                ),
                // **New in 0.10.4**: one MLS 3.7 12.9 `ModelicaInternal_print` call,
                // reported each time the action is active at a settled event.
                Action::Print { message } => {
                    ("print", crate::dae_expr_format::format_expr(view, message))
                }
                // The only kind that moves a trajectory — see
                // `dae_counts::has_discontinuities`, which counts exactly these.
                Action::Reinitialize { state, value } => (
                    "reinitialize",
                    format!(
                        "{} := {}",
                        view.variable(state.into())
                            .map_or_else(|| "<state>".to_owned(), |v| v.name().to_string(),),
                        crate::dae_expr_format::format_expr(view, value),
                    ),
                ),
            };
            serde_json::json!({
                "kind": kind,
                "detail": detail,
                "trigger": render_condition(view, action.trigger(), CONDITION_DEPTH),
                "guard": render_condition(view, action.guard(), CONDITION_DEPTH),
            })
        })
        .collect();

    serde_json::json!({
        // Arena lengths under the accessor names that produce them. A key here
        // that is not a `*_count()` on `DaeView` is a renamed field, which is
        // the defect this pane retires.
        "summary": {
            "conditions": view.condition_count(),
            "relations": view.relation_count(),
            "roots": view.root_count(),
            "structured_roots": view.structured_root_count(),
            "time_events": view.time_event_count(),
            "event_actions": view.event_action_count(),
            "clocks": view.clock_count(),
            "previous_values": view.previous_value_count(),
            "terminals": view.terminal_count(),
            "discrete_real_equations": view.discrete_real_equation_count(),
            "discrete_value_definitions": view.discrete_value_definition_count(),
        },
        "conditions": conditions,
        "relations": relations,
        "roots": roots,
        "event_actions": event_actions,
    })
}

/// **Every MLS §3.7.5 history-operator site the compile lowered, and what each
/// one became** — the re-model of the `pre()`-lowering view.
///
/// # Why this needs no instrumentation, which was measured rather than assumed
///
/// `pre_lowering_anim`'s predecessor animated `lower_pre_operator`, a pass over
/// a finished DAE that 0.10.0 does not have: `pre()` is lowered inside
/// `construction::expression`'s builtin dispatch. Both that module's doc comment
/// and `docs/tech-debt.md` predicted the replacement would have to read
/// construction through a new seam.
///
/// **It does not, because construction's work survives into the artifact.** Two
/// facts carry it, and a probe confirmed both:
///
/// 1. Every node the history-operator lowering synthesises is tagged
///    [`rumoca_ir_dae::DaeGeneration::PreValueLowering`], while a `pre()` the
///    model wrote is `Source`. **That is what lets this claim `edge(b)` *became*
///    `b and not pre(b)`** rather than merely noting both appear.
/// 2. The owner span on a generated node is the call that owns it — exactly
///    `edge(b)` — so grouping by span reassembles each expansion, and the label
///    is read from the span rather than matched by name.
///
/// # The order is provable, not chosen
///
/// Ascending arena index is insertion order, and a parent cannot reference a
/// child that does not exist yet, so within a site the highest index is the
/// root. Sites come out in the order construction reached them, because a site's
/// first node is its lowest index.
///
/// # What a smooth model yields, and why that is a report
///
/// An empty list. Most models use no history operator, and
/// `pre_lowering_anim` says so in those words; `the_pre_lowering_sites_are_only_
/// the_models_own` is the must-fire guard that an empty result means absence
/// rather than a walk that stopped looking.
fn pre_lowering_from_construction(
    view: rumoca_ir_dae::DaeView<'_>,
    // **The map, not the specimen text** (corrected 2026-11-04). A `pre`, `edge` or
    // `change` written inside an MSL component — a friction or brake model's mode
    // bookkeeping — owns a span in *that* file, and slicing the specimen for it would
    // label the site with whatever bytes sit at those offsets in the model. No corpus
    // specimen reaches that path today, so this was latent rather than live; the same
    // mistake was live next door in `unmatched_unknown_locations`. See
    // [`span_location`].
    source_map: &rumoca_core::SourceMap,
) -> Vec<crate::pre_lowering_anim::PreLoweringSite> {
    use rumoca_ir_dae::{CoordinateView, DaeGeneration, DaeProvenanceOrigin, ExpressionOperation};

    /// The lane a `pre` reference reads, under Rumoca's own variant name.
    ///
    /// **Published as Rumoca spells it.** `PreState` and `PreAlgebraic` are the
    /// event-entry snapshot lanes a continuous coordinate gets, and renaming
    /// either to something friendlier would be a claim — the mistake the
    /// `zero_crossing_conditions` episode cost three weeks.
    const fn lane_of(operation: ExpressionOperation<'_>) -> Option<&'static str> {
        match operation {
            ExpressionOperation::Coordinate(CoordinateView::PreDiscreteReal(_)) => {
                Some("PreDiscreteReal")
            }
            ExpressionOperation::Coordinate(CoordinateView::PreDiscreteValue(_)) => {
                Some("PreDiscreteValue")
            }
            ExpressionOperation::Coordinate(CoordinateView::PreState(_)) => Some("PreState"),
            ExpressionOperation::Coordinate(CoordinateView::PreAlgebraic(_)) => {
                Some("PreAlgebraic")
            }
            _ => None,
        }
    }

    /// A site's identity: the owner span, which is the compiler's own grouping.
    type SiteKey = (u64, usize, usize);

    struct Group {
        /// Terms in arena order, so the last is the root.
        steps: Vec<String>,
        lane: Option<&'static str>,
        generated: bool,
        span: rumoca_core::Span,
    }

    let mut groups: std::collections::HashMap<SiteKey, Group> = std::collections::HashMap::new();
    // First-seen order of the keys, which is construction order of the sites.
    let mut order: Vec<SiteKey> = Vec::new();

    for index in 0..view.expression_count() {
        let Some(id) = view.expression_id(index) else {
            continue;
        };
        let Some(expression) = view.expression(id) else {
            continue;
        };
        let provenance = expression.provenance();
        let operation = expression.operation();
        let lane = lane_of(operation);
        let generated = matches!(
            provenance.origin(),
            DaeProvenanceOrigin::Generated(DaeGeneration::PreValueLowering)
        );
        // **Generated is tested first on purpose.** `edge(b)`'s synthesised
        // `pre(b)` is *both* a pre-lane reference and a generated node; treating
        // it as a hand-written read would split one expansion into two sites and
        // report a `pre(b)` the model never wrote.
        if !generated && lane.is_none() {
            continue;
        }

        let span = provenance.span();
        let key: SiteKey = (span.source.0, span.start.0, span.end.0);
        let term = crate::dae_expr_format::format_expr(view, id);
        match groups.get_mut(&key) {
            Some(group) => {
                group.steps.push(term);
                group.lane = group.lane.or(lane);
                // A span owning any generated node is an expansion. A
                // hand-written `pre()` never shares its span with one.
                group.generated |= generated;
            }
            None => {
                order.push(key);
                groups.insert(
                    key,
                    Group {
                        steps: vec![term],
                        lane,
                        generated,
                        span,
                    },
                );
            }
        }
    }

    order
        .into_iter()
        .filter_map(|key| groups.remove(&key))
        .map(|group| {
            let location = span_location(source_map, &group.span);
            crate::pre_lowering_anim::PreLoweringSite {
                // The excerpt is the source text the span covers, so a site is
                // labelled `edge(b)` because Rumoca says those bytes own it.
                call: location
                    .as_ref()
                    .and_then(|l| l["excerpt"].as_str())
                    .map_or_else(
                        || "<span in no registered source>".to_owned(),
                        |excerpt| excerpt.trim().to_owned(),
                    ),
                line: location
                    .as_ref()
                    .and_then(|l| l["line"].as_u64())
                    .map(|line| line as usize),
                lane: group.lane.map(str::to_owned),
                generated: group.generated,
                steps: group.steps,
            }
        })
        .collect()
}

/// Solve lowering (phase 8) — the DAE lowered to a `SolveModel`, the
/// solvable form the simulator runs (residual programs, variable layout, mass
/// matrix, Jacobian sparsity).
///
/// Unlike the structural/IC stages, `SolveModel` derives `Serialize`, so we
/// can use `serde_json::to_value(&sm)` directly instead of building JSON
/// manually. This is the simplest stage extractor — just serialize and wrap.
///
/// Rumoca API: `lower_solve_problem(&dae)` transforms the mathematical DAE
/// into the solver's executable form.
fn solve_lowering_stage(result: Option<&PhaseResult>) -> Stage {
    if let Some(stage) = not_reached_stage(result) {
        return stage;
    }
    let cr = unwrap_success(result);
    match rumoca_phase_solve::lower_solve_problem(&cr.dae) {
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
fn incidence_to_json<'dae>(
    inc: &rumoca_phase_structural::Incidence<'dae>,
    view: Option<rumoca_ir_dae::DaeView<'dae>>,
) -> serde_json::Value {
    // **No short form any more, and that is faithful rather than lazy.**
    // This used `expr_format::equation_labels`, which rendered each equation in
    // a "short" form — the bare right-hand side for a residual, `x = rhs` when
    // the equation had been solved for `x`. A 0.10.0 equation *is* a residual
    // and records no solved-for variable, so there are no longer two forms to
    // choose between; every row reads `0 = …`.
    let eq_texts: Vec<String> = view
        .map(|view| {
            (0..view.continuous_equation_count())
                .map(|index| {
                    view.continuous_equation(index).map_or_else(
                        || format!("f_x[{index}]"),
                        |equation| crate::dae_expr_format::format_equation(view, equation),
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let rows: Vec<serde_json::Value> = inc
        .eq_unknowns
        .iter()
        .enumerate()
        .map(|(i, cols)| {
            let mut sorted: Vec<usize> = cols.to_vec();
            sorted.sort_unstable();
            // **Rumoca's own labelling function**, not a copy of it. Matching
            // lookups in `IncidenceMatrix::from_report` correlate by this exact
            // string, so a reimplementation that drifts silently unmatches
            // everything — which is what happened here (see
            // `partial_matching_to_json`). Without a DAE there is no origin to
            // look up, and the bare ref is what `equation_label` would return
            // anyway.
            let eq_name = match view {
                Some(view) => rumoca_phase_structural::equation_label(view, &inc.equation_refs[i]),
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
    // **`Incidence::unknown_names` is gone; the names are derived.** 0.10.0
    // carries `unknowns: Vec<UnknownId>` -- identities, not strings -- and
    // `unknown_label` is the crate's own way to name one. That is the point of
    // using it rather than formatting the id: these strings have to be the ones
    // `StructuralReport` uses, or the two panes cannot be correlated.
    let unknown_names: Vec<String> = inc
        .unknowns
        .iter()
        .enumerate()
        .map(|(i, unknown)| match view {
            Some(view) => rumoca_phase_structural::unknown_label(view, *unknown),
            None => format!("unknown[{i}]"),
        })
        .collect();
    serde_json::json!({
        "n_eq": inc.n_eq,
        "n_var": inc.n_var,
        "unknown_names": unknown_names,
        "rows": rows,
    })
}

// `before_report_json` was deleted in the v0.10.0 rebase rather than ported.
//
// It built the Index Reduction tab's "before" pane — the raw, pre-reduction
// system with its rank deficiency highlighted. Porting it would have produced a
// function nothing calls: live-looking plumbing with no caller sends the next
// session hunting for why it never runs. **The re-model replaced it** (2026-11-04):
// `index_reduction_from_observation` publishes `before` as the Structural stage's
// own report, observed rather than rebuilt. `git show` has the old function.
// *(This said the tab was "disabled awaiting a re-model" until the 2026-11-07 sweep.)*

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
fn partial_matching_to_json<'dae>(
    inc: &rumoca_phase_structural::Incidence<'dae>,
    match_eq: &[Option<usize>],
    view: rumoca_ir_dae::DaeView<'dae>,
) -> serde_json::Value {
    let pairs: Vec<serde_json::Value> = match_eq
        .iter()
        .enumerate()
        .filter_map(|(eq_idx, var_idx)| {
            var_idx.map(|v| {
                serde_json::json!({
                    "equation": rumoca_phase_structural::equation_label(
                        view, &inc.equation_refs[eq_idx],
                    ),
                    // The AUTHORITATIVE identity, not a derivation: the reference
                    // Rumoca kept is in hand here, unlike in `structural_to_json`
                    // where `StructuralReport` has already discarded it. Without
                    // this, pointing at a row of the index-reduction Before pane
                    // resolved to nothing — the pane names 321 equations across the
                    // notebook and could identify none of them.
                    "id": inc.equation_refs[eq_idx].to_string(),
                    "unknown": rumoca_phase_structural::unknown_label(view, inc.unknowns[v]),
                })
            })
        })
        .collect();
    serde_json::Value::Array(pairs)
}

/// Convert a `StructuralError` into structured JSON for the UI and for Claude.
///
/// Takes the compile's **source map** so unmatched unknowns can be reported with the
/// line that declares them (ideas #45) — *in the file that declares them*, which is
/// usually an MSL component rather than the specimen. It took the specimen text until
/// 2026-11-04 and mislabelled every library-declared unknown; [`span_location`] holds
/// the measurement. It no longer takes the incidence either: the only thing that used
/// it was the rank-deficiency computation, which was *wrong* to use it — see the
/// comment on that field.
fn structural_error_to_json(
    e: &rumoca_phase_structural::StructuralError,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    match e {
        rumoca_phase_structural::StructuralError::Singular {
            n_equations,
            n_unknowns,
            n_matched,
            unmatched_equations,
            unmatched_unknowns,
            unmatched_unknown_spans,
            over_determined_block,
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
            // **The identities beside the labels, added 2026-11-03.**
            //
            // v0.10.0 decorates these strings: `unmatched_equations` now reads
            // `"f_x[46] (SI.Angle phi)"` where 0.9.20 gave a bare id. HRW publishes
            // Rumoca's own string unchanged — that is the rule — but a *decorated*
            // label is not an identity, and pointing at one resolves to nothing.
            // Thirteen labels across the notebook were in that state, which
            // `tests_equation_identity::every_committed_trace_identifies_every_equation_it_names`
            // found by walking the published trees rather than by anyone checking
            // this site.
            //
            // So the bare ids go beside them, which is one of the two shapes that
            // checker documents (`{"equations": [label…], "ids": [id…]}`). **Derived
            // from the labels, not recomputed** — a second derivation could disagree
            // with the string it is supposed to identify, and an id that names a
            // different equation than the label beside it is worse than no id.
            "unmatched_equation_ids": unmatched_equations
                .iter()
                .map(|l| crate::equation_id_from_label(l))
                .collect::<Vec<_>>(),
            "unmatched_unknowns": unmatched_unknowns,
            // The point of ideas #45: where in *Doug's source* each untethered
            // unknown is declared, so a failure can be diagnosed rather than merely
            // described.
            "unmatched_unknown_locations": unmatched_unknown_locations(
                source_map,
                unmatched_unknowns,
                unmatched_unknown_spans,
            ),
            // **New in v0.10.0, and it answers the question the counts raise.**
            // `n_equations > n_unknowns` says the system is over-determined
            // somewhere; this says *where*, as the witness block's own shape
            // plus a sample of its equations. Published because a singular
            // report is the one place a reader most needs to stop guessing, and
            // `sample` is Rumoca's own selection rather than HRW's.
            "over_determined_block": {
                "equations": over_determined_block.equations,
                "unknowns": over_determined_block.unknowns,
                "sample": over_determined_block.sample,
            },
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
/// Convert a [`rumoca_phase_solve::LowerError`] into structured JSON for UI
/// rendering.
///
/// **Much thinner than its 0.9.20 predecessor, and that is faithful rather than
/// lazy.** `SolveModelLowerError::Structural` carried the
/// `rumoca_phase_structural::StructuralError` itself, so this could publish
/// `n_equations`, `n_unknowns`, `n_matched` and the unmatched equation and
/// unknown lists. 0.10.0's `LowerError::Structural` carries `reason: String` —
/// the structured failure is rendered before it reaches here, and the numbers
/// are not recoverable from the error.
///
/// So the counts are **absent rather than reconstructed**. Re-deriving them by
/// running structural analysis again would produce numbers from a second
/// execution and present them as this failure's, which is the one thing this
/// repository will not do. The Structural stage still reports them, for the run
/// that actually failed.
fn solve_lower_error_to_json(e: &rumoca_phase_solve::LowerError) -> serde_json::Value {
    use rumoca_phase_solve::LowerError;
    let msg = format!("{e}");
    // Rumoca's own variant name, published rather than renamed: a friendlier
    // label here would be a claim about the failure that the compiler did not
    // make.
    let (kind, reason, span) = match e {
        LowerError::Unsupported { reason, span } => ("Unsupported", reason, Some(span)),
        LowerError::NonComputable { reason, span } => ("NonComputable", reason, Some(span)),
        LowerError::Structural { reason, span } => ("Structural", reason, span.as_ref()),
        LowerError::ContractViolation { reason, span } => ("ContractViolation", reason, Some(span)),
        LowerError::UnspannedContractViolation { reason } => {
            ("UnspannedContractViolation", reason, None)
        }
        // New in 0.10.4.
        LowerError::BudgetExceeded { reason, span } => ("BudgetExceeded", reason, Some(span)),
    };
    let guidance = match e {
        LowerError::Structural { .. } => {
            "Solve lowering could not order the system. The Structural stage shows the \
             matching and blocks for the run that failed."
        }
        LowerError::NonComputable { .. } => {
            "An equation reached solve lowering in a form with no computable program."
        }
        LowerError::Unsupported { .. } => {
            "The model uses a construct solve lowering does not implement yet."
        }
        LowerError::ContractViolation { .. } | LowerError::UnspannedContractViolation { .. } => {
            "Solve lowering detected a violated internal contract \u{2014} a compiler defect \
             rather than a model error, and worth an upstream-issues entry."
        }
        // **Deliberately not grouped with the contract violations above**, which
        // call themselves a compiler defect. A budget refusal is neither that nor a
        // malformed model: lowering declined an allocation *before attempting it*
        // because a resource limit would be exceeded. Saying "compiler defect" here
        // would send a reader to file an upstream issue about a working limit.
        LowerError::BudgetExceeded { .. } => {
            "Solve lowering refused an allocation before attempting it, because a \
             resource budget would be exceeded. The reason names the budget."
        }
    };
    let mut json = serde_json::json!({
        "kind": kind,
        "message": msg,
        "reason": reason,
        "guidance": guidance,
    });
    if let Some(span) = span {
        let obj = json
            .as_object_mut()
            .expect("the literal above constructs an object");
        obj.insert("span_start".to_owned(), span.start.0.into());
        obj.insert("span_end".to_owned(), span.end.0.into());
    }
    json
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
        // **New in v0.10.0, and it is a compaction rather than a third kind of
        // block.** A regular equation family whose rows are each their own
        // scalar block is reported once with its point count, instead of
        // expanded into one line per element — upstream's own doc gives the
        // reason: a 2048-element array would otherwise bury the rest of the
        // dump.
        //
        // **No `id` field, deliberately.** Every other arm carries one because a
        // click on a spy-plot block captures it, and `equation_id_from_label`
        // derives that id from a label naming *one* equation. This arm names a
        // family covering `point_count` domain points, so there is no single
        // equation for an id to mean. Emitting one would make a capture resolve
        // to whichever row the derivation happened to land on — the kind of
        // confident wrong answer `identity-and-provenance.md` forbids. The
        // counts are published instead, so a reader can see what the row covers.
        BlockReport::StructuredScalar {
            origin,
            point_count,
            equations_per_point,
            first_row,
            row_count,
        } => serde_json::json!({
            "kind": "structured_scalar",
            "size": point_count * equations_per_point,
            "origin": origin,
            "point_count": point_count,
            "equations_per_point": equations_per_point,
            // **The rows the family accounts for**, published 2026-11-06 so a consumer
            // checking that the blocks PARTITION the equations can see what this one
            // covers. Without it `fidelity::check_f4` had no way to resolve a compacted
            // family to rows -- it resolves names through the incidence labels and a
            // family has no per-row label -- so it reported every such block as naming
            // no equations AND counted the family's rows as holes in the partition.
            // Loud and blind at once, from v0.10.0 until this. `docs/tech-debt.md`.
            "first_row": first_row,
            "row_count": row_count,
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
/// `lib::unidentified_equation_labels` now runs for them instead.
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
fn flatten_stage(result: Option<&PhaseResult>, specimen: SpanFile<'_>) -> Stage {
    match result {
        Some(PhaseResult::Success(cr)) => match serde_json::to_value(&cr.flat) {
            Ok(v) => {
                let mut s = Stage::ok(v);
                // From the same `cr.flat` just serialized, so the renderings and the
                // tree cannot describe different models.
                s.rendered = crate::equation_text::for_flatten(&cr.flat);
                s
            }
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
                    let diag_json = diagnostics_to_json(diagnostics, specimen, None);
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
                    // `last_successful_stage` is unaffected: its `ok` predicate
                    // requires `value.is_some()` (and, since 2026-11-07, a
                    // non-`Failed` outcome), and `info` carries no value, so Flatten does not become the furthest good stage. The
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
/// Found by `docs/fixture-labs/dae-construction.md`, whose counterexample stop opens
/// this exact tab on `UnbalancedShaft`.
fn dae_absent_stage(result: Option<&PhaseResult>, specimen: SpanFile<'_>) -> Stage {
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
                dae_construction_error_to_json(error, error_code, diagnostics, specimen),
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
/// Recursively runs the JSON tree. When it finds an object key that matches
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
/// Map every document the session holds to the `SourceId` its URI hashes to.
///
/// **Needed because 0.10.0's `Location` carries a `SourceId` rather than a file
/// name**, and HRW wants the name — it is what `get_document` takes and what a
/// "jump to declaration" has to open.
///
/// **This is an identity index, not a name search.** `SourceId`s are derived
/// from the URI by `source_id_for_name`, the same function `SourceMap::add`
/// uses, so hashing each known URI reproduces exactly the ids the IR carries.
/// Nothing here compares or matches text, which is what
/// `docs/identity-and-provenance.md` forbids; a URI the session does not hold
/// simply has no entry.
fn source_names(uris: &[&str]) -> std::collections::HashMap<rumoca_core::SourceId, String> {
    uris.iter()
        .map(|uri| (rumoca_core::source_id_for_name(uri), (*uri).to_owned()))
        .collect()
}

fn build_def_index(
    tree: &rumoca_ir_ast::ClassTree,
    value: &serde_json::Value,
    source_names: &std::collections::HashMap<rumoca_core::SourceId, String>,
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
                // `Location` carries a `SourceId` on 0.10.0 where it used to
                // carry the file name itself. The map is keyed by the id the
                // session's own document URIs hash to, so this is an identity
                // lookup rather than a name match — and `None` when the
                // declaring document is not one the session holds, which is a
                // real state rather than a reason to invent a path.
                file_name: source_names.get(&class.location.source).cloned(),
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
#[allow(
    unsafe_code,
    reason = "libc::pipe writes exactly two fds through the pointer, and `fds` is a \
              [i32; 2], so the write stays in bounds; the result is checked. A real OS \
              pipe is required because Rumoca's tracing and C-level output write to fd \
              1/2 below Rust's stdout"
)]
unsafe fn create_pipe(fds: &mut [i32; 2]) -> bool {
    unsafe { libc::pipe(fds.as_mut_ptr()) == 0 }
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "libc::pipe writes exactly two fds through the pointer, and `fds` is a \
              [i32; 2], so the write stays in bounds; the result is checked. A real OS \
              pipe is required because Rumoca's tracing and C-level output write to fd \
              1/2 below Rust's stdout"
)]
unsafe fn create_pipe(fds: &mut [i32; 2]) -> bool {
    // _O_BINARY = 0x8000: no CR/LF translation.
    unsafe { libc::pipe(fds.as_mut_ptr(), 65536, 0x8000) == 0 }
}

#[cfg(unix)]
#[allow(
    unsafe_code,
    reason = "takes ownership of a pipe read end OutputCapture::start has just created and \
              hands to exactly one reader thread, so the fd is owned once and closed once, \
              by that File's drop"
)]
unsafe fn file_from_raw_fd(fd: i32) -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    unsafe { std::fs::File::from_raw_fd(fd) }
}

#[cfg(windows)]
#[allow(
    unsafe_code,
    reason = "takes ownership of the OS handle behind a pipe read end OutputCapture::start \
              has just created and hands to exactly one reader thread, so it is owned once \
              and closed once, by that File's drop"
)]
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
    #[allow(
        unsafe_code,
        reason = "dup/dup2/close on fds 1 and 2 and on pipes this function creates. Every \
                  failure path closes what it opened before returning None, and the saved \
                  originals are restored exactly once, by Drop. This is the one place HRW \
                  can corrupt process-wide state, which is why the lint guards it"
    )]
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
    #[allow(
        unsafe_code,
        reason = "restores fds 1 and 2 from the copies start() saved, then closes each copy \
                  once; this is what makes the reader threads see EOF and exit"
    )]
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
    // **Initialization was listed here and should not have been.** It called
    // `rumoca_phase_structural::build_ic_plan` (on 0.10.x it reads the pins of
    // `inspect_prepare_for_solve`), a real algorithm in an instrumented
    // crate — the claim was written after reading the first ten lines of
    // `initialization_stage` and stopping before the call. The
    // `calls-no-Rumoca-function` check below exists because of that.
    (
        "Events",
        None,
        "HRW renders this tab by walking the DAE's own condition, root and \
         event-action arenas through `dae.inspect`; no Rumoca function is called. \
         *(The reason named `dae.events` until 2026-11-03 \u{2014} a 0.9.20 field, and \
         there is no such field now: the re-model reads nine arenas.)*",
    ),
    // **Three phases went silent on v0.10.0; two of them still are on v0.10.4.**
    //
    // *2026-11-04, re-measured on v0.10.4 — `tracing::` call sites per phase crate:*
    // flatten 9, instantiate 2, structural **2**, **solve 2**, resolve 1, and
    // **typecheck 0, dae 0**. v0.10.0 had cut tracing sharply — the
    // `structural_trace!` wrapper that accounted for 27 sites is gone — and the two
    // below still emit nothing at all.
    //
    // **Solve Lowering was the third and came back in v0.10.4**, with two
    // `tracing::debug!` sites in `lower/scalar/register_ledger.rs`. The test below
    // is what removed it, failing with *"now has 2 tracing call site(s)"* on the
    // 0.10.4 rebase — the same way it removed Typecheck on 2026-08-04. That is the
    // second time this mechanism has retired its own entry, which is the argument
    // for pinning a claim of absence to something that can notice.
    //
    // **Typecheck is a re-entry, which is the mechanism working.** It was on this
    // list until 2026-08-04, when the crate gained tracing and the test below
    // removed it by failing with "now has 8 tracing call site(s)". The same test
    // will force all three off again the moment they speak, so these entries are
    // version facts that cannot quietly become permanent.
    (
        "Typecheck",
        Some("rumoca-phase-typecheck"),
        "the v0.10.0 crate emits no tracing at all; it had 8 call sites on 0.9.20, \
         so this is a regression in observability rather than a property of the phase",
    ),
    (
        "DAE",
        Some("rumoca-phase-dae"),
        "the v0.10.0 crate emits no tracing; the twenty-odd single-purpose modules \
         that carried it collapsed into `construction/` and the calls did not survive",
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
///
/// # v0.10.0 FALSIFIED THE PREMISE, AND THIS IS THAT CHANGE
///
/// *2026-11-03.* The measurement above is still accurate about 0.9.20 and the reasoning
/// built on it was sound. What it rested on is the row reading **0 errors** for a good
/// model: the library produced warnings and notes, never errors, so "any error is this
/// model's error" was safe.
///
/// **On v0.10.0 the MSL produces about 75 error-severity resolve diagnostics** — all of
/// the form *"reference to non-constant variable `X` from a lexically nested class is
/// not allowed (MLS §5.3.1)"*, over the `Modelica.Mechanics` friction and brake state
/// machines (`startForward`, `v_relfric`, `locked`, `a_relfric`, `f0_max`). Whether
/// Rumoca is right to reject that code is a question for upstream and is recorded as
/// one; **it is not this function's business either way.**
///
/// What it did to HRW was total: **every specimen's Resolve stage read `Failed`**,
/// including `TwoLoops`, which contains no `Modelica.` reference at all. Nine stages
/// after it still read `Ok` with IR, so the pane asserted the pipeline stopped at a
/// stage it demonstrably ran past — breaking the one-`Failed`-per-compile invariant
/// `the_corpus_outcome_matrix_is_unchanged` pins, in 25 rows at once.
///
/// **So the fix is the model-scoped subset that was not available in 2026-08.** The
/// note above rejected `global_resolution_failure` because it returned the same
/// library-wide set; the subset is now built from the diagnostics themselves, by
/// [`diagnostic_source`], which is identity rather than name matching.
///
/// **The failure direction is the dangerous one, which is why two tests bracket it.**
/// If the source comparison matched nothing — a path spelled differently from the name
/// the session registered — every model's Resolve would read clean and HRW would
/// silently stop reporting resolve failures, which is exactly the hazard
/// `Session::strict_compile_resolved`'s own doc comment warns about. So
/// `a_broken_model_still_reports_its_own_resolve_error` requires a genuinely broken
/// specimen to still fail, and the corpus matrix requires all 25 rows to be right.
fn resolve_diagnostics_indicate_failure(diags: &rumoca_core::Diagnostics, uri: &str) -> bool {
    !model_resolve_errors(diags, uri).is_empty()
}

/// The `SourceId` a diagnostic is *about*, using Rumoca's own precedence.
///
/// Primary label, then the first label — the order
/// `rumoca_core::Diagnostic::to_miette_with_source_map` uses to decide which file to
/// render a diagnostic against. **Agreeing with Rumoca about where a diagnostic lives
/// is the point**: a second opinion here would be HRW re-deriving an attribution the
/// compiler already makes, which is the pattern this project keeps having to undo.
///
/// `None` for a diagnostic with no labels at all. Those are counted as **not** this
/// model's, because the alternative — treating an unplaceable error as local — is how
/// the library's errors became every specimen's in the first place.
fn diagnostic_source(d: &rumoca_core::Diagnostic) -> Option<rumoca_core::SourceId> {
    d.labels
        .iter()
        .find(|l| l.primary)
        .or_else(|| d.labels.first())
        .map(|l| l.span.source)
}

/// Error diagnostics that belong to `uri`'s own source, not to the library.
///
/// `source_id_for_name` derives the id from the name deterministically, so no source
/// map is needed and no lookup can go stale.
fn model_resolve_errors<'a>(
    diags: &'a rumoca_core::Diagnostics,
    uri: &str,
) -> Vec<&'a rumoca_core::Diagnostic> {
    let own = rumoca_core::source_id_for_name(uri);
    diags
        .iter()
        .filter(|d| d.is_error() && diagnostic_source(d) == Some(own))
        .collect()
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
/// Doug found it running the lab that teaches DAE construction, and named the class:
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
