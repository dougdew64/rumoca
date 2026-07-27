use super::*;

fn state_has_any_equation_reference(dae: &Dae, state_name: &VarName) -> bool {
    dae.continuous
        .equations
        .iter()
        .any(|eq| expr_contains_var(&eq.rhs, state_name))
}

fn state_has_any_derivative_reference(dae: &Dae, state_name: &VarName) -> bool {
    let bindings = structural_scalar_bindings(dae);
    dae.continuous
        .equations
        .iter()
        .any(|eq| expr_contains_active_exact_der_of_state(&eq.rhs, state_name, &bindings))
}

fn try_match_state_to_row(
    state_idx: usize,
    state_to_rows: &[Vec<usize>],
    row_to_state: &mut [Option<usize>],
    seen_rows: &mut [bool],
) -> bool {
    for &row_idx in &state_to_rows[state_idx] {
        if seen_rows[row_idx] {
            continue;
        }
        seen_rows[row_idx] = true;
        if let Some(other_state_idx) = row_to_state[row_idx] {
            if try_match_state_to_row(other_state_idx, state_to_rows, row_to_state, seen_rows) {
                row_to_state[row_idx] = Some(state_idx);
                return true;
            }
            continue;
        }
        row_to_state[row_idx] = Some(state_idx);
        return true;
    }
    false
}

fn states_with_assignable_derivative_rows(dae: &Dae, state_names: &[VarName]) -> HashSet<usize> {
    let bindings = structural_scalar_bindings(dae);
    let state_to_rows: Vec<Vec<usize>> = state_names
        .iter()
        .map(|state_name| {
            dae.continuous
                .equations
                .iter()
                .enumerate()
                .filter_map(|(row_idx, eq)| {
                    if !state_derivative_row_is_assignable(eq, state_name, state_names, &bindings) {
                        return None;
                    }
                    if let Some(alias) = try_extract_derivative_alias(eq, state_name)
                        && dae.variables.states.contains_key(&alias)
                    {
                        return Some(row_idx);
                    }
                    Some(row_idx)
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let mut state_order: Vec<usize> = (0..state_names.len()).collect();
    state_order.sort_by_key(|idx| state_to_rows[*idx].len());

    let mut row_to_state: Vec<Option<usize>> = vec![None; dae.continuous.equations.len()];
    for state_idx in state_order {
        if state_to_rows[state_idx].is_empty() {
            continue;
        }
        let mut seen_rows = vec![false; dae.continuous.equations.len()];
        let _ =
            try_match_state_to_row(state_idx, &state_to_rows, &mut row_to_state, &mut seen_rows);
    }

    row_to_state.into_iter().flatten().collect()
}

fn state_derivative_row_is_assignable(
    eq: &Equation,
    state_name: &VarName,
    state_names: &[VarName],
    bindings: &HashMap<String, f64>,
) -> bool {
    if !all_active_derivative_args_are_states(&eq.rhs, state_names, bindings)
        || !expr_contains_active_exact_der_of_state(&eq.rhs, state_name, bindings)
    {
        return false;
    }
    if let Some(value) = try_extract_der_value(&eq.rhs, state_name)
        && expr_contains_active_exact_der_of_state(&value, state_name, bindings)
    {
        return false;
    }
    true
}

fn expr_contains_active_exact_der_of_state(
    expr: &Expression,
    state_name: &VarName,
    bindings: &HashMap<String, f64>,
) -> bool {
    let mut checker = ExactStateDerivativeChecker {
        state_name,
        bindings,
        found: false,
    };
    checker.visit_expression(expr);
    checker.found
}

struct ExactStateDerivativeChecker<'a> {
    state_name: &'a VarName,
    bindings: &'a HashMap<String, f64>,
    found: bool,
}

impl ExpressionVisitor for ExactStateDerivativeChecker<'_> {
    fn visit_expression(&mut self, expr: &Expression) {
        if !self.found {
            self.walk_expression(expr);
        }
    }

    fn visit_builtin_call(&mut self, function: &BuiltinFunction, args: &[Expression]) {
        if *function == BuiltinFunction::Der
            && args
                .first()
                .is_some_and(|arg| derivative_arg_matches_state(arg, self.state_name))
        {
            self.found = true;
            return;
        }
        for arg in args {
            self.visit_expression(arg);
        }
    }

    fn visit_if(&mut self, branches: &[(Expression, Expression)], else_branch: &Expression) {
        for (condition, value) in branches {
            match eval_static_bool(condition, self.bindings) {
                Some(true) => {
                    self.visit_expression(value);
                    return;
                }
                Some(false) => continue,
                None => {
                    self.visit_expression(condition);
                    self.visit_expression(value);
                }
            }
        }
        self.visit_expression(else_branch);
    }
}

fn derivative_arg_matches_state(expr: &Expression, state_name: &VarName) -> bool {
    let Some(exact_name) = expression_exact_name(expr) else {
        return false;
    };
    exact_name == state_name.as_str()
        || rumoca_core::parse_scalar_name(&exact_name)
            .is_some_and(|scalar| scalar.base == state_name.as_str())
}

fn all_active_derivative_args_are_states(
    expr: &Expression,
    state_names: &[VarName],
    bindings: &HashMap<String, f64>,
) -> bool {
    let mut checker = DerivativeArgsAreStatesChecker {
        state_names,
        bindings,
        all_are_states: true,
    };
    checker.visit_expression(expr);
    checker.all_are_states
}

struct DerivativeArgsAreStatesChecker<'a> {
    state_names: &'a [VarName],
    bindings: &'a HashMap<String, f64>,
    all_are_states: bool,
}

impl ExpressionVisitor for DerivativeArgsAreStatesChecker<'_> {
    fn visit_expression(&mut self, expr: &Expression) {
        if self.all_are_states {
            self.walk_expression(expr);
        }
    }

    fn visit_builtin_call(&mut self, function: &BuiltinFunction, args: &[Expression]) {
        if *function == BuiltinFunction::Der {
            let is_state_derivative = args.first().is_some_and(|arg| {
                self.state_names
                    .iter()
                    .any(|state_name| derivative_arg_matches_state(arg, state_name))
            });
            if !is_state_derivative {
                self.all_are_states = false;
            }
            return;
        }
        for arg in args {
            self.visit_expression(arg);
        }
    }

    fn visit_if(&mut self, branches: &[(Expression, Expression)], else_branch: &Expression) {
        for (condition, value) in branches {
            match eval_static_bool(condition, self.bindings) {
                Some(true) => {
                    self.visit_expression(value);
                    return;
                }
                Some(false) => continue,
                None => {
                    self.visit_expression(condition);
                    self.visit_expression(value);
                }
            }
        }
        self.visit_expression(else_branch);
    }
}

fn structural_scalar_bindings(dae: &Dae) -> HashMap<String, f64> {
    let mut bindings = HashMap::new();
    for _ in 0..dae.variables.parameters.len().max(1) {
        let before = bindings.len();
        for (name, var) in dae.variables.constants.iter().chain(
            dae.variables
                .parameters
                .iter()
                .filter(|(_, var)| !var.is_tunable),
        ) {
            let Some(start) = var.start.as_ref() else {
                continue;
            };
            let Some(value) = eval_static_number(start, &bindings) else {
                continue;
            };
            bindings.insert(name.as_str().to_string(), value);
        }
        if bindings.len() == before {
            break;
        }
    }
    bindings
}

fn eval_static_bool(expr: &Expression, bindings: &HashMap<String, f64>) -> Option<bool> {
    match expr {
        Expression::Literal {
            value: Literal::Boolean(value),
            ..
        } => Some(*value),
        Expression::Unary {
            op: OpUnary::Not,
            rhs,
            ..
        } => Some(!eval_static_bool(rhs, bindings)?),
        Expression::BuiltinCall { function, args, .. }
            if matches!(*function, BuiltinFunction::NoEvent) =>
        {
            eval_static_bool(args.first()?, bindings)
        }
        Expression::Binary { op, lhs, rhs, .. } => {
            let lhs = eval_static_number(lhs, bindings)?;
            let rhs = eval_static_number(rhs, bindings)?;
            match op {
                OpBinary::Lt => Some(lhs < rhs),
                OpBinary::Le => Some(lhs <= rhs),
                OpBinary::Gt => Some(lhs > rhs),
                OpBinary::Ge => Some(lhs >= rhs),
                OpBinary::Eq => Some((lhs - rhs).abs() <= f64::EPSILON),
                OpBinary::Neq => Some((lhs - rhs).abs() > f64::EPSILON),
                _ => None,
            }
        }
        _ => eval_static_number(expr, bindings).map(|value| value != 0.0),
    }
}

fn eval_static_number(expr: &Expression, bindings: &HashMap<String, f64>) -> Option<f64> {
    match expr {
        Expression::Literal { value, .. } => match value {
            Literal::Integer(value) => Some(*value as f64),
            Literal::Real(value) => Some(*value),
            Literal::Boolean(value) => Some(if *value { 1.0 } else { 0.0 }),
            _ => None,
        },
        Expression::VarRef {
            name, subscripts, ..
        } if subscripts.is_empty() => bindings.get(name.as_str()).copied(),
        Expression::Unary { op, rhs, .. } => {
            let value = eval_static_number(rhs, bindings)?;
            match op {
                OpUnary::Plus | OpUnary::DotPlus | OpUnary::Empty => Some(value),
                OpUnary::Minus | OpUnary::DotMinus => Some(-value),
                OpUnary::Not => Some(if value == 0.0 { 1.0 } else { 0.0 }),
            }
        }
        Expression::Binary { op, lhs, rhs, .. } => {
            let lhs = eval_static_number(lhs, bindings)?;
            let rhs = eval_static_number(rhs, bindings)?;
            match op {
                OpBinary::Add | OpBinary::AddElem => Some(lhs + rhs),
                OpBinary::Sub | OpBinary::SubElem => Some(lhs - rhs),
                OpBinary::Mul | OpBinary::MulElem => Some(lhs * rhs),
                OpBinary::Div | OpBinary::DivElem => Some(lhs / rhs),
                OpBinary::Exp | OpBinary::ExpElem => Some(lhs.powf(rhs)),
                OpBinary::Lt => Some(if lhs < rhs { 1.0 } else { 0.0 }),
                OpBinary::Le => Some(if lhs <= rhs { 1.0 } else { 0.0 }),
                OpBinary::Gt => Some(if lhs > rhs { 1.0 } else { 0.0 }),
                OpBinary::Ge => Some(if lhs >= rhs { 1.0 } else { 0.0 }),
                OpBinary::Eq => Some(if (lhs - rhs).abs() <= f64::EPSILON {
                    1.0
                } else {
                    0.0
                }),
                OpBinary::Neq => Some(if (lhs - rhs).abs() > f64::EPSILON {
                    1.0
                } else {
                    0.0
                }),
                _ => None,
            }
        }
        Expression::BuiltinCall { function, args, .. }
            if matches!(*function, BuiltinFunction::NoEvent) =>
        {
            eval_static_number(args.first()?, bindings)
        }
        _ => None,
    }
}

fn expression_exact_name(expr: &Expression) -> Option<String> {
    match expr {
        Expression::VarRef {
            name, subscripts, ..
        } => append_subscripts(name.as_str().to_string(), subscripts),
        Expression::Index {
            base, subscripts, ..
        } => {
            let base_name = expression_exact_name(base)?;
            append_subscripts(base_name, subscripts)
        }
        Expression::FieldAccess { base, field, .. } => {
            let base_name = expression_exact_name(base)?;
            Some(format!("{base_name}.{field}"))
        }
        _ => None,
    }
}

fn append_subscripts(base: String, subscripts: &[Subscript]) -> Option<String> {
    if subscripts.is_empty() {
        return Some(base);
    }
    let mut indices = Vec::with_capacity(subscripts.len());
    for subscript in subscripts {
        indices.push(subscript_index_text(subscript)?);
    }
    Some(format!("{base}[{}]", indices.join(",")))
}

fn subscript_index_text(subscript: &Subscript) -> Option<String> {
    match subscript {
        Subscript::Index { value, .. } => Some(value.to_string()),
        Subscript::Expr { expr, .. } => match expr.as_ref() {
            Expression::Literal {
                value: Literal::Integer(value),
                ..
            } => Some(value.to_string()),
            Expression::Literal {
                value: Literal::Real(value),
                ..
            } if value.is_finite() && value.fract() == 0.0 => Some((*value as i64).to_string()),
            _ => None,
        },
        Subscript::Colon { .. } => None,
    }
}

/// Demote states that are no longer referenced by any continuous equation.
///
/// Trivial elimination may remove an alias/binding equation that was the only
/// remaining reference to a misclassified state-like variable. Such orphan
/// states cannot have valid ODE rows and should be treated as algebraics.
pub fn demote_orphan_states_without_equation_refs(dae: &mut Dae) -> usize {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    let mut demoted = 0usize;
    for name in state_names {
        if state_has_any_equation_reference(dae, &name) {
            continue;
        }
        if let Some(var) = dae.variables.states.shift_remove(&name) {
            dae.variables.algebraics.insert(name, var);
            demoted += 1;
        }
    }
    demoted
}

/// Demote state variables that have no `der(state)` occurrence in any equation.
///
/// Promotion of algebraics used in `der(...)` expressions can temporarily mark
/// variables as states even if later structural passes remove all derivative
/// occurrences for that variable. Such variables cannot be solved as states and
/// must remain algebraic.
pub fn demote_states_without_derivative_refs(dae: &mut Dae) -> usize {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    let mut demoted = 0usize;
    for name in state_names {
        if state_has_any_derivative_reference(dae, &name) {
            continue;
        }
        if sim_trace_enabled() {
            crate::structural_trace!(
                "[sim-trace] demoting state without derivative refs: {}",
                name.as_str()
            );
        }
        if let Some(var) = dae.variables.states.shift_remove(&name) {
            dae.variables.algebraics.insert(name, var);
            demoted += 1;
        }
    }
    demoted
}

/// Demote states that cannot be assigned a unique derivative row.
///
/// The simulator's ODE row ordering needs at least one assignable derivative
/// equation per retained state. We compute a maximum bipartite matching between
/// states and derivative-bearing rows; unmatched states are demoted to
/// algebraics.
pub fn demote_states_without_assignable_derivative_rows(dae: &mut Dae) -> usize {
    let mut total_demoted = 0usize;

    loop {
        let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
        if state_names.is_empty() {
            break;
        }

        let matched_states = states_with_assignable_derivative_rows(dae, &state_names);
        let to_demote: Vec<VarName> = state_names
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| (!matched_states.contains(&idx)).then_some(name.clone()))
            .collect();

        if to_demote.is_empty() {
            break;
        }

        let mut demoted_this_round = 0usize;
        for name in to_demote {
            if sim_trace_enabled() {
                crate::structural_trace!(
                    "[sim-trace] demoting state without assignable derivative row: {}",
                    name.as_str()
                );
            }
            if let Some(var) = dae.variables.states.shift_remove(&name) {
                dae.variables.algebraics.insert(name, var);
                demoted_this_round += 1;
            }
        }
        if demoted_this_round == 0 {
            break;
        }
        total_demoted += demoted_this_round;
    }

    total_demoted
}

/// Final state cleanup after late prepare passes that can remove continuous rows.
///
/// MLS Appendix B / SPEC_0003: retained states require retained derivative
/// rows. This combines the existing no-derivative and no-assignable-row
/// demotions without adding logging, timeout, or backend policy.
pub fn demote_states_without_retained_derivative_rows(
    dae: &mut Dae,
) -> Result<(usize, usize), StructuralError> {
    let n_no_derivative_refs = demote_states_without_derivative_refs(dae);
    let n_unassignable_derivative_rows = demote_states_without_assignable_derivative_rows(dae);
    Ok((n_no_derivative_refs, n_unassignable_derivative_rows))
}

/// Phase-1 structural index reduction.
///
/// For each state without a `der(state)` equation, find a non-ODE constraint
/// referencing that state and differentiate it once with symbolic chain-rule.
/// The differentiated equation must explicitly contain `der(state)` to be
/// accepted; otherwise it is discarded.
pub fn index_reduce_missing_state_derivatives_once(
    dae: &mut Dae,
) -> Result<usize, StructuralError> {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    if state_names.is_empty() {
        return Ok(0);
    }
    let state_name_set: HashSet<String> = state_names
        .iter()
        .map(|name| name.as_str().to_string())
        .collect();
    let state_derivative_matcher = DerivativeNameMatcher::from_var_names(&state_names);

    let defining_expr_index = collect_residual_defining_expr_index(dae);
    let mut changed = 0usize;
    let mut used_eq = HashSet::new();

    for state_name in &state_names {
        if state_has_standalone_der_equation(dae, state_name, &state_names)? {
            continue;
        }

        let candidate_indices: Vec<usize> = dae
            .continuous
            .equations
            .iter()
            .enumerate()
            .filter_map(|(idx, eq)| {
                if used_eq.contains(&idx) {
                    return None;
                }
                if eq_contains_any_state_der_with_matcher(&eq.rhs, &state_derivative_matcher) {
                    return None;
                }
                if dae
                    .variables
                    .algebraics
                    .keys()
                    .any(|alg_name| is_unsliced_algebraic_definition(eq, alg_name))
                {
                    return None;
                }
                if is_indexed_state_component_alias_definition(eq, state_name) {
                    return None;
                }
                Some(idx)
            })
            .collect();

        for idx in candidate_indices {
            let seed_exprs = vec![dae.continuous.equations[idx].rhs.clone()];
            let der_map = build_relaxed_derivative_map_for_exprs_with_index(
                dae,
                &defining_expr_index,
                &seed_exprs,
            )?;
            let differentiated =
                symbolic_time_derivative(&dae.continuous.equations[idx].rhs, dae, &der_map);
            let Some(new_rhs) = differentiated else {
                continue;
            };
            let der_states = derivative_states_in_eq(&new_rhs, &state_names);
            if !der_states.iter().any(|der_state| der_state == state_name) {
                continue;
            }
            if expr_contains_der_of_non_state(&new_rhs, &state_name_set) {
                continue;
            }

            let old_origin = dae.continuous.equations[idx].origin.clone();
            dae.continuous.equations[idx].rhs = new_rhs;
            dae.continuous.equations[idx].origin = if old_origin.is_empty() {
                format!("index_reduction:d_dt_for_{}", state_name.as_str())
            } else {
                format!(
                    "{}|index_reduction:d_dt_for_{}",
                    old_origin,
                    state_name.as_str()
                )
            };
            used_eq.insert(idx);
            changed += 1;
            break;
        }
    }

    Ok(changed)
}

fn is_unsliced_algebraic_definition(eq: &Equation, alg_name: &VarName) -> bool {
    let Expression::Binary { op, lhs, rhs, .. } = &eq.rhs else {
        return false;
    };
    if !matches!(op, OpBinary::Sub) {
        return false;
    }
    [lhs.as_ref(), rhs.as_ref()].into_iter().any(|expr| {
        matches!(
            expr,
            Expression::VarRef { name, subscripts, .. }
                if name.var_name() == alg_name && subscripts.is_empty()
        )
    })
}

fn is_indexed_state_component_alias_definition(eq: &Equation, state_name: &VarName) -> bool {
    let Expression::Binary { op, lhs, rhs, .. } = &eq.rhs else {
        return false;
    };
    if !matches!(op, OpBinary::Sub) {
        return false;
    }
    let lhs_is_state_component = is_indexed_component_of_state(lhs, state_name);
    let rhs_is_state_component = is_indexed_component_of_state(rhs, state_name);
    if lhs_is_state_component == rhs_is_state_component {
        return false;
    }
    let other = if lhs_is_state_component { rhs } else { lhs };
    !expr_contains_var(other, state_name)
}

fn is_indexed_component_of_state(expr: &Expression, state_name: &VarName) -> bool {
    let Some(exact_name) = expression_exact_name(expr) else {
        return false;
    };
    exact_name != state_name.as_str()
        && rumoca_core::parse_scalar_name(&exact_name)
            .is_some_and(|scalar| scalar.base == state_name.as_str())
}

pub fn index_reduce_missing_state_derivatives(dae: &mut Dae) -> Result<usize, StructuralError> {
    let max_rounds = dae.variables.states.len().clamp(1, 8);
    let mut total_changed = 0usize;
    for _round in 0..max_rounds {
        let changed = index_reduce_missing_state_derivatives_once(dae)?;
        if changed == 0 {
            break;
        }
        total_changed += changed;
    }
    Ok(total_changed)
}

/// One step of the index-reduction algorithm, recorded for animation replay.
#[derive(Debug, Clone)]
pub enum IndexReductionStep {
    /// The system as tracing begins, before any traced work.
    ///
    /// Emitted once by [`emit_index_reduction_start`]. Without it a replay opens
    /// on the first `BeginState`, which announces an *intention* ("searching for
    /// a constraint…") and so reads as though work has already happened — with
    /// no frame anywhere showing the system before reduction. An animation whose
    /// purpose is to show what reduction changed needs a visible "before".
    ///
    /// `states` and `equations` describe the DAE at the moment tracing starts,
    /// which is not necessarily the raw DAE: callers may have run untraced
    /// demotion steps first (`demote_exact_alias_component_states` and
    /// `demote_direct_assigned_states` both run before the traced passes in the
    /// standard pipeline). This is the baseline for the *animation*, and is
    /// labelled as such rather than as the original system.
    Start { states: Vec<String>, equations: usize },
    /// Starting to evaluate a candidate state for reduction.
    BeginState { state: String },
    /// A constraint was differentiated — before and after RHS expressions.
    Differentiated {
        state: String,
        before_rhs: Expression,
        after_rhs: Expression,
    },
    /// No suitable constraint found for this state in this round.
    CandidateExhausted { state: String },
    /// State demoted from differential to algebraic.
    Demoted { state: String },
    /// An outer-loop round completed.
    RoundComplete { round: usize, demotions_this_round: usize },
}

/// Snapshot of the index-reduction state at each step, for animation rendering.
#[derive(Debug, Clone)]
pub struct IndexReductionFrame {
    pub step: IndexReductionStep,
    /// Names of states demoted so far during this reduction pass.
    pub demoted_so_far: Vec<String>,
    /// Current outer-loop round (0-based).
    pub round: usize,
}

/// Result of the traced index-reduction pass.
pub struct IndexReductionTraceResult {
    pub total_demoted: usize,
    pub frames: Vec<IndexReductionFrame>,
}

pub fn emit_index_reduction_frame(
    frames: &mut Vec<IndexReductionFrame>,
    live: Option<&crate::LiveTrace<IndexReductionFrame>>,
    frame: IndexReductionFrame,
) {
    if let Some(lt) = live {
        lt.push(frame.clone());
    }
    frames.push(frame);
}

/// Emit the opening [`IndexReductionStep::Start`] frame.
///
/// Call once, immediately before the first traced reduction pass. It is a
/// separate call rather than something the passes do for themselves because the
/// standard pipeline runs *two* traced passes back to back
/// (`reduce_constrained_dummy_derivatives_with_trace` then
/// `index_reduce_missing_state_derivatives_with_trace`) and only their combined
/// output is one animation — self-emission would produce two "start" frames in
/// the middle of a single replay.
///
/// The snapshot describes the DAE at this instant, which may already differ from
/// the raw DAE if the caller ran untraced demotion steps first. See
/// [`IndexReductionStep::Start`].
pub fn emit_index_reduction_start(
    frames: &mut Vec<IndexReductionFrame>,
    live: Option<&crate::LiveTrace<IndexReductionFrame>>,
    dae: &Dae,
    demoted_so_far: &[String],
) {
    emit_index_reduction_frame(frames, live, IndexReductionFrame {
        step: IndexReductionStep::Start {
            states: dae
                .variables
                .states
                .keys()
                .map(|name| name.as_str().to_string())
                .collect(),
            equations: dae.continuous.equations.len(),
        },
        demoted_so_far: demoted_so_far.to_vec(),
        round: 0,
    });
}

/// Like [`index_reduce_missing_state_derivatives`], but records every
/// algorithmic step for animation replay.
pub fn index_reduce_missing_state_derivatives_with_trace(
    dae: &mut Dae,
    live: Option<&crate::LiveTrace<IndexReductionFrame>>,
    frames: &mut Vec<IndexReductionFrame>,
    demoted_so_far: &mut Vec<String>,
    round_offset: usize,
) -> Result<usize, StructuralError> {
    let max_rounds = dae.variables.states.len().clamp(1, 8);
    let mut total_changed = 0usize;
    for round in 0..max_rounds {
        let changed = index_reduce_missing_state_derivatives_once_with_trace(
            dae, live, frames, demoted_so_far, round_offset + round,
        )?;
        emit_index_reduction_frame(frames, live, IndexReductionFrame {
            step: IndexReductionStep::RoundComplete {
                round: round_offset + round,
                demotions_this_round: changed,
            },
            demoted_so_far: demoted_so_far.clone(),
            round: round_offset + round,
        });
        if changed == 0 {
            break;
        }
        total_changed += changed;
    }
    Ok(total_changed)
}

fn index_reduce_missing_state_derivatives_once_with_trace(
    dae: &mut Dae,
    live: Option<&crate::LiveTrace<IndexReductionFrame>>,
    frames: &mut Vec<IndexReductionFrame>,
    demoted_so_far: &mut Vec<String>,
    round: usize,
) -> Result<usize, StructuralError> {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    if state_names.is_empty() {
        return Ok(0);
    }
    let state_name_set: HashSet<String> = state_names
        .iter()
        .map(|name| name.as_str().to_string())
        .collect();
    let state_derivative_matcher = DerivativeNameMatcher::from_var_names(&state_names);

    let defining_expr_index = collect_residual_defining_expr_index(dae);
    let mut changed = 0usize;
    let mut used_eq = HashSet::new();

    for state_name in &state_names {
        if state_has_standalone_der_equation(dae, state_name, &state_names)? {
            continue;
        }

        emit_index_reduction_frame(frames, live, IndexReductionFrame {
            step: IndexReductionStep::BeginState { state: state_name.to_string() },
            demoted_so_far: demoted_so_far.clone(),
            round,
        });

        let candidate_indices: Vec<usize> = dae
            .continuous
            .equations
            .iter()
            .enumerate()
            .filter_map(|(idx, eq)| {
                if used_eq.contains(&idx) {
                    return None;
                }
                if eq_contains_any_state_der_with_matcher(&eq.rhs, &state_derivative_matcher) {
                    return None;
                }
                if dae
                    .variables
                    .algebraics
                    .keys()
                    .any(|alg_name| is_unsliced_algebraic_definition(eq, alg_name))
                {
                    return None;
                }
                if is_indexed_state_component_alias_definition(eq, state_name) {
                    return None;
                }
                Some(idx)
            })
            .collect();

        let mut found = false;
        for idx in candidate_indices {
            let seed_exprs = vec![dae.continuous.equations[idx].rhs.clone()];
            let der_map = build_relaxed_derivative_map_for_exprs_with_index(
                dae,
                &defining_expr_index,
                &seed_exprs,
            )?;
            let differentiated =
                symbolic_time_derivative(&dae.continuous.equations[idx].rhs, dae, &der_map);
            let Some(new_rhs) = differentiated else {
                continue;
            };
            let der_states = derivative_states_in_eq(&new_rhs, &state_names);
            if !der_states.iter().any(|der_state| der_state == state_name) {
                continue;
            }
            if expr_contains_der_of_non_state(&new_rhs, &state_name_set) {
                continue;
            }

            let before_rhs = dae.continuous.equations[idx].rhs.clone();
            emit_index_reduction_frame(frames, live, IndexReductionFrame {
                step: IndexReductionStep::Differentiated {
                    state: state_name.to_string(),
                    before_rhs,
                    after_rhs: new_rhs.clone(),
                },
                demoted_so_far: demoted_so_far.clone(),
                round,
            });

            let old_origin = dae.continuous.equations[idx].origin.clone();
            dae.continuous.equations[idx].rhs = new_rhs;
            dae.continuous.equations[idx].origin = if old_origin.is_empty() {
                format!("index_reduction:d_dt_for_{}", state_name.as_str())
            } else {
                format!(
                    "{}|index_reduction:d_dt_for_{}",
                    old_origin,
                    state_name.as_str()
                )
            };
            used_eq.insert(idx);
            changed += 1;
            found = true;
            break;
        }
        if !found {
            emit_index_reduction_frame(frames, live, IndexReductionFrame {
                step: IndexReductionStep::CandidateExhausted { state: state_name.to_string() },
                demoted_so_far: demoted_so_far.clone(),
                round,
            });
        }
    }

    Ok(changed)
}

/// Regularisation epsilon levels to try, from most accurate to least.
///
/// The larger fallback values help stiff, switch-heavy MSL examples that can
/// otherwise fail early with very small accepted timesteps.
pub const REGULARIZATION_LEVELS: &[f64] = &[1e-8, 1e-6, 1e-4, 1e-3, 1e-2, 1e-1];

/// Determine the sign of `der(state)` in an expression by tracking negations.
///
/// Returns +1 if der(state) appears with positive coefficient, -1 if negative, 0 if absent.
/// Tracks sign flips through subtraction (RHS negated) and unary minus.
pub fn der_sign_in_expr(expr: &Expression, state_name: &VarName, current_sign: i32) -> i32 {
    match expr {
        Expression::BuiltinCall {
            function: BuiltinFunction::Der,
            args,
            ..
        } if args.len() == 1 && expr_refers_to_var(&args[0], state_name) => current_sign,
        Expression::Binary { op, lhs, rhs, .. } => match op {
            OpBinary::Add | OpBinary::AddElem => {
                let l = der_sign_in_expr(lhs, state_name, current_sign);
                if l != 0 {
                    return l;
                }
                der_sign_in_expr(rhs, state_name, current_sign)
            }
            OpBinary::Sub | OpBinary::SubElem => {
                let l = der_sign_in_expr(lhs, state_name, current_sign);
                if l != 0 {
                    return l;
                }
                der_sign_in_expr(rhs, state_name, -current_sign)
            }
            OpBinary::Mul | OpBinary::MulElem => {
                let l = der_sign_in_expr(lhs, state_name, current_sign);
                if l != 0 {
                    return l;
                }
                der_sign_in_expr(rhs, state_name, current_sign)
            }
            _ => 0,
        },
        Expression::Unary { op, rhs, .. } => match op {
            OpUnary::Minus | OpUnary::DotMinus => der_sign_in_expr(rhs, state_name, -current_sign),
            _ => der_sign_in_expr(rhs, state_name, current_sign),
        },
        Expression::If {
            branches,
            else_branch,
            ..
        } => {
            for (_, v) in branches {
                let s = der_sign_in_expr(v, state_name, current_sign);
                if s != 0 {
                    return s;
                }
            }
            der_sign_in_expr(else_branch, state_name, current_sign)
        }
        _ => 0,
    }
}

/// Normalize ODE equation signs so that `der(state)` has positive coefficient.
///
/// The mass-matrix formulation `M * y' = f` with `f = -eval(equation)` for ODE
/// rows requires `der(state)` to appear with coefficient +1 in the residual.
/// Equations like `0 = v - der(s)` (from `v = der(s)` in Modelica) have
/// coefficient -1 and produce the wrong sign.
///
/// This pass negates equations where `der(state)` has negative coefficient.
pub fn normalize_ode_equation_signs(dae: &mut Dae) {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    for (i, state_name) in state_names.iter().enumerate() {
        if i >= dae.continuous.equations.len() {
            break;
        }
        let sign = der_sign_in_expr(&dae.continuous.equations[i].rhs, state_name, 1);
        if sign < 0 {
            let old_rhs = dae.continuous.equations[i].rhs.clone();
            let span = dae.continuous.equations[i].span;
            dae.continuous.equations[i].rhs = Expression::Unary {
                op: OpUnary::Minus,
                rhs: Box::new(old_rhs),
                span,
            };
        }
    }
}

/// After ODE row selection, non-ODE residual rows must not keep standalone
/// `der(state)` calls because compiled residual evaluation lowers `der(...)`
/// to zero outside the mass-matrix rows. Substitute any duplicate standalone
/// state derivative that can be resolved from the selected ODE rows.
pub fn substitute_standalone_state_derivatives_in_non_ode_rows(dae: &mut Dae) -> usize {
    let n_x: usize = dae.variables.states.values().map(Variable::size).sum();
    if n_x == 0 {
        return 0;
    }

    let der_map = build_der_value_map(dae);
    if der_map.is_empty() {
        return 0;
    }

    // Scalar states only. This pass substitutes a single `der_map[state]` value
    // for every `der(state)` occurrence, which is well-defined only for scalar
    // states. An array/matrix state `R[m,n]` has one ODE row *per component*
    // (`der(R[i,j]) = ...`); `der_map[R]` is the whole-array der value, and
    // `der(R[i,j])` matches state `R` by base name, so substituting here would
    // (a) overwrite component derivatives with the entire array and (b) rewrite
    // all-but-one component ODE row (only one "defining row" per state name is
    // protected), corrupting them into unmatchable residuals. The matcher
    // handles component ODE rows directly, so array states are left untouched.
    let state_names: Vec<VarName> = dae
        .variables
        .states
        .iter()
        .filter(|(_, var)| var.size() <= 1)
        .map(|(name, _)| name.clone())
        .collect();

    // For each state, locate the equation that *defines* its derivative — the
    // first row whose rhs is `der(state)` with an extractable value (the same
    // row `build_der_value_map` uses). `der(state)` must never be substituted
    // inside its own defining row: that collapses the row to `value = value`
    // and orphans `der(state)`. (Keying off equation order — "skip the first
    // n_x rows" — is unsafe: the ODE rows are not guaranteed to come first.)
    let mut defining_row: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for state_name in &state_names {
        if !der_map.contains_key(state_name.as_str()) {
            continue;
        }
        for (index, eq) in dae.continuous.equations.iter().enumerate() {
            if expr_contains_der_of(&eq.rhs, state_name)
                && try_extract_der_value(&eq.rhs, state_name).is_some()
            {
                defining_row.insert(state_name.as_str(), index);
                break;
            }
        }
    }

    let mut rewritten_rows = 0usize;
    for (index, eq) in dae.continuous.equations.iter_mut().enumerate() {
        let mut rewritten = false;
        for state_name in &state_names {
            // Never rewrite the row that defines this state's derivative.
            if defining_row.get(state_name.as_str()) == Some(&index) {
                continue;
            }
            let Some(replacement) = der_map.get(state_name.as_str()) else {
                continue;
            };
            if expression_contains_any_der_call(replacement) {
                continue;
            }
            if !expr_contains_der_of(&eq.rhs, state_name) {
                continue;
            }
            eq.rhs = substitute_der_of_state(&eq.rhs, state_name, replacement);
            rewritten = true;
        }
        rewritten_rows += usize::from(rewritten);
    }

    rewritten_rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use rumoca_core::{BuiltinFunction, Reference, SourceId};

    fn test_span() -> Span {
        Span::from_offsets(
            SourceId::from_source_name("state_row_reduction_test.mo"),
            12,
            31,
        )
    }

    fn var_ref(name: &str, span: Span) -> Expression {
        Expression::VarRef {
            name: Reference::from_var_name(VarName::new(name)),
            subscripts: Vec::new(),
            span,
        }
    }

    fn der_call(name: &str, span: Span) -> Expression {
        Expression::BuiltinCall {
            function: BuiltinFunction::Der,
            args: vec![var_ref(name, span)],
            span,
        }
    }

    #[test]
    fn emit_index_reduction_frame_pushes_to_both_vec_and_live_trace() {
        let (lt, rx) = crate::LiveTrace::new();
        let mut frames = Vec::new();
        let frame = IndexReductionFrame {
            step: IndexReductionStep::BeginState { state: "x".into() },
            demoted_so_far: vec![],
            round: 0,
        };
        emit_index_reduction_frame(&mut frames, Some(&lt), frame);
        let frame2 = IndexReductionFrame {
            step: IndexReductionStep::Demoted { state: "x".into() },
            demoted_so_far: vec!["x".into()],
            round: 0,
        };
        emit_index_reduction_frame(&mut frames, Some(&lt), frame2);

        assert_eq!(frames.len(), 2);
        let live_frames: Vec<_> = rx.try_iter().collect();
        assert_eq!(live_frames.len(), 2);
        assert_eq!(frames[0].demoted_so_far, live_frames[0].demoted_so_far);
        assert_eq!(frames[1].demoted_so_far, live_frames[1].demoted_so_far);
    }

    #[test]
    fn emit_index_reduction_frame_works_without_live_trace() {
        let mut frames = Vec::new();
        let frame = IndexReductionFrame {
            step: IndexReductionStep::RoundComplete { round: 0, demotions_this_round: 1 },
            demoted_so_far: vec!["y".into()],
            round: 0,
        };
        emit_index_reduction_frame(&mut frames, None, frame);
        assert_eq!(frames.len(), 1);
    }

    /// The opening frame must describe the system as tracing begins, so a
    /// replay has a visible "before" rather than starting on the first
    /// `BeginState` — which announces an intention and reads as though work
    /// has already happened.
    #[test]
    fn emit_index_reduction_start_snapshots_states_and_equations() {
        let span = test_span();
        let mut dae = Dae::new();
        for name in ["s", "t"] {
            dae.variables.states.insert(
                VarName::new(name),
                Variable::new(VarName::new(name), span),
            );
        }
        dae.continuous.equations.push(Equation::residual(
            var_ref("v", span), span, "test",
        ));

        let (lt, rx) = crate::LiveTrace::new();
        let mut frames = Vec::new();
        emit_index_reduction_start(&mut frames, Some(&lt), &dae, &["earlier".to_owned()]);

        assert_eq!(frames.len(), 1, "exactly one opening frame");
        let IndexReductionStep::Start { states, equations } = &frames[0].step else {
            panic!("expected a Start step, got {:?}", frames[0].step);
        };
        assert_eq!(equations, &1);
        let mut states = states.clone();
        states.sort();
        assert_eq!(states, vec!["s".to_owned(), "t".to_owned()]);
        // Carries whatever was already demoted, rather than assuming nothing was.
        assert_eq!(frames[0].demoted_so_far, vec!["earlier".to_owned()]);
        assert_eq!(frames[0].round, 0);

        // The live consumer sees it too, so a debug session opens on it.
        let live: Vec<_> = rx.try_iter().collect();
        assert_eq!(live.len(), 1, "the opening frame must reach the live trace");
    }

    #[test]
    fn normalize_ode_equation_sign_uses_equation_span() {
        let span = test_span();
        let mut dae = Dae::new();
        dae.variables.states.insert(
            VarName::new("s"),
            Variable::new(
                VarName::new("s"),
                rumoca_core::Span::from_offsets(
                    rumoca_core::SourceId::from_source_name(file!()),
                    1,
                    2,
                ),
            ),
        );
        dae.continuous.equations.push(Equation::residual(
            Expression::Binary {
                op: OpBinary::Sub,
                lhs: Box::new(var_ref("v", span)),
                rhs: Box::new(der_call("s", span)),
                span,
            },
            span,
            "test",
        ));

        normalize_ode_equation_signs(&mut dae);

        let Expression::Unary {
            op: OpUnary::Minus,
            span: actual,
            ..
        } = dae.continuous.equations[0].rhs
        else {
            panic!("expected normalized unary minus");
        };
        assert_eq!(actual, span);
    }
}
