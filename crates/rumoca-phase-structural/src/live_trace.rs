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
//! ## Not used by any phase
//!
//! As of 2026-07-29 the traced phases take [`rumoca_core::FrameObserver`] — a
//! plain callback — rather than a `LiveTrace`. This type is one *implementation*
//! of an observer, used by the consumer (HRW) which wires it up as
//! `Some(&|frame| lt.push(frame.clone()))`.
//!
//! The change was forced by instrumenting `rumoca-phase-dae`: taking a
//! `LiveTrace` would have meant DAE construction depending on
//! `rumoca-phase-structural`, a dependency pointing the wrong way down the
//! pipeline. A callback needs no dependency, and lets a consumer buffer, stream,
//! count or step frames without any phase being changed to allow it.
//!
//! It stays here rather than moving out because [`live_trace_breakpoint`] is the
//! debugger anchor, and both the `opt-level = 0` override and HRW's
//! `bridge::find_live_trace_line` are keyed to this file. Moving it is a
//! separate change with its own risks.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Producer half of the live trace channel.
///
/// The algorithm thread owns this and calls [`push`](Self::push) to send frames.
/// No lock is shared with the UI thread — `Sender::send()` takes `&self` and
/// uses internal synchronization, so no `Mutex` wrapper is needed.
///
/// Generic over the frame type `F` — works with `MatchingFrame`, `TarjanFrame`,
/// `IndexReductionFrame`, `PreLoweringFrame`, or any future traced-algorithm
/// type. Phases no longer name it: they take a [`rumoca_core::FrameObserver`],
/// and this is what the consumer puts behind that callback.
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
/// on it land somewhere else entirely. The failure chain (diagnosed on Windows
/// 2026-07-27, see `hrw/docs/windows-migration.md`):
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
        assert_eq!(last_frame_index(), 42, "the anchor must record its argument");
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
