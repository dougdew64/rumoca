//! Animated alias-elimination stepper — reveals variables being substituted
//! away, one at a time.
//!
//! ## Why this phase is worth animating
//!
//! Connecting two components writes `a.v = b.v`. Do that a dozen times and the
//! DAE is full of equations that say nothing except "these two names are the
//! same value". Each such **alias equation** is an opportunity: delete one of
//! the two variables, replace it with the other everywhere it appears, and
//! delete the equation too. The system shrinks by one unknown *and* one
//! equation, and nothing about its solution changes.
//!
//! That is why a flattened Modelica model with hundreds of connection
//! equations solves a system far smaller than its equation count suggests, and
//! it is the reason a variable you wrote in the source can be missing from the
//! solver's unknown vector — it was aliased away, and its value is recovered
//! afterwards from whatever replaced it.
//!
//! ## What kind of animation this is
//!
//! Unlike the matching, BLT, tearing and `pre()`-lowering views, this one is a
//! **reveal of a recorded list, not a replay of a search**. Rumoca's elimination
//! pass is not a search: it walks the alias equations and substitutes. There is
//! no backtracking, no competing candidate, no decision that could have gone
//! another way — so there is nothing a live trace could show that the report
//! does not already contain, and this view offers no Debug button.
//!
//! Being explicit about that is the point. Not every phase has a hidden
//! process, and pretending otherwise would teach something false. What the
//! stepping *does* buy is the accumulation: watching the unknown count fall one
//! substitution at a time, and reading each replacement as it lands, rather
//! than meeting a finished table of forty rows.

use eframe::egui;

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. Faster than the decision-making views:
/// each frame is one substitution, and they are meant to be read as a stream.
const FRAME_INTERVAL: f64 = 0.45;

/// One elimination: the variable that went away and what took its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasFrame {
    /// The variable that was deleted.
    pub variable: String,
    /// The expression it was replaced by, already abbreviated for display.
    pub replacement: String,
}

/// Reveal of the alias eliminations recorded for a model.
pub struct AliasAnimation {
    playback: Playback<AliasFrame>,
    /// Unknown count before any elimination, so the running state can say what
    /// the system has shrunk *from*. `None` when the report did not carry it.
    unknowns_before: Option<usize>,
    /// Eliminations the report listed that this parser could not read. Rendered at
    /// the top of the pane; see `from_report` for why a dropped one corrupts
    /// `unknowns_before` as well as the frame list.
    problems: Vec<String>,
}

impl AliasAnimation {
    /// Build from a structural report's `reduction.eliminations`.
    ///
    /// Reads the JSON rather than taking `ReductionView`'s parsed rows: that
    /// type keeps its fields private and this view wants the raw replacement
    /// text, not the abbreviated one, when the abbreviation would hide the
    /// substitution being made.
    pub fn from_report(report: &serde_json::Value) -> Option<Self> {
        let red = report.get("reduction")?;
        // **A dropped elimination is not just a missing frame.** `unknowns_before`
        // below is computed as `n_unknowns + frames.len()`, so an entry this parser
        // could not read **understates the size of the original system** — the
        // animation would then narrate removing three variables from a system that
        // it also reports as one variable smaller than it was. Silent until the
        // 2026-08-04 sweep.
        //
        // `eliminations` is required here (the `?` below) rather than optional as in
        // `reduction_view`, because an alias animation with no eliminations has
        // nothing to animate at all.
        red.get("eliminations")?.as_array()?;
        let mut problems = Vec::new();
        let frames: Vec<AliasFrame> =
            crate::json_read::parse_list(red, "eliminations", &mut problems, |e| {
                Some(AliasFrame {
                    variable: e.get("variable")?.as_str()?.to_owned(),
                    replacement: crate::reduction_view::abbreviate_expr(
                        e.get("replacement")?.as_str()?,
                    ),
                })
            });
        let unknowns_before = report
            .get("n_unknowns")
            .and_then(serde_json::Value::as_u64)
            .map(
                // The report's count is the system *after* elimination, so the
                // starting size is that plus the variables this pass removed.
                |n| n as usize + frames.len(),
            );
        Some(Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
            unknowns_before,
            problems,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the controls and the reveal. No Debug button: see the module note
    /// on why this phase has no live trace to offer.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        // Above everything, including the "no eliminations" message below — which
        // would otherwise be flatly false when the eliminations existed and this
        // parser could not read them.
        for p in &self.problems {
            ui.colored_label(ui.visuals().error_fg_color, format!("\u{26a0} {p}"));
        }
        if self.playback.is_empty() {
            ui.label("No alias eliminations in this model.");
            ui.weak(
                "Nothing was substituted away \u{2014} either the model has no connection \
                 equations, or none of them turned out to be simple aliases.",
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
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new("\u{2702}").size(16.0));
            ui.label(
                egui::RichText::new(step_summary(frame))
                    .color(crate::colors::ANIM_PATH_FOUND)
                    .strong(),
            );
        });
    }

    /// Goal line plus the shrinking system, then the substitutions so far.
    fn render_running_state(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "Goal: every alias equation `a = b` lets one variable be deleted and replaced by \
                 the other everywhere \u{2014} one fewer unknown and one fewer equation, with the \
                 same solution.",
            )
            .italics()
            .color(crate::colors::ANIM_EXPLORE),
        );

        let (done, total) = self.playback.position();
        let done = done + 1; // position() is 0-based; the cursor frame is done.
        ui.add_space(4.0);
        ui.label(match self.unknowns_before {
            Some(before) => format!(
                "{done} of {total} eliminated \u{2014} system down from {before} to {} unknowns",
                before - done,
            ),
            None => format!("{done} of {total} eliminated"),
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Substitutions so far").strong());
        // **No inner scroll area, and no height cap.** `App::alias_anim_ui`
        // already wraps this whole view in a vertical scroll area, so a second
        // one nested inside it did two harmful things and no useful one: it
        // capped the list at 320pt — about 16–18 rows — and it captured the
        // mouse wheel, so scrolling over the list moved the list instead of the
        // page.
        //
        // `Drivetrain` has **77** alias eliminations, so under a quarter of them
        // were reachable, inside a small box while the pane around it stayed
        // empty. That specimen is the index-reduction tour's centrepiece.
        //
        // The parent scrolls; this view just renders. A tall model makes a tall
        // pane, which is the honest result. Held by
        // `playback::tests_layout::a_view_inside_a_scrolling_pane_does_not_scroll_or_cap_itself`.
        egui::Grid::new("alias_elim_grid")
            .num_columns(3)
            .spacing([10.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                for (i, f) in self.playback.frames().iter().take(done).enumerate() {
                    ui.label(format!("{}.", i + 1));
                    ui.label(egui::RichText::new(&f.variable).monospace());
                    ui.label(
                        egui::RichText::new(format!("\u{2192} {}", f.replacement)).monospace(),
                    );
                    ui.end_row();
                }
            });
    }
}

impl Animated for AliasAnimation {
    fn which(&self) -> &'static str {
        "alias_elimination"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, _arming: bool) -> crate::LiveState {
        // This view never runs live — the phase has no search to trace.
        crate::LiveState::Idle
    }

    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let frame = self.playback.current()?;
        let (done, total) = self.playback.position();
        Some(serde_json::json!({
            "step": step_summary(frame),
            "variable": frame.variable,
            "replacement": frame.replacement,
            "eliminated_so_far": done + 1,
            "eliminations_total": total,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

/// The one-line description of a step — **shared by the view and the capture**,
/// so the screen and the emitted context cannot give different accounts.
fn step_summary(frame: &AliasFrame) -> String {
    format!(
        "Eliminated {} \u{2014} every use of it becomes {}",
        frame.variable, frame.replacement,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(elims: &[(&str, &str)], n_unknowns: u64) -> serde_json::Value {
        serde_json::json!({
            "n_unknowns": n_unknowns,
            "reduction": {
                "eliminations": elims.iter()
                    .map(|(v, r)| serde_json::json!({"variable": v, "replacement": r}))
                    .collect::<Vec<_>>(),
            },
        })
    }

    #[test]
    fn the_summary_names_both_sides_of_the_substitution() {
        let s = step_summary(&AliasFrame {
            variable: "r1.p.v".into(),
            replacement: "src.n.v".into(),
        });
        assert!(s.contains("r1.p.v") && s.contains("src.n.v"), "{s}");
    }

    /// The report's unknown count is the system *after* elimination, so the
    /// starting size has to be reconstructed. Getting this backwards would show
    /// the system growing.
    #[test]
    fn the_running_count_reconstructs_the_starting_size() {
        let anim = AliasAnimation::from_report(&report(&[("a", "b"), ("c", "d"), ("e", "f")], 20))
            .expect("a report with eliminations parses");
        assert_eq!(anim.unknowns_before, Some(23), "20 left + 3 removed");
        assert_eq!(anim.position(), (0, 3));
    }

    #[test]
    fn the_capture_carries_the_substitution_and_the_progress() {
        let anim = AliasAnimation::from_report(&report(&[("r1.p.v", "src.n.v")], 5))
            .expect("a report with eliminations parses");
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["variable"], "r1.p.v");
        assert_eq!(ctx["replacement"], "src.n.v");
        assert_eq!(ctx["eliminated_so_far"], 1);
        assert_eq!(ctx["eliminations_total"], 1);
        assert_eq!(anim.which(), "alias_elimination");
    }

    /// A model that eliminated nothing is a legitimate outcome, not an error.
    #[test]
    fn a_model_with_no_eliminations_is_empty() {
        let anim = AliasAnimation::from_report(&report(&[], 12))
            .expect("an empty eliminations array still parses");
        assert!(anim.is_empty());
        assert!(anim.current_frame_context().is_none());
    }

    /// A report with no `reduction` section at all (the Structural tab) yields
    /// no view rather than an empty one, so the caller can hide the tab.
    #[test]
    fn a_report_without_a_reduction_section_yields_nothing() {
        assert!(AliasAnimation::from_report(&serde_json::json!({"n_unknowns": 3})).is_none());
    }

    /// This view never claims to be live, whatever the arming state says.
    #[test]
    fn the_view_is_never_live() {
        let anim = AliasAnimation::from_report(&report(&[("a", "b")], 3)).unwrap();
        assert_eq!(anim.live_state(true), crate::LiveState::Idle);
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }
}
