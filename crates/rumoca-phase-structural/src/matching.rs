//! Maximum matching via augmenting paths (Kuhn's algorithm).

use std::collections::HashSet;

use rumoca_core::FrameObserver;

/// One step of the augmenting-path algorithm, recorded for animation replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchingStep {
    /// **Before the search begins: nothing matched, and nothing tried yet.**
    ///
    /// A trace that opened on [`Self::TryEquation`] began with an intention
    /// already announced, so a replay had no frame describing the problem as the
    /// algorithm found it — the one thing a replay needs in order to show what
    /// the algorithm *changed*. The same gap was closed for index-reduction
    /// traces by [`crate::dae_prepare::emit_index_reduction_start`]; this is that
    /// fix for matching.
    ///
    /// The dimensions are carried rather than left to the consumer to supply,
    /// because a consumer that computes them from elsewhere is describing a
    /// different run than the one it is drawing.
    ///
    /// The frame's `match_eq` is all-`None`, which is not padding: it is the
    /// genuine state of the matching at this instant.
    Start {
        n_equations: usize,
        n_unknowns: usize,
    },
    /// Starting augmenting-path search from an unmatched equation.
    TryEquation(usize),
    /// Exploring edge (equation, variable) — is the variable free or matched?
    Explore { eq: usize, var: usize },
    /// Variable is free: augmenting path found.
    FoundFree { eq: usize, var: usize },
    /// Variable is matched to `holder`; recursing to try to displace it.
    TryDisplace {
        eq: usize,
        var: usize,
        holder: usize,
    },
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

thread_local! {
    /// Ambient capture buffer: `Some` while a capture scope is open.
    static CAPTURE: std::cell::RefCell<Option<Vec<MatchingFrame>>> =
        const { std::cell::RefCell::new(None) };
}

/// **Begin capturing matching frames on this thread.**
///
/// [`maximum_matching_with_trace`] is the right tool when the caller runs matching
/// itself. But `build_structural_report` runs it *internally* and returns only the
/// result, so a tool that wants to animate the search has to **run matching a second
/// time** on the same incidence matrix.
///
/// That re-derivation is deterministic, so it agrees — but it agrees by luck of the
/// algorithm rather than by construction, and the frames then describe a run that
/// produced nothing. The matching a reader is shown and the matching the blocks were
/// built from are two different executions with nothing tying them together.
///
/// With a scope open, [`crate::build_structural_report`] traces its own run and
/// deposits the frames here. Same shape as the captures in `rumoca-phase-flatten`
/// and `rumoca-phase-dae`.
///
/// # Cost
///
/// **Nothing when closed** — the untraced path still runs, and no frame is built.
/// While open, matching takes the traced path and retains one frame per step, which
/// is why a scope should wrap a model being *looked at* rather than a corpus sweep.
/// [`take_capture`] drains and closes.
pub fn start_capture() {
    CAPTURE.with(|c| *c.borrow_mut() = Some(Vec::new()));
}

/// Take the captured frames and close the scope.
pub fn take_capture() -> Vec<MatchingFrame> {
    CAPTURE.with(|c| c.borrow_mut().take()).unwrap_or_default()
}

/// Whether a capture scope is open, so callers can pick the traced path.
pub(crate) fn capturing() -> bool {
    CAPTURE.with(|c| c.borrow().is_some())
}

pub(crate) fn deposit_capture(frames: Vec<MatchingFrame>) {
    CAPTURE.with(|c| {
        if let Some(buf) = c.borrow_mut().as_mut() {
            *buf = frames;
        }
    });
}

/// Like `maximum_matching`, but records every algorithmic step for animation.
///
/// When `observer` is `Some`, each frame is handed to it as it is produced —
/// the hook a live, debugger-stepped session parks on. See
/// [`rumoca_core::FrameObserver`] for why this is a callback rather than a
/// concrete tracer type.
pub fn maximum_matching_with_trace(
    n_eq: usize,
    n_var: usize,
    eq_vars: &[HashSet<usize>],
    observer: Option<FrameObserver<'_, MatchingFrame>>,
) -> MatchingTraceResult {
    let mut match_eq: Vec<Option<usize>> = vec![None; n_eq];
    let mut match_var: Vec<Option<usize>> = vec![None; n_var];
    let mut frames = Vec::new();

    // The opening frame. Emitted here rather than by each caller because every
    // traced path — the report's own run, a rebuild for playback, and a live
    // debugger session — enters through this function, so this is the one place
    // that cannot disagree with itself about where the search began.
    emit_matching_frame(
        &mut frames,
        observer,
        MatchingFrame {
            step: MatchingStep::Start {
                n_equations: n_eq,
                n_unknowns: n_var,
            },
            match_eq: match_eq.clone(),
        },
    );

    for eq in 0..n_eq {
        emit_matching_frame(
            &mut frames,
            observer,
            MatchingFrame {
                step: MatchingStep::TryEquation(eq),
                match_eq: match_eq.clone(),
            },
        );
        let mut visited = vec![false; n_var];
        let found = augment_traced(
            eq,
            &mut match_eq,
            &mut match_var,
            eq_vars,
            &mut visited,
            &mut frames,
            observer,
        );
        if !found {
            emit_matching_frame(
                &mut frames,
                observer,
                MatchingFrame {
                    step: MatchingStep::EquationFailed(eq),
                    match_eq: match_eq.clone(),
                },
            );
        }
    }

    MatchingTraceResult {
        match_eq,
        match_var,
        frames,
    }
}

/// Push a frame to the replay vec and, if anyone is watching, to the observer.
fn emit_matching_frame(
    frames: &mut Vec<MatchingFrame>,
    observer: Option<FrameObserver<'_, MatchingFrame>>,
    frame: MatchingFrame,
) {
    // By reference, so an untraced run never clones and a watching one clones
    // only if it decides to keep the frame.
    if let Some(observe) = observer {
        observe(&frame);
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
    observer: Option<FrameObserver<'_, MatchingFrame>>,
) -> bool {
    let mut vars: Vec<usize> = eq_vars[eq].iter().copied().collect();
    vars.sort_unstable();
    for var in vars {
        if visited[var] {
            continue;
        }
        visited[var] = true;
        emit_matching_frame(
            frames,
            observer,
            MatchingFrame {
                step: MatchingStep::Explore { eq, var },
                match_eq: match_eq.to_vec(),
            },
        );
        let can_augment = match match_var[var] {
            None => {
                emit_matching_frame(
                    frames,
                    observer,
                    MatchingFrame {
                        step: MatchingStep::FoundFree { eq, var },
                        match_eq: match_eq.to_vec(),
                    },
                );
                true
            }
            Some(holder) => {
                emit_matching_frame(
                    frames,
                    observer,
                    MatchingFrame {
                        step: MatchingStep::TryDisplace { eq, var, holder },
                        match_eq: match_eq.to_vec(),
                    },
                );
                let ok = augment_traced(
                    holder, match_eq, match_var, eq_vars, visited, frames, observer,
                );
                emit_matching_frame(
                    frames,
                    observer,
                    MatchingFrame {
                        step: if ok {
                            MatchingStep::DisplaceOk { eq, var }
                        } else {
                            MatchingStep::DisplaceFail { eq, var }
                        },
                        match_eq: match_eq.to_vec(),
                    },
                );
                ok
            }
        };
        if !can_augment {
            continue;
        }
        match_eq[eq] = Some(var);
        match_var[var] = Some(eq);
        emit_matching_frame(
            frames,
            observer,
            MatchingFrame {
                step: MatchingStep::Assign { eq, var },
                match_eq: match_eq.to_vec(),
            },
        );
        return true;
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

    /// **A capture scope records, and taking it closes.**
    ///
    /// The closing half is what stops one model's search appearing under the next
    /// model's animation — a wrong picture with no symptom, which is the failure
    /// mode this whole capture exists to remove rather than introduce.
    #[test]
    fn a_matching_capture_scope_records_and_closes_on_take() {
        assert!(!capturing(), "no scope is open to begin with");
        assert!(take_capture().is_empty(), "and taking yields nothing");

        start_capture();
        assert!(
            capturing(),
            "the scope is open, so callers take the traced path"
        );
        deposit_capture(vec![MatchingFrame {
            step: MatchingStep::TryEquation(0),
            match_eq: vec![None],
        }]);
        let got = take_capture();
        assert_eq!(got.len(), 1, "the deposited frames come back");

        assert!(!capturing(), "taking closed the scope");
        assert!(
            take_capture().is_empty(),
            "capture continued after take \u{2014} one model's search would animate \
             under the next model's view",
        );
    }

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

    /// The trace opens on the problem, then announces the first attempt.
    ///
    /// Both halves matter. The opening frame must describe the system *before*
    /// the search — carrying the real dimensions and an empty matching — and the
    /// frame after it must be the first attempt, or the opening frame has
    /// replaced information rather than added it.
    #[test]
    fn trace_starts_before_the_search_then_tries_the_first_equation() {
        let eq_vars = vec![HashSet::from([0])];
        let traced = maximum_matching_with_trace(1, 1, &eq_vars, None);

        assert!(matches!(
            traced.frames[0].step,
            MatchingStep::Start {
                n_equations: 1,
                n_unknowns: 1
            }
        ));
        assert!(
            traced.frames[0].match_eq.iter().all(Option::is_none),
            "nothing is matched before the search runs"
        );
        assert!(matches!(
            traced.frames[1].step,
            MatchingStep::TryEquation(0)
        ));
    }

    /// The opening frame reaches a live observer, not just the returned buffer.
    ///
    /// A debugger-stepped session and a recorded replay must begin on the same
    /// frame; emitting the opening one only into `frames` would give the live
    /// path a different first step, which is the inconsistency this whole change
    /// exists to remove.
    #[test]
    fn the_opening_frame_reaches_an_observer() {
        let eq_vars = vec![HashSet::from([0])];
        let seen = std::cell::RefCell::new(Vec::new());
        let observe = |f: &MatchingFrame| seen.borrow_mut().push(f.step.clone());
        let _ = maximum_matching_with_trace(1, 1, &eq_vars, Some(&observe));

        assert!(
            matches!(
                seen.borrow().first(),
                Some(MatchingStep::Start {
                    n_equations: 1,
                    n_unknowns: 1
                })
            ),
            "observed: {:?}",
            seen.borrow(),
        );
    }

    #[test]
    fn trace_contains_assign_for_each_matched_pair() {
        let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
        let traced = maximum_matching_with_trace(2, 2, &eq_vars, None);
        let assigns: Vec<_> = traced
            .frames
            .iter()
            .filter(|f| matches!(f.step, MatchingStep::Assign { .. }))
            .collect();
        assert_eq!(assigns.len(), 2 + 1); // 2 original + 1 re-assignment from displacement
    }

    #[test]
    fn trace_records_displacement_on_conflict() {
        // eq0 and eq1 both want var0; eq0 gets it first, then eq1 displaces eq0
        let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0])];
        let traced = maximum_matching_with_trace(2, 2, &eq_vars, None);
        let displacements: Vec<_> = traced
            .frames
            .iter()
            .filter(|f| matches!(f.step, MatchingStep::TryDisplace { .. }))
            .collect();
        assert!(
            !displacements.is_empty(),
            "should record displacement attempt"
        );
    }

    #[test]
    fn trace_records_equation_failed_when_unmatched() {
        let eq_vars = vec![HashSet::from([0]), HashSet::from([0])];
        let traced = maximum_matching_with_trace(2, 1, &eq_vars, None);
        let failures: Vec<_> = traced
            .frames
            .iter()
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
        // The observer must see exactly what the returned buffer holds: a live
        // session steps the observer, and playback afterwards reads the buffer,
        // so a divergence would make stepping and replaying the same run show
        // different things.
        let observed = std::cell::RefCell::new(Vec::new());
        let traced = maximum_matching_with_trace(
            3,
            3,
            &eq_vars,
            Some(&|f: &MatchingFrame| observed.borrow_mut().push(f.clone())),
        );
        let observed = observed.into_inner();
        assert_eq!(traced.frames.len(), observed.len());
        for (i, (ret, live)) in traced.frames.iter().zip(observed.iter()).enumerate() {
            assert_eq!(ret.step, live.step, "frame {i} step mismatch");
            assert_eq!(ret.match_eq, live.match_eq, "frame {i} match_eq mismatch");
        }
    }
}
