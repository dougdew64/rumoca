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
        /// The equations this set produced, as their rendered
        /// [`EquationOrigin`](rumoca_ir_flat::EquationOrigin)s — e.g.
        /// `"flow sum equation: C.n.i + src.n.i + gnd.p.i = 0"`.
        ///
        /// **Without this the trace says how many equations a set produced and never
        /// which**, so an observer can report *"3 variables became 1 equation"* but
        /// cannot show the equation. That is the difference between a step counter and
        /// a view of the rule: Kirchhoff's law is the *content* of the flow row, not
        /// its count.
        ///
        /// Rendered origins rather than residual expressions, deliberately — the origin
        /// is the form a reader can check against the flat model's own equation listing,
        /// so an observer showing this and a tool showing the equation sheet cannot
        /// disagree about what was generated.
        ///
        /// Read from `Model::equations` after the generating call, so it is what the
        /// model actually gained. Like `equations_added` it is **measured, not
        /// predicted**, and stays honest when array scalarization turns one logical
        /// equation into several.
        equations: Vec<String>,
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

thread_local! {
    /// Ambient live sink: `Some` while a live scope is open.
    #[allow(clippy::type_complexity)]
    static LIVE: std::cell::RefCell<Option<Box<dyn Fn(&ConnectionFrame)>>> =
        const { std::cell::RefCell::new(None) };
}

/// **Deliver each connection frame the moment it is emitted**, rather than buffering.
///
/// # Why this exists alongside [`start_capture`]
///
/// Same problem, opposite deadline. [`start_capture`] answers *"what did that pass
/// do?"* after it finishes. This answers *"what is that pass doing right now?"* —
/// which is what a debugger-driven walk needs, because the reader is stopped
/// **inside** the pass and the buffer will not exist for another few thousand
/// statements.
///
/// A caller that can pass a [`FrameObserver`] should. This is for the same callers
/// [`start_capture`] exists for: those driving the *session* API from a dozen stack
/// frames above, where threading an observer down is the invasive change described
/// there.
///
/// # What it buys, and why it is not a re-run
///
/// The alternative — and what every other live-stepped view in Rumoca's observatory
/// does — is to **re-run the phase** with an observer attached, so the debugger has
/// something to stop inside. That is a second execution, and the tool must then
/// prove it was configured like the first.
///
/// With this, the pass being stepped **is the compilation**. The reader stops in the
/// real run, in the real call stack, with the real values. Nothing has to be proven
/// equivalent, because nothing was repeated.
///
/// # Cost, and the one hazard
///
/// One thread-local read per emit when closed, as with the buffer. While open, the
/// sink runs **on the emitting thread, inside the pass** — so a sink that blocks
/// blocks the compiler. That is precisely what a debugger sink wants (it is how the
/// reader is held at a breakpoint), and precisely what a logging sink must avoid.
///
/// [`end_live`] closes the scope. Leaving it open leaks the closure for the life of
/// the thread and keeps paying the call on every later compile.
pub fn start_live(sink: Box<dyn Fn(&ConnectionFrame)>) {
    LIVE.with(|c| *c.borrow_mut() = Some(sink));
}

/// Close the live scope opened by [`start_live`].
pub fn end_live() {
    LIVE.with(|c| *c.borrow_mut() = None);
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
    let frame = ConnectionFrame {
        step,
        sets_so_far,
        equations_so_far,
    };
    if let Some(obs) = observer {
        obs(&frame);
    }
    // **Before the buffer, deliberately.** A live sink is usually a debugger stop, so
    // the frame must reach the reader *while the pass is still standing here*. Pushing
    // to the buffer first would be harmless today and wrong the moment a sink panics
    // or a scope is closed mid-pass: the reader would be shown a frame the buffer had
    // already recorded and the sink never saw.
    //
    // Borrowed for the call rather than cloned out, so a sink cannot re-enter `emit`
    // and deadlock on the `RefCell` — the failure would present as a hang inside the
    // compiler with no message.
    LIVE.with(|c| {
        if let Some(sink) = c.borrow().as_ref() {
            sink(&frame);
        }
    });
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
        emit(
            None,
            ConnectionStep::Start {
                connect_statements: 1,
            },
            0,
            0,
        );
        assert!(
            take_capture().is_empty(),
            "no scope was open, so there is nothing to take"
        );

        start_capture();
        emit(
            None,
            ConnectionStep::Start {
                connect_statements: 4,
            },
            0,
            0,
        );
        emit(
            None,
            ConnectionStep::Start {
                connect_statements: 5,
            },
            1,
            2,
        );
        let frames = take_capture();
        assert_eq!(frames.len(), 2, "both frames were recorded: {frames:?}");
        assert_eq!(frames[1].sets_so_far, 1, "and carry their running counts");

        // Taking closed the scope: later frames must not leak into the next take.
        emit(
            None,
            ConnectionStep::Start {
                connect_statements: 9,
            },
            0,
            0,
        );
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
            emit(
                Some(&sink),
                ConnectionStep::Start {
                    connect_statements: 7,
                },
                3,
                4,
            );
        }
        let captured = take_capture();
        assert_eq!(
            seen.into_inner(),
            captured,
            "both destinations get the same frame"
        );
    }

    /// With no observer the emit call is a no-op; with one, the frame arrives
    /// carrying the running counts. Those counts are the reason the frame type
    /// exists — a step alone cannot say how far the pass has got.
    #[test]
    fn emit_is_a_no_op_without_an_observer_and_delivers_with_one() {
        emit(
            None,
            ConnectionStep::Start {
                connect_statements: 4,
            },
            0,
            0,
        );

        let seen = RefCell::new(Vec::new());
        let sink = |f: &ConnectionFrame| seen.borrow_mut().push(f.clone());
        emit(
            Some(&sink),
            ConnectionStep::EquationsGenerated {
                kind: "potential",
                set_size: 3,
                equations_added: 2,
                equations: vec![
                    "connection equation: a = b".to_string(),
                    "connection equation: b = c".to_string(),
                ],
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
            ConnectionStep::EquationsGenerated {
                set_size: 3,
                equations_added: 2,
                ..
            },
        ));
    }
}

#[cfg(test)]
mod live_scope_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn start_frame(n: usize) -> ConnectionStep {
        ConnectionStep::Start {
            connect_statements: n,
        }
    }

    /// **A live sink sees each frame as it is emitted, and stops when closed.**
    ///
    /// The three properties a debugger-driven walk depends on, and the two that
    /// would make it untrustworthy: frames arriving from nowhere (a sink left
    /// installed) and one model's frames reaching the sink of the next.
    #[test]
    fn a_live_scope_delivers_frames_and_stops_when_closed() {
        let seen: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));

        // Closed: nothing is delivered.
        emit(None, start_frame(1), 0, 0);
        assert!(seen.borrow().is_empty());

        let sink_seen = Rc::clone(&seen);
        start_live(Box::new(move |f: &ConnectionFrame| {
            if let ConnectionStep::Start { connect_statements } = f.step {
                sink_seen.borrow_mut().push(connect_statements);
            }
        }));

        emit(None, start_frame(2), 0, 0);
        emit(None, start_frame(3), 0, 0);
        assert_eq!(
            *seen.borrow(),
            vec![2, 3],
            "each frame must reach the sink as it is emitted",
        );

        end_live();
        emit(None, start_frame(4), 0, 0);
        assert_eq!(
            *seen.borrow(),
            vec![2, 3],
            "a closed scope must deliver nothing \u{2014} otherwise one model's frames \
             appear under the next",
        );
    }

    /// **The live sink and the capture buffer see the same frames.**
    ///
    /// They are independent destinations for one emit, and a reader stepping live
    /// while a recording is taken must not be shown a different pass from the one
    /// that gets recorded.
    #[test]
    fn a_live_sink_and_a_capture_scope_agree() {
        let seen: Rc<RefCell<Vec<usize>>> = Rc::new(RefCell::new(Vec::new()));
        let sink_seen = Rc::clone(&seen);
        start_live(Box::new(move |f: &ConnectionFrame| {
            if let ConnectionStep::Start { connect_statements } = f.step {
                sink_seen.borrow_mut().push(connect_statements);
            }
        }));
        start_capture();

        emit(None, start_frame(7), 0, 0);
        emit(None, start_frame(8), 0, 0);

        let captured: Vec<usize> = take_capture()
            .into_iter()
            .filter_map(|f| match f.step {
                ConnectionStep::Start { connect_statements } => Some(connect_statements),
                _ => None,
            })
            .collect();
        end_live();

        assert_eq!(
            *seen.borrow(),
            captured,
            "the stepped pass and the recorded pass must be the same pass",
        );
        assert!(
            !captured.is_empty(),
            "nothing was emitted, so nothing was compared"
        );
    }
}
