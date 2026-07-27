//! Animated index-reduction stepper — replays the constrained-dummy and
//! missing-derivative reduction algorithms frame by frame.
//!
//! Follows the same dual-mode pattern as `matching_anim` and `tarjan_anim`:
//! - **Recorded**: pre-computed frames from the compilation worker (play/pause/step)
//! - **Live**: reads frames from a shared `LiveTrace` buffer as a debugger
//!   steps through the algorithm on a worker thread

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::dae_prepare::{
    IndexReductionFrame, IndexReductionStep,
};

use crate::expr_format;

/// Animation state for index reduction — supports recorded and live modes.
pub struct ReductionAnimation {
    frames: Vec<IndexReductionFrame>,
    cursor: usize,
    playing: bool,
    interval: f64,
    elapsed: f64,
    live_rx: Option<mpsc::Receiver<IndexReductionFrame>>,
    live_done: Arc<AtomicBool>,
}

impl ReductionAnimation {
    /// Build the animation from pre-recorded frames (recorded mode).
    pub fn from_frames(frames: Vec<IndexReductionFrame>) -> Self {
        Self {
            frames,
            cursor: 0,
            playing: false,
            interval: 0.6,
            elapsed: 0.0,
            live_rx: None,
            live_done: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Start a live debug session: spawn a thread that runs the index
    /// reduction algorithm with a `LiveTrace` producer.
    pub fn start_live(
        dae: rumoca_ir_dae::Dae,
        on_complete: impl FnOnce() + Send + 'static,
    ) -> Option<Self> {
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(std::time::Duration::from_millis(20));
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);

        thread::Builder::new()
            .name("reduction-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                let mut dae = dae;
                let mut frames = Vec::new();
                let mut demoted_so_far = Vec::new();
                let _ = rumoca_phase_structural::dae_prepare
                    ::reduce_constrained_dummy_derivatives_with_trace(
                        &mut dae, Some(&lt), &mut frames, &mut demoted_so_far,
                    );
                let round_offset = frames.iter()
                    .filter_map(|f| match &f.step {
                        IndexReductionStep::RoundComplete { round, .. } => Some(*round + 1),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                let _ = rumoca_phase_structural::dae_prepare
                    ::index_reduce_missing_state_derivatives_with_trace(
                        &mut dae, Some(&lt), &mut frames, &mut demoted_so_far,
                        round_offset,
                    );
                on_complete();
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self {
            frames: Vec::new(),
            cursor: 0,
            playing: false,
            interval: 0.6,
            elapsed: 0.0,
            live_rx: Some(rx),
            live_done: done,
        })
    }

    pub fn is_live(&self) -> bool {
        self.live_rx.is_some()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.live_rx.is_none()
    }

    fn current_frame(&self) -> Option<&IndexReductionFrame> {
        self.frames.get(self.cursor)
    }

    fn sync_live(&mut self) {
        if let Some(rx) = &self.live_rx {
            let before = self.frames.len();
            self.frames.extend(rx.try_iter());
            if self.frames.len() > before {
                self.cursor = self.frames.len().saturating_sub(1);
            }
        }
    }

    pub fn live_finished(&self) -> bool {
        self.live_done.load(Ordering::Acquire)
    }

    /// Render the animation controls and the step display.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.sync_live();

        if self.frames.is_empty() {
            if self.is_live() {
                ui.label("Waiting for first frame from debugger...");
                ui.ctx().request_repaint();
            } else {
                ui.label("No index-reduction trace available.");
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

        if let Some(frame) = self.current_frame() {
            ui.add_space(4.0);
            render_step(ui, frame);
        }

        ui.add_space(8.0);

        if let Some(frame) = self.current_frame() {
            render_state_table(ui, frame);
        }
    }
}

/// Render the current step description with an icon.
fn render_step(ui: &mut egui::Ui, frame: &IndexReductionFrame) {
    let (icon, color, summary) = match &frame.step {
        IndexReductionStep::BeginState { state } => (
            "\u{1f50d}",
            crate::colors::ANIM_EXPLORE,
            format!("Round {}: searching for a constraint to differentiate for state {state}", frame.round),
        ),
        IndexReductionStep::Differentiated { state, .. } => (
            "\u{2702}",
            crate::colors::ANIM_PATH_FOUND,
            format!("Differentiated constraint for {state}"),
        ),
        IndexReductionStep::CandidateExhausted { state } => (
            "\u{274c}",
            crate::colors::ANIM_FAIL,
            format!("No suitable constraint found for state {state}"),
        ),
        IndexReductionStep::Demoted { state } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            format!("State {state} demoted to algebraic"),
        ),
        IndexReductionStep::RoundComplete { round, demotions_this_round } => (
            "\u{2705}",
            crate::colors::ANIM_PATH_FOUND,
            format!("Round {round} complete: {demotions_this_round} demotion(s)"),
        ),
    };

    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(icon).size(16.0));
        ui.label(egui::RichText::new(&summary).color(color).strong());
    });

    if let IndexReductionStep::Differentiated { before_rhs, after_rhs, .. } = &frame.step {
        let before = expr_format::format_expr(before_rhs);
        let after = expr_format::format_expr(after_rhs);
        ui.add_space(4.0);
        egui::Grid::new("diff_eq_grid")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Before:").weak());
                ui.label(egui::RichText::new(format!("0 = {before}")).monospace());
                ui.end_row();
                ui.label(egui::RichText::new("After:").weak());
                ui.label(egui::RichText::new(format!("0 = {after}")).monospace());
                ui.end_row();
            });
    }
}

/// Render the table of states with their current status.
fn render_state_table(ui: &mut egui::Ui, frame: &IndexReductionFrame) {
    if frame.demoted_so_far.is_empty() {
        return;
    }

    ui.label(egui::RichText::new("Demoted states").strong());
    egui::Grid::new("demoted_states_grid")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (i, name) in frame.demoted_so_far.iter().enumerate() {
                ui.label(format!("{}.", i + 1));
                ui.label(egui::RichText::new(name).monospace().color(
                    crate::colors::MATCHED_MARKER,
                ));
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_empty_frames() {
        let anim = ReductionAnimation::from_frames(Vec::new());
        assert!(anim.is_empty());
        assert!(!anim.is_live());
        assert!(anim.live_finished());
    }

    #[test]
    fn from_frames_cursor_starts_at_zero() {
        let frames = vec![
            IndexReductionFrame {
                step: IndexReductionStep::BeginState { state: "x".into() },
                demoted_so_far: vec![],
                round: 0,
            },
            IndexReductionFrame {
                step: IndexReductionStep::Demoted { state: "x".into() },
                demoted_so_far: vec!["x".into()],
                round: 0,
            },
        ];
        let anim = ReductionAnimation::from_frames(frames);
        assert!(!anim.is_empty());
        assert_eq!(anim.cursor, 0);
        assert_eq!(anim.frames.len(), 2);
    }

    #[test]
    fn current_frame_returns_correct_frame() {
        let frames = vec![
            IndexReductionFrame {
                step: IndexReductionStep::RoundComplete { round: 0, demotions_this_round: 3 },
                demoted_so_far: vec!["a".into(), "b".into(), "c".into()],
                round: 0,
            },
        ];
        let anim = ReductionAnimation::from_frames(frames);
        let f = anim.current_frame().unwrap();
        assert_eq!(f.demoted_so_far.len(), 3);
        assert!(matches!(&f.step, IndexReductionStep::RoundComplete { demotions_this_round: 3, .. }));
    }
}
