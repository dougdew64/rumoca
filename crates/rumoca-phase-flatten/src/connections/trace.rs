//! Observation hooks for connection expansion (MLS §9).
//!
//! Additive and observation-only: nothing here changes what the phase
//! computes. When no observer is attached the cost is one `Option` check per
//! connection set.
//!
//! # What the frames are for
//!
//! The flat model's equation count is far larger than the number of equations
//! anyone wrote, and connection expansion is where most of the difference comes
//! from. The *result* — a `flat::Model` full of `EquationOrigin::Connection`
//! and `EquationOrigin::FlowSum` rows — shows the equations but not the rule
//! that produced them, and the rule is the interesting part:
//!
//! - a **potential** set of *n* variables yields *n − 1* equality equations
//!   (`v1 = v2 = … = vn` written as a chain), while
//! - a **flow** set of the same *n* variables yields exactly **one** equation,
//!   the sum-to-zero (MLS §9.2 — Kirchhoff's current law).
//!
//! That asymmetry is the single most useful thing to see here, and it is
//! visible only if you can watch each set produce its equations. The frames
//! also expose that connection *sets* are transitive: `connect(a, b)` and
//! `connect(b, c)` form one set of three, not two sets of two, because the sets
//! are built by union-find.
//!
//! # Granularity
//!
//! Frames are emitted per connection **set**, not per union-find merge. The
//! merges happen several call levels below `process_connections`
//! (`process_connection` → `connect_primitive_vars` → …), and threading an
//! observer down there would touch six functions to show a data structure that
//! the set membership already reports. Per-merge frames remain a possible
//! refinement if the set view turns out to leave a question unanswered.

use rumoca_core::FrameObserver;

/// One step of connection expansion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStep {
    /// The pass is starting, with the number of `connect()` statements it
    /// collected (after skipping ones involving disabled components).
    Start { connect_statements: usize },
    /// A connection set has been built: these variables are connected to one
    /// another, transitively, at this scope.
    SetFormed {
        /// `"flow"`, `"potential"` or `"stream"`.
        kind: &'static str,
        /// The hierarchy level the `connect()` was declared at; empty = root.
        scope: String,
        variables: Vec<String>,
    },
    /// Equations were generated for the set just formed.
    ///
    /// `equations_added` is measured, not predicted — the difference in the
    /// model's equation count across the generating call — so it stays honest
    /// if array scalarization makes one logical equation into several.
    EquationsGenerated {
        kind: &'static str,
        set_size: usize,
        equations_added: usize,
    },
    /// A flow variable in no connection set: MLS §9.2 gives it `f = 0`.
    UnconnectedFlow { equations_added: usize },
    /// The pass is finished.
    Complete {
        sets: usize,
        /// Every equation this pass added to the model.
        equations_added: usize,
    },
}

/// One frame of the connection-expansion trace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionFrame {
    pub step: ConnectionStep,
    /// Connection sets completed so far — the running count.
    pub sets_so_far: usize,
    /// Equations this pass has added so far.
    pub equations_so_far: usize,
}

thread_local! {
    /// Ambient capture buffer: `Some` while a capture scope is open.
    static CAPTURE: std::cell::RefCell<Option<Vec<ConnectionFrame>>> =
        const { std::cell::RefCell::new(None) };
}

/// **Begin capturing connection frames on this thread**, for callers that cannot
/// thread an observer down to this point.
///
/// # Why this exists alongside `FrameObserver`
///
/// [`FrameObserver`] is the right tool when the caller is calling flatten
/// directly: it is explicit, borrows freely, and costs nothing when absent. But a
/// tool driving the *session* API — `compile_model_strict_reachable_*` — is a dozen
/// stack frames above this one, through functions whose signatures have nothing to
/// do with observability. Threading an `Option<FrameObserver>` through all of them
/// to reach one emit site is a large, invasive change to make a small, optional
/// thing possible.
///
/// The alternative such tools resort to is **re-running the phase** with an
/// observer attached, which is worse than invasive — it is *inaccurate*. The
/// frames then describe a second execution, and the tool must separately prove that
/// second execution was configured like the first. HRW (the Rumoca observatory)
/// did exactly this, and the options were kept in step by hand across two crates.
///
/// So: an opt-in, thread-local buffer, in the shape `tracing` already established
/// for the same problem. Nothing changes for existing callers, no signature moves,
/// and a wrapping tool can observe **the compilation that actually happened**.
///
/// # Cost
///
/// Nothing at all when no scope is open — one thread-local read per emit. While
/// open, frames accumulate in memory, so a scope should wrap **one model**, not a
/// whole library. [`take_capture`] both drains and closes.
pub fn start_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Take the captured frames and close the scope.
///
/// Returns empty if no scope was open, which is the honest answer: no frames were
/// requested, so none were recorded.
pub fn take_capture() -> Vec<ConnectionFrame> {
    CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default()
}

/// Emit one frame if an observer is attached.
///
/// Free function rather than a method so the call sites read as a single line
/// and the `Option` check stays visible at each one.
pub(crate) fn emit(
    observer: Option<FrameObserver<'_, ConnectionFrame>>,
    step: ConnectionStep,
    sets_so_far: usize,
    equations_so_far: usize,
) {
    // Built once and shared by both destinations, so an explicit observer and an
    // ambient capture in the same run cannot disagree about what a frame said.
    let frame = ConnectionFrame { step, sets_so_far, equations_so_far };
    if let Some(obs) = observer {
        obs(&frame);
    }
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            buf.push(frame);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// **A capture scope records frames without an observer, and closes on take.**
    ///
    /// The property a wrapping tool depends on: frames from the run that actually
    /// happened, with no signature threaded down and no second execution. Also
    /// checks the two failure modes that would make it untrustworthy — recording
    /// while no scope is open (frames from nowhere) and continuing to record after
    /// a take (one model's frames appearing under the next).
    #[test]
    fn a_capture_scope_records_this_run_and_stops_when_taken() {
        // Closed: nothing is recorded, and taking yields nothing.
        emit(None, ConnectionStep::Start { connect_statements: 1 }, 0, 0);
        assert!(take_capture().is_empty(), "no scope was open, so there is nothing to take");

        start_capture();
        emit(None, ConnectionStep::Start { connect_statements: 4 }, 0, 0);
        emit(None, ConnectionStep::Start { connect_statements: 5 }, 1, 2);
        let frames = take_capture();
        assert_eq!(frames.len(), 2, "both frames were recorded: {frames:?}");
        assert_eq!(frames[1].sets_so_far, 1, "and carry their running counts");

        // Taking closed the scope: later frames must not leak into the next take.
        emit(None, ConnectionStep::Start { connect_statements: 9 }, 0, 0);
        assert!(
            take_capture().is_empty(),
            "capture continued after take \u{2014} one model's frames would appear \
             under the next model's view",
        );
    }

    /// An explicit observer and an ambient capture see the same frame.
    #[test]
    fn an_observer_and_a_capture_agree() {
        let seen = RefCell::new(Vec::new());
        start_capture();
        {
            let sink = |f: &ConnectionFrame| seen.borrow_mut().push(f.clone());
            emit(Some(&sink), ConnectionStep::Start { connect_statements: 7 }, 3, 4);
        }
        let captured = take_capture();
        assert_eq!(seen.into_inner(), captured, "both destinations get the same frame");
    }

    /// With no observer the emit call is a no-op; with one, the frame arrives
    /// carrying the running counts. Those counts are the reason the frame type
    /// exists — a step alone cannot say how far the pass has got.
    #[test]
    fn emit_is_a_no_op_without_an_observer_and_delivers_with_one() {
        emit(None, ConnectionStep::Start { connect_statements: 4 }, 0, 0);

        let seen = RefCell::new(Vec::new());
        let sink = |f: &ConnectionFrame| seen.borrow_mut().push(f.clone());
        emit(
            Some(&sink),
            ConnectionStep::EquationsGenerated {
                kind: "potential",
                set_size: 3,
                equations_added: 2,
            },
            1,
            2,
        );

        let seen = seen.into_inner();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].sets_so_far, 1);
        assert_eq!(seen[0].equations_so_far, 2);
        assert!(matches!(
            seen[0].step,
            ConnectionStep::EquationsGenerated { set_size: 3, equations_added: 2, .. },
        ));
    }
}
