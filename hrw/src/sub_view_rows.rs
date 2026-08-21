//! **The three stage-owned sub-view rows** — Flatten, Events, Initialization.
//!
//! Lifted out of `central_panel_ui` on 2026-08-21, the third cut into that router. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # What this is a list of, and which member is shaped differently
//!
//! Four stages offer a sub-view row. Three of them are here; the fourth — the report
//! stages — is [`crate::report_sub_view`], which left on 2026-08-19 because it carries a
//! banner, a stage-change default and a nine-way selector. What is left is the plain
//! shape: a gate, a `ui.horizontal` of `selectable_value`, a separator.
//!
//! **The odd member of the three is Flatten, and the asymmetry was a defect.** Events and
//! Initialization each show *both* their tabs whenever the row shows at all, so their
//! selection can never name a tab that is not on screen. Flatten has **two conditional
//! tabs** — Source Map exists only when the sheet carries source spans, Connections only
//! when the model has `connect()` statements — and until 2026-08-21 nothing clamped
//! `FlattenView` when they vanished. See [`flatten_row_ui`] for the stranding that
//! followed and why the clamp is silent.
//!
//! # One predicate per stage, three consumers — the report stages' shape
//!
//! Doug, 2026-08-21, ruling on the asymmetry: *"Accuracy is a requirement. And,
//! consistency reduces my learning friction. So … we should make changes to ensure
//! accuracy and we should make changes to improve consistency."*
//!
//! So each stage here has an availability predicate — [`flatten_view_available`],
//! [`events_view_available`], [`init_view_available`] — and it is consulted by **all
//! three** doors, exactly as `App::structural_view_available` is:
//!
//! | consumer | what it decides |
//! |---|---|
//! | the row below | whether a tab is drawn |
//! | `App::apply_pending_view_and_seek` | whether an `hrw://` link is honoured |
//! | [`flatten_row_ui`]'s clamp | whether the surviving selection is drawable |
//!
//! **The link guard was a `_ => true` wildcard until this ruling**, so a tour link naming
//! `Flatten/SourceMap` for a model without one was accepted, selected a tab that is not
//! drawn, and landed the reader on the tree — the *exact* defect Doug reported on
//! `Structural/Summary` on 2026-08-12, in the one arm the fix for it did not reach.
//! `app::tests::every_tour_sub_view_link_is_available_for_its_specimen` carried the same
//! belief in a comment: *"Flatten/Events/Initialization sub-views are always present."*
//!
//! **`Tree` is available unconditionally in all three**, matching
//! `structural_view_available`'s rule: it is what the stage falls back to when no row is
//! drawn, so a link naming it is never refused.
//!
//! # Each row returns its own gate, and the four gates are mutually exclusive
//!
//! The `bool` each function returns is the same value the caller used to name
//! `flatten_ready` / `events_ready` / `init_ready`, and the pane dispatch below the rows
//! re-tests it on every arm. **At most one can be true in a frame**, because each starts
//! with `stage == <its own stage>` and the app shows one stage — which is why the caller
//! draws all three unconditionally and they cannot fight over the pane.
//!
//! Returning the gate rather than recomputing it in the caller is what keeps the tab and
//! the pane from disagreeing: the row that decided *"there is nothing to offer here"* is
//! the same answer the dispatch uses to decide *"draw the tree instead"*. That is
//! [`crate::report_sub_view`]'s single-predicate rule, arrived at from the other side.
//!
//! # The default sub-view is the leftmost tab
//!
//! Flatten reads `Equations | Source Map | Connections ▶ | Tree` and the other two read
//! `Tree | …`, which looks like two different conventions until you check
//! [`Viewport::default`](crate::stage_view::Viewport): Flatten opens on `Equations`,
//! Events and Initialization open on `Tree`. **The rule is "the default is leftmost", and
//! Tree's position is a consequence of it** — generic where it is the fallback, first
//! where it is the landing place. Asserted by
//! `the_default_sub_view_is_the_leftmost_tab_of_its_row`, because the rule is invisible in
//! any one row and only a reader who compares all three can see it at all.

use eframe::egui;

use crate::stage_view::{EventsView, FlattenView, InitView};
use crate::worker::StageKind;

/// What the Flatten compile produced, and therefore which of its tabs exist.
///
/// The caller answers these rather than the row reading `App`, the same trade
/// [`crate::report_sub_view::TabAvailability`] makes: the predicates read
/// `cached_equation_sheet` and `frames.connection`, two fields this row never otherwise
/// touches.
///
/// Every field is `pub(crate)` so a test can build one field-by-field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FlattenContent {
    /// A flattened equation sheet was built. **This is the row's gate** — with no sheet
    /// there is nothing to put beside the tree, so no row is drawn at all.
    pub(crate) equation_sheet: bool,
    /// The sheet carries source spans, so equations can be shown beside the text they
    /// came from.
    pub(crate) source_map: bool,
    /// The model has `connect()` statements to expand. A hand-written model has none, and
    /// shows no empty tab.
    pub(crate) connections: bool,
}

/// **Whether a Flatten sub-view will show what it names** — one predicate, consulted by
/// the tab row below, by the `hrw://` link guard in `App::apply_pending_view_and_seek`,
/// and by the row's own clamp.
///
/// The same contract as [`App::structural_view_available`](crate::app), down to the
/// treatment of `Tree`: **the tree is what the stage falls back to, row or no row**, so it
/// is available unconditionally and a link naming it is never refused. The other three
/// name panes that only exist for some models, and selecting one the compile did not
/// produce shows the tree while claiming otherwise.
///
/// Pure, and takes [`FlattenContent`] rather than `&App`, so a checker can reach it
/// without a compile — the property that lets
/// `App::structural_view_available_from_stage` be called from the tour-link test.
pub(crate) fn flatten_view_available(v: FlattenView, have: FlattenContent) -> bool {
    match v {
        // Falls back to the generic tree, which every stage has.
        FlattenView::Tree => true,
        FlattenView::Equations => have.equation_sheet,
        FlattenView::SourceMap => have.equation_sheet && have.source_map,
        FlattenView::Connections => have.equation_sheet && have.connections,
    }
}

/// Whether an Events sub-view will show what it names. See [`flatten_view_available`].
pub(crate) fn events_view_available(v: EventsView, have_pre_lowering: bool) -> bool {
    match v {
        EventsView::Tree => true,
        EventsView::PreLowering => have_pre_lowering,
    }
}

/// Whether an Initialization sub-view will show what it names. See
/// [`flatten_view_available`].
pub(crate) fn init_view_available(v: InitView, have_ic_plan: bool) -> bool {
    match v {
        InitView::Tree => true,
        InitView::IcPlan => have_ic_plan,
    }
}

/// The Flatten stage's sub-view row: the equation sheet, the source map, the connection
/// replay, the tree.
///
/// Returns `flatten_ready` — whether this stage has a sheet to offer, which is also what
/// the caller's pane dispatch gates on.
///
/// # Why this row clamps and the other two do not
///
/// **Flatten is the only one of the three with conditional tabs.** Events and
/// Initialization gate the whole row, so once it is drawn both of its tabs are drawn and
/// the selection can never name a missing one. Flatten's Source Map and Connections come
/// and go with the model, and `FlattenView` lives in the
/// [`Viewport`](crate::stage_view::Viewport) — a camera, which `clear_specimen_state`
/// deliberately does not reset.
///
/// So before 2026-08-21: choose **Source Map** on a specimen that has one, open a specimen
/// that does not, and the row drew `Equations | Tree` with **nothing highlighted**, over a
/// pane reading `(no source mapping available)`. **The same shape as
/// `report_sub_view`'s `AliasAnim` defect**, which is why the report stages have both a
/// stage-change default and `App::clamp_structural_sub_view`.
///
/// **The clamp here is the analogue of `default_sub_view_for`, not of the backstop, and
/// that is why it is silent.** The report stages' clamp notifies because after its 2026-08-19
/// fix nothing should reach it; this one runs on the ordinary path — a reader switching
/// specimens — where a notice would report the app working. `Equations` is the landing
/// view for the same reason `Viewport::default()` picks it.
///
/// The clamp runs **before** the caller's `apply_pending_view_and_seek`, so an `hrw://`
/// link still wins over it — the ordering `report_sub_view` documents for the same reason.
pub(crate) fn flatten_row_ui(
    ui: &mut egui::Ui,
    stage: StageKind,
    have: FlattenContent,
    view: &mut FlattenView,
) -> bool {
    // The Flatten stage offers an equation sheet alongside the tree.
    if stage != StageKind::Flatten || !have.equation_sheet {
        return false;
    }
    // **The result check, not a fourth guard.** Both doors that write this field consult
    // `flatten_view_available`; this asks whether what they left is drawable, which is
    // what the report stages learned to do the hard way.
    if !flatten_view_available(*view, have) {
        *view = FlattenView::Equations;
    }
    ui.horizontal(|ui| {
        // **Every tab is drawn iff the predicate approves it**, so a tab that exists and a
        // link that is honoured cannot disagree. The conditions used to be written out
        // here — `have.source_map`, `!frames.connection.is_empty()` — which is how this
        // row became the only place in the app that knew they were conditional.
        for (v, label, hover) in [
            (FlattenView::Equations, "Equations", None),
            (FlattenView::SourceMap, "Source Map", None),
            (
                FlattenView::Connections,
                "Connections \u{25b6}",
                Some(
                    "Watch connect() statements become equations. A potential set \
                     of n variables yields n-1 equalities; a flow set of the same \
                     n yields one sum-to-zero equation (Kirchhoff).",
                ),
            ),
            (FlattenView::Tree, "Tree", None),
        ] {
            if !flatten_view_available(v, have) {
                continue;
            }
            let r = ui.selectable_value(view, v, label);
            if let Some(hover) = hover {
                r.on_hover_text(hover);
            }
        }
    });
    ui.separator();
    true
}

/// The Events stage's sub-view row: the tree, or the `pre()`-lowering replay.
///
/// Returns `events_ready`. `have_pre_lowering` is whether the compile captured a trace to
/// replay — a smooth model has none, and shows no row rather than an empty tab.
pub(crate) fn events_row_ui(
    ui: &mut egui::Ui,
    stage: StageKind,
    have_pre_lowering: bool,
    view: &mut EventsView,
) -> bool {
    // The Events stage offers a replay of `pre()` lowering beside the
    // tree — only when there is a trace to replay, so smooth models
    // never show an empty tab.
    if stage != StageKind::Events || !have_pre_lowering {
        return false;
    }
    ui.horizontal(|ui| {
        ui.selectable_value(view, EventsView::Tree, "Tree");
        ui.selectable_value(view, EventsView::PreLowering, "pre() lowering \u{25b6}")
            .on_hover_text(
                "Replay where the __pre__ parameter slots are manufactured. They \
                 appear in no source file: a `when` equation needs a value to hold \
                 when no branch fires, and a DAE cannot say \u{201c}unchanged\u{201d}.",
            );
    });
    ui.separator();
    true
}

/// The Initialization stage's sub-view row: the tree, or a walk of the IC solve plan.
///
/// Returns `init_ready`. `have_ic_plan` is whether the stage produced a plan — a model
/// whose initialization failed has none, and shows no row rather than an empty tab.
pub(crate) fn init_row_ui(
    ui: &mut egui::Ui,
    stage: StageKind,
    have_ic_plan: bool,
    view: &mut InitView,
) -> bool {
    // The Initialization stage offers a walk of the initial-condition
    // solve plan beside the tree -- only when there is a plan, so a
    // model whose initialization failed never shows an empty tab.
    if stage != StageKind::Initialization || !have_ic_plan {
        return false;
    }
    ui.horizontal(|ui| {
        ui.selectable_value(view, InitView::Tree, "Tree");
        ui.selectable_value(view, InitView::IcPlan, "IC plan \u{25b6}")
            .on_hover_text(
                "Walk the plan for computing a consistent state at t=0. Mostly \
                 plain assignment; the few blocks that iterate are where \
                 initialization fails when it fails.",
            );
    });
    ui.separator();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stage_view::Viewport;
    use eframe::egui::accesskit::Toggled;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};

    /// The tab labels of each row, **left to right as drawn**.
    ///
    /// Written out rather than derived: they are the strings on screen, and the
    /// leftmost-is-default rule below is a claim about this order.
    const FLATTEN_TABS: &[&str] = &["Equations", "Source Map", "Connections \u{25b6}", "Tree"];
    const EVENTS_TABS: &[&str] = &["Tree", "pre() lowering \u{25b6}"];
    const INIT_TABS: &[&str] = &["Tree", "IC plan \u{25b6}"];

    /// What the three gates said this frame.
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    struct Gates {
        flatten: bool,
        events: bool,
        init: bool,
    }

    /// Everything the three rows are handed, plus what they handed back.
    ///
    /// One fixture for all three because **the caller draws all three every frame**,
    /// and a harness that called one in isolation would not be reproducing the caller.
    struct Rows {
        stage: StageKind,
        have: FlattenContent,
        pre_lowering: bool,
        ic_plan: bool,
        flatten: FlattenView,
        events: EventsView,
        init: InitView,
        gates: Gates,
    }

    impl Rows {
        /// Everything the compile could possibly have produced, on the given stage.
        ///
        /// **The sub-view fields start at [`Viewport`]'s defaults**, not at hand-picked
        /// variants, because the leftmost rule below is a claim about exactly those
        /// values — a fixture that chose its own would assert nothing about the app.
        fn on(stage: StageKind) -> Self {
            let v = Viewport::default();
            Rows {
                stage,
                have: FlattenContent {
                    equation_sheet: true,
                    source_map: true,
                    connections: true,
                },
                pre_lowering: true,
                ic_plan: true,
                flatten: v.flatten,
                events: v.events,
                init: v.init,
                gates: Gates::default(),
            }
        }
    }

    /// Draw all three rows once, the way `central_panel_ui` does, and hand back the
    /// harness.
    ///
    /// **Sized like a pane** (`1200×900`): a clipped widget stays in the accessibility
    /// tree while behaving as though it is not there, so a query finds it and a click
    /// does nothing. `stage_tabs`, `equation_sheet_view` and `nav_view` each lost time to
    /// that before it was written down.
    fn draw(rows: Rows) -> Harness<'static, Rows> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui_state(
                |ui, r: &mut Rows| {
                    r.gates = Gates {
                        flatten: flatten_row_ui(ui, r.stage, r.have, &mut r.flatten),
                        events: events_row_ui(ui, r.stage, r.pre_lowering, &mut r.events),
                        init: init_row_ui(ui, r.stage, r.ic_plan, &mut r.init),
                    };
                },
                rows,
            );
        h.run_steps(2);
        h
    }

    /// Each row belongs to one stage, and the other two draw nothing at all.
    ///
    /// **This is what lets the caller draw all three unconditionally.** The gates are
    /// mutually exclusive because each opens with `stage == <its own stage>`, and if that
    /// ever stopped being true two rows would stack up and two panes would compete for
    /// the same `else if` chain below. Nothing else states it.
    #[test]
    fn each_row_appears_only_on_its_own_stage() {
        for (stage, expected, tabs) in [
            (
                StageKind::Flatten,
                Gates {
                    flatten: true,
                    ..Gates::default()
                },
                FLATTEN_TABS,
            ),
            (
                StageKind::Events,
                Gates {
                    events: true,
                    ..Gates::default()
                },
                EVENTS_TABS,
            ),
            (
                StageKind::Initialization,
                Gates {
                    init: true,
                    ..Gates::default()
                },
                INIT_TABS,
            ),
        ] {
            let h = draw(Rows::on(stage));
            assert_eq!(
                h.state().gates,
                expected,
                "{stage:?}: exactly one gate may be true, or two rows draw at once",
            );
            for tab in tabs {
                assert!(
                    h.query_by_label(tab).is_some(),
                    "{stage:?}: its own row must offer {tab:?}",
                );
            }
        }
    }

    /// A stage with no sub-views draws no row, however much the compile produced.
    ///
    /// `Dae` stands for the eight tree-only stages: they have no sub-tabs at all, and a
    /// row appearing on one would be a claim that they do.
    #[test]
    fn a_tree_only_stage_draws_no_row() {
        let h = draw(Rows::on(StageKind::Dae));
        assert_eq!(
            h.state().gates,
            Gates::default(),
            "a tree-only stage offers nothing, so every gate is closed",
        );
        for tab in FLATTEN_TABS.iter().chain(EVENTS_TABS).chain(INIT_TABS) {
            assert!(
                h.query_by_label(tab).is_none(),
                "a tree-only stage must not draw {tab:?}",
            );
        }
    }

    /// No equation sheet, no Flatten row — not an empty one.
    ///
    /// The gate is the sheet rather than the stage, because the tree is on screen either
    /// way and a lone "Tree" tab would say a choice exists where none does.
    #[test]
    fn the_flatten_row_needs_an_equation_sheet() {
        let mut rows = Rows::on(StageKind::Flatten);
        rows.have.equation_sheet = false;
        let h = draw(rows);

        assert!(!h.state().gates.flatten, "no sheet, no row");
        assert!(
            h.query_by_label("Equations").is_none(),
            "the row must not be drawn at all",
        );
    }

    /// The Source Map tab exists only when the sheet carries source spans.
    #[test]
    fn the_source_map_tab_needs_source_spans() {
        let mut rows = Rows::on(StageKind::Flatten);
        rows.have.source_map = false;
        let h = draw(rows);

        assert!(h.state().gates.flatten, "the sheet is still there");
        assert!(
            h.query_by_label("Source Map").is_none(),
            "a sheet with no spans has no source map to show",
        );
        assert!(
            h.query_by_label("Equations").is_some(),
            "the rest of the row is unaffected",
        );
    }

    /// The Connections tab exists only for a model with `connect()` statements.
    ///
    /// A hand-written model has none, and an empty replay tab would invite the reader to
    /// conclude the expansion produced nothing.
    #[test]
    fn the_connections_tab_needs_connect_statements() {
        let mut rows = Rows::on(StageKind::Flatten);
        rows.have.connections = false;
        let h = draw(rows);

        assert!(
            h.query_by_label("Connections \u{25b6}").is_none(),
            "no connect() statements, no expansion tab",
        );
        assert!(
            h.query_by_label("Tree").is_some(),
            "the rest of the row is unaffected",
        );
    }

    /// The Events row needs a captured `pre()`-lowering trace; the Initialization row
    /// needs a solve plan.
    ///
    /// Both are whole-row gates rather than tab gates, which is what makes those two rows
    /// unable to strand a selection — see the module docs.
    #[test]
    fn the_events_and_init_rows_each_need_their_content() {
        let mut rows = Rows::on(StageKind::Events);
        rows.pre_lowering = false;
        let h = draw(rows);
        assert!(!h.state().gates.events, "a smooth model replays nothing");
        assert!(
            h.query_by_label("pre() lowering \u{25b6}").is_none(),
            "the Events row must not be drawn at all",
        );

        let mut rows = Rows::on(StageKind::Initialization);
        rows.ic_plan = false;
        let h = draw(rows);
        assert!(!h.state().gates.init, "a failed initialization has no plan");
        assert!(
            h.query_by_label("IC plan \u{25b6}").is_none(),
            "the Initialization row must not be drawn at all",
        );
    }

    /// A click lands in the caller's view field, which is the row's only output besides
    /// its gate.
    #[test]
    fn a_tab_click_lands_in_the_view() {
        let mut h = draw(Rows::on(StageKind::Flatten));
        h.get_by_label("Source Map").click();
        h.run_steps(2);
        assert_eq!(
            h.state().flatten,
            FlattenView::SourceMap,
            "the row owns the selection, so the click must land in `viewport.flatten`",
        );

        let mut h = draw(Rows::on(StageKind::Events));
        h.get_by_label("pre() lowering \u{25b6}").click();
        h.run_steps(2);
        assert_eq!(h.state().events, EventsView::PreLowering);

        let mut h = draw(Rows::on(StageKind::Initialization));
        h.get_by_label("IC plan \u{25b6}").click();
        h.run_steps(2);
        assert_eq!(h.state().init, InitView::IcPlan);
    }

    /// **The default sub-view is the leftmost tab of its row**, in all three rows.
    ///
    /// Flatten ends with Tree and the other two begin with it, which reads as two
    /// conventions until this rule is applied: Flatten opens on `Equations`, Events and
    /// Initialization open on `Tree`, and each of those is drawn first. **The rule is
    /// invisible in any single row** — only a reader who compares all three can see it,
    /// which is exactly the kind of claim that belongs in a test rather than a comment.
    ///
    /// Asserted through the accessibility tree's `toggled` flag and the widget rects, so
    /// it fails if either the order or the default changes.
    #[test]
    fn the_default_sub_view_is_the_leftmost_tab_of_its_row() {
        for (stage, tabs) in [
            (StageKind::Flatten, FLATTEN_TABS),
            (StageKind::Events, EVENTS_TABS),
            (StageKind::Initialization, INIT_TABS),
        ] {
            let h = draw(Rows::on(stage));
            let mut selected: Vec<&str> = Vec::new();
            let mut leftmost: Option<(&str, f32)> = None;
            for tab in tabs {
                let node = h
                    .query_by_label(tab)
                    .unwrap_or_else(|| panic!("{stage:?}: {tab:?} is not on screen"));
                if node.accesskit_node().toggled() == Some(Toggled::True) {
                    selected.push(tab);
                }
                let x = node.rect().min.x;
                if leftmost.is_none_or(|(_, best)| x < best) {
                    leftmost = Some((tab, x));
                }
            }
            assert_eq!(
                selected.len(),
                1,
                "{stage:?}: exactly one tab is selected at the default, got {selected:?}",
            );
            assert_eq!(
                selected[0],
                leftmost.expect("the row has tabs").0,
                "{stage:?}: the default sub-view must be the leftmost tab",
            );
        }
    }

    /// **A Flatten view the model no longer offers is clamped back to the landing tab**,
    /// so the row is never drawn with nothing selected.
    ///
    /// Both conditional tabs, because they are two independent conditions and a clamp that
    /// covered one would look identical from outside.
    ///
    /// # The path is ordinary use, not a broken link
    ///
    /// `FlattenView` is viewport state — a camera, which `clear_specimen_state`
    /// deliberately does not reset — so choosing **Source Map** on a specimen that has one
    /// and then opening a specimen that does not carries the selection across. Until
    /// 2026-08-21 the row drew `Equations | Tree` with nothing highlighted, over a pane
    /// reading `(no source mapping available)`. Doug ruled on it the day it was found:
    /// *"Accuracy is a requirement. And, consistency reduces my learning friction."*
    #[test]
    fn a_flatten_view_the_model_no_longer_offers_is_clamped_to_the_landing_tab() {
        for (missing, stranded) in [
            (FlattenView::SourceMap, "Source Map"),
            (FlattenView::Connections, "Connections \u{25b6}"),
        ] {
            let mut rows = Rows::on(StageKind::Flatten);
            match missing {
                FlattenView::SourceMap => rows.have.source_map = false,
                _ => rows.have.connections = false,
            }
            rows.flatten = missing;
            let h = draw(rows);

            assert!(
                h.query_by_label(stranded).is_none(),
                "precondition: {stranded:?} has no tab for this model",
            );
            assert_eq!(
                h.state().flatten,
                FlattenView::Equations,
                "a selection with no tab must fall back to the landing view, not survive",
            );
            let node = h.get_by_label("Equations");
            assert_eq!(
                node.accesskit_node().toggled(),
                Some(Toggled::True),
                "and the fallback must be highlighted \u{2014} the row is never drawn blank",
            );
        }
    }

    /// **A tab is drawn exactly when its predicate approves it**, across every combination
    /// of what a Flatten compile can produce.
    ///
    /// This is the property the three doors share: the row draws what
    /// [`flatten_view_available`] approves, `App::apply_pending_view_and_seek` honours a
    /// link the same predicate approves, and the clamp above rejects what it does not. **A
    /// row that answered the question its own way is how the link guard came to disagree
    /// with the tabs for nine days.**
    ///
    /// `Tree` is available unconditionally and is therefore drawn in every row here; that
    /// is deliberate and matches `structural_view_available`.
    #[test]
    fn a_flatten_tab_is_drawn_exactly_when_the_predicate_approves_it() {
        for source_map in [false, true] {
            for connections in [false, true] {
                let have = FlattenContent {
                    equation_sheet: true,
                    source_map,
                    connections,
                };
                let mut rows = Rows::on(StageKind::Flatten);
                rows.have = have;
                let h = draw(rows);

                for (v, label) in [
                    (FlattenView::Equations, "Equations"),
                    (FlattenView::SourceMap, "Source Map"),
                    (FlattenView::Connections, "Connections \u{25b6}"),
                    (FlattenView::Tree, "Tree"),
                ] {
                    assert_eq!(
                        h.query_by_label(label).is_some(),
                        flatten_view_available(v, have),
                        "{have:?}: {label:?} on screen disagrees with the predicate",
                    );
                }
            }
        }
    }

    /// **Events and Initialization need no clamp, and this is why** — both tabs are drawn
    /// whenever the row is, so no selection can name a missing one.
    ///
    /// Stated as a test rather than a comment because it is the *reason* those two rows
    /// differ from Flatten. If either ever grows a conditional tab this fails, which is
    /// the moment it would need a clamp of its own.
    #[test]
    fn the_events_and_init_rows_have_no_conditional_tabs() {
        for v in EventsView::ALL {
            assert!(
                events_view_available(*v, true),
                "{v:?} must be offered whenever the Events row is drawn",
            );
        }
        for v in InitView::ALL {
            assert!(
                init_view_available(*v, true),
                "{v:?} must be offered whenever the Initialization row is drawn",
            );
        }
        let h = draw(Rows::on(StageKind::Events));
        for tab in EVENTS_TABS {
            assert!(h.query_by_label(tab).is_some(), "{tab:?} must be drawn");
        }
        let h = draw(Rows::on(StageKind::Initialization));
        for tab in INIT_TABS {
            assert!(h.query_by_label(tab).is_some(), "{tab:?} must be drawn");
        }
    }

    /// **The tree is available on all three stages whatever the compile produced**, so a
    /// link naming it is never refused.
    ///
    /// The same rule `App::structural_view_available_from_stage` states for
    /// `StructuralView::Tree`, and it is what makes "available" mean *"selecting this shows
    /// what it names"* rather than *"a tab is drawn"*: with no row at all, the stage falls
    /// back to the generic tree, which is exactly what `Tree` names.
    #[test]
    fn the_tree_is_available_whatever_the_compile_produced() {
        assert!(flatten_view_available(
            FlattenView::Tree,
            FlattenContent::default()
        ));
        assert!(events_view_available(EventsView::Tree, false));
        assert!(init_view_available(InitView::Tree, false));

        assert!(!flatten_view_available(
            FlattenView::Equations,
            FlattenContent::default()
        ));
        assert!(!events_view_available(EventsView::PreLowering, false));
        assert!(!init_view_available(InitView::IcPlan, false));
    }
}
