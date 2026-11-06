//! Observation-only seam for maximum matching, Tarjan's SCC search and
//! tearing.
//!
//! Every borrowed type here names data the algorithm already computed to make
//! its own decision; nothing is computed for observation's sake. `observe`
//! returns `()` and its argument only borrows, so an observer cannot feed
//! anything back into the search it is watching -- that is a type-level
//! guarantee, not a promise. `()` is the production observer: its `observe`
//! body is empty, so [`crate::sort`] allocates and clones nothing for this
//! seam. [`StructuralRecorder`] is the one observer that does real work,
//! turning each borrowed event into an owned [`StructuralRecord`] for
//! [`crate::inspect_sort`], the sole crate-external surface. The trait and the
//! borrowed event types stay private to this crate; consumers receive only
//! extracted data, never the search protocol.
//!
//! This is deliberately the same shape as
//! [`crate::dae_transform::observation`], which observes index reduction. One
//! algorithm, one observer parameter, and no ambient state: a traced run and an
//! untraced run execute the *same* code, so a recorded step sequence always
//! describes the run that produced the result beside it.
//!
//! # What the three step vocabularies do and do not claim
//!
//! **Matching** reports Kuhn's algorithm at edge granularity -- every variable
//! considered, every displacement attempted, every assignment made -- plus two
//! phases that bracket it. [`MatchingStep::SeedStructured`] reports a row
//! matched straight from a regular-family descriptor before Kuhn runs at all;
//! without it a replay would open on equations already matched for no visible
//! reason. The preference phase
//! ([`MatchingStep::ClaimedPreference`] /
//! [`MatchingStep::PreferenceUnavailable`]) is reported at **decision**
//! granularity, one step per preferred edge, not at edge granularity: its
//! relocation search is a second depth-first search whose interior is not
//! reported. The accompanying `match_eq` is exact either way, so a replay
//! never shows a matching that did not exist -- it shows this phase advancing
//! in larger jumps than Kuhn's.
//!
//! **Tarjan** reports one traversal per [`TarjanStep::Start`]. Node indices are
//! only meaningful against that `Start`'s `equation_of_node`: when compact
//! equation families are condensed out of the node set, Tarjan runs over a
//! renumbered graph, and reading those indices as equation indices would name
//! the wrong rows.
//!
//! **Tearing** reports one pass per [`TearingStep::PassStart`]. One algebraic
//! loop produces *up to two* passes, because the planner runs the greedy search
//! under both tear priorities and keeps the smaller tear set;
//! [`TearingStep::PlanSelected`] names the priority whose pass was adopted.
//! Reading an unselected pass as the plan would describe a tear sequence the
//! compiler discarded.

/// Which tear-selection priority one tearing pass ran under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TearPriorityKind {
    /// Prefer the variable whose removal makes the most equations causal.
    CausalUnlocks,
    /// Prefer the variable appearing in the most remaining equations.
    VariableDegree,
}

/// One step of the augmenting-path algorithm (Kuhn's).
///
/// Every variant carries only indices the matching code already held, so this
/// enum is owned without borrowing and needs no twin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MatchingStep {
    /// **Before anything is matched: nothing seeded, nothing tried.**
    ///
    /// A trace that opened on [`Self::TryEquation`] began with an intention
    /// already announced, so a replay had no frame describing the problem as
    /// the algorithm found it -- the one thing a replay needs in order to show
    /// what the algorithm *changed*.
    ///
    /// The dimensions are carried rather than left to the consumer to supply,
    /// because a consumer that computes them from elsewhere is describing a
    /// different run than the one it is drawing.
    ///
    /// The accompanying `match_eq` is all-`None`, which is not padding: it is
    /// the genuine state of the matching at this instant.
    Start {
        n_equations: usize,
        n_unknowns: usize,
    },
    /// A row matched directly from a regular-family descriptor, before Kuhn
    /// runs. These rows are then skipped by the augmenting search entirely.
    SeedStructured { eq: usize, var: usize },
    /// Starting an augmenting-path search from an unmatched equation.
    TryEquation(usize),
    /// Exploring edge (equation, variable) -- is the variable free or matched?
    Explore { eq: usize, var: usize },
    /// Variable is free: augmenting path found.
    FoundFree { eq: usize, var: usize },
    /// Variable is matched to `holder`; descending to try to displace it.
    TryDisplace {
        eq: usize,
        var: usize,
        holder: usize,
    },
    /// Displacement succeeded: the holder found an alternative.
    DisplaceOk { eq: usize, var: usize },
    /// Displacement failed: the holder has no alternative; backtracking.
    DisplaceFail { eq: usize, var: usize },
    /// Recording the match: equation `eq` now owns variable `var`.
    Assign { eq: usize, var: usize },
    /// Equation has no augmenting path -- it will remain unmatched.
    EquationFailed(usize),
    /// The preference phase moved `eq` onto its causal target `var`, which no
    /// earlier equation had locked. Reported per decision, not per edge.
    ClaimedPreference { eq: usize, var: usize },
    /// The preference phase could not give `eq` its causal target `var`
    /// without reducing the matching's cardinality, so the matching is
    /// unchanged.
    PreferenceUnavailable { eq: usize, var: usize },
}

/// One step of Tarjan's SCC search, borrowed for the duration of one `observe`
/// call. [`TarjanStepRecord`] is its owned twin.
pub(crate) enum TarjanStep<'a> {
    /// **Before the traversal begins: no node visited, no component closed.**
    ///
    /// The counterpart of [`MatchingStep::Start`], for the same reason, and
    /// additionally the only place the node numbering is stated.
    ///
    /// `equation_of_node` is `Some` when compact equation families were
    /// condensed out of the node set, in which case node `i` is equation
    /// `equation_of_node[i]`; `None` means nodes *are* equation indices.
    Start {
        n_nodes: usize,
        equation_of_node: Option<&'a [usize]>,
    },
    /// Visiting node `v` for the first time (index and lowlink assigned).
    Visit(usize),
    /// Exploring edge from → to.
    ExploreEdge { from: usize, to: usize },
    /// The edge leads to an unvisited node -- descending.
    TreeEdge { from: usize, to: usize },
    /// The edge leads to an on-stack node -- back edge, lowlink updated.
    BackEdge { from: usize, to: usize },
    /// Returning from `to`, updating `from`'s lowlink.
    Return { from: usize, to: usize },
    /// SCC found, rooted at `root` and containing `members`.
    SccFound { root: usize, members: &'a [usize] },
}

/// The owned twin of [`TarjanStep`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TarjanStepRecord {
    Start {
        n_nodes: usize,
        equation_of_node: Option<Vec<usize>>,
    },
    Visit(usize),
    ExploreEdge {
        from: usize,
        to: usize,
    },
    TreeEdge {
        from: usize,
        to: usize,
    },
    BackEdge {
        from: usize,
        to: usize,
    },
    Return {
        from: usize,
        to: usize,
    },
    SccFound {
        root: usize,
        members: Vec<usize>,
    },
}

impl From<TarjanStep<'_>> for TarjanStepRecord {
    fn from(step: TarjanStep<'_>) -> Self {
        match step {
            TarjanStep::Start {
                n_nodes,
                equation_of_node,
            } => Self::Start {
                n_nodes,
                equation_of_node: equation_of_node.map(<[usize]>::to_vec),
            },
            TarjanStep::Visit(node) => Self::Visit(node),
            TarjanStep::ExploreEdge { from, to } => Self::ExploreEdge { from, to },
            TarjanStep::TreeEdge { from, to } => Self::TreeEdge { from, to },
            TarjanStep::BackEdge { from, to } => Self::BackEdge { from, to },
            TarjanStep::Return { from, to } => Self::Return { from, to },
            TarjanStep::SccFound { root, members } => Self::SccFound {
                root,
                members: members.to_vec(),
            },
        }
    }
}

/// One step of greedy Cellier-style tearing.
///
/// Tearing is a *greedy* algorithm, and greedy algorithms are exactly the kind
/// worth watching: every step is a choice made on local information, and the
/// interesting question is always "why that one?". Each variant carries the
/// reason, not just the outcome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TearingStep {
    /// A coupled BLT block is about to be considered for tearing.
    ///
    /// **Emitted for every coupled block, including ones tearing never runs
    /// on**, which is what makes it the block identity rather than a
    /// convenience. A block whose SCC is not square is rejected before the
    /// greedy search starts and so contributes no [`Self::PassStart`] at all;
    /// a consumer inferring the block from the *position* of a pass group
    /// would then attribute every later block's tears to the wrong rows.
    /// `ordinal` counts coupled blocks in BLT order from zero.
    BlockStart {
        ordinal: usize,
        equations: usize,
        unknowns: usize,
    },
    /// One pass begins, under `priority`, over a loop of `n` equations.
    PassStart {
        priority: TearPriorityKind,
        n: usize,
    },
    /// An equation had exactly one unknown left, so it can be solved directly
    /// -- no iteration needed. `competitors` is how many equations could have
    /// solved for this variable; when it is more than one, the tie was broken
    /// by preferring the equation with fewer total unknowns.
    Causal {
        equation: usize,
        variable: usize,
        competitors: usize,
    },
    /// No equation has a single remaining unknown, so the loop must be *cut*.
    /// The chosen variable appears in `appearances` of the remaining
    /// equations, which under [`TearPriorityKind::VariableDegree`] is the most
    /// of any candidate and is the greedy criterion.
    Torn {
        variable: usize,
        appearances: usize,
        remaining_equations: usize,
    },
    /// This pass finished. `residuals` equations remain, driven by the solver
    /// against `tears` guessed variables -- that pair is the size of the
    /// iteration the solver is left with, and the whole point of the exercise.
    Complete { tears: usize, residuals: usize },
    /// This pass produced no plan the compiler can use, for one of three
    /// reasons: no remaining unknown was left to cut (every equation
    /// references every unknown), the tear set did not shrink the problem, or
    /// the tear and residual counts disagreed. The step is emitted at each of
    /// them rather than left as the absence of a [`Self::Complete`], because
    /// an absence is indistinguishable from a step that was never reported.
    NoProgress,
    /// The plan this loop actually adopted, fired once per loop after both
    /// priorities have been compared. The pass whose `PassStart` named this
    /// priority is the one describing the compiler's plan.
    PlanSelected {
        priority: TearPriorityKind,
        tears: usize,
        residuals: usize,
    },
}

/// One fact the search already computed, borrowed for the duration of one
/// `observe` call. The running state travels beside the step because that is
/// what makes a step renderable: a consumer that reconstructs the state is
/// describing a different run than the one it is drawing.
pub(crate) enum StructuralEvent<'a> {
    Matching {
        step: MatchingStep,
        /// The matching as of this instant: `match_eq[i] = Some(j)` means
        /// equation `i` owns variable `j`.
        match_eq: &'a [Option<usize>],
    },
    Tarjan {
        step: TarjanStep<'a>,
        /// Nodes currently on the Tarjan stack.
        stack: &'a [usize],
        /// Components closed so far, in the order they closed.
        sccs_so_far: &'a [Vec<usize>],
    },
    Tearing {
        step: TearingStep,
        /// Tear variables chosen so far in this pass, in order.
        tears_so_far: &'a [usize],
        /// Equations solved causally so far in this pass, as
        /// `(equation, variable)`.
        causal_so_far: &'a [(usize, usize)],
    },
}

/// The read-only seam production and diagnostic code share.
///
/// `observe` returns `()` and its argument is built entirely from data the
/// caller already had in hand, so an implementation cannot influence which
/// variable is tried next, which equation is matched, or what the search
/// returns -- the trait admits no channel back into the search it watches.
pub(crate) trait StructuralObserver {
    fn observe(&mut self, event: StructuralEvent<'_>);
}

impl StructuralObserver for () {
    fn observe(&mut self, _event: StructuralEvent<'_>) {}
}

/// One recorded matching step and the matching it left behind.
#[derive(Clone, Debug)]
pub struct MatchingRecord {
    pub step: MatchingStep,
    pub match_eq: Vec<Option<usize>>,
}

/// One recorded Tarjan step and the traversal state it left behind.
#[derive(Clone, Debug)]
pub struct TarjanRecord {
    pub step: TarjanStepRecord,
    pub stack: Vec<usize>,
    pub sccs_so_far: Vec<Vec<usize>>,
}

/// One recorded tearing step and the plan it left behind.
#[derive(Clone, Debug)]
pub struct TearingRecord {
    pub step: TearingStep,
    pub tears_so_far: Vec<usize>,
    pub causal_so_far: Vec<(usize, usize)>,
}

/// One recorded fact from a traced [`crate::inspect_sort`] call, the owned twin
/// of [`StructuralEvent`].
#[derive(Clone, Debug)]
pub enum StructuralRecord {
    Matching(MatchingRecord),
    Tarjan(TarjanRecord),
    Tearing(TearingRecord),
}

impl StructuralRecord {
    /// Resolve every borrow in one event into owned values.
    ///
    /// Both observers that do real work go through here rather than each
    /// owning a copy of the conversion. Two copies is precisely the defect
    /// this module was written to remove: they agree only while both are
    /// maintained, and a live view disagreeing with a replay of the same model
    /// is exactly the kind of divergence nothing would catch.
    fn from_event(event: StructuralEvent<'_>) -> Self {
        match event {
            StructuralEvent::Matching { step, match_eq } => Self::Matching(MatchingRecord {
                step,
                match_eq: match_eq.to_vec(),
            }),
            StructuralEvent::Tarjan {
                step,
                stack,
                sccs_so_far,
            } => Self::Tarjan(TarjanRecord {
                step: step.into(),
                stack: stack.to_vec(),
                sccs_so_far: sccs_so_far.to_vec(),
            }),
            StructuralEvent::Tearing {
                step,
                tears_so_far,
                causal_so_far,
            } => Self::Tearing(TearingRecord {
                step,
                tears_so_far: tears_so_far.to_vec(),
                causal_so_far: causal_so_far.to_vec(),
            }),
        }
    }
}

/// The owned report [`crate::inspect_sort`] returns: every step the traced
/// call actually observed, in the order it observed them.
///
/// The three vocabularies share one sequence because that sequence *is* the
/// execution: matching runs to completion, then Tarjan over the dependency
/// graph it implies, then tearing within each coupled block Tarjan closed.
/// Splitting them at record time would discard that ordering; the accessors
/// below recover each algorithm's view without inventing one.
#[derive(Debug, Default)]
pub struct StructuralTrace {
    pub records: Vec<StructuralRecord>,
}

impl StructuralTrace {
    /// Every matching step, in order.
    pub fn matching(&self) -> impl Iterator<Item = &MatchingRecord> {
        self.records.iter().filter_map(|record| match record {
            StructuralRecord::Matching(matching) => Some(matching),
            StructuralRecord::Tarjan(_) | StructuralRecord::Tearing(_) => None,
        })
    }

    /// Tarjan steps grouped into traversals, split on
    /// [`TarjanStepRecord::Start`].
    ///
    /// A compile runs Tarjan more than once -- `build_blt_blocks` may run it
    /// over a condensed node set, and each run has its own numbering. Splicing
    /// two runs into one replay would address rows the other graph does not
    /// have, so the grouping is the consumer's only safe unit.
    pub fn tarjan_runs(&self) -> Vec<Vec<&TarjanRecord>> {
        group_by_start(
            self.records.iter().filter_map(|record| match record {
                StructuralRecord::Tarjan(tarjan) => Some(tarjan),
                StructuralRecord::Matching(_) | StructuralRecord::Tearing(_) => None,
            }),
            |record| matches!(record.step, TarjanStepRecord::Start { .. }),
        )
    }

    /// Tearing steps grouped into passes, split on
    /// [`TearingStep::PassStart`].
    ///
    /// One algebraic loop contributes up to two passes plus a terminating
    /// [`TearingStep::PlanSelected`], which lands in the pass it follows.
    /// Compare that record's priority against each pass's `PassStart` to find
    /// the pass describing the plan the compiler adopted.
    ///
    /// Use [`Self::tearing_blocks`] instead when the steps have to be
    /// attributed to a particular block's rows.
    pub fn tearing_passes(&self) -> Vec<Vec<&TearingRecord>> {
        group_by_start(self.tearing(), |record| {
            matches!(record.step, TearingStep::PassStart { .. })
        })
    }

    /// Every tearing step, in order.
    pub fn tearing(&self) -> impl Iterator<Item = &TearingRecord> {
        self.records.iter().filter_map(|record| match record {
            StructuralRecord::Tearing(tearing) => Some(tearing),
            StructuralRecord::Matching(_) | StructuralRecord::Tarjan(_) => None,
        })
    }

    /// Tearing steps grouped by coupled block, split on
    /// [`TearingStep::BlockStart`].
    ///
    /// **This is the grouping a consumer needs to draw tears on a block's own
    /// rows**, because the block identity is stated rather than inferred from
    /// position. A coupled block whose SCC is not square is rejected before
    /// the greedy search starts, so it contributes a `BlockStart` and no
    /// passes at all — positional grouping would hand its successor's tears to
    /// it and shift every later block by one.
    ///
    /// Each group's opening `BlockStart` carries the `ordinal` to match
    /// against the coupled blocks of the structural report, in BLT order.
    pub fn tearing_blocks(&self) -> Vec<Vec<&TearingRecord>> {
        group_by_start(self.tearing(), |record| {
            matches!(record.step, TearingStep::BlockStart { .. })
        })
    }
}

/// Split a sequence into groups, each beginning at an item `is_start` accepts.
///
/// Items before the first start form their own leading group rather than being
/// dropped: a dropped item is a step that happened and is not reported, which
/// is the one thing this module exists to prevent.
fn group_by_start<T>(items: impl Iterator<Item = T>, is_start: impl Fn(&T) -> bool) -> Vec<Vec<T>> {
    let mut groups: Vec<Vec<T>> = Vec::new();
    for item in items {
        if is_start(&item) || groups.is_empty() {
            groups.push(Vec::new());
        }
        if let Some(group) = groups.last_mut() {
            group.push(item);
        }
    }
    groups
}

/// An observer that hands each step to a sink the moment it happens, instead
/// of accumulating them.
///
/// [`StructuralRecorder`] cannot serve a caller that is *watching* a search
/// rather than reading it afterwards: with a breakpoint set inside matching,
/// a batch report delivers nothing until the whole analysis has finished, so
/// there is no step to show while the search is stopped. This delivers one
/// owned record per step as it is observed.
///
/// It records exactly what the recorder records --
/// `streaming_sees_the_same_records_the_batch_report_collects` pins that --
/// so a live view and a replay of the same model cannot disagree about what
/// happened.
pub(crate) struct StructuralStreamer<F> {
    sink: F,
}

impl<F: FnMut(StructuralRecord)> StructuralStreamer<F> {
    pub(crate) const fn new(sink: F) -> Self {
        Self { sink }
    }
}

impl<F: FnMut(StructuralRecord)> StructuralObserver for StructuralStreamer<F> {
    fn observe(&mut self, event: StructuralEvent<'_>) {
        (self.sink)(StructuralRecord::from_event(event));
    }
}

/// The one [`StructuralObserver`] outside `()` this crate builds: it clones
/// each borrowed event into an owned [`StructuralRecord`], which is exactly
/// the work `()` skips on every other call.
#[derive(Default)]
pub(crate) struct StructuralRecorder {
    records: Vec<StructuralRecord>,
}

impl StructuralRecorder {
    pub(crate) fn finish(self) -> StructuralTrace {
        StructuralTrace {
            records: self.records,
        }
    }
}

impl StructuralObserver for StructuralRecorder {
    fn observe(&mut self, event: StructuralEvent<'_>) {
        self.records.push(StructuralRecord::from_event(event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matching(step: MatchingStep) -> StructuralRecord {
        StructuralRecord::Matching(MatchingRecord {
            step,
            match_eq: Vec::new(),
        })
    }

    fn tarjan(step: TarjanStepRecord) -> StructuralRecord {
        StructuralRecord::Tarjan(TarjanRecord {
            step,
            stack: Vec::new(),
            sccs_so_far: Vec::new(),
        })
    }

    fn tearing(step: TearingStep) -> StructuralRecord {
        StructuralRecord::Tearing(TearingRecord {
            step,
            tears_so_far: Vec::new(),
            causal_so_far: Vec::new(),
        })
    }

    /// The production observer must stay a no-op, because `sort` pays for this
    /// seam on every compile. A `()` that recorded anything would not fail any
    /// behavioural test -- it would just allocate forever.
    #[test]
    fn the_production_observer_records_nothing() {
        let mut production = ();
        production.observe(StructuralEvent::Matching {
            step: MatchingStep::TryEquation(0),
            match_eq: &[None],
        });
        // `()` has no state to assert against; what this pins is that the call
        // compiles and returns, i.e. the seam is callable with no recorder.
        let trace = StructuralRecorder::default().finish();
        assert!(
            trace.records.is_empty(),
            "a recorder nobody fed must report nothing"
        );
    }

    #[test]
    fn the_recorder_keeps_every_event_in_order() {
        let mut recorder = StructuralRecorder::default();
        recorder.observe(StructuralEvent::Matching {
            step: MatchingStep::Start {
                n_equations: 2,
                n_unknowns: 2,
            },
            match_eq: &[None, None],
        });
        recorder.observe(StructuralEvent::Tarjan {
            step: TarjanStep::Start {
                n_nodes: 2,
                equation_of_node: None,
            },
            stack: &[],
            sccs_so_far: &[],
        });
        recorder.observe(StructuralEvent::Tearing {
            step: TearingStep::NoProgress,
            tears_so_far: &[],
            causal_so_far: &[],
        });
        let trace = recorder.finish();
        assert_eq!(trace.records.len(), 3, "no event may be dropped");
        assert_eq!(trace.matching().count(), 1);
        assert_eq!(trace.tarjan_runs().len(), 1);
        assert_eq!(trace.tearing_passes().len(), 1);
    }

    /// A `Start` carrying a condensed numbering must survive into the record,
    /// because without it the node indices in that run name the wrong rows.
    #[test]
    fn a_condensed_tarjan_numbering_survives_recording() {
        let mut recorder = StructuralRecorder::default();
        recorder.observe(StructuralEvent::Tarjan {
            step: TarjanStep::Start {
                n_nodes: 2,
                equation_of_node: Some(&[4, 7]),
            },
            stack: &[],
            sccs_so_far: &[],
        });
        let trace = recorder.finish();
        let runs = trace.tarjan_runs();
        assert_eq!(
            runs[0][0].step,
            TarjanStepRecord::Start {
                n_nodes: 2,
                equation_of_node: Some(vec![4, 7])
            },
            "the node-to-equation mapping is the only thing that makes the \
             indices in this run readable"
        );
    }

    /// Two traversals of different graphs must not be spliced into one replay.
    #[test]
    fn tarjan_runs_split_on_each_start() {
        let trace = StructuralTrace {
            records: vec![
                tarjan(TarjanStepRecord::Start {
                    n_nodes: 1,
                    equation_of_node: None,
                }),
                tarjan(TarjanStepRecord::Visit(0)),
                tarjan(TarjanStepRecord::Start {
                    n_nodes: 1,
                    equation_of_node: Some(vec![3]),
                }),
                tarjan(TarjanStepRecord::Visit(0)),
                tarjan(TarjanStepRecord::Visit(0)),
            ],
        };
        let runs = trace.tarjan_runs();
        assert_eq!(runs.len(), 2, "each Start opens a new traversal");
        assert_eq!(runs[0].len(), 2);
        assert_eq!(runs[1].len(), 3);
    }

    /// Two tear priorities over one loop are two passes, and only one of them
    /// describes the plan the compiler adopted.
    #[test]
    fn tearing_passes_split_on_each_pass_start() {
        let trace = StructuralTrace {
            records: vec![
                tearing(TearingStep::PassStart {
                    priority: TearPriorityKind::CausalUnlocks,
                    n: 3,
                }),
                tearing(TearingStep::NoProgress),
                tearing(TearingStep::PassStart {
                    priority: TearPriorityKind::VariableDegree,
                    n: 3,
                }),
                tearing(TearingStep::Complete {
                    tears: 1,
                    residuals: 1,
                }),
                tearing(TearingStep::PlanSelected {
                    priority: TearPriorityKind::VariableDegree,
                    tears: 1,
                    residuals: 1,
                }),
            ],
        };
        let passes = trace.tearing_passes();
        assert_eq!(passes.len(), 2, "each PassStart opens a new pass");
        assert_eq!(
            passes[1].len(),
            3,
            "PlanSelected lands in the pass it follows"
        );
    }

    /// **A coupled block that is never torn must still occupy its own
    /// group.** `tear_scc` rejects a non-square SCC before the greedy search
    /// starts, so such a block contributes a `BlockStart` and no passes. If
    /// grouping lost it, every later block's tears would shift onto the wrong
    /// rows — which is the whole reason `BlockStart` exists.
    #[test]
    fn a_block_with_no_passes_keeps_its_own_group() {
        let trace = StructuralTrace {
            records: vec![
                tearing(TearingStep::BlockStart {
                    ordinal: 0,
                    equations: 3,
                    unknowns: 2,
                }),
                // Not square: rejected, no pass ran.
                tearing(TearingStep::BlockStart {
                    ordinal: 1,
                    equations: 2,
                    unknowns: 2,
                }),
                tearing(TearingStep::PassStart {
                    priority: TearPriorityKind::CausalUnlocks,
                    n: 2,
                }),
                tearing(TearingStep::Complete {
                    tears: 1,
                    residuals: 1,
                }),
            ],
        };
        let blocks = trace.tearing_blocks();
        assert_eq!(blocks.len(), 2, "both coupled blocks must be represented");
        assert_eq!(
            blocks[0].len(),
            1,
            "the untorn block keeps its own group rather than vanishing"
        );
        assert_eq!(blocks[1].len(), 3, "the torn block keeps all of its steps");
        // And the ordinal, not the position, is what a consumer matches on.
        assert_eq!(
            blocks[1].first().map(|record| &record.step),
            Some(&TearingStep::BlockStart {
                ordinal: 1,
                equations: 2,
                unknowns: 2
            })
        );
    }

    /// Steps emitted before any `Start` are kept rather than dropped. A
    /// dropped step is one that happened and is not reported.
    #[test]
    fn records_before_the_first_start_are_not_dropped() {
        let trace = StructuralTrace {
            records: vec![
                tarjan(TarjanStepRecord::Visit(0)),
                tarjan(TarjanStepRecord::Start {
                    n_nodes: 1,
                    equation_of_node: None,
                }),
            ],
        };
        let runs = trace.tarjan_runs();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].len(), 1, "the orphan step keeps its own group");
    }

    /// The three vocabularies interleave in one sequence, and each accessor
    /// must see only its own.
    #[test]
    fn the_accessors_do_not_leak_other_vocabularies() {
        let trace = StructuralTrace {
            records: vec![
                matching(MatchingStep::TryEquation(0)),
                tarjan(TarjanStepRecord::Visit(0)),
                tearing(TearingStep::NoProgress),
                matching(MatchingStep::EquationFailed(0)),
            ],
        };
        assert_eq!(trace.matching().count(), 2);
        assert_eq!(trace.tarjan_runs().iter().flatten().count(), 1);
        assert_eq!(trace.tearing_passes().iter().flatten().count(), 1);
    }
}

/// The must-fire half: these drive the *real* algorithms through the seam.
///
/// Everything in [`tests`] above builds records by hand, so it proves the
/// recorder works and nothing about whether matching, Tarjan or tearing ever
/// call it. Deleting every `emit` in those three modules leaves all of those
/// tests green. These fail instead -- which is the whole point, because an
/// observer that reports nothing looks exactly like an algorithm that did
/// nothing.
#[cfg(test)]
mod algorithms {
    use std::collections::HashSet;

    use super::*;
    use crate::incidence::StructuredMatchingFamily;
    use crate::incidence::rows::IncidenceRows;

    fn test_span() -> rumoca_core::Span {
        rumoca_core::Span::from_offsets(
            rumoca_core::SourceId::from_source_name("structural_observation_source_1.mo"),
            0,
            1,
        )
    }

    /// Run the real matching and return the final matching plus every step it
    /// reported.
    fn matching_run(
        n_eq: usize,
        n_var: usize,
        rows: Vec<HashSet<usize>>,
        preferred: &[Option<usize>],
        families: &[StructuredMatchingFamily],
    ) -> (Vec<Option<usize>>, Vec<MatchingStep>) {
        let eq_vars = IncidenceRows::from_sets(rows);
        let mut recorder = StructuralRecorder::default();
        let (match_eq, _) = crate::matching::maximum_matching_with_structured(
            n_eq,
            n_var,
            &eq_vars,
            preferred,
            families,
            &mut recorder,
        );
        let trace = recorder.finish();
        let steps = trace.matching().map(|r| r.step.clone()).collect();
        (match_eq, steps)
    }

    fn tarjan_run(
        n: usize,
        adj: &[Vec<usize>],
        equation_of_node: Option<&[usize]>,
    ) -> (Vec<Vec<usize>>, Vec<TarjanStepRecord>) {
        let mut recorder = StructuralRecorder::default();
        let sccs = crate::tarjan::tarjan_scc_with_observer(n, adj, equation_of_node, &mut recorder);
        let trace = recorder.finish();
        let steps = trace
            .tarjan_runs()
            .into_iter()
            .flatten()
            .map(|r| r.step.clone())
            .collect();
        (sccs, steps)
    }

    fn tearing_run(
        n: usize,
        rows: &[HashSet<usize>],
    ) -> (Option<crate::tearing::TearingResult>, Vec<TearingStep>) {
        let mut recorder = StructuralRecorder::default();
        let plan = crate::tearing::tear_algebraic_loop_with_observer(n, rows, rows, &mut recorder);
        let trace = recorder.finish();
        let steps = trace
            .tearing_passes()
            .into_iter()
            .flatten()
            .map(|r| r.step.clone())
            .collect();
        (plan, steps)
    }

    /// The whole augmenting search of a 2x2 system that needs one failed
    /// displacement, pinned step for step.
    ///
    /// `eq0` can only take `v0`; `eq1` reaches for `v0` first, finds `eq0`
    /// holding it, descends, finds `eq0` has no alternative, backtracks and
    /// settles for `v1`. Every one of those is a step a replay has to show,
    /// and asserting the exact sequence is what makes a *missing* step a
    /// failure rather than a shorter list nobody notices.
    #[test]
    fn matching_reports_the_search_it_actually_ran() {
        let (match_eq, steps) = matching_run(
            2,
            2,
            vec![HashSet::from([0]), HashSet::from([0, 1])],
            &[],
            &[],
        );
        assert_eq!(match_eq, vec![Some(0), Some(1)], "both rows match");
        assert_eq!(
            steps,
            vec![
                MatchingStep::Start {
                    n_equations: 2,
                    n_unknowns: 2
                },
                MatchingStep::TryEquation(0),
                MatchingStep::Explore { eq: 0, var: 0 },
                MatchingStep::FoundFree { eq: 0, var: 0 },
                MatchingStep::Assign { eq: 0, var: 0 },
                MatchingStep::TryEquation(1),
                MatchingStep::Explore { eq: 1, var: 0 },
                MatchingStep::TryDisplace {
                    eq: 1,
                    var: 0,
                    holder: 0
                },
                MatchingStep::DisplaceFail { eq: 1, var: 0 },
                MatchingStep::Explore { eq: 1, var: 1 },
                MatchingStep::FoundFree { eq: 1, var: 1 },
                MatchingStep::Assign { eq: 1, var: 1 },
            ]
        );
    }

    /// The matching travelling with each step must be the matching at that
    /// instant. A consumer draws from it, so a stale or final-value snapshot
    /// would show a state the algorithm never passed through.
    #[test]
    fn each_step_carries_the_matching_as_it_stood() {
        let eq_vars = IncidenceRows::from_sets(vec![HashSet::from([0]), HashSet::from([0, 1])]);
        let mut recorder = StructuralRecorder::default();
        crate::matching::maximum_matching_with_structured(2, 2, &eq_vars, &[], &[], &mut recorder);
        let trace = recorder.finish();
        let snapshots: Vec<_> = trace.matching().map(|r| r.match_eq.clone()).collect();
        assert_eq!(
            snapshots.first().map(Vec::as_slice),
            Some([None, None].as_slice()),
            "the opening step must show nothing matched"
        );
        assert_eq!(
            snapshots.last().map(Vec::as_slice),
            Some([Some(0), Some(1)].as_slice()),
            "the closing step must show the finished matching"
        );
    }

    /// A displacement that succeeds: `eq0` holds `v0` but can move to `v1`, so
    /// `eq1` gets `v0` and the step is `DisplaceOk`, not `DisplaceFail`.
    #[test]
    fn a_displacement_that_succeeds_is_reported_as_such() {
        let (match_eq, steps) = matching_run(
            2,
            2,
            vec![HashSet::from([0, 1]), HashSet::from([0])],
            &[],
            &[],
        );
        assert_eq!(match_eq, vec![Some(1), Some(0)]);
        assert!(
            steps.contains(&MatchingStep::DisplaceOk { eq: 1, var: 0 }),
            "the successful displacement must be reported: {steps:?}"
        );
        assert!(
            !steps
                .iter()
                .any(|step| matches!(step, MatchingStep::DisplaceFail { .. })),
            "nothing failed here: {steps:?}"
        );
    }

    /// An equation that cannot be matched must say so. This is the path HRW's
    /// predecessor deliberately kept observed: a system that fails to match is
    /// the one whose search is most worth watching.
    #[test]
    fn an_equation_with_no_augmenting_path_is_reported_as_failed() {
        let (match_eq, steps) =
            matching_run(2, 1, vec![HashSet::from([0]), HashSet::from([0])], &[], &[]);
        assert_eq!(match_eq, vec![Some(0), None], "one row cannot match");
        assert!(
            steps.contains(&MatchingStep::EquationFailed(1)),
            "the unmatched row must be named: {steps:?}"
        );
    }

    /// Rows seeded from a regular family are matched before Kuhn runs and are
    /// then skipped by it. Without these steps a replay opens on equations
    /// already matched for no visible reason.
    #[test]
    fn a_structured_family_seed_is_reported_before_kuhn_runs() {
        let family = StructuredMatchingFamily {
            source_owner_index: 0,
            first_equation_index: 0,
            equations_per_point: 1,
            point_count: 2,
            extents: vec![2],
            cell_strides: vec![1],
            base_unknowns: vec![0],
            unknown_steps: vec![vec![1]],
            span: test_span(),
        };
        let (match_eq, steps) = matching_run(
            2,
            2,
            vec![HashSet::from([0]), HashSet::from([1])],
            &[],
            std::slice::from_ref(&family),
        );
        assert_eq!(match_eq, vec![Some(0), Some(1)]);
        assert_eq!(
            steps,
            vec![
                MatchingStep::Start {
                    n_equations: 2,
                    n_unknowns: 2
                },
                MatchingStep::SeedStructured { eq: 0, var: 0 },
                MatchingStep::SeedStructured { eq: 1, var: 1 },
            ],
            "a fully seeded family leaves Kuhn nothing to do, and the seeding \
             is the only thing that explains the finished matching"
        );
    }

    /// The preference phase reports one decision per preferred edge, and both
    /// outcomes are reachable. `eq1` prefers `v1`, which it can have; `eq0`
    /// prefers `v1` too but cannot take it without unmatching `eq1`.
    #[test]
    fn the_preference_phase_reports_each_decision() {
        let (_, steps) = matching_run(
            2,
            2,
            vec![HashSet::from([0, 1]), HashSet::from([1])],
            &[Some(1), Some(1)],
            &[],
        );
        let decisions: Vec<_> = steps
            .iter()
            .filter(|step| {
                matches!(
                    step,
                    MatchingStep::ClaimedPreference { .. }
                        | MatchingStep::PreferenceUnavailable { .. }
                )
            })
            .collect();
        assert!(
            !decisions.is_empty(),
            "the preference phase ran and must report it: {steps:?}"
        );
        assert!(
            decisions
                .iter()
                .any(|step| matches!(step, MatchingStep::PreferenceUnavailable { eq: 0, var: 1 })),
            "eq0 cannot have v1 without unmatching eq1: {decisions:?}"
        );
    }

    /// One traversal of a graph with a 3-cycle and a tail, pinned by the step
    /// kinds it must contain. A back edge is what closes a component, so a
    /// trace missing it cannot explain the SCC it reports.
    #[test]
    fn tarjan_reports_the_traversal_it_actually_ran() {
        // 0 -> 1 -> 2 -> 0 is a cycle; 2 -> 3 is a tail.
        let adj = vec![vec![1], vec![2], vec![0, 3], vec![]];
        let (sccs, steps) = tarjan_run(4, &adj, None);
        assert_eq!(sccs, vec![vec![3], vec![2, 1, 0]]);
        assert_eq!(
            steps.first(),
            Some(&TarjanStepRecord::Start {
                n_nodes: 4,
                equation_of_node: None
            }),
            "the traversal must open by stating the graph it was handed"
        );
        for node in 0..4 {
            assert!(
                steps.contains(&TarjanStepRecord::Visit(node)),
                "every node is visited: {steps:?}"
            );
        }
        assert!(
            steps.contains(&TarjanStepRecord::BackEdge { from: 2, to: 0 }),
            "the back edge that closes the cycle must be reported: {steps:?}"
        );
        assert!(
            steps.contains(&TarjanStepRecord::TreeEdge { from: 0, to: 1 }),
            "descending into an unvisited node is a tree edge: {steps:?}"
        );
        assert!(
            steps.contains(&TarjanStepRecord::Return { from: 2, to: 3 }),
            "the lowlink fold on pop must be reported: {steps:?}"
        );
        assert!(
            steps.contains(&TarjanStepRecord::SccFound {
                root: 0,
                members: vec![2, 1, 0]
            }),
            "the closed component must name its members: {steps:?}"
        );
    }

    /// A condensed traversal must state its numbering through the algorithm,
    /// not just through the recorder: node indices read as equation indices
    /// would name the wrong rows.
    #[test]
    fn a_condensed_tarjan_run_states_its_numbering() {
        let adj = vec![vec![1], vec![]];
        let (_, steps) = tarjan_run(2, &adj, Some(&[4, 9]));
        assert_eq!(
            steps.first(),
            Some(&TarjanStepRecord::Start {
                n_nodes: 2,
                equation_of_node: Some(vec![4, 9])
            }),
            "node 0 is equation 4 and node 1 is equation 9; losing that \
             mapping silently renumbers every later step"
        );
    }

    /// Tearing runs the greedy search under both priorities and keeps the
    /// better plan, so the trace must say which pass the plan came from. A
    /// consumer reading the first pass as the plan would describe a tear
    /// sequence the compiler discarded.
    #[test]
    fn tearing_reports_its_passes_and_which_one_was_selected() {
        // A 3-equation loop: eq0 {v0,v1}, eq1 {v1,v2}, eq2 {v0,v2}.
        let rows = vec![
            HashSet::from([0, 1]),
            HashSet::from([1, 2]),
            HashSet::from([0, 2]),
        ];
        let (plan, steps) = tearing_run(3, &rows);
        let plan = plan.expect("this loop tears");
        let starts: Vec<_> = steps
            .iter()
            .filter(|step| matches!(step, TearingStep::PassStart { .. }))
            .collect();
        assert!(
            !starts.is_empty(),
            "every pass must announce itself: {steps:?}"
        );
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, TearingStep::Torn { .. })),
            "a torn loop must report the cut it made: {steps:?}"
        );
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, TearingStep::Causal { .. })),
            "the causal steps the tear unlocked must be reported: {steps:?}"
        );
        let selected = steps
            .iter()
            .find_map(|step| match step {
                TearingStep::PlanSelected {
                    priority,
                    tears,
                    residuals,
                } => Some((*priority, *tears, *residuals)),
                _ => None,
            })
            .expect("a plan was adopted, so a selection must be reported");
        assert_eq!(
            (selected.1, selected.2),
            (
                plan.tear_var_local_indices.len(),
                plan.residual_eq_local_indices.len()
            ),
            "the selection must describe the plan actually returned"
        );
        assert!(
            starts.iter().any(
                |step| matches!(step, TearingStep::PassStart { priority, .. }
                    if *priority == selected.0)
            ),
            "the selected priority must name a pass that actually ran: {steps:?}"
        );
    }

    /// A loop that cannot be torn must say so rather than fall silent. The
    /// absence of a `Complete` is indistinguishable from a step that was never
    /// reported, which is the failure this whole module exists to prevent.
    #[test]
    fn a_loop_that_cannot_be_torn_reports_no_progress() {
        // Every equation references every unknown: nothing to cut helps.
        let rows = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
        let (plan, steps) = tearing_run(2, &rows);
        if plan.is_none() {
            assert!(
                steps.contains(&TearingStep::NoProgress),
                "a pass that produced no plan must say so: {steps:?}"
            );
        } else {
            assert!(
                steps
                    .iter()
                    .any(|step| matches!(step, TearingStep::Complete { .. })),
                "a pass that produced a plan must report completing: {steps:?}"
            );
        }
    }

    /// **The property that justifies the redesign.** The traced and untraced
    /// paths are the same code, so they must agree on every input -- a
    /// recorded step sequence always describes the run that produced the
    /// result beside it.
    ///
    /// Its predecessor could not state this: matching and Tarjan each had a
    /// second, separately written traced implementation, so agreement was a
    /// property of two bodies of code staying in step rather than of there
    /// being one.
    #[test]
    fn observing_a_search_does_not_change_what_it_returns() {
        let mut seed = 0x9E37_79B9_7F4A_7C15u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        for case in 0..300u32 {
            let n = 1 + (next() % 8) as usize;
            let rows: Vec<HashSet<usize>> = (0..n)
                .map(|_| (0..n).filter(|_| next().is_multiple_of(3)).collect())
                .collect();
            let eq_vars = IncidenceRows::from_sets(rows.clone());

            let untraced = crate::matching::maximum_matching_with_structured(
                n,
                n,
                &eq_vars,
                &[],
                &[],
                &mut (),
            );
            let mut recorder = StructuralRecorder::default();
            let traced = crate::matching::maximum_matching_with_structured(
                n,
                n,
                &eq_vars,
                &[],
                &[],
                &mut recorder,
            );
            assert_eq!(untraced, traced, "case {case}: matching diverged");
            assert!(
                !recorder.finish().records.is_empty(),
                "case {case}: a search that ran must report something"
            );

            let adj: Vec<Vec<usize>> = rows
                .iter()
                .map(|row| row.iter().copied().collect())
                .collect();
            let mut recorder = StructuralRecorder::default();
            assert_eq!(
                crate::tarjan::tarjan_scc(n, &adj),
                crate::tarjan::tarjan_scc_with_observer(n, &adj, None, &mut recorder),
                "case {case}: Tarjan diverged"
            );

            let mut recorder = StructuralRecorder::default();
            let untorn = crate::tearing::tear_algebraic_loop(n, &rows);
            let torn =
                crate::tearing::tear_algebraic_loop_with_observer(n, &rows, &rows, &mut recorder);
            assert_eq!(
                untorn.map(|plan| plan.tear_var_local_indices),
                torn.map(|plan| plan.tear_var_local_indices),
                "case {case}: tearing diverged"
            );
        }
    }
}
