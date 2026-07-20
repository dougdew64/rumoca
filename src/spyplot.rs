//! BLT block-structure spy-plot (Arc 3, increment 2) — the first custom-`Painter`
//! view, drawn on the reusable [`crate::canvas`] scaffold.
//!
//! **What it shows and why only this.** Rumoca's structural phase produces a
//! `StructuralReport` whose *public* surface is the maximum matching (which
//! equation determines which unknown) and the BLT blocks (the block-lower-
//! triangular evaluation order: scalar solves, and coupled strongly-connected
//! components — algebraic loops — with their tearing). The raw incidence matrix
//! the matching runs on (every equation's full set of referenced unknowns, the
//! off-diagonal sparsity) is `pub(crate)` in `rumoca-phase-structural` and not
//! reachable from HRW; reproducing it would mean re-walking the DAE ourselves and
//! risking a *subtly-wrong* incidence, which the charter's "respect phase
//! boundaries" rule forbids. So this plot draws exactly what the report exposes:
//! the **diagonal blocks** in BLT order. Scalar blocks are single diagonal cells;
//! coupled blocks are boxes on the diagonal (all their equations × unknowns are
//! mutually coupled). Inter-block (lower-triangular) couplings are not drawn —
//! they need the unexposed incidence.
//!
//! Per the observatory's dual-emitter goal, the plot is both a thing to *read*
//! (block structure at a glance — where the algebraic loops are) and a thing to
//! *point at*: hover a block to inspect it, click to capture it into the bridge
//! (`focus.json`) so Claude can explain that block.

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::canvas::Canvas;

/// One BLT block, positioned on the diagonal at `[start, start + size)`.
struct Block {
    /// Index into the structural report's `blocks` array (for capture paths).
    report_index: usize,
    /// First row/column (equations and unknowns share the BLT order here).
    start: usize,
    /// Number of equations/unknowns in the block (1 for scalar).
    size: usize,
    coupled: bool,
    equations: Vec<String>,
    unknowns: Vec<String>,
    /// Present only for coupled blocks that were torn: (tear vars, residual eqs).
    tearing: Option<(Vec<String>, Vec<String>)>,
}

/// A parsed, drawable BLT structure. Owns its strings (no borrow of the report),
/// so building it releases the borrow on `App`'s structural value immediately.
pub struct Plot {
    /// Total dimension (matched equations = unknowns along the diagonal).
    n: usize,
    blocks: Vec<Block>,
    coupled_count: usize,
}

fn str_vec(v: Option<&Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

impl Plot {
    /// Parse the structural report JSON (as emitted by the worker) into a plot.
    /// Returns `None` if there are no blocks to draw.
    pub fn from_report(report: &Value) -> Option<Plot> {
        let blocks_json = report.get("blocks")?.as_array()?;
        if blocks_json.is_empty() {
            return None;
        }
        let mut blocks = Vec::with_capacity(blocks_json.len());
        let mut pos = 0usize;
        let mut coupled_count = 0usize;
        for (report_index, b) in blocks_json.iter().enumerate() {
            let coupled = b.get("kind").and_then(Value::as_str) == Some("coupled");
            let (equations, unknowns) = if coupled {
                (str_vec(b.get("equations")), str_vec(b.get("unknowns")))
            } else {
                let eq = b.get("equation").and_then(Value::as_str).unwrap_or("").to_owned();
                let un = b.get("unknown").and_then(Value::as_str).unwrap_or("").to_owned();
                (vec![eq], vec![un])
            };
            let size = unknowns.len().max(equations.len()).max(1);
            let tearing = b.get("tearing").and_then(|t| {
                if t.is_null() {
                    None
                } else {
                    Some((str_vec(t.get("tear_vars")), str_vec(t.get("residual_equations"))))
                }
            });
            if coupled {
                coupled_count += 1;
            }
            blocks.push(Block { report_index, start: pos, size, coupled, equations, unknowns, tearing });
            pos += size;
        }
        Some(Plot { n: pos, blocks, coupled_count })
    }

    /// One-line caption summarizing the structure (drawn above the canvas).
    pub fn caption(&self) -> String {
        format!(
            "{} block(s) along the diagonal · {} coupled (algebraic loop{}) · {}×{} matched — \
             hover a block to inspect, click to capture",
            self.blocks.len(),
            self.coupled_count,
            if self.coupled_count == 1 { "" } else { "s" },
            self.n,
            self.n,
        )
    }

    /// The block whose diagonal region contains world cell `(col, row)`, if any.
    /// Only the diagonal boxes are interactive (that's all that's drawn).
    fn block_at(&self, col: usize, row: usize) -> Option<&Block> {
        self.blocks.iter().find(|b| {
            let in_range = |i: usize| i >= b.start && i < b.start + b.size;
            in_range(col) && in_range(row)
        })
    }

    /// Draw the plot and handle interaction. Sets `capture` to a bridge key-path
    /// (`blocks[i]`) when the user clicks a block.
    pub fn ui(&self, ui: &mut egui::Ui, canvas: &mut Canvas, capture: &mut Option<Vec<Seg>>) {
        let n = self.n as f32;
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(n, n));
        let (response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        // Backdrop for the matrix area, so the plot reads as a distinct surface.
        painter.rect_filled(view.to_screen_rect(bounds), egui::CornerRadius::ZERO, visuals.extreme_bg_color);

        // Which block is under the pointer this frame (for highlight + tooltip).
        let hovered: Option<&Block> = response.hover_pos().and_then(|p| {
            let w = view.to_world(p);
            if w.x < 0.0 || w.y < 0.0 {
                return None;
            }
            self.block_at(w.x as usize, w.y as usize)
        });

        let matched_color = egui::Color32::from_rgb(0x3F, 0xB9, 0x50); // shared "signal" green
        let coupled_fill = egui::Color32::from_rgba_unmultiplied(0xF2, 0x8C, 0x28, 0x55);
        let coupled_stroke = egui::Color32::from_rgb(0xF2, 0x8C, 0x28);
        let grid = visuals.weak_text_color().gamma_multiply(0.35);

        // Faint grid only when cells are big enough that it isn't a smear.
        if view.zoom() >= 6.0 {
            let stroke = egui::Stroke::new(1.0, grid);
            for i in 0..=self.n {
                let t = i as f32;
                let a = view.to_screen(egui::pos2(t, 0.0));
                let b = view.to_screen(egui::pos2(t, n));
                painter.line_segment([a, b], stroke);
                let c = view.to_screen(egui::pos2(0.0, t));
                let d = view.to_screen(egui::pos2(n, t));
                painter.line_segment([c, d], stroke);
            }
        }

        let cell_rect = |col: usize, row: usize| -> egui::Rect {
            view.to_screen_rect(egui::Rect::from_min_size(
                egui::pos2(col as f32, row as f32),
                egui::vec2(1.0, 1.0),
            ))
        };

        for block in &self.blocks {
            let is_hovered = hovered.is_some_and(|h| h.report_index == block.report_index);
            let block_world = egui::Rect::from_min_size(
                egui::pos2(block.start as f32, block.start as f32),
                egui::vec2(block.size as f32, block.size as f32),
            );
            let block_screen = view.to_screen_rect(block_world);

            if block.coupled {
                // Shade the whole k×k coupled region; the matched diagonal on top.
                painter.rect_filled(block_screen, egui::CornerRadius::ZERO, coupled_fill);
                painter.rect_stroke(
                    block_screen,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(if is_hovered { 2.5 } else { 1.5 }, coupled_stroke),
                    egui::StrokeKind::Inside,
                );
            }

            for i in 0..block.size {
                let cell = cell_rect(block.start + i, block.start + i);
                painter.rect_filled(cell.shrink(view.zoom() * 0.12), egui::CornerRadius::ZERO, matched_color);
            }

            if is_hovered && !block.coupled {
                painter.rect_stroke(
                    block_screen,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, matched_color),
                    egui::StrokeKind::Outside,
                );
            }
        }

        // Hover tooltip + click-to-capture (only over a diagonal block).
        if let Some(block) = hovered {
            if response.clicked() {
                *capture = Some(vec![Seg::Key("blocks".to_owned()), Seg::Index(block.report_index)]);
            }
            response.on_hover_ui(|ui| block_tooltip(ui, block));
        }
    }
}

fn block_tooltip(ui: &mut egui::Ui, block: &Block) {
    if block.coupled {
        ui.strong(format!("Coupled block · size {} (algebraic loop)", block.size));
    } else {
        ui.strong("Scalar block · size 1");
    }
    ui.separator();
    let list = |ui: &mut egui::Ui, title: &str, items: &[String]| {
        ui.label(egui::RichText::new(title).weak());
        for it in items {
            ui.label(egui::RichText::new(it).monospace());
        }
    };
    list(ui, "equations", &block.equations);
    ui.add_space(4.0);
    list(ui, "unknowns", &block.unknowns);
    if let Some((tear_vars, residuals)) = &block.tearing {
        ui.add_space(4.0);
        ui.separator();
        list(ui, "tear (iteration) vars", tear_vars);
        list(ui, "residual equations", residuals);
    }
    ui.add_space(4.0);
    ui.weak("click to capture this block for “explain”");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Blocks lay out consecutively along the diagonal; `block_at` maps a
    /// diagonal cell back to its block, and off-block cells map to nothing.
    #[test]
    fn blocks_tile_the_diagonal_and_hit_test() {
        let report = json!({
            "blocks": [
                { "kind": "scalar", "equation": "e0", "unknown": "u0" },
                { "kind": "coupled", "equations": ["e1", "e2"], "unknowns": ["u1", "u2"],
                  "tearing": { "tear_vars": ["u1"], "residual_equations": ["e1"] } },
                { "kind": "scalar", "equation": "e3", "unknown": "u3" },
            ]
        });
        let plot = Plot::from_report(&report).expect("plot");
        assert_eq!(plot.n, 4); // 1 + 2 + 1 along the diagonal
        assert_eq!(plot.coupled_count, 1);

        // Diagonal cells resolve to the right block index.
        assert_eq!(plot.block_at(0, 0).map(|b| b.report_index), Some(0));
        assert_eq!(plot.block_at(2, 2).map(|b| b.report_index), Some(1)); // inside the coupled box
        assert_eq!(plot.block_at(1, 2).map(|b| b.report_index), Some(1)); // off-diagonal *within* the box
        assert_eq!(plot.block_at(3, 3).map(|b| b.report_index), Some(2));

        // Off-block (between diagonal blocks) is empty — not interactive.
        assert!(plot.block_at(0, 3).is_none());
        assert!(plot.block_at(3, 0).is_none());
    }

    #[test]
    fn empty_report_has_no_plot() {
        assert!(Plot::from_report(&json!({ "blocks": [] })).is_none());
        assert!(Plot::from_report(&json!({})).is_none());
    }
}
