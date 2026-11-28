//! **The one real preparation a compile shows** — every tab from Simplify to Solve
//! lowering is read from it, and the Simulation tab integrates its artifact.
//!
//! # Why this replaced HRW's own calls (2026-11-26)
//!
//! Until this date the Structural, Index reduction and Initialization tabs were
//! built from HRW calling `inspect_structural_report` and
//! `inspect_prepare_for_solve` on the DAE as compiled, the Solve lowering tab from a
//! separate `lower_solve_problem`, and the simulation lowered the model a third time
//! inside `rumoca-sim`. None of those was the preparation Rumoca simulates: solve
//! lowering first folds evaluable parameters, inlines annotated calls, quotients
//! aliases and folds constants, and only then analyses and reduces — so for any
//! model a transform changes, the tabs showed a different execution, and an
//! Index reduction reconcile had to patch its outcome to agree with lowering.
//!
//! Now one call — [`rumoca_sim::prepare_runtime_observed`], the lowering half of
//! the very entry the simulation used — runs under both preparation capture
//! scopes (`rumoca_phase_structural::preparation_trace` and
//! `rumoca_phase_solve::preparation_trace`), and its observer reads the lowered
//! model. Every tab below is a reading of that one run, and the runtime it
//! returns is the one Simulation integrates.

use rumoca_phase_solve::preparation_trace::{
    self as solve_trace, DaeShape, SelectionRoute, SolvePreparationEvent, SourceTransform,
};
use rumoca_phase_structural::preparation_trace::{
    self as structural_trace, AnalysisContext, CapturedIncidence, PreparationEvent,
};

use super::{
    Stage, StructuralFrames, reduction_lanes_used, reduction_outcome_summary,
    reduction_record_to_json, span_location, structural_error_to_json, structural_to_json,
};

/// Everything one real preparation produced, owned.
pub(super) struct RealPreparation {
    /// Every structural analysis, reduction record and alias quotient, in order.
    pub structural: Vec<PreparationEvent>,
    /// Every source transform, routing decision and the final selection, in order.
    pub solve: Vec<SolvePreparationEvent>,
    /// What the lowered model said, read inside the observer. `None` when
    /// preparation failed before a model was lowered.
    pub lowered: Option<LoweredFacts>,
    /// The runtime the simulation integrates, or why there is none.
    pub runtime: Result<rumoca_sim::PreparedRuntime, String>,
}

/// The lowered model, read while it was still borrowed by the observer.
pub(super) struct LoweredFacts {
    /// The executable `SolveProblem`, serialized.
    pub problem: Result<serde_json::Value, String>,
    /// The prepared system lowering consumed, in the Structural report's shape.
    pub after: serde_json::Value,
    /// The initialization plan Rumoca lowered, row by row.
    pub initialization: serde_json::Value,
    /// `after["blocks"]` before the run's tearing was attached to it, so the
    /// Structural tab can tell whether its own blocks are the same system.
    pub after_blocks_bare: serde_json::Value,
    /// Every multi-row block of the run's algebraic projection plan.
    pub run_tearings: Vec<RunTearing>,
}

/// One coupled block **as the run solves it**: a block of solve lowering's
/// `algebraic_projection_plan`, the plan the simulation's Newton iterates on.
///
/// **Why the Structural tab's tearing is not enough** (2026-11-28). The
/// structural phase proposes a tearing, and solve lowering re-tears any block
/// whose causal steps it cannot prove safe (`proven_tearing::retear_unproven_blocks`).
/// `ProportionalLoop`'s structural plan solves `error` from
/// `command = controllerGain*error`, which divides by a parameter, so the run
/// tears `error` instead of `command`. Showing only the structural plan taught
/// a tearing the simulation never uses.
///
/// Everything here is Rumoca's own: rows are `EquationRef`s labelled by
/// `equation_label` on the prepared system the plan was lowered from, and
/// unknowns are named by `SolveProblem::solver_name`.
#[derive(Clone)]
pub(super) struct RunTearing {
    /// The block's rows as `EquationRef` displays (`f_x[k]`), sorted — the key a
    /// report block is paired by, exactly and never by label text.
    ids: Vec<String>,
    json: serde_json::Value,
}

/// Run Rumoca's real simulation preparation of `dae` under both capture scopes.
pub(super) fn observe(dae: &rumoca_ir_dae::Dae, opts: &rumoca_sim::SimOptions) -> RealPreparation {
    // HRW's own renderer, so captured rows read like the DAE tab's equations. It
    // runs on each analysed system while that system exists; see
    // `set_equation_renderer` for why Rumoca cannot do this itself.
    structural_trace::set_equation_renderer(Some(Box::new(|view, index| {
        view.continuous_equation(index)
            .map(|equation| crate::dae_expr_format::format_equation(view, equation))
    })));
    structural_trace::start_capture();
    solve_trace::start_capture();
    let source_map = dae.source_map();
    let mut lowered = None;
    let runtime = rumoca_sim::prepare_runtime_observed(dae, opts, |model| {
        lowered = Some(lowered_facts(model, source_map));
    })
    .map_err(|error| error.to_string());
    let solve = solve_trace::take_capture();
    let structural = structural_trace::take_capture();
    structural_trace::set_equation_renderer(None);
    RealPreparation {
        structural,
        solve,
        lowered,
        runtime,
    }
}

fn lowered_facts(
    model: &rumoca_phase_solve::LoweredSolveModel<'_>,
    source_map: &rumoca_core::SourceMap,
) -> LoweredFacts {
    let problem = &model.model().problem;
    let mut after = prepared_system_json(model.prepared(), model.model().state_scalar_count());
    let after_blocks_bare = after["blocks"].clone();
    let run_tearings = run_tearings(model);
    attach_run_tearing(&mut after["blocks"], &run_tearings);
    LoweredFacts {
        problem: serde_json::to_value(problem).map_err(|error| error.to_string()),
        after,
        initialization: initialization_plan_json(problem, source_map),
        after_blocks_bare,
        run_tearings,
    }
}

/// Read every multi-row block of the run's algebraic projection plan.
fn run_tearings(model: &rumoca_phase_solve::LoweredSolveModel<'_>) -> Vec<RunTearing> {
    use rumoca_phase_structural::{EquationRef, equation_label};
    let problem = &model.model().problem;
    // A private slot has no display name; say which slot rather than invent one.
    let name = |y: usize| {
        problem
            .solver_name(y)
            .unwrap_or_else(|| format!("<solver Y[{y}], unnamed>"))
    };
    model.prepared().inspect(|system| {
        let view = system.view;
        let label = |row: usize| equation_label(view, &EquationRef(row));
        let id = |row: usize| EquationRef(row).to_string();
        problem
            .continuous
            .algebraic_projection_plan
            .blocks
            .explicit_blocks()
            .filter(|block| block.rows.len() > 1)
            .map(|block| {
                let mut ids: Vec<String> = block.rows.iter().map(|&row| id(row)).collect();
                ids.sort();
                let json = match &block.tearing {
                    Some(tearing) => serde_json::json!({
                        "tear_vars": tearing.tear_y_indices.iter().map(|&y| name(y)).collect::<Vec<_>>(),
                        "residual_equations": tearing.residual_rows.iter().map(|&r| label(r)).collect::<Vec<_>>(),
                        "residual_equation_ids": tearing.residual_rows.iter().map(|&r| id(r)).collect::<Vec<_>>(),
                        "causal_sequence": tearing.causal_steps.iter().map(|step| serde_json::json!({
                            "equation": label(step.row),
                            "id": id(step.row),
                            "variable": name(step.y_index),
                            // Rumoca's proof that the step's divisor is nonzero:
                            // `Unit`, `Literal`, or `Unproven` (no proof).
                            "coefficient": format!("{:?}", step.coefficient),
                        })).collect::<Vec<_>>(),
                    }),
                    None => serde_json::json!({ "dense": true }),
                };
                RunTearing { ids, json }
            })
            .collect()
    })
}

/// Pair each coupled report block with the run's block of exactly the same
/// equations, and say whether the run tears it the way the report does.
///
/// Paired by the sorted set of `EquationRef`s, which is an identity; a block
/// the plan has no exact match for says so rather than borrowing a near one.
fn attach_run_tearing(blocks: &mut serde_json::Value, runs: &[RunTearing]) {
    let Some(blocks) = blocks.as_array_mut() else {
        return;
    };
    for block in blocks.iter_mut().filter(|block| block["kind"] == "coupled") {
        let mut ids: Vec<String> = block["ids"]
            .as_array()
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        ids.sort();
        let Some(run) = runs.iter().find(|run| run.ids == ids) else {
            block["run_tearing"] = serde_json::json!({
                "absent": "no block of solve lowering's algebraic projection plan has exactly \
                           these equations, so the run does not solve them as this one loop",
            });
            continue;
        };
        let mut json = run.json.clone();
        let same = plans_agree(&block["tearing"], &json);
        json["same_as_structural"] = serde_json::json!(same);
        json["source"] = serde_json::json!(
            "solve lowering's continuous.algebraic_projection_plan: the tearing the \
             simulation's Newton iterates on"
        );
        if !same {
            json["note"] = serde_json::json!(run_tearing_note(&block["tearing"], &json));
        }
        block["run_tearing"] = json;
    }
}

/// The two plans agree when they tear the same unknowns against the same
/// residuals and take the same causal steps, in order.
fn plans_agree(structural: &serde_json::Value, run: &serde_json::Value) -> bool {
    let steps = |plan: &serde_json::Value| -> Vec<(serde_json::Value, serde_json::Value)> {
        plan["causal_sequence"]
            .as_array()
            .map(|steps| {
                steps
                    .iter()
                    .map(|step| (step["id"].clone(), step["variable"].clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    run.get("dense").is_none()
        && structural.is_object()
        && structural["tear_vars"] == run["tear_vars"]
        && structural["residual_equation_ids"] == run["residual_equation_ids"]
        && steps(structural) == steps(run)
}

/// Why the run's plan differs, in Rumoca's documented terms, plus the
/// structural steps the run does not take (a comparison, not an inference).
fn run_tearing_note(structural: &serde_json::Value, run: &serde_json::Value) -> String {
    if run.get("dense").is_some() {
        return "the run solves this block by a dense Newton over every unknown: its \
                projection plan carries no tearing"
            .to_owned();
    }
    let run_steps: Vec<(&serde_json::Value, &serde_json::Value)> = run["causal_sequence"]
        .as_array()
        .map(|steps| steps.iter().map(|s| (&s["id"], &s["variable"])).collect())
        .unwrap_or_default();
    let dropped: Vec<String> = structural["causal_sequence"]
        .as_array()
        .map(|steps| {
            steps
                .iter()
                .filter(|s| !run_steps.contains(&(&s["id"], &s["variable"])))
                .map(|s| {
                    format!(
                        "{} solving {}",
                        s["id"].as_str().unwrap_or("?"),
                        s["variable"].as_str().unwrap_or("?")
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    let mut note = "solve lowering re-tore this block (proven_tearing::retear_unproven_blocks): \
                    a causal step divides by the coefficient of the unknown it isolates, and \
                    Rumoca admits one only with a proof that the coefficient is nonzero \
                    (SPEC_0043 §4)"
        .to_owned();
    if !dropped.is_empty() {
        // A comparison of the two plans, not a list of the steps that lacked a
        // proof: once one step is replaced, the rest of the plan can change too.
        note.push_str(&format!(
            "; of the structural plan's steps, the run's plan does not contain {}",
            dropped.join(", ")
        ));
    }
    let any_unproven = run["causal_sequence"]
        .as_array()
        .is_some_and(|steps| steps.iter().any(|s| s["coefficient"] == "Unproven"));
    if any_unproven {
        note.push_str(
            "; a step still marked Unproven is promoted to a tear when the refresh plan \
             is built (proven_tearing.rs)",
        );
    }
    note
}

/// The prepared system, in the same shape as the Structural report so one reader
/// parses either side of the Index reduction split view.
///
/// **`n_states` is the lowered model's state scalar count** — what the solver
/// integrates. The prepared DAE's own count is of *declarations*, and a
/// formal-derivative basis is one declaration of several scalars
/// (`$state_coordinates`), so `Drivetrain` would read 1 where it integrates 3. The
/// declaration count is kept beside it as `n_state_declarations`.
fn prepared_system_json(
    prepared: &rumoca_phase_structural::PreparedDae<'_>,
    state_scalars: usize,
) -> serde_json::Value {
    let counts = crate::dae_counts::DaeCounts::of(prepared.as_dae());
    let mut json = prepared.inspect(|system| {
        if system.structural.is_some() {
            structural_to_json(&prepared.structural_report())
        } else {
            serde_json::json!({})
        }
    });
    prepared.inspect(|system| {
        // A projection of the prepared system, not a second analysis: the
        // incidence is the first step of an analysis and changes nothing.
        if let Ok(incidence) = rumoca_phase_structural::build_incidence(system.view) {
            json["incidence"] = super::incidence_to_json(&incidence, Some(system.view));
        }
        json["n_manifold_constraints"] = serde_json::json!(system.manifold.len());
    });
    json["n_states"] = serde_json::json!(state_scalars);
    json["n_state_declarations"] = serde_json::json!(counts.states);
    json["n_algebraics"] = serde_json::json!(counts.algebraics);
    json["n_equations"] = serde_json::json!(counts.continuous_scalar_rows);
    json
}

/// Rumoca's lowered initialization plan: what each residual row does.
fn initialization_plan_json(
    problem: &rumoca_ir_solve::SolveProblem,
    source_map: &rumoca_core::SourceMap,
) -> serde_json::Value {
    let system = &problem.initialization;
    let name = |slot: &rumoca_ir_solve::ScalarSlot| problem.layout.scalar_name_for_slot(*slot);
    let spans = row_spans(system.residual());
    let mut tally = std::collections::BTreeMap::<String, usize>::new();
    let rows: Vec<serde_json::Value> = system
        .row_roles()
        .iter()
        .enumerate()
        .map(|(row, role)| {
            let role = format!("{role:?}");
            *tally.entry(role.clone()).or_default() += 1;
            serde_json::json!({
                "row": row,
                "role": role,
                "solves": system.row_targets().get(row).copied().flatten().map(|slot| name(&slot)),
                "at": spans
                    .get(row)
                    .copied()
                    .flatten()
                    .and_then(|span| span_location(source_map, &span)),
            })
        })
        .collect();
    serde_json::json!({
        // A list rather than a map keyed by role, so a role name is a value and
        // never a key (field help routes keys).
        "summary": tally
            .iter()
            .map(|(role, count)| serde_json::json!({ "role": role, "rows": count }))
            .collect::<Vec<_>>(),
        "rows": rows,
        "unknowns": system.projection_unknowns().iter().map(name).collect::<Vec<_>>(),
        "given_states": system
            .given_state_indices()
            .iter()
            .map(|index| problem.solver_name(*index))
            .collect::<Vec<_>>(),
        "manifold_rows": system.manifold_row_count(),
        "roles_note":
            "Rumoca's own InitializationRowRole per residual row: Solved rows determine \
             an unknown; a SurplusCheck row reads only values something else fixed, so \
             it is a consistency check (MLS 3.6 \u{a7}8.6) that the run evaluates",
    })
}

/// The source span of each scalar row of `block`, where its nodes carry one.
fn row_spans(block: &rumoca_ir_solve::ComputeBlock) -> Vec<Option<rumoca_core::Span>> {
    let mut spans = Vec::new();
    for node in &block.nodes {
        match node {
            rumoca_ir_solve::ComputeNode::ScalarPrograms(programs) => {
                spans.extend(programs.program_spans().iter().copied().map(Some));
            }
            // Non-scalar nodes carry no per-row span here; stop rather than
            // misalign every later row with the wrong one.
            _ => break,
        }
    }
    spans
}

// ── Stages ────────────────────────────────────────────────────────────────────

/// Simplify: the four source transforms solve lowering runs before any
/// structural work, and the alias classes the quotient found.
pub(super) fn simplify_stage(prep: &RealPreparation) -> Stage {
    let transforms: Vec<serde_json::Value> = prep
        .solve
        .iter()
        .filter_map(|event| match event {
            SolvePreparationEvent::Transform {
                transform,
                before,
                after,
            } => Some(serde_json::json!({
                "transform": transform_name(*transform),
                "what_it_does": transform_meaning(*transform),
                "changed": after.is_some(),
                "before": shape_json(before),
                "after": after.as_ref().map(shape_json),
            })),
            _ => None,
        })
        .collect();
    let quotients: Vec<serde_json::Value> = prep
        .structural
        .iter()
        .filter_map(|event| match event {
            PreparationEvent::AliasQuotient { scope, report } => Some(serde_json::json!({
                "scope": format!("{scope:?}"),
                "eliminated": report.eliminated_count(),
                "classes": report.classes.iter().map(|class| format!("{class:?}")).collect::<Vec<_>>(),
            })),
            _ => None,
        })
        .collect();
    if transforms.is_empty() {
        return Stage::info(prep_failure_note(prep, "no source transform ran"));
    }
    let changed = transforms.iter().filter(|t| t["changed"] == true).count();
    let mut stage = Stage::ok(serde_json::json!({
        "transforms": transforms,
        "alias_quotients": quotients,
        "note": "solve lowering runs these four, in this order, on the DAE before any \
                 structural analysis; each reads the previous one's output",
    }));
    stage.note = Some(format!(
        "{changed} of {} transform(s) changed the system",
        transforms.len()
    ));
    stage
}

fn transform_name(transform: SourceTransform) -> &'static str {
    match transform {
        SourceTransform::FoldEvaluableParameters => "fold_evaluable_parameters",
        SourceTransform::InlineAnnotatedCalls => "inline_annotated_calls",
        SourceTransform::QuotientAliases => "quotient_aliases",
        SourceTransform::FoldConstantValues => "fold_constant_values",
    }
}

fn transform_meaning(transform: SourceTransform) -> &'static str {
    match transform {
        SourceTransform::FoldEvaluableParameters => {
            "replaces reads of evaluable parameters by their settled values"
        }
        SourceTransform::InlineAnnotatedCalls => {
            "inlines Inline / LateInline pure function calls before index reduction"
        }
        SourceTransform::QuotientAliases => {
            "merges copy and negation aliases (a = b, a = -b) onto one representative"
        }
        SourceTransform::FoldConstantValues => {
            "evaluates constant pure calls and propagates literal algebraics"
        }
    }
}

fn shape_json(shape: &DaeShape) -> serde_json::Value {
    serde_json::json!({
        "states": shape.states,
        "algebraics": shape.algebraics,
        "outputs": shape.outputs,
        "parameters": shape.parameters,
        "constants": shape.constants,
        "inputs": shape.inputs,
        "discrete": shape.discrete,
        "continuous_equations": shape.continuous_equations,
        "initialization_equations": shape.initialization_equations,
    })
}

/// The first analysis of the system preparation was given — the one the
/// reducer starts from.
fn source_analysis(prep: &RealPreparation) -> Option<&PreparationEvent> {
    prep.structural.iter().find(|event| {
        matches!(
            event,
            PreparationEvent::StructuralAnalysis {
                context: AnalysisContext::Source,
                ..
            }
        )
    })
}

/// Structural: the analysis solve lowering ran on the simplified system, with
/// its matching, BLT, tearing and the search frames that produced them.
pub(super) fn structural_stage(
    prep: &RealPreparation,
    source_map: &rumoca_core::SourceMap,
) -> (Stage, StructuralFrames) {
    let empty = StructuralFrames {
        matching: Vec::new(),
        tarjan: Vec::new(),
        tearing: Vec::new(),
    };
    let Some(PreparationEvent::StructuralAnalysis {
        incidence,
        outcome,
        trace,
        ..
    }) = source_analysis(prep)
    else {
        return (
            Stage::info(prep_failure_note(prep, "no structural analysis ran")),
            empty,
        );
    };
    let frames = StructuralFrames {
        matching: trace.matching().cloned().collect(),
        tarjan: trace
            .tarjan_runs()
            .into_iter()
            .map(|run| run.into_iter().cloned().collect())
            .collect(),
        // Per block, as every consumer addresses a block — see the note this
        // replaced in `worker.rs`'s history (`tearing_blocks`, not passes).
        tearing: trace
            .tearing_blocks()
            .into_iter()
            .map(|block| block.into_iter().cloned().collect())
            .collect(),
    };
    let mut json = match outcome {
        Ok(report) => structural_to_json(report),
        Err(_) => serde_json::json!({}),
    };
    if let Some(incidence) = incidence {
        json["incidence"] = captured_incidence_json(incidence);
    }
    json["analysed_system"] = serde_json::json!(
        "the DAE after the Simplify tab's transforms: this is the analysis solve \
         lowering ran first, and the one the reducer started from"
    );
    json["analyses"] = analyses_json(prep);
    // **The run's tearing, but only where it is provably this system's.** The
    // plan was lowered from the *prepared* system, whose equation numbering is
    // its own once index reduction changed anything. Its blocks being identical
    // to this report's is the proof that they are the same equations; otherwise
    // the run's tearing is shown where it belongs, on Index reduction's `after`.
    if let Some(lowered) = &prep.lowered {
        if json["blocks"] == lowered.after_blocks_bare {
            attach_run_tearing(&mut json["blocks"], &lowered.run_tearings);
        } else {
            json["run_tearing"] = serde_json::json!(
                "the run's tearing belongs to the prepared system, which index reduction \
                 changed, so it is not paired with these blocks: see Index reduction's \
                 `after` blocks"
            );
        }
    }
    let stage = match outcome {
        Ok(_) => Stage::ok(json),
        Err(error) => {
            if let (Some(incidence), Some(last)) = (incidence, trace.matching().last()) {
                json["matching"] = captured_partial_matching(incidence, &last.match_eq);
            }
            json["error"] = structural_error_to_json(error, source_map);
            let note = match error {
                rumoca_phase_structural::StructuralError::Singular { .. } => "singular".to_owned(),
                other => format!("{other}"),
            };
            Stage::recovered(json, note)
        }
    };
    (stage, frames)
}

/// Every structural analysis preparation ran, in order, and why.
fn analyses_json(prep: &RealPreparation) -> serde_json::Value {
    let analyses: Vec<serde_json::Value> = prep
        .structural
        .iter()
        .filter_map(|event| match event {
            PreparationEvent::StructuralAnalysis {
                context,
                outcome,
                incidence,
                ..
            } => Some(serde_json::json!({
                "context": format!("{context:?}"),
                "equations": incidence.as_ref().map(|i| i.rows.len()),
                "unknowns": incidence.as_ref().map(|i| i.unknowns.len()),
                "result": match outcome {
                    Ok(report) => format!(
                        "sorted: {} block(s), {} coupled",
                        report.blocks.len(),
                        report.coupled_block_count()
                    ),
                    Err(error) => format!("{error}"),
                },
            })),
            _ => None,
        })
        .collect();
    serde_json::Value::Array(analyses)
}

/// A [`CapturedIncidence`] in the shape `incidence_to_json` publishes, so every
/// incidence reader parses it unchanged.
fn captured_incidence_json(incidence: &CapturedIncidence) -> serde_json::Value {
    let rows: Vec<serde_json::Value> = incidence
        .rows
        .iter()
        .map(|row| {
            let mut json = serde_json::json!({
                "equation": row.label,
                "id": row.id,
                "unknowns": row.unknowns,
            });
            if let Some(text) = &row.text {
                json["equation_text"] = serde_json::json!(text);
            }
            json
        })
        .collect();
    serde_json::json!({
        "n_eq": incidence.rows.len(),
        "n_var": incidence.unknowns.len(),
        "unknown_names": incidence.unknowns,
        "rows": rows,
    })
}

/// The matching a failed analysis reached, read from its last recorded step.
fn captured_partial_matching(
    incidence: &CapturedIncidence,
    match_eq: &[Option<usize>],
) -> serde_json::Value {
    let pairs: Vec<serde_json::Value> = match_eq
        .iter()
        .enumerate()
        .filter_map(|(row, unknown)| {
            let (row, unknown) = (incidence.rows.get(row)?, (*unknown)?);
            Some(serde_json::json!({
                "equation": row.label,
                "id": row.id,
                "unknown": incidence.unknowns.get(unknown)?,
            }))
        })
        .collect();
    serde_json::Value::Array(pairs)
}

/// Index reduction: the reducer's records from the real run, the routing state
/// selection took, what it integrates, and the prepared system lowering consumed.
pub(super) fn index_reduction_stage(
    prep: &RealPreparation,
    structural: Option<&serde_json::Value>,
    source_map: &rumoca_core::SourceMap,
) -> Stage {
    use rumoca_phase_structural::ReductionRecord;
    let records: Vec<&ReductionRecord> = prep
        .structural
        .iter()
        .filter_map(|event| match event {
            PreparationEvent::Reduction(record) => Some(record),
            _ => None,
        })
        .collect();
    let routes: Vec<&'static str> = prep
        .solve
        .iter()
        .filter_map(|event| match event {
            SolvePreparationEvent::Route(route) => Some(route_name(route)),
            _ => None,
        })
        .collect();
    let selected = prep.solve.iter().find_map(|event| match event {
        SolvePreparationEvent::Selected {
            integrated,
            alternate_charts,
        } => Some((integrated, *alternate_charts)),
        _ => None,
    });
    if source_analysis(prep).is_none() {
        return Stage::info(prep_failure_note(prep, "preparation reached no reduction"));
    }
    let rounds = records
        .iter()
        .filter(|record| matches!(record, ReductionRecord::Round { .. }))
        .count();
    let owned: Vec<ReductionRecord> = records.iter().map(|record| (*record).clone()).collect();
    let admitted = prep.lowered.is_some();

    let mut before = structural.cloned().unwrap_or_else(|| serde_json::json!({}));
    if let Some(shape) = simplified_shape(prep) {
        before["n_states"] = serde_json::json!(shape.states);
        before["n_algebraics"] = serde_json::json!(shape.algebraics);
    }
    if let Some(rows) = before["incidence"]["n_eq"].as_u64() {
        before["n_equations"] = serde_json::json!(rows);
    }
    let after = prep
        .lowered
        .as_ref()
        .map_or(serde_json::Value::Null, |lowered| lowered.after.clone());
    let json = serde_json::json!({
        "rounds": rounds,
        "n_records": owned.len(),
        "outcome": reduction_outcome_summary(&owned, admitted, rounds, source_map),
        "lanes": reduction_lanes_used(&owned),
        "route": routes,
        "integrated": selected.map(|(names, _)| names.clone()),
        "alternate_charts": selected.map(|(_, charts)| charts),
        "residue_definition":
            "(n_equations - n_matched) + (n_unknowns - n_matched): both deficits \
             summed, so a square system's residue is twice its rank deficiency",
        "ordinals_note":
            "state_ordinal, rhs_ordinal, variable_ordinal and the holonomic ordinals \
             index the rebuilt system of their own round, not the raw DAE. They are \
             deliberately unnamed; resolve a candidate through declared_at, which is \
             a span and stable across rebuilds",
        "before": before,
        "after": after,
        "search": owned
            .iter()
            .map(|record| reduction_record_to_json(record, source_map))
            .collect::<Vec<_>>(),
    });
    let integrated = selected.map_or(0, |(names, _)| names.len());
    if admitted {
        let mut stage = Stage::ok(json);
        stage.note = Some(format!(
            "{}; {} \u{2014} integrates {integrated} state coordinate(s)",
            reducer_phrase(&stage.value, rounds),
            routes.last().copied().unwrap_or("prepared")
        ));
        stage
    } else {
        Stage::recovered(json, prep_failure_note(prep, "preparation failed"))
    }
}

/// What the reducer concluded, in the words the tab note uses.
///
/// **`index-1` only for `borrowed`**, which is what it means: the system sorted and
/// normalization left it alone. `sorted` with zero rounds is derivative-alias
/// normalization *rebuilding* the system (`StabilizedPendulum`), and calling it
/// index-1 would hide a rebuild behind a word meaning nothing happened.
fn reducer_phrase(value: &Option<serde_json::Value>, rounds: usize) -> String {
    let kind = value
        .as_ref()
        .and_then(|v| v["outcome"]["kind"].as_str())
        .unwrap_or("unreported");
    match (kind, rounds) {
        ("borrowed", _) => "already index-1".to_owned(),
        ("sorted", 0) => "normalized without a reduction round".to_owned(),
        ("sorted", n) => format!("reducer sorted after {n} round(s)"),
        ("singular", _) => "reducer stayed singular".to_owned(),
        ("unreported", _) => "reducer reported no outcome".to_owned(),
        (other, _) => format!("reducer: {other}"),
    }
}

/// The shape of the system structural preparation was handed: the last
/// transform's output, or the DAE itself when no transform changed it.
fn simplified_shape(prep: &RealPreparation) -> Option<DaeShape> {
    let mut shape = None;
    for event in &prep.solve {
        if let SolvePreparationEvent::Transform { before, after, .. } = event {
            shape = Some(after.unwrap_or(*before));
        }
    }
    shape
}

fn route_name(route: &SelectionRoute) -> &'static str {
    match route {
        SelectionRoute::LoopClosureReduced => "loop closure reduced without the reducer",
        SelectionRoute::ReducerAccepted => "reducer accepted the system",
        SelectionRoute::InertStatesDemoted => "inert states demoted, prepared again",
        SelectionRoute::RecoveredViaFormal => "singular system recovered via formal derivatives",
        SelectionRoute::PreferencesDecided => "no manifold: StateSelect preferences decided",
        SelectionRoute::RetainedNoSmallerBasis => "retained: no smaller formal basis",
        SelectionRoute::RetainedDefinitional => "retained: every constraint is a first integral",
        SelectionRoute::ReducedToFormalBasis => "reduced to a formal-derivative basis",
    }
}

/// Initialization: the plan Rumoca lowered, row by row.
pub(super) fn initialization_stage(prep: &RealPreparation) -> Stage {
    match &prep.lowered {
        Some(lowered) => {
            // Both surplus roles: `SurplusCheck` and `SurplusAlgebraicCheck` (a check
            // over a reconstructed algebraic — `OverInitRc`'s `der(C.v) = 0`).
            let surplus: u64 = lowered.initialization["summary"]
                .as_array()
                .map(|summary| {
                    summary
                        .iter()
                        .filter(|entry| {
                            entry["role"]
                                .as_str()
                                .is_some_and(|role| role.starts_with("Surplus"))
                        })
                        .filter_map(|entry| entry["rows"].as_u64())
                        .sum()
                })
                .unwrap_or(0);
            let rows = lowered.initialization["rows"]
                .as_array()
                .map_or(0, Vec::len);
            let mut stage = Stage::ok(lowered.initialization.clone());
            stage.note = Some(format!(
                "{rows} initialization row(s), {surplus} surplus check(s)"
            ));
            stage
        }
        None => Stage::info(prep_failure_note(prep, "no initialization was lowered")),
    }
}

/// Solve lowering: the executable problem the simulation integrates.
pub(super) fn solve_lowering_stage(prep: &RealPreparation) -> Stage {
    match &prep.lowered {
        Some(LoweredFacts {
            problem: Ok(problem),
            ..
        }) => Stage::ok(problem.clone()),
        Some(LoweredFacts {
            problem: Err(error),
            ..
        }) => Stage::err(format!("serialize SolveProblem: {error}")),
        None => Stage::err_with_details(
            serde_json::json!({ "message": prep.runtime.as_ref().err() }),
            prep_failure_note(prep, "solve lowering failed"),
        ),
    }
}

/// The lowered events, added to the Events tab beside the DAE's record of them.
pub(super) fn lowered_events(prep: &RealPreparation) -> Option<serde_json::Value> {
    match &prep.lowered {
        Some(LoweredFacts {
            problem: Ok(problem),
            ..
        }) => Some(serde_json::json!({
            "events": problem["events"],
            "discrete": problem["discrete"],
        })),
        _ => None,
    }
}

/// A stage note naming why preparation stopped, when it did.
fn prep_failure_note(prep: &RealPreparation, fallback: &str) -> String {
    match &prep.runtime {
        Err(error) => format!("{fallback}: {error}"),
        Ok(_) => fallback.to_owned(),
    }
}
