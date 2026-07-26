//! Animated matching stepper — replays Kuhn's augmenting-path algorithm
//! frame by frame on the incidence matrix.
//!
//! The animation uses `MatchingFrame`s recorded by the traced matching
//! in `rumoca_phase_structural::matching`. Each frame captures one
//! algorithmic decision (explore an edge, find a free variable, displace
//! a match, assign a pair, or fail) plus a snapshot of the partial
//! matching at that moment.
//!
//! The stepper renders the incidence matrix with:
//! - Partial matching: green circles on currently-matched cells
//! - Active exploration: yellow highlight on the edge being explored
//! - Success/failure flash: green or red on the last outcome
//!
//! Controls: play/pause, step forward/back, reset, speed slider.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::matching::{
    MatchingFrame, MatchingStep, maximum_matching_with_trace,
};

use crate::canvas::Canvas;
use crate::incidence_view::IncidenceMatrix;

/// Animation state machine — supports three modes:
/// 1. **Recorded**: pre-computed frames from `from_incidence` (play/pause/step)
/// 2. **Live**: reads frames from a shared `LiveTrace` buffer as a debugger
///    steps through the algorithm on a worker thread
pub struct MatchingAnimation {
    frames: Vec<MatchingFrame>,
    n_eq: usize,
    n_var: usize,
    equation_names: Vec<String>,
    unknown_names: Vec<String>,
    rows: Vec<Vec<usize>>,
    cursor: usize,
    playing: bool,
    /// Seconds between auto-advance frames.
    interval: f64,
    elapsed: f64,
    /// Live trace buffer — when `Some`, the animation polls for new frames
    /// from a debugger-paused algorithm thread instead of replaying recorded ones.
    live: Option<LiveTrace<MatchingFrame>>,
    /// Whether the live algorithm thread has finished.
    live_done: Arc<Mutex<bool>>,
}

impl MatchingAnimation {
    /// Build the animation trace from a parsed incidence matrix (recorded mode).
    pub fn from_incidence(mat: &IncidenceMatrix) -> Self {
        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let result = maximum_matching_with_trace(mat.n_eq(), mat.n_var(), &eq_vars, None);
        Self {
            frames: result.frames,
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
            cursor: 0,
            playing: false,
            interval: 0.4,
            elapsed: 0.0,
            live: None,
            live_done: Arc::new(Mutex::new(false)),
        }
    }

    /// Start a live debug session: spawn a thread that runs the matching
    /// algorithm with a shared `LiveTrace`, then return an animation that
    /// reads from it. The debugger breakpoint on `live_trace_breakpoint`
    /// pauses the thread after each frame push; the user steps with F5.
    ///
    /// `on_complete` runs inside the algorithm thread after the last frame
    /// but before the thread exits — the caller uses this to remove the
    /// armed breakpoint via the bridge, preventing SIGSTOP from LLDB when
    /// the thread terminates.
    pub fn start_live(mat: &IncidenceMatrix, on_complete: impl FnOnce() + Send + 'static) -> Option<Self> {
        let lt = LiveTrace::new().with_frame_delay(std::time::Duration::from_millis(20));
        let lt_for_thread = lt.clone();
        let done = Arc::new(Mutex::new(false));
        let done_for_thread = Arc::clone(&done);

        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let n_eq = mat.n_eq();
        let n_var = mat.n_var();

        thread::Builder::new()
            .name("matching-debug".to_owned())
            .spawn(move || {
                maximum_matching_with_trace(n_eq, n_var, &eq_vars, Some(&lt_for_thread));
                on_complete();
                *done_for_thread.lock().expect("done lock") = true;
            })
            .ok()?;

        Some(Self {
            frames: Vec::new(),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
            cursor: 0,
            playing: false,
            interval: 0.4,
            elapsed: 0.0,
            live: Some(lt),
            live_done: done,
        })
    }

    /// Whether this animation is in live debug mode.
    pub fn is_live(&self) -> bool {
        self.live.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.live.is_none()
    }

    fn current_frame(&self) -> Option<&MatchingFrame> {
        self.frames.get(self.cursor)
    }

    /// In live mode, pull any new frames from the shared buffer and
    /// auto-advance the cursor to the latest one.
    fn sync_live(&mut self) {
        if let Some(lt) = &self.live {
            let live_len = lt.len();
            if live_len > self.frames.len() {
                let snapshot = lt.snapshot();
                self.frames = snapshot;
                self.cursor = self.frames.len().saturating_sub(1);
            }
        }
    }

    /// Whether the live algorithm thread has finished running.
    pub fn live_finished(&self) -> bool {
        *self.live_done.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Render the animation controls and the annotated incidence matrix.
    pub fn ui(&mut self, ui: &mut egui::Ui, canvas: &mut Canvas, tracked: Option<&str>) {
        // In live mode, sync new frames from the shared buffer.
        self.sync_live();

        if self.frames.is_empty() {
            if self.is_live() {
                ui.label("Waiting for first frame from debugger...");
                ui.ctx().request_repaint();
            } else {
                ui.label("No matching trace available.");
            }
            return;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playing && !self.is_live() {
            self.elapsed += dt;
            if self.elapsed >= self.interval {
                self.elapsed = 0.0;
                if self.cursor + 1 < self.frames.len() {
                    self.cursor += 1;
                } else {
                    self.playing = false;
                }
            }
            ui.ctx().request_repaint();
        }
        if self.is_live() && !self.live_finished() {
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        let n_frames = self.frames.len();
        let is_live = self.is_live();
        let live_finished = self.live_finished();
        crate::animation_controls(
            ui,
            &mut self.cursor,
            &mut self.playing,
            &mut self.elapsed,
            &mut self.interval,
            n_frames,
            is_live,
            live_finished,
        );

        // --- Step description ---
        if let Some(frame) = self.current_frame() {
            ui.horizontal(|ui| {
                let (icon, desc) = step_description(
                    &frame.step,
                    &self.equation_names,
                    &self.unknown_names,
                );
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.label(desc);
            });
        }

        ui.add_space(4.0);

        // --- Animated incidence matrix ---
        self.draw_matrix(ui, canvas, tracked);
    }

    fn draw_matrix(&self, ui: &mut egui::Ui, canvas: &mut Canvas, tracked: Option<&str>) {
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

        let grid = visuals
            .weak_text_color()
            .gamma_multiply(crate::colors::GRID_ALPHA);
        view.draw_grid(&painter, self.n_var, self.n_eq, grid);

        let cell_color = crate::colors::INCIDENCE_CELL.gamma_multiply(0.4);

        // Draw incidence cells (dimmed — the animation overlays are the focus).
        for (row, cols) in self.rows.iter().enumerate() {
            for &col in cols {
                let rect = view.cell_rect(col, row).shrink(view.zoom() * 0.08);
                painter.rect_filled(rect, egui::CornerRadius::ZERO, cell_color);
            }
        }

        let Some(frame) = self.current_frame() else {
            return;
        };

        // Draw partial matching from the frame snapshot.
        let matched_color = crate::colors::MATCHED_MARKER;
        for (row, mc) in frame.match_eq.iter().enumerate() {
            if let Some(col) = *mc {
                let rect = view.cell_rect(col, row);
                // Matched cell: bright fill.
                painter.rect_filled(
                    rect.shrink(view.zoom() * 0.08),
                    egui::CornerRadius::ZERO,
                    crate::colors::INCIDENCE_CELL,
                );
                let center = rect.center();
                let r = view.zoom() * 0.2;
                painter.circle_filled(center, r, matched_color);
            }
        }

        // Highlight based on current step.
        match &frame.step {
            MatchingStep::TryEquation(eq) => {
                // Highlight the entire row being attempted.
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *eq as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.2),
                );
            }
            MatchingStep::Explore { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.5),
                );
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::colors::ANIM_EXPLORE),
                    egui::StrokeKind::Inside,
                );
            }
            MatchingStep::FoundFree { eq, var } | MatchingStep::DisplaceOk { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_PATH_FOUND.gamma_multiply(0.5),
                );
            }
            MatchingStep::DisplaceFail { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_FAIL.gamma_multiply(0.5),
                );
            }
            MatchingStep::TryDisplace { eq, var, holder } => {
                // The contested cell.
                let rect = view.cell_rect(*var, *eq);
                painter.rect_stroke(
                    rect,
                    egui::CornerRadius::ZERO,
                    egui::Stroke::new(2.0, crate::colors::ANIM_EXPLORE),
                    egui::StrokeKind::Inside,
                );
                // Highlight the holder's row — it must find an alternative.
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *holder as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_EXPLORE.gamma_multiply(0.15),
                );
            }
            MatchingStep::Assign { eq, var } => {
                let rect = view.cell_rect(*var, *eq);
                painter.rect_filled(
                    rect,
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_PATH_FOUND.gamma_multiply(0.6),
                );
                let center = rect.center();
                let r = view.zoom() * 0.25;
                painter.circle_filled(center, r, crate::colors::MATCHED_MARKER);
            }
            MatchingStep::EquationFailed(eq) => {
                let band = egui::Rect::from_min_size(
                    egui::pos2(0.0, *eq as f32),
                    egui::vec2(self.n_var as f32, 1.0),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::ANIM_FAIL.gamma_multiply(0.2),
                );
            }
        }

        if let Some(name) = tracked {
            let tracked_col = self.unknown_names.iter().position(|u| {
                u == name || crate::identifier_index::matches_tracked(u, name)
            });
            if let Some(col) = tracked_col {
                let band = egui::Rect::from_min_size(
                    egui::pos2(col as f32, 0.0),
                    egui::vec2(1.0, self.n_eq as f32),
                );
                painter.rect_filled(
                    view.to_screen_rect(band),
                    egui::CornerRadius::ZERO,
                    crate::colors::TRACKED_FILL,
                );
            }
        }

        if view.zoom() >= crate::LABEL_ZOOM_THRESHOLD {
            crate::draw_matrix_axis_labels(
                ui, &painter, view,
                &self.unknown_names, &self.equation_names, 20, 20,
            );
        }

        // Cell tooltip — shows full equation and variable names on hover.
        let hovered_cell = view.hovered_cell(&response, self.n_var, self.n_eq);
        if let Some((col, row)) = hovered_cell {
            response.on_hover_ui(|ui| {
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
            });
        }
    }
}

fn step_description(
    step: &MatchingStep,
    eq_names: &[String],
    var_names: &[String],
) -> (&'static str, String) {
    let eq_name = |i: usize| eq_names.get(i).map(String::as_str).unwrap_or("?");
    let var_name = |i: usize| var_names.get(i).map(String::as_str).unwrap_or("?");
    match step {
        MatchingStep::TryEquation(eq) => (
            "\u{1f50d}",
            format!("Starting augmenting-path search for equation {}: {}", eq, eq_name(*eq)),
        ),
        MatchingStep::Explore { eq, var } => (
            "\u{1f449}",
            format!(
                "Equation {} ({}) exploring variable {} ({})",
                eq, eq_name(*eq), var, var_name(*var),
            ),
        ),
        MatchingStep::FoundFree { eq, var } => (
            "\u{2705}",
            format!(
                "Variable {} ({}) is free — augmenting path found for eq {}",
                var, var_name(*var), eq,
            ),
        ),
        MatchingStep::TryDisplace { eq: _, var, holder } => (
            "\u{1f504}",
            format!(
                "Variable {} ({}) held by eq {} ({}). Can eq {} find an alternative?",
                var, var_name(*var), holder, eq_name(*holder), holder,
            ),
        ),
        MatchingStep::DisplaceOk { eq, var } => (
            "\u{2714}",
            format!(
                "Displacement succeeded — eq {} can take variable {} ({})",
                eq, var, var_name(*var),
            ),
        ),
        MatchingStep::DisplaceFail { eq, var } => (
            "\u{274c}",
            format!(
                "Displacement failed — variable {} ({}) cannot be freed for eq {}",
                var, var_name(*var), eq,
            ),
        ),
        MatchingStep::Assign { eq, var } => (
            "\u{1f517}",
            format!(
                "Matched: equation {} ({}) \u{2194} variable {} ({})",
                eq, eq_name(*eq), var, var_name(*var),
            ),
        ),
        MatchingStep::EquationFailed(eq) => (
            "\u{26a0}",
            format!(
                "Equation {} ({}) has no augmenting path — unmatched (rank deficiency)",
                eq, eq_name(*eq),
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_report() -> serde_json::Value {
        json!({
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
        })
    }

    #[test]
    fn animation_from_incidence_produces_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert!(!anim.is_empty());
        assert!(anim.frames.len() > 3); // at least TryEquation + Explore + Assign per eq
    }

    #[test]
    fn animation_starts_paused_at_frame_zero() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert_eq!(anim.cursor, 0);
        assert!(!anim.playing);
    }

    #[test]
    fn step_description_produces_readable_text() {
        let eq_names = vec!["eq_a".to_string()];
        let var_names = vec!["x".to_string()];
        let (icon, desc) = step_description(
            &MatchingStep::TryEquation(0),
            &eq_names,
            &var_names,
        );
        assert!(!icon.is_empty());
        assert!(desc.contains("eq_a"));
    }

    #[test]
    fn final_frame_has_complete_matching() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        let last = anim.frames.last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "3x3 system should have perfect matching");
    }

    #[test]
    fn live_mode_receives_all_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let mut anim = MatchingAnimation::start_live(&mat, || {}).expect("spawn thread");
        for _ in 0..100 {
            if anim.live_finished() { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anim.sync_live();
        assert!(!anim.frames.is_empty());
        assert!(anim.is_live());
        assert!(anim.live_finished());
        let last = anim.frames.last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "live mode should reach same final matching");
    }
}
