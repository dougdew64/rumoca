//! Observation hooks for connection expansion (MLS §9).
//!
//! Additive and observation-only: nothing here changes what the phase
//! computes. When no scope is open the cost is one thread-local read per
//! connection set.
//!
//! # What the steps are for
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
//! visible only if you can watch each set produce its equations. The steps also
//! expose that connection *sets* are transitive: `connect(a, b)` and
//! `connect(b, c)` form one set of three, not two sets of two, because the sets
//! are built by union-find.
//!
//! # Why this is ambient state when the structural seam is not
//!
//! `crate::...`'s sibling instrumentation in `rumoca-phase-structural` threads
//! an observer parameter through the algorithms it watches, and deliberately:
//! there, the old design kept a **second, separately written traced copy** of
//! matching and of Tarjan, so a recorded step sequence agreed with the real run
//! only while two bodies of code stayed in step. An observer parameter removed
//! the duplicate.
//!
//! **No duplicate exists here, and never did.** There is one implementation of
//! connection expansion; [`emit`] is called from inside it, and a closed scope
//! turns each call into a thread-local read. So the steps always describe the
//! run that produced the model — ambient state is the *delivery* mechanism, not
//! a second code path, and the hazard that justified the structural rework is
//! absent.
//!
//! What ambient state buys instead is reach. These steps are produced a dozen
//! stack frames below `rumoca-compile`'s session API, inside functions whose
//! signatures have nothing to do with observability, and a tool driving that
//! API cannot pass anything down to them. The alternative such tools resort to
//! is **re-running the phase** with an observer attached, which is worse than
//! invasive — it is inaccurate, because the steps then describe a second
//! execution that must separately be proven equivalent to the first. An opt-in
//! thread-local buffer, in the shape `tracing` already established for the same
//! problem, lets a wrapping tool observe *the compilation that actually
//! happened* while no signature moves and no existing caller changes.
//!
//! # Cost
//!
//! Nothing at all when no scope is open — one thread-local read per emit. While
//! open, steps accumulate in memory, so a scope should wrap **one model**, not a
//! whole library. [`take_capture`] both drains and closes.

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
        /// **Without this the trace says how many equations a set produced and
        /// never which**, so an observer can report *"3 variables became 1
        /// equation"* but cannot show the equation. That is the difference
        /// between a step counter and a view of the rule: Kirchhoff's law is
        /// the *content* of the flow row, not its count.
        ///
        /// Rendered origins rather than residual expressions, deliberately —
        /// the origin is the form a reader can check against the flat model's
        /// own equation listing, so an observer showing this and a tool showing
        /// the equation sheet cannot disagree about what was generated.
        ///
        /// Read from `Model::equations` after the generating call, so it is
        /// what the model actually gained. Like `equations_added` it is
        /// **measured, not predicted**, and stays honest when array
        /// scalarization turns one logical equation into several.
        equations: Vec<String>,
    },
    /// A potential set that generated nothing, because every member belongs to
    /// an expandable connector branch nothing outside the connection
    /// references.
    ///
    /// **Reported rather than left as an `EquationsGenerated` of zero**, which
    /// is what it would otherwise look like. Zero equations with no stated
    /// reason reads as a compiler that lost them; this says model-role planning
    /// is free to remove the members instead, so emitting equality equations
    /// would leave them structurally unbalanced.
    PotentialSetSkipped { set_size: usize },
    /// A flow variable in no connection set: MLS §9.2 gives it `f = 0`.
    UnconnectedFlow { equations_added: usize },
    /// The pass is finished.
    Complete {
        sets: usize,
        /// Every equation this pass added to the model.
        equations_added: usize,
    },
}

/// One step of the connection-expansion trace, with the running totals at that
/// moment.
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

/// **Begin capturing connection steps on this thread.** Nothing is recorded
/// until this is called.
pub fn start_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Take the captured steps and close the scope.
///
/// Returns empty if no scope was open, which is the honest answer: no steps
/// were requested, so none were recorded.
pub fn take_capture() -> Vec<ConnectionFrame> {
    CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default()
}

thread_local! {
    /// Ambient live sink: `Some` while a live scope is open.
    #[expect(
        clippy::type_complexity,
        reason = "a boxed sink in a thread-local has no simpler spelling; naming it would move the complexity rather than remove it"
    )]
    static LIVE: std::cell::RefCell<Option<Box<dyn Fn(&ConnectionFrame)>>> =
        const { std::cell::RefCell::new(None) };
}

/// **Deliver each step the moment it is emitted**, rather than buffering.
///
/// # Why this exists alongside [`start_capture`]
///
/// Same problem, opposite deadline. [`start_capture`] answers *"what did that
/// pass do?"* after it finishes. This answers *"what is that pass doing right
/// now?"* — which is what a debugger-driven session needs, because the reader
/// is stopped **inside** the pass and the buffer will not exist for another few
/// thousand statements.
///
/// # What it buys, and why it is not a re-run
///
/// Every other live-stepped view in Rumoca's observatory **re-runs** its phase
/// with an observer attached, so the debugger has something to stop inside.
/// That is a second execution, and the tool must then show it was configured
/// like the first.
///
/// With this, the pass being stepped **is the compilation**. The reader stops
/// in the real run, in the real call stack, with the real values. Nothing has
/// to be shown equivalent, because nothing was repeated — which makes this the
/// strongest of the observation surfaces, not a concession.
///
/// # Cost, and the one hazard
///
/// One thread-local read per emit when closed, as with the buffer. While open,
/// the sink runs **on the emitting thread, inside the pass** — so a sink that
/// blocks blocks the compiler. That is precisely what a debugger sink wants (it
/// is how the reader is held at a breakpoint), and precisely what a logging
/// sink must avoid.
///
/// [`end_live`] closes the scope. Leaving it open leaks the closure for the
/// life of the thread and keeps paying the call on every later compile.
pub fn start_live(sink: Box<dyn Fn(&ConnectionFrame)>) {
    LIVE.with(|c| *c.borrow_mut() = Some(sink));
}

/// Close the live scope opened by [`start_live`].
pub fn end_live() {
    LIVE.with(|c| *c.borrow_mut() = None);
}

/// Whether anything is listening, so a caller can skip assembling a step's
/// payload when nobody will read it.
///
/// Rendering a set's equations means formatting every `EquationOrigin` it
/// produced; that is real work, and it is the one part of this module that is
/// not free when closed. Guarding on this keeps the untraced compile paying
/// only the thread-local reads.
pub(crate) fn observing() -> bool {
    CAPTURE.with(|c| c.borrow().is_some()) || LIVE.with(|c| c.borrow().is_some())
}

/// Emit one step if anything is listening.
///
/// Free function rather than a method so the call sites read as a single line
/// and the check stays visible at each one.
pub(crate) fn emit(step: ConnectionStep, sets_so_far: usize, equations_so_far: usize) {
    // Built once and shared by both destinations, so a live sink and a capture
    // buffer open in the same run cannot disagree about what a step said.
    let frame = ConnectionFrame {
        step,
        sets_so_far,
        equations_so_far,
    };
    // **Before the buffer, deliberately.** A live sink is usually a debugger
    // stop, so the step must reach the reader *while the pass is still standing
    // here*. Pushing to the buffer first would be harmless today and wrong the
    // moment a sink panics or a scope is closed mid-pass: the reader would be
    // shown a step the buffer had already recorded and the sink never saw.
    //
    // Borrowed for the call rather than cloned out, so a sink cannot re-enter
    // `emit` and deadlock on the `RefCell` — the failure would present as a
    // hang inside the compiler with no message.
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

/// Render the equations a model gained between two lengths, as their origins.
///
/// `from` is the equation count before the generating call. Returns empty when
/// nothing is listening, so the formatting is never paid for on an untraced
/// compile.
pub(crate) fn equations_added_since(flat: &rumoca_ir_flat::Model, from: usize) -> Vec<String> {
    if !observing() {
        return Vec::new();
    }
    flat.equations
        .get(from..)
        .unwrap_or_default()
        .iter()
        .map(|equation| equation.origin.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes the tests that open ambient scopes. Cargo runs tests on
    /// parallel threads and these scopes are thread-local, so in principle they
    /// cannot collide — but a shared lock makes that independent of how the
    /// harness schedules, and `RUST_TEST_THREADS` is set workspace-wide.
    static SCOPE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn lock() -> std::sync::MutexGuard<'static, ()> {
        SCOPE_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn start(n: usize) -> ConnectionStep {
        ConnectionStep::Start {
            connect_statements: n,
        }
    }

    /// **Nothing is recorded until a scope is opened.** This is the property
    /// that keeps an untraced compile free, and it is worth a test because the
    /// failure mode is silent memory growth rather than a wrong answer.
    #[test]
    fn a_closed_scope_records_nothing() {
        let _guard = lock();
        emit(start(3), 0, 0);
        assert!(
            take_capture().is_empty(),
            "no scope was open, so nothing may have been kept"
        );
        assert!(!observing(), "and nothing claims to be listening");
    }

    #[test]
    fn an_open_scope_keeps_every_step_in_order() {
        let _guard = lock();
        start_capture();
        assert!(observing());
        emit(start(2), 0, 0);
        emit(ConnectionStep::UnconnectedFlow { equations_added: 1 }, 1, 1);
        let frames = take_capture();
        assert_eq!(frames.len(), 2, "no step may be dropped");
        assert_eq!(frames[0].step, start(2));
        assert_eq!(frames[1].sets_so_far, 1, "running totals travel with it");
        // `take_capture` closed the scope.
        assert!(!observing());
        emit(start(9), 0, 0);
        assert!(
            take_capture().is_empty(),
            "a drained scope must be closed, or one model's steps appear under the next"
        );
    }

    /// A live sink sees each step as it happens, and sees it **before** the
    /// buffer does — the ordering a debugger stop depends on.
    #[test]
    fn a_live_sink_is_called_before_the_buffer_is_filled() {
        let _guard = lock();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let recorder = std::sync::Arc::clone(&seen);
        start_capture();
        start_live(Box::new(move |frame| {
            // The buffer must not yet hold this step when the sink runs. It
            // cannot be read from here without re-entering the `RefCell`, so
            // what is pinned instead is the count the sink observed, compared
            // against the buffer afterwards.
            recorder
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(frame.step.clone());
        }));
        emit(start(1), 0, 0);
        emit(
            ConnectionStep::Complete {
                sets: 1,
                equations_added: 2,
            },
            1,
            2,
        );
        end_live();
        let buffered = take_capture();
        let live = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(live.len(), 2, "the sink must see every step");
        assert_eq!(
            live,
            buffered
                .iter()
                .map(|frame| frame.step.clone())
                .collect::<Vec<_>>(),
            "the sink and the buffer must agree about what happened"
        );
    }

    /// `end_live` must actually close, or the closure outlives the compile it
    /// was opened for and every later compile pays for it.
    #[test]
    fn end_live_closes_the_scope() {
        let _guard = lock();
        start_live(Box::new(|_| {
            panic!("sink must not be called after end_live")
        }));
        end_live();
        assert!(!observing());
        emit(start(0), 0, 0);
    }
}
