//! The solver's accepted steps are reportable, and reporting them changes
//! nothing about the solve.
//!
//! `SimResult` describes the trajectory: samples at the output grid. It cannot
//! answer what step size the integrator chose or what order its continuous
//! extension reached, because those are properties of the *solve*. The ME
//! session can report them through
//! [`rumoca_solver::fmi_me::step_observation`], and
//! [`rumoca_sim::simulate_with_step_records`] is the entry that collects them.
//!
//! The one property that has to hold for any of it to be worth having is that
//! **an observed run and an unobserved run are the same run**. That is asserted
//! here against a real model rather than argued from the observer's signature.

use rumoca::Compiler;
use rumoca_sim::{
    SimOptions, SimSolverMode, simulate_dae_with_diagnostics, simulate_with_step_records,
};

/// A stiff-ish decay, so the integrator has a reason to vary its step.
///
/// A constant-slope model would let a BDF method take one step size for the
/// whole run and the step-size assertions below would pass without the solver
/// having made a single decision.
const DECAY: &str = r"
model Decay
  Real x(start = 1, fixed = true);
equation
  der(x) = -40 * x;
end Decay;
";

fn options() -> SimOptions {
    SimOptions {
        t_end: 1.0,
        dt: Some(0.05),
        rtol: 1e-8,
        atol: 1e-8,
        solver_mode: SimSolverMode::Bdf,
        ..Default::default()
    }
}

#[test]
fn the_accepted_steps_are_reported_and_describe_a_real_solve() {
    let compiled = Compiler::new()
        .model("Decay")
        .compile_str(DECAY, "decay.mo")
        .expect("the fixture compiles");

    let (result, steps) = simulate_with_step_records(&compiled.dae, &options())
        .expect("the BDF backend simulates this model");

    assert!(
        !steps.is_empty(),
        "a run over [0, 1] cannot have taken no accepted steps; an empty list \
         would mean the seam is installed and never reached",
    );

    // **Contiguous and forward.** Each step starts where the last one stopped,
    // which is what makes the sequence a partition of the interval rather than
    // a bag of samples. A gap would mean a step went unreported.
    for pair in steps.windows(2) {
        let [previous, next] = pair else { continue };
        assert!(
            (next.start_time - previous.accepted_time).abs() < 1e-12,
            "step starting at {} does not continue from the previous accepted \
             time {}",
            next.start_time,
            previous.accepted_time,
        );
    }

    for step in &steps {
        assert!(
            step.step_size() > 0.0,
            "an accepted step must cover a positive interval: {step:?}",
        );
        assert!(
            step.order >= 1,
            "a continuous extension has order at least 1: {step:?}",
        );
    }

    // The steps span the run the trajectory reports.
    let first = steps.first().expect("non-empty");
    let last = steps.last().expect("non-empty");
    let t_end = *result.times.last().expect("the run produced samples");
    assert!(
        first.start_time <= 1e-12,
        "the first step starts at the initial time, not inside the run: {}",
        first.start_time,
    );
    assert!(
        last.accepted_time >= t_end - 1e-9,
        "the accepted steps must reach at least as far as the last output \
         sample ({t_end}), or the trajectory was sampled past where the solver \
         integrated: {}",
        last.accepted_time,
    );
}

/// **Observing changes nothing.** The whole claim of an observation-only seam.
///
/// Compared on the trajectory rather than on a summary: the same sample times
/// and the same values, to the bit. A run that merely *looked* similar would
/// satisfy a tolerance comparison and hide exactly the kind of interference
/// this test exists to rule out.
#[test]
fn an_observed_run_is_the_same_run() {
    let compiled = Compiler::new()
        .model("Decay")
        .compile_str(DECAY, "decay.mo")
        .expect("the fixture compiles");

    let plain = simulate_dae_with_diagnostics(&compiled.dae, &options())
        .expect("the unobserved run simulates");
    let (observed, steps) =
        simulate_with_step_records(&compiled.dae, &options()).expect("the observed run simulates");

    assert!(
        !steps.is_empty(),
        "the observed run must actually have been observed, or this test \
         compares two unobserved runs and proves nothing",
    );
    assert_eq!(
        observed.times, plain.times,
        "observation must not move a sample time",
    );
    assert_eq!(
        observed.names, plain.names,
        "observation must not change which variables are reported",
    );
    assert_eq!(
        observed.data, plain.data,
        "observation must not change a single value",
    );
}
