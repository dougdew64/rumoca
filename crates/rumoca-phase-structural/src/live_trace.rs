//! Shared trace buffer for live-stepped algorithm observation.
//!
//! A `LiveTrace<F>` wraps an `Arc<Mutex<Vec<F>>>` that the traced algorithm
//! pushes frames into while a separate thread (typically a UI) reads them.
//! The key debugging technique: set a breakpoint on [`live_trace_breakpoint`] —
//! each time the debugger pauses there, the UI thread can read the latest
//! frame and render the algorithm's current state.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A thread-safe trace buffer shared between an algorithm producer and a
/// UI consumer. Clone the `LiveTrace` to share it across threads (cloning
/// shares the same underlying buffer via `Arc`).
///
/// Generic over the frame type `F` — works with `MatchingFrame`,
/// `TarjanFrame`, or any future traced-algorithm frame type.
pub struct LiveTrace<F> {
    frames: Arc<Mutex<Vec<F>>>,
    /// When set, `push` sleeps for this duration after each frame, giving a
    /// UI thread time to poll and render before the debugger pauses all
    /// threads at the [`live_trace_breakpoint`] call.
    frame_delay: Option<Duration>,
}

impl<F> Clone for LiveTrace<F> {
    fn clone(&self) -> Self {
        LiveTrace {
            frames: Arc::clone(&self.frames),
            frame_delay: self.frame_delay,
        }
    }
}

impl<F: Clone> LiveTrace<F> {
    /// Create a new empty live trace buffer (no inter-frame delay).
    pub fn new() -> Self {
        LiveTrace { frames: Arc::new(Mutex::new(Vec::new())), frame_delay: None }
    }

    /// Builder: add an inter-frame delay so a UI thread can render each
    /// frame before the debugger pauses all threads at the breakpoint.
    /// Use [`Duration::from_millis(20)`] for typical egui frame rates.
    pub fn with_frame_delay(mut self, delay: Duration) -> Self {
        self.frame_delay = Some(delay);
        self
    }

    /// Push a frame into the shared buffer.
    ///
    /// When a `frame_delay` is set (see [`with_frame_delay`]), this method
    /// sleeps after the push and then calls [`live_trace_breakpoint`] —
    /// **set your debugger breakpoint on that function**, not here.
    pub fn push(&self, frame: F) {
        let index = {
            let mut buf = self.frames.lock().expect("live trace lock poisoned");
            buf.push(frame);
            buf.len() - 1
        };
        if let Some(delay) = self.frame_delay {
            std::thread::sleep(delay);
            // ---- DEBUGGER: set breakpoint on the next line ----
            live_trace_breakpoint(index);
        }
    }

    /// Number of frames recorded so far.
    pub fn len(&self) -> usize {
        self.frames.lock().expect("live trace lock poisoned").len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Clone all frames out of the buffer. Used by the UI to get the
    /// full trace when the algorithm completes.
    pub fn snapshot(&self) -> Vec<F> {
        self.frames.lock().expect("live trace lock poisoned").clone()
    }

    /// Clone a single frame by index, if it exists.
    pub fn get(&self, index: usize) -> Option<F> {
        self.frames.lock().expect("live trace lock poisoned").get(index).cloned()
    }
}

impl<F: Clone> Default for LiveTrace<F> {
    fn default() -> Self {
        Self::new()
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
static LAST_FRAME_INDEX: AtomicUsize = AtomicUsize::new(0);

#[inline(never)]
pub fn live_trace_breakpoint(frame_index: usize) {
    LAST_FRAME_INDEX.store(frame_index, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_snapshot() {
        let lt = LiveTrace::new();
        lt.push(1);
        lt.push(2);
        lt.push(3);
        assert_eq!(lt.len(), 3);
        assert_eq!(lt.snapshot(), vec![1, 2, 3]);
    }

    #[test]
    fn get_returns_cloned_frame() {
        let lt = LiveTrace::new();
        lt.push("hello".to_string());
        assert_eq!(lt.get(0), Some("hello".to_string()));
        assert_eq!(lt.get(1), None);
    }

    #[test]
    fn clone_shares_buffer() {
        let lt1 = LiveTrace::new();
        let lt2 = lt1.clone();
        lt1.push(42);
        assert_eq!(lt2.len(), 1);
        assert_eq!(lt2.get(0), Some(42));
    }

    #[test]
    fn empty_initially() {
        let lt: LiveTrace<i32> = LiveTrace::new();
        assert!(lt.is_empty());
        assert_eq!(lt.len(), 0);
        assert!(lt.snapshot().is_empty());
    }

    #[test]
    fn with_frame_delay_calls_breakpoint() {
        let lt = LiveTrace::new().with_frame_delay(Duration::from_millis(1));
        lt.push(99);
        assert_eq!(lt.len(), 1);
        assert_eq!(lt.get(0), Some(99));
    }

    #[test]
    fn frame_delay_propagates_through_clone() {
        let lt1 = LiveTrace::new().with_frame_delay(Duration::from_millis(1));
        let lt2 = lt1.clone();
        lt2.push(7);
        assert_eq!(lt1.len(), 1);
    }
}
