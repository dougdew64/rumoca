//! Observation of the **structural-preparation funnel** as a sequence of steps.
//!
//! # What this adds, and why the existing tracing was not enough
//!
//! `dae_prepare` already publishes per-step detail for the two passes that reduce
//! index: [`crate::dae_prepare::IndexReductionFrame`] records every candidate
//! considered, every constraint differentiated and every state demoted. That is the
//! *inside* of two steps.
//!
//! What has no representation is the **funnel itself** — which steps ran, in what
//! order, and what each did to the system. A caller can observe the DAE before and
//! after the whole sequence and must infer the rest.
//!
//! Inference is not a hypothetical cost. A consumer reading only the final DAE
//! concluded that a model performed *zero* differentiations, because the rows
//! carrying differentiation markers had been removed by a later elimination step.
//! Both observations were individually correct and the conclusion was wrong: nothing
//! reported that the intermediate step had happened at all.
//!
//! # Shape
//!
//! One [`FunnelStepFrame`] per step, carrying the system's size on either side of it
//! and how the step finished. Sizes rather than contents, deliberately — a frame per
//! step must stay cheap enough that observing is never a reason not to, and a consumer
//! that wants contents has the DAE.
//!
//! Delivered through [`rumoca_core::FrameObserver`], the same contract every other
//! traced phase here uses: `None` costs a branch, and the observer chooses whether to
//! buffer, stream or count.

use rumoca_ir_dae::Dae;

/// How a funnel step finished.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FunnelStepOutcome {
    /// Ran to completion. The frame's before/after counts say what it changed.
    Completed,
    /// Ran to completion and reported a count of states demoted.
    ///
    /// Distinct from [`Self::Completed`] because a step that *reports* zero and a step
    /// that reports nothing are different facts, and collapsing them is how "the step
    /// did nothing" becomes indistinguishable from "the step does not say".
    Demoted(usize),
    /// Ran to completion and reported a count of rows it rewrote.
    ///
    /// Separate from [`Self::Demoted`] because rewriting a row and demoting a state are
    /// different acts on different objects, and a consumer showing "n" beside a step
    /// name needs to know which noun it counts.
    Rewrote(usize),
    /// Returned an error. The funnel stops here, so no later step ran.
    ///
    /// **This is the variant that makes the trace a diagnostic rather than a report.**
    /// Without it a failing funnel yields one error at the top and no indication of
    /// which of ten steps produced it.
    Failed(String),
}

/// One step of the structural-preparation funnel, as it happened.
#[derive(Debug, Clone)]
pub struct FunnelStepFrame {
    /// The step's name, as the funnel knows it — e.g.
    /// `"reduce_constrained_dummy_derivatives"`.
    pub step: &'static str,
    /// Differential states before the step ran.
    pub states_before: usize,
    /// Differential states after it ran. Unchanged from `states_before` on a step
    /// that demotes nothing.
    pub states_after: usize,
    /// Continuous equations before the step ran.
    pub equations_before: usize,
    /// Continuous equations after it ran.
    pub equations_after: usize,
    /// How it finished.
    pub outcome: FunnelStepOutcome,
}

impl FunnelStepFrame {
    /// Did this step change the system at all?
    ///
    /// Most steps in a typical funnel run do nothing, and a consumer rendering ten
    /// rows of "no change" buries the two that matter.
    #[must_use]
    pub fn changed_anything(&self) -> bool {
        self.states_before != self.states_after || self.equations_before != self.equations_after
    }
}

/// The system's size, as a funnel step sees it.
///
/// Taken before and after each step rather than diffed from a clone: cloning a DAE per
/// step would make observation cost more than the work being observed, on a phase that
/// already dominates compile time for large models.
#[must_use]
pub fn dae_shape(dae: &Dae) -> (usize, usize) {
    (dae.variables.states.len(), dae.continuous.equations.len())
}

/// Send one funnel-step frame to an observer, if anybody is watching.
///
/// By reference, so an unobserved run allocates nothing and a watching one clones only
/// if it decides to keep the frame — the same contract as
/// [`crate::dae_prepare::emit_index_reduction_frame`].
pub fn emit_funnel_step(
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
    frame: FunnelStepFrame,
) {
    if let Some(observe) = observer {
        observe(&frame);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unobserved_emit_is_a_no_op() {
        // The contract's cheap half: no observer, nothing happens, nothing allocated.
        emit_funnel_step(
            None,
            FunnelStepFrame {
                step: "demote_direct_assigned_states",
                states_before: 9,
                states_after: 9,
                equations_before: 97,
                equations_after: 97,
                outcome: FunnelStepOutcome::Demoted(0),
            },
        );
    }

    #[test]
    fn an_observer_receives_every_frame_it_is_sent() {
        use std::cell::RefCell;

        let seen: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let observe = |f: &FunnelStepFrame| seen.borrow_mut().push(f.step.to_string());

        for step in ["demote_direct_assigned_states", "eliminate_trivial"] {
            emit_funnel_step(
                Some(&observe),
                FunnelStepFrame {
                    step,
                    states_before: 3,
                    states_after: 3,
                    equations_before: 20,
                    equations_after: 20,
                    outcome: FunnelStepOutcome::Completed,
                },
            );
        }

        assert_eq!(
            seen.into_inner(),
            vec![
                "demote_direct_assigned_states".to_string(),
                "eliminate_trivial".to_string()
            ],
            "an observer must see every step in the order the funnel ran them; order is \
             the whole point of a funnel trace",
        );
    }

    #[test]
    fn a_step_that_changes_nothing_says_so() {
        let unchanged = FunnelStepFrame {
            step: "expand_compound_derivatives",
            states_before: 3,
            states_after: 3,
            equations_before: 20,
            equations_after: 20,
            outcome: FunnelStepOutcome::Completed,
        };
        assert!(!unchanged.changed_anything());

        let demoting = FunnelStepFrame {
            states_after: 2,
            ..unchanged.clone()
        };
        assert!(
            demoting.changed_anything(),
            "a demotion changes the state count, and a consumer needs to find the steps \
             that did something among the ones that did not",
        );

        let eliminating = FunnelStepFrame {
            equations_after: 12,
            ..unchanged
        };
        assert!(
            eliminating.changed_anything(),
            "and a step may change only the equation count \u{2014} `eliminate_trivial` \
             removes rows without demoting anything",
        );
    }
}
