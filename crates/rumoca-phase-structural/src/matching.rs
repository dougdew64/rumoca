//! Maximum matching via augmenting paths (Kuhn's algorithm).

use std::collections::HashSet;

use crate::live_trace::LiveTrace;

/// One step of the augmenting-path algorithm, recorded for animation replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchingStep {
    /// Starting augmenting-path search from an unmatched equation.
    TryEquation(usize),
    /// Exploring edge (equation, variable) — is the variable free or matched?
    Explore { eq: usize, var: usize },
    /// Variable is free: augmenting path found.
    FoundFree { eq: usize, var: usize },
    /// Variable is matched to `holder`; recursing to try to displace it.
    TryDisplace { eq: usize, var: usize, holder: usize },
    /// Displacement succeeded: `holder` found an alternative.
    DisplaceOk { eq: usize, var: usize },
    /// Displacement failed: `holder` has no alternative; backtracking.
    DisplaceFail { eq: usize, var: usize },
    /// Recording the match: equation `eq` now owns variable `var`.
    Assign { eq: usize, var: usize },
    /// Equation has no augmenting path — it will remain unmatched.
    EquationFailed(usize),
}

/// Snapshot of the matching state at each step, for animation rendering.
#[derive(Debug, Clone)]
pub struct MatchingFrame {
    pub step: MatchingStep,
    /// Current (partial) matching: `match_eq[i] = Some(j)` means eq i matched to var j.
    pub match_eq: Vec<Option<usize>>,
}

/// Result of `maximum_matching_with_trace`: the final matching plus the frame sequence.
pub struct MatchingTraceResult {
    pub match_eq: Vec<Option<usize>>,
    pub match_var: Vec<Option<usize>>,
    pub frames: Vec<MatchingFrame>,
}

/// Like `maximum_matching`, but records every algorithmic step for animation.
///
/// When `live` is `Some`, each frame is also pushed to the shared
/// [`LiveTrace`] buffer — set a debugger breakpoint on [`LiveTrace::push`]
/// to single-step through the algorithm while a UI thread renders each
/// frame as it arrives.
pub fn maximum_matching_with_trace(
    n_eq: usize,
    n_var: usize,
    eq_vars: &[HashSet<usize>],
    live: Option<&LiveTrace<MatchingFrame>>,
) -> MatchingTraceResult {
    let mut match_eq: Vec<Option<usize>> = vec![None; n_eq];
    let mut match_var: Vec<Option<usize>> = vec![None; n_var];
    let mut frames = Vec::new();

    for eq in 0..n_eq {
        emit_matching_frame(&mut frames, live, MatchingFrame {
            step: MatchingStep::TryEquation(eq),
            match_eq: match_eq.clone(),
        });
        let mut visited = vec![false; n_var];
        let found = augment_traced(
            eq,
            &mut match_eq,
            &mut match_var,
            eq_vars,
            &mut visited,
            &mut frames,
            live,
        );
        if !found {
            emit_matching_frame(&mut frames, live, MatchingFrame {
                step: MatchingStep::EquationFailed(eq),
                match_eq: match_eq.clone(),
            });
        }
    }

    MatchingTraceResult { match_eq, match_var, frames }
}

/// Push a frame to the local vec and, if present, to the live trace buffer.
fn emit_matching_frame(
    frames: &mut Vec<MatchingFrame>,
    live: Option<&LiveTrace<MatchingFrame>>,
    frame: MatchingFrame,
) {
    if let Some(lt) = live {
        lt.push(frame.clone());
    }
    frames.push(frame);
}

fn augment_traced(
    eq: usize,
    match_eq: &mut [Option<usize>],
    match_var: &mut [Option<usize>],
    eq_vars: &[HashSet<usize>],
    visited: &mut [bool],
    frames: &mut Vec<MatchingFrame>,
    live: Option<&LiveTrace<MatchingFrame>>,
) -> bool {
    let mut vars: Vec<usize> = eq_vars[eq].iter().copied().collect();
    vars.sort_unstable();
    for var in vars {
        if !visited[var] {
            visited[var] = true;
            emit_matching_frame(frames, live, MatchingFrame {
                step: MatchingStep::Explore { eq, var },
                match_eq: match_eq.to_vec(),
            });
            let can_augment = match match_var[var] {
                None => {
                    emit_matching_frame(frames, live, MatchingFrame {
                        step: MatchingStep::FoundFree { eq, var },
                        match_eq: match_eq.to_vec(),
                    });
                    true
                }
                Some(holder) => {
                    emit_matching_frame(frames, live, MatchingFrame {
                        step: MatchingStep::TryDisplace { eq, var, holder },
                        match_eq: match_eq.to_vec(),
                    });
                    let ok = augment_traced(holder, match_eq, match_var, eq_vars, visited, frames, live);
                    if ok {
                        emit_matching_frame(frames, live, MatchingFrame {
                            step: MatchingStep::DisplaceOk { eq, var },
                            match_eq: match_eq.to_vec(),
                        });
                    } else {
                        emit_matching_frame(frames, live, MatchingFrame {
                            step: MatchingStep::DisplaceFail { eq, var },
                            match_eq: match_eq.to_vec(),
                        });
                    }
                    ok
                }
            };
            if can_augment {
                match_eq[eq] = Some(var);
                match_var[var] = Some(eq);
                emit_matching_frame(frames, live, MatchingFrame {
                    step: MatchingStep::Assign { eq, var },
                    match_eq: match_eq.to_vec(),
                });
                return true;
            }
        }
    }
    false
}

/// Find maximum matching in a bipartite graph using augmenting paths.
///
/// Returns `(match_eq, match_var)` where:
/// - `match_eq[i] = Some(j)` means equation `i` is matched to variable `j`
/// - `match_var[j] = Some(i)` means variable `j` is matched to equation `i`
pub fn maximum_matching(
    n_eq: usize,
    n_var: usize,
    eq_vars: &[HashSet<usize>],
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let mut match_eq: Vec<Option<usize>> = vec![None; n_eq];
    let mut match_var: Vec<Option<usize>> = vec![None; n_var];

    for eq in 0..n_eq {
        let mut visited = vec![false; n_var];
        augment(eq, &mut match_eq, &mut match_var, eq_vars, &mut visited);
    }

    (match_eq, match_var)
}

/// Try to find an augmenting path starting from an unmatched equation.
fn augment(
    eq: usize,
    match_eq: &mut [Option<usize>],
    match_var: &mut [Option<usize>],
    eq_vars: &[HashSet<usize>],
    visited: &mut [bool],
) -> bool {
    // Deterministic traversal is critical for reproducible BLT/matching.
    // HashSet iteration order is process-random and can otherwise change
    // structural choices between runs.
    let mut vars: Vec<usize> = eq_vars[eq].iter().copied().collect();
    vars.sort_unstable();
    for var in vars {
        if !visited[var] {
            visited[var] = true;
            let can_augment = match match_var[var] {
                None => true,
                Some(matched_eq) => augment(matched_eq, match_eq, match_var, eq_vars, visited),
            };
            if can_augment {
                match_eq[eq] = Some(var);
                match_var[var] = Some(eq);
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_maximum_matching_perfect() {
        let eq_vars = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let (match_eq, _match_var) = maximum_matching(3, 3, &eq_vars);
        let size = match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(size, 3, "should find perfect matching");
    }

    #[test]
    fn test_maximum_matching_imperfect() {
        let eq_vars = vec![
            HashSet::from([0]),
            HashSet::from([0]),
            HashSet::from([1, 2]),
        ];
        let (match_eq, _match_var) = maximum_matching(3, 3, &eq_vars);
        let size = match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(size, 2, "imperfect matching: two equations compete for v0");
    }

    #[test]
    fn test_maximum_matching_is_deterministic_under_ties() {
        let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
        let (match_eq, match_var) = maximum_matching(2, 2, &eq_vars);
        assert_eq!(match_eq, vec![Some(1), Some(0)]);
        assert_eq!(match_var, vec![Some(1), Some(0)]);
    }

    #[test]
    fn traced_matching_produces_same_result() {
        let eq_vars = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let (match_eq, _) = maximum_matching(3, 3, &eq_vars);
        let traced = maximum_matching_with_trace(3, 3, &eq_vars, None);
        assert_eq!(match_eq, traced.match_eq);
    }

    #[test]
    fn trace_starts_with_try_equation() {
        let eq_vars = vec![HashSet::from([0])];
        let traced = maximum_matching_with_trace(1, 1, &eq_vars, None);
        assert!(matches!(traced.frames[0].step, MatchingStep::TryEquation(0)));
    }

    #[test]
    fn trace_contains_assign_for_each_matched_pair() {
        let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
        let traced = maximum_matching_with_trace(2, 2, &eq_vars, None);
        let assigns: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, MatchingStep::Assign { .. }))
            .collect();
        assert_eq!(assigns.len(), 2 + 1); // 2 original + 1 re-assignment from displacement
    }

    #[test]
    fn trace_records_displacement_on_conflict() {
        // eq0 and eq1 both want var0; eq0 gets it first, then eq1 displaces eq0
        let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0])];
        let traced = maximum_matching_with_trace(2, 2, &eq_vars, None);
        let displacements: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, MatchingStep::TryDisplace { .. }))
            .collect();
        assert!(!displacements.is_empty(), "should record displacement attempt");
    }

    #[test]
    fn trace_records_equation_failed_when_unmatched() {
        let eq_vars = vec![HashSet::from([0]), HashSet::from([0])];
        let traced = maximum_matching_with_trace(2, 1, &eq_vars, None);
        let failures: Vec<_> = traced.frames.iter()
            .filter(|f| matches!(f.step, MatchingStep::EquationFailed(_)))
            .collect();
        assert_eq!(failures.len(), 1, "one equation should fail to match");
    }

    #[test]
    fn live_trace_receives_same_frames_as_returned() {
        let eq_vars = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let lt = LiveTrace::new();
        let traced = maximum_matching_with_trace(3, 3, &eq_vars, Some(&lt));
        let live_frames = lt.snapshot();
        assert_eq!(traced.frames.len(), live_frames.len());
        for (i, (ret, live)) in traced.frames.iter().zip(live_frames.iter()).enumerate() {
            assert_eq!(ret.step, live.step, "frame {i} step mismatch");
            assert_eq!(ret.match_eq, live.match_eq, "frame {i} match_eq mismatch");
        }
    }
}
