//! **The sub-view selector** for the report stages (Structural, Index Reduction): the
//! singularity banner, then spy plot, incidence matrix, the four animations, the tree.
//!
//! Lifted out of `central_panel_ui` on 2026-08-02, and out of `App` on 2026-08-19. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! Only ever reached when `report_ready` — the stage is a report stage *and* it produced
//! a value — which the caller checks, so this does not re-test it.
//!
//! # Where the stage-change reset lives
//!
//! [`StageViewCaches::reset_for`](crate::stage_caches::StageViewCaches::reset_for) is
//! called here rather than on the tab click, because the sub-view a reader lands on
//! depends on what the *new* stage turned out to be: singular Structural and Index
//! Reduction open on Summary, everything else on the spy plot. That decision needs the
//! report, which only exists by the time this row is drawn.
//!
//! [`default_sub_view_for`] holds that choice as a pure function, so the rule can be
//! asserted without a compile, an `App` or a frame. The old `&mut self` body could only
//! be reached by building an `App`, giving it a worker and driving a specimen to a report
//! stage — which is why nothing had ever checked it, and why a missing case had survived
//! there: **`AliasAnim` was not redirected on a stage change**, so it could stay selected
//! on a Structural stage that offers no such tab. Fixed the day the function was
//! separated; see its doc comment for the symptom.
//!
//! # Why availability arrives as a struct rather than being computed here
//!
//! Four of the tabs exist only for some models, and the predicate that decides —
//! `App::structural_view_available` — is **the same one the `hrw://` link guard uses**, so
//! that a tab which exists and a link which is honoured cannot disagree. That predicate
//! reads `frames.index_reduction`, a field this row never otherwise touches, and it is
//! cited by name from `DECISIONS.md` and `fidelity-plan.md` as living on `App`.
//!
//! So the caller answers the four questions and passes [`TabAvailability`]. One parameter
//! instead of a fifth state group, and the single-predicate invariant is untouched — the
//! row renders what it is told is available and owns no policy about it.
//!
//! # What stayed behind
//!
//! `apply_pending_view_and_seek` runs immediately *after* this row, in `App`, and the
//! order matters: the default-sub-view reset above forces Summary whenever a report stage
//! is entered singular, and a link saying "show me the matching animation" has to win over
//! it. Deferring the reset out of this row would invert that.

use eframe::egui;

use crate::stage_caches::StageViewCaches;
use crate::stage_view::StructuralView;
use crate::worker::{StageBundle, StageKind};

/// Which sub-view tabs this model has — **one field per tab the row draws**.
///
/// # It covered three of eight until 2026-11-04, and the row made up the rest
///
/// It said *"which of the four conditional tabs this model has … the other five are
/// decided from singularity alone, which this row can read off the stage note"* — and
/// that is exactly what went wrong. `Incidence` and `Matching` were drawn
/// **unconditionally**, `BLT` and `Tearing` under a `!is_singular || is_index_reduction`
/// the row recomputed from the note. So `App::structural_view_available` could refuse a
/// view and the tab appeared anyway, and the row's own comment claimed the opposite: *"a
/// tab that exists and a link that is honoured must not be able to disagree."*
///
/// **Doug found it by running the app** — Drivetrain → Index Reduction showing Matching,
/// BLT and Tearing with animation triangles and no animation behind any of them. A test
/// written the day before had asked `structural_view_available` about those views and
/// agreed with itself, because the row was not consulting it.
///
/// So every tab now has a field, the row consults it and nothing else, and
/// `tests::the_tabs_drawn_are_exactly_the_tabs_offered` compares the two sets.
///
/// Every field is `pub(crate)` because `..Default::default()` in a test needs the whole
/// struct visible, not only the fields being set (the `ContextBarState` lesson).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TabAvailability {
    /// The singular-system explanation, plus Index Reduction's own report.
    pub(crate) summary: bool,
    /// The reduction replay — Index Reduction only, and only when it logged a search.
    pub(crate) animate: bool,
    /// The BLT spy plot, which needs a complete matching to mean anything.
    pub(crate) spy_plot: bool,
    /// The incidence matrix. On Index Reduction it is the **reduced** system's, which a
    /// refused reduction does not publish.
    pub(crate) incidence: bool,
    /// The augmenting-path search. Shown even when singular on Structural — the search
    /// failing is the instructive part — and never on Index Reduction, whose frames
    /// belong to the raw system.
    pub(crate) matching_anim: bool,
    /// The SCC search. Same frame restriction as `matching_anim`.
    pub(crate) tarjan_anim: bool,
    /// Tearing a coupled block open. Same frame restriction.
    pub(crate) tearing_anim: bool,
    /// The raw IR tree, which every report stage offers.
    pub(crate) tree: bool,
}

impl TabAvailability {
    /// Ask one predicate about every tab, so no tab can be forgotten.
    ///
    /// **Built from a closure rather than field by field** because the three-of-eight
    /// gap was a field-by-field omission: the struct listed what someone remembered to
    /// list. A caller now passes `App::structural_view_available` once and every arm is
    /// filled from it.
    pub(crate) fn from_predicate(mut offered: impl FnMut(StructuralView) -> bool) -> Self {
        Self {
            summary: offered(StructuralView::Summary),
            animate: offered(StructuralView::Animate),
            spy_plot: offered(StructuralView::SpyPlot),
            incidence: offered(StructuralView::Incidence),
            matching_anim: offered(StructuralView::MatchingAnim),
            tarjan_anim: offered(StructuralView::TarjanAnim),
            tearing_anim: offered(StructuralView::TearingAnim),
            tree: offered(StructuralView::Tree),
        }
    }

    /// Whether this tab is offered, by view.
    ///
    /// The inverse of [`Self::from_predicate`], so a test can compare the set the row
    /// drew against the set it was handed without naming fields.
    ///
    /// `#[cfg(test)]` because production has no reader: the row consults the fields
    /// directly, and a second production path to the same answer is what this whole
    /// change exists to remove.
    #[cfg(test)]
    pub(crate) fn offers(&self, v: StructuralView) -> bool {
        match v {
            StructuralView::Summary => self.summary,
            StructuralView::Animate => self.animate,
            StructuralView::SpyPlot => self.spy_plot,
            StructuralView::Incidence => self.incidence,
            StructuralView::MatchingAnim => self.matching_anim,
            StructuralView::TarjanAnim => self.tarjan_anim,
            StructuralView::TearingAnim => self.tearing_anim,
            StructuralView::Tree => self.tree,
        }
    }
}

/// Rumoca's own word for how reduction ended, from the Index Reduction payload.
///
/// `None` when the stage published nothing — a stage that was never reached. Read rather
/// than inferred from the note, which is prose and has already been wrong about this
/// once; `worker::reduction_outcome_summary` owns the vocabulary.
fn reduction_outcome(stages: &StageBundle) -> Option<&str> {
    stages.index_reduction.value.as_ref()?["outcome"]["kind"].as_str()
}

/// How many reduction rounds ran, which is what disambiguates `sorted`.
///
/// `sorted` with zero rounds means normalization rebuilt the system and reduction did
/// nothing; `sorted` with rounds means reduction ran. The word alone cannot tell them
/// apart, which is why the banner takes both.
fn reduction_rounds(stages: &StageBundle) -> u64 {
    stages
        .index_reduction
        .value
        .as_ref()
        .and_then(|v| v["rounds"].as_u64())
        .unwrap_or(0)
}

/// The sub-view a reader lands on when the report stage changes.
///
/// Singular Structural and Index Reduction open on **Summary**, because that is where the
/// explanation is.
///
/// # The rule for everything else: carry over what the new stage still offers
///
/// A view the reader chose is kept across the stage change, so switching stages leaves you
/// looking at the same *kind* of thing. That is only sound for the views both report stages
/// have — Incidence, Matching, BLT, Tearing, Tree — so the three that are **Index-Reduction
/// only** are redirected to the spy plot instead: Summary (the reduction report), Animate
/// (the reduction replay) and AliasAnim (the alias replay).
///
/// # AliasAnim was missing from that list until 2026-08-19, and it stranded the pane
///
/// Doug asked whether the asymmetry recorded here was a finding or a bug; checking it made
/// it a bug. On a non-singular model with aliases — `RcCircuit`, `TwoLoops`,
/// `ProportionalLoop`, `MixedLoop` all qualify — choosing **Aliases ▶** on Index Reduction
/// and then clicking the **Structural** tab kept `AliasAnim` selected. Structural never
/// offers that tab (`App::structural_view_available` requires the Index Reduction stage),
/// so the row drew **no highlighted tab at all**, and the panel below rendered the alias
/// view against the *Structural* report, which carries no eliminations: the pane said
/// *"(no alias eliminations in this report)"* about a model that has several.
///
/// **That is absence being filled rather than stated** — a reader standing on Structural
/// would conclude `RcCircuit` has no aliases. `Animate` was in the redirect list and
/// `AliasAnim` was not, for no reason beyond the Aliases tab having been added later.
///
/// Pure, and separated from the row for that reason: it is the one piece of this pane that
/// is a rule rather than a widget — and the separation is what made the missing case
/// visible, having been three branches inside a `&mut self` render body before.
fn default_sub_view_for(
    is_index_reduction: bool,
    is_singular: bool,
    current: StructuralView,
) -> StructuralView {
    if is_index_reduction || is_singular {
        StructuralView::Summary
    } else if matches!(
        current,
        // The Index-Reduction-only views. Keep this list in step with
        // `App::structural_view_available`: a view that stage does not offer must never
        // survive the change, or it survives with no tab to un-select it.
        //
        // **Three until 2026-11-04**, when `AliasAnim` went with the view it selected.
        StructuralView::Summary | StructuralView::Animate
    ) {
        StructuralView::SpyPlot
    } else {
        current
    }
}

/// Draw the banner and the sub-tab row, selecting into `structural`.
///
/// Takes `&mut StructuralView` rather than the whole `Viewport`: this row reads and writes
/// exactly one of its fields, and the signature is what teaches the next reader that.
pub(crate) fn report_sub_view_row_ui(
    ui: &mut egui::Ui,
    stage: StageKind,
    stages: &StageBundle,
    stage_views: &mut StageViewCaches,
    structural: &mut StructuralView,
    available: TabAvailability,
) {
    let is_index_reduction = stage == StageKind::IndexReduction;
    let note = stages.get(stage).note.as_deref().unwrap_or("");
    let is_singular = note.contains("singular");

    // Invalidate caches when switching between Structural
    // and IndexReduction — each has different report data.
    if stage_views.reset_for(stage) {
        *structural = default_sub_view_for(is_index_reduction, is_singular, *structural);
        // `reset_for` already recorded the new key.
    }
    // The pending sub-view is applied by `App::apply_pending_view_and_seek`, which the
    // caller invokes immediately after this row — *after* the default-sub-view logic
    // above, precisely because that logic would otherwise overwrite it: it forces Summary
    // whenever a report stage is entered singular, and a link saying "show me the matching
    // animation" has to win over it.

    // Status banner
    if is_index_reduction {
        // **Read from the payload's own outcome, not from the note, since 2026-11-04.**
        //
        // It asked `note.contains("singular")` and said *"Index-1 — already
        // non-singular; reduction funnel is a no-op"* otherwise. With the re-modelled
        // notes that is **exactly backwards**: a successful reduction's note reads
        // `"sorted after 6 round(s): 9 state(s) → 5"` and contains no "singular", so
        // `Drivetrain` — six rounds, four states demoted — was told it was a no-op, in
        // the most prominent line on the tab. The word "funnel" had also outlived the
        // funnel.
        //
        // The outcome word is Rumoca's own and distinguishes the three cases the note
        // cannot: `borrowed` (nothing to do), `sorted` (reduction ran, or normalization
        // did), and a refusal.
        let (label, colour, gloss) = match reduction_outcome(stages) {
            Some("borrowed") => (
                "Index-1",
                crate::colors::ANIM_PATH_FOUND,
                "\u{2014} already index-1; the system sorted as it stood and nothing was \
                 rebuilt",
            ),
            Some("sorted") if reduction_rounds(stages) == 0 => (
                "Rebuilt",
                crate::colors::ANIM_PATH_FOUND,
                "\u{2014} sorted with no reduction round: derivative-alias normalization \
                 rebuilt the system",
            ),
            Some("sorted") => (
                "Reduced",
                crate::colors::ANIM_PATH_FOUND,
                "\u{2014} the raw DAE was structurally singular; reduction ran until it \
                 sorted",
            ),
            Some("singular") => (
                "Singular",
                crate::colors::ANIM_FAIL,
                "\u{2014} still singular after reduction; no candidate in either lane \
                 could reduce the residue",
            ),
            // A stop this banner does not know, or a stage with no payload. Named rather
            // than folded into one of the above, because guessing here is what produced
            // the claim this replaced.
            other => (
                "Unreported",
                crate::colors::ANIM_FAIL,
                other.map_or(
                    "\u{2014} this stage published no outcome to report",
                    |_| "\u{2014} the stage reports an outcome this banner does not know",
                ),
            ),
        };
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(label).color(colour).strong());
            ui.weak(gloss);
        });
        ui.add_space(2.0);
    } else if is_singular {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Singular")
                    .color(crate::colors::ANIM_FAIL)
                    .strong(),
            );
            ui.weak(
                "\u{2014} structurally singular; no perfect matching exists (see Index Reduction)",
            );
        });
        ui.add_space(2.0);
    }

    // Sub-tab bar
    ui.horizontal(|ui| {
        // Availability was decided by `App::structural_view_available`, the same
        // predicate the link guard uses — a tab that exists and a link that is
        // honoured must not be able to disagree.
        if available.summary {
            ui.selectable_value(structural, StructuralView::Summary, "Summary");
            ui.separator();
        }
        if available.animate {
            ui.selectable_value(structural, StructuralView::Animate, "Reduction \u{25b6}");
        }
        // **The `Aliases` tab was removed 2026-11-04.** Its gate read
        // `reduction.eliminations`, which v0.10.0 never publishes, so the tab had
        // already stopped appearing — and the pass it showed is not a Rumoca phase
        // any more: upstream deleted `rumoca-phase-structural/src/eliminate/`
        // outright. Measured on the corpus, no specimen's equation count shrinks on
        // 0.10.0, where `Drivetrain` once went from 97 equations to 20.
        // Spy-plot, Matching, BLT require a full matching —
        // hide them when the Structural stage is singular.
        if available.spy_plot {
            ui.selectable_value(structural, StructuralView::SpyPlot, "Spy-plot");
        }
        if available.incidence {
            ui.selectable_value(structural, StructuralView::Incidence, "Incidence");
        }
        // Matching is shown *even when singular* — that is the whole
        // point of it. The other three below need a complete matching
        // before they mean anything; this one is a replay of the
        // *search*, and the search failing is the most instructive
        // thing on a singular stage. It was hidden here until
        // 2026-07-29, when writing a lab to answer "what does a rank
        // deficiency of 1 mean?" ran straight into its absence
        // (ideas #44). Nothing else was needed: the trace already
        // emits `MatchingStep::EquationFailed` and the view already
        // paints the failed row red. The feature was built, then
        // gated out of reach.
        // **Gated like every other tab since 2026-11-04.** This one was drawn
        // unconditionally and the two below under a `!is_singular || is_index_reduction`
        // recomputed right here, so `App::structural_view_available` refusing them had no
        // effect on screen. Doug found that by running the app.
        //
        // The triangle means *this view animates*, so a tab carrying one over a view that
        // cannot animate on this stage is a promise the pane does not keep.
        if available.matching_anim {
            ui.selectable_value(
                structural,
                StructuralView::MatchingAnim,
                "Matching \u{25b6}",
            )
            .on_hover_text(if is_singular {
                "Watch the augmenting-path search run out. The equation it \
                     gives up on is the rank deficiency."
            } else {
                "Replay the augmenting-path search that pairs each equation \
                     with one unknown."
            });
        }
        if available.tarjan_anim {
            ui.selectable_value(structural, StructuralView::TarjanAnim, "BLT \u{25b6}");
        }
        // Tearing operates on the coupled blocks BLT finds, so it needs the same full
        // matching those two do.
        if available.tearing_anim {
            ui.selectable_value(structural, StructuralView::TearingAnim, "Tearing \u{25b6}");
        }
        if available.tree {
            ui.selectable_value(structural, StructuralView::Tree, "Tree");
        }
    });
    ui.separator();
}

/// **What the extraction bought.**
///
/// This row had never been tested. As `fn report_sub_view_row_ui(&mut self, ..)` it could
/// only be reached by constructing an `App`, giving it a worker, compiling a specimen and
/// driving it to Structural or Index Reduction with the right singularity — so every rule
/// it holds was unasserted: which banner appears, which tabs a singular system hides, and
/// the default sub-view a stage change lands on. All of that now runs against a
/// hand-built [`StageBundle`] in hundredths of a second.
///
/// The two banner tests are **must-fire** cases in the strict sense: the row's whole job
/// on a singular system is to say so, and a row that silently drew the tabs without the
/// banner would have looked complete.
#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Everything the row reads and writes, so one harness closure can drive it.
    struct Row {
        stages: StageBundle,
        stage: StageKind,
        stage_views: StageViewCaches,
        structural: StructuralView,
        available: TabAvailability,
    }

    /// Written by hand because neither [`StageKind`] nor [`StructuralView`] has a
    /// `Default` — a stage is a position in the pipeline and a sub-view is a camera, and
    /// neither has a neutral value. `Structural` + `SpyPlot` is what a non-singular model
    /// actually opens on.
    impl Default for Row {
        fn default() -> Self {
            Self {
                stages: StageBundle::default(),
                stage: StageKind::Structural,
                stage_views: StageViewCaches::default(),
                structural: StructuralView::SpyPlot,
                available: TabAvailability {
                    summary: false,
                    animate: false,
                    spy_plot: true,
                    incidence: true,
                    matching_anim: true,
                    tarjan_anim: true,
                    tearing_anim: true,
                    tree: true,
                },
            }
        }
    }

    /// Mark the row's stage singular the way Rumoca does — by writing a note.
    ///
    /// The wording varies between specimens (`"singular"` alone on `Drivetrain`, a whole
    /// sentence on `BenchActuator`), and the row tests the same `contains` the app does,
    /// so any of them serves.
    fn singular(mut row: Row) -> Row {
        row.stages.get_mut(row.stage).note = Some("structurally singular system".into());
        row
    }

    /// **Sized like a pane, deliberately.** A small viewport clips the tab row, and a
    /// clipped widget stays in the accessibility tree — so a query finds it while a click
    /// does nothing, which reads as "the row ignored the press". Both earlier extractions
    /// lost time to that.
    fn harness(row: Row) -> Harness<'static, Row> {
        Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui_state(
                |ui, r: &mut Row| {
                    report_sub_view_row_ui(
                        ui,
                        r.stage,
                        &r.stages,
                        &mut r.stage_views,
                        &mut r.structural,
                        r.available,
                    );
                },
                row,
            )
    }

    /// A structurally singular Structural stage **says so**.
    ///
    /// Silence is the failure mode this guards: without the banner the row still draws a
    /// full set of tabs, and a reader would take the missing spy plot for a rendering
    /// glitch rather than for the compiler refusing to match the system.
    #[test]
    fn a_singular_structural_stage_shows_the_singular_banner() {
        let mut h = harness(singular(Row::default()));
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Singular").is_some(),
            "a singular system must be named as one",
        );
        assert!(
            h.query_by_label_contains("no perfect matching exists")
                .is_some(),
            "and the banner must say what singular MEANS — the phrase is the lesson",
        );
    }

    /// A non-singular Index Reduction stage **reports the no-op**.
    ///
    /// The complement of the test above, and the one that matters most for
    /// `BouncingBall`: a funnel that did nothing must say it did nothing, rather than
    /// leaving the reader to infer it from an empty pane.
    #[test]
    fn a_non_singular_index_reduction_stage_shows_the_index_1_banner() {
        // **The banner reads the payload now, not the note.** It asked
        // `note.contains("singular")`, which with the re-modelled notes is true only for
        // reductions that FAILED — so a successful `Drivetrain` was told its reduction
        // was "a no-op" while six rounds had demoted four states.
        let stages = StageBundle {
            index_reduction: crate::worker::Stage::ok(serde_json::json!({
                "rounds": 0,
                "outcome": { "kind": "borrowed", "admitted": true },
            })),
            ..StageBundle::default()
        };
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            stages,
            available: TabAvailability {
                summary: true,
                ..TabAvailability::default()
            },
            ..Row::default()
        });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Index-1").is_some(),
            "an already-index-1 system must be named as one",
        );
        assert!(
            h.query_by_label_contains("sorted as it stood").is_some(),
            "and the row must say nothing was rebuilt; `funnel` went with the funnel",
        );
        assert!(
            h.query_by_label_contains("Singular").is_none(),
            "the two banners are exclusive — showing both would be incoherent",
        );
    }

    /// **A reduction that ran is not reported as a no-op** — the inverse of the test
    /// above, and the claim Doug was actually shown.
    ///
    /// `Drivetrain` reduces for six rounds and demotes four states. The banner read
    /// `note.contains("singular")`, the re-modelled note reads `"sorted after 6 round(s):
    /// 9 state(s) -> 5"`, and so the most prominent line on the tab said the reduction
    /// did nothing. Both directions are pinned, because one test alone is satisfied by a
    /// banner that always says the same thing.
    #[test]
    fn a_reduced_index_reduction_stage_is_not_reported_as_a_no_op() {
        let stages = StageBundle {
            index_reduction: crate::worker::Stage::ok(serde_json::json!({
                "rounds": 6,
                "outcome": { "kind": "sorted", "admitted": true },
            })),
            ..StageBundle::default()
        };
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            stages,
            available: TabAvailability {
                summary: true,
                ..TabAvailability::default()
            },
            ..Row::default()
        });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Reduced").is_some(),
            "a system reduction worked on must be named as reduced",
        );
        assert!(
            h.query_by_label_contains("Index-1").is_none(),
            "and not index-1, which is what it was NOT when it arrived",
        );
        assert!(
            h.query_by_label_contains("no-op").is_none(),
            "nor a no-op: six rounds demoted four states",
        );
    }

    /// **`sorted` with zero rounds is a third thing, and the banner says which.**
    ///
    /// `StabilizedPendulum`: no reduction round ran and the system still changed, because
    /// derivative-alias normalization rebuilt it. Reported as neither index-1 nor
    /// reduced, because it is neither.
    #[test]
    fn a_normalized_index_reduction_stage_says_it_was_rebuilt() {
        let stages = StageBundle {
            index_reduction: crate::worker::Stage::ok(serde_json::json!({
                "rounds": 0,
                "outcome": { "kind": "sorted", "admitted": true },
            })),
            ..StageBundle::default()
        };
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            stages,
            available: TabAvailability {
                summary: true,
                ..TabAvailability::default()
            },
            ..Row::default()
        });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Rebuilt").is_some(),
            "normalization rebuilt it, which is neither index-1 nor reduced",
        );
        assert!(
            h.query_by_label_contains("Index-1").is_none(),
            "calling it index-1 would hide a rebuild behind a word meaning nothing \
             happened",
        );
    }

    /// A **non**-singular stage shows no banner at all.
    ///
    /// Without this the two above are satisfied by a row that always draws both.
    #[test]
    fn a_non_singular_structural_stage_shows_no_banner() {
        let mut h = harness(Row::default());
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Singular").is_none(),
            "nothing is singular here, so nothing may say so",
        );
        assert!(
            h.query_by_label_contains("Index-1").is_none(),
            "and Index-1 is the Index Reduction stage's word, not Structural's",
        );
    }

    /// Singularity **hides the three tabs that need a complete matching**, and keeps
    /// Matching.
    ///
    /// This is the row's oldest rule and the one with a history: Matching was gated out
    /// with the other three until 2026-07-29, when a lab asking "what does a rank
    /// deficiency of 1 mean?" ran into its absence. Watching the augmenting-path search
    /// fail is the *most* instructive thing on a singular stage, so it is exactly the tab
    /// that must survive.
    #[test]
    fn a_singular_stage_hides_blt_and_tearing_but_keeps_matching() {
        // `matching_anim` has to be OFFERED for the row to draw it: since 2026-11-04 the
        // row consults availability for every tab rather than deciding this one itself.
        // The rule under test is unchanged — it moved to the predicate, where
        // `App::structural_view_available_from_stage` makes Matching available whatever
        // the singularity.
        let mut h = harness(singular(Row {
            available: TabAvailability {
                matching_anim: true,
                incidence: true,
                tree: true,
                ..TabAvailability::default()
            },
            ..Row::default()
        }));
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("BLT").is_none(),
            "BLT needs a complete matching",
        );
        assert!(
            h.query_by_label_contains("Tearing").is_none(),
            "and so does tearing, which operates on the blocks BLT finds",
        );
        assert!(
            h.query_by_label_contains("Matching").is_some(),
            "but the matching REPLAY must stay — the search failing is the lesson",
        );
        assert!(
            h.query_by_label_contains("Incidence").is_some(),
            "and the incidence pattern is always available",
        );
    }

    /// A tab click **selects the sub-view**, which is the row's only output.
    #[test]
    fn clicking_a_tab_selects_that_sub_view() {
        let mut h = harness(Row::default());
        h.run_steps(2);

        h.get_all_by_label_contains("Incidence")
            .next()
            .expect("an Incidence tab")
            .click();
        h.run_steps(2);

        assert_eq!(
            h.state().structural,
            StructuralView::Incidence,
            "the row owns the selection, so the click must land in `viewport.structural`",
        );
    }

    /// An unavailable tab **is not drawn**, so it cannot be selected.
    ///
    /// The availability struct is the caller's answer to a predicate the link guard also
    /// consults; a row that ignored it would let a reader reach a view the guard refuses,
    /// which is the disagreement the single-predicate rule exists to prevent.
    #[test]
    fn an_unavailable_tab_is_absent() {
        let mut h = harness(Row {
            available: TabAvailability::default(),
            ..Row::default()
        });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("Spy-plot").is_none(),
            "spy plot was reported unavailable, so it must not be on screen",
        );
        assert!(
            h.query_by_label_contains("Summary").is_none(),
            "nor summary",
        );
        assert!(
            h.query_by_label_contains("Aliases").is_none(),
            "nor the alias replay",
        );
    }

    /// Entering **Index Reduction lands on Summary**, whatever you were looking at.
    ///
    /// Summary is that stage's report; every other view there is a comparison against it.
    #[test]
    fn a_stage_change_into_index_reduction_lands_on_summary() {
        assert_eq!(
            default_sub_view_for(true, false, StructuralView::Incidence),
            StructuralView::Summary,
        );
        assert_eq!(
            default_sub_view_for(true, true, StructuralView::Tree),
            StructuralView::Summary,
        );
    }

    /// A **singular** Structural stage lands on Summary too — the explanation, not the
    /// plot it cannot draw.
    #[test]
    fn a_stage_change_into_a_singular_structural_stage_lands_on_summary() {
        assert_eq!(
            default_sub_view_for(false, true, StructuralView::SpyPlot),
            StructuralView::Summary,
        );
    }

    /// A **non**-singular Structural stage lands on the spy plot — but only from the three
    /// views that do not exist there.
    ///
    /// Coming from Incidence or Tree, the reader keeps looking at the same kind of thing
    /// across the stage change. Summary, Animate and AliasAnim are Index-Reduction-only,
    /// so each is redirected instead.
    #[test]
    fn a_stage_change_into_a_healthy_structural_stage_keeps_a_view_it_still_offers() {
        assert_eq!(
            default_sub_view_for(false, false, StructuralView::Summary),
            StructuralView::SpyPlot,
            "Summary is not offered here, so it must be replaced",
        );
        assert_eq!(
            default_sub_view_for(false, false, StructuralView::Animate),
            StructuralView::SpyPlot,
            "nor is the reduction replay",
        );
        assert_eq!(
            default_sub_view_for(false, false, StructuralView::Incidence),
            StructuralView::Incidence,
            "but Incidence exists on both stages, so it carries over",
        );
        assert_eq!(
            default_sub_view_for(false, false, StructuralView::Tree),
            StructuralView::Tree,
            "and so does the tree",
        );
    }

    /// **No Index-Reduction-only view survives a move to Structural** — the regression
    /// guard for the 2026-08-19 defect.
    ///
    /// They are asserted **together**, so that adding another such view and forgetting
    /// the redirect fails here rather than on screen. That is exactly how `AliasAnim`
    /// came to be missing: `Animate` was in the list, the Aliases tab was added later,
    /// and nothing compared the two.
    ///
    /// **`AliasAnim` itself went on 2026-11-04** with the alias view, so this list is
    /// two again. The history stays because the *reason for the list* is what it
    /// teaches, and that outlives any one member.
    #[test]
    fn no_index_reduction_only_view_survives_a_move_to_structural() {
        for view in [StructuralView::Summary, StructuralView::Animate] {
            assert_eq!(
                default_sub_view_for(false, false, view),
                StructuralView::SpyPlot,
                "{view:?} exists only on Index Reduction, so a non-singular Structural \
                 stage must not keep it selected — it would leave the row with no \
                 highlighted tab and the panel rendering against the wrong report",
            );
        }
    }

    /// And the same thing through the widget, because the pure function is only half the
    /// path: the row must actually *call* it when the stage changes.
    ///
    /// This is the probe that found the defect, kept as a test. It runs the reader's
    /// route — pick an Index-Reduction-only tab, then click the Structural tab — and
    /// asserts on what the tab row shows afterwards, which is where the symptom was
    /// visible: no tab highlighted at all.
    ///
    /// # Re-keyed from `Aliases` to `Reduction` on 2026-11-04
    ///
    /// The Aliases tab went with the alias view, whose pass v0.10.0 does not run. **The
    /// property is unchanged and still live**, so this moved to `Animate` rather than
    /// being deleted: it is now the Index-Reduction-only tab that a stage change could
    /// strand, and this is the only check that runs the route through the real widgets
    /// instead of calling `default_sub_view_for` directly.
    ///
    /// Availability is set on the fixture rather than derived, which is why re-keying
    /// works even though `App::structural_view_available` currently returns `false` for
    /// `Animate` — this row's subject is the *redirect*, not what the app offers.
    #[test]
    fn choosing_an_index_reduction_only_tab_then_moving_to_structural_leaves_a_real_tab_selected() {
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            structural: StructuralView::Summary,
            available: TabAvailability {
                summary: true,
                animate: true,
                spy_plot: true,
                incidence: true,
                // The three animations are not offered on Index Reduction, which is the
                // state this row is in — their frames belong to the raw system.
                matching_anim: false,
                tarjan_anim: false,
                tearing_anim: false,
                tree: true,
            },
            ..Row::default()
        });
        h.run_steps(2);

        h.get_all_by_label_contains("Reduction")
            .next()
            .expect("Index Reduction offers a Reduction tab")
            .click();
        h.run_steps(2);
        assert_eq!(
            h.state().structural,
            StructuralView::Animate,
            "precondition: the reader is watching the reduction replay",
        );

        // The reader clicks the Structural stage tab. `App` swaps the stage and
        // re-answers the availability questions; Structural offers no reduction view.
        h.state_mut().stage = StageKind::Structural;
        h.state_mut().available = TabAvailability {
            summary: false,
            animate: false,
            spy_plot: true,
            incidence: true,
            matching_anim: true,
            tarjan_anim: true,
            tearing_anim: true,
            tree: true,
        };
        h.run_steps(2);

        assert_eq!(
            h.state().structural,
            StructuralView::SpyPlot,
            "the reduction view has no tab here, so keeping it selected would strand the \
             pane",
        );
        assert!(
            h.query_by_label_contains("Reduction").is_none(),
            "and the tab itself must still be absent — the fix is the selection moving, \
             not the tab appearing",
        );
    }

    /// The reset **only fires on a stage change**, so a redraw does not throw the reader
    /// off the tab they chose.
    ///
    /// `StageViewCaches::reset_for` returns whether it reset, and this row is the only
    /// caller that acts on that bool. Ignoring it would send every frame back to the
    /// default sub-view — a tab that cannot be selected because it un-selects itself.
    #[test]
    fn redrawing_the_row_does_not_re_apply_the_default_sub_view() {
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            available: TabAvailability {
                summary: true,
                // The tab this test clicks; the row draws it only when offered now.
                incidence: true,
                ..TabAvailability::default()
            },
            ..Row::default()
        });
        h.run_steps(2);
        assert_eq!(
            h.state().structural,
            StructuralView::Summary,
            "precondition: entering Index Reduction defaulted to Summary",
        );

        h.get_all_by_label_contains("Incidence")
            .next()
            .expect("an Incidence tab")
            .click();
        h.run_steps(3);

        assert_eq!(
            h.state().structural,
            StructuralView::Incidence,
            "the stage did not change, so the default must not be re-applied",
        );
    }

    /// The on-screen label of each tab, so the drawn set can be read back.
    ///
    /// Shared with the row rather than restated: a label changed in one place and not the
    /// other would make this test pass while the reader sees something else. That is why
    /// it maps *view to label* and the assertion below queries by it, instead of listing
    /// strings.
    fn label(v: StructuralView) -> &'static str {
        match v {
            StructuralView::Summary => "Summary",
            // **The triangles are part of the label on purpose.** A bare "Reduction"
            // also matches the status banner, and matching the whole label asserts the
            // second half of Doug's report: the marker appears on exactly the three tabs
            // whose views animate, so a tab cannot promise an animation the pane does
            // not have.
            StructuralView::Animate => "Reduction \u{25b6}",
            StructuralView::SpyPlot => "Spy-plot",
            StructuralView::Incidence => "Incidence",
            StructuralView::MatchingAnim => "Matching \u{25b6}",
            StructuralView::TarjanAnim => "BLT \u{25b6}",
            StructuralView::TearingAnim => "Tearing \u{25b6}",
            StructuralView::Tree => "Tree",
        }
    }

    /// **The tabs drawn are exactly the tabs offered — no more, no fewer.**
    ///
    /// # The defect this closes, and why nothing else could
    ///
    /// `TabAvailability` carried three of the eight tabs. `Incidence` and `Matching` were
    /// drawn unconditionally; `BLT` and `Tearing` under a singularity the row recomputed
    /// from the stage note. So `App::structural_view_available` could refuse a view and
    /// its tab appeared regardless — and the row's own comment promised the opposite.
    ///
    /// **Doug found it by running the app**: Drivetrain → Index Reduction offering
    /// Matching, BLT and Tearing with animation triangles over views that cannot animate
    /// there. A test written the day before had asked the predicate about those views and
    /// agreed with itself, because the row was not reading it. **One fact with two
    /// expressions, for the third time in a day.**
    ///
    /// This compares the two sets through the widgets, so a tab drawn outside the
    /// availability check fails here by name whichever direction it drifts.
    ///
    /// # Why both directions
    ///
    /// A drawn-but-not-offered tab is the defect above: a reader clicks it and the pane
    /// has nothing, or worse, has the wrong system's data. An offered-but-not-drawn tab
    /// is the inverse and just as bad — `hrw://` links and
    /// `App::apply_pending_view_and_seek` honour availability, so a lab could navigate a
    /// reader to a view with no tab to leave by.
    #[test]
    fn the_tabs_drawn_are_exactly_the_tabs_offered() {
        // Every combination that matters: the two report stages, each with the tab set
        // actually produced for a healthy and a singular system. Hand-built rather than
        // taken from a compile, because this is a claim about the ROW and not about any
        // specimen — the compile-fed version is
        // `app::tests::every_offered_sub_view_has_something_to_draw`.
        let cases = [
            (
                "structural, healthy",
                StageKind::Structural,
                TabAvailability {
                    summary: false,
                    animate: false,
                    spy_plot: true,
                    incidence: true,
                    matching_anim: true,
                    tarjan_anim: true,
                    tearing_anim: true,
                    tree: true,
                },
            ),
            (
                "structural, singular",
                StageKind::Structural,
                TabAvailability {
                    summary: true,
                    animate: false,
                    spy_plot: false,
                    incidence: true,
                    matching_anim: true,
                    tarjan_anim: false,
                    tearing_anim: false,
                    tree: true,
                },
            ),
            (
                // The shape Doug was looking at.
                "index reduction, reduced",
                StageKind::IndexReduction,
                TabAvailability {
                    summary: true,
                    animate: true,
                    spy_plot: true,
                    incidence: true,
                    matching_anim: false,
                    tarjan_anim: false,
                    tearing_anim: false,
                    tree: true,
                },
            ),
            (
                // A refused reduction: no reduced matrix to draw either.
                "index reduction, refused",
                StageKind::IndexReduction,
                TabAvailability {
                    summary: true,
                    animate: true,
                    spy_plot: false,
                    incidence: false,
                    matching_anim: false,
                    tarjan_anim: false,
                    tearing_anim: false,
                    tree: true,
                },
            ),
        ];

        let mut drawn_total = 0usize;
        for (name, stage, available) in cases {
            let h = harness(Row {
                stage,
                available,
                // Something always offered, so the selection cannot itself be clamped
                // away and change what the row draws.
                structural: StructuralView::Tree,
                ..Row::default()
            });

            for v in StructuralView::ALL {
                let drawn = h.query_by_label_contains(label(*v)).is_some();
                let offered = available.offers(*v);
                assert_eq!(
                    drawn,
                    offered,
                    "{name}: {} is {} but its tab is {}",
                    crate::stage_view::structural_view_name(*v),
                    if offered { "offered" } else { "not offered" },
                    if drawn { "drawn" } else { "absent" },
                );
                if drawn {
                    drawn_total += 1;
                }
            }
        }

        // Non-vacuity: a harness that drew no tabs at all would satisfy every `false`
        // above and prove nothing.
        assert!(
            drawn_total > 12,
            "only {drawn_total} tabs were drawn across four cases, so the row is not \
             being exercised",
        );
    }
}
