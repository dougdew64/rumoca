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
    /// What the report contained that this parser could not read. Surfaced in
    /// [`caption`](Self::caption), because **this canvas is one of the three surfaces
    /// `egui_kittest` cannot reach** (`docs/tech-debt.md`) — so its correctness rests
    /// on the parsed data being checkable, and on a problem being impossible to
    /// render without.
    problems: Vec<String>,
}

// `str_vec` is gone from this file deliberately: every list it read here is one
// whose loss changes the picture (block members, tear variables), so all four calls
// moved to `str_vec_checked`. See the 2026-08-04 sweep note in `docs/tech-debt.md`.

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
        let mut problems: Vec<String> = Vec::new();
        let mut blocks = Vec::with_capacity(blocks_json.len());
        let mut pos = 0usize;
        let mut coupled_count = 0usize;
        for (report_index, b) in blocks_json.iter().enumerate() {
            // **An unreadable `kind` is not a scalar block.**
            //
            // `== Some("coupled")` made every unreadable kind fall to the `else`
            // branch, which builds a **1x1 block from a single `equation`/`unknown`
            // pair** — so a coupled block whose kind could not be read was drawn as
            // one cell on the diagonal. The spy plot exists to show *where the
            // coupling is*; silently reclassifying a coupled block as scalar
            // inverts the one thing the picture is for. Found by the 2026-08-04
            // sweep.
            let kind = b.get("kind").and_then(Value::as_str);
            if kind.is_none() {
                problems.push(format!(
                    "block {report_index} has no readable `kind` \u{2014} it is drawn as \
                     a scalar block, which is what an unreadable kind used to become \
                     silently"
                ));
            }
            let coupled = kind == Some("coupled");
            let (equations, unknowns) = if coupled {
                let (eqs, p1) = crate::str_vec_checked(b.get("equations"), "equations");
                let (uns, p2) = crate::str_vec_checked(b.get("unknowns"), "unknowns");
                problems.extend(p1.into_iter().chain(p2).map(|p| format!("block {report_index}: {p}")));
                (eqs, uns)
            } else {
                let eq = b.get("equation").and_then(Value::as_str).unwrap_or("").to_owned();
                let un = b.get("unknown").and_then(Value::as_str).unwrap_or("").to_owned();
                (vec![eq], vec![un])
            };
            let size = unknowns.len().max(equations.len()).max(1);
            // Tear variables are the *answer* tearing produces, so a silently
            // dropped one shows a block torn on fewer variables than it was.
            let tearing = match b.get("tearing") {
                Some(t) if !t.is_null() => {
                    let (tv, p1) = crate::str_vec_checked(t.get("tear_vars"), "tear_vars");
                    let (re, p2) =
                        crate::str_vec_checked(t.get("residual_equations"), "residual_equations");
                    problems
                        .extend(p1.into_iter().chain(p2).map(|p| format!("block {report_index}: {p}")));
                    Some((tv, re))
                }
                _ => None,
            };
            if coupled {
                coupled_count += 1;
            }
            blocks.push(Block { report_index, start: pos, size, coupled, equations, unknowns, tearing });
            pos += size;
        }
        Some(Plot { n: pos, blocks, coupled_count, problems })
    }

    /// What the parser could not read — empty when the report read cleanly.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// One-line caption summarizing the BLT structure, shown above the canvas.
    /// Example: "12 block(s) along the diagonal, 2 coupled (algebraic loops), 14x14 matched"
    ///
    /// **Leads with a warning when anything failed to read**, because the count it
    /// quotes — *"2 coupled (algebraic loops)"* — is exactly the number an unreadable
    /// `kind` used to understate, by silently reclassifying a coupled block as
    /// scalar.
    pub fn caption(&self) -> String {
        let caveat = if self.problems.is_empty() {
            String::new()
        } else {
            format!(
                " \u{26a0} {} part(s) of the report could not be read, so the block \
                 structure below is INCOMPLETE and the coupled count may be too low \u{2014}",
                self.problems.len(),
            )
        };
        format!(
            "{} block(s) along the diagonal \u{00b7} {} coupled (algebraic loop{}) \u{00b7} \
             {}\u{00d7}{} matched{} \u{2014} hover a block to inspect, click to capture",
            self.blocks.len(),
            self.coupled_count,
            if self.coupled_count == 1 { "" } else { "s" },
            self.n,
            self.n,
            caveat,
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
    pub fn ui(&self, ui: &mut egui::Ui, canvas: &mut Canvas, capture: &mut Option<Vec<Seg>>, tracked: Option<&str>) {
        // World bounds: an n x n grid starting at origin, with headroom above
        // for angled column labels (visible at zoom >= 16).
        let n = self.n as f32;
        let label_headroom = 1.0_f32;
        let matrix_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(n, n));
        let bounds = egui::Rect::from_min_max(
            egui::pos2(matrix_rect.min.x, matrix_rect.min.y - label_headroom),
            matrix_rect.max,
        );
        // `canvas.show` allocates the drawing area, applies pan/zoom input,
        // and returns the interaction response, coordinate transform, and painter.
        let (response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        // Draw a background rectangle for the entire matrix area, so the plot
        // stands out from the panel background (especially in dark mode).
        painter.rect_filled(view.to_screen_rect(matrix_rect), egui::CornerRadius::ZERO, visuals.extreme_bg_color);

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

        // --- Tracked identifier block highlight ---
        if let Some(name) = tracked {
            for block in &self.blocks {
                if block.unknowns.iter().any(|u| {
                    crate::identifier_index::same_variable(u, name)
                }) {
                    let block_world = egui::Rect::from_min_size(
                        egui::pos2(block.start as f32, block.start as f32),
                        egui::vec2(block.size as f32, block.size as f32),
                    );
                    let block_screen = view.to_screen_rect(block_world);
                    painter.rect_stroke(
                        block_screen,
                        egui::CornerRadius::ZERO,
                        egui::Stroke::new(2.5, crate::colors::TRACKED_GOLD),
                        egui::StrokeKind::Outside,
                    );
                }
            }
        }

        // --- Axis labels (equation names on left, unknown names on top) ---
        let labels_visible = view.zoom() >= crate::LABEL_ZOOM_THRESHOLD;
        if labels_visible {
            let mut col_labels = Vec::with_capacity(self.n);
            let mut row_labels = Vec::with_capacity(self.n);
            for block in &self.blocks {
                col_labels.extend(block.unknowns.iter().cloned());
                row_labels.extend(block.equations.iter().cloned());
            }
            crate::draw_matrix_axis_labels(
                ui, &painter, view,
                &col_labels, &row_labels, 20, 20,
            );
        }

        let canvas_rect = response.rect;

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

        if !labels_visible {
            let hint_pos = canvas_rect.left_bottom() + egui::vec2(4.0, -18.0);
            ui.painter().text(
                hint_pos,
                egui::Align2::LEFT_BOTTOM,
                "Zoom in to see row and column labels",
                egui::FontId::proportional(11.0),
                ui.visuals().weak_text_color(),
            );
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
    ui.weak("click to point at this block, then ask in the chat");
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
        assert!(plot.problems().is_empty(), "a clean report: {:?}", plot.problems());
    }

    /// **An unreadable `kind` is not silently a scalar block.**
    ///
    /// The sweep's finding, 2026-08-04. `kind == Some("coupled")` sent every
    /// unreadable kind down the `else` branch, which builds a **1x1 block from a
    /// single equation/unknown pair** — so a coupled block whose kind could not be
    /// read was drawn as one cell on the diagonal, and `coupled_count` (quoted in the
    /// caption as "N coupled (algebraic loops)") was one too low.
    ///
    /// **The spy plot exists to show where the coupling is.** Reclassifying a coupled
    /// block as scalar inverts the single thing the picture is for.
    #[test]
    fn a_block_with_no_readable_kind_is_reported() {
        let report = json!({ "blocks": [
            { "equations": ["e0", "e1"], "unknowns": ["u0", "u1"] },
        ]});
        let plot = Plot::from_report(&report).expect("parses");
        assert_eq!(plot.problems().len(), 1, "{:?}", plot.problems());
        assert!(plot.problems()[0].contains("kind"), "{:?}", plot.problems()[0]);
        assert!(
            plot.caption().contains("INCOMPLETE"),
            "the caption quotes the coupled count, so it must carry the caveat: {}",
            plot.caption(),
        );
    }

    /// **A tear variable that is not a name is reported, not dropped.**
    ///
    /// Tear variables are the *answer* tearing produces, so losing one shows a block
    /// torn on fewer variables than it was. This loss came through `str_vec`, which
    /// hides a `filter_map` and an `unwrap_or_default` behind a name that suggests
    /// neither — and was therefore invisible to the sweep's own `filter_map` audit.
    #[test]
    fn a_tear_variable_that_is_not_a_name_is_reported() {
        let report = json!({ "blocks": [{
            "kind": "coupled",
            "equations": ["e0", "e1"],
            "unknowns": ["u0", "u1"],
            "tearing": { "tear_vars": ["u0", 7], "residual_equations": ["e0"] },
        }]});
        let plot = Plot::from_report(&report).expect("parses");
        assert_eq!(plot.problems().len(), 1, "{:?}", plot.problems());
        assert!(
            plot.problems()[0].contains("1 of 2") && plot.problems()[0].contains("tear_vars"),
            "{:?}",
            plot.problems()[0],
        );
    }
}
