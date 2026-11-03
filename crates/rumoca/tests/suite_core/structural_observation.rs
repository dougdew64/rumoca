//! End-to-end regression for the observation-only structural seam.
//!
//! The focused unit tests live beside the algorithms, in
//! `rumoca_phase_structural::observation::algorithms`, and drive matching,
//! Tarjan and tearing on hand-built inputs. They cannot prove the one thing
//! this file is for: that [`inspect_sort`] threads the observer all the way
//! from the public entry point, through a real compiled model, into all three
//! searches -- and that what it reports agrees with the analysis it returned
//! beside it.
//!
//! Assertions check the shape the design guarantees rather than incidental
//! current numbers: a step kind appearing, a reported count equalling the
//! returned plan's count. An exact step sequence over a real model would fail
//! on any unrelated change to equation ordering and would be pinning Rumoca's
//! current output rather than this seam's contract.

use rumoca::Compiler;
use rumoca_phase_structural as structural;

/// One state plus a genuine three-equation algebraic loop.
///
/// `a`, `b` and `c` depend cyclically (`a` needs `b` and `c`, `b` needs `c`,
/// `c` needs `a`), so Tarjan must close a coupled component and tearing must
/// cut it -- which is what gives all three vocabularies something to report
/// from a single compile. The coefficient matrix is nonsingular
/// (determinant -1/2), so the model really does solve.
const COUPLED_LOOP: &str = r#"
model CoupledLoop
  Real x(start = 1, fixed = true);
  Real a;
  Real b;
  Real c;
equation
  der(x) = -a;
  a = b + c;
  b = 2*c - x;
  c = 0.5*a + x;
end CoupledLoop;
"#;

fn compile(source: &str, model: &str) -> rumoca::CompilationResult {
    Compiler::new()
        .model(model)
        .compile_str(source, &format!("{model}.mo"))
        .expect("the fixture must compile")
}

/// The seam reaches every search from the public entry, and the trace outlives
/// the view it was recorded from.
///
/// The `SortedDae` is branded to the inspected root and cannot leave the
/// closure; the [`structural::StructuralTrace`] is fully owned and can, which
/// is the entire reason the borrowed events have owned twins. Returning it
/// here is that property under test, not a convenience.
#[test]
fn inspect_sort_reports_every_search_it_ran() {
    let compiled = compile(COUPLED_LOOP, "CoupledLoop");
    let trace = compiled.dae.inspect(|view| {
        let (sorted, trace) = structural::inspect_sort(view);
        sorted.expect("CoupledLoop is a well-posed square system");
        trace
    });

    assert!(
        !trace.records.is_empty(),
        "a compile that ran the searches must report something"
    );

    let matching: Vec<_> = trace.matching().collect();
    assert!(
        !matching.is_empty(),
        "matching ran, so it must report: {:?}",
        trace.records.len()
    );
    assert!(
        matching
            .iter()
            .any(|record| matches!(record.step, structural::MatchingStep::Start { .. })),
        "the matching must open by stating the system it was handed"
    );
    assert!(
        matching
            .iter()
            .any(|record| matches!(record.step, structural::MatchingStep::Assign { .. })),
        "a system that matched perfectly must report its assignments"
    );

    let tarjan_runs = trace.tarjan_runs();
    assert!(
        !tarjan_runs.is_empty(),
        "the dependency graph was traversed, so a traversal must be reported"
    );
    assert!(
        tarjan_runs.iter().all(|run| matches!(
            run.first().map(|record| &record.step),
            Some(structural::TarjanStepRecord::Start { .. })
        )),
        "every traversal must open with the Start that states its numbering, \
         because its node indices are unreadable without one"
    );
}

/// The matching the trace ends on must be the matching the analysis used.
///
/// This is the property the previous design could not state: its traced
/// matching was a second implementation, so the recorded steps described a
/// run that merely tended to agree with the one that produced the blocks.
#[test]
fn the_reported_matching_is_the_matching_that_produced_the_blocks() {
    let compiled = compile(COUPLED_LOOP, "CoupledLoop");
    compiled.dae.inspect(|view| {
        let (sorted, trace) = structural::inspect_sort(view);
        let sorted = sorted.expect("CoupledLoop is a well-posed square system");
        let final_matching = trace
            .matching()
            .last()
            .map(|record| record.match_eq.clone())
            .expect("matching reported at least one step");
        assert_eq!(
            final_matching.iter().filter(|entry| entry.is_some()).count(),
            sorted.matching.len(),
            "the last reported matching must have exactly as many matched \
             equations as the analysis reported pairs"
        );
        assert!(
            final_matching.iter().all(Option::is_some),
            "a system that sorted must leave no equation unmatched: \
             {final_matching:?}"
        );
    });
}

/// A torn algebraic loop must report which pass's plan was adopted, and that
/// report must describe the plan actually attached to the block.
///
/// Tearing runs the greedy search under both priorities and keeps the smaller
/// tear set, so a consumer reading the first pass as the plan would describe a
/// tear sequence the compiler discarded. If this model stops producing a torn
/// loop the test says so rather than passing vacuously.
#[test]
fn a_torn_block_reports_the_plan_it_actually_adopted() {
    let compiled = compile(COUPLED_LOOP, "CoupledLoop");
    compiled.dae.inspect(|view| {
        let (sorted, trace) = structural::inspect_sort(view);
        let sorted = sorted.expect("CoupledLoop is a well-posed square system");
        let plans: Vec<_> = sorted
            .blocks
            .iter()
            .filter_map(|block| match block {
                structural::BltBlock::AlgebraicLoop { tearing, .. } => tearing.as_ref(),
                _ => None,
            })
            .collect();
        assert!(
            !plans.is_empty(),
            "CoupledLoop's a/b/c cycle must produce a torn algebraic loop; if \
             this fails the fixture no longer exercises tearing and the \
             assertions below would pass vacuously"
        );

        let selections: Vec<_> = trace
            .tearing_passes()
            .into_iter()
            .flatten()
            .filter_map(|record| match record.step {
                structural::TearingStep::PlanSelected {
                    tears, residuals, ..
                } => Some((tears, residuals)),
                _ => None,
            })
            .collect();
        assert_eq!(
            selections.len(),
            plans.len(),
            "every adopted plan must be reported exactly once"
        );
        for (plan, (tears, residuals)) in plans.iter().zip(&selections) {
            assert_eq!(
                (*tears, *residuals),
                (
                    plan.tear_var_local_indices.len(),
                    plan.residual_eq_local_indices.len()
                ),
                "the reported selection must describe the plan on the block"
            );
        }
    });
}

/// The canonical index-2 system, which is *balanced* but structurally
/// singular as written.
///
/// The unknowns are `der(x)` and `y`. The first equation reaches both; the
/// second constrains `x`, which is a state rather than an unknown, so it
/// reaches *none* -- which is exactly why no perfect matching exists and why
/// index reduction has to differentiate the constraint before the system can
/// be solved. An unbalanced model would not do: Rumoca's balance check
/// rejects those before structural analysis ever runs, so a test built on one
/// passes without reaching the path it claims to cover.
const INDEX_TWO: &str = r#"
model IndexTwo
  Real x(start = 0, fixed = true);
  Real y;
equation
  der(x) = y;
  x = sin(time);
end IndexTwo;
"#;

/// A system that fails to match must still report the search that proved it.
///
/// This is the path the seam's predecessor deliberately kept observed: a
/// system that fails to match is the one whose search is most worth watching,
/// and the matching steps that proved it singular are already recorded by the
/// time the error is returned. The trace comes back beside the `Err`, not
/// instead of it -- a property this seam gets by construction, because the
/// recorder outlives the `?` that returns the error.
#[test]
fn a_singular_system_still_reports_the_search_that_proved_it() {
    let compiled = compile(INDEX_TWO, "IndexTwo");
    compiled.dae.inspect(|view| {
        let (sorted, trace) = structural::inspect_sort(view);
        let error = sorted
            .err()
            .expect("an index-2 system has no perfect matching as written");
        assert!(
            matches!(error, structural::StructuralError::Singular { .. }),
            "the failure must be the singularity, not something incidental: \
             {error:?}"
        );
        let matching: Vec<_> = trace.matching().collect();
        assert!(
            !matching.is_empty(),
            "the search that proved the system singular must be reported \
             alongside the error, not discarded with it"
        );
        assert!(
            matching
                .iter()
                .any(|record| matches!(
                    record.step,
                    structural::MatchingStep::EquationFailed(_)
                )),
            "the equation that could claim no unknown must be named: \
             {:?}",
            matching.iter().map(|r| &r.step).collect::<Vec<_>>()
        );
    });
}
