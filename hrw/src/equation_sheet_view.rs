//! The **equation sheet** pane — the flattened model's equations and its
//! variable classification, rendered from [`crate::equation_sheet::EquationSheet`].
//!
//! Split out of `app.rs` (see `docs/app-split-plan.md`). The pane renders and
//! *reports*; it owns no policy. Both of its clickable surfaces — an equation
//! row and a variable name — come back to `App` as a single [`SheetClick`],
//! because egui delivers a press to one widget per frame, so two could never
//! be reported together.

use eframe::egui;

use crate::equation_sheet::EquationSheet;

/// What the reader clicked, if anything.
///
/// One value rather than two accumulators: an equation row and a variable name
/// are distinct widgets, and a frame carries at most one press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SheetClick {
    /// An equation row. The payload is the *new* highlight — `None` when the
    /// already-highlighted row was clicked again, which un-highlights it.
    Equation(Option<usize>),
    /// A variable name in the classification grid.
    Variable(String),
}

/// Draw the equation sheet.
///
/// `has_incidence` decides whether equation rows are clickable at all: without
/// a Structural stage there is nothing to highlight them *in*. It is computed
/// by the caller, which owns the two stage groups it reads.
pub(crate) fn equation_sheet_ui(
    ui: &mut egui::Ui,
    sheet: Option<&EquationSheet>,
    has_incidence: bool,
    tracked: Option<&str>,
    highlighted_eq_row: Option<usize>,
) -> Option<SheetClick> {
    let Some(sheet) = sheet else {
        ui.weak("(no equation sheet)");
        return None;
    };

    let mut click = None;

    egui::ScrollArea::both()
        .id_salt("equation_sheet")
        .auto_shrink(false)
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} continuous equations   |   {} states, {} algebraics, {} parameters",
                    sheet.n_equations, sheet.n_states, sheet.n_algebraics, sheet.n_parameters,
                ))
                .strong(),
            );
            if sheet.n_constants > 0
                || sheet.n_discrete > 0
                || sheet.n_inputs > 0
                || sheet.n_outputs > 0
            {
                let mut extras = Vec::new();
                if sheet.n_constants > 0 {
                    extras.push(format!("{} constants", sheet.n_constants));
                }
                if sheet.n_discrete > 0 {
                    extras.push(format!("{} discrete", sheet.n_discrete));
                }
                if sheet.n_inputs > 0 {
                    extras.push(format!("{} inputs", sheet.n_inputs));
                }
                if sheet.n_outputs > 0 {
                    extras.push(format!("{} outputs", sheet.n_outputs));
                }
                ui.weak(extras.join(", "));
            }

            if has_incidence {
                ui.weak("Click an equation to highlight it in the incidence matrix.");
            }

            ui.add_space(8.0);

            // **The family heading, drawn once above a contiguous run.** The three
            // `connect`-derived groups share a cause, and rendering them as flat
            // siblings of `Component equations` implied they did not — Doug,
            // 2026-08-13: *"the flow variables are presented as though they create
            // some other kind of equations which are not connection equations."*
            // `cmp_key` keeps the run contiguous, so tracking the previous family
            // is enough; no grouping pass is needed.
            let mut family_shown: Option<&'static str> = None;
            for (cat, eqs) in &sheet.groups {
                ui.add_space(6.0);
                if let Some(family) = cat.family()
                    && family_shown != Some(family)
                {
                    let total: usize = sheet
                        .groups
                        .iter()
                        .filter(|(c, _)| c.family() == Some(family))
                        .map(|(_, e)| e.len())
                        .sum();
                    ui.label(
                        egui::RichText::new(format!("{family} ({total})"))
                            .strong()
                            .size(15.0),
                    );
                    ui.weak("Every one of these exists because two connectors were joined.");
                    ui.add_space(2.0);
                }
                family_shown = cat.family();
                // Indented under the family heading when there is one, so the
                // nesting is visible rather than merely asserted.
                let indent = if cat.family().is_some() { 16.0 } else { 0.0 };
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.label(
                        egui::RichText::new(format!("{} ({})", cat.label(), eqs.len())).strong(),
                    );
                });
                ui.horizontal(|ui| {
                    ui.add_space(indent);
                    ui.weak(cat.description());
                });
                ui.add_space(2.0);
                // Equations are Modelica-shaped text, so they get the same
                // syntax colouring as the specimen source view. The tracked
                // identifier is highlighted per token rather than by tinting
                // the whole row — `eq.text.contains(t)` used to match
                // `height` when tracking `h`, and then shade the entire
                // equation rather than the mention within it.
                let modelica = crate::source_view::ModelicaText::new(ui)
                    .tracked(tracked.map(|t| (t, crate::colors::TRACKED_FILL_MEDIUM)));
                for eq in eqs {
                    let selected = highlighted_eq_row == Some(eq.index);
                    let text = modelica.job(&eq.text);
                    if has_incidence {
                        let resp = ui.selectable_label(selected, text);
                        if resp.clicked() {
                            click = Some(SheetClick::Equation(if selected {
                                None
                            } else {
                                Some(eq.index)
                            }));
                        }
                        resp.on_hover_text(format!("f_x[{}] — {}", eq.index, &eq.origin));
                    } else {
                        ui.horizontal(|ui| {
                            ui.label(text);
                        });
                    }
                }
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(4.0);
            ui.label(egui::RichText::new("Variable classification").strong());
            ui.add_space(2.0);

            egui::Grid::new("var_grid")
                .striped(true)
                .num_columns(5)
                .spacing([12.0, 2.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Name").strong());
                    ui.label(egui::RichText::new("Kind").strong());
                    // **A hover is not a hint.** The first version of this put the
                    // reason behind a tooltip on the Kind cell, and Doug reported
                    // the identical complaint that prompted it: "I don't see what
                    // you've added to enable me to understand why h is a state."
                    // Nothing on screen suggested there was anything to hover.
                    // The column is the hint; the tooltip is the detail.
                    ui.label(egui::RichText::new("Why").strong()).on_hover_text(
                        "Why the Kind column says what it says. A variable is a \
                                 state exactly when some equation differentiates it \u{2014} \
                                 hover a cell for the equation itself.",
                    );
                    ui.label(egui::RichText::new("Start").strong());
                    ui.label(egui::RichText::new("Unit").strong());
                    ui.end_row();

                    for v in &sheet.variables {
                        let is_tracked = tracked == Some(v.name.as_str());
                        let mut name_rt = egui::RichText::new(&v.name).monospace();
                        if is_tracked {
                            name_rt = name_rt
                                .strong()
                                .background_color(crate::colors::TRACKED_FILL_MEDIUM);
                        }
                        // Reverse tracking (#37): clicking a variable here
                        // tracks it, and the source view scrolls to its
                        // declaration. Clicking the tracked one again clears
                        // it, matching the source view's toggle behaviour.
                        let resp = ui.add(egui::Label::new(name_rt).sense(egui::Sense::click()));
                        if resp.clicked() {
                            click = Some(SheetClick::Variable(v.name.clone()));
                        }
                        // One vocabulary with every other follow surface, via
                        // the shared helper. Two hand-written variants of
                        // the same sentence drift, and this one still said
                        // "track" after the rename.
                        resp.on_hover_text(crate::follow_hover(&v.name, is_tracked));
                        // **The classification says why it holds.** Doug,
                        // 2026-08-16: "There's no hint provided in the HRW UI
                        // as to why this is a state instead of an algebraic."
                        // Both the short reason and the full sentence are
                        // computed in `equation_sheet`, not here — the paint
                        // path renders strings and decides nothing, so every
                        // claim it makes is unit-testable.
                        let why = v.kind_explanation();
                        let kind_label = ui.label(v.kind);
                        if let Some(text) = &why {
                            kind_label.on_hover_text(text.clone());
                        }

                        // The visible reason. `Sense::hover()` is explicit
                        // because the tooltip is the whole point of the cell and
                        // a bare `ui.label` leaves that to the widget default.
                        let cell =
                            egui::Label::new(egui::RichText::new(v.why_short()).monospace().weak())
                                .sense(egui::Sense::hover());
                        let resp = ui.add(cell);
                        if let Some(text) = &why {
                            resp.on_hover_text(text.clone());
                        }

                        ui.label(v.start.as_deref().unwrap_or("—"));
                        ui.label(v.unit.as_deref().unwrap_or(""));
                        ui.end_row();
                    }
                });
        });

    click
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::equation_sheet::{ClassifiedVariable, EquationCategory, FormattedEquation};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    fn eq(index: usize, text: &str, category: EquationCategory) -> FormattedEquation {
        FormattedEquation {
            index,
            text: text.to_string(),
            origin: "test".to_string(),
            category,
            source_lines: Vec::new(),
        }
    }

    fn var(name: &str, kind: &'static str) -> ClassifiedVariable {
        ClassifiedVariable {
            name: name.to_string(),
            kind,
            unit: None,
            description: None,
            start: None,
            derivative_evidence: None,
        }
    }

    /// A sheet with **two** of the three connector-derived groups, so the family
    /// heading has something to be wrong about.
    fn connector_sheet() -> EquationSheet {
        EquationSheet {
            groups: vec![
                (
                    EquationCategory::Component,
                    vec![eq(0, "der(h) = v", EquationCategory::Component)],
                ),
                (
                    EquationCategory::Connection,
                    vec![
                        eq(1, "p1.v = p2.v", EquationCategory::Connection),
                        eq(2, "p2.v = p3.v", EquationCategory::Connection),
                    ],
                ),
                (
                    EquationCategory::FlowSum,
                    vec![eq(3, "p1.i + p2.i = 0", EquationCategory::FlowSum)],
                ),
            ],
            n_equations: 4,
            variables: vec![var("h", "state"), var("v", "algebraic")],
            n_states: 1,
            n_algebraics: 1,
            ..Default::default()
        }
    }

    /// Render once and return the harness, so a test can query what reached the screen.
    ///
    /// **Sized like a real pane, not like a widget.** The body is a `ScrollArea`, and a
    /// clipped widget stays in the accessibility tree while refusing clicks — the trap
    /// `stage_tabs` recorded, where the failure reads as "the pane did not report the
    /// press".
    fn draw(sheet: EquationSheet, has_incidence: bool) -> Harness<'static, ()> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui(move |ui| {
                equation_sheet_ui(ui, Some(&sheet), has_incidence, None, None);
            });
        h.run_steps(2);
        h
    }

    /// The inputs the click tests vary, plus the press the pane reported.
    struct Pane {
        sheet: EquationSheet,
        highlighted: Option<usize>,
        reported: Option<SheetClick>,
    }

    /// **Accumulate the report rather than overwrite it.** egui delivers the press on
    /// one frame and the harness runs several; assigning the return value on every
    /// frame throws the press away on the next one.
    fn click_harness(highlighted: Option<usize>) -> Harness<'static, Pane> {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(1200.0, 900.0))
            .build_ui_state(
                |ui, p: &mut Pane| {
                    let click = equation_sheet_ui(ui, Some(&p.sheet), true, None, p.highlighted);
                    if click.is_some() {
                        p.reported = click;
                    }
                },
                Pane {
                    sheet: connector_sheet(),
                    highlighted,
                    reported: None,
                },
            );
        h.run_steps(2);
        h
    }

    /// **The family heading counts the whole family, not the group under it.**
    ///
    /// Doug, 2026-08-13: the three `connect`-derived groups share a cause, and drawing
    /// them as flat siblings of `Component equations` said they did not. The heading
    /// that fixed it sums *every* group in the family — here 2 potential equalities plus
    /// 1 flow conservation — so a heading that reported its own group's length would say
    /// `(2)` and quietly under-count the connector's contribution to the model.
    #[test]
    fn the_family_heading_totals_every_group_in_the_family() {
        let h = draw(connector_sheet(), false);

        assert!(
            h.query_by_label_contains("Connector equations (3)")
                .is_some(),
            "the family heading must total all three connector-derived equations"
        );
        // Non-vacuity: the children are on screen under it, with their own counts.
        assert!(
            h.query_by_label_contains("Potential equality (2)")
                .is_some(),
            "the child groups must still carry their own labels and counts"
        );
        assert!(
            h.query_by_label_contains("Flow conservation (1)").is_some(),
            "the second child of the family must render too"
        );
    }

    /// **Drawn once above the contiguous run, not once per group.** The render tracks
    /// only the previous family because `cmp_key` keeps the run contiguous; a heading
    /// per group would repeat it and re-imply the separateness it exists to deny.
    #[test]
    fn the_family_heading_is_drawn_once_for_the_whole_run() {
        let h = draw(connector_sheet(), false);

        assert_eq!(
            h.get_all_by_label_contains("Connector equations").count(),
            1,
            "two groups in one family must share a single heading"
        );
    }

    /// **No incidence matrix, no invitation to click one.** The hint and the
    /// selectability are the same fact stated twice; offering the click where the
    /// highlight has nowhere to land would promise navigation that cannot happen.
    #[test]
    fn without_an_incidence_matrix_the_pane_does_not_offer_the_click() {
        let h = draw(connector_sheet(), false);

        assert!(
            h.query_by_label_contains("Click an equation").is_none(),
            "the click hint must be withheld when there is no matrix to highlight in"
        );
        // Non-vacuity: the equations themselves did render, so the query above was
        // looking at a populated pane.
        assert!(
            h.query_by_label_contains("p1.v = p2.v").is_some(),
            "the equations must render regardless of the incidence matrix"
        );
    }

    /// The partner: with a matrix present, the hint appears.
    #[test]
    fn with_an_incidence_matrix_the_pane_says_the_rows_are_clickable() {
        let h = draw(connector_sheet(), true);

        assert!(
            h.query_by_label_contains("Click an equation").is_some(),
            "a clickable row must say it is clickable"
        );
    }

    /// **Clicking a row reports the index; clicking the highlighted row reports
    /// `None`.** The toggle is the pane's whole click contract, and it lived in a
    /// local that only `App` could observe.
    #[test]
    fn an_equation_row_reports_its_index_and_toggles_off_when_already_highlighted() {
        for (highlighted, expected) in [(None, Some(2)), (Some(2), None)] {
            let mut h = click_harness(highlighted);
            h.get_by_label_contains("p2.v = p3.v").click();
            h.run_steps(2);

            assert_eq!(
                h.state().reported,
                Some(SheetClick::Equation(expected)),
                "a row highlighted={highlighted:?} must report {expected:?}"
            );
        }
    }

    /// **Clicking a variable name reports the name** — reverse tracking (#37). What
    /// `App` then does with it is policy and stays there.
    #[test]
    fn a_variable_name_reports_itself() {
        let mut h = click_harness(None);
        h.get_by_label("h").click();
        h.run_steps(2);

        assert_eq!(
            h.state().reported,
            Some(SheetClick::Variable("h".to_string())),
            "a click on a variable name must come back as that name"
        );
    }
}
