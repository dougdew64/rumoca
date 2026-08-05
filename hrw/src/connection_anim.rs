//! Animated connection-expansion stepper — watches `connect()` statements
//! become equations (MLS §9).
//!
//! ## Why this phase is worth animating
//!
//! Open the Flatten tab on any component-based model and the equation count is
//! several times what anyone wrote. Connection expansion is where most of the
//! difference comes from, and the finished flat model does not explain the
//! rule that produced it. The rule is short and asymmetric:
//!
//! - a **potential** (ordinary) set of *n* connected variables becomes *n − 1*
//!   equality equations — `v1 = v2 = … = vn` written as a chain;
//! - a **flow** set of the same *n* becomes exactly **one** equation, the
//!   sum-to-zero. This is Kirchhoff's current law, generalised: whatever flows
//!   into a junction flows out of it.
//!
//! Seeing three variables produce two equations on one line and one equation on
//! the next is the single most useful thing this view does. It is also where
//! the sign convention lives (inside connector +1, outside −1), and why a
//! model's unknown count and equation count stay balanced no matter how many
//! components you wire together.
//!
//! The second thing the frames show is that connection sets are **transitive**.
//! `connect(a, b)` and `connect(b, c)` do not make two sets of two; they make
//! one set of three, because Rumoca builds the sets with union-find. A reader
//! of the flat model sees the consequence (two equality equations, not two
//! separate pairs) without the cause.
//!
//! ## Recorded only, and why
//!
//! Unlike the matching, BLT, tearing and `pre()`-lowering replays this view has
//! **no Debug button**, and the reason is plumbing rather than principle: the
//! phase *is* instrumented for a live trace
//! (`flatten_ref_with_options_traced`), but re-running it needs the resolved
//! `ClassTree` and the instance overlay, and the tree contains the whole of the
//! MSL. Shipping that to the UI thread to arm a breakpoint is a bigger change
//! than this view is worth on its own; a worker-side live-debug path would be
//! the right fix. Recorded playback is complete and faithful in the meantime —
//! the worker re-runs flatten with an observer attached at compile time, so
//! these frames come from a real run of the real pass.

use eframe::egui;

use rumoca_phase_flatten::connections::trace::{ConnectionFrame, ConnectionStep};

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. A set and its equations are two frames
/// that belong together, so the pace is brisk enough to read them as a pair.
const FRAME_INTERVAL: f64 = 0.5;

/// Replay of connection expansion.
pub struct ConnectionAnimation {
    playback: Playback<ConnectionFrame>,
}

impl ConnectionAnimation {
    /// Build from frames recorded during compilation.
    pub fn from_frames(frames: Vec<ConnectionFrame>) -> Self {
        Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the controls, the step line, and the running state.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.playback.is_empty() {
            ui.label("No connections in this model.");
            ui.weak(
                "Nothing to expand \u{2014} every equation in the flat model was written by hand, \
                 not generated from a connect().",
            );
            return;
        }

        let live = crate::LiveState::Idle;
        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }
        let _ = crate::animation_controls(ui, self.playback.controls(), live, false);

        ui.add_space(4.0);
        self.render_current(ui);
        ui.add_space(8.0);
        self.render_running_state(ui);
    }

    fn render_current(&self, ui: &mut egui::Ui) {
        let Some(frame) = self.playback.current() else {
            return;
        };
        let (icon, color, summary) = step_style(frame);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0));
            ui.label(egui::RichText::new(summary).color(color).strong());
        });

        // A set's membership is the evidence for the equation count on the next
        // frame, so it is shown in full rather than summarised.
        if let ConnectionStep::SetFormed { variables, .. } = &frame.step
            && !variables.is_empty()
        {
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(200.0)
                .show(ui, |ui| {
                    for v in variables {
                        ui.label(egui::RichText::new(v).monospace());
                    }
                });
        }
    }

    /// Goal line plus the two running totals: sets closed, equations made.
    fn render_running_state(&self, ui: &mut egui::Ui) {
        let Some(frame) = self.playback.current() else {
            return;
        };
        ui.label(
            egui::RichText::new(
                "Goal: turn every connect() into equations \u{2014} equal potentials, and flows \
                 that sum to zero. This is where a flat model gets most of its equations.",
            )
            .italics()
            .color(crate::colors::ANIM_EXPLORE),
        );
        ui.add_space(4.0);
        ui.label(format!(
            "{} connection set{} closed \u{2014} {} equation{} generated so far",
            frame.sets_so_far,
            if frame.sets_so_far == 1 { "" } else { "s" },
            frame.equations_so_far,
            if frame.equations_so_far == 1 { "" } else { "s" },
        ));
    }
}

impl Animated for ConnectionAnimation {
    fn which(&self) -> &'static str {
        "connection_expansion"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, _arming: bool) -> crate::LiveState {
        // Recorded only — see the module note on why there is no live path yet.
        crate::LiveState::Idle
    }

    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        let mut ctx = serde_json::json!({
            "step": step_summary(frame),
            "sets_so_far": frame.sets_so_far,
            "equations_so_far": frame.equations_so_far,
        });
        // The set's members are what make the next frame's equation count
        // interpretable, so the capture carries them when they exist.
        if let ConnectionStep::SetFormed {
            variables,
            kind,
            scope,
        } = &frame.step
        {
            let obj = ctx.as_object_mut().expect("built as an object");
            obj.insert("set_kind".to_owned(), serde_json::json!(kind));
            obj.insert("set_scope".to_owned(), serde_json::json!(scope));
            obj.insert("set_variables".to_owned(), serde_json::json!(variables));
        }
        Some(ctx)
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

/// The one-line description of a step — **shared by the view and the capture**,
/// so the screen and the emitted context cannot give different accounts.
fn step_summary(frame: &ConnectionFrame) -> String {
    step_style(frame).2
}

/// Icon, colour and summary. Icons are only ever codepoints this app already
/// renders elsewhere.
fn step_style(frame: &ConnectionFrame) -> (&'static str, egui::Color32, String) {
    match &frame.step {
        ConnectionStep::Start { connect_statements } => (
            "\u{1f3ac}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "Expanding {connect_statements} connect() statement{} into equations",
                if *connect_statements == 1 { "" } else { "s" },
            ),
        ),
        ConnectionStep::SetFormed {
            kind,
            scope,
            variables,
        } => (
            "\u{1f50d}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "A {kind} set of {}{}: these variables are all connected to one another{}",
                variables.len(),
                if scope.is_empty() {
                    String::new()
                } else {
                    format!(" at {scope}")
                },
                // Transitivity is the surprise, and it is only visible when a
                // set is bigger than the pair someone actually wrote.
                if variables.len() > 2 {
                    " \u{2014} more than any single connect() named, because connection sets are \
                     transitive"
                } else {
                    ""
                },
            ),
        ),
        ConnectionStep::EquationsGenerated {
            kind,
            set_size,
            equations_added,
        } => {
            let why = match *kind {
                // The two halves of MLS §9.2, each said where it applies.
                "potential" => " (n-1 equalities chain n variables together)",
                "flow" => " (one sum-to-zero equation \u{2014} Kirchhoff's current law)",
                "stream" => " (stream variables carry no ordinary equation; MLS §15)",
                _ => "",
            };
            (
                "\u{2b07}",
                crate::colors::MATCHED_MARKER,
                format!(
                    "{set_size} {kind} variables \u{2192} {equations_added} equation{}{why}",
                    if *equations_added == 1 { "" } else { "s" },
                ),
            )
        }
        ConnectionStep::UnconnectedFlow { equations_added } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            if *equations_added == 0 {
                "Every flow variable is connected \u{2014} no zero-flow equations needed".to_owned()
            } else {
                format!(
                    "{equations_added} unconnected flow variable{} set to zero \u{2014} a port \
                     wired to nothing carries nothing (MLS \u{00a7}9.2)",
                    if *equations_added == 1 { "" } else { "s" },
                )
            },
        ),
        ConnectionStep::Complete {
            sets,
            equations_added,
        } => (
            "\u{2705}",
            crate::colors::ANIM_PATH_FOUND,
            format!(
                "Done: {sets} connection set{} produced {equations_added} equation{}",
                if *sets == 1 { "" } else { "s" },
                if *equations_added == 1 { "" } else { "s" },
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(step: ConnectionStep, sets: usize, eqs: usize) -> ConnectionFrame {
        ConnectionFrame {
            step,
            sets_so_far: sets,
            equations_so_far: eqs,
        }
    }

    /// The asymmetry is the reason this view exists, so both halves must reach
    /// the screen with their explanation attached — the counts alone would
    /// leave the reader to guess the rule.
    #[test]
    fn the_potential_flow_asymmetry_is_explained_not_just_counted() {
        let potential = frame(
            ConnectionStep::EquationsGenerated {
                kind: "potential",
                set_size: 3,
                equations_added: 2,
            },
            1,
            2,
        );
        let s = step_summary(&potential);
        assert!(
            s.contains("3 potential") && s.contains("2 equations"),
            "{s}"
        );
        assert!(
            s.contains("n-1"),
            "the rule must be stated, not just the count: {s}"
        );

        let flow = frame(
            ConnectionStep::EquationsGenerated {
                kind: "flow",
                set_size: 3,
                equations_added: 1,
            },
            2,
            3,
        );
        let s = step_summary(&flow);
        assert!(s.contains("3 flow") && s.contains("1 equation"), "{s}");
        assert!(s.contains("Kirchhoff"), "{s}");
    }

    /// Transitivity is only remarkable when the set is bigger than the pair
    /// someone wrote, so the explanation appears there and not on every set.
    #[test]
    fn transitivity_is_called_out_only_when_it_shows() {
        let three = frame(
            ConnectionStep::SetFormed {
                kind: "potential",
                scope: String::new(),
                variables: vec!["a.v".into(), "b.v".into(), "c.v".into()],
            },
            0,
            0,
        );
        assert!(
            step_summary(&three).contains("transitive"),
            "{}",
            step_summary(&three)
        );

        let two = frame(
            ConnectionStep::SetFormed {
                kind: "potential",
                scope: String::new(),
                variables: vec!["a.v".into(), "b.v".into()],
            },
            0,
            0,
        );
        assert!(
            !step_summary(&two).contains("transitive"),
            "{}",
            step_summary(&two)
        );
    }

    /// Zero unconnected flows is a result, not an absence — rendering "0 flow
    /// variables set to zero" would read like something failed.
    #[test]
    fn no_unconnected_flows_reads_as_a_result() {
        let s = step_summary(&frame(
            ConnectionStep::UnconnectedFlow { equations_added: 0 },
            2,
            3,
        ));
        assert!(s.contains("Every flow variable is connected"), "{s}");
    }

    #[test]
    fn every_step_renders() {
        for step in [
            ConnectionStep::Start {
                connect_statements: 4,
            },
            ConnectionStep::SetFormed {
                kind: "stream",
                scope: "sub".into(),
                variables: vec!["a.h".into()],
            },
            ConnectionStep::EquationsGenerated {
                kind: "stream",
                set_size: 2,
                equations_added: 0,
            },
            ConnectionStep::UnconnectedFlow { equations_added: 2 },
            ConnectionStep::Complete {
                sets: 3,
                equations_added: 7,
            },
        ] {
            assert!(!step_summary(&frame(step, 0, 0)).is_empty());
        }
    }

    /// The capture carries the sentence on screen, both running totals, and —
    /// on a set frame — the membership that makes the next frame's count
    /// interpretable.
    #[test]
    fn the_capture_carries_the_set_and_the_running_totals() {
        let anim = ConnectionAnimation::from_frames(vec![frame(
            ConnectionStep::SetFormed {
                kind: "flow",
                scope: String::new(),
                variables: vec!["a.i".into(), "b.i".into(), "c.i".into()],
            },
            1,
            2,
        )]);
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["set_kind"], "flow");
        assert_eq!(
            ctx["set_variables"],
            serde_json::json!(["a.i", "b.i", "c.i"])
        );
        assert_eq!(ctx["sets_so_far"], 1);
        assert_eq!(ctx["equations_so_far"], 2);
        assert_eq!(anim.which(), "connection_expansion");
    }

    /// A frame that is not a set carries no membership rather than an empty
    /// list — an empty list would read as "a set with no variables".
    #[test]
    fn a_non_set_frame_carries_no_membership() {
        let anim = ConnectionAnimation::from_frames(vec![frame(
            ConnectionStep::Complete {
                sets: 2,
                equations_added: 5,
            },
            2,
            5,
        )]);
        let ctx = anim.current_frame_context().unwrap();
        assert!(ctx.get("set_variables").is_none(), "{ctx}");
    }

    #[test]
    fn a_model_with_no_connections_is_empty() {
        let anim = ConnectionAnimation::from_frames(Vec::new());
        assert!(anim.is_empty());
        assert!(anim.current_frame_context().is_none());
        assert_eq!(anim.live_state(true), crate::LiveState::Idle);
    }
}
