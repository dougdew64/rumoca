//! BLT block-structure spy-plot — a custom-painted matrix view.
//!
//! ## What is a BLT spy-plot?
//!
//! "BLT" stands for **Block Lower Triangular** — a canonical form for systems
//! of equations. After Rumoca's structural analysis phase:
//!
//! 1. A **maximum matching** is computed: each equation is paired with the one
//!    unknown it will "determine" (solve for). This gives a square system.
//!
//! 2. **Strongly connected components (SCCs)** are identified in the dependency
//!    graph. Each SCC becomes a "block":
//!    - A **scalar block** (size 1) means one equation determines one unknown
//!      independently — it can be solved in isolation.
//!    - A **coupled block** (size > 1) means those equations form an **algebraic
//!      loop** — they must be solved simultaneously (e.g., by Newton's method).
//!
//! 3. The blocks are arranged in a **topological order** so that each block
//!    depends only on blocks that come before it (lower-triangular structure).
//!    This ordering is the BLT form.
//!
//! A "spy plot" is a matrix visualization (from MATLAB's `spy()` function) that
//! shows the sparsity pattern — which entries are non-zero. This plot draws the
//! BLT's **diagonal blocks**: scalar blocks as single green cells on the
//! diagonal, coupled blocks as orange-shaded rectangles. The structure reveals
//! at a glance: how many algebraic loops exist, how large they are, and whether
//! the system is mostly sequential (many small diagonal blocks) or heavily
//! coupled (few large blocks).
//!
//! The full incidence matrix (every equation's referenced unknowns, including
//! off-diagonal entries) is shown separately in [`crate::incidence_view`].
//!
//! ## Interaction
//!
//! - **Hover** a block to see a tooltip with its equations, unknowns, and
//!   tearing information (for coupled blocks).
//! - **Click** a block to capture it into the bridge focus file for Claude.
//! - **Pan/zoom** via the shared `Canvas` scaffold (drag to pan, scroll to zoom).

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::canvas::Canvas;

// One BLT block positioned on the diagonal.
//
// Blocks tile the diagonal consecutively: the first block occupies rows/columns
// [0, size0), the second [size0, size0+size1), etc. This `start` + `size`
// encoding makes hit-testing simple: "is (col, row) inside this block?"
struct Block {
    // Index into the structural report's `blocks` JSON array. Used to build the
    // bridge capture path: `blocks[report_index]`.
    report_index: usize,
    // First row/column of this block on the diagonal.
    start: usize,
    // Number of equations/unknowns in the block. Scalar blocks have size=1;
    // coupled blocks have size >= 2.
    size: usize,
    // Whether this block is a coupled SCC (algebraic loop). Scalar blocks
    // are drawn as single green diagonal cells; coupled blocks as orange boxes.
    coupled: bool,
    // Human-readable names of the equations and unknowns in this block.
    equations: Vec<String>,
    unknowns: Vec<String>,
    // For coupled blocks that use tearing: the iteration ("tear") variables
    // and residual equations. Tearing decomposes a coupled block into a
    // smaller nonlinear system by selecting some variables to iterate on.
    // `None` if the block is scalar or untorn.
    tearing: Option<(Vec<String>, Vec<String>)>,
}

/// A parsed, drawable BLT structure ready for rendering.
///
/// Constructed from the structural report JSON by `Plot::from_report`. Owns
/// all its strings (no lifetime dependency on the report `Value`), so the
/// borrow on the app's structural data is released immediately after construction.
/// This is important because egui's immediate-mode rendering needs to borrow
/// the app mutably for interaction while the plot is being drawn.
pub struct Plot {
    // Total dimension: the number of matched equation-unknown pairs.
    // The spy-plot is an n x n grid.
    n: usize,
    blocks: Vec<Block>,
    // Count of coupled blocks (algebraic loops) — shown in the caption.
    coupled_count: usize,
}

use crate::str_vec;

impl Plot {
    /// Parse the structural report JSON into a drawable `Plot`.
    ///
    /// The report is produced by the worker thread from Rumoca's structural
    /// analysis phase. Returns `None` if there are no blocks (e.g., the phase
    /// failed or the model has no equations).
    ///
    /// Blocks are laid out consecutively along the diagonal: each block's
    /// `start` is the cumulative sum of all previous blocks' sizes.
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

    /// One-line caption summarizing the BLT structure, shown above the canvas.
    /// Example: "12 block(s) along the diagonal, 2 coupled (algebraic loops), 14x14 matched"
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

    // Hit-test: which block contains the world cell (col, row)?
    //
    // Only diagonal blocks are drawn and interactive. Off-diagonal cells
    // (between blocks) return None — they are empty space in the BLT form.
    // The linear scan is fine because block counts are small (typically < 50).
    fn block_at(&self, col: usize, row: usize) -> Option<&Block> {
        self.blocks.iter().find(|b| {
            let in_range = |i: usize| i >= b.start && i < b.start + b.size;
            in_range(col) && in_range(row)
        })
    }

    /// Draw the BLT spy-plot and handle hover/click interaction.
    ///
    /// This is the main rendering function, called each frame by the app.
    /// It uses the shared `Canvas` for pan/zoom and coordinate transforms.
    ///
    /// Sets `capture` to a bridge key-path (`blocks[i]`) when the user clicks
    /// a block, enabling the bridge to write a focus file for that block.
    pub fn ui(&self, ui: &mut egui::Ui, canvas: &mut Canvas, capture: &mut Option<Vec<Seg>>) {
        // World bounds: an n x n grid starting at origin.
        let n = self.n as f32;
        let bounds = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(n, n));
        // `canvas.show` allocates the drawing area, applies pan/zoom input,
        // and returns the interaction response, coordinate transform, and painter.
        let (response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        // Draw a background rectangle for the entire matrix area, so the plot
        // stands out from the panel background (especially in dark mode).
        painter.rect_filled(view.to_screen_rect(bounds), egui::CornerRadius::ZERO, visuals.extreme_bg_color);

        let hovered: Option<&Block> = view
            .hovered_cell(&response, self.n, self.n)
            .and_then(|(col, row)| self.block_at(col, row));

        // Color palette:
        // - Green: matched diagonal cells (the eq-unknown pairing).
        // - Orange fill (semi-transparent): coupled block background.
        // - Orange stroke: coupled block outline (thicker when hovered).
        let matched_color = crate::colors::OK_GREEN;
        let coupled_fill = crate::colors::coupled_fill();
        let coupled_stroke = crate::colors::COUPLED_STROKE;
        let grid = visuals.weak_text_color().gamma_multiply(crate::colors::GRID_ALPHA);

        view.draw_grid(&painter, self.n, self.n, grid);

        // --- Draw each block ---
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
                let cell = view.cell_rect(block.start + i, block.start + i);
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

        // --- Hover tooltip + click-to-capture ---
        // Only active when the pointer is over a diagonal block (not empty space).
        if let Some(block) = hovered {
            if response.clicked() {
                // Build the bridge capture path: blocks[<index>].
                // This key-path addresses the block in the structural report JSON,
                // so Claude can look up its equations, unknowns, and tearing info.
                *capture = Some(vec![Seg::Key("blocks".to_owned()), Seg::Index(block.report_index)]);
            }
            // `on_hover_ui` shows an egui tooltip near the cursor.
            response.on_hover_ui(|ui| block_tooltip(ui, block));
        }
    }
}

// Render the tooltip content for a hovered block.
// Shows: block type (scalar vs coupled), size, equation/unknown lists,
// and tearing information for coupled blocks.
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

    #[test]
    fn caption_summarizes_structure() {
        let report = json!({
            "blocks": [
                { "kind": "scalar", "equation": "e0", "unknown": "u0" },
                { "kind": "coupled", "equations": ["e1", "e2"], "unknowns": ["u1", "u2"] },
                { "kind": "scalar", "equation": "e3", "unknown": "u3" },
            ]
        });
        let plot = Plot::from_report(&report).unwrap();
        let c = plot.caption();
        assert!(c.contains("3 block(s)"), "should report 3 blocks: {c}");
        assert!(c.contains("1 coupled"), "should report 1 coupled: {c}");
        assert!(c.contains("4×4"), "should report 4×4 matched: {c}");
    }

    #[test]
    fn caption_pluralizes_coupled() {
        let report = json!({
            "blocks": [
                { "kind": "coupled", "equations": ["e0", "e1"], "unknowns": ["u0", "u1"] },
                { "kind": "coupled", "equations": ["e2", "e3"], "unknowns": ["u2", "u3"] },
            ]
        });
        let plot = Plot::from_report(&report).unwrap();
        let c = plot.caption();
        assert!(c.contains("2 coupled"), "should report 2 coupled: {c}");
        assert!(c.contains("loops"), "should pluralize 'loops': {c}");
    }

    #[test]
    fn caption_no_coupled_blocks() {
        let report = json!({
            "blocks": [
                { "kind": "scalar", "equation": "e0", "unknown": "u0" },
                { "kind": "scalar", "equation": "e1", "unknown": "u1" },
            ]
        });
        let plot = Plot::from_report(&report).unwrap();
        let c = plot.caption();
        assert!(c.contains("0 coupled"), "should report 0 coupled: {c}");
        assert!(c.contains("2×2"), "should report 2×2: {c}");
    }

    #[test]
    fn tearing_info_parsed() {
        let report = json!({
            "blocks": [
                {
                    "kind": "coupled",
                    "equations": ["e0", "e1", "e2"],
                    "unknowns": ["u0", "u1", "u2"],
                    "tearing": {
                        "tear_vars": ["u0"],
                        "residual_equations": ["e0"]
                    }
                }
            ]
        });
        let plot = Plot::from_report(&report).unwrap();
        let block = plot.block_at(0, 0).expect("block at origin");
        assert!(block.coupled);
        let (tear_vars, residuals) = block.tearing.as_ref().expect("should have tearing");
        assert_eq!(tear_vars, &["u0"]);
        assert_eq!(residuals, &["e0"]);
    }
}
