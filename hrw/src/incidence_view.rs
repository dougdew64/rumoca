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

/// A parsed incidence matrix ready for rendering, with matching and BLT overlays.
///
/// Constructed from the structural report JSON. Like `spyplot::Plot`, this
/// struct owns all its data (no lifetime dependency on the JSON), so the
/// borrow on the app state is released after construction.
pub struct IncidenceMatrix {
    /// **What the report contained that this parser could not resolve.**
    ///
    /// Added by the 2026-08-04 sweep, which found three silent-loss sites here — the
    /// most consequential in HRW, because this matrix is the object the matching and
    /// BLT tours teach on:
    ///
    /// 1. A non-numeric entry in a row's `unknowns` **dropped a column**, so the
    ///    matrix would show an equation not depending on a variable it depends on.
    /// 2. A BLT block whose member names did not resolve **lost those members and was
    ///    still displayed**, so a four-equation coupled block could render as a
    ///    one-equation block — a decomposition that is simply wrong.
    /// 3. An unresolved matching pair was skipped, and `n_matched` **is the
    ///    structural rank** (`caption` reports rank deficiency from it). A dropped
    ///    pair makes a fully-matched system look singular, which is the single
    ///    conclusion the matching curriculum turns on.
    ///
    /// None of these could have been caught downstream: every one produces a
    /// well-formed matrix that is quietly a different matrix.
    problems: Vec<String>,
    // Dimensions: n_eq equations (rows) x n_var unknowns (columns).
    n_eq: usize,
    n_var: usize,
    // Equation identifiers for matching/BLT overlay lookups (e.g. `f_x[0]`).
    equation_names: Vec<String>,
    // Pretty-printed equation text for display (e.g. `der(w) - tau / J`).
    // Falls back to equation_names when unavailable.
    equation_texts: Vec<String>,
    unknown_names: Vec<String>,
    // Sparse row storage (CSR-like): `rows[i]` is a sorted list of column
    // indices where equation i has a non-zero entry. Sorted so we can use
    // binary search for O(log n) hit-testing in `cell_at`.
    rows: Vec<Vec<usize>>,
    // --- Matching overlay ---
    // For each equation row, the column index of the matched unknown (the
    // transversal). `None` if the equation is unmatched (rank deficiency).
    matched_col: Vec<Option<usize>>,
    // For each unknown column, whether it is matched by any equation.
    col_matched: Vec<bool>,
    // Total number of matched pairs (= structural rank).
    n_matched: usize,
    // --- BLT block boundaries ---
    // Each entry is (start_row, start_col, size) in the matched ordering.
    // These outline the diagonal blocks on the incidence matrix.
    blt_blocks: Vec<BltOverlay>,
}

struct BltOverlay {
    // Block position in equation/unknown index space. These are indices into
    // the matched ordering — the block covers equations [eq_start..eq_start+size)
    // and unknowns [var_start..var_start+size). For the overlay to align with
    // the incidence matrix, we map equation/unknown *names* from the blocks
    // array back to their row/column indices in the incidence matrix.
    eq_indices: Vec<usize>,
    var_indices: Vec<usize>,
    coupled: bool,
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

        let mut problems: Vec<String> = Vec::new();
        let mut equation_names = Vec::with_capacity(n_eq);
        let mut equation_texts = Vec::with_capacity(n_eq);
        let mut rows = Vec::with_capacity(n_eq);
        for (row_i, r) in rows_json.iter().enumerate() {
            let eq_name = r
                .get("equation")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            let eq_text = r
                .get("equation_text")
                .and_then(Value::as_str)
                .unwrap_or(eq_name.as_str())
                .to_owned();
            equation_names.push(eq_name);
            equation_texts.push(eq_text);
            // **A dropped column is a different sparsity pattern.** This was
            // `filter_map(|v| v.as_u64())`, so a non-numeric entry silently removed a
            // dependency — the equation would appear not to involve a variable it
            // does, and every structural conclusion drawn from the picture would be
            // drawn from the wrong matrix.
            let mut cols: Vec<usize> = Vec::new();
            if let Some(a) = r.get("unknowns").and_then(Value::as_array) {
                let mut bad = 0usize;
                for v in a {
                    match v.as_u64() {
                        Some(x) => cols.push(x as usize),
                        None => bad += 1,
                    }
                }
                if bad > 0 {
                    problems.push(format!(
                        "row {row_i}: {bad} of {} column index(es) were not numbers \u{2014} \
                         those dependencies are missing from the matrix",
                        a.len(),
                    ));
                }
            }
            cols.sort_unstable();
            rows.push(cols);
        }

        // Build name → index lookups (used by both matching + BLT parsing).
        let eq_index: std::collections::HashMap<&str, usize> = equation_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();
        let var_index: std::collections::HashMap<&str, usize> = unknown_names
            .iter()
            .enumerate()
            .map(|(i, n)| (n.as_str(), i))
            .collect();

        // --- Parse matching overlay ---
        let mut matched_col = vec![None; n_eq];
        let mut col_matched = vec![false; n_var];
        let mut n_matched = 0;
        if let Some(matching) = report.get("matching").and_then(Value::as_array) {
            for m in matching {
                let eq_name = m.get("equation").and_then(Value::as_str).unwrap_or("");
                let var_name = m.get("unknown").and_then(Value::as_str).unwrap_or("");
                if let (Some(&r), Some(&c)) = (eq_index.get(eq_name), var_index.get(var_name)) {
                    matched_col[r] = Some(c);
                    col_matched[c] = true;
                    n_matched += 1;
                } else {
                    // **`n_matched` IS the structural rank** — `caption` reports rank
                    // deficiency by comparing it against `min(n_eq, n_var)`. A pair
                    // skipped here therefore makes a fully-matched system read as
                    // singular, which is the one conclusion the matching curriculum
                    // turns on. Silently skipping was the behaviour until 2026-08-04.
                    problems.push(format!(
                        "the matching pairs `{eq_name}` with `{var_name}`, and \
                         {} is not in this matrix \u{2014} the pair is missing, so the \
                         structural rank shown below is too low",
                        if eq_index.contains_key(eq_name) { "the unknown" } else { "the equation" },
                    ));
                }
            }
        }

        // --- Parse BLT block boundaries ---
        let mut blt_blocks = Vec::new();
        if let Some(blocks) = report.get("blocks").and_then(Value::as_array) {
            for b in blocks {
                let coupled = b.get("kind").and_then(Value::as_str) == Some("coupled");
                let (eq_names, var_names) = if coupled {
                    (str_vec(b.get("equations")), str_vec(b.get("unknowns")))
                } else {
                    let eq = b.get("equation").and_then(Value::as_str).unwrap_or("").to_owned();
                    let un = b.get("unknown").and_then(Value::as_str).unwrap_or("").to_owned();
                    (vec![eq], vec![un])
                };
                let eq_indices: Vec<usize> = eq_names.iter()
                    .filter_map(|n| eq_index.get(n.as_str()).copied())
                    .collect();
                let var_indices: Vec<usize> = var_names.iter()
                    .filter_map(|n| var_index.get(n.as_str()).copied())
                    .collect();
                // **A block that lost members is a wrong block, not a smaller one.**
                //
                // The `filter_map`s above drop any member whose name is not in this
                // matrix, and the push below only required *one* survivor — so a
                // four-equation coupled block could be drawn as a one-equation block,
                // which is a decomposition the compiler never produced. Same family
                // as the fabricated BLT tabs Doug found on 2026-08-04, arriving by a
                // different route.
                //
                // Still drawn rather than suppressed: a block with real members
                // carries real information, and hiding it would trade one silent loss
                // for another. What changes is that the loss is now stated.
                if eq_indices.len() != eq_names.len() || var_indices.len() != var_names.len() {
                    problems.push(format!(
                        "a {} BLT block names {} equation(s) and {} unknown(s), of which \
                         {} and {} are in this matrix \u{2014} it is drawn below with the \
                         members that resolved, so its size is understated",
                        if coupled { "coupled" } else { "single" },
                        eq_names.len(),
                        var_names.len(),
                        eq_indices.len(),
                        var_indices.len(),
                    ));
                }
                if !eq_indices.is_empty() && !var_indices.is_empty() {
                    blt_blocks.push(BltOverlay { eq_indices, var_indices, coupled });
                }
            }
        }

        Some(IncidenceMatrix {
            problems,
            n_eq,
            n_var,
            equation_names,
            equation_texts,
            unknown_names,
            rows,
            matched_col,
            col_matched,
            n_matched,
            blt_blocks,
        })
    }

    /// What the parser could not resolve — empty when the report read cleanly.
    ///
    /// **Rendered by the caller, above the matrix**, because every entry here means
    /// the picture below is not the compiler's picture. See the field's docs for the
    /// three ways that used to happen silently.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    /// Render the caption above the canvas, **with the problems if there are any**.
    ///
    /// One method rather than two calls at each site, because the emphasis is the
    /// point: the caption is drawn `weak` (dim grey), and a warning in dim grey is a
    /// warning nobody reads. When anything failed to resolve, the caption switches to
    /// the error colour and every problem is listed under it.
    ///
    /// **The caller must not be able to render the caption and forget the problems**,
    /// which is exactly what happened when both were available separately: the two
    /// call sites in `app.rs` had `ui.weak(mat.caption())` and nothing else.
    pub fn caption_ui(&self, ui: &mut egui::Ui) {
        if self.problems.is_empty() {
            ui.weak(self.caption());
            return;
        }
        ui.colored_label(ui.visuals().error_fg_color, self.caption());
        for p in &self.problems {
            ui.colored_label(ui.visuals().error_fg_color, format!("  \u{26a0} {p}"));
        }
    }

    /// One-line summary shown above the canvas.
    ///
    /// **Leads with a warning when anything failed to resolve.** The rank it reports
    /// is computed from `n_matched`, which a dropped matching pair understates — so
    /// on a report this parser could not fully read, the deficiency shown here may be
    /// HRW's and not the model's, and saying "rank deficient" without that caveat is
    /// the exact false conclusion the sweep was looking for.
    pub fn caption(&self) -> String {
        let nnz: usize = self.rows.iter().map(|r| r.len()).sum();
        let total = self.n_eq * self.n_var;
        let density = if total > 0 {
            (nnz as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let rank_info = if self.n_matched < self.n_eq.min(self.n_var) {
            let deficiency = self.n_eq.min(self.n_var) - self.n_matched;
            format!(" · {}/{} matched (rank deficiency {})", self.n_matched, self.n_eq.min(self.n_var), deficiency)
        } else {
            format!(" · {}/{} matched (full rank)", self.n_matched, self.n_eq.min(self.n_var))
        };
        let caveat = if self.problems.is_empty() {
            String::new()
        } else {
            format!(
                " \u{26a0} {} part(s) of the report could not be read, so this matrix \
                 is INCOMPLETE and the rank above may be HRW's fault rather than the \
                 model's \u{2014}",
                self.problems.len(),
            )
        };
        format!(
            "{}\u{00d7}{} incidence \u{00b7} {} non-zeros ({:.1}% dense){}{} \u{2014} \
             hover a cell to inspect, click to capture",
            self.n_eq, self.n_var, nnz, density, rank_info, caveat,
        )
    }

    pub fn n_eq(&self) -> usize { self.n_eq }
    pub fn n_var(&self) -> usize { self.n_var }
    pub fn equation_names(&self) -> &[String] { &self.equation_names }
    pub fn equation_texts(&self) -> &[String] { &self.equation_texts }
    pub fn unknown_names(&self) -> &[String] { &self.unknown_names }
    pub fn rows(&self) -> &[Vec<usize>] { &self.rows }

    /// **What Rumoca said the matching is**, per equation row, resolved from the
    /// report's names to column indices. `None` = that equation is unmatched.
    ///
    /// Exists for `docs/fidelity-plan.md` **F1**: the animations *re-run*
    /// matching rather than reading this, so the two need comparing. Note the
    /// comparison is only meaningful because both sides index the same
    /// row/column order — if the JSON round trip permuted anything, this is
    /// where it shows up.
    pub fn reported_matching(&self) -> &[Option<usize>] { &self.matched_col }

    /// **What Rumoca said the BLT blocks are**, as equation-index sets in report
    /// order. F1 compares these against Tarjan's re-derived SCCs.
    ///
    /// Index sets rather than the `(start, size)` ranges the overlay draws with,
    /// because a strongly connected component is a *set* — comparing rendered
    /// geometry would test the drawing code instead of the decision.
    pub fn reported_blocks(&self) -> Vec<Vec<usize>> {
        self.blt_blocks.iter().map(|b| b.eq_indices.clone()).collect()
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
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.unknown_names.iter().position(|n| {
            crate::identifier_index::same_variable(n, name)
        })
    }

    pub fn ui(
        &self,
        ui: &mut egui::Ui,
        canvas: &mut Canvas,
        capture: &mut Option<Vec<Seg>>,
        highlighted_row: Option<usize>,
        highlighted_col: Option<usize>,
    ) {
        // Bounds include the label anchor region above the matrix (labels sit
        // at y = -0.6). The fit_vertical_bias (0.1) reserves 10% of the view
        // height above the bounds, giving the angled column labels room.
        let label_headroom = 1.0_f32;
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

        let hovered_cell = view.hovered_cell(&response, self.n_var, self.n_eq);

        let cell_color = crate::colors::INCIDENCE_CELL;
        let hover_color = crate::colors::INCIDENCE_HOVER;
        let grid = visuals.weak_text_color().gamma_multiply(crate::colors::GRID_ALPHA);

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

        // --- Persistent highlight from equation sheet cross-link ---
        if let Some(hr) = highlighted_row
            && hr < self.n_eq {
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, hr as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                let highlight_color = crate::colors::INCIDENCE_HOVER.gamma_multiply(0.25);
                painter.rect_filled(view.to_screen_rect(band), egui::CornerRadius::ZERO, highlight_color);
            }

        // --- Persistent column highlight from tracked identifier ---
        if let Some(hc) = highlighted_col
            && hc < self.n_var {
                let band = egui::Rect::from_min_size(
                    egui::pos2(hc as f32, 0.0),
                    egui::vec2(1.0, self.n_eq as f32),
                );
                let highlight_color = crate::colors::TRACKED_FILL;
                painter.rect_filled(view.to_screen_rect(band), egui::CornerRadius::ZERO, highlight_color);
            }

        // --- Unmatched row/column bands (rank deficiency) ---
        // Draw faint red bands behind unmatched rows and columns so rank
        // deficiency is visible spatially, not just as a count in the caption.
        let unmatched_band = crate::colors::UNMATCHED_BAND.gamma_multiply(0.15);
        for (row, mc) in self.matched_col.iter().enumerate() {
            if mc.is_none() {
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, row as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(view.to_screen_rect(band), egui::CornerRadius::ZERO, unmatched_band);
            }
        }
        for (col, &matched) in self.col_matched.iter().enumerate() {
            if !matched {
                let band = egui::Rect::from_min_size(
                    egui::pos2(col as f32, 0.0),
                    egui::vec2(1.0, self.n_eq as f32),
                );
                painter.rect_filled(view.to_screen_rect(band), egui::CornerRadius::ZERO, unmatched_band);
            }
        }

        // Draw filled cells (the incidence entries).
        for (row, cols) in self.rows.iter().enumerate() {
            for &col in cols {
                let is_hovered = hovered_cell == Some((col, row));
                let color = if is_hovered { hover_color } else { cell_color };
                let rect = view.cell_rect(col, row).shrink(view.zoom() * 0.08);
                painter.rect_filled(rect, egui::CornerRadius::ZERO, color);
            }
        }

        // --- Matched-pair markers (the transversal) ---
        // Draw a distinct marker on each matched (row, col) cell so the
        // learner sees which unknown each equation "owns."
        let matched_color = crate::colors::MATCHED_MARKER;
        for (row, mc) in self.matched_col.iter().enumerate() {
            if let Some(col) = *mc {
                let rect = view.cell_rect(col, row);
                let center = rect.center();
                let r = view.zoom() * 0.2;
                painter.circle_filled(center, r, matched_color);
            }
        }

        // --- BLT block boundaries ---
        // Draw outlines around each BLT block's equations and unknowns,
        // connecting the incidence matrix visually to the spy-plot's blocks.
        let blt_stroke_color = crate::colors::BLT_BOUNDARY;
        for block in &self.blt_blocks {
            if block.eq_indices.is_empty() || block.var_indices.is_empty() {
                continue;
            }
            let row_min = *block.eq_indices.iter().min().unwrap();
            let row_max = *block.eq_indices.iter().max().unwrap();
            let col_min = *block.var_indices.iter().min().unwrap();
            let col_max = *block.var_indices.iter().max().unwrap();
            let block_world = egui::Rect::from_min_max(
                egui::pos2(col_min as f32, row_min as f32),
                egui::pos2(col_max as f32 + 1.0, row_max as f32 + 1.0),
            );
            let width = if block.coupled { 2.5 } else { 1.5 };
            painter.rect_stroke(
                view.to_screen_rect(block_world),
                egui::CornerRadius::ZERO,
                egui::Stroke::new(width, blt_stroke_color),
                egui::StrokeKind::Inside,
            );
        }

        // Axis labels — only drawn when zoomed in far enough that they won't
        // overlap (zoom >= 16 means each cell is at least 16px wide).
        let labels_visible = view.zoom() >= crate::LABEL_ZOOM_THRESHOLD;
        if labels_visible {
            crate::draw_matrix_axis_labels(
                ui, &painter, view,
                &self.unknown_names, &self.equation_texts, 20, 30,
            );
        }

        let canvas_rect = response.rect;

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

    fn cell_tooltip(&self, ui: &mut egui::Ui, col: usize, row: usize, filled: bool) {
        ui.strong(if filled {
            "incidence entry (equation references this unknown)"
        } else {
            "empty (no reference)"
        });
        ui.separator();
        ui.label(egui::RichText::new("equation (row)").weak());
        let eq_text = self.equation_texts.get(row).map(String::as_str).unwrap_or("?");
        let eq_name = self.equation_names.get(row).map(String::as_str).unwrap_or("?");
        ui.label(egui::RichText::new(eq_text).monospace());
        if eq_text != eq_name {
            ui.label(egui::RichText::new(eq_name).weak().small());
        }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("unknown (col)").weak());
        ui.label(
            egui::RichText::new(
                self.unknown_names.get(col).map(String::as_str).unwrap_or("?"),
            )
            .monospace(),
        );
        ui.add_space(4.0);
        ui.weak("click to point at this equation\u{2019}s incidence, then ask in the chat");
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

    fn report_with_matching() -> Value {
        json!({
            "matching": [
                { "equation": "f_x[0]", "unknown": "der(x)" },
                { "equation": "f_x[1]", "unknown": "y" },
            ],
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
    fn matching_overlay_resolves_names_to_indices() {
        let mat = IncidenceMatrix::from_report(&report_with_matching()).unwrap();
        assert_eq!(mat.n_matched, 2);
        assert_eq!(mat.matched_col[0], Some(0)); // f_x[0] -> der(x) = col 0
        assert_eq!(mat.matched_col[1], Some(1)); // f_x[1] -> y = col 1
        assert_eq!(mat.matched_col[2], None);    // f_x[2] unmatched
    }

    #[test]
    fn col_matched_tracks_matched_columns() {
        let mat = IncidenceMatrix::from_report(&report_with_matching()).unwrap();
        assert!(mat.col_matched[0]);  // der(x) matched
        assert!(mat.col_matched[1]);  // y matched
        assert!(!mat.col_matched[2]); // z unmatched
    }

    #[test]
    fn no_matching_field_yields_all_unmatched() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        assert_eq!(mat.n_matched, 0);
        assert!(mat.matched_col.iter().all(|m| m.is_none()));
        assert!(mat.col_matched.iter().all(|&m| !m));
    }

    #[test]
    fn caption_shows_rank_deficiency() {
        let mat = IncidenceMatrix::from_report(&report_with_matching()).unwrap();
        let cap = mat.caption();
        assert!(cap.contains("2/3 matched"), "caption: {cap}");
        assert!(cap.contains("rank deficiency 1"), "caption: {cap}");
    }

    #[test]
    fn caption_shows_full_rank() {
        let report = json!({
            "matching": [
                { "equation": "f_x[0]", "unknown": "der(x)" },
                { "equation": "f_x[1]", "unknown": "y" },
                { "equation": "f_x[2]", "unknown": "z" },
            ],
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
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        let cap = mat.caption();
        assert!(cap.contains("3/3 matched"), "caption: {cap}");
        assert!(cap.contains("full rank"), "caption: {cap}");
    }

    #[test]
    fn blt_scalar_block_parsed() {
        let report = json!({
            "matching": [
                { "equation": "f_x[0]", "unknown": "der(x)" },
            ],
            "blocks": [
                { "kind": "scalar", "equation": "f_x[0]", "unknown": "der(x)" },
            ],
            "incidence": {
                "n_eq": 2,
                "n_var": 2,
                "unknown_names": ["der(x)", "y"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0] },
                    { "equation": "f_x[1]", "unknowns": [1] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        assert_eq!(mat.blt_blocks.len(), 1);
        assert!(!mat.blt_blocks[0].coupled);
        assert_eq!(mat.blt_blocks[0].eq_indices, vec![0]);
        assert_eq!(mat.blt_blocks[0].var_indices, vec![0]);
    }

    #[test]
    fn blt_coupled_block_parsed() {
        let report = json!({
            "matching": [],
            "blocks": [
                {
                    "kind": "coupled",
                    "equations": ["f_x[0]", "f_x[1]"],
                    "unknowns": ["der(x)", "y"],
                },
            ],
            "incidence": {
                "n_eq": 2,
                "n_var": 2,
                "unknown_names": ["der(x)", "y"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0, 1] },
                    { "equation": "f_x[1]", "unknowns": [0, 1] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        assert_eq!(mat.blt_blocks.len(), 1);
        assert!(mat.blt_blocks[0].coupled);
        assert_eq!(mat.blt_blocks[0].eq_indices, vec![0, 1]);
        assert_eq!(mat.blt_blocks[0].var_indices, vec![0, 1]);
    }

    #[test]
    fn equation_texts_parsed_from_json() {
        let report = json!({
            "blocks": [],
            "incidence": {
                "n_eq": 2,
                "n_var": 2,
                "unknown_names": ["x", "y"],
                "rows": [
                    { "equation": "f_x[0]", "equation_text": "der(w) - tau / J", "unknowns": [0] },
                    { "equation": "f_x[1]", "equation_text": "der(phi) - w", "unknowns": [1] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        assert_eq!(mat.equation_names(), &["f_x[0]", "f_x[1]"]);
        assert_eq!(mat.equation_texts(), &["der(w) - tau / J", "der(phi) - w"]);
    }

    #[test]
    fn equation_texts_fallback_to_names_when_missing() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        assert_eq!(mat.equation_texts(), mat.equation_names());
    }

    #[test]
    fn unresolvable_matching_names_silently_skipped() {
        let report = json!({
            "matching": [
                { "equation": "BOGUS", "unknown": "ALSO_BOGUS" },
            ],
            "blocks": [],
            "incidence": {
                "n_eq": 1,
                "n_var": 1,
                "unknown_names": ["x"],
                "rows": [
                    { "equation": "eq1", "unknowns": [0] },
                ],
            }
        });
        let mat = IncidenceMatrix::from_report(&report).unwrap();
        assert_eq!(mat.n_matched, 0);
        assert_eq!(mat.matched_col[0], None);
    }

    /// A 2x2 report that reads cleanly, for the three tests below to perturb.
    fn clean_report() -> Value {
        serde_json::json!({
            "incidence": {
                "n_eq": 2, "n_var": 2,
                "unknown_names": ["x", "y"],
                "rows": [
                    { "equation": "e0", "unknowns": [0, 1] },
                    { "equation": "e1", "unknowns": [1] },
                ],
            },
            "matching": [
                { "equation": "e0", "unknown": "x" },
                { "equation": "e1", "unknown": "y" },
            ],
        })
    }

    /// **A clean report reports no problems**, or the warning means nothing.
    #[test]
    fn a_report_that_reads_cleanly_has_no_problems() {
        let mat = IncidenceMatrix::from_report(&clean_report()).expect("parses");
        assert!(mat.problems().is_empty(), "{:?}", mat.problems());
        assert_eq!(mat.n_matched, 2);
        assert!(
            !mat.caption().contains("INCOMPLETE"),
            "a clean matrix must not carry the caveat: {}",
            mat.caption(),
        );
    }

    /// **A dropped column is reported.** It was silently removed until 2026-08-04,
    /// which makes the equation appear not to involve a variable that it does — every
    /// structural conclusion below then comes from the wrong matrix.
    #[test]
    fn a_non_numeric_column_index_is_reported_not_dropped() {
        let mut report = clean_report();
        report["incidence"]["rows"][0]["unknowns"] = serde_json::json!([0, "oops"]);
        let mat = IncidenceMatrix::from_report(&report).expect("parses");

        assert_eq!(mat.rows()[0], vec![0], "the unreadable index has nothing to draw");
        assert_eq!(mat.problems().len(), 1, "{:?}", mat.problems());
        assert!(mat.problems()[0].contains("row 0"), "{:?}", mat.problems()[0]);
        assert!(
            mat.caption().contains("INCOMPLETE"),
            "the caption above the matrix must say the picture is partial: {}",
            mat.caption(),
        );
    }

    /// **An unresolved matching pair is reported**, because `n_matched` *is* the
    /// structural rank and `caption` reports deficiency from it. Skipping the pair
    /// silently makes a fully-matched system read as singular — the single conclusion
    /// the matching curriculum turns on.
    #[test]
    fn an_unresolvable_matching_pair_is_reported() {
        let mut report = clean_report();
        report["matching"][1]["unknown"] = serde_json::json!("not_a_variable");
        let mat = IncidenceMatrix::from_report(&report).expect("parses");

        assert_eq!(mat.n_matched, 1, "the pair genuinely could not be applied");
        assert_eq!(mat.problems().len(), 1, "{:?}", mat.problems());
        assert!(
            mat.problems()[0].contains("structural rank"),
            "the notice must name the consequence, not just the parse failure: {:?}",
            mat.problems()[0],
        );
        // Without the caveat this caption reads "1/2 matched (rank deficiency 1)" —
        // a false claim that this model is structurally singular.
        assert!(mat.caption().contains("INCOMPLETE"), "{}", mat.caption());
    }

    /// **A BLT block that lost members is reported.** It was still drawn, at its
    /// reduced size, so a coupled block could appear as a single-equation block —
    /// a decomposition the compiler never produced.
    #[test]
    fn a_blt_block_with_unresolvable_members_is_reported() {
        let mut report = clean_report();
        report["blocks"] = serde_json::json!([{
            "kind": "coupled",
            "equations": ["e0", "e1", "e_ghost"],
            "unknowns": ["x", "y"],
        }]);
        let mat = IncidenceMatrix::from_report(&report).expect("parses");

        assert_eq!(mat.problems().len(), 1, "{:?}", mat.problems());
        assert!(
            mat.problems()[0].contains("3 equation(s)") && mat.problems()[0].contains("2 and 2"),
            "the notice must give both sizes so the gap is visible: {:?}",
            mat.problems()[0],
        );
    }
}
