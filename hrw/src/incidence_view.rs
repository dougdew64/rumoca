//! Incidence-matrix custom-painter view.
//!
//! ## What is an incidence matrix?
//!
//! An **incidence matrix** (also called a bipartite adjacency matrix) represents
//! which equations reference which unknowns. It is the fundamental input to
//! structural analysis: before the compiler can determine a solve order (the BLT
//! form), it must know which variables appear in which equations.
//!
//! In this matrix:
//! - **Rows** = equations (e.g., `der(x) = v`, `der(v) = F/m`)
//! - **Columns** = unknowns (e.g., `x`, `v`, `F`)
//! - A **filled cell** at (row, col) means that equation references that unknown
//! - An **empty cell** means no reference
//!
//! The matrix is typically very **sparse** (most equations reference only a few
//! of the system's unknowns), which is why we store it in sparse row format
//! rather than a dense 2D array.
//!
//! The incidence matrix is the input to the **maximum matching** algorithm
//! (Hopcroft-Karp), which pairs each equation with one unknown it can determine.
//! The matching result feeds into the BLT decomposition shown in the spy-plot.
//!
//! ## History
//!
//! This view was deferred during pass one because `build_incidence` was
//! `pub(crate)` in Rumoca — inaccessible from outside the crate. Now that HRW
//! lives inside the Rumoca workspace, the function was widened to `pub` and the
//! worker calls it alongside `build_structural_report`.
//!
//! ## Interaction
//!
//! - **Hover** a cell to see the equation name (row) and unknown name (column),
//!   plus whether the cell is filled or empty.
//! - **Click** a cell to capture that equation's incidence row for Claude.
//! - Crosshair bands highlight the hovered row and column.
//! - Axis labels appear when zoomed in enough (zoom >= 16).

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::canvas::Canvas;
use crate::str_vec;

/// A parsed incidence matrix ready for rendering.
///
/// Constructed from the structural report JSON. Like `spyplot::Plot`, this
/// struct owns all its data (no lifetime dependency on the JSON), so the
/// borrow on the app state is released after construction.
pub struct IncidenceMatrix {
    // Dimensions: n_eq equations (rows) x n_var unknowns (columns).
    n_eq: usize,
    n_var: usize,
    // Human-readable names for display in tooltips and axis labels.
    equation_names: Vec<String>,
    unknown_names: Vec<String>,
    // Sparse row storage (CSR-like): `rows[i]` is a sorted list of column
    // indices where equation i has a non-zero entry. Sorted so we can use
    // binary search for O(log n) hit-testing in `cell_at`.
    rows: Vec<Vec<usize>>,
}

impl IncidenceMatrix {
    /// Parse the incidence data from a structural report JSON.
    ///
    /// The report contains an `incidence` object with `n_eq`, `n_var`,
    /// `unknown_names`, and `rows` (each row has an `equation` name and
    /// a list of `unknowns` column indices). Returns `None` if the data
    /// is missing or malformed (defensive parsing throughout).
    pub fn from_report(report: &Value) -> Option<IncidenceMatrix> {
        let inc = report.get("incidence")?;
        let n_eq = inc.get("n_eq")?.as_u64()? as usize;
        let n_var = inc.get("n_var")?.as_u64()? as usize;
        if n_eq == 0 || n_var == 0 {
            return None;
        }

        let unknown_names = str_vec(inc.get("unknown_names"));
        if unknown_names.len() != n_var {
            return None;
        }

        let rows_json = inc.get("rows")?.as_array()?;
        if rows_json.len() != n_eq {
            return None;
        }

        let mut equation_names = Vec::with_capacity(n_eq);
        let mut rows = Vec::with_capacity(n_eq);
        for r in rows_json {
            let eq_name = r
                .get("equation")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            equation_names.push(eq_name);
            let cols: Vec<usize> = r
                .get("unknowns")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_u64().map(|x| x as usize)).collect())
                .unwrap_or_default();
            rows.push(cols);
        }

        Some(IncidenceMatrix {
            n_eq,
            n_var,
            equation_names,
            unknown_names,
            rows,
        })
    }

    /// One-line summary shown above the canvas.
    /// Reports dimensions, non-zero count, and density percentage.
    pub fn caption(&self) -> String {
        let nnz: usize = self.rows.iter().map(|r| r.len()).sum();
        let total = self.n_eq * self.n_var;
        let density = if total > 0 {
            (nnz as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        format!(
            "{}×{} incidence · {} non-zeros ({:.1}% dense) — \
             hover a cell to inspect, click to capture",
            self.n_eq, self.n_var, nnz, density,
        )
    }

    // Does equation `row` reference unknown `col`?
    //
    // Uses binary search on the sorted column-index list for O(log n) lookup.
    // This is called per-frame for the hovered cell, so it needs to be fast.
    fn cell_at(&self, col: usize, row: usize) -> bool {
        if row >= self.n_eq || col >= self.n_var {
            return false;
        }
        self.rows[row].binary_search(&col).is_ok()
    }

    /// Draw the incidence matrix and handle hover/click interaction.
    ///
    /// Uses the shared `Canvas` for pan/zoom. The rendering has several
    /// level-of-detail tiers:
    /// - Always: filled cells (the actual incidence entries)
    /// - zoom >= 6: grid lines between cells
    /// - zoom >= 16: axis labels (equation names on left, unknown names on top)
    pub fn ui(&self, ui: &mut egui::Ui, canvas: &mut Canvas, capture: &mut Option<Vec<Seg>>) {
        // Reserve extra world-space above the matrix for angled column labels.
        // Without this headroom, the fit-to-content would crop the labels.
        // 6 world units is generous enough for long Modelica variable names
        // rendered at -45 degrees.
        let label_headroom = 6.0_f32;
        let matrix_rect = egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(self.n_var as f32, self.n_eq as f32),
        );
        let bounds = egui::Rect::from_min_max(
            egui::pos2(matrix_rect.min.x, matrix_rect.min.y - label_headroom),
            matrix_rect.max,
        );
        let (response, view, painter) = canvas.show(ui, bounds);

        let visuals = ui.visuals();
        painter.rect_filled(
            view.to_screen_rect(matrix_rect),
            egui::CornerRadius::ZERO,
            visuals.extreme_bg_color,
        );

        let hovered_cell: Option<(usize, usize)> = response.hover_pos().and_then(|p| {
            let w = view.to_world(p);
            if w.x < 0.0 || w.y < 0.0 {
                return None;
            }
            let col = w.x as usize;
            let row = w.y as usize;
            if col < self.n_var && row < self.n_eq {
                Some((col, row))
            } else {
                None
            }
        });

        let cell_color = egui::Color32::from_rgb(0x42, 0x9E, 0xF5);
        let hover_color = egui::Color32::from_rgb(0xFF, 0xC1, 0x07);
        let grid = visuals.weak_text_color().gamma_multiply(0.25);

        view.draw_grid(&painter, self.n_var, self.n_eq, grid);

        // Crosshair bands: highlight the full row and column of the hovered cell
        // with a faint colored band. This visual cue helps the user trace which
        // equation (row) and which unknown (column) the cursor is on, even in
        // large matrices where cell coordinates are hard to count.
        if let Some((hc, hr)) = hovered_cell {
            let row_band = egui::Rect::from_min_size(
                egui::pos2(0.0, hr as f32),
                egui::vec2(self.n_var as f32, 1.0),
            );
            let col_band = egui::Rect::from_min_size(
                egui::pos2(hc as f32, 0.0),
                egui::vec2(1.0, self.n_eq as f32),
            );
            let band_color = hover_color.gamma_multiply(0.12);
            painter.rect_filled(view.to_screen_rect(row_band), egui::CornerRadius::ZERO, band_color);
            painter.rect_filled(view.to_screen_rect(col_band), egui::CornerRadius::ZERO, band_color);
        }

        // Draw filled cells.
        for (row, cols) in self.rows.iter().enumerate() {
            for &col in cols {
                let is_hovered = hovered_cell == Some((col, row));
                let color = if is_hovered { hover_color } else { cell_color };
                let rect = view.cell_rect(col, row).shrink(view.zoom() * 0.08);
                painter.rect_filled(rect, egui::CornerRadius::ZERO, color);
            }
        }

        // Axis labels — only drawn when zoomed in far enough that they won't
        // overlap (zoom >= 16 means each cell is at least 16px wide).
        if view.zoom() >= 16.0 {
            let font = egui::FontId::proportional(view.zoom() * 0.35);
            let label_color = visuals.text_color().gamma_multiply(0.7);
            // Column (unknown) labels: drawn above the matrix at -45 degrees.
            // The angle prevents long Modelica names from overlapping each other.
            let angle = -std::f32::consts::FRAC_PI_4;
            for (col, name) in self.unknown_names.iter().enumerate() {
                // Anchor at the top of each column, slightly above the matrix.
                let anchor = view.to_screen(egui::pos2(col as f32 + 0.5, -0.15));
                let galley = painter.layout_no_wrap(
                    truncate_label(name, 20).to_owned(),
                    font.clone(),
                    label_color,
                );
                // TextShape allows rotated text — egui's `painter.text()` can't rotate.
                let mut shape = egui::epaint::TextShape::new(anchor, galley, label_color);
                shape.angle = angle;
                shape.override_text_color = Some(label_color);
                painter.add(shape);
            }
            // Row (equation) labels: drawn to the left of each row, right-aligned.
            for (row, name) in self.equation_names.iter().enumerate() {
                let pos = view.to_screen(egui::pos2(-0.1, row as f32 + 0.5));
                painter.text(
                    pos,
                    egui::Align2::RIGHT_CENTER,
                    truncate_label(name, 20),
                    font.clone(),
                    label_color,
                );
            }
        }

        // Tooltip and click-to-capture. The capture path addresses the equation's
        // row in the incidence data: `incidence.rows[<row_index>]`.
        if let Some((col, row)) = hovered_cell {
            let filled = self.cell_at(col, row);
            if response.clicked() {
                *capture = Some(vec![
                    Seg::Key("incidence".to_owned()),
                    Seg::Key("rows".to_owned()),
                    Seg::Index(row),
                ]);
            }
            response.on_hover_ui(|ui| {
                self.cell_tooltip(ui, col, row, filled);
            });
        }
    }

    fn cell_tooltip(&self, ui: &mut egui::Ui, col: usize, row: usize, filled: bool) {
        ui.strong(if filled {
            "incidence entry (equation references this unknown)"
        } else {
            "empty (no reference)"
        });
        ui.separator();
        ui.label(egui::RichText::new("equation (row)").weak());
        ui.label(
            egui::RichText::new(
                self.equation_names.get(row).map(String::as_str).unwrap_or("?"),
            )
            .monospace(),
        );
        ui.add_space(4.0);
        ui.label(egui::RichText::new("unknown (col)").weak());
        ui.label(
            egui::RichText::new(
                self.unknown_names.get(col).map(String::as_str).unwrap_or("?"),
            )
            .monospace(),
        );
        ui.add_space(4.0);
        ui.weak("click to capture this equation\u{2019}s incidence for \u{201c}explain\u{201d}");
    }
}

// Truncate a label to at most `max` bytes for display, safely handling
// multi-byte UTF-8 (falls back to the full string if `max` splits a
// character boundary).
fn truncate_label(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        s.get(..max).unwrap_or(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_report() -> Value {
        json!({
            "blocks": [],
            "incidence": {
                "n_eq": 3,
                "n_var": 3,
                "unknown_names": ["der(x)", "y", "z"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0, 1] },
                    { "equation": "f_x[1]", "unknowns": [1, 2] },
                    { "equation": "f_x[2]", "unknowns": [0, 2] },
                ],
            }
        })
    }

    #[test]
    fn parse_and_hit_test() {
        let mat = IncidenceMatrix::from_report(&sample_report()).expect("matrix");
        assert_eq!(mat.n_eq, 3);
        assert_eq!(mat.n_var, 3);

        assert!(mat.cell_at(0, 0));
        assert!(mat.cell_at(1, 0));
        assert!(!mat.cell_at(2, 0));

        assert!(!mat.cell_at(0, 1));
        assert!(mat.cell_at(1, 1));
        assert!(mat.cell_at(2, 1));

        assert!(mat.cell_at(0, 2));
        assert!(!mat.cell_at(1, 2));
        assert!(mat.cell_at(2, 2));
    }

    #[test]
    fn truncate_label_ascii() {
        assert_eq!(truncate_label("abcde", 3), "abc");
        assert_eq!(truncate_label("ab", 3), "ab");
        assert_eq!(truncate_label("abc", 3), "abc");
    }

    #[test]
    fn truncate_label_multibyte_does_not_panic() {
        // U+00E9 (é) is 2 bytes; slicing at byte 1 would split the character.
        let s = "élan";
        assert_eq!(truncate_label(s, 1), s); // falls back to full string
        assert_eq!(truncate_label(s, 2), "é"); // lands on a boundary
    }

    #[test]
    fn empty_incidence_returns_none() {
        assert!(IncidenceMatrix::from_report(&json!({})).is_none());
        assert!(IncidenceMatrix::from_report(&json!({ "incidence": { "n_eq": 0, "n_var": 0, "unknown_names": [], "rows": [] } })).is_none());
    }
}
