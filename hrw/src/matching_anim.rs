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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::matching::{
    MatchingFrame, MatchingStep, maximum_matching_with_trace,
};

use crate::canvas::Canvas;
use crate::playback::{Animated, Playback};
use crate::incidence_view::IncidenceMatrix;

/// Seconds between auto-advance frames.
const FRAME_INTERVAL: f64 = 0.4;

/// Animation state machine — supports three modes:
/// 1. **Recorded**: pre-computed frames from `from_incidence` (play/pause/step)
/// 2. **Live**: reads frames from an `mpsc` channel receiver as a debugger
///    steps through the algorithm on a worker thread
pub struct MatchingAnimation {
    /// Cursor, timing and live-session state — see [`Playback`]. The three
    /// animation views used to declare those seven fields each; now only the
    /// matrix geometry below is this view's own.
    playback: Playback<MatchingFrame>,
    n_eq: usize,
    n_var: usize,
    equation_names: Vec<String>,
    unknown_names: Vec<String>,
    rows: Vec<Vec<usize>>,
}

impl MatchingAnimation {
    /// Equations the search gave up on, in the order it gave up.
    ///
    /// One entry per `MatchingStep::EquationFailed`, which is exactly the rank
    /// deficiency of the incidence matrix: each failure is an equation with no
    /// augmenting path left, so it stays unmatched. An empty result means a
    /// perfect matching.
    ///
    /// Reads the whole frame stream rather than the cursor, so it answers "how
    /// does this end" regardless of where playback has got to.
    pub fn failed_equations(&self) -> Vec<usize> {
        self.playback
            .frames()
            .iter()
            .filter_map(|f| match f.step {
                MatchingStep::EquationFailed(eq) => Some(eq),
                _ => None,
            })
            .collect()
    }

    /// `(matched, total)` at the end of the trace — the final matching, not the
    /// cursor's partial one. `matched < total` is a structurally singular system.
    pub fn match_progress(&self) -> (usize, usize) {
        let matched = self
            .playback
            .frames()
            .last()
            .map(|f| f.match_eq.iter().filter(|m| m.is_some()).count())
            .unwrap_or(0);
        (matched, self.n_eq)
    }

    /// Every step in the trace, in order.
    pub fn steps(&self) -> Vec<MatchingStep> {
        self.playback.frames().iter().map(|f| f.step.clone()).collect()
    }

    /// Build the animation trace from a parsed incidence matrix (recorded mode).
    pub fn from_incidence(mat: &IncidenceMatrix) -> Self {
        let eq_vars: Vec<HashSet<usize>> = mat
            .rows()
            .iter()
            .map(|cols| cols.iter().copied().collect())
            .collect();
        let result = maximum_matching_with_trace(mat.n_eq(), mat.n_var(), &eq_vars, None);
        Self {
            // `Playback::recorded` sets `live_done` true, which is the honest
            // answer to "is a live session still running?" for a recorded
            // animation — and what `live_debug_lifecycle` relies on to release a
            // breakpoint armed for a session that is never coming.
            playback: Playback::recorded(result.frames, FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        }
    }

    /// Start a live debug session: spawn a thread that runs the matching
    /// algorithm with a `LiveTrace` producer, then return an animation that
    /// drains frames from the channel receiver. The debugger breakpoint on
    /// `live_trace_breakpoint` pauses the thread after each frame push;
    /// the user steps with F5.
    ///
    /// `on_complete` runs inside the algorithm thread after the last frame
    /// but before the thread exits — the caller uses this to remove the
    /// armed breakpoint via the bridge, preventing SIGSTOP from LLDB when
    /// the thread terminates.
    pub fn start_live(mat: &IncidenceMatrix, on_complete: impl FnOnce() + Send + 'static) -> Option<Self> {
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(std::time::Duration::from_millis(20));
        let done = Arc::new(AtomicBool::new(false));
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
                lt.wait_for_debugger();
                // Where HRW's `LiveTrace` meets the phase's observer callback.
                // The phase crate never learns `LiveTrace` exists — see
                // `rumoca_core::FrameObserver`.
                let observe = |f: &MatchingFrame| lt.push(f.clone());
                maximum_matching_with_trace(n_eq, n_var, &eq_vars, Some(&observe));
                on_complete();
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
            n_eq: mat.n_eq(),
            n_var: mat.n_var(),
            equation_names: mat.equation_texts().to_vec(),
            unknown_names: mat.unknown_names().to_vec(),
            rows: mat.rows().to_vec(),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the animation controls and the annotated incidence matrix.
    ///
    /// Returns `true` on the frame the Debug button is clicked — the caller
    /// owns the bridge state needed to actually arm a session.
    #[must_use]
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        canvas: &mut Canvas,
        tracked: Option<&str>,
        arming: bool,
        debug_enabled: bool,
    ) -> bool {
        // In live mode, sync new frames from the channel receiver.
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        // Nothing to show at all — no recorded frames and no live session.
        if self.playback.is_empty() && !arming {
            ui.label("No matching trace available.");
            return false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }

        // --- Controls ---
        let debug_clicked =
            crate::animation_controls(ui, self.playback.controls(), live, debug_enabled);

        // A session is starting or the debugger is parked at the startup gate,
        // so no frames have arrived. The controls above stay rendered (disabled)
        // rather than the whole row vanishing until the first Continue.
        if self.playback.frames().is_empty() {
            ui.add_space(4.0);
            ui.label("Waiting for first frame from debugger\u{2026}");
            ui.ctx().request_repaint();
            return debug_clicked;
        }

        // --- Step description ---
        if let Some(frame) = self.playback.current() {
            ui.horizontal(|ui| {
                let (icon, desc) = step_description(
                    &frame.step,
                    &self.equation_names,
                    &self.unknown_names,
                );
                ui.label(egui::RichText::new(icon).size(16.0));
                ui.label(desc);
            });
            self.render_running_state(ui, frame);
        }

        ui.add_space(4.0);

        // --- Animated incidence matrix ---
        self.draw_matrix(ui, canvas, tracked);

        debug_clicked
    }

}

impl Animated for MatchingAnimation {
    fn which(&self) -> &'static str {
        "matching"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The step description the view is drawing, plus how much of the matching
    /// is settled.
    ///
    /// `matched` is the count the frame's own `match_eq` snapshot carries, so it
    /// is the state *at this frame* rather than the final result — which is the
    /// whole point of watching an augmenting-path algorithm run. `step` comes
    /// from `step_description`, shared with the on-screen label so the two
    /// cannot give different accounts of the same frame.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        let (_, desc) = step_description(&frame.step, &self.equation_names, &self.unknown_names);
        Some(serde_json::json!({
            "step": desc,
            "matched_so_far": frame.match_eq.iter().filter(|m| m.is_some()).count(),
            "n_equations": self.n_eq,
            "n_unknowns": self.n_var,
        }))
    }
}

impl MatchingAnimation {
    /// What the matrix is showing, in words.
    ///
    /// The step line says what just happened; this says **where the algorithm
    /// stands**, and states the goal once so the picture is legible rather than
    /// decorative. Doug, 2026-07-29: the text-only reduction and `pre()` replays
    /// turned out more useful than expected — *"the text only playbacks provide
    /// useful summaries of what I will find if I decide to step through the
    /// algorithm code"* — and he asked for the same beside the visual ones.
    ///
    /// The counts come from the frame's own `match_eq` snapshot, so they are the
    /// state *at this frame*, not the final result. Watching "3 of 8" climb is
    /// the content of an augmenting-path algorithm; the final number says
    /// nothing about how it got there.
    fn render_running_state(&self, ui: &mut egui::Ui, frame: &MatchingFrame) {
        let matched = frame.match_eq.iter().filter(|m| m.is_some()).count();
        ui.add_space(2.0);
        ui.horizontal_wrapped(|ui| {
            ui.weak("Goal:");
            ui.weak(
                "pair every equation with a distinct unknown it will solve for. \
                 Kuhn's algorithm grows the pairing one equation at a time, \
                 backtracking through already-paired unknowns when it has to.",
            );
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(
                egui::RichText::new(format!("Matched {matched} of {}", self.n_eq))
                    .strong()
                    .color(if matched == self.n_eq {
                        crate::colors::ANIM_PATH_FOUND
                    } else {
                        crate::colors::ANIM_EXPLORE
                    }),
            );
            // Naming what is *not* matched yet is the useful half: a system that
            // ends with an unmatched equation is structurally singular, and that
            // is the outcome the Index Reduction stage exists to fix.
            let unmatched: Vec<&str> = frame
                .match_eq
                .iter()
                .enumerate()
                .filter(|(_, m)| m.is_none())
                .map(|(eq, _)| self.equation_names.get(eq).map(String::as_str).unwrap_or("?"))
                .take(6)
                .collect();
            if !unmatched.is_empty() {
                ui.weak(format!("\u{2014} still unmatched: {}", unmatched.join(", ")));
            }
        });
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

        let Some(frame) = self.playback.current() else {
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
                crate::identifier_index::same_variable(u, name)
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
        assert!(anim.position().1 > 3); // at least TryEquation + Explore + Assign per eq
    }

    /// A recorded animation must report that no live session is running.
    ///
    /// `live_debug_lifecycle` uses this as its breakpoint-cleanup safety net: an
    /// armed breakpoint with no live session coming has to be released. It was
    /// once `false` here, which made that net inert. Now guaranteed by
    /// `Playback::recorded` for every view at once, but asserted here too —
    /// this is the view whose regression prompted it.
    #[test]
    fn recorded_animation_reports_no_live_session() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }

    #[test]
    fn animation_starts_paused_at_frame_zero() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let anim = MatchingAnimation::from_incidence(&mat);
        assert_eq!(anim.position().0, 0);
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
        let last = anim.playback.frames().last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "3x3 system should have perfect matching");
    }

    #[test]
    fn live_mode_receives_all_frames() {
        let mat = IncidenceMatrix::from_report(&sample_report()).unwrap();
        let mut anim = MatchingAnimation::start_live(&mat, || {}).expect("spawn thread");
        for _ in 0..100 {
            if anim.live_state(false) == crate::LiveState::Finished { break; }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        anim.playback.sync_live();
        assert!(!anim.playback.frames().is_empty());
        assert_eq!(anim.live_state(false), crate::LiveState::Finished);
        let last = anim.playback.frames().last().unwrap();
        let matched = last.match_eq.iter().filter(|m| m.is_some()).count();
        assert_eq!(matched, 3, "live mode should reach same final matching");
    }
}
