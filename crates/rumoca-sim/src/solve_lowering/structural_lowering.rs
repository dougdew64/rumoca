//! The shared structural-preparation and elimination funnel that both the
//! simulation lowering and the `--inspect structure` report run through, so the
//! report and the simulator always agree on the matched system.

use rumoca_ir_dae as dae;
use rumoca_phase_structural::dae_prepare::{
    FunnelStepFrame, FunnelStepOutcome, IndexReductionFrame, dae_shape, emit_funnel_step,
};
use rumoca_solver::SimOptions;

use super::expr_util::{
    debug_render_expr, equation_lhs_prefix, remove_duplicate_continuous_equations,
};
use super::timing::{log_solve_lowering_done, log_solve_lowering_start, stage_timer_start};

/// Shared structural preparation run before both the simulation lowering and the
/// `--inspect structure` report: scalarize (when requested), demote pseudo-states
/// and reduce index, eliminate derivative aliases, and rewrite standalone
/// `der(state)` references in non-ODE rows (`y = der(x)` → `y = <x's ODE rhs>`).
///
/// Keeping this in one place ensures the structural report and the simulator
/// agree on the matched system, and that fixes apply to both paths at once.
pub(super) fn prepare_dae_for_structural_analysis(
    lowered: &mut dae::Dae,
    opts: &SimOptions,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    prepare_dae_for_structural_analysis_observed(lowered, opts, None)
}

/// Run one funnel step, reporting what it did.
///
/// Wraps the timing logs the funnel already emitted, so a step is named once rather
/// than three times and the sequence below reads as a list of steps instead of a list
/// of log calls.
///
/// `label` is the timing label (`"prepare.<step>"`); the frame carries the step name
/// with that prefix removed, because the prefix says which *funnel* this is and every
/// step here is in the same one.
fn observed_step<E>(
    lowered: &mut dae::Dae,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
    label: &'static str,
    run: impl FnOnce(&mut dae::Dae) -> Result<FunnelStepOutcome, E>,
) -> Result<(), E> {
    let step = label.strip_prefix("prepare.").unwrap_or(label);
    let (states_before, equations_before) = dae_shape(lowered);

    log_solve_lowering_start(label);
    let timer = stage_timer_start();
    let result = run(lowered);
    log_solve_lowering_done(label, timer);

    let (states_after, equations_after) = dae_shape(lowered);
    // **A failing step is reported before its error propagates.** Otherwise the funnel
    // stops with one error at the top and no indication which of ten steps produced
    // it — which is exactly the case a diagnostic trace exists for.
    let outcome = match &result {
        Ok(outcome) => outcome.clone(),
        Err(_) => FunnelStepOutcome::Failed(format!("{step} returned an error")),
    };
    emit_funnel_step(
        observer,
        FunnelStepFrame {
            step,
            states_before,
            states_after,
            equations_before,
            equations_after,
            outcome,
        },
    );
    result.map(|_| ())
}

/// The funnel above, reporting each step to `observer` as it runs.
///
/// # Why this exists
///
/// The funnel's *result* has always been observable and its *process* has not: a
/// caller sees the DAE before and after and must infer which of ten steps did what.
/// **Inference across that gap has already produced a wrong conclusion** — a consumer
/// reading the final DAE reported that a model performed zero differentiations,
/// because the differentiated rows had been removed by a later elimination step.
///
/// It also gives a *failing* funnel a location. Today an error surfaces once, at the
/// top, naming no step.
///
/// # Contract
///
/// **Additive and semantics-preserving.** [`prepare_dae_for_structural_analysis`] now
/// delegates here with `None`, so there is one implementation rather than two that can
/// drift — which matters more than it sounds, because the step *order* is itself the
/// thing consumers most need and most easily get wrong when they reproduce it.
///
/// `None` costs a branch per step. See [`rumoca_core::FrameObserver`].
pub fn prepare_dae_for_structural_analysis_observed(
    lowered: &mut dae::Dae,
    opts: &SimOptions,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    prepare_dae_for_structural_analysis_fully_observed(lowered, opts, observer, None)
}

/// The funnel, reporting **both** levels: each step, and the inside of the two
/// index-reduction passes.
///
/// Two observers rather than one because they answer different questions and have very
/// different volumes. `observer` fires once per step — nine or ten frames. `inner`
/// fires per candidate, per differentiation and per demotion, which on a large model is
/// hundreds. A consumer wanting only the shape of the funnel should not pay for the
/// second, and one wanting the animation should not have to re-run the phase to get it.
///
/// **Supplying `inner` routes the two reduction passes through their `_with_trace`
/// variants.** Those are parallel implementations of the same algorithm, so
/// `the_traced_and_untraced_reduction_agree` pins them to the same result — without it
/// this parameter could quietly change what the compiler computes, which is the one
/// thing an observation API may never do.
pub fn prepare_dae_for_structural_analysis_fully_observed(
    lowered: &mut dae::Dae,
    opts: &SimOptions,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
    inner: Option<rumoca_core::FrameObserver<'_, IndexReductionFrame>>,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    scalarize_and_demote_pseudo_states(lowered, opts, observer)?;
    reduce_index_and_eliminate_aliases(lowered, observer, inner)?;
    rewrite_derivative_references(lowered, observer)?;
    trace_prepared_equations(lowered);
    Ok(())
}

/// Scalarize (when asked) and demote the states that were never states.
///
/// Split from the funnel body for `too_many_lines`, at a seam the sequence already
/// had: everything here removes *spurious* states — aliases and directly assigned
/// variables — before any index reduction is attempted. Doing it first is what keeps
/// the reduction pass from working on states that should not exist.
fn scalarize_and_demote_pseudo_states(
    lowered: &mut dae::Dae,
    opts: &SimOptions,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    if opts.scalarize {
        observed_step(lowered, observer, "prepare.scalarize_equations", |d| {
            rumoca_phase_structural::scalarize::scalarize_equations(d)
                .map(|()| FunnelStepOutcome::Completed)
                .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        })?;
    }
    observed_step(
        lowered,
        observer,
        "prepare.demote_exact_alias_component_states",
        |d| {
            rumoca_phase_structural::dae_prepare::demote_exact_alias_component_states(d)
                .map(FunnelStepOutcome::Demoted)
                .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    observed_step(
        lowered,
        observer,
        "prepare.demote_direct_assigned_states",
        |d| {
            rumoca_phase_structural::dae_prepare::demote_direct_assigned_states(d)
                .map(FunnelStepOutcome::Demoted)
                .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    Ok(())
}

/// Reduce the index, then remove the derivative aliases it leaves behind.
///
/// **The two reduction passes here have per-step tracing of their own**
/// (`IndexReductionFrame`: every candidate considered, every constraint
/// differentiated, every state demoted). When `inner` is supplied they run through
/// their `_with_trace` variants so that detail reaches the caller from **this** run —
/// the alternative being that a consumer re-runs them itself to see inside, which is
/// a second execution presented as the first.
fn reduce_index_and_eliminate_aliases(
    lowered: &mut dae::Dae,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
    inner: Option<rumoca_core::FrameObserver<'_, IndexReductionFrame>>,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    // Shared across both passes: the second continues the first's rounds and its list
    // of what has been demoted so far, which is what makes the two read as one replay.
    let mut frames: Vec<IndexReductionFrame> = Vec::new();
    let mut demoted_so_far: Vec<String> = Vec::new();

    if inner.is_some() {
        rumoca_phase_structural::dae_prepare::emit_index_reduction_start(
            &mut frames,
            inner,
            lowered,
            &demoted_so_far,
        );
    }

    observed_step(
        lowered,
        observer,
        "prepare.reduce_constrained_dummy_derivatives",
        |d| {
            match inner {
                Some(_) => rumoca_phase_structural::dae_prepare::
                    reduce_constrained_dummy_derivatives_with_trace(
                        d,
                        inner,
                        &mut frames,
                        &mut demoted_so_far,
                    ),
                None => {
                    rumoca_phase_structural::dae_prepare::reduce_constrained_dummy_derivatives(d)
                }
            }
            .map(FunnelStepOutcome::Demoted)
            .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    let round_offset = frames.last().map_or(0, |f| f.round + 1);
    observed_step(
        lowered,
        observer,
        "prepare.index_reduce_missing_state_derivatives",
        |d| {
            match inner {
                Some(_) => rumoca_phase_structural::dae_prepare::
                    index_reduce_missing_state_derivatives_with_trace(
                        d,
                        inner,
                        &mut frames,
                        &demoted_so_far,
                        round_offset,
                    ),
                None => {
                    rumoca_phase_structural::dae_prepare::index_reduce_missing_state_derivatives(d)
                }
            }
            .map(FunnelStepOutcome::Demoted)
            .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    observed_step::<rumoca_phase_solve::SolveModelLowerError>(
        lowered,
        observer,
        "prepare.demote_states_without_assignable_derivative_rows",
        |d| {
            let n = rumoca_phase_structural::dae_prepare::demote_states_without_assignable_derivative_rows(
                d,
            );
            Ok(FunnelStepOutcome::Demoted(n))
        },
    )?;
    observed_step(
        lowered,
        observer,
        "prepare.eliminate_derivative_aliases",
        |d| {
            rumoca_phase_structural::dae_prepare::eliminate_derivative_aliases(d)
                .map(|()| FunnelStepOutcome::Completed)
                .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    observed_step(
        lowered,
        observer,
        "prepare.demote_states_without_retained_derivative_rows",
        |d| {
            rumoca_phase_structural::dae_prepare::demote_states_without_retained_derivative_rows(d)
                // This step reports two categories — states with no derivative
                // reference at all, and states whose derivative row is unassignable —
                // and the frame carries one number. Summed rather than dropped: the
                // frame answers "how many did this step demote", and the split stays
                // available from the step's own return value to a direct caller.
                .map(|(no_derivative_refs, unassignable)| {
                    FunnelStepOutcome::Demoted(no_derivative_refs + unassignable)
                })
                .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })
        },
    )?;
    Ok(())
}

/// Rewrite the derivative references that demotion leaves behind.
///
/// **Order is load-bearing and this is why it is its own function**: both steps here
/// must run *after* demotion, and the comments below are the reasons.
fn rewrite_derivative_references(
    lowered: &mut dae::Dae,
    observer: Option<rumoca_core::FrameObserver<'_, FunnelStepFrame>>,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    // After demotion, any `der(<algebraic>)` (a differentiated algebraic such as
    // `a_rel = der(w_rel)`, or successive `Der` blocks) is expanded symbolically
    // via the chain rule, leaving only `der(state)`. Running this after demotion
    // is essential: a `der`'d algebraic with its own defining equation is first
    // demoted from a spurious state, then its derivative is expanded here rather
    // than left as an orphan column (which the matcher reports as singular).
    observed_step::<rumoca_phase_solve::SolveModelLowerError>(
        lowered,
        observer,
        "prepare.expand_compound_derivatives",
        |d| {
            rumoca_phase_structural::dae_prepare::expand_compound_derivatives(d);
            Ok(FunnelStepOutcome::Completed)
        },
    )?;
    // Rewrite `y = der(x)` (e.g. a `Modelica.Blocks.Continuous.Der` block reading
    // a state derivative) into `y = <x's ODE rhs>` so `y` is matchable. Without
    // this, the standalone `der(x)` reference in a non-ODE row has no column to
    // match and the system reports a spurious structural singularity.
    observed_step::<rumoca_phase_solve::SolveModelLowerError>(
        lowered,
        observer,
        "prepare.substitute_standalone_state_derivatives_in_non_ode_rows",
        |d| {
            let n = rumoca_phase_structural::dae_prepare::substitute_standalone_state_derivatives_in_non_ode_rows(d);
            Ok(FunnelStepOutcome::Rewrote(n))
        },
    )?;
    Ok(())
}

/// Dump every prepared equation at DEBUG, unchanged from before the funnel gained an
/// observer — this is the pre-existing `tracing` path and is not part of the frame API.
fn trace_prepared_equations(lowered: &dae::Dae) {
    if tracing::enabled!(target: "rumoca_phase_structural", tracing::Level::DEBUG) {
        for (index, eq) in lowered.continuous.equations.iter().enumerate() {
            let summary = format!("{}{}", equation_lhs_prefix(eq), debug_render_expr(&eq.rhs));
            tracing::debug!(
                target: "rumoca_phase_structural",
                "[sim-trace] prepared f_x[{index}] origin='{}' {}",
                eq.origin,
                summary
            );
        }
    }
}

pub(super) struct StructurallyLoweredDae {
    pub(super) dae: dae::Dae,
    pub(super) metadata_dae: dae::Dae,
    pub(super) visible_expressions: Vec<rumoca_phase_solve::VisibleExpression>,
}

struct PreparedStructuralDaes {
    source_dae: dae::Dae,
    lowered: dae::Dae,
    metadata_dae: dae::Dae,
}

pub(super) fn structurally_lower_dae_for_simulation(
    dae_model: &dae::Dae,
    opts: &SimOptions,
) -> Result<StructurallyLoweredDae, rumoca_phase_solve::SolveModelLowerError> {
    let PreparedStructuralDaes {
        source_dae,
        mut lowered,
        mut metadata_dae,
    } = prepare_structural_daes(dae_model, opts)?;

    log_solve_lowering_start("structural.eliminate_trivial");
    let timer = stage_timer_start();
    let elimination = rumoca_phase_structural::eliminate::eliminate_trivial(&mut lowered)
        .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
    log_solve_lowering_done("structural.eliminate_trivial", timer);
    if let Some(source) = elimination.blt_error {
        if dae_model.variables.states.is_empty() {
            validate_residual_shapes_for_simulation(dae_model)?;
        }
        return Err(rumoca_phase_solve::SolveModelLowerError::Structural { source });
    }

    apply_simulation_elimination(&mut lowered, &elimination.substitutions)?;
    trace_simulation_elimination(&lowered, &elimination.substitutions);
    mark_state_selection_metadata(&mut metadata_dae, &elimination.substitutions)?;
    let visible_expressions =
        visible_expressions_after_elimination(&source_dae, &elimination.substitutions, opts)?;

    Ok(StructurallyLoweredDae {
        dae: lowered,
        metadata_dae,
        visible_expressions,
    })
}

fn prepare_structural_daes(
    dae_model: &dae::Dae,
    opts: &SimOptions,
) -> Result<PreparedStructuralDaes, rumoca_phase_solve::SolveModelLowerError> {
    log_solve_lowering_start("structural.attach_dae_reference_metadata");
    let timer = stage_timer_start();
    let mut source_dae = dae_model.clone();
    rumoca_phase_dae::attach_dae_reference_metadata(&mut source_dae)
        .map_err(metadata_attachment_lower_error)?;
    log_solve_lowering_done("structural.attach_dae_reference_metadata", timer);
    log_solve_lowering_start("structural.clone_source_for_lowered");
    let timer = stage_timer_start();
    let mut lowered = source_dae.clone();
    log_solve_lowering_done("structural.clone_source_for_lowered", timer);
    prepare_dae_for_structural_analysis(&mut lowered, opts)?;
    log_solve_lowering_start("structural.remove_duplicate_continuous_equations");
    let timer = stage_timer_start();
    remove_duplicate_continuous_equations(&mut lowered);
    log_solve_lowering_done("structural.remove_duplicate_continuous_equations", timer);
    log_solve_lowering_start("structural.clone_metadata_dae");
    let timer = stage_timer_start();
    let metadata_dae = lowered.clone();
    log_solve_lowering_done("structural.clone_metadata_dae", timer);

    Ok(PreparedStructuralDaes {
        source_dae,
        lowered,
        metadata_dae,
    })
}

fn apply_simulation_elimination(
    lowered: &mut dae::Dae,
    substitutions: &[rumoca_phase_structural::eliminate::Substitution],
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    log_solve_lowering_start("structural.demote_states_without_retained_derivative_rows");
    let timer = stage_timer_start();
    rumoca_phase_structural::dae_prepare::demote_states_without_retained_derivative_rows(lowered)
        .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
    log_solve_lowering_done(
        "structural.demote_states_without_retained_derivative_rows",
        timer,
    );
    log_solve_lowering_start("structural.apply_elimination_substitutions_to_dae");
    let timer = stage_timer_start();
    rumoca_phase_structural::eliminate::apply_elimination_substitutions_to_dae(
        lowered,
        substitutions,
    )
    .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
    log_solve_lowering_done("structural.apply_elimination_substitutions_to_dae", timer);
    Ok(())
}

fn trace_simulation_elimination(
    lowered: &dae::Dae,
    substitutions: &[rumoca_phase_structural::eliminate::Substitution],
) {
    if tracing::enabled!(target: "rumoca_phase_structural", tracing::Level::DEBUG) {
        for sub in substitutions {
            tracing::debug!(
                target: "rumoca_phase_structural",
                "[sim-trace] substitution {} := {}",
                sub.var_name.as_str(),
                debug_render_expr(&sub.expr)
            );
        }
        for (index, eq) in lowered.continuous.equations.iter().enumerate() {
            tracing::debug!(
                target: "rumoca_phase_structural",
                "[sim-trace] post-elim f_x[{index}] origin='{}' {}{}",
                eq.origin,
                equation_lhs_prefix(eq),
                debug_render_expr(&eq.rhs)
            );
        }
    }
}

fn mark_state_selection_metadata(
    metadata_dae: &mut dae::Dae,
    substitutions: &[rumoca_phase_structural::eliminate::Substitution],
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    log_solve_lowering_start("structural.clone_state_selection_dae");
    let timer = stage_timer_start();
    let mut state_selection_dae = metadata_dae.clone();
    log_solve_lowering_done("structural.clone_state_selection_dae", timer);
    log_solve_lowering_start("structural.apply_state_selection_substitutions");
    let timer = stage_timer_start();
    rumoca_phase_structural::eliminate::apply_elimination_substitutions_to_dae(
        &mut state_selection_dae,
        substitutions,
    )
    .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
    log_solve_lowering_done("structural.apply_state_selection_substitutions", timer);
    log_solve_lowering_start("structural.demote_state_selection_dae");
    let timer = stage_timer_start();
    rumoca_phase_structural::dae_prepare::demote_states_without_retained_derivative_rows(
        &mut state_selection_dae,
    )
    .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
    log_solve_lowering_done("structural.demote_state_selection_dae", timer);
    log_solve_lowering_start("structural.mark_constrained_dummy_states_in_metadata");
    let timer = stage_timer_start();
    mark_constrained_dummy_states_in_metadata(&state_selection_dae, metadata_dae);
    log_solve_lowering_done(
        "structural.mark_constrained_dummy_states_in_metadata",
        timer,
    );
    Ok(())
}

fn visible_expressions_after_elimination(
    source_dae: &dae::Dae,
    substitutions: &[rumoca_phase_structural::eliminate::Substitution],
    opts: &SimOptions,
) -> Result<Vec<rumoca_phase_solve::VisibleExpression>, rumoca_phase_solve::SolveModelLowerError> {
    log_solve_lowering_start("structural.clone_observation_dae");
    let timer = stage_timer_start();
    let mut observation_dae = source_dae.clone();
    log_solve_lowering_done("structural.clone_observation_dae", timer);
    if opts.scalarize {
        log_solve_lowering_start("structural.scalarize_observation_dae");
        let timer = stage_timer_start();
        rumoca_phase_structural::scalarize::scalarize_equations(&mut observation_dae)
            .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
        log_solve_lowering_done("structural.scalarize_observation_dae", timer);
    }
    if !substitutions.is_empty() {
        log_solve_lowering_start("structural.resolve_observation_substitutions");
        let timer = stage_timer_start();
        for eq in &mut observation_dae.continuous.equations {
            eq.rhs = rumoca_phase_structural::eliminate::resolve_substitutions_in_expr(
                &eq.rhs,
                substitutions,
            )
            .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
        }
        log_solve_lowering_done("structural.resolve_observation_substitutions", timer);
    }
    log_solve_lowering_start("structural.visible_expressions_for_dae");
    let timer = stage_timer_start();
    let mut visible_expressions = rumoca_phase_solve::visible_expressions_for_dae(&observation_dae)
        .map_err(rumoca_phase_solve::SolveModelLowerError::Lower)?;
    log_solve_lowering_done("structural.visible_expressions_for_dae", timer);
    if !substitutions.is_empty() {
        log_solve_lowering_start("structural.resolve_visible_expression_substitutions");
        let timer = stage_timer_start();
        for visible in &mut visible_expressions {
            visible.expr = rumoca_phase_structural::eliminate::resolve_substitutions_in_expr(
                &visible.expr,
                substitutions,
            )
            .map_err(|source| rumoca_phase_solve::SolveModelLowerError::Structural { source })?;
        }
        log_solve_lowering_done("structural.resolve_visible_expression_substitutions", timer);
    }
    Ok(visible_expressions)
}

pub(super) fn metadata_attachment_lower_error(
    err: rumoca_phase_dae::ToDaeError,
) -> rumoca_phase_solve::SolveModelLowerError {
    let reason = format!("DAE reference metadata attachment failed: {err}");
    rumoca_phase_solve::SolveModelLowerError::Lower(lower_contract_error_from_optional_span(
        reason,
        err.source_span(),
    ))
}

fn lower_contract_error_from_optional_span(
    reason: String,
    span: Option<rumoca_core::Span>,
) -> rumoca_phase_solve::lower::LowerError {
    match span {
        Some(span) if !span.is_dummy() => {
            rumoca_phase_solve::lower::LowerError::ContractViolation { reason, span }
        }
        Some(_) | None => {
            rumoca_phase_solve::lower::LowerError::UnspannedContractViolation { reason }
        }
    }
}

fn validate_residual_shapes_for_simulation(
    dae_model: &dae::Dae,
) -> Result<(), rumoca_phase_solve::SolveModelLowerError> {
    let layout = rumoca_phase_solve::build_var_layout(dae_model)?;
    rumoca_phase_solve::lower::lower_residual(dae_model, &layout)?;
    Ok(())
}

fn mark_constrained_dummy_states_in_metadata(
    structural_dae: &dae::Dae,
    metadata_dae: &mut dae::Dae,
) {
    for state_name in
        rumoca_phase_structural::dae_prepare::constrained_dummy_state_names(structural_dae)
    {
        let name = rumoca_core::VarName::new(state_name);
        if let Some(var) = metadata_dae.variables.states.shift_remove(&name) {
            metadata_dae.variables.algebraics.insert(name, var);
        }
    }
}
