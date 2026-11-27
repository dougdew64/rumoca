//! Observation of the solve path's preparation **as it runs**: which source
//! transforms fired, which state-selection route the run took, and what it
//! chose to integrate.
//!
//! Additive and observation-only, and closed by default: with no scope open
//! each emit is one thread-local read, and a [`DaeShape`] is measured only
//! while a scope is open.
//!
//! # Why
//!
//! Solve lowering does not prepare the DAE it is handed. It folds evaluable
//! parameters, inlines annotated calls, quotients aliases and folds constants
//! first, and only then runs structural preparation — and when that succeeds
//! it still chooses between retaining the reduced system and reducing to a
//! formal-derivative basis. None of that is visible in the lowered result, and
//! a tool that wants to show it otherwise has to call the transforms itself,
//! which shows a second execution rather than this one.
//!
//! This module records the decisions; `rumoca-phase-structural`'s
//! `preparation_trace` records the structural analyses and reduction rounds
//! the same run performed. A caller wanting the whole preparation opens both.
//!
//! [`take_capture`] drains and closes; a scope should wrap one model.

use std::cell::RefCell;

use rumoca_ir_dae as dae;

/// One of the source transforms run before structural preparation, in the
/// order `prepare_with_input_policy` runs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTransform {
    /// Reads of evaluable parameters replaced by their settled values.
    FoldEvaluableParameters,
    /// `Inline` / `LateInline` pure calls inlined before index reduction.
    InlineAnnotatedCalls,
    /// Copy and negation alias classes quotiented onto a representative.
    QuotientAliases,
    /// Constant pure calls evaluated and literal algebraics propagated.
    FoldConstantValues,
}

/// The size of one DAE, by variable role and equation partition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DaeShape {
    pub states: usize,
    pub algebraics: usize,
    pub outputs: usize,
    pub parameters: usize,
    pub constants: usize,
    pub inputs: usize,
    pub discrete: usize,
    pub continuous_equations: usize,
    pub initialization_equations: usize,
}

impl DaeShape {
    /// Measure `model`: declarations by role, equations by partition.
    #[must_use]
    pub fn of(model: &dae::Dae) -> Self {
        model.inspect(|view| {
            let mut shape = Self {
                continuous_equations: view.continuous_equation_count(),
                initialization_equations: view.initialization_equation_count(),
                ..Self::default()
            };
            for (_, variable) in view.variables() {
                *shape.slot(variable.role()) += 1;
            }
            shape
        })
    }

    fn slot(&mut self, role: dae::VariableRole) -> &mut usize {
        match role {
            dae::VariableRole::State => &mut self.states,
            dae::VariableRole::Algebraic => &mut self.algebraics,
            dae::VariableRole::Output => &mut self.outputs,
            dae::VariableRole::Parameter => &mut self.parameters,
            dae::VariableRole::Constant => &mut self.constants,
            dae::VariableRole::Input => &mut self.inputs,
            dae::VariableRole::DiscreteReal | dae::VariableRole::DiscreteValue => {
                &mut self.discrete
            }
        }
    }
}

/// Which way state selection prepared the system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectionRoute {
    /// The source holds a redundant loop closure, so the formal selection was
    /// built directly and the per-state reducer did not run.
    LoopClosureReduced,
    /// The per-state reducer prepared the system.
    ReducerAccepted,
    /// The reducer reported the system singular; states no equation reads were
    /// declared algebraic and the system was prepared again.
    InertStatesDemoted,
    /// The reducer reported the system singular and the formal-derivative path
    /// recovered it.
    RecoveredViaFormal,
    /// The reduced system has no manifold, so `StateSelect` preferences decided.
    PreferencesDecided,
    /// A manifold remains but no formal basis is smaller than the retained
    /// states, so the reduced system was retained.
    RetainedNoSmallerBasis,
    /// Every manifold constraint is a conserved first integral and no
    /// preferred value forces a reduction, so the reduced system was retained.
    RetainedDefinitional,
    /// The system was reduced to a formal-derivative basis.
    ReducedToFormalBasis,
}

/// One observed step of solve preparation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SolvePreparationEvent {
    /// One source transform ran on a system of shape `before`; `after` is the
    /// shape it produced, or `None` when it changed nothing.
    Transform {
        transform: SourceTransform,
        before: DaeShape,
        after: Option<DaeShape>,
    },
    /// A routing decision, in the order it was made.
    Route(SelectionRoute),
    /// What the finished selection integrates: the primary chart's integrated
    /// scalar names, and how many alternate charts were prepared beside it.
    Selected {
        integrated: Vec<String>,
        alternate_charts: usize,
    },
}

thread_local! {
    /// Ambient capture buffer: `Some` while a capture scope is open.
    static CAPTURE: RefCell<Option<Vec<SolvePreparationEvent>>> = const { RefCell::new(None) };
}

/// **Begin capturing solve preparation on this thread.** A scope already open
/// is restarted empty.
pub fn start_capture() {
    CAPTURE.with(|capture| *capture.borrow_mut() = Some(Vec::new()));
}

/// Take the captured events and close the scope. Empty if none was open.
pub fn take_capture() -> Vec<SolvePreparationEvent> {
    CAPTURE
        .with(|capture| capture.borrow_mut().take())
        .unwrap_or_default()
}

fn capturing() -> bool {
    CAPTURE.with(|capture| capture.borrow().is_some())
}

fn emit(event: SolvePreparationEvent) {
    CAPTURE.with(|capture| {
        if let Some(events) = capture.borrow_mut().as_mut() {
            events.push(event);
        }
    });
}

/// Record one transform's run, measuring only while a scope is open.
pub(crate) fn transform(transform: SourceTransform, input: &dae::Dae, output: Option<&dae::Dae>) {
    if capturing() {
        emit(SolvePreparationEvent::Transform {
            transform,
            before: DaeShape::of(input),
            after: output.map(DaeShape::of),
        });
    }
}

/// Record one routing decision.
pub(crate) fn route(route: SelectionRoute) {
    if capturing() {
        emit(SolvePreparationEvent::Route(route));
    }
}

/// Record the finished selection, naming its integrated scalars only while a
/// scope is open.
///
/// `None` from `integrated` records nothing: a selection whose names cannot be
/// resolved is not reported as integrating an empty set.
pub(crate) fn selected(integrated: impl FnOnce() -> Option<Vec<String>>, alternate_charts: usize) {
    if capturing()
        && let Some(integrated) = integrated()
    {
        emit(SolvePreparationEvent::Selected {
            integrated,
            alternate_charts,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_scope_records_nothing() {
        assert!(take_capture().is_empty());
        route(SelectionRoute::ReducerAccepted);
        assert!(take_capture().is_empty());
    }

    #[test]
    fn take_drains_and_closes_the_scope() {
        start_capture();
        route(SelectionRoute::ReducerAccepted);
        assert_eq!(
            take_capture(),
            vec![SolvePreparationEvent::Route(
                SelectionRoute::ReducerAccepted
            )]
        );
        assert!(!capturing());
    }
}
