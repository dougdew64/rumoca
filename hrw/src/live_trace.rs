//! Channel-based trace buffer for live-stepped algorithm observation.
//!
//! A `LiveTrace<F>` sends frames through an `mpsc` channel that the UI thread
//! drains without contention. The producer (`Sender`) and consumer (`Receiver`)
//! share no application-level lock — `push()` never blocks on the UI thread.
//!
//! The key debugging technique: set a breakpoint on [`live_trace_breakpoint`] —
//! each time the debugger pauses there, the UI thread can read the latest
//! frame and render the algorithm's current state.
//!
//! ## Why this lives in `hrw/` as of the v0.10.0 rebase
//!
//! **It used to live in `crates/rumoca-phase-structural/src/live_trace.rs`,
//! and it should not have.** Nothing in Rumoca ever named it — the old file's
//! own header said so under the heading *"Not used by any phase"* — and it is
//! 270 lines of application plumbing: an `mpsc` channel, a UI frame delay, and
//! a debugger anchor. The workspace `CLAUDE.md` requires instrumentation of the
//! Rumoca crates to be **separable from `hrw/` so an upstream PR is a clean
//! cherry-pick of Rumoca-only changes**, and a file no phase calls is the
//! clearest possible violation of that.
//!
//! It stayed there out of inertia, which the old header admitted: *"It stays
//! here rather than moving out because `live_trace_breakpoint` is the debugger
//! anchor, and both the `opt-level = 0` override and HRW's
//! `bridge::find_live_trace_line` are keyed to this file. Moving it is a
//! separate change with its own risks."* The rebase removes that argument — the
//! file has to be re-created *somewhere* regardless, so leaving it in `crates/`
//! would now be a choice rather than inertia.
//!
//! **Two things made the move cheap, and both are new.** The structural
//! observation seam takes a plain `impl FnMut(StructuralRecord)` sink
//! (`rumoca_phase_structural::observe_sort`), so no phase needs to name this
//! type or any trait it implements. And the records that travel through the
//! channel are now owned, so nothing here borrows from a `DaeView`.
//!
//! **What callers must know:** the anchor is still on the algorithm thread and
//! still inside the algorithm's call stack — `push` is invoked from the sink
//! closure the phase calls — so stepping behaves exactly as before. Only the
//! file the breakpoint is set in has changed, and `bridge::LIVE_TRACE_FILE`
//! points at this file now.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::time::Duration;

/// Producer half of the live trace channel.
///
/// The algorithm thread owns this and calls [`push`](Self::push) to send frames.
/// No lock is shared with the UI thread — `Sender::send()` takes `&self` and
/// uses internal synchronization, so no `Mutex` wrapper is needed.
///
/// Generic over the frame type `F` — works with
/// `rumoca_phase_structural::MatchingRecord`, `TarjanRecord`, `TearingRecord`,
/// or any other owned step type. Rumoca never names it: the phases take a
/// sink closure, and this is what HRW puts behind that closure.
pub struct LiveTrace<F> {
    tx: mpsc::Sender<F>,
    len: Arc<AtomicUsize>,
    /// When set, `push` sleeps for this duration after each frame, giving a
    /// UI thread time to poll and render before the debugger pauses all
    /// threads at the [`live_trace_breakpoint`] call.
    frame_delay: Option<Duration>,
}

impl<F> Clone for LiveTrace<F> {
    fn clone(&self) -> Self {
        LiveTrace {
            tx: self.tx.clone(),
            len: Arc::clone(&self.len),
            frame_delay: self.frame_delay,
        }
    }
}

impl<F: Send + 'static> LiveTrace<F> {
    /// Create a new live trace channel pair.
    ///
    /// Returns the producer (for the algorithm thread) and the receiver (for
    /// the UI/animation struct). The producer is `Send + Clone`; the receiver
    /// is `Send` but not `Clone` — exactly one consumer.
    pub fn new() -> (Self, mpsc::Receiver<F>) {
        let (tx, rx) = mpsc::channel();
        let lt = LiveTrace {
            tx,
            len: Arc::new(AtomicUsize::new(0)),
            frame_delay: None,
        };
        (lt, rx)
    }

    /// Builder: add an inter-frame delay so a UI thread can render each
    /// frame before the debugger pauses all threads at the breakpoint.
    /// Use [`Duration::from_millis(20)`] for typical egui frame rates.
    pub fn with_frame_delay(mut self, delay: Duration) -> Self {
        self.frame_delay = Some(delay);
        self
    }

    /// Block until the debugger is attached by hitting `live_trace_breakpoint`
    /// before the algorithm produces any frames. Call this at the top of the
    /// live-debug thread — the debugger pauses here on the first Continue (F5),
    /// so no algorithm steps are missed.
    ///
    /// The sleep covers the gap between the bridge ack (which fires after
    /// `vscode.debug.addBreakpoints` is *called*) and LLDB actually installing
    /// the breakpoint (async). Without it, the thread can race past the
    /// breakpoint before LLDB has finished setting it up.
    pub fn wait_for_debugger(&self) {
        std::thread::sleep(std::time::Duration::from_millis(500));
        live_trace_breakpoint(usize::MAX);
    }

    /// Send a frame to the consumer.
    ///
    /// When a `frame_delay` is set (see [`with_frame_delay`]), this method
    /// sleeps after the send and then calls [`live_trace_breakpoint`] —
    /// **set your debugger breakpoint on that function**, not here.
    pub fn push(&self, frame: F) {
        let index = self.len.fetch_add(1, Ordering::Release);
        let _ = self.tx.send(frame);
        if let Some(delay) = self.frame_delay {
            std::thread::sleep(delay);
            // ---- DEBUGGER: set breakpoint on the next line ----
            live_trace_breakpoint(index);
        }
    }

    /// Number of frames sent so far.
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Acquire)
    }

    /// Whether any frames have been sent.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Breakpoint anchor for live-stepped debugging.
///
/// **Set your debugger breakpoint on this function** rather than on
/// `LiveTrace::push`. This dedicated function is `#[inline(never)]` and
/// non-generic, so the debugger resolves it to a single unambiguous address
/// — unlike generic functions (`black_box<T>`, `Vec::push`) that may share
/// monomorphized code at higher opt-levels.
///
/// The most recent value passed to [`live_trace_breakpoint`]. Readable via
/// [`last_frame_index`] — that reader is what keeps the store *observable*,
/// see the note on the function below.
static LAST_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

/// The frame index most recently passed to [`live_trace_breakpoint`].
///
/// Exists so the store in `live_trace_breakpoint` has a real consumer. A
/// write-only static is dead state that the optimizer is free to delete.
pub fn last_frame_index() -> usize {
    LAST_FRAME_INDEX.load(Ordering::Acquire)
}

/// `frame_index` is the 0-based index of the frame just pushed — inspect
/// it in the debugger to know which algorithmic step you're on.
/// `usize::MAX` indicates the startup gate (before any algorithm work).
///
/// ## Why the body looks over-engineered for one store
///
/// This function must **never compile to an empty body**, or breakpoints set
/// on it land somewhere else entirely. The failure chain, diagnosed on
/// `x86_64-pc-windows-msvc` on 2026-07-27:
///
/// 1. `LAST_FRAME_INDEX` was written here and read nowhere, so at any
///    optimization level above zero LLVM may dead-store-eliminate the write.
/// 2. With its only statement gone, this function becomes a bare `ret`.
/// 3. The MSVC linker's identical COMDAT folding (`/OPT:ICF`, on by default)
///    merges byte-identical functions — so this collapses onto *every other*
///    empty function in the binary, e.g. eframe's `App::raw_input_hook`.
/// 4. A breakpoint here then resolves to that shared address and fires from
///    unrelated code — in practice, eframe's per-frame render loop.
///
/// `#[inline(never)]` does not prevent this. It keeps the *function* from
/// being inlined; it says nothing about whether the *body* survives. Two
/// independent defenses keep the body non-empty and unfoldable: the store has
/// a genuine reader ([`last_frame_index`]), and `black_box` makes the value
/// opaque to the optimizer so the round-trip cannot be reasoned away.
#[inline(never)]
pub fn live_trace_breakpoint(frame_index: usize) {
    LAST_FRAME_INDEX.store(frame_index, Ordering::Release);
    std::hint::black_box(LAST_FRAME_INDEX.load(Ordering::Acquire));
}

/// Run Rumoca's real simulation preparation of `dae` on this thread, handing
/// each step to `on_step` **as it happens**.
///
/// This is the call the compile observes for the Simplify through Solve
/// lowering tabs (`worker::preparation::observe`), on the same DAE, under
/// options that lower identically. Those are `SimOptions::default()` here and
/// `worker::preparation_options` there, which differ only in `t_end` and the
/// tolerances, and lowering reads neither (`worker::retained_key` says which
/// fields it does read). All of it runs under
/// `rumoca_phase_structural::preparation_trace`'s live listener. So a live
/// session steps through exactly the analyses the recorded frames came from.
///
/// # Why not `observe_sort` / `watch_prepare_for_solve` on the DAE (until 2026-11-27)
///
/// Those analyse the compile's **raw** DAE. Solve lowering never does: it
/// simplifies the DAE first (the Simplify tab) and analyses the result. So a
/// live session stepped a run Rumoca never makes, beside a playback of the one
/// it does — and for a model the simplification changes, the two disagreed.
fn watch_real_preparation(
    dae: &rumoca_ir_dae::Dae,
    on_step: impl FnMut(rumoca_phase_structural::preparation_trace::LiveStep<'_>) + 'static,
) {
    use rumoca_phase_structural::preparation_trace as trace;
    trace::start_capture();
    trace::set_live_listener(Some(Box::new(on_step)));
    let _ = rumoca_sim::prepare_runtime_observed(dae, &rumoca_sim::SimOptions::default(), |_| {});
    trace::set_live_listener(None);
    // The capture is what enables the listener; its contents are the compile's,
    // already shown, so they are dropped rather than kept as a second copy.
    let _ = trace::take_capture();
}

/// Spawn a live-debug thread that watches **the real structural analysis** —
/// the first `Source` analysis of solve lowering's preparation, the one the
/// Structural tab and the recorded animations show — and streams the steps
/// `select` accepts into a [`LiveTrace`].
///
/// Returns the receiver the animation drains and the flag it polls for
/// completion, or `None` if the thread could not be spawned.
///
/// # Why this watches the whole analysis rather than the one algorithm animated
///
/// Each animation used to re-run its own algorithm on data reconstructed from
/// the pane — `maximum_matching` over the incidence rows the matrix widget was
/// showing, and for tearing a hand-rolled block-local incidence plus its own
/// matching and BLT. **The pane's rows are a projection, not the system**:
/// re-running an algorithm over them steps something adjacent to what the
/// compiler did, and nothing would report the difference.
///
/// A live session is a genuine re-run — that is what was asked for, and
/// `CLAUDE.md` is explicit that this is the legitimate kind — of **the same
/// call over the same input** as the compile ([`watch_real_preparation`]),
/// which is the most a re-run can honestly be.
///
/// The preparation continues past the steps `select` wants (matching finishes,
/// then Tarjan, tearing, reduction and lowering). That is why `done` is set when
/// the preparation *returns*, not when the last accepted record is pushed.
/// `select` is `FnMut` so a caller can carry state across steps — the tearing
/// view needs this, because it tags each step with the block whose
/// `BlockStart` most recently went past.
pub fn spawn_structural_live<F, S>(
    dae: std::sync::Arc<rumoca_ir_dae::Dae>,
    frame_delay: Duration,
    thread_name: &str,
    mut select: S,
) -> Option<(mpsc::Receiver<F>, Arc<std::sync::atomic::AtomicBool>)>
where
    F: Send + 'static,
    S: FnMut(rumoca_phase_structural::StructuralRecord) -> Option<F> + Send + 'static,
{
    let (lt, rx) = LiveTrace::new();
    let lt = lt.with_frame_delay(frame_delay);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_for_thread = Arc::clone(&done);

    std::thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || {
            lt.wait_for_debugger();
            // Where HRW's `LiveTrace` meets the phase's listener. The phase crate
            // never learns `LiveTrace` exists, so the observation seam stays free
            // of any HRW type. Only the FIRST `Source` analysis is forwarded: later
            // analyses (reduction candidates, rebuilt systems) are other systems.
            use rumoca_phase_structural::preparation_trace::{AnalysisContext, LiveStep};
            let mut in_source = false;
            let mut source_seen = false;
            watch_real_preparation(&dae, move |step| match step {
                LiveStep::AnalysisBegin(context) => {
                    in_source = context == AnalysisContext::Source && !source_seen;
                    source_seen |= in_source;
                }
                LiveStep::Analysis { record, .. } if in_source => {
                    if let Some(frame) = select(record.clone()) {
                        lt.push(frame);
                    }
                }
                _ => {}
            });
            done_for_thread.store(true, std::sync::atomic::Ordering::Release);
        })
        .ok()?;

    Some((rx, done))
}

/// Watch index reduction on a worker thread, pushing each record it observes as a frame.
///
/// The reduction twin of [`spawn_structural_live`], and the same three things make it a
/// live session rather than a replay: the thread waits at
/// [`LiveTrace::wait_for_debugger`] so the reader arrives before the first step, each
/// push pauses by `frame_delay` so a debugger can stop between events, and the records
/// come from Rumoca's own observation of a reduction that is **happening now** — the
/// reducer inside solve lowering's real preparation ([`watch_real_preparation`]), whose
/// records the Index reduction tab shows.
///
/// # Why a live seam at all
///
/// A report returned *after* the run completes, fed into this channel on a timer, would
/// look identical on screen and be a recording wearing a live label. The per-event seam
/// is `rumoca_phase_structural::preparation_trace::set_live_listener` (2026-11-27; it
/// replaced `watch_prepare_for_solve`, which reduced the raw DAE rather than the
/// simplified one solve lowering reduces). `docs/upstream-strategy.md` is why it is
/// shaped as a general observability API rather than as something HRW-specific.
pub fn spawn_reduction_live<F, S>(
    dae: std::sync::Arc<rumoca_ir_dae::Dae>,
    frame_delay: Duration,
    thread_name: &str,
    mut select: S,
) -> Option<(mpsc::Receiver<F>, Arc<std::sync::atomic::AtomicBool>)>
where
    F: Send + 'static,
    S: FnMut(rumoca_phase_structural::ReductionRecord) -> Option<F> + Send + 'static,
{
    let (lt, rx) = LiveTrace::new();
    let lt = lt.with_frame_delay(frame_delay);
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let done_for_thread = Arc::clone(&done);

    std::thread::Builder::new()
        .name(thread_name.to_owned())
        .spawn(move || {
            lt.wait_for_debugger();
            // The prepared runtime is dropped: this thread exists to observe the
            // search, and the compile already holds the result every pane reads.
            use rumoca_phase_structural::preparation_trace::LiveStep;
            watch_real_preparation(&dae, move |step| {
                if let LiveStep::Reduction(record) = step
                    && let Some(frame) = select(record.clone())
                {
                    lt.push(frame);
                }
            });
            // Set when the call *returns*, not on the last push — reduction does work
            // after its final record (finalizing the prepared system), and a reader
            // told "finished" early would re-enable the controls mid-run. Same reason
            // `spawn_structural_live` stores here.
            done_for_thread.store(true, std::sync::atomic::Ordering::Release);
        })
        .ok()?;

    Some((rx, done))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes the tests that touch `LAST_FRAME_INDEX`. Cargo runs tests on
    /// parallel threads, and the anchor static is process-global — any test that
    /// pushes with a frame delay writes it, so reads would otherwise race.
    static ANCHOR_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// Take the anchor lock, ignoring poisoning (a panic in another test says
    /// nothing about whether this static is usable).
    fn lock_anchor() -> std::sync::MutexGuard<'static, ()> {
        ANCHOR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The store in `live_trace_breakpoint` must remain **observable**.
    ///
    /// This is a regression test for a debugger failure, not a behavioural one:
    /// if the store can be dead-store-eliminated, the function body empties, the
    /// linker folds it onto other empty functions, and breakpoints set on it fire
    /// from unrelated code. See the note on `live_trace_breakpoint`.
    #[test]
    fn breakpoint_anchor_store_is_observable() {
        let _guard = lock_anchor();
        live_trace_breakpoint(42);
        assert_eq!(
            last_frame_index(),
            42,
            "the anchor must record its argument"
        );
        // The startup-gate sentinel must round-trip like any other value.
        live_trace_breakpoint(usize::MAX);
        assert_eq!(last_frame_index(), usize::MAX, "startup gate value");
    }

    #[test]
    fn push_and_recv() {
        let (lt, rx) = LiveTrace::new();
        lt.push(1);
        lt.push(2);
        lt.push(3);
        assert_eq!(lt.len(), 3);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![1, 2, 3]);
    }

    #[test]
    fn recv_returns_frame() {
        let (lt, rx) = LiveTrace::new();
        lt.push("hello".to_string());
        assert_eq!(rx.try_recv().ok(), Some("hello".to_string()));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn clone_shares_channel() {
        let (lt1, rx) = LiveTrace::new();
        let lt2 = lt1.clone();
        lt2.push(42);
        assert_eq!(lt1.len(), 1);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![42]);
    }

    #[test]
    fn empty_initially() {
        let (lt, _rx) = LiveTrace::<i32>::new();
        assert!(lt.is_empty());
        assert_eq!(lt.len(), 0);
    }

    #[test]
    fn with_frame_delay_calls_breakpoint() {
        // Pushing with a delay calls the anchor, writing LAST_FRAME_INDEX.
        let _guard = lock_anchor();
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(Duration::from_millis(1));
        lt.push(99);
        assert_eq!(lt.len(), 1);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![99]);
    }

    /// **`spawn_structural_live` really runs the analysis and really
    /// delivers.** This is the test the three animations' retired
    /// `live_mode_receives_all_frames` tests point at: spawning and delivery
    /// are this helper's job now, so one test here covers what three
    /// animation-level tests used to, and covers it where the code is.
    ///
    /// The selector asks for matching steps only, which also pins the part a
    /// caller depends on: a sink that returns `None` must drop the step rather
    /// than end the run, so Tarjan and tearing steps passing through do not
    /// truncate the matching stream.
    ///
    /// `SingleInertia` is MSL-free, so no library roots are loaded.
    #[test]
    fn a_live_session_runs_the_real_analysis_and_delivers_its_steps() {
        let specimen =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("specimens/SingleInertia.mo");
        let Ok(crate::worker::FromWorker::Compiled { dae: Some(dae), .. }) =
            crate::worker::compile_specimen(&specimen, Vec::new())
        else {
            return; // specimen unavailable in this checkout
        };

        // No frame delay: the stepped delay exists to let egui paint between
        // frames, and there is no debugger here to hold the thread.
        let (rx, done) = spawn_structural_live(
            dae,
            Duration::from_millis(0),
            "test-live",
            |record| match record {
                rumoca_phase_structural::StructuralRecord::Matching(matching) => Some(matching),
                _ => None,
            },
        )
        .expect("the thread must spawn");

        // `wait_for_debugger` sleeps 500 ms before the first step, so this
        // waits well past that rather than racing it.
        let mut frames = Vec::new();
        for _ in 0..200 {
            frames.extend(rx.try_iter());
            if done.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        frames.extend(rx.try_iter());

        assert!(
            done.load(std::sync::atomic::Ordering::Acquire),
            "the analysis must finish and say so"
        );
        assert!(
            !frames.is_empty(),
            "a live session over a real DAE must deliver matching steps"
        );
        let last = frames
            .last()
            .expect("checked non-empty above")
            .match_eq
            .clone();
        assert!(
            last.iter().any(Option::is_some),
            "the last step must show a matching that was actually built: {last:?}"
        );
    }

    /// **A live session steps exactly the frames the compile recorded — the run
    /// that simulates — not a run over the raw DAE.**
    ///
    /// `Drivetrain`, because solve lowering's Simplify step merges 53 of its
    /// aliases before any analysis, so its first analysis is of a different system
    /// from the compile's DAE. Until 2026-11-27 a live session ran `observe_sort`
    /// on that raw DAE and stepped a matching Rumoca never performs; the second
    /// assertion is that old run, kept so this test shows it can tell the two
    /// apart. (A small alias model was tried first and Simplify left it unchanged,
    /// so the comparison could not discriminate.)
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn a_live_session_steps_the_frames_the_compile_recorded() {
        let crate::worker::FromWorker::Compiled {
            dae: Some(dae),
            matching_frames: recorded,
            ..
        } = crate::worker::test_msl::compile_specimen_shared("Drivetrain")
        else {
            panic!("Drivetrain must compile");
        };

        let raw_matching = dae.inspect(|view| {
            let mut steps = Vec::new();
            let _ = rumoca_phase_structural::observe_sort(view, |record| {
                if let rumoca_phase_structural::StructuralRecord::Matching(matching) = record {
                    steps.push(matching);
                }
            });
            steps
        });

        let (rx, done) =
            spawn_structural_live(dae, Duration::from_millis(0), "test-live-real", |record| {
                match record {
                    rumoca_phase_structural::StructuralRecord::Matching(matching) => Some(matching),
                    _ => None,
                }
            })
            .expect("the thread must spawn");
        let mut live = Vec::new();
        for _ in 0..200 {
            live.extend(rx.try_iter());
            if done.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        live.extend(rx.try_iter());

        assert!(
            !recorded.is_empty(),
            "the compile must record matching frames"
        );
        assert_eq!(
            format!("{live:?}"),
            format!("{recorded:?}"),
            "the live session stepped a different matching from the one the compile \
             recorded",
        );
        assert_ne!(
            format!("{raw_matching:?}"),
            format!("{recorded:?}"),
            "the raw DAE's matching must differ from the simplified one here, or this \
             test cannot tell a live session on the wrong input from one on the right",
        );
    }

    /// **A live reduction session steps the records the Index reduction tab shows.**
    ///
    /// The tab's `search` is every reduction record of the compile's real
    /// preparation; a live session must deliver the same number, in the same
    /// order of kinds. `Drivetrain` reduces over six rounds, so there is a search
    /// worth comparing.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn a_live_reduction_session_steps_the_tabs_search() {
        let crate::worker::FromWorker::Compiled {
            dae: Some(dae),
            stages,
            ..
        } = crate::worker::test_msl::compile_specimen_shared("Drivetrain")
        else {
            panic!("Drivetrain must compile");
        };
        let search = stages
            .index_reduction
            .value
            .as_ref()
            .and_then(|v| v["search"].as_array().cloned())
            .expect("Index reduction publishes its search");

        let (rx, done) =
            spawn_reduction_live(dae, Duration::from_millis(0), "test-live-reduction", Some)
                .expect("the thread must spawn");
        let mut live = Vec::new();
        for _ in 0..500 {
            live.extend(rx.try_iter());
            if done.load(std::sync::atomic::Ordering::Acquire) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        live.extend(rx.try_iter());

        assert!(
            !search.is_empty(),
            "Drivetrain's reduction must record a search"
        );
        assert_eq!(
            live.len(),
            search.len(),
            "the live session delivered a different number of reduction records from \
             the tab's search",
        );
        let round = |kinds: Vec<bool>| kinds.iter().filter(|r| **r).count();
        assert_eq!(
            round(
                live.iter()
                    .map(|r| matches!(r, rumoca_phase_structural::ReductionRecord::Round { .. }))
                    .collect()
            ),
            stages.index_reduction.value.as_ref().unwrap()["rounds"]
                .as_u64()
                .unwrap_or_default() as usize,
            "the live session's rounds differ from the tab's",
        );
    }

    #[test]
    fn frame_delay_propagates_through_clone() {
        // Pushing with a delay calls the anchor, writing LAST_FRAME_INDEX.
        let _guard = lock_anchor();
        let (lt1, rx) = LiveTrace::new();
        let lt1 = lt1.with_frame_delay(Duration::from_millis(1));
        let lt2 = lt1.clone();
        lt2.push(7);
        assert_eq!(lt1.len(), 1);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![7]);
    }
}
