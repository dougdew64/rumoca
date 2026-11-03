//! Animated index-reduction stepper — replays the constrained-dummy and
//! missing-derivative reduction algorithms frame by frame.
//!
//! Follows the same dual-mode pattern as `matching_anim` and `tarjan_anim`:
//! - **Recorded**: pre-computed frames from the compilation worker (play/pause/step)
//! - **Live**: reads frames from an `mpsc` channel receiver as a debugger
//!   steps through the algorithm on a worker thread

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::LiveTrace;
use rumoca_phase_structural::dae_prepare::{IndexReductionFrame, IndexReductionStep};

use crate::expr_format;
use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. Slower than matching or Tarjan: each
/// index-reduction step changes the *system* rather than moving a cursor, so it
/// needs longer to read.
const FRAME_INTERVAL: f64 = 0.6;

/// Animation state for index reduction — supports recorded and live modes.
///
/// Nothing but playback. Unlike the matching and Tarjan views this algorithm
/// needs no matrix geometry, so the whole type is a [`Playback`] — which is
/// what made it the right one to migrate first.
pub struct ReductionAnimation {
    playback: Playback<IndexReductionFrame>,
}

impl ReductionAnimation {
    /// Build the animation from pre-recorded frames (recorded mode).
    pub fn from_frames(frames: Vec<IndexReductionFrame>) -> Self {
        Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
        }
    }

    /// Start a live debug session: spawn a thread that runs the index
    /// reduction algorithm with a `LiveTrace` producer.
    pub fn start_live(dae: rumoca_ir_dae::Dae, frame_delay: std::time::Duration) -> Option<Self> {
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(frame_delay);
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);

        thread::Builder::new()
            .name("reduction-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                let mut dae = dae;
                let mut frames = Vec::new();
                let mut demoted_so_far = Vec::new();
                // Opening frame — the first Continue after the startup gate
                // lands here, on the system before any reduction, rather than
                // mid-search.
                // Where HRW's `LiveTrace` meets the phase's observer callback.
                // The phase crate never learns `LiveTrace` exists — see
                // `rumoca_core::FrameObserver`.
                let observe = |f: &IndexReductionFrame| lt.push(f.clone());
                rumoca_phase_structural::dae_prepare::emit_index_reduction_start(
                    &mut frames,
                    Some(&observe),
                    &dae,
                    &demoted_so_far,
                );
                let _ = rumoca_phase_structural::dae_prepare
                    ::reduce_constrained_dummy_derivatives_with_trace(
                        &mut dae, Some(&observe), &mut frames, &mut demoted_so_far,
                    );
                let round_offset = frames
                    .iter()
                    .filter_map(|f| match &f.step {
                        IndexReductionStep::RoundComplete { round, .. } => Some(*round + 1),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);
                let _ = rumoca_phase_structural::dae_prepare
                    ::index_reduce_missing_state_derivatives_with_trace(
                        &mut dae, Some(&observe), &mut frames, &demoted_so_far,
                        round_offset,
                    );
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
        })
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the animation controls and the step display.
    ///
    /// Returns `true` on the frame the Debug button is clicked — the caller
    /// owns the bridge state needed to actually arm a session.
    #[must_use]
    pub fn ui(&mut self, ui: &mut egui::Ui, arming: bool, debug_enabled: bool) -> bool {
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        // Nothing to show at all — no recorded frames and no live session.
        if self.playback.is_empty() && !arming {
            ui.label("No index-reduction trace available.");
            return false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }

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

        if let Some(frame) = self.playback.current() {
            ui.add_space(4.0);
            render_step(ui, frame);
        }

        ui.add_space(8.0);

        if let Some(frame) = self.playback.current() {
            render_state_table(ui, frame);
        }

        debug_clicked
    }
}

impl Animated for ReductionAnimation {
    fn which(&self) -> &'static str {
        "reduction"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The step description the view is drawing, plus the reduction's running
    /// state — which states have been demoted so far, and which round this is.
    ///
    /// Those two carry the *shape* of the algorithm: index reduction is a loop
    /// that demotes states until the system stops being structurally singular,
    /// so "round 2, one state demoted" says more about where you are than the
    /// frame number does.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        Some(serde_json::json!({
            "step": step_summary(frame),
            "round": frame.round,
            "demoted_so_far": frame.demoted_so_far,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

/// Render the current step description with an icon.
fn render_step(ui: &mut egui::Ui, frame: &IndexReductionFrame) {
    let (icon, color, summary) = step_style(frame);

    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(icon).size(16.0));
        ui.label(egui::RichText::new(&summary).color(color).strong());
    });

    render_start_states(ui, frame);
}

/// The one-line description of a step — **shared by the view and the capture.**
///
/// `Animated::current_frame_context` hands this exact string to Claude, so
/// what is on screen and what is emitted cannot drift into two different
/// accounts of the same frame. Split out of `render_step` for that reason;
/// re-deriving the wording in the bridge would have been a second definition of
/// what a step means.
fn step_summary(frame: &IndexReductionFrame) -> String {
    step_style(frame).2
}

/// Icon, colour and summary for a step. Icons are only ever codepoints this app
/// already renders — egui ships far less than the whole of Unicode, and an
/// unproven one shows as a tofu box.
fn step_style(frame: &IndexReductionFrame) -> (&'static str, egui::Color32, String) {
    match &frame.step {
        IndexReductionStep::Start { states, equations } => (
            // Clapper board: the start of the take. NOT a checkered flag
            // (U+1F3C1) — that reads as a finish line at the head of the replay.
            "\u{1f3ac}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "Starting point: {} state{}, {} equation{} \u{2014} nothing reduced yet",
                states.len(),
                if states.len() == 1 { "" } else { "s" },
                equations,
                if *equations == 1 { "" } else { "s" },
            ),
        ),
        IndexReductionStep::BeginState { state } => (
            "\u{1f50d}",
            crate::colors::ANIM_EXPLORE,
            // Future tense on purpose. `BeginState` is emitted on *entering* the
            // loop for this state, before any equation is examined — the outcome
            // arrives in the next frame (Differentiated or CandidateExhausted).
            // Every other step here reports a result, so this one has to be
            // visibly the odd one out or it reads as work already done.
            format!(
                "Round {}: state {state} \u{2014} about to search for a constraint to differentiate",
                frame.round,
            ),
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
        IndexReductionStep::RoundComplete {
            round,
            demotions_this_round,
        } => (
            "\u{2705}",
            crate::colors::ANIM_PATH_FOUND,
            format!("Round {round} complete: {demotions_this_round} demotion(s)"),
        ),
    }
}

/// The extra detail some steps carry below the one-line summary.
fn render_start_states(ui: &mut egui::Ui, frame: &IndexReductionFrame) {
    // List the starting states, laid out like the "Demoted states" table below
    // so the before/after comparison reads as a pair.
    if let IndexReductionStep::Start { states, .. } = &frame.step
        && !states.is_empty()
    {
        ui.add_space(4.0);
        ui.label(egui::RichText::new("States entering reduction").strong());
        egui::Grid::new("start_states_grid")
            .num_columns(2)
            .spacing([12.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                for (i, name) in states.iter().enumerate() {
                    ui.label(format!("{}.", i + 1));
                    ui.label(egui::RichText::new(name).monospace());
                    ui.end_row();
                }
            });
    }

    if let IndexReductionStep::Differentiated {
        before_rhs,
        after_rhs,
        ..
    } = &frame.step
    {
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
                ui.label(
                    egui::RichText::new(name)
                        .monospace()
                        .color(crate::colors::MATCHED_MARKER),
                );
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The capture gets the same sentence the view draws.
    ///
    /// `view.animation` used to carry position only — `frame: 12 of 47` — which
    /// says where the user is but not what they are looking at, because frames
    /// live in memory and appear in no stage IR. That mattered for Doug's route
    /// into the algorithms: watch, get confused, ask. Asserted against
    /// `step_summary`, the function `render_step` also uses, so the two cannot
    /// drift into different accounts of one frame.
    #[test]
    fn the_capture_gets_the_same_step_description_the_view_draws() {
        let frames = vec![
            IndexReductionFrame {
                step: IndexReductionStep::BeginState {
                    state: "emf.phi".into(),
                },
                demoted_so_far: vec![],
                round: 0,
            },
            IndexReductionFrame {
                step: IndexReductionStep::Demoted {
                    state: "emf.phi".into(),
                },
                demoted_so_far: vec!["emf.phi".into()],
                round: 1,
            },
        ];
        let expected_first = step_summary(&frames[0]);
        let mut anim = ReductionAnimation::from_frames(frames);

        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["step"], serde_json::json!(expected_first));
        assert_eq!(ctx["round"], serde_json::json!(0));
        assert_eq!(ctx["demoted_so_far"], serde_json::json!([]));

        // The running state travels with the cursor — "round 1, one state
        // demoted" is what says where in the algorithm this is, more than the
        // frame number does.
        *anim.playback.controls().cursor = 1;
        let ctx = anim.current_frame_context().expect("frame 1");
        assert!(
            ctx["step"].as_str().is_some_and(|s| s.contains("demoted")),
            "{ctx}"
        );
        assert_eq!(ctx["round"], serde_json::json!(1));
        assert_eq!(ctx["demoted_so_far"], serde_json::json!(["emf.phi"]));
    }

    /// Before a live session's first frame arrives there is nothing to describe.
    /// `None` is the honest answer, not an empty object.
    #[test]
    fn no_frame_means_no_context() {
        let anim = ReductionAnimation::from_frames(Vec::new());
        assert!(anim.current_frame_context().is_none());
    }

    #[test]
    fn from_empty_frames() {
        let anim = ReductionAnimation::from_frames(Vec::new());
        assert!(anim.is_empty());
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
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
        assert_eq!(anim.position(), (0, 2));
    }

    #[test]
    fn current_frame_returns_correct_frame() {
        let frames = vec![IndexReductionFrame {
            step: IndexReductionStep::RoundComplete {
                round: 0,
                demotions_this_round: 3,
            },
            demoted_so_far: vec!["a".into(), "b".into(), "c".into()],
            round: 0,
        }];
        let anim = ReductionAnimation::from_frames(frames);
        let f = anim.playback.current().unwrap();
        assert_eq!(f.demoted_so_far.len(), 3);
        assert!(matches!(
            &f.step,
            IndexReductionStep::RoundComplete {
                demotions_this_round: 3,
                ..
            }
        ));
    }
}
