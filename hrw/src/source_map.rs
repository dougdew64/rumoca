//! **The source map** — the specimen's text beside the equations it produced.
//!
//! Lifted out of `app.rs` on 2026-08-19, the first *rendering* function to leave and
//! the proof that one can. See [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why this one first
//!
//! **Coupling, not size.** It is 245 lines and touches exactly **four** of `App`'s fields,
//! where `central_panel_ui` is 602 lines and touches **43**. The two measures turned out
//! nearly uncorrelated, and coupling is what decides whether an extraction is possible at
//! all: at 43 fields the only signatures available are forty-three parameters or `&mut App`,
//! and the plan rejects the second as reducing nothing.
//!
//! # The signature is the whole point
//!
//! Four parameters instead of `&mut self`. **That is what makes this an extraction rather
//! than a rename** — a reader of this file learns exactly what the source map can touch, and
//! the compiler now enforces it. `viewport` is `&mut` because the view genuinely moves the
//! camera; the other three are shared.

use eframe::egui;

use crate::stage_view::Viewport;
use crate::{equation_sheet, identifier_index as ident_index};

/// Fraction of available width given to the source column in the split view.
///
/// Moved here with the view it configures: a constant used by exactly one function is
/// state that function owns, and leaving it behind would mean the extraction reduced
/// what app.rs holds without reducing what it declares.
const SOURCE_MAP_SPLIT_FRACTION: f32 = 0.45;
pub(crate) fn source_map_ui(
    ui: &mut egui::Ui,
    cached_equation_sheet: &Option<equation_sheet::EquationSheet>,
    identifier_index: &Option<ident_index::IdentifierIndex>,
    tracked_identifier: &Option<String>,
    viewport: &mut Viewport,
) {
    let Some(sheet) = &cached_equation_sheet else {
        ui.weak("(no equation sheet)");
        return;
    };
    if sheet.source_lines.is_empty() {
        ui.weak("(no source mapping available)");
        return;
    }

    let highlighted_line = viewport.highlighted_source_line;
    let highlighted_eq = viewport.highlighted_eq_row;
    let tracked = tracked_identifier.as_deref();
    let tracked_line = tracked_identifier.as_deref().and_then(|name| {
        identifier_index
            .as_ref()
            .and_then(|idx| idx.variables.get(name))
            .map(|v| v.source_line)
    });

    // Collect equation indices associated with the highlighted source line.
    let line_eq_indices: Vec<usize> = highlighted_line
        .and_then(|ln| sheet.source_lines.get(ln as usize - 1))
        .map(|sl| sl.equation_indices.clone())
        .unwrap_or_default();

    // Collect source lines associated with the highlighted equation.
    let eq_source_lines: Vec<u32> = if let Some(eq_idx) = highlighted_eq {
        sheet
            .groups
            .iter()
            .flat_map(|(_, eqs)| eqs)
            .find(|eq| eq.index == eq_idx)
            .map(|eq| eq.source_lines.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let mut clicked_line = None;
    let mut clicked_eq = None;

    let avail = ui.available_size();
    let left_width = (avail.x * SOURCE_MAP_SPLIT_FRACTION).max(200.0);

    // Use StripBuilder-style layout: a left child_ui for source, a
    // separator, then the remaining space for equations. Both children
    // get the full available height.
    let full_rect = ui.available_rect_before_wrap();
    let left_rect =
        egui::Rect::from_min_size(full_rect.min, egui::vec2(left_width, full_rect.height()));
    let sep_x = left_rect.max.x;
    let right_rect =
        egui::Rect::from_min_max(egui::pos2(sep_x + 6.0, full_rect.min.y), full_rect.max);

    // ---- Left pane: source code ----
    let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
    left_ui.label(egui::RichText::new("Modelica source").strong());
    left_ui.weak("Click a line to see which equations it produced.");
    left_ui.add_space(4.0);

    egui::ScrollArea::both()
        .id_salt("source_map_source")
        .auto_shrink(false)
        .show(&mut left_ui, |ui| {
            for sl in &sheet.source_lines {
                let is_selected = highlighted_line == Some(sl.line_number);
                let is_eq_linked = eq_source_lines.contains(&sl.line_number);
                let is_tracked = tracked_line == Some(sl.line_number);
                let has_equations = !sl.equation_indices.is_empty();

                // Foreground = syntax, background = relationship. The line
                // number is not Modelica, so it is appended plainly rather
                // than being run through the lexer.
                let background = if is_tracked {
                    Some(crate::colors::TRACKED_FILL_MEDIUM)
                } else if is_eq_linked {
                    Some(crate::colors::SOURCE_MAP_LINK)
                } else {
                    None
                };
                let modelica = crate::source_view::ModelicaText::new(ui).background(background);
                let mut job = egui::text::LayoutJob::default();
                modelica.append_plain(
                    &mut job,
                    &format!("{:>4} ", sl.line_number),
                    ui.visuals().weak_text_color(),
                );
                modelica.append(&mut job, &sl.text);
                let text = job;

                if has_equations {
                    if let Some(cat) = sl.category {
                        let color = cat.color().gamma_multiply(0.7);
                        let bar_rect = ui
                            .horizontal(|ui| {
                                let resp = ui.selectable_label(is_selected, text);
                                if resp.clicked() {
                                    clicked_line = Some(if is_selected {
                                        None
                                    } else {
                                        Some(sl.line_number)
                                    });
                                }
                                resp.rect
                            })
                            .inner;
                        let painter = ui.painter();
                        let bar = egui::Rect::from_min_size(
                            bar_rect.left_top(),
                            egui::vec2(3.0, bar_rect.height()),
                        );
                        painter.rect_filled(bar, egui::CornerRadius::ZERO, color);
                    } else {
                        let resp = ui.selectable_label(is_selected, text);
                        if resp.clicked() {
                            clicked_line = Some(if is_selected {
                                None
                            } else {
                                Some(sl.line_number)
                            });
                        }
                    }
                } else {
                    ui.label(text);
                }
            }
        });

    // Separator line between the two panes.
    ui.painter().vline(
        sep_x + 2.0,
        full_rect.y_range(),
        ui.visuals().widgets.noninteractive.bg_stroke,
    );

    // ---- Right pane: equations linked to the selected source line ----
    let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
    right_ui.label(egui::RichText::new("Flat equations").strong());
    if let Some(ln) = highlighted_line {
        let count = line_eq_indices.len();
        if count > 0 {
            right_ui.weak(format!(
                "{count} equation{} from line {ln}",
                if count == 1 { "" } else { "s" },
            ));
        } else {
            right_ui.weak(format!("Line {ln} — no equations"));
        }
    } else {
        right_ui.weak("Click a source line to see its equations.");
    }
    right_ui.add_space(4.0);

    egui::ScrollArea::both()
        .id_salt("source_map_equations")
        .auto_shrink(false)
        .show(&mut right_ui, |ui| {
            for (cat, eqs) in &sheet.groups {
                let visible_eqs: Vec<_> = if line_eq_indices.is_empty() {
                    eqs.iter().collect()
                } else {
                    eqs.iter()
                        .filter(|eq| line_eq_indices.contains(&eq.index))
                        .collect()
                };
                if visible_eqs.is_empty() {
                    continue;
                }

                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} ({})", cat.label(), visible_eqs.len()))
                        .strong()
                        .color(cat.color()),
                );
                ui.add_space(2.0);

                for eq in visible_eqs {
                    let is_selected = highlighted_eq == Some(eq.index);

                    // No line-linked cue here, deliberately. This list is
                    // already *filtered* to the selected line's equations
                    // (see `visible_eqs` above), so such a cue would be true
                    // of every visible row and false of none — it cannot
                    // mark a subset when the subset is the whole list. The
                    // filter, plus the "N equations from line X" header, is
                    // the signal. The source-lines column is different: it
                    // is unfiltered, so its highlight does pick out a subset.
                    //
                    // Only per-equation facts get colour: the tracked
                    // identifier, and selectable_label's own selection state.
                    let text = crate::source_view::ModelicaText::new(ui)
                        .tracked(tracked.map(|t| (t, crate::colors::TRACKED_FILL_MEDIUM)))
                        .job(&eq.text);

                    let resp = ui.selectable_label(is_selected, text);
                    if resp.clicked() {
                        clicked_eq = Some(if is_selected { None } else { Some(eq.index) });
                    }
                    if eq.source_lines.is_empty() {
                        resp.on_hover_text(
                            format!("f_x[{}] — {} (library)", eq.index, &eq.origin,),
                        );
                    } else if eq.source_lines.len() == 1 {
                        resp.on_hover_text(format!(
                            "f_x[{}] — {} (line {})",
                            eq.index, &eq.origin, eq.source_lines[0],
                        ));
                    } else {
                        let lines_str: Vec<String> =
                            eq.source_lines.iter().map(|ln| ln.to_string()).collect();
                        resp.on_hover_text(format!(
                            "f_x[{}] — {} (lines {})",
                            eq.index,
                            &eq.origin,
                            lines_str.join(", "),
                        ));
                    }
                }
            }
        });

    // Consume the full rect so the parent layout knows this space is used.
    ui.allocate_rect(full_rect, egui::Sense::hover());

    if let Some(new_val) = clicked_line {
        viewport.highlighted_source_line = new_val;
        viewport.highlighted_eq_row = None;
    }
    if let Some(new_val) = clicked_eq {
        viewport.highlighted_eq_row = new_val;
        if let Some(eq_idx) = new_val {
            let sheet = cached_equation_sheet.as_ref().unwrap();
            let line = sheet
                .groups
                .iter()
                .flat_map(|(_, eqs)| eqs)
                .find(|eq| eq.index == eq_idx)
                .and_then(|eq| eq.source_lines.first().copied());
            viewport.highlighted_source_line = line;
        }
    }
}
