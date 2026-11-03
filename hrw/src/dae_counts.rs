//! How many of each thing a DAE holds, counted once.
//!
//! # Why this is a module rather than a few `.len()` calls
//!
//! Before v0.10.0 every caller read a field: `dae.variables.states.len()`,
//! `dae.continuous.equations.len()`. Those reads were spread across
//! `worker.rs`, `equation_sheet.rs` and the stage summaries, and each was
//! independently correct, so nothing could disagree.
//!
//! On 0.10.0 the eight pre-partitioned variable vectors are gone: a `Dae`
//! holds one variable arena and each entry carries a
//! [`dae::VariableRole`]. A count is now a **tally**, and a tally written out
//! at each call site is a tally that can be written differently at each call
//! site — one caller folding both discrete roles together, another counting
//! outputs as algebraics. That is the kind of disagreement nobody notices,
//! because every number on screen looks plausible.
//!
//! So the tally happens here, once, and callers read fields off the result.
//! Every count is one accessor on a [`dae::DaeView`], named as Rumoca names it.

use rumoca_ir_dae as dae;

/// Everything countable about one DAE, gathered in a single `inspect`.
///
/// Field names follow Rumoca's own vocabulary rather than HRW's older labels,
/// so a reader comparing a pane against the IR is comparing like with like.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DaeCounts {
    // --- variables, by role ---
    pub states: usize,
    pub algebraics: usize,
    pub parameters: usize,
    pub constants: usize,
    pub inputs: usize,
    pub outputs: usize,
    pub discrete_reals: usize,
    pub discrete_values: usize,

    // --- equations and the event structure ---
    /// Scalar residual rows, the `f_x` of the stage trees.
    pub continuous_equations: usize,
    pub initialization_equations: usize,
    /// MLS §8.5 definitions of discrete Real coordinates — the `f_z` updates.
    pub discrete_real_equations: usize,
    /// Definitions of discrete-valued coordinates — the `f_m` updates.
    pub discrete_value_definitions: usize,
    /// MLS §8.3.5 activation conditions.
    pub conditions: usize,
    /// MLS §3.5 relations that can generate a state event.
    pub relations: usize,
    /// Monitored scalar root surfaces.
    pub roots: usize,
    /// Tensor-native families of root surfaces over a compact domain.
    pub structured_roots: usize,
    /// MLS §8.3.6 time events.
    pub time_events: usize,
    /// MLS §8.3.4 event actions.
    pub event_actions: usize,
}

impl DaeCounts {
    /// Tally one DAE.
    #[must_use]
    pub fn of(model: &dae::Dae) -> Self {
        model.inspect(|view| {
            let mut counts = Self {
                continuous_equations: view.continuous_equation_count(),
                initialization_equations: view.initialization_equation_count(),
                discrete_real_equations: view.discrete_real_equation_count(),
                discrete_value_definitions: view.discrete_value_definition_count(),
                conditions: view.condition_count(),
                relations: view.relation_count(),
                roots: view.root_count(),
                structured_roots: view.structured_root_count(),
                time_events: view.time_event_count(),
                event_actions: view.event_action_count(),
                ..Self::default()
            };
            for (_id, variable) in view.variables() {
                // Exhaustive on purpose: a role added upstream fails to compile
                // here rather than quietly going uncounted, which would show as
                // a total that is simply too small.
                let counter = match variable.role() {
                    dae::VariableRole::State => &mut counts.states,
                    dae::VariableRole::Algebraic => &mut counts.algebraics,
                    dae::VariableRole::Parameter => &mut counts.parameters,
                    dae::VariableRole::Constant => &mut counts.constants,
                    dae::VariableRole::Input => &mut counts.inputs,
                    dae::VariableRole::Output => &mut counts.outputs,
                    dae::VariableRole::DiscreteReal => &mut counts.discrete_reals,
                    dae::VariableRole::DiscreteValue => &mut counts.discrete_values,
                };
                *counter += 1;
            }
            counts
        })
    }

    /// Both discrete roles together, which is how several panes report them.
    ///
    /// A helper rather than a field so the two roles stay separately countable:
    /// a pane that wants the split can have it, and a pane that wants the total
    /// cannot accidentally get one role's count alone.
    #[must_use]
    pub const fn discrete(self) -> usize {
        self.discrete_reals + self.discrete_values
    }

    /// Whether the model has an update that makes a trajectory jump.
    ///
    /// **A bare zero crossing does not count.** A relation that merely changes
    /// sign leaves the trajectory continuous; what breaks it is an assignment —
    /// a `when` body or a `reinit`. Plot code uses this to decide whether to
    /// split its polylines, so including zero crossings here would cut a
    /// perfectly continuous curve at every crossing.
    #[must_use]
    pub const fn has_discontinuities(self) -> bool {
        self.discrete_real_equations > 0 || self.discrete_value_definitions > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumoca_core::{SourceMap, Span, TypeId, VarName};

    /// A DAE with one variable in each of four roles and one equation.
    fn fixture() -> dae::Dae {
        let mut sources = SourceMap::new();
        let sid = sources.add("test://counts.mo", "Real x; Real a; parameter Real p; Real u;");
        let at = dae::DaeProvenance::source(Span::from_offsets(sid, 0, 7))
            .expect("fixture span lies inside the fixture source");

        dae::Dae::construct(sources, |model| {
            let real = model.types(|types| {
                types.intern(
                    TypeId::new(0),
                    dae::ValueType::scalar(dae::ScalarType::Real),
                    at,
                )
            })?;
            let x = model.variables(|variables| {
                let x = variables.state(
                    VarName::new("x"),
                    real,
                    at,
                    dae::VariableAttributes::default(),
                )?;
                variables.algebraic(
                    VarName::new("a"),
                    real,
                    at,
                    dae::VariableAttributes::default(),
                )?;
                variables.parameter(
                    VarName::new("p"),
                    real,
                    at,
                    dae::VariableAttributes::default(),
                )?;
                variables.output(
                    VarName::new("u"),
                    real,
                    at,
                    dae::VariableAttributes::default(),
                )?;
                Ok(x)
            })?;
            let residual = model.expressions(|expressions| {
                expressions
                    .at(at)
                    .coordinate(dae::CoordinateInput::Derivative(x))
            })?;
            model.continuous(|continuous| {
                continuous.equation(at, |equation| equation.residual(residual))?;
                Ok(())
            })
        })
        .expect("fixture DAE is well-formed")
    }

    /// Each role is counted in its own field, and nothing lands in a neighbour.
    ///
    /// The zero assertions are the half that matters: a tally that wrote every
    /// variable into one counter would satisfy a test that only checked the
    /// roles present.
    #[test]
    fn each_role_is_counted_separately() {
        let counts = DaeCounts::of(&fixture());
        assert_eq!(counts.states, 1);
        assert_eq!(counts.algebraics, 1);
        assert_eq!(counts.parameters, 1);
        assert_eq!(counts.outputs, 1);
        assert_eq!(counts.constants, 0);
        assert_eq!(counts.inputs, 0);
        assert_eq!(counts.discrete_reals, 0);
        assert_eq!(counts.discrete_values, 0);
        assert_eq!(counts.discrete(), 0);
        assert_eq!(counts.continuous_equations, 1);
    }

    /// A model with no update is not reported as discontinuous.
    ///
    /// This is the direction worth pinning: `has_discontinuities` returning true
    /// here would make the plot break a continuous curve into segments, which
    /// reads as a jump the model never had.
    #[test]
    fn a_model_with_no_updates_is_continuous() {
        let counts = DaeCounts::of(&fixture());
        assert_eq!(counts.discrete_real_equations, 0);
        assert_eq!(counts.discrete_value_definitions, 0);
        assert!(!counts.has_discontinuities());
    }
}
