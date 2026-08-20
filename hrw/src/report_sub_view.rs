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

/// Which of the four **conditional** sub-view tabs this model has.
///
/// The other five — Incidence, Matching, Tree, BLT and Tearing — are decided from
/// singularity alone, which this row can read off the stage note.
///
/// Every field is `pub(crate)` because `..Default::default()` in a test needs the whole
/// struct visible, not only the fields being set (the `ContextBarState` lesson).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TabAvailability {
    /// The singular-system explanation, plus Index Reduction's own report.
    pub(crate) summary: bool,
    /// The reduction replay — Index Reduction only, and only when frames were captured.
    pub(crate) animate: bool,
    /// The alias-elimination replay — Index Reduction only, and only when something was
    /// actually eliminated, so a model with no aliases shows no empty tab.
    pub(crate) aliases: bool,
    /// The BLT spy plot, which needs a complete matching to mean anything.
    pub(crate) spy_plot: bool,
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
        // The three Index-Reduction-only views. Keep this list in step with
        // `App::structural_view_available`: a view that stage does not offer must never
        // survive the change, or it survives with no tab to un-select it.
        StructuralView::Summary | StructuralView::Animate | StructuralView::AliasAnim
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
        if is_singular {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Singular")
                        .color(crate::colors::ANIM_FAIL)
                        .strong(),
                );
                ui.weak("\u{2014} raw DAE was structurally singular; index reduction performed");
            });
        } else {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Index-1")
                        .color(crate::colors::ANIM_PATH_FOUND)
                        .strong(),
                );
                ui.weak("\u{2014} already non-singular; reduction funnel is a no-op");
            });
        }
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
        // Alias elimination is reported by this stage only, and
        // only when something was actually eliminated -- a model
        // with no aliases must not show an empty tab.
        if available.aliases {
            ui.selectable_value(structural, StructuralView::AliasAnim, "Aliases \u{25b6}")
                .on_hover_text(
                    "Watch variables be substituted away. Every connection \
                     equation `a = b` lets one of the two be deleted, which is \
                     why the solved system is far smaller than the equation \
                     count suggests.",
                );
        }
        // Spy-plot, Matching, BLT require a full matching —
        // hide them when the Structural stage is singular.
        if available.spy_plot {
            ui.selectable_value(structural, StructuralView::SpyPlot, "Spy-plot");
        }
        ui.selectable_value(structural, StructuralView::Incidence, "Incidence");
        // Matching is shown *even when singular* — that is the whole
        // point of it. The other three below need a complete matching
        // before they mean anything; this one is a replay of the
        // *search*, and the search failing is the most instructive
        // thing on a singular stage. It was hidden here until
        // 2026-07-29, when writing a tour to answer "what does a rank
        // deficiency of 1 mean?" ran straight into its absence
        // (ideas #44). Nothing else was needed: the trace already
        // emits `MatchingStep::EquationFailed` and the view already
        // paints the failed row red. The feature was built, then
        // gated out of reach.
        ui.selectable_value(
            structural,
            StructuralView::MatchingAnim,
            "Matching \u{25b6}",
        )
        .on_hover_text(if is_singular && !is_index_reduction {
            "Watch the augmenting-path search run out. The equation it \
                 gives up on is the rank deficiency."
        } else {
            "Replay the augmenting-path search that pairs each equation \
                 with one unknown."
        });
        if !is_singular || is_index_reduction {
            ui.selectable_value(structural, StructuralView::TarjanAnim, "BLT \u{25b6}");
            // Tearing operates on the coupled blocks BLT finds,
            // so it needs the same full matching those two do.
            ui.selectable_value(structural, StructuralView::TearingAnim, "Tearing \u{25b6}");
        }
        ui.selectable_value(structural, StructuralView::Tree, "Tree");
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
                    aliases: false,
                    spy_plot: true,
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
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
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
            h.query_by_label_contains("no-op").is_some(),
            "and the row must say the funnel did nothing",
        );
        assert!(
            h.query_by_label_contains("Singular").is_none(),
            "the two banners are exclusive — showing both would be incoherent",
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
    /// with the other three until 2026-07-29, when a tour asking "what does a rank
    /// deficiency of 1 mean?" ran into its absence. Watching the augmenting-path search
    /// fail is the *most* instructive thing on a singular stage, so it is exactly the tab
    /// that must survive.
    #[test]
    fn a_singular_stage_hides_blt_and_tearing_but_keeps_matching() {
        let mut h = harness(singular(Row {
            available: TabAvailability::default(),
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

    /// **The alias replay must not survive a move to Structural** — the regression guard
    /// for the 2026-08-19 defect.
    ///
    /// The three Index-Reduction-only views are asserted **together**, so that adding a
    /// fourth such view and forgetting the redirect fails here rather than on screen. That
    /// is exactly how `AliasAnim` came to be missing: `Animate` was in the list, the
    /// Aliases tab was added later, and nothing compared the two.
    #[test]
    fn no_index_reduction_only_view_survives_a_move_to_structural() {
        for view in [
            StructuralView::Summary,
            StructuralView::Animate,
            StructuralView::AliasAnim,
        ] {
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
    /// This is the probe that found the defect, kept as a test. It walks the reader's
    /// route — pick Aliases on Index Reduction, then click the Structural tab — and
    /// asserts on what the tab row shows afterwards, which is where the symptom was
    /// visible: no tab highlighted at all.
    #[test]
    fn choosing_aliases_then_moving_to_structural_leaves_a_real_tab_selected() {
        let mut h = harness(Row {
            stage: StageKind::IndexReduction,
            structural: StructuralView::Summary,
            available: TabAvailability {
                summary: true,
                animate: true,
                aliases: true,
                spy_plot: true,
            },
            ..Row::default()
        });
        h.run_steps(2);

        h.get_all_by_label_contains("Aliases")
            .next()
            .expect("Index Reduction offers an Aliases tab")
            .click();
        h.run_steps(2);
        assert_eq!(
            h.state().structural,
            StructuralView::AliasAnim,
            "precondition: the reader is watching the alias replay",
        );

        // The reader clicks the Structural stage tab. `App` swaps the stage and
        // re-answers the availability questions; Structural offers no alias view.
        h.state_mut().stage = StageKind::Structural;
        h.state_mut().available = TabAvailability {
            summary: false,
            animate: false,
            aliases: false,
            spy_plot: true,
        };
        h.run_steps(2);

        assert_eq!(
            h.state().structural,
            StructuralView::SpyPlot,
            "the alias view has no tab here, so keeping it selected would strand the pane",
        );
        assert!(
            h.query_by_label_contains("Aliases").is_none(),
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
}
