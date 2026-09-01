//! **The default artifact pane** — the stage's IR as a tree, and the error summary
//! beside it.
//!
//! Lifted out of `central_panel_ui` on 2026-08-21, the fourth cut into that router. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # What this is a member of, and why it is the one that has no gate
//!
//! `central_panel_ui`'s pane dispatch is a chain of `else if` arms, every one of them
//! guarded by a sub-view selection (`report_ready && … == SpyPlot`, `flatten_ready && …
//! == Equations`, and so on). **This is the final `else`** — the arm with no condition at
//! all, which is what makes it the odd member: the others answer *"is this particular
//! pane selected?"* and this one answers *"nothing else claimed the frame."*
//!
//! That is also why it draws for **most of the stages**. Parse, Resolve, Instantiate, Dae
//! and Typecheck have no sub-views at all, so their whole on-screen life is this
//! function. It is the most-drawn pane in HRW and was the last one still inline.
//!
//! # The summary sits BESIDE the artifact, not instead of it
//!
//! Doug, 2026-08-05, walking `failure-typecheck.md`: *"there is no tree in the failing
//! typecheck stage view."* There was one in the data — `DimensionMismatch`'s Typecheck
//! stage carries 7.4 KB of instantiated overlay **plus** an `error` key, assembled by the
//! worker on purpose, whose comment reads *"the instantiated overlay is the last good
//! state to show **beside** them"*. The pane rendered the summary in an `if` with the
//! whole tree as the `else`, so **beside became instead of**, and the overlay was
//! discarded at the last step with nothing on screen saying content had been withheld.
//!
//! **The condition is not the outcome**, which is the part worth having in mind when
//! reading [`artifact_pane_ui`]. [`Stage::note_is_error`] is true for *both* abnormal
//! outcomes, and they differ in what the value holds:
//!
//! | constructor | outcome | `value` | what belongs on screen |
//! |---|---|---|---|
//! | [`Stage::recovered`] | `Flagged` | a real IR **plus** an `error` key | both — summary above, tree below |
//! | [`Stage::err_with_details`] | `Failed` | **only** `{"error": …}` | the summary alone |
//!
//! So the test is not the outcome but whether the value carries anything **beyond**
//! `error` — [`has_content_beside_the_error`], which is the question this module makes
//! checkable without an `App`.
//!
//! # A node link that does not resolve must SAY so
//!
//! The tree otherwise expands as far as the path goes and stops, which reads as *"it
//! opened something"* rather than *"that path is wrong"* — the silent partial failure the
//! aim and seek verbs deliberately avoid. [`resolve_jump_target`] is the check, and it
//! runs against the value **actually about to be drawn**, so a link naming a path from
//! another stage is reported rather than half-followed.
//!
//! The complaint travels back to the caller as a returned `Option<String>` rather than
//! being posted here: it is collected while `stage` is borrowed and acted on after, the
//! same deferred-intent pattern as `FrameIntent`. That is also what leaves this function
//! with **no `&mut App` at all**.

use std::collections::{BTreeMap, HashMap};

use eframe::egui;
use serde_json::Value;

use crate::bridge::{self, Seg};
use crate::tree;
use crate::worker::{DefInfo, Stage, StageKind};

/// What the pane says about the specimen, as opposed to about the stage.
///
/// Bundled for the [`tree::TreeOptions`] reason — four more positional arguments on a
/// signature that already carries two bundles.
pub(crate) struct ArtifactChrome<'a> {
    /// The label on the tree's root node — the model's name, or `"model"` when there
    /// is not one.
    pub(crate) label: &'a str,
    /// The previous stage's IR, aligned to this stage's tree root, for the "changed by
    /// this stage" highlight. `None` for Parse, which has no previous stage.
    pub(crate) prev: Option<&'a Value>,
    /// How many identifiers the compile found, for the line above the tree.
    ///
    /// A count rather than the set, because the count is all this pane shows. `None`
    /// before a successful compile, and then the line is omitted entirely rather than
    /// reading "0 identifier(s)" — absence is stated, never filled.
    pub(crate) identifier_count: Option<usize>,
    /// Whether a compile is in flight, which decides between "compiling…" and "(no
    /// output for this stage)" on a stage with nothing to show.
    pub(crate) compiling: bool,
}

/// Everything the tree widget is handed, plus the two node addresses this pane resolves.
///
/// `opts` arrives carrying the specimen's annotations (`App::specimen_tree_options`);
/// **this pane fills in `jump_to` and `highlight` itself** from the two fields below,
/// overwriting whatever they held. It has to: `jump_to` is only legal once the path has
/// been checked against the value about to be drawn, and doing that here is what keeps
/// the check and the use from drifting apart.
pub(crate) struct ArtifactTree<'a> {
    /// The specimen's annotations — tracked identifier, known variables, source lines.
    pub(crate) opts: tree::TreeOptions<'a>,
    /// DefId → resolved name, for the inline `type_def_id` annotations.
    pub(crate) def_index: &'a BTreeMap<u64, DefInfo>,
    /// Field name → help text, for the tooltips.
    pub(crate) field_help: &'a HashMap<String, String>,
    /// The node a link asked to scroll to, **before** it has been checked.
    pub(crate) jump_target: Option<&'a [Seg]>,
    /// The node a link pointed at, washed so it stays findable after the scroll.
    ///
    /// Not checked, and deliberately: it only tints a row that the tree draws anyway, so
    /// a stale address paints nothing rather than misleading. `jump_target` moves the
    /// viewport, which is why only that one is validated.
    pub(crate) jump_highlight: Option<&'a [Seg]>,
}

/// Does this stage's value carry anything besides its error payload?
///
/// **The question that decides "beside" from "instead of"**, split out of the pane so it
/// can be asserted without painting: `true` means a real IR is present and the summary
/// folds above it, `false` means the value *is* the error and the summary fills the pane.
///
/// A stage with no value at all answers `false` — there is nothing to draw beside.
pub(crate) fn has_content_beside_the_error(stage: &Stage) -> bool {
    stage
        .value
        .as_ref()
        .and_then(Value::as_object)
        .is_some_and(|o| o.keys().any(|k| k != "error"))
}

/// Report whether a link's node path exists in the value about to be drawn.
///
/// `Ok(())` when it resolves; `Err(message)` naming the path when it does not. An empty
/// path is the root and always resolves.
///
/// **Why validate at all:** the tree otherwise expands as far as the path goes and stops,
/// which reads as "it opened something" rather than "that path is wrong". The camera aim
/// and the frame seek both refuse-and-report; this makes the third verb consistent. A
/// link naming something that is not there is a bug in the lab, and must be visible.
///
/// A free function rather than a method because the caller holds an immutable borrow of
/// the stage while rendering, so it cannot also take `&mut self` — the same constraint
/// that makes [`artifact_pane_ui`] *return* the message instead of posting it. Moved here
/// from `app.rs` with its only caller on 2026-08-21.
pub(crate) fn resolve_jump_target(stage_value: &Value, target: &[Seg]) -> Result<(), String> {
    if target.is_empty() || bridge::navigate(stage_value, target).is_some() {
        return Ok(());
    }
    Err(format!(
        "no node at {} in this stage \u{2014} the link names a path that is not here",
        bridge::describe_path(target),
    ))
}

/// Draw a stage's artifact: its error summary, its identifier count and its IR tree.
///
/// Returns the complaint about an unresolvable node link, if there was one. The caller
/// owns `context.jump_target` and is the only thing that may clear it — see the module
/// docs for why this function takes no `&mut App`.
pub(crate) fn artifact_pane_ui(
    ui: &mut egui::Ui,
    stage: &Stage,
    stage_kind: StageKind,
    chrome: ArtifactChrome<'_>,
    tree_in: ArtifactTree<'_>,
    actions: &mut tree::TreeActions,
) -> Option<String> {
    // Set when a `hrw://…/node/<path>` link names a path this stage does not have.
    let mut bad_jump: Option<String> = None;

    // See the module docs: `note_is_error()` is true for both abnormal outcomes, so the
    // test is what the value holds, not what the outcome was.
    let error_data = stage
        .note_is_error()
        .then(|| stage.value.as_ref().and_then(|v| v.get("error")).cloned())
        .flatten();
    let has_other_content = has_content_beside_the_error(stage);

    if let Some(error) = error_data.clone().filter(|_| !has_other_content) {
        // Nothing but the error: the summary fills the pane, and there is no tree below
        // for a jump to land in.
        egui::ScrollArea::vertical()
            .id_salt("error_summary")
            .auto_shrink(false)
            .show(ui, |ui| {
                crate::error_summary::generic_error_summary(ui, &error, stage_kind);
            });
        return bad_jump;
    }

    // **Collapsible, and open by default.** The summary is the more urgent of the two and
    // goes first, but a reader who has read it and wants the whole overlay can fold it
    // away rather than scroll past it on every visit.
    if let Some(error) = error_data {
        egui::CollapsingHeader::new("\u{26a0} What went wrong")
            .id_salt("error_summary_beside_tree")
            .default_open(true)
            .show(ui, |ui| {
                crate::error_summary::generic_error_summary(ui, &error, stage_kind);
            });
        ui.separator();
    }

    match &stage.value {
        Some(value) => {
            // **The count, without the checkbox that used to sit beside it.** "Reveal
            // identifiers" was removed 2026-08-04 (`DECISIONS.md`). The count is a plain
            // fact about the model and costs one line, so it stays; finding a
            // *particular* identifier is what Follow does, and it scrolls to the match
            // instead of opening every path that might contain one.
            if let Some(n) = chrome.identifier_count {
                ui.weak(format!(
                    "{n} identifier(s) in this model \u{2014} right-click an underlined \
                     value to follow one",
                ));
            }
            let jump_to = match tree_in.jump_target {
                Some(t) => match resolve_jump_target(value, t) {
                    Ok(()) => Some(t),
                    Err(msg) => {
                        bad_jump = Some(msg);
                        None
                    }
                },
                None => None,
            };
            // **The two node addresses are what this tree adds** to what every tree is
            // told about the specimen; see `App::specimen_tree_options` for the rest.
            let opts = tree::TreeOptions {
                jump_to,
                highlight: tree_in.jump_highlight,
                ..tree_in.opts
            };
            egui::ScrollArea::both()
                .id_salt("tree")
                .auto_shrink(false)
                .show(ui, |ui| {
                    tree::tree_ui(
                        ui,
                        chrome.label,
                        value,
                        chrome.prev,
                        actions,
                        tree_in.def_index,
                        tree_in.field_help,
                        opts,
                    );
                });
        }
        None if stage.note.is_none() => {
            ui.weak(if chrome.compiling {
                "compiling…"
            } else {
                "(no output for this stage)"
            });
        }
        // A stage with a note and no value has already said its piece: the note is drawn
        // as a banner above this pane by `central_panel_ui`. Drawing anything here would
        // be a second, weaker copy of it.
        None => {}
    }

    bad_jump
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use serde_json::json;

    /// The root label, distinctive enough that finding it means the *tree* drew.
    const MODEL: &str = "RcCircuit";
    /// The collapsing header the summary hides under when it shares the pane.
    const BESIDE_HEADING: &str = "What went wrong";

    /// What the pane was handed, plus what it handed back.
    ///
    /// **Sized like a pane** (`1200×900`): a clipped widget stays in the accessibility
    /// tree while behaving as though it is not there, so a query finds it and a click
    /// does nothing. Four earlier modules lost time to that before it was written down.
    struct Pane {
        stage: Stage,
        kind: StageKind,
        compiling: bool,
        jump_target: Option<Vec<Seg>>,
        /// The complaint the pane returned, accumulated rather than assigned — the press
        /// on the second frame would otherwise overwrite the first frame's report.
        notice: Option<String>,
    }

    impl Pane {
        fn showing(stage: Stage) -> Self {
            Pane {
                stage,
                kind: StageKind::Structural,
                compiling: false,
                jump_target: None,
                notice: None,
            }
        }
    }

    fn draw(pane: Pane) -> Harness<'static, Pane> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui_state(
                |ui, p: &mut Pane| {
                    let def_index = BTreeMap::new();
                    let field_help = HashMap::new();
                    let mut actions = tree::TreeActions::default();
                    let notice = artifact_pane_ui(
                        ui,
                        &p.stage,
                        p.kind,
                        ArtifactChrome {
                            label: MODEL,
                            prev: None,
                            identifier_count: None,
                            compiling: p.compiling,
                        },
                        ArtifactTree {
                            opts: tree::TreeOptions::default(),
                            def_index: &def_index,
                            field_help: &field_help,
                            jump_target: p.jump_target.as_deref(),
                            jump_highlight: None,
                        },
                        &mut actions,
                    );
                    if notice.is_some() {
                        p.notice = notice;
                    }
                },
                pane,
            );
        h.run_steps(2);
        h
    }

    /// **The summary sits BESIDE the artifact, not instead of it.**
    ///
    /// Doug, 2026-08-05: *"there is no tree in the failing typecheck stage view."* A
    /// `Flagged` stage carries a real IR **and** an error, and rendering the summary in an
    /// `if` with the tree as the `else` discarded 7.4 KB of instantiated overlay with
    /// nothing on screen saying so. Both must be present, and the summary must be the one
    /// that folds.
    #[test]
    fn a_flagged_stage_shows_its_error_beside_its_artifact() {
        let h = draw(Pane::showing(Stage::recovered(
            json!({
                "error": { "kind": "singular" },
                "equations": [{ "lhs": "der(v)" }],
            }),
            "singular",
        )));

        assert!(
            h.query_by_label_contains(BESIDE_HEADING).is_some(),
            "a flagged stage must show why it was flagged",
        );
        assert!(
            h.query_by_label_contains(MODEL).is_some(),
            "and it must still show the artifact the compiler did produce",
        );
    }

    /// The non-vacuity partner: a stage whose value **is** the error gets no tree.
    ///
    /// Without this, a pane that drew the tree unconditionally would pass the test above.
    /// `err_with_details` builds `{"error": …}` and nothing else, so a tree here would
    /// render the error payload as an IR — noise beside the summary that already says it.
    #[test]
    fn a_failed_stage_shows_the_summary_alone() {
        let h = draw(Pane::showing(Stage::err_with_details(
            json!({ "kind": "singular" }),
            "singular",
        )));

        assert!(
            h.query_by_label_contains(MODEL).is_none(),
            "a value that is nothing but an error has no artifact to draw",
        );
        assert!(
            h.query_by_label_contains(BESIDE_HEADING).is_none(),
            "and the summary fills the pane rather than folding into a header",
        );
    }

    /// The predicate that decides between the two panes above, asserted without painting.
    #[test]
    fn only_a_value_with_more_than_an_error_has_content_beside_it() {
        assert!(
            has_content_beside_the_error(&Stage::recovered(
                json!({ "error": {}, "equations": [] }),
                "flagged",
            )),
            "an IR alongside the error is content to draw beside it",
        );
        assert!(
            !has_content_beside_the_error(&Stage::err_with_details(json!({}), "failed")),
            "a value that is only the error is not",
        );
        assert!(
            !has_content_beside_the_error(&Stage::info("did not run")),
            "and a stage with no value at all has nothing to draw beside",
        );
    }

    /// A link naming a path this stage does not have is **reported**, and the tree still
    /// draws.
    ///
    /// The two halves matter together: refusing the jump silently would leave the reader
    /// looking at a tree that did not move, and refusing to draw would lose the artifact
    /// over a bad address. The pane returns the complaint and renders anyway.
    #[test]
    fn an_unresolvable_link_is_reported_and_the_artifact_still_draws() {
        let mut pane = Pane::showing(Stage::ok(json!({ "equations": [] })));
        pane.jump_target = Some(bridge::parse_path("no.such.node").expect("well-formed"));
        let h = draw(pane);

        let notice = h
            .state()
            .notice
            .as_deref()
            .expect("a path that is not in the stage must be reported");
        assert!(
            notice.contains("no.such.node"),
            "the complaint names the path: {notice}",
        );
        assert!(
            h.query_by_label_contains(MODEL).is_some(),
            "a bad address must not cost the reader the artifact",
        );
    }

    /// The partner: a path that *is* there produces no complaint.
    ///
    /// Without it, a pane that reported every jump would pass the test above.
    #[test]
    fn a_resolvable_link_is_not_reported() {
        let mut pane = Pane::showing(Stage::ok(json!({ "equations": [] })));
        pane.jump_target = Some(bridge::parse_path("equations").expect("well-formed"));
        let h = draw(pane);

        assert!(
            h.state().notice.is_none(),
            "a path the stage has must be followed in silence",
        );
    }

    /// A stage with nothing to show says which kind of nothing it is.
    ///
    /// **Absence is stated, never filled** — and the two absences are different: a compile
    /// in flight will produce something, and a finished stage that produced nothing will
    /// not. A stage that already carries a *note* says its piece in the banner above this
    /// pane, so a third message here would be a weaker copy of one already on screen.
    #[test]
    fn a_stage_with_no_value_says_which_kind_of_nothing_it_is() {
        for (compiling, expected, absent) in [
            (true, "compiling", "(no output for this stage)"),
            (false, "(no output for this stage)", "compiling"),
        ] {
            // `Stage::default()` is the only value-less, note-less shape: every named
            // constructor that omits a value supplies a note (`err`, `info`), which is
            // the case the arm below this one deliberately leaves to the banner.
            let mut pane = Pane::showing(Stage::default());
            pane.compiling = compiling;
            let h = draw(pane);
            assert!(
                h.query_by_label_contains(expected).is_some(),
                "compiling={compiling}: the pane must say {expected:?}",
            );
            assert!(
                h.query_by_label_contains(absent).is_none(),
                "compiling={compiling}: and must not also say {absent:?}",
            );
        }
    }

    /// A node path that does not exist is reported, not half-followed.
    ///
    /// Without this the tree expands as far as the path goes and stops, which reads as
    /// "it opened something" rather than "that path is wrong". The camera aim and the
    /// frame seek both refuse-and-report; this is the third verb made consistent.
    ///
    /// Moved here from `app::tests` on 2026-08-21 with the function it exercises.
    #[test]
    fn an_unresolvable_node_path_is_reported() {
        let stage = json!({
            "error": { "unmatched_unknowns": ["gnd.p.i"] },
            "blocks": [{ "kind": "scalar" }],
        });

        // Paths that exist resolve silently.
        for good in ["", "error", "error.unmatched_unknowns[0]", "blocks[0].kind"] {
            let path = bridge::parse_path(good).expect("well-formed");
            assert_eq!(
                resolve_jump_target(&stage, &path),
                Ok(()),
                "{good:?} should resolve"
            );
        }

        // Well-formed but absent: parses fine, navigates to nothing, must be reported.
        let path = bridge::parse_path("error.matched_unknowns[0]").expect("well-formed");
        let Err(msg) = resolve_jump_target(&stage, &path) else {
            panic!("a path that is not in the stage must be reported");
        };
        assert!(
            msg.contains("error.matched_unknowns[0]"),
            "the message names the path: {msg}"
        );

        // Past the end of a real array counts as absent too.
        let path = bridge::parse_path("blocks[9]").expect("well-formed");
        assert!(resolve_jump_target(&stage, &path).is_err());
    }
}
