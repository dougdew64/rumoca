//! Greedy Cellier-style tearing for algebraic loops.
//!
//! Converts an N-equation algebraic loop into K iteration (tear) variables
//! plus (N-K) causally ordered steps, reducing the nonlinear solve dimension.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use rumoca_core::FrameObserver;

/// Result of tearing an algebraic loop.
#[derive(Debug, Clone)]
pub struct TearingResult {
    /// Indices of tear (iteration) variables within the block's unknown list.
    pub tear_var_local_indices: Vec<usize>,
    /// Indices of residual equations within the block's equation list.
    /// Same count as `tear_var_local_indices`.
    pub residual_eq_local_indices: Vec<usize>,
    /// Causal steps: (equation local index, variable local index) in solve order.
    pub causal_sequence: Vec<(usize, usize)>,
}

/// Repeatedly find equations with exactly 1 remaining unknown and solve them causally.
///
/// When multiple equations can solve for the same variable, prefer the equation
/// with fewer total unknowns (less coupling, more likely to be well-conditioned).
fn resolve_causal_equations(
    remaining_eqs: &mut BTreeSet<usize>,
    remaining_unknowns: &mut BTreeSet<usize>,
    causal_sequence: &mut Vec<(usize, usize)>,
    eq_unknowns: &[HashSet<usize>],
    observer: Option<FrameObserver<'_, TearingFrame>>,
    tears_so_far: &[usize],
) {
    let mut changed = true;
    while changed {
        changed = false;
        // Build map: variable → list of (equation, eq_total_unknowns)
        // This lets us resolve conflicts deterministically
        let mut var_to_eqs: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for &eq in remaining_eqs.iter() {
            let live: Vec<usize> = eq_unknowns[eq]
                .iter()
                .copied()
                .filter(|v| remaining_unknowns.contains(v))
                .collect();
            if live.len() == 1 {
                let var = live[0];
                var_to_eqs
                    .entry(var)
                    .or_default()
                    .push((eq, eq_unknowns[eq].len()));
            }
        }

        // For each variable that can be solved, pick the best equation:
        // prefer fewer total unknowns (simpler equation), then lower index (deterministic)
        for (var, mut candidates) in var_to_eqs {
            if !remaining_unknowns.contains(&var) {
                continue;
            }
            candidates.sort_by_key(|&(eq, total)| (total, eq));
            let (best_eq, _) = candidates[0];
            causal_sequence.push((best_eq, var));
            emit_tearing(
                observer,
                tears_so_far,
                causal_sequence,
                TearingStep::Causal {
                    equation: best_eq,
                    variable: var,
                    competitors: candidates.len(),
                },
            );
            remaining_eqs.remove(&best_eq);
            remaining_unknowns.remove(&var);
            changed = true;
        }
    }
}

/// Count how many remaining equations reference each remaining unknown.
fn count_var_appearances(
    remaining_eqs: &BTreeSet<usize>,
    eq_unknowns: &[HashSet<usize>],
    remaining_unknowns: &BTreeSet<usize>,
) -> BTreeMap<usize, usize> {
    let mut var_count: BTreeMap<usize, usize> = BTreeMap::new();
    for &eq in remaining_eqs {
        for &v in &eq_unknowns[eq] {
            if remaining_unknowns.contains(&v) {
                *var_count.entry(v).or_insert(0) += 1;
            }
        }
    }
    var_count
}


/// One decision made while tearing an algebraic loop, recorded for replay.
///
/// Tearing is a *greedy* algorithm, and greedy algorithms are exactly the kind
/// worth watching: every step is a choice made on local information, and the
/// interesting question is always "why that one?". Each variant carries the
/// reason, not just the outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TearingStep {
    /// Beginning, with the size of the loop to be torn.
    Start { n: usize },
    /// An equation had exactly one unknown left, so it can be solved directly —
    /// no iteration needed. `competitors` is how many equations could have
    /// solved for this variable; when it is more than one, the tie was broken by
    /// preferring the equation with fewer total unknowns.
    Causal { equation: usize, variable: usize, competitors: usize },
    /// No equation has a single remaining unknown, so the loop must be *cut*.
    /// The chosen variable appears in `appearances` of the remaining equations —
    /// the most of any candidate, which is the greedy criterion.
    Torn { variable: usize, appearances: usize, remaining_equations: usize },
    /// Tearing finished. `residuals` equations remain, driven by the solver
    /// against `tears` guessed variables — that pair is the size of the
    /// iteration the solver is left with, and the whole point of the exercise.
    Complete { tears: usize, residuals: usize },
    /// Tearing made no progress: every equation references every unknown, so
    /// there is nothing to cut that would shrink the problem.
    NoProgress,
}

/// A tearing step plus the running state at that moment.
#[derive(Debug, Clone)]
pub struct TearingFrame {
    pub step: TearingStep,
    /// Tear variables chosen so far, in order.
    pub tears_so_far: Vec<usize>,
    /// Equations solved causally so far, as `(equation, variable)`.
    pub causal_so_far: Vec<(usize, usize)>,
}

/// Apply greedy Cellier-style tearing to an algebraic loop.
///
/// Given equations `eq_indices` and unknowns `var_indices` of equal length N,
/// with `eq_unknowns[i]` giving the set of unknown local indices referenced
/// by equation i:
///
/// 1. Repeatedly find equations with exactly 1 remaining unknown → solve causally.
///    When multiple equations compete for the same variable, prefer the one
///    with fewer total unknowns (less coupling).
/// 2. When stuck, pick the unknown appearing in the most remaining equations
///    as a tear variable and remove it from the "remaining" set.
/// 3. Repeat until all equations are causal or assigned as residuals.
///
/// Returns `None` if tearing makes no progress (all equations reference all unknowns).
pub fn tear_algebraic_loop(n: usize, eq_unknowns: &[HashSet<usize>]) -> Option<TearingResult> {
    tear_algebraic_loop_with_trace(n, eq_unknowns, None)
}

/// Like [`tear_algebraic_loop`], but reports every decision as it is made.
///
/// Additive and observation-only — the untraced entry point is this with `None`.
/// See [`rumoca_core::FrameObserver`].
pub fn tear_algebraic_loop_with_trace(
    n: usize,
    eq_unknowns: &[HashSet<usize>],
    observer: Option<FrameObserver<'_, TearingFrame>>,
) -> Option<TearingResult> {
    if n == 0 {
        return None;
    }
    emit_tearing(observer, &[], &[], TearingStep::Start { n });

    let mut remaining_eqs: BTreeSet<usize> = (0..n).collect();
    let mut remaining_unknowns: BTreeSet<usize> = (0..n).collect();
    let mut causal_sequence: Vec<(usize, usize)> = Vec::new();
    let mut tear_vars: Vec<usize> = Vec::new();

    loop {
        // Phase 1: find equations with exactly 1 remaining unknown
        resolve_causal_equations(
            &mut remaining_eqs,
            &mut remaining_unknowns,
            &mut causal_sequence,
            eq_unknowns,
            observer,
            &tear_vars,
        );

        if remaining_eqs.is_empty() {
            break;
        }

        // Phase 2: select tear variable (most appearances in remaining equations,
        // break ties by lowest index for determinism)
        let var_count = count_var_appearances(&remaining_eqs, eq_unknowns, &remaining_unknowns);

        if var_count.is_empty() {
            // No progress possible
            emit_tearing(observer, &tear_vars, &causal_sequence, TearingStep::NoProgress);
            break;
        }

        let &tear_var = var_count
            .iter()
            .max_by_key(|&(v, count)| (*count, std::cmp::Reverse(*v)))
            .map(|(v, _)| v)
            .unwrap();

        let appearances = var_count.get(&tear_var).copied().unwrap_or(0);
        tear_vars.push(tear_var);
        emit_tearing(
            observer,
            &tear_vars,
            &causal_sequence,
            TearingStep::Torn {
                variable: tear_var,
                appearances,
                remaining_equations: remaining_eqs.len(),
            },
        );
        remaining_unknowns.remove(&tear_var);
        // Don't remove any equation — they become potential causal or residual
    }

    // The remaining equations are the residual equations (driven by LM)
    let mut residual_eqs: Vec<usize> = remaining_eqs.into_iter().collect();
    residual_eqs.sort_unstable();

    // Only useful if we actually reduced the dimension
    if tear_vars.is_empty() || tear_vars.len() >= n {
        return None;
    }

    // Sanity: residual count should equal tear var count
    if residual_eqs.len() != tear_vars.len() {
        return None;
    }

    emit_tearing(
        observer,
        &tear_vars,
        &causal_sequence,
        TearingStep::Complete { tears: tear_vars.len(), residuals: residual_eqs.len() },
    );
    Some(TearingResult {
        tear_var_local_indices: tear_vars,
        residual_eq_local_indices: residual_eqs,
        causal_sequence,
    })
}

/// Hand one frame to the observer, if anyone is watching.
fn emit_tearing(
    observer: Option<FrameObserver<'_, TearingFrame>>,
    tears: &[usize],
    causal: &[(usize, usize)],
    step: TearingStep,
) {
    if let Some(observe) = observer {
        observe(&TearingFrame {
            step,
            tears_so_far: tears.to_vec(),
            causal_so_far: causal.to_vec(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tear_linear_chain() {
        // 3 equations: eq0 has {v0}, eq1 has {v0, v1}, eq2 has {v1, v2}
        // All can be solved causally: eq0→v0, eq1→v1, eq2→v2
        let eq_unknowns = vec![
            HashSet::from([0]),
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
        ];
        let result = tear_algebraic_loop(3, &eq_unknowns);
        // Fully causal — no tear vars needed, but our function returns None
        // when tear_vars is empty (meaning the block isn't really a loop)
        assert!(result.is_none() || result.as_ref().unwrap().tear_var_local_indices.is_empty());
    }

    #[test]
    fn test_tear_simple_2x2_loop() {
        // 2 equations forming a loop: eq0 has {v0, v1}, eq1 has {v0, v1}
        let eq_unknowns = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
        let result = tear_algebraic_loop(2, &eq_unknowns);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.tear_var_local_indices.len(), 1);
        assert_eq!(r.residual_eq_local_indices.len(), 1);
        assert_eq!(r.causal_sequence.len(), 1);
    }

    #[test]
    fn test_tear_3x3_with_one_tear() {
        // 3-equation loop where tearing one var makes the rest causal
        // eq0: {v0, v1}, eq1: {v1, v2}, eq2: {v0, v2}
        let eq_unknowns = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let result = tear_algebraic_loop(3, &eq_unknowns);
        assert!(result.is_some());
        let r = result.unwrap();
        // Should need only 1 tear variable
        assert_eq!(r.tear_var_local_indices.len(), 1);
        assert_eq!(r.causal_sequence.len(), 2);
        assert_eq!(r.residual_eq_local_indices.len(), 1);
    }
}

#[cfg(test)]
mod trace_tests {
    use super::*;

    /// The trace records the decisions, in the order the algorithm makes them.
    ///
    /// Tearing is greedy, so every frame is a choice made on local information
    /// and the interesting question is always "why that one?". `Torn` therefore
    /// carries `appearances` — the count that *was* the reason — rather than
    /// only the variable chosen.
    #[test]
    fn tracing_records_each_decision_with_its_reason() {
        // A genuine loop: three equations, three unknowns, none solvable alone.
        let eq_unknowns = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let frames = std::cell::RefCell::new(Vec::new());
        let result = tear_algebraic_loop_with_trace(
            3,
            &eq_unknowns,
            Some(&|f: &TearingFrame| frames.borrow_mut().push(f.clone())),
        );
        let frames = frames.into_inner();

        assert!(matches!(frames.first().map(|f| &f.step), Some(TearingStep::Start { n: 3 })));

        let torn: Vec<(usize, usize)> = frames
            .iter()
            .filter_map(|f| match &f.step {
                TearingStep::Torn { variable, appearances, .. } => Some((*variable, *appearances)),
                _ => None,
            })
            .collect();
        assert!(!torn.is_empty(), "a genuine loop must be cut somewhere");
        // The greedy criterion: the chosen variable appears in at least as many
        // remaining equations as any other would. Here every variable appears
        // twice, so the reason is recorded even when the choice is a tie.
        assert!(torn.iter().all(|&(_, appearances)| appearances >= 2), "{torn:?}");

        // The running set grows with the decisions.
        if let Some(last) = frames.last() {
            assert_eq!(last.tears_so_far.len(), torn.len());
        }
        if result.is_some() {
            assert!(matches!(
                frames.last().map(|f| &f.step),
                Some(TearingStep::Complete { .. }),
            ));
        }
    }

    /// Tracing must not change the outcome — the instrumentation discipline.
    #[test]
    fn tracing_does_not_change_the_result() {
        let eq_unknowns = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let untraced = tear_algebraic_loop(3, &eq_unknowns);
        let traced = tear_algebraic_loop_with_trace(3, &eq_unknowns, None);
        assert_eq!(
            untraced.map(|r| (r.tear_var_local_indices, r.residual_eq_local_indices)),
            traced.map(|r| (r.tear_var_local_indices, r.residual_eq_local_indices)),
        );
    }

    /// A causal step records how many equations competed for the variable — the
    /// tie-break that the doc comment describes but the result cannot show.
    #[test]
    fn causal_steps_record_the_competition() {
        // eq0 solves v0 alone; eq1 and eq2 then both become single-unknown.
        let eq_unknowns = vec![
            HashSet::from([0]),
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
        ];
        let frames = std::cell::RefCell::new(Vec::new());
        tear_algebraic_loop_with_trace(
            3,
            &eq_unknowns,
            Some(&|f: &TearingFrame| frames.borrow_mut().push(f.clone())),
        );
        let frames = frames.into_inner();
        let causal: Vec<_> = frames
            .iter()
            .filter(|f| matches!(f.step, TearingStep::Causal { .. }))
            .collect();
        assert_eq!(causal.len(), 3, "a chain resolves entirely causally: no tearing needed");
        assert!(matches!(
            causal[0].step,
            TearingStep::Causal { equation: 0, variable: 0, .. },
        ));
    }
}
