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

/// One step of the reveal: the opening state, or one elimination.
///
/// **`Start` is the opening frame — nothing eliminated, nothing attempted.**
/// Added 2026-08-23 after Doug reported that this view "opens to frame 1 with
/// progress already having been made". It did: the frame list was the
/// eliminations themselves, so frame 1 showed one substitution *already
/// applied*, and there was no state describing the system before the pass.
///
/// That is the same defect `09634b15` fixed for index reduction a month earlier
/// — *"nothing in the trace described the system before reduction, which is the
/// one thing a replay needs in order to show what reduction changed"* — and the
/// four views built on Rumoca capture scopes all carry a `Start` step for it.
/// This view and `ic_plan_anim` parse a report *list* rather than a capture, so
/// they never got one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AliasStep {
    /// Before any substitution. Carries nothing: what it needs to say is the
    /// unknown count, which the animation already holds.
    Start,
    /// One variable substituted away.
    Eliminated(AliasFrame),
}

/// Reveal of the alias eliminations recorded for a model.
pub struct AliasAnimation {
    /// `[Start, Eliminated(0), …, Eliminated(n-1)]` — **one longer than the
    /// elimination count**, so anything reporting a total must subtract the
    /// opening frame.
    playback: Playback<AliasStep>,
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
        let eliminations: Vec<AliasFrame> =
            crate::json_read::parse_list(red, "eliminations", &mut problems, |e| {
                Some(AliasFrame {
                    variable: e.get("variable")?.as_str()?.to_owned(),
                    replacement: crate::reduction_view::abbreviate_expr(
                        e.get("replacement")?.as_str()?,
                    ),
                })
            });
        // **Captured, not computed** — `before.n_unknowns` is the size of the
        // system as the reduction stage actually found it, recorded during the
        // run. HRW used to derive this as `n_unknowns + eliminations.len()`,
        // reasoning that each alias elimination removes exactly one unknown.
        //
        // That reasoning was sound and the two agree on every specimen with a
        // committed trace (2026-08-23: Drivetrain 97, MotorWithBrake and
        // BenchActuator 48, GearWithBrake 44, RcCircuit and OverInitRc 23,
        // SingleInertia 2) — but it was *arithmetic HRW did*, and it understated
        // the starting size whenever an elimination failed to parse. The
        // captured number cannot: it was measured before the pass ran.
        // `the_starting_size_is_read_from_the_run_not_reconstructed` holds them
        // to each other.
        //
        // The derivation survives only as a fallback for a report with no
        // `before` block, and its old failure mode is still announced through
        // `problems`.
        let unknowns_before = report
            .get("before")
            .and_then(|b| b.get("n_unknowns"))
            .and_then(serde_json::Value::as_u64)
            .map(|n| n as usize)
            .or_else(|| {
                report
                    .get("n_unknowns")
                    .and_then(serde_json::Value::as_u64)
                    .map(|n| n as usize + eliminations.len())
            });

        // **No opening frame when there is nothing to open.** A lone `Start`
        // would make `is_empty()` false for a model with no eliminations, and
        // the pane would offer a replay of nothing instead of saying so.
        let frames: Vec<AliasStep> = if eliminations.is_empty() {
            Vec::new()
        } else {
            std::iter::once(AliasStep::Start)
                .chain(eliminations.into_iter().map(AliasStep::Eliminated))
                .collect()
        };

        Some(Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
            unknowns_before,
            problems,
        })
    }

    /// Eliminations recorded, which is one fewer than the frame count.
    fn n_eliminations(&self) -> usize {
        self.playback.position().1.saturating_sub(1)
    }

    /// What the opening frame says. The system as the pass finds it.
    fn start_summary(&self) -> String {
        match self.unknowns_before {
            Some(before) => format!(
                "Starting point: {before} unknowns, {} alias equation{} to substitute away \
                 \u{2014} nothing eliminated yet",
                self.n_eliminations(),
                if self.n_eliminations() == 1 { "" } else { "s" },
            ),
            None => format!(
                "Starting point: {} alias equation{} to substitute away \u{2014} nothing \
                 eliminated yet",
                self.n_eliminations(),
                if self.n_eliminations() == 1 { "" } else { "s" },
            ),
        }
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
        let Some(step) = self.playback.current() else {
            return;
        };
        // Clapper board and `ANIM_EXPLORE` on the opening frame, matching the
        // four capture-driven views. `34c22d56`: a start icon, not a finish
        // flag — nothing at the head of a replay may read as an ending.
        let (icon, color, summary) = match step {
            AliasStep::Start => (
                "\u{1f3ac}",
                crate::colors::ANIM_EXPLORE,
                self.start_summary(),
            ),
            AliasStep::Eliminated(frame) => (
                "\u{2702}",
                crate::colors::ANIM_PATH_FOUND,
                step_summary(frame),
            ),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0));
            ui.label(egui::RichText::new(summary).color(color).strong());
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

        // **The cursor IS the count of eliminations applied**, because frame 0
        // is the opening state: at the opening frame none have been applied, and
        // at frame k exactly k have. This used to read `cursor + 1`, which is
        // what made the view open claiming one substitution had already
        // happened.
        let done = self.playback.cursor();
        let total = self.n_eliminations();
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
                // `skip(1)` steps over the opening frame, which is a state
                // rather than a substitution and has no row to contribute.
                for (i, step) in self.playback.frames().iter().skip(1).take(done).enumerate() {
                    let AliasStep::Eliminated(f) = step else {
                        continue;
                    };
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
        let step = self.playback.current()?;
        // `eliminations_total` counts substitutions, so it excludes the opening
        // frame — a capture saying "77 of 78" would be describing a list that
        // does not exist.
        let total = self.n_eliminations();
        Some(match step {
            AliasStep::Start => serde_json::json!({
                "step": self.start_summary(),
                "eliminated_so_far": 0,
                "eliminations_total": total,
            }),
            AliasStep::Eliminated(frame) => serde_json::json!({
                "step": step_summary(frame),
                "variable": frame.variable,
                "replacement": frame.replacement,
                "eliminated_so_far": self.playback.cursor(),
                "eliminations_total": total,
            }),
        })
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

    /// **The fallback path**, for a report carrying no `before` block: the
    /// report's unknown count is the system *after* elimination, so the starting
    /// size is reconstructed. Getting this backwards would show the system
    /// growing.
    ///
    /// A real compile takes the captured path instead — see
    /// [`tests::the_starting_size_is_read_from_the_run_not_reconstructed`], which
    /// also proves the two agree.
    #[test]
    fn the_running_count_reconstructs_the_starting_size() {
        let anim = AliasAnimation::from_report(&report(&[("a", "b"), ("c", "d"), ("e", "f")], 20))
            .expect("a report with eliminations parses");
        assert_eq!(anim.unknowns_before, Some(23), "20 left + 3 removed");
        assert_eq!(
            anim.position(),
            (0, 4),
            "three eliminations plus the opening frame"
        );
        assert_eq!(anim.n_eliminations(), 3, "the opening frame is not one");
    }

    /// **The replay opens before anything has happened.**
    ///
    /// Doug, 2026-08-23: this view *"is opening to frame 1, with progress
    /// already having been made"*. It was: the frame list was the eliminations
    /// themselves, so the first frame showed one substitution already applied
    /// and nothing described the system beforehand.
    ///
    /// The regression guard is the pair — the opening frame reports **zero**
    /// eliminated, and the one after it reports one. Asserting only the first
    /// would pass on a view that never advanced.
    #[test]
    fn the_replay_opens_before_anything_has_been_eliminated() {
        let mut anim = AliasAnimation::from_report(&report(&[("a", "b"), ("c", "d")], 8))
            .expect("a report with eliminations parses");

        assert_eq!(anim.playback.current(), Some(&AliasStep::Start));
        let opening = anim.current_frame_context().expect("an opening frame");
        assert_eq!(opening["eliminated_so_far"], 0);
        assert_eq!(opening["eliminations_total"], 2);
        assert!(
            opening["step"].as_str().is_some_and(|s| s.contains("10")),
            "the opening frame states the system it found: 8 left + 2 removed = 10, got {}",
            opening["step"]
        );
        assert!(
            opening["variable"].is_null(),
            "the opening frame substitutes nothing, so it must claim no variable"
        );

        assert!(anim.seek(1), "the first substitution is frame 1");
        let first = anim.current_frame_context().expect("a frame");
        assert_eq!(first["eliminated_so_far"], 1);
        assert_eq!(first["variable"], "a");
    }

    #[test]
    fn the_capture_carries_the_substitution_and_the_progress() {
        let mut anim = AliasAnimation::from_report(&report(&[("r1.p.v", "src.n.v")], 5))
            .expect("a report with eliminations parses");
        assert!(
            anim.seek(1),
            "step past the opening frame to the elimination"
        );
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["variable"], "r1.p.v");
        assert_eq!(ctx["replacement"], "src.n.v");
        assert_eq!(ctx["eliminated_so_far"], 1);
        assert_eq!(ctx["eliminations_total"], 1);
        assert_eq!(anim.which(), "alias_elimination");
    }

    /// **The starting size is read from the run, not reconstructed from it.**
    ///
    /// The opening frame states the system before the pass, and that number used
    /// to be HRW's own arithmetic: the report's after-count plus the number of
    /// eliminations parsed. Sound reasoning — each alias elimination removes one
    /// unknown — but arithmetic that understated the starting size whenever an
    /// elimination failed to parse, and the reader had no way to tell.
    ///
    /// The reduction stage records `before.n_unknowns` during the real run, so
    /// the view reads that instead. This test is what keeps the two honest: it
    /// checks the captured number against the derivation that used to stand in
    /// for it, on a real compile. **A disagreement means one of them is lying**,
    /// and this fails naming both.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn the_starting_size_is_read_from_the_run_not_reconstructed() {
        let crate::worker::FromWorker::Compiled { stages, .. } =
            crate::worker::test_msl::compile_specimen_shared("Drivetrain")
        else {
            panic!("expected Compiled");
        };
        let report = stages
            .index_reduction
            .value
            .as_ref()
            .expect("Drivetrain reaches index reduction");

        let anim = AliasAnimation::from_report(report).expect("Drivetrain has alias eliminations");
        let captured = report["before"]["n_unknowns"]
            .as_u64()
            .expect("the reduction stage records the size it found")
            as usize;
        let after = report["n_unknowns"]
            .as_u64()
            .expect("and the size it produced") as usize;

        assert_eq!(
            anim.unknowns_before,
            Some(captured),
            "the view must show the captured starting size, not a reconstruction"
        );
        assert_eq!(
            captured,
            after + anim.n_eliminations(),
            "the captured starting size and the old derivation must agree: {captured} \
             captured, {after} after + {} eliminated",
            anim.n_eliminations(),
        );
    }

    /// **Every frame describes itself, including the opening one.**
    ///
    /// The behavioural half of
    /// `playback::tests_animated_contract::every_animation_context_names_the_step_on_screen`,
    /// which reads the source and so cannot see that this view builds its context in
    /// **two branches**. The opening frame carries no substitution, so it takes the
    /// other arm — and an arm that forgot `step` would put an empty answer in the
    /// capture rather than a missing one.
    #[test]
    fn every_frame_describes_itself() {
        let mut anim = AliasAnimation::from_report(&report(&[("a", "b"), ("c", "d")], 8))
            .expect("a report with eliminations parses");

        for frame in 0..2 {
            assert!(anim.seek(frame), "frame {frame} exists");
            let ctx = anim
                .current_frame_context()
                .expect("a frame is under the cursor");
            assert!(
                ctx["step"].as_str().is_some_and(|s| !s.trim().is_empty()),
                "frame {frame} put no readable `step` in the capture: {ctx}",
            );
        }
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
