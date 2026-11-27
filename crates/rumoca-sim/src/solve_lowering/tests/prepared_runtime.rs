//! `prepare_runtime_observed` + `simulate_prepared_runtime_with_optional_step_records`
//! is `simulate_with_optional_step_records`'s `Auto` arm split in two, so a
//! caller can inspect the lowering and then integrate that same artifact.
//! These pin that splitting it changes nothing about the run.

use super::*;

const OSCILLATOR: &str = r#"
model PreparedOscillator
  parameter Real omega = 2.0;
  Real x(start = 1.0);
  Real v(start = 0.0);
equation
  der(x) = v;
  der(v) = -omega ^ 2 * x;
end PreparedOscillator;
"#;

fn auto_options() -> SimOptions {
    SimOptions {
        t_end: 1.0,
        solver_mode: SimSolverMode::Auto,
        ..SimOptions::default()
    }
}

#[test]
fn a_prepared_runtime_integrates_exactly_as_the_combined_entry() {
    let model = compile(OSCILLATOR, "PreparedOscillator");
    let opts = auto_options();

    let (combined, combined_steps) =
        crate::simulate_with_optional_step_records(&model, &opts).unwrap();

    let runtime = crate::prepare_runtime_observed(&model, &opts, |_| {}).unwrap();
    let (split, split_steps) =
        crate::simulate_prepared_runtime_with_optional_step_records(runtime, &opts).unwrap();

    assert_eq!(split.names, combined.names);
    assert_eq!(
        split.times, combined.times,
        "the split run sampled different times"
    );
    assert_eq!(
        split.data, combined.data,
        "the split run integrated differently"
    );
    assert_eq!(
        split_steps.map(|steps| steps.len()),
        combined_steps.map(|steps| steps.len()),
        "the split run reported a different step count",
    );
}

const ALIASED_DECAY: &str = r#"
model AliasedDecay
  Real x(start = 1.0);
  Real y;
equation
  der(x) = -x;
  y = x;
end AliasedDecay;
"#;

#[test]
fn both_preparation_traces_record_the_run_that_was_integrated() {
    use rumoca_phase_solve::preparation_trace::{self as solve_trace, SolvePreparationEvent};
    use rumoca_phase_structural::preparation_trace::{
        self as structural_trace, AnalysisContext, PreparationEvent,
    };

    let model = compile(ALIASED_DECAY, "AliasedDecay");
    let opts = auto_options();

    let runtime = crate::prepare_runtime_observed(&model, &opts, |_| {}).unwrap();
    let (unobserved, _) =
        crate::simulate_prepared_runtime_with_optional_step_records(runtime, &opts).unwrap();

    structural_trace::start_capture();
    solve_trace::start_capture();
    let runtime = crate::prepare_runtime_observed(&model, &opts, |_| {}).unwrap();
    let solve_events = solve_trace::take_capture();
    let structural_events = structural_trace::take_capture();
    let (observed, _) =
        crate::simulate_prepared_runtime_with_optional_step_records(runtime, &opts).unwrap();

    assert_eq!(
        observed.data, unobserved.data,
        "capturing preparation changed the simulation it observed",
    );

    // The four source transforms, in the order the solve path runs them, and
    // the alias quotient is the one that fires on `y = x`.
    let transforms: Vec<_> = solve_events
        .iter()
        .filter_map(|event| match event {
            SolvePreparationEvent::Transform {
                transform, after, ..
            } => Some((*transform, after.is_some())),
            _ => None,
        })
        .collect();
    use solve_trace::SourceTransform as T;
    assert_eq!(
        transforms,
        vec![
            (T::FoldEvaluableParameters, false),
            (T::InlineAnnotatedCalls, false),
            (T::QuotientAliases, true),
            (T::FoldConstantValues, false),
        ],
    );
    assert!(
        solve_events
            .iter()
            .any(|event| matches!(event, SolvePreparationEvent::Route(_))),
        "no selection route was recorded: {solve_events:?}",
    );
    assert!(
        matches!(
            solve_events.last(),
            Some(SolvePreparationEvent::Selected { integrated, .. }) if integrated == &["x"]
        ),
        "the selection must be recorded last and integrate x: {solve_events:?}",
    );

    // The structural side saw the quotient and then analysed the source.
    assert!(
        structural_events
            .iter()
            .any(|event| matches!(event, PreparationEvent::AliasQuotient { report, .. } if report.eliminated_count() == 1)),
        "the alias quotient that eliminated y was not recorded",
    );
    let (report, source_incidence) = structural_events
        .iter()
        .find_map(|event| match event {
            PreparationEvent::StructuralAnalysis {
                context: AnalysisContext::Source,
                outcome: Ok(report),
                incidence: Some(incidence),
                ..
            } => Some((report, incidence)),
            _ => None,
        })
        .expect("the source analysis the reducer started from was not recorded");
    // The captured incidence is the system the report matched: one row per
    // matched equation, and every matched unknown is one of its columns. (The
    // quotient keeps `y` with `y = x` as its definition, so both remain.)
    assert_eq!(source_incidence.rows.len(), report.n_equations);
    assert_eq!(source_incidence.unknowns.len(), report.n_unknowns);
    for (_, unknown) in &report.matching {
        assert!(
            source_incidence.unknowns.contains(unknown),
            "matched unknown {unknown} is not a column of the captured incidence",
        );
    }
}

#[test]
fn the_observer_sees_the_lowered_model_once_and_before_integration() {
    let model = compile(OSCILLATOR, "PreparedOscillator");
    let opts = auto_options();

    let mut observed_states = Vec::new();
    let runtime = crate::prepare_runtime_observed(&model, &opts, |lowered| {
        observed_states.push(lowered.model().state_scalar_count());
    })
    .unwrap();

    assert_eq!(
        observed_states,
        vec![2],
        "the observer must fire exactly once, with the two states this model integrates",
    );
    let (result, _) =
        crate::simulate_prepared_runtime_with_optional_step_records(runtime, &opts).unwrap();
    assert_eq!(result.n_states, 2);
}
