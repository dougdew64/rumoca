//! Channel-based trace buffer for live-stepped algorithm observation.
//!
//! A `LiveTrace<F>` sends frames through an `mpsc` channel that the UI thread
//! drains without contention. The producer (`Sender`) and consumer (`Receiver`)
//! share no application-level lock — `push()` never blocks on the UI thread.
//!
//! The key debugging technique: set a breakpoint on [`live_trace_breakpoint`] —
//! each time the debugger pauses there, the UI thread can read the latest
//! frame and render the algorithm's current state.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Producer half of the live trace channel.
///
/// The algorithm thread owns this and calls [`push`](Self::push) to send frames.
/// No lock is shared with the UI thread — `Sender::send()` takes `&self` and
/// uses internal synchronization, so no `Mutex` wrapper is needed.
///
/// Generic over the frame type `F` — works with `MatchingFrame`,
/// `TarjanFrame`, `IndexReductionFrame`, or any future traced-algorithm type.
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
/// `frame_index` is the 0-based index of the frame just pushed — inspect
/// it in the debugger to know which algorithmic step you're on.
/// `usize::MAX` indicates the startup gate (before any algorithm work).
static LAST_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

#[inline(never)]
pub fn live_trace_breakpoint(frame_index: usize) {
    LAST_FRAME_INDEX.store(frame_index, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(Duration::from_millis(1));
        lt.push(99);
        assert_eq!(lt.len(), 1);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![99]);
    }

    #[test]
    fn frame_delay_propagates_through_clone() {
        let (lt1, rx) = LiveTrace::new();
        let lt1 = lt1.with_frame_delay(Duration::from_millis(1));
        let lt2 = lt1.clone();
        lt2.push(7);
        assert_eq!(lt1.len(), 1);
        let frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(frames, vec![7]);
    }
}
