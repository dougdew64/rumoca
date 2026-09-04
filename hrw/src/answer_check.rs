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
//! # Two sources of truth, because the two artifacts differ
//!
//! A **lab** is versioned, so it is checked against the committed
//! `docs/specimen-notebook/<Model>/trace/`, which is generated and correct by
//! construction. An **Answer** is about what is on screen right now, so it is checked
//! against `.hrw-bridge/stages/`, the pane's own input.
//!
//! **That distinction is not pedantry — it is the bug it prevents.** While fixing the
//! third defect above, the claim was verified against the notebook rather than against
//! the file feeding the pane. The two happened to agree, so it did not bite, but a
//! sibling artifact had been checked instead of the screen, which is the habit that
//! produced the first three errors.

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
/// The slug is `Tree` for all five; the others are `EquationSheet`, `Incidence`, `IcPlan`
/// and the animations.
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
    "Initialization",
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

/// A backticked token in the prose that appears in **no** stage IR that was loaded.
///
/// # Advisory, never a gate — and the asymmetry is the point
///
/// This is the check that would have caught `Y[1]` and `Minus`, and it cannot be made
/// exact: markdown does not say whether `` `when` `` is a claim about a pane, a Modelica
/// keyword, or a Rust identifier. So it is **reported for a human to read, not asserted**.
///
/// As a tool's advisory output a false positive costs a glance. As a test it would be
/// fatal, and the test would be deleted or the rule loosened — which is how a checker
/// becomes a checker nobody trusts.
pub fn tokens_absent_from_every_stage(text: &str, loaded: &[Value]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let haystacks: Vec<String> = loaded.iter().map(Value::to_string).collect();

    for token in backticked_tokens(text) {
        if !could_name_a_node(&token) {
            continue;
        }
        if haystacks.iter().any(|h| h.contains(&token)) {
            continue;
        }
        // **A subscript is legitimate when its base holds an ARRAY.** `f_x[0]` never
        // appears literally in JSON — `f_x` is a key and `0` an index — so flagging it was
        // noise, and the loudest kind: seven of thirteen advisory lines on 2026-09-04 for
        // zero real findings, which is how an advisory stops being read.
        //
        // **"The base exists as a key" is NOT the rule, and trying it first proved why.**
        // `Y` *is* a key in Solve lowering — `bindings.v.Y` — so that version excluded
        // `Y[1]`, the original defect this scan was built to catch. The difference is what
        // the key holds: `f_x` is an array, so `f_x[0]` addresses something; `Y` is an
        // object mapping a name to a slot, so `Y[1]` addresses nothing and was exactly as
        // unfindable as Doug reported.
        if let Some(base) = token.split('[').next().filter(|b| !b.is_empty())
            && base != token
            && loaded.iter().any(|ir| key_holds_an_array(ir, base))
        {
            continue;
        }
        if !out.contains(&token) {
            out.push(token);
        }
    }
    out
}

/// Whether any node anywhere in `ir` is `key` mapped to an array.
///
/// The question a subscripted token asks: is `base[0]` addressing something? A key holding
/// an object answers no, which is what separates `f_x[0]` from `Y[1]`.
fn key_holds_an_array(ir: &Value, key: &str) -> bool {
    match ir {
        Value::Object(map) => map
            .iter()
            .any(|(k, v)| (k == key && v.is_array()) || key_holds_an_array(v, key)),
        Value::Array(items) => items.iter().any(|v| key_holds_an_array(v, key)),
        _ => false,
    }
}

/// Whether a backticked token could plausibly be a name in stage IR.
///
/// **Four structural exclusions, all of them "this is a different kind of thing" rather
/// than a judgement about content.** The first run of the advisory printed seventeen
/// tokens for two real findings, and an advisory nobody reads is worse than none —
/// so these remove classes, not cases:
///
/// - **A dotted path** (`problem.layout.bindings`) never appears literally in JSON, where
///   it is nested keys. It is also **already checked properly** by [`check`], so flagging
///   it here is noise about something a real resolver has ruled on.
/// - **Parentheses** make it a call or an annotation — `pre(v)`, `experiment(...)`.
/// - **A file extension** makes it a file: `CLAUDE.md`, `worker.rs`.
/// - **Very short** tokens are `h`, `v`, `e` — Modelica names whose presence in IR says
///   nothing, since a single letter matches almost any JSON.
///
/// What survives is what the scan is for: an identifier-shaped or subscript-shaped token
/// asserted to be in a pane. `Y[1]` and `Minus` both survive.
fn could_name_a_node(token: &str) -> bool {
    if token.len() < 3 || token.contains(' ') || token.contains('/') {
        return false;
    }
    if token.contains('(') || token.contains(')') {
        return false;
    }
    if token.ends_with(".md") || token.ends_with(".rs") || token.ends_with(".mo") {
        return false;
    }
    // A trailing dot makes it a prose fragment naming a prefix — `conditions.`,
    // `discrete.` — not a name anything could carry.
    if token.ends_with('.') {
        return false;
    }
    // A dotted path with identifier-ish segments, as against a name that merely
    // contains a dot. `bridge::parse_path` is the arbiter, since it is what a link
    // would be resolved with.
    if token.contains('.') && bridge::parse_path(token).is_some_and(|segs| segs.len() > 1) {
        return false;
    }
    true
}

/// Every single-backtick span in the text.
///
/// Fenced blocks are skipped: their content is quoted *output*, and a fence's opening
/// line carries a language tag that is not a claim about anything.
fn backticked_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let mut rest = line;
        while let Some(open) = rest.find('`') {
            let after = &rest[open + 1..];
            match after.find('`') {
                Some(close) => {
                    let token = &after[..close];
                    if !token.is_empty() {
                        out.push(token.to_owned());
                    }
                    rest = &after[close + 1..];
                }
                None => break,
            }
        }
    }
    out
}

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

    /// The advisory scan finds a token no stage contains, and ignores ones they do.
    ///
    /// `Y[1]` and `Minus` are the two real 2026-09-03 defects; `initial_y` is the
    /// control, and without it a scan that flagged everything would pass this test.
    #[test]
    fn the_token_scan_finds_the_invented_notation_and_spares_the_real_names() {
        let text = "The state is `Y[1]`, updated as a `Minus` over a `Mul`, and \
                    `initial_y` holds its start values.";
        let absent = tokens_absent_from_every_stage(text, &[solve_ir()]);

        assert!(absent.contains(&"Y[1]".to_owned()), "got: {absent:?}");
        assert!(absent.contains(&"Minus".to_owned()), "got: {absent:?}");
        assert!(
            !absent.contains(&"initial_y".to_owned()),
            "a real node name must not be flagged, or the scan is noise: {absent:?}",
        );
    }

    /// The advisory drops the four classes that made it unreadable, and keeps the signal.
    ///
    /// Seventeen tokens for two real findings is an advisory nobody reads. Each exclusion
    /// here is a different *kind* of thing rather than a judgement about content.
    #[test]
    fn the_advisory_excludes_kinds_of_token_that_are_never_node_names() {
        let text = "A dotted `problem.layout.nonesuch`, a call `pre(v)`, a file \
                    `CLAUDE.md`, a letter `h` \u{2014} and a real find, `Y[9]`.";
        let absent = tokens_absent_from_every_stage(text, &[solve_ir()]);

        assert_eq!(
            absent,
            vec!["Y[9]".to_owned()],
            "only the subscript-shaped claim survives: {absent:?}",
        );
    }

    /// A subscript is spared when its base holds an ARRAY, and flagged otherwise.
    ///
    /// The line between the noise and the original catch, and the two are the same shape.
    /// `f_x` is an array, so `f_x[0]` addresses something. `Y` is an **object** — a name
    /// mapped to a slot — so `Y[1]` addresses nothing, which is exactly why Doug could not
    /// find it. **"The base exists as a key" fails here**: `Y` exists, and that version of
    /// the rule excluded the very defect this scan was built for.
    #[test]
    fn a_subscript_is_spared_only_when_its_base_holds_an_array() {
        let ir = serde_json::json!({
            "f_x": [1, 2],
            "bindings": { "v": { "Y": { "index": 1 } } },
        });
        let text = "Hover `f_x[0]` and `f_x[1]`, but `Y[1]` is my invention, and \
                    `conditions.` is a prefix.";
        let absent = tokens_absent_from_every_stage(text, &[ir]);

        assert_eq!(
            absent,
            vec!["Y[1]".to_owned()],
            "`Y` is present as a key and still must not spare `Y[1]`: {absent:?}",
        );
    }

    /// Fenced blocks are quoted output, not claims, so their contents are not scanned.
    #[test]
    fn a_fenced_block_is_not_scanned_for_tokens() {
        let text = "```text\n`Nonesuch`\n```\nProse mentions `AlsoNonesuch`.";
        let absent = tokens_absent_from_every_stage(text, &[solve_ir()]);
        assert!(absent.contains(&"AlsoNonesuch".to_owned()));
        assert!(!absent.contains(&"Nonesuch".to_owned()), "got: {absent:?}");
    }
}
