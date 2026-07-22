//! Incidence-matrix custom-painter view (Pass two) — the equation×unknown
//! bipartite adjacency that the matching runs on.
//!
//! Pass one deferred this because `build_incidence` was `pub(crate)`. Now that
//! it's `pub`, the worker calls it alongside `build_structural_report` and ships
//! the sparse adjacency in the stage JSON. This view draws it: equations as rows,
//! unknowns as columns, a filled cell wherever an equation references an unknown.
//! Hover a cell to see the equation + unknown name; click to capture.

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::canvas::Canvas;

pub struct IncidenceMatrix {
    n_eq: usize,
    n_var: usize,
    equation_names: Vec<String>,
    unknown_names: Vec<String>,
    /// Sparse row storage: for each equation, sorted column indices.
    rows: Vec<Vec<usize>>,
}

impl IncidenceMatrix {
    pub fn from_report(report: &Value) -> Option<IncidenceMatrix> {
        let inc = report.get("incidence")?;
        let n_eq = inc.get("n_eq")?.as_u64()? as usize;
        let n_var = inc.get("n_var")?.as_u64()? as usize;
        if n_eq == 0 || n_var == 0 {
            return None;
        }

        let unknown_names: Vec<String> = inc
            .get("unknown_names")?
            .as_array()?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect();
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

    fn cell_at(&self, col: usize, row: usize) -> bool {
        if row >= self.n_eq || col >= self.n_var {
            return false;
        }
        self.rows[row].binary_search(&col).is_ok()
    }

    pub fn ui(&self, ui: &mut egui::Ui, canvas: &mut Canvas, capture: &mut Option<Vec<Seg>>) {
        // Reserve headroom above the matrix for the angled column labels so the
        // fit-to-content keeps them visible. The labels are ~0.35 zoom-units tall
        // at -45°; 6 world units is generous enough for long names.
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

        // Grid lines when zoomed in enough.
        if view.zoom() >= 6.0 {
            let stroke = egui::Stroke::new(1.0, grid);
            for col in 0..=self.n_var {
                let a = view.to_screen(egui::pos2(col as f32, 0.0));
                let b = view.to_screen(egui::pos2(col as f32, self.n_eq as f32));
                painter.line_segment([a, b], stroke);
            }
            for row in 0..=self.n_eq {
                let a = view.to_screen(egui::pos2(0.0, row as f32));
                let b = view.to_screen(egui::pos2(self.n_var as f32, row as f32));
                painter.line_segment([a, b], stroke);
            }
        }

        let cell_rect = |col: usize, row: usize| -> egui::Rect {
            view.to_screen_rect(egui::Rect::from_min_size(
                egui::pos2(col as f32, row as f32),
                egui::vec2(1.0, 1.0),
            ))
        };

        // Highlight the hovered row and column with a faint band.
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
                let rect = cell_rect(col, row).shrink(view.zoom() * 0.08);
                painter.rect_filled(rect, egui::CornerRadius::ZERO, color);
            }
        }

        // Axis labels when zoomed in enough.
        if view.zoom() >= 16.0 {
            let font = egui::FontId::proportional(view.zoom() * 0.35);
            let label_color = visuals.text_color().gamma_multiply(0.7);
            let angle = -std::f32::consts::FRAC_PI_4; // -45° (reads bottom-left to top-right)
            for (col, name) in self.unknown_names.iter().enumerate() {
                let anchor = view.to_screen(egui::pos2(col as f32 + 0.5, -0.15));
                let galley = painter.layout_no_wrap(
                    truncate_label(name, 20).to_owned(),
                    font.clone(),
                    label_color,
                );
                let mut shape = egui::epaint::TextShape::new(anchor, galley, label_color);
                shape.angle = angle;
                // Pivot at the bottom-right of the text so labels fan out above/left.
                shape.override_text_color = Some(label_color);
                painter.add(shape);
            }
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

        // Tooltip + click-to-capture.
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

fn truncate_label(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
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
    fn empty_incidence_returns_none() {
        assert!(IncidenceMatrix::from_report(&json!({})).is_none());
        assert!(IncidenceMatrix::from_report(&json!({ "incidence": { "n_eq": 0, "n_var": 0, "unknown_names": [], "rows": [] } })).is_none());
    }
}
