//! Does a pointer in prose land on a node that exists?
//!
//! # The gap this closes, measured 2026-09-03
//!
//! Doug read one Answer and reported four times that he could not find what it
//! described. Three of the four were mechanically checkable and nothing checked them:
//!
//! | claim | what was wrong |
//! |---|---|
//! | `Y[1]` | a notation nothing in HRW or Rumoca renders |
//! | `discrete_real_updates` "in Solve lowering" | an **Events** summary field |
//! | a `Minus` over a `Mul` | no `Minus` in that stage — or any expression tree at all |
//!
//! **And the surface is not Answer-specific.** Before this module, *nothing in HRW
//! resolved an `hrw://stage/<S>/node/<path>` link against real stage IR* — not for the
//! Answer and not for the fixture labs, which carry fourteen such links. The lab
//! checkers verify link *syntax*, that labs and stations exist, and that `src` links
//! resolve. That a node path lands on a node was checked nowhere.
//!
//! Same shape as the debug bridge's saturated-stack bug the night before: the logic to
//! detect the problem existed, and the case that actually occurs was never routed
//! through it.
//!
//! # What it deliberately does NOT check
//!
//! **That a resolving pointer is USABLE.** The fourth failure was
//! `problem.layout.bindings`, which resolves and contains exactly what the prose said.
//! It failed because the prose named the node by an invented concept — *"the state
//! vector"*, which is nowhere a label — and buried the fact two expansions down while
//! `initial_y` sat on the top level saying the same thing. No checker sees that; it is a
//! habit (*cite the node by its label, prefer the shallowest node carrying the fact*) and
//! Doug's report remains the only instrument, as it is for layout.
//!
//! # One source of truth: the committed trace
//!
//! A **lab** is versioned, so it is checked against the committed
//! `docs/specimen-notebook/<Model>/trace/`, which is generated and correct by
//! construction. The caller supplies the loader, so this module never chooses a source
//! itself.
//!
//! **There used to be a second source.** The HRW Answer was checked against
//! `.hrw-bridge/stages/`, the pane's own input, by `examples/check_answer.rs`, because
//! an Answer was about what was on screen right now. The Answer was removed from HRW on
//! 2026-11-07 (charter Decision 19: answers are given on the platform — the chat, a VS
//! Code tab, System Modeler or Wolfram Desktop — never in HRW), and the example and its
//! token-scan advisory went with it. The module was renamed from `answer_check` then.

use crate::bridge;
use crate::worker::StageKind;
use serde_json::Value;

/// What became of one pointer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// The path resolved to a node in that stage's IR.
    Resolved,
    /// The stage's IR was read and the path is not in it. **A defect.**
    NoSuchNode,
    /// The path is not parseable as a node path. **A defect.**
    Malformed,
    /// No IR available for that stage, so this pointer is **unjudged** — neither pass
    /// nor fail. Reported separately so a run cannot look clean by having checked
    /// nothing, which is this repository's most-repeated failure shape.
    StageUnavailable,
    /// Resolves, but the link names a multi-sub-view stage without saying which sub-view.
    ///
    /// **A defect, and the one that resolves perfectly while doing nothing.** See
    /// [`STAGES_WITH_SUB_VIEWS`]: the jump acts on a tree, and the pane may be showing an
    /// equation sheet or an animation instead.
    SubViewUnstated,
    /// A `node` pointer appeared with no preceding `load`, so no specimen is known.
    /// Unjudged for the same reason, and usually a real authoring bug as well — six
    /// verbs need a loaded specimen and the load must come first.
    NoSpecimen,
    /// Marked [`EXPECTS_NO_NODE`] and correctly resolves to nothing.
    AbsentAsExpected,
    /// Marked [`EXPECTS_NO_NODE`] and **does** resolve. **A defect**, and the subtle
    /// one: the lab's expectation has silently become false and the station has stopped
    /// testing what it was written for.
    UnexpectedlyPresent,
}

impl Verdict {
    /// Whether this verdict is a defect, as against merely unjudged.
    pub fn is_defect(self) -> bool {
        matches!(
            self,
            Verdict::NoSuchNode
                | Verdict::Malformed
                | Verdict::UnexpectedlyPresent
                | Verdict::SubViewUnstated
        )
    }
}

/// Marks a node link that is **supposed** to resolve to nothing.
///
/// # Why a marker and not a skip list
///
/// `node-pointing.md` Station 5 points at `error.unmatched_unknowns[0]` on `RcCircuit`
/// *on purpose*: the station exists to verify that HRW refuses a dead path and says so
/// in the status bar, rather than expanding partway. The first run of the lab checker
/// reported it as a defect, which is the false positive that gets a checker distrusted
/// and then deleted.
///
/// **A bare exemption would be worse than the false positive.** If `RcCircuit` ever
/// gained an `error` node, the station would keep passing while testing nothing — the
/// exact silent-success shape the must-fire rule exists to forbid. So the marker is a
/// claim in both directions, and a marked link that *does* resolve fails by name.
///
/// Same principle as `<!-- unbuilt: … -->`: a claim of absence is checkable, and one
/// that has quietly become false is the error nobody catches, because believing it means
/// not looking.
pub const EXPECTS_NO_NODE: &str = "<!-- expects-no-node -->";

/// One `hrw://stage/<S>/node/<path>` pointer found in prose.
#[derive(Debug, Clone)]
pub struct Pointer {
    pub raw: String,
    /// 1-based, so a finding names a place rather than a string to go hunting for.
    pub line: usize,
    /// The specimen in force, from the nearest preceding `hrw://load/<Model>`.
    pub specimen: Option<String>,
    pub stage: StageKind,
    pub path: String,
    pub verdict: Verdict,
}

/// The specimen a `load` link selects, if the link is a load.
///
/// Both `hrw://load/<Model>` and `hrw://load/<Model>/<Stage>` select a specimen; the
/// second also switches tab, which does not concern us.
fn load_target(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("hrw://load/")?;
    let model = rest.split('/').next()?;
    (!model.is_empty()).then_some(model)
}

/// Stages whose pane has more than one sub-view, so a bare `node` link is conditional.
///
/// # Why a resolving pointer can still do nothing
///
/// Doug, 2026-09-04: *"This link and other links do not cause anything to happen."* Every
/// one of them resolved — `check_answer` reported 19 of 19 against the pane — because
/// resolving is a claim about the **IR** and doing something is a claim about the **pane**.
///
/// `PointAtNode` with no sub-view "leaves whatever sub-view the stage is already showing,
/// which for a tree-only stage is its only one". For these five it is not: he was on Flatten
/// showing `EquationSheet`, which has no tree, so the jump target had nothing to act on and
/// the click was silent. Ten of the Answer's links were conditional on which sub-view
/// happened to be up.
///
/// So a node link into one of these must name its sub-view — `stage/Flatten/Tree/node/…`.
/// The slug is `Tree` for all four; the others are `EquationSheet`, `Incidence` and the
/// animations. *(It was five until 2026-11-04, when Initialization became tree-only and
/// `IcPlan` stopped existing.)*
///
/// # A list, but not a second opinion
///
/// `every_sub_viewed_stage_is_listed` derives the truth from `SubView::from_slug` — the UI's
/// own answer — and asserts this matches it in both directions, so the list cannot fall
/// behind a new sub-view or invent one. It exists as a list only because this module checks
/// prose and has no business importing the view enum.
pub const STAGES_WITH_SUB_VIEWS: &[&str] = &[
    "Structural",
    "IndexReduction",
    "Flatten",
    "Events",
    // **`Initialization` left this list on 2026-11-04**, when `InitView` went with the
    // IC-plan view: v0.10.0 states initialization as initial-value pins rather than as
    // a solve sequence, so there was nothing for that sub-view to show. The stage is
    // tree-only now, and `every_sub_viewed_stage_is_listed` is what required the entry
    // out rather than leaving a node link into it obliged to name a sub-view that no
    // longer parses.
];

/// The stage and node path a `node` link addresses, if the link is one.
///
/// **Both arms of the real grammar**, which `App::parse_hrw_link` spells as
/// `["stage", s, view, "node", path]` and `["stage", s, "node", path]`. Handling only
/// the second silently examined 4 of the labs' 14 node links, and the non-vacuity guard
/// on `every_lab_node_link_lands_on_a_real_node` is what said so — a checker that looks
/// at a third of its subject and reports nothing wrong is the shape this module exists
/// to end, so it was worth the guard costing a red run before any real finding.
fn node_target(url: &str) -> Option<(&str, &str, bool)> {
    let rest = url.strip_prefix("hrw://stage/")?;
    let segs: Vec<&str> = rest.split('/').collect();
    let (stage, path, has_sub_view) = match segs.as_slice() {
        [stage, _view, "node", path] => (*stage, *path, true),
        [stage, "node", path] => (*stage, *path, false),
        _ => return None,
    };
    (!stage.is_empty() && !path.is_empty()).then_some((stage, path, has_sub_view))
}

/// Resolve every node pointer in `text`, in document order.
///
/// `load_ir` is handed `(specimen, stage)` and returns that stage's IR, or `None` when
/// it is unavailable. **A closure rather than a path** so the same resolver serves the
/// committed notebook and the live bridge — and so the tests below can supply IR
/// in-memory, which is what makes the specimen-tracking testable at all.
pub fn check(
    text: &str,
    mut load_ir: impl FnMut(&str, StageKind) -> Option<Value>,
) -> Vec<Pointer> {
    let mut out = Vec::new();
    let mut specimen: Option<String> = None;

    // **Line by line, not over the whole text.** The marker is per link, and a finding
    // that cannot name a line sends its reader hunting for a URL.
    for (i, line) in text.lines().enumerate() {
        let expects_absent = line.contains(EXPECTS_NO_NODE);

        for url in crate::app::hrw_links_in_order(line) {
            if let Some(model) = load_target(&url) {
                specimen = Some(model.to_owned());
                continue;
            }
            let Some((stage_slug, path, has_sub_view)) = node_target(&url) else {
                continue;
            };
            // An unknown stage slug is already caught by the link checkers, so it is
            // not this module's finding to report twice.
            let Some(stage) = StageKind::from_slug(stage_slug) else {
                continue;
            };

            let resolution = match specimen.as_deref() {
                None => Verdict::NoSpecimen,
                Some(model) => match load_ir(model, stage) {
                    None => Verdict::StageUnavailable,
                    Some(ir) => match bridge::parse_path(path) {
                        None => Verdict::Malformed,
                        Some(segs) => {
                            let mut node = &ir;
                            let mut ok = true;
                            for seg in &segs {
                                match seg.get_in(node) {
                                    Some(next) => node = next,
                                    None => {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            if ok {
                                Verdict::Resolved
                            } else {
                                Verdict::NoSuchNode
                            }
                        }
                    },
                },
            };

            // The marker flips the two judged outcomes and leaves the unjudged ones
            // alone: an unavailable stage says nothing about whether a path is absent.
            let mut verdict = match (expects_absent, resolution) {
                (true, Verdict::NoSuchNode) => Verdict::AbsentAsExpected,
                (true, Verdict::Resolved) => Verdict::UnexpectedlyPresent,
                (_, other) => other,
            };
            // **Checked only once the path resolves**, so a broken path is reported as a
            // broken path rather than as a missing sub-view. A pointer that resolves and
            // still does nothing is the subtler defect and deserves its own verdict.
            if verdict == Verdict::Resolved
                && !has_sub_view
                && STAGES_WITH_SUB_VIEWS.contains(&stage_slug)
            {
                verdict = Verdict::SubViewUnstated;
            }

            out.push(Pointer {
                raw: url.clone(),
                line: i + 1,
                specimen: specimen.clone(),
                stage,
                path: path.to_owned(),
                verdict,
            });
        }
    }
    out
}

// **The backticked-token scan was here** — `tokens_absent_from_every_stage` and three
// helpers, an *advisory* that flagged a backticked name appearing in no stage IR. It was
// the check that caught invented notation like `Y[1]` and `Minus` in HRW Answers, and its
// only caller was `examples/check_answer.rs`, which read `.hrw-bridge/answer.md`. Both
// went with the HRW Answer on 2026-11-07 (charter Decision 19). Recoverable from git if a
// checker for answers on the platform is ever wanted.

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// IR shaped like Solve lowering's, small enough to read.
    fn solve_ir() -> Value {
        json!({
            "initial_y": [1.0, 0.0],
            "problem": {
                "layout": { "bindings": { "v": { "Y": { "index": 1 } } } },
                "discrete": { "update_targets": [{ "Y": { "index": 1 } }] },
            },
        })
    }

    fn loader(ir: Value) -> impl FnMut(&str, StageKind) -> Option<Value> {
        move |_model, stage| (stage == StageKind::SolveLowering).then(|| ir.clone())
    }

    /// `STAGES_WITH_SUB_VIEWS` is exactly the stages that accept a sub-view slug.
    ///
    /// **Derived from `SubView::from_slug`, which is the UI's own answer.** A list that fell
    /// behind a new sub-view would stop warning about the very links that need it, and would
    /// do so silently — the shape this module exists to end. A stage wrongly *in* the list is
    /// the cheaper error, a spurious defect on a link that works, and is caught here too.
    #[test]
    fn every_sub_viewed_stage_is_listed() {
        for stage in StageKind::ALL {
            let accepts = crate::app::SubView::from_slug(*stage, "Tree").is_some();
            let listed = STAGES_WITH_SUB_VIEWS.contains(&stage.slug());
            assert_eq!(
                accepts,
                listed,
                "{}: accepts a `Tree` sub-view = {accepts}, listed = {listed}. A node link \
                 into a stage with sub-views must name one, and this list is what warns \
                 about the ones that do not.",
                stage.slug(),
            );
        }
    }

    /// A bare node link into a sub-viewed stage is a defect even though it resolves.
    ///
    /// Doug's 2026-09-04 report: nineteen pointers reported as resolving, and the clicks did
    /// nothing, because he was on Flatten's equation sheet and the jump acts on a tree.
    /// Resolving is a claim about the IR; doing something is a claim about the pane.
    #[test]
    fn a_bare_node_link_into_a_sub_viewed_stage_is_a_defect() {
        let ir = serde_json::json!({ "equations": [1, 2] });
        let make = || {
            let ir = ir.clone();
            move |_m: &str, _s: StageKind| Some(ir.clone())
        };

        let bare = "[l](hrw://load/RcCircuit)\n[x](hrw://stage/Flatten/node/equations[0])\n";
        let found = check(bare, make());
        assert_eq!(
            found[0].verdict,
            Verdict::SubViewUnstated,
            "Flatten has sub-views, so this works only when the tree happens to be up",
        );
        assert!(found[0].verdict.is_defect());

        let named = "[l](hrw://load/RcCircuit)\n[x](hrw://stage/Flatten/Tree/node/equations[0])\n";
        assert_eq!(
            check(named, make())[0].verdict,
            Verdict::Resolved,
            "naming the sub-view fixes it",
        );

        // A tree-only stage needs no sub-view, and must not be nagged about one.
        let dae = "[l](hrw://load/RcCircuit)\n[x](hrw://stage/Dae/node/equations[0])\n";
        assert_eq!(
            check(dae, make())[0].verdict,
            Verdict::Resolved,
            "Dae is tree-only, so a bare link there is complete",
        );
    }

    /// A pointer at a real node resolves, and one at an absent node is a defect.
    ///
    /// **The non-vacuity half is the second assertion.** A resolver that returned
    /// `Resolved` unconditionally would satisfy the first, and that is precisely the
    /// silent-success shape this module exists to end.
    #[test]
    fn a_real_node_resolves_and_an_absent_one_is_a_defect() {
        let text = "\
            [load](hrw://load/BouncingBall)\n\
            [good](hrw://stage/SolveLowering/node/problem.layout.bindings)\n\
            [bad](hrw://stage/SolveLowering/node/problem.layout.state_vector)\n";
        let found = check(text, loader(solve_ir()));

        assert_eq!(found.len(), 2, "both pointers seen: {found:?}");
        assert_eq!(found[0].verdict, Verdict::Resolved);
        assert_eq!(
            found[1].verdict,
            Verdict::NoSuchNode,
            "`state_vector` is the invented label Doug could not find, 2026-09-03",
        );
        assert!(found[1].verdict.is_defect() && !found[0].verdict.is_defect());
    }

    /// The stage in the link is the stage the IR is read from.
    ///
    /// This is the `discrete_real_updates` defect: the path exists, in **Events**, and
    /// the prose attributed it to Solve lowering. Resolving against "any stage" would
    /// have called it fine.
    #[test]
    fn a_path_from_the_wrong_stage_does_not_resolve() {
        let text = "[l](hrw://load/BouncingBall)\n\
                    [x](hrw://stage/Events/node/problem.layout.bindings)\n";
        // The loader supplies IR for SolveLowering only, so Events is unavailable —
        // which must read as UNJUDGED, never as resolved.
        let found = check(text, loader(solve_ir()));
        assert_eq!(found[0].verdict, Verdict::StageUnavailable);
        assert!(
            !found[0].verdict.is_defect(),
            "an unjudged pointer is not a pass and not a failure",
        );
    }

    /// A node pointer before any load is unjudged, and says so.
    #[test]
    fn a_node_pointer_with_no_load_before_it_is_unjudged() {
        let text = "[x](hrw://stage/SolveLowering/node/initial_y)\n";
        let found = check(text, loader(solve_ir()));
        assert_eq!(found[0].verdict, Verdict::NoSpecimen);
        assert_eq!(found[0].specimen, None);
    }

    /// The specimen in force is the nearest PRECEDING load, not the first or the last.
    ///
    /// The reason `hrw_links_in_order` was split out of `extract_hrw_links`: the
    /// de-duplicating version cannot express this, and with one specimen per document
    /// nothing would ever have noticed.
    #[test]
    fn the_nearest_preceding_load_is_the_one_in_force() {
        let text = "\
            [a](hrw://load/BouncingBall)\n\
            [one](hrw://stage/SolveLowering/node/initial_y)\n\
            [b](hrw://load/RcCircuit/SolveLowering)\n\
            [two](hrw://stage/SolveLowering/node/initial_y)\n";
        let found = check(text, loader(solve_ir()));
        assert_eq!(found[0].specimen.as_deref(), Some("BouncingBall"));
        assert_eq!(
            found[1].specimen.as_deref(),
            Some("RcCircuit"),
            "a second load switches the specimen for everything after it",
        );
    }

    /// An unparseable path is a defect rather than a silent skip.
    #[test]
    fn a_malformed_path_is_reported() {
        let text = "[l](hrw://load/BouncingBall)\n\
                    [x](hrw://stage/SolveLowering/node/problem..layout)\n";
        let found = check(text, loader(solve_ir()));
        assert_eq!(found[0].verdict, Verdict::Malformed);
        assert!(found[0].verdict.is_defect());
    }

    /// A marked link that resolves to nothing passes; one that resolves FAILS.
    ///
    /// The second half is the one worth having. `node-pointing.md` Station 5 points at a
    /// dead path deliberately, to verify HRW refuses it. If the specimen ever gained
    /// that node, a bare exemption would let the station keep passing while testing
    /// nothing — so the marker is a claim about absence, and it is checked.
    #[test]
    fn the_marker_is_a_claim_in_both_directions() {
        let absent = format!(
            "[l](hrw://load/BouncingBall)\n\
             [x](hrw://stage/SolveLowering/node/problem.layout.no_such_thing) {}\n",
            EXPECTS_NO_NODE
        );
        let found = check(&absent, loader(solve_ir()));
        assert_eq!(found[0].verdict, Verdict::AbsentAsExpected);
        assert!(
            !found[0].verdict.is_defect(),
            "a warranted marker is not a defect"
        );

        let present = format!(
            "[l](hrw://load/BouncingBall)\n\
             [x](hrw://stage/SolveLowering/node/initial_y) {}\n",
            EXPECTS_NO_NODE
        );
        let found = check(&present, loader(solve_ir()));
        assert_eq!(
            found[0].verdict,
            Verdict::UnexpectedlyPresent,
            "a claim of absence that has become false must fail, not pass quietly",
        );
        assert!(found[0].verdict.is_defect());
    }

    /// A finding names its line, so it can be gone to rather than searched for.
    #[test]
    fn a_pointer_carries_its_line_number() {
        let text = "[l](hrw://load/BouncingBall)\n\
                    filler\n\
                    [x](hrw://stage/SolveLowering/node/initial_y)\n";
        let found = check(text, loader(solve_ir()));
        assert_eq!(found[0].line, 3);
    }

    /// Both spellings of a node link are recognised.
    ///
    /// Handling only the bare form examined 4 of the labs' 14 node links and reported
    /// nothing wrong — caught by a non-vacuity guard, not by the checker itself.
    #[test]
    fn a_node_link_with_a_sub_view_is_recognised_too() {
        let text = "[l](hrw://load/BouncingBall)\n\
                    [bare](hrw://stage/SolveLowering/node/initial_y)\n\
                    [viewed](hrw://stage/SolveLowering/Tree/node/initial_y)\n";
        let found = check(text, loader(solve_ir()));
        assert_eq!(found.len(), 2, "both forms seen: {found:?}");
        assert!(found.iter().all(|p| p.verdict == Verdict::Resolved));
    }
}
