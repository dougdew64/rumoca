//! Animated `pre()`-lowering stepper — replays the manufacture of a
//! `__pre__.x` parameter slot, frame by frame.
//!
//! ## Why this phase is worth animating
//!
//! `__pre__.overSpeed` appears in the Events IR, in Solve lowering's parameter
//! vector, and **in no source file anywhere**. It is synthesized: a `when`
//! equation needs a value to hold when no branch fires, and a DAE has no way to
//! say "unchanged", so the previous value gets a variable of its own. Reading
//! the phase's *output* shows the slot already existing; only watching the pass
//! run shows it being made.
//!
//! Four beats, in the order the algorithm takes them:
//!
//! 1. **Discovered** — a `pre(x)` reference is found.
//! 2. **Named** — `pre_slot_name(x)` constructs `__pre__.x`. The single most
//!    informative frame: everything downstream follows from this line.
//! 3. **Materialized** — built as a *parameter*, inheriting the base variable's
//!    shape and start value. This is why a pre-slot lands among the parameters
//!    rather than the unknowns, and why it shows up as `P[…]` in solve lowering.
//! 4. **Substituted** — a partition's equations are rewritten to reference it.
//!
//! ## The first non-structural phase instrumented
//!
//! Idea #40 also asked whether `LiveTrace` generalises beyond the structural
//! phases. It does not need to: `rumoca-phase-dae` takes an **observer
//! callback** rather than a `LiveTrace`, because `LiveTrace` lives in
//! `rumoca-phase-structural` and a dependency from DAE construction onto
//! structural analysis would run backwards through the pipeline. The glue is
//! here instead — HRW owns the `LiveTrace` and hands the phase a closure that
//! pushes into it. See `hrw/DECISIONS.md` (2026-07-29).
//!
//! What *did* generalise is [`Playback`]: this view declares no cursor, no
//! timing, no channel — it is the fourth user of yesterday's refactor, and the
//! reason that refactor was scheduled before this work.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use eframe::egui;

use rumoca_phase_dae::{PreLoweringFrame, PreLoweringStep};
use rumoca_phase_structural::LiveTrace;

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. Matches the reduction view's pace:
/// each step changes the *system* rather than moving a cursor.
const FRAME_INTERVAL: f64 = 0.6;

/// Replay of `pre()` lowering — recorded or live.
pub struct PreLoweringAnimation {
    playback: Playback<PreLoweringFrame>,
}

impl PreLoweringAnimation {
    /// Build from frames recorded during compilation.
    pub fn from_frames(frames: Vec<PreLoweringFrame>) -> Self {
        Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
        }
    }

    /// Start a live debug session.
    ///
    /// Re-runs **DAE construction** on the flat model, not the pass alone. By
    /// the time HRW holds a DAE, pre-lowering has already run: the slots exist
    /// and the `pre()` calls are gone, so re-running the pass on it would trace
    /// nothing. The flat model is the last artifact from before the pass, which
    /// is why `CompilationResult::flat` is what gets carried here.
    ///
    /// The closure is where HRW's `LiveTrace` meets the phase's observer
    /// callback — the phase crate never learns that `LiveTrace` exists.
    pub fn start_live(
        flat: rumoca_ir_flat::Model,
        frame_delay: std::time::Duration,
    ) -> Option<Self> {
        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(frame_delay);
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);

        thread::Builder::new()
            .name("pre-lowering-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                let _ = rumoca_phase_dae::to_dae_with_options_traced(
                    &flat,
                    Default::default(),
                    Some(&|frame: &PreLoweringFrame| lt.push(frame.clone())),
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

    /// Render the controls and the step display.
    ///
    /// Returns `true` on the frame the Debug button is clicked — the caller owns
    /// the bridge state needed to actually arm a session.
    #[must_use]
    pub fn ui(&mut self, ui: &mut egui::Ui, arming: bool, debug_enabled: bool) -> bool {
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        if self.playback.is_empty() && !arming {
            ui.label("No pre() lowering trace available.");
            return false;
        }

        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }

        let debug_clicked =
            crate::animation_controls(ui, self.playback.controls(), live, debug_enabled);

        if self.playback.frames().is_empty() {
            ui.add_space(4.0);
            ui.label("Waiting for first frame from debugger\u{2026}");
            ui.ctx().request_repaint();
            return debug_clicked;
        }

        if let Some(frame) = self.playback.current() {
            ui.add_space(4.0);
            render_step(ui, frame);
            ui.add_space(8.0);
            render_slots(ui, frame);
        }

        debug_clicked
    }
}

impl Animated for PreLoweringAnimation {
    fn which(&self) -> &'static str {
        "pre_lowering"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The step description the view is drawing, plus the slots created so far.
    ///
    /// `slots_so_far` is the running set, not the final one — watching it go
    /// from empty to three is the whole content of this phase.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        Some(serde_json::json!({
            "step": step_summary(frame),
            "slots_so_far": frame.slots_so_far,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

/// The one-line description of a step — **shared by the view and the capture**,
/// so the screen and the emitted context cannot give different accounts of one
/// frame. Same discipline as `reduction_anim::step_summary`.
fn step_summary(frame: &PreLoweringFrame) -> String {
    step_style(frame).2
}

/// Icon, colour and summary. Icons are only ever codepoints this app already
/// renders — egui ships far less than the whole of Unicode, and an unproven one
/// shows as a tofu box.
fn step_style(frame: &PreLoweringFrame) -> (&'static str, egui::Color32, String) {
    match &frame.step {
        PreLoweringStep::Start { equations, .. } => (
            // Clapper board: the start of the take. Deliberately not a
            // checkered flag, which reads as a finish line at the head.
            "\u{1f3ac}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "Starting a pass over {equations} equation{} \u{2014} nothing lowered yet",
                if *equations == 1 { "" } else { "s" },
            ),
        ),
        PreLoweringStep::Discovered { target } => (
            "\u{1f50d}",
            crate::colors::ANIM_EXPLORE,
            format!("Found pre({target}) \u{2014} a reference to {target}'s previous value"),
        ),
        PreLoweringStep::Named { base, slot } => (
            "\u{1f3af}",
            crate::colors::ANIM_PATH_FOUND,
            // The moment the name begins to exist. Phrased to make that the
            // point, because it is the answer to "where did this come from?".
            format!("Named the slot: pre({base}) becomes {slot} \u{2014} a name in no source file"),
        ),
        PreLoweringStep::Materialized {
            slot,
            inherited_from,
        } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            format!(
                "Built {slot} as a PARAMETER, inheriting {inherited_from}'s shape and start value",
            ),
        ),
        PreLoweringStep::Substituted {
            partition,
            equations,
        } => (
            "\u{2702}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "Rewrote {equations} {partition} equation{} to reference the slots",
                if *equations == 1 { "" } else { "s" },
            ),
        ),
        PreLoweringStep::Complete { slots_created } => (
            "\u{2705}",
            crate::colors::ANIM_PATH_FOUND,
            if *slots_created == 0 {
                // Not a failure. This pass runs twice per compile and the second
                // run legitimately finds nothing left to do — a fact about the
                // algorithm that only a replay makes visible.
                "Pass complete \u{2014} no new slots; nothing left to lower".to_owned()
            } else {
                format!(
                    "Pass complete \u{2014} {slots_created} slot{} created",
                    if *slots_created == 1 { "" } else { "s" },
                )
            },
        ),
    }
}

fn render_step(ui: &mut egui::Ui, frame: &PreLoweringFrame) {
    let (icon, color, summary) = step_style(frame);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(icon).size(16.0));
        ui.label(egui::RichText::new(&summary).color(color).strong());
    });
}

/// The slots created so far, growing as the replay advances.
fn render_slots(ui: &mut egui::Ui, frame: &PreLoweringFrame) {
    if frame.slots_so_far.is_empty() {
        ui.weak("No slots created yet.");
        return;
    }
    ui.label(egui::RichText::new("Slots created so far").strong());
    egui::Grid::new("pre_slots_grid")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (i, slot) in frame.slots_so_far.iter().enumerate() {
                ui.label(format!("{}.", i + 1));
                ui.label(egui::RichText::new(slot).monospace());
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(step: PreLoweringStep, slots: &[&str]) -> PreLoweringFrame {
        PreLoweringFrame {
            step,
            slots_so_far: slots.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// Every step renders, and the wording names the thing that matters.
    ///
    /// `Named` is the frame the whole view exists for — it is where the answer
    /// to "where did `__pre__.overSpeed` come from?" lives — so its text is
    /// pinned rather than merely non-empty.
    #[test]
    fn every_step_renders_and_naming_says_what_happened() {
        let named = frame(
            PreLoweringStep::Named {
                base: "overSpeed".into(),
                slot: "__pre__.overSpeed".into(),
            },
            &[],
        );
        let summary = step_summary(&named);
        assert!(summary.contains("pre(overSpeed)"), "{summary}");
        assert!(summary.contains("__pre__.overSpeed"), "{summary}");

        for step in [
            PreLoweringStep::Start {
                pass: 1,
                equations: 51,
            },
            PreLoweringStep::Discovered {
                target: "overSpeed".into(),
            },
            PreLoweringStep::Materialized {
                slot: "__pre__.overSpeed".into(),
                inherited_from: "overSpeed".into(),
            },
            PreLoweringStep::Substituted {
                partition: "condition equations",
                equations: 2,
            },
            PreLoweringStep::Complete { slots_created: 3 },
        ] {
            assert!(!step_summary(&frame(step, &[])).is_empty());
        }
    }

    /// A pass that creates nothing is reported as a result, not an absence.
    ///
    /// The second of the two passes per compile legitimately finds nothing left
    /// to lower. Rendering that as "0 slots created" would read like a failure;
    /// it is a fact about the algorithm, and one only a replay can show.
    #[test]
    fn a_pass_that_creates_nothing_says_so_plainly() {
        let summary = step_summary(&frame(PreLoweringStep::Complete { slots_created: 0 }, &[]));
        assert!(summary.contains("nothing left to lower"), "{summary}");
        assert!(
            !summary.contains('0'),
            "a bare zero reads as failure: {summary}"
        );
    }

    /// The capture carries the same sentence the view draws, plus the running
    /// slot set — which is the content of this phase, growing frame by frame.
    #[test]
    fn the_capture_gets_the_step_and_the_running_slot_set() {
        let anim = PreLoweringAnimation::from_frames(vec![frame(
            PreLoweringStep::Materialized {
                slot: "__pre__.overSpeed".into(),
                inherited_from: "overSpeed".into(),
            },
            &["__pre__.load.w", "__pre__.overSpeed"],
        )]);
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert!(
            ctx["step"]
                .as_str()
                .is_some_and(|s| s.contains("PARAMETER")),
            "{ctx}"
        );
        assert_eq!(
            ctx["slots_so_far"],
            serde_json::json!(["__pre__.load.w", "__pre__.overSpeed"]),
        );
        assert_eq!(anim.which(), "pre_lowering");
    }

    #[test]
    fn an_empty_trace_is_empty() {
        let anim = PreLoweringAnimation::from_frames(Vec::new());
        assert!(anim.is_empty());
        assert!(anim.current_frame_context().is_none());
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }
}
