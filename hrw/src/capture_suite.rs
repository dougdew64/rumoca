//! **The capture complement of `every_offered_sub_view_has_something_to_draw`.**
//!
//! # The one sentence this exists to falsify
//!
//! That test walks every sub-view the app *offers* and asks whether a pane can find
//! data for it. Its own doc states what it cannot do, and `docs/tech-debt.md` repeats
//! it:
//!
//! > It is **not** a claim about pixels; `egui_kittest` cannot reach a painted cell,
//! > so a tab whose data parses can still look wrong, and **only Doug can see that**.
//!
//! Written 2026-11-04, hours before [`crate::screenshot`] existed. This module is the
//! same enumeration driven through a different instrument: the test says *the pane
//! finds data*, the suite says *the pane drew it*.
//!
//! # Why it is not a mirror of the rest of `ui_tests.rs`
//!
//! Fifty-odd headless tests assert facts the accessibility tree can see — a notice
//! exists, a tab row's labels, a split fraction, a scroll offset. Photographing those
//! would assert the same fact more slowly, needing a window and a reader, and two
//! expressions of one fact is how a tab row came to draw five of eight tabs while a
//! test agreed with it. **The suite covers only the column `ui_tests.rs`'s own header
//! marks as not queryable:** the incidence matrix, the spy plot, the Tarjan graph and
//! the animations.
//!
//! # What has a number, and what does not
//!
//! The runner checks everything with a number — the reply carries no `error`, the view
//! it reports is the one asked for, the PNG decodes at the size the reply claims, a
//! `pane` request was answered with a pane. **What is left for the reader is the one
//! question none of those answers: is anything drawn, and can it be read.** That split
//! is `ui_tests.rs`'s own rule — *"when a layout defect is reported, ask what number
//! was wrong… what stays genuinely his is what has no number"* — applied one layer out.
//!
//! # It is a survey, not a gate
//!
//! Nothing here asserts pixel equality, so a drift nobody photographs stays invisible,
//! and the suite is run deliberately rather than on every commit: after a rebase, after
//! touching a painter, after a stage re-model. Saying so plainly is the point — the
//! failure mode of a half-automated instrument is being read as coverage it does not
//! provide.
//!
//! # Availability is ASKED, never re-derived
//!
//! The suite requests **every** canvas view on every stage and reads what came back.
//! It does not compute which views are offered, because a checker that re-derived
//! availability instead of asking the app is exactly the defect that let the Index
//! Reduction tab row draw tabs with nothing behind them — and the first fix for it
//! passed while the app disagreed. A view the app refuses answers with a `notice`
//! saying so, which is the app's own answer and is recorded as [`Outcome::Refused`].

use serde_json::Value;

use crate::bridge;

/// The sub-views that are drawn with `Painter` calls, and so are invisible to
/// `egui_kittest`.
///
/// **The roster is checked against the enum** by
/// [`tests::every_structural_view_is_either_photographed_or_excused`], so a new
/// sub-view forces a decision here rather than being silently never photographed —
/// the "forgetting is loud" rule `StructuralView::ALL` already carries.
pub const CANVAS_VIEWS: &[&str] = &[
    "SpyPlot",
    "Incidence",
    "MatchingAnim",
    "TarjanAnim",
    "TearingAnim",
    "Animate",
];

/// The sub-views the accessibility tree can already reach, which therefore need no
/// photograph.
///
/// `Tree` is the generic serde inspector and `Summary` is labels and a table; both are
/// queryable, and both are asserted on in `ui_tests.rs` and `app/tests.rs`.
pub const QUERYABLE_VIEWS: &[&str] = &["Summary", "Tree"];

/// The stages that have canvas sub-views.
pub const CANVAS_STAGES: &[&str] = &["Structural", "IndexReduction"];

/// One photograph to take: a stage, a sub-view within it, and the link that gets there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shot {
    pub stage: &'static str,
    pub view: &'static str,
}

impl Shot {
    /// The `hrw://` link that navigates to this shot, given a specimen.
    ///
    /// **One link, not two.** `hrw://load/<Specimen>/<Stage>/<SubView>` loads and
    /// switches in a single dispatch, which `dispatch_hrw_link`'s own comment records
    /// as the verb that got the sub-view ordering right when its three siblings did
    /// not. Issuing a load and then a stage switch would re-enter the path whose
    /// sub-view reset silently overwrites the first one.
    #[must_use]
    pub fn link(&self, specimen: &str) -> String {
        format!("hrw://load/{specimen}/{}/{}", self.stage, self.view)
    }

    /// A short stable name for an index row.
    #[must_use]
    pub fn label(&self, specimen: &str) -> String {
        format!("{specimen} / {} / {}", self.stage, self.view)
    }
}

/// Every canvas view on every canvas stage — the full request list for one specimen.
///
/// Deliberately **not** filtered by what the app offers; see the module header.
#[must_use]
pub fn plan() -> Vec<Shot> {
    let mut shots = Vec::with_capacity(CANVAS_STAGES.len() * CANVAS_VIEWS.len());
    for stage in CANVAS_STAGES {
        for view in CANVAS_VIEWS {
            shots.push(Shot { stage, view });
        }
    }
    shots
}

/// What came back for one shot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The app went where it was asked and photographed it. The reader looks at this.
    Landed { path: String, size: (u64, u64) },
    /// **The app refused the link and said why.** Not an error: it is the app
    /// answering *"this stage has no such view for this model"*, which is exactly the
    /// fact the suite would otherwise have had to compute for itself.
    Refused { notice: String },
    /// Something went wrong that is nobody's answer to anything.
    Failed { error: String },
}

/// Classify one reply against the shot that asked for it.
///
/// **Four things must agree before a shot counts as landed**, and each disagreement
/// has produced a defect in this project: the reply carries no `error`; the app is on
/// the stage asked for; the sub-view it reports is the one asked for; and there is an
/// image. A reply that merely *has a PNG* proves none of the rest — the app can refuse
/// a link, stay where it was, and be photographed looking perfectly healthy.
#[must_use]
pub fn classify(shot: &Shot, specimen: &str, reply: &Value) -> Outcome {
    if let Some(error) = reply.get("error").and_then(Value::as_str) {
        return Outcome::Failed {
            error: error.to_string(),
        };
    }
    // The notice is checked BEFORE the view, because a refusal leaves the app on the
    // view it was already showing — which can coincidentally be the one requested, and
    // then a view-only check calls a refusal a success.
    if let Some(notice) = reply.get("notice").and_then(Value::as_str) {
        return Outcome::Refused {
            notice: notice.to_string(),
        };
    }
    let on_specimen = reply.get("specimen").and_then(Value::as_str);
    if on_specimen != Some(specimen) {
        return Outcome::Failed {
            error: format!(
                "asked for {specimen} and the app is showing {}",
                on_specimen.unwrap_or("no specimen")
            ),
        };
    }
    // `StageKind::name` spells these with a space ("Index reduction") while the link
    // slug does not ("IndexReduction"). Comparing them raw would fail every Index
    // Reduction shot, so the comparison ignores spacing rather than either side being
    // rewritten to match the other — both spellings are owned elsewhere.
    let on_stage = reply.get("stage").and_then(Value::as_str).unwrap_or("");
    if !same_stage(on_stage, shot.stage) {
        return Outcome::Failed {
            error: format!(
                "asked for stage {} and the app is on {on_stage}",
                shot.stage
            ),
        };
    }
    let on_view = reply
        .pointer("/showing/stage_view")
        .and_then(Value::as_str)
        .unwrap_or("");
    if on_view != shot.view {
        return Outcome::Failed {
            error: format!(
                "asked for sub-view {} and the app is showing {on_view}",
                shot.view
            ),
        };
    }
    let Some(image) = reply.get("image").filter(|i| !i.is_null()) else {
        return Outcome::Failed {
            error: "the reply reports no error and no image, which is neither a \
                    photograph nor a reason"
                .to_string(),
        };
    };
    Outcome::Landed {
        path: image
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        size: (
            image.get("width").and_then(Value::as_u64).unwrap_or(0),
            image.get("height").and_then(Value::as_u64).unwrap_or(0),
        ),
    }
}

/// Whether two stage spellings name the same stage, ignoring spaces and case.
///
/// The link slug is `IndexReduction` and `StageKind::name` is `Index reduction`.
/// Neither is wrong and neither is this module's to change.
fn same_stage(a: &str, b: &str) -> bool {
    let normalise = |s: &str| -> String {
        s.chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    };
    normalise(a) == normalise(b)
}

/// One row of the run index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub shot: Shot,
    pub seq: u64,
    pub outcome: Outcome,
}

/// Render the run index that the reader opens.
///
/// Markdown rather than JSON because **the consumer is a reader, not a program**: the
/// runner has already made every machine-checkable judgement by the time this is
/// written, and what is left is a list of pictures to look at with their labels.
///
/// The refused rows are kept and shown. A view the app declines is a fact about the
/// model, and omitting it would make the index silently shorter on exactly the
/// specimens where the reduction funnel did the most work.
#[must_use]
pub fn index_markdown(specimen: &str, region: bridge::ShotRegion, rows: &[Row]) -> String {
    use std::fmt::Write as _;
    let landed = rows
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Landed { .. }))
        .count();
    let refused = rows
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Refused { .. }))
        .count();
    let failed = rows
        .iter()
        .filter(|r| matches!(r.outcome, Outcome::Failed { .. }))
        .count();

    let mut out = String::new();
    let _ = writeln!(out, "# Capture suite \u{2014} {specimen}");
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "The canvas sub-views of {}, photographed with `region: {}`. \
         {landed} landed, {refused} refused by the app, {failed} failed.",
        CANVAS_STAGES.join(" and "),
        region.as_str(),
    );
    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "A **refused** row is the app answering \u{2014} the stage has no such view for \
         this model \u{2014} and is not a defect. A **failed** row is."
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "| seq | stage | view | outcome | image |");
    let _ = writeln!(out, "|---|---|---|---|---|");
    for row in rows {
        let (outcome, image) = match &row.outcome {
            Outcome::Landed { path, size } => (
                format!("landed, {}x{}", size.0, size.1),
                format!("`{path}`"),
            ),
            Outcome::Refused { notice } => (
                format!("refused \u{2014} {notice}"),
                String::from("\u{2014}"),
            ),
            Outcome::Failed { error } => (
                format!("**FAILED** \u{2014} {error}"),
                String::from("\u{2014}"),
            ),
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {outcome} | {image} |",
            row.seq, row.shot.stage, row.shot.view
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A reply shaped like the real one, landed on the shot asked for.
    fn landed_reply(stage: &str, view: &str) -> Value {
        json!({
            "seq": 1,
            "specimen": "Drivetrain",
            "stage": stage,
            "showing": {"stage_view": view},
            "image": {"path": "/tmp/shot-0001.png", "width": 1200, "height": 900},
            "notice": null,
            "error": null,
        })
    }

    /// **Every sub-view is either photographed or excused by name.**
    ///
    /// The roster this guards is the same shape as `StructuralView::ALL`, and for the
    /// same reason: a new variant that nobody listed here would simply never be
    /// photographed, and the suite would report a clean run over a smaller surface
    /// than it claims to cover. That is the wrong-negative shape this repository
    /// treats as the error nobody catches, because acting on it means not looking.
    #[test]
    fn every_structural_view_is_either_photographed_or_excused() {
        let listed: Vec<&str> = CANVAS_VIEWS
            .iter()
            .chain(QUERYABLE_VIEWS.iter())
            .copied()
            .collect();
        for view in crate::stage_view::StructuralView::ALL {
            let name = crate::stage_view::structural_view_name(*view);
            assert!(
                listed.contains(&name),
                "`{name}` is in StructuralView::ALL and in neither CANVAS_VIEWS nor \
                 QUERYABLE_VIEWS, so the capture suite would never photograph it and \
                 would report a clean run anyway"
            );
        }
        assert_eq!(
            listed.len(),
            crate::stage_view::StructuralView::ALL.len(),
            "the two rosters together must name each view exactly once"
        );
    }

    /// The plan is the full product, unfiltered — availability is the app's to say.
    #[test]
    fn the_plan_requests_every_canvas_view_on_every_canvas_stage() {
        let plan = plan();
        assert_eq!(plan.len(), CANVAS_STAGES.len() * CANVAS_VIEWS.len());
        assert!(
            plan.iter()
                .any(|s| s.stage == "IndexReduction" && s.view == "MatchingAnim"),
            "the plan must ask for views the app is expected to REFUSE \u{2014} the \
             refusal is the app's own answer, and computing it here instead is the \
             defect that let the tab row draw tabs with nothing behind them"
        );
        assert_eq!(
            plan[0].link("Drivetrain"),
            "hrw://load/Drivetrain/Structural/SpyPlot"
        );
    }

    /// A landed shot needs the specimen, the stage, the view and an image to agree.
    #[test]
    fn a_shot_lands_only_when_everything_agrees() {
        let shot = Shot {
            stage: "Structural",
            view: "Incidence",
        };
        let Outcome::Landed { size, .. } = classify(
            &shot,
            "Drivetrain",
            &landed_reply("Structural", "Incidence"),
        ) else {
            panic!("a reply that agrees on every field is a landed shot");
        };
        assert_eq!(size, (1200, 900));

        // The stage spellings differ by a space and a case, and both are owned
        // elsewhere; a shot must not fail for that.
        let reduction = Shot {
            stage: "IndexReduction",
            view: "SpyPlot",
        };
        assert!(
            matches!(
                classify(
                    &reduction,
                    "Drivetrain",
                    &landed_reply("Index reduction", "SpyPlot")
                ),
                Outcome::Landed { .. }
            ),
            "`Index reduction` and `IndexReduction` name one stage"
        );
    }

    /// **A refusal must not be read as a success, even when the view happens to match.**
    ///
    /// This is the case the ordering inside `classify` exists for. HRW refuses a link
    /// by showing a notice and *staying where it is*, so a reply can carry the
    /// requested view, a healthy image, and a refusal all at once.
    #[test]
    fn a_refusal_is_refused_even_when_the_view_matches() {
        let shot = Shot {
            stage: "IndexReduction",
            view: "MatchingAnim",
        };
        let mut reply = landed_reply("Index reduction", "MatchingAnim");
        reply["notice"] = json!("Index reduction has no MatchingAnim view for this model");
        let Outcome::Refused { notice } = classify(&shot, "Drivetrain", &reply) else {
            panic!("a notice means the app declined, whatever the rest of the reply says");
        };
        assert!(notice.contains("no MatchingAnim view"));
    }

    /// Each way a reply can disagree is a failure that names what disagreed.
    #[test]
    fn every_disagreement_fails_and_says_which() {
        let shot = Shot {
            stage: "Structural",
            view: "Incidence",
        };
        let cases: Vec<(&str, Value, &str)> = vec![
            (
                "wrong specimen",
                {
                    let mut r = landed_reply("Structural", "Incidence");
                    r["specimen"] = json!("CapacitorLoop");
                    r
                },
                "CapacitorLoop",
            ),
            (
                "wrong stage",
                landed_reply("Flatten", "Incidence"),
                "Flatten",
            ),
            (
                "wrong view",
                landed_reply("Structural", "SpyPlot"),
                "SpyPlot",
            ),
            (
                "no image and no reason",
                {
                    let mut r = landed_reply("Structural", "Incidence");
                    r["image"] = json!(null);
                    r
                },
                "neither a photograph nor a reason",
            ),
        ];
        for (case, reply, needle) in cases {
            let Outcome::Failed { error } = classify(&shot, "Drivetrain", &reply) else {
                panic!("{case}: must fail");
            };
            assert!(
                error.contains(needle),
                "{case}: the error must name what disagreed, got {error}"
            );
        }
    }

    /// The index names every row, keeps the refusals, and marks the failures loudly.
    #[test]
    fn the_index_keeps_refusals_and_shouts_failures() {
        let rows = vec![
            Row {
                shot: Shot {
                    stage: "Structural",
                    view: "Incidence",
                },
                seq: 10,
                outcome: Outcome::Landed {
                    path: "/tmp/shot-0010.png".to_string(),
                    size: (1200, 900),
                },
            },
            Row {
                shot: Shot {
                    stage: "IndexReduction",
                    view: "TarjanAnim",
                },
                seq: 11,
                outcome: Outcome::Refused {
                    notice: "no TarjanAnim view for this model".to_string(),
                },
            },
            Row {
                shot: Shot {
                    stage: "Structural",
                    view: "SpyPlot",
                },
                seq: 12,
                outcome: Outcome::Failed {
                    error: "the app is on Flatten".to_string(),
                },
            },
        ];
        let out = index_markdown("Drivetrain", bridge::ShotRegion::Pane, &rows);
        assert!(out.contains("1 landed, 1 refused by the app, 1 failed"));
        assert!(out.contains("shot-0010.png"));
        assert!(
            out.contains("no TarjanAnim view for this model"),
            "a refused row stays in the index: it is a fact about the model, and \
             dropping it makes the index silently shorter"
        );
        assert!(
            out.contains("**FAILED**"),
            "a failure must be impossible to skim past"
        );
        assert!(
            out.contains("`region: pane`"),
            "the index says how much was kept"
        );
    }
}
