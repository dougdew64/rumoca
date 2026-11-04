//! Observation-only reporting of accepted numerical steps.
//!
//! A simulation's `SimResult` carries the trajectory it was asked for: samples
//! at the output grid. It says nothing about *how* the integrator got there —
//! what step size it chose, and what order its continuous extension reached.
//! Those are properties of the solve rather than of the model, and they are
//! what a reader needs to tell a stiff interval from a smooth one, or to see a
//! BDF method raise its order as the solution settles.
//!
//! # The shape, and why it is this one
//!
//! Modelled on [`crate::fmi_me::session::MeSimulationSession`]'s own
//! discipline: nothing here can change what the solver does.
//!
//! - The event is **borrowed** and names only values the session already holds
//!   at the moment it holds them, so emitting one allocates nothing and copies
//!   nothing.
//! - [`MeStepObserver::observe`] returns `()`. **It cannot fail**, which is
//!   load-bearing rather than convenient: the session's accepted-step path has
//!   no rollback proof past the plugin's `advance`, so a hook that could return
//!   an error would be a new way to lose a live session. The only thing an
//!   observer can do is be slow.
//! - `()` implements the trait with an empty body, so a session that installs
//!   no observer pays one `Option` branch per accepted step and nothing else.
//! - The owned [`MeStepRecord`] is a separate type, because a caller wants the
//!   steps *after* the run while the event borrows state that only exists
//!   during it.
//!
//! # What it deliberately does not report
//!
//! Only **accepted** steps. A BDF implementation may try and reject several
//! candidate sizes before one succeeds, and those attempts are the library's
//! internal business — `MeIntegratorBackend::advance` returns once per accepted
//! step and the session never sees the rest. Reporting "steps" that silently
//! meant accepted ones would be the more useful-sounding and less true choice,
//! so the name says which.

/// One fact about the solve, borrowed from the session that holds it.
///
/// Non-exhaustive: a later release may report more about a step, and a consumer
/// matching on this should not break when it does.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum MeStepEvent {
    /// The integrator accepted a step, and the host proved its coordinates.
    ///
    /// Emitted once the step exists as a proved proposal, not when the plugin
    /// returns a candidate: a candidate that fails its obligations never
    /// happened as far as the trajectory is concerned, and reporting one would
    /// put a step on a plot that the solver did not take.
    ///
    /// Both endpoints are reported rather than a step size, because the
    /// endpoints are what the session holds. A `size` field would be a second
    /// representation of the same fact and could disagree with it;
    /// [`MeStepRecord::step_size`] derives it instead.
    Accepted {
        /// The time the step started from — the host-issued current coordinate.
        start_time: f64,
        /// The time the step reached.
        accepted_time: f64,
        /// The order of the plugin's local continuous extension, always ≥ 1.
        order: u32,
    },
}

/// Something that watches a simulation's accepted steps.
///
/// `()` is the production implementation and its `observe` body is empty.
pub trait MeStepObserver {
    /// Receive one fact. Must not panic, and cannot fail.
    fn observe(&mut self, event: MeStepEvent);
}

impl MeStepObserver for () {
    fn observe(&mut self, _event: MeStepEvent) {}
}

/// A mutable borrow observes by forwarding.
///
/// This is what lets a caller keep its recorder on the stack and lend it for
/// the run: `session.observe_steps(Box::new(&mut recorder))` installs a borrow
/// rather than moving the recorder in, so the records are readable afterwards
/// without the session having to hand anything back.
///
/// The alternative was interior mutability — an `Rc<RefCell<…>>` shared between
/// caller and session — which would have cost a runtime borrow check per step
/// and made the observer `!Send` for no reason.
impl<T: MeStepObserver + ?Sized> MeStepObserver for &mut T {
    fn observe(&mut self, event: MeStepEvent) {
        (**self).observe(event);
    }
}

/// One accepted step, owned, so it outlives the session that reported it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeStepRecord {
    /// The time the step started from.
    pub start_time: f64,
    /// The time the step reached.
    pub accepted_time: f64,
    /// The order of the local continuous extension over this step.
    pub order: u32,
}

impl MeStepRecord {
    /// The step size the integrator chose.
    ///
    /// Derived rather than stored, so it cannot disagree with the endpoints it
    /// is computed from.
    #[must_use]
    pub fn step_size(&self) -> f64 {
        self.accepted_time - self.start_time
    }
}

/// Collects every accepted step into a `Vec`.
///
/// The obvious observer, provided here so that each consumer does not write it
/// again. A consumer wanting to stream instead implements the trait directly.
#[derive(Debug, Default)]
pub struct MeStepRecorder {
    records: Vec<MeStepRecord>,
}

impl MeStepRecorder {
    /// A recorder holding no steps yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The steps recorded so far, in the order the solver took them.
    #[must_use]
    pub fn records(&self) -> &[MeStepRecord] {
        &self.records
    }

    /// Take the steps, leaving the recorder empty.
    #[must_use]
    pub fn into_records(self) -> Vec<MeStepRecord> {
        self.records
    }
}

impl MeStepObserver for MeStepRecorder {
    fn observe(&mut self, event: MeStepEvent) {
        match event {
            MeStepEvent::Accepted {
                start_time,
                accepted_time,
                order,
            } => self.records.push(MeStepRecord {
                start_time,
                accepted_time,
                order,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The unit observer records nothing and the recorder records everything.**
    ///
    /// Both halves matter: `()` compiling to an empty body is what makes the
    /// seam free in production, and a recorder that dropped an event would make
    /// every plot built on it quietly short.
    #[test]
    fn the_recorder_keeps_each_step_and_the_unit_observer_keeps_none() {
        let events = [
            MeStepEvent::Accepted {
                start_time: 0.0,
                accepted_time: 0.125,
                order: 1,
            },
            MeStepEvent::Accepted {
                start_time: 0.125,
                accepted_time: 0.5,
                order: 3,
            },
        ];

        let mut unit = ();
        let mut recorder = MeStepRecorder::new();
        for event in events {
            unit.observe(event);
            recorder.observe(event);
        }

        assert_eq!(recorder.records().len(), 2, "both steps are kept");
        assert_eq!(recorder.records()[1].order, 3, "and in the order taken");
        let sizes: Vec<f64> = recorder.records().iter().map(MeStepRecord::step_size).collect();
        assert_eq!(
            sizes,
            vec![0.125, 0.375],
            "step size is the interval the solver covered, derived from its endpoints",
        );
        assert!(
            recorder.into_records().len() == 2,
            "taking the records yields the same two",
        );
    }

    /// A step size is the difference of the endpoints, including when the
    /// solver takes a very small one.
    ///
    /// The small case is the reason these plots exist: Doug could not verify a
    /// solver claim because a 1e-4 step rendered as `0.000`, so a step size
    /// that reaches a reader as zero is the failure this type sits upstream of.
    #[test]
    fn a_tiny_step_keeps_its_magnitude() {
        let record = MeStepRecord {
            start_time: 1.0,
            accepted_time: 1.000_1,
            order: 2,
        };
        assert!(
            (record.step_size() - 1e-4).abs() < 1e-12,
            "a 1e-4 step must survive as 1e-4, not round to nothing: {}",
            record.step_size(),
        );
    }
}
