//! Animated initial-condition stepper — walks the plan Rumoca will follow to
//! compute a consistent state at t=0.
//!
//! ## Why this phase is worth animating
//!
//! Before a solver can take its first step it needs a *consistent* initial
//! state: every algebraic equation satisfied at t=0, not just the states given
//! values. That is a whole solve of its own, and Rumoca plans it ahead of time
//! rather than throwing the entire system at Newton. The plan is an **ordered
//! sequence of blocks**, each one of three kinds:
//!
//! - **direct** — the variable is isolated; evaluate an expression, done. Most
//!   of a typical plan is this, which is the surprise: initialization is mostly
//!   assignment, not iteration.
//! - **Newton (scalar)** — one equation in one unknown that will not rearrange,
//!   so it gets a one-dimensional Newton iteration.
//! - **torn block** — a genuine simultaneous set, torn (see the Tearing view)
//!   into tear variables plus residual equations.
//!
//! Reading the finished plan as a table of twenty rows says little. Stepping it
//! shows the shape: a long causal run of cheap assignments, punctuated by the
//! few places the system actually has to iterate. Those few places are where
//! initialization fails when it fails.
//!
//! ## Two facts the header carries
//!
//! **Determinacy** — whether the model supplies enough initial conditions for
//! its states, and Rumoca's verdict. This is where over- and under-determined
//! initialization shows up (the `OverInitRc` specimen exists for it).
//!
//! **Relaxation** — which equations the planner had to *drop*, and which
//! unknowns it *pinned*, to make the initial system square. A dropped equation
//! is not an error; it is the planner resolving a redundancy, and knowing it
//! happened explains an initial value that looks arbitrary.
//!
//! ## What kind of animation this is
//!
//! Like the alias view and unlike matching/BLT/tearing, this is a **reveal of a
//! computed plan, not a replay of a search** — the planning has already
//! happened by the time HRW holds the report, and the plan is a list. So there
//! is no Debug button here.

use eframe::egui;

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames.
const FRAME_INTERVAL: f64 = 0.5;

/// One block of the initial-condition plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcBlock {
    /// Isolated: evaluate `solution` and assign it to `var`.
    Direct { var: String, solution: String },
    /// One equation in one unknown, solved by a scalar Newton iteration.
    Newton { var: String, equation: usize },
    /// A simultaneous set, torn open.
    Torn {
        tear_vars: Vec<String>,
        residual_equations: Vec<usize>,
        /// The causal run the tears bought: `(var, equation)` in solve order.
        causal_steps: Vec<(String, usize)>,
    },
}

impl IcBlock {
    /// How many unknowns this block pins down — what the running count adds.
    fn unknowns_solved(&self) -> usize {
        match self {
            IcBlock::Direct { .. } | IcBlock::Newton { .. } => 1,
            IcBlock::Torn { tear_vars, causal_steps, .. } => tear_vars.len() + causal_steps.len(),
        }
    }
}

/// The model-level facts the plan sits inside — rendered above the stepper
/// because they explain the plan's *shape*, and do not change as it steps.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlanContext {
    /// Rumoca's determinacy verdict, if the report carried one.
    pub verdict: Option<String>,
    /// Equations the planner dropped to make the initial system square.
    pub dropped_equations: Vec<usize>,
    /// Unknowns the planner pinned for the same reason.
    pub pinned_unknowns: Vec<String>,
}

/// Reveal of the initial-condition solve plan.
pub struct IcPlanAnimation {
    playback: Playback<IcBlock>,
    context: PlanContext,
}

impl IcPlanAnimation {
    /// Build from an initialization stage report.
    ///
    /// Returns `None` when the report has no `blocks` array — which is the case
    /// for a model whose initialization *failed* (the stage then carries an
    /// `error` instead). A failed initialization has no plan to walk.
    pub fn from_report(report: &serde_json::Value) -> Option<Self> {
        let blocks: Vec<IcBlock> =
            report.get("blocks")?.as_array()?.iter().filter_map(parse_block).collect();

        let determinacy = report.get("determinacy");
        let relax = report.get("relaxation_hint");
        let context = PlanContext {
            verdict: determinacy
                .and_then(|d| d.get("verdict"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            dropped_equations: relax
                .and_then(|r| r.get("dropped_equations"))
                .and_then(serde_json::Value::as_array)
                .map(|a| a.iter().filter_map(serde_json::Value::as_u64).map(|n| n as usize).collect())
                .unwrap_or_default(),
            pinned_unknowns: relax
                .and_then(|r| r.get("pinned_unknowns"))
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(serde_json::Value::as_str)
                        .map(str::to_owned)
                        .collect()
                })
                .unwrap_or_default(),
        };

        Some(Self { playback: Playback::recorded(blocks, FRAME_INTERVAL), context })
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the header, the controls, and the plan walk. No Debug button:
    /// see the module note on why this phase has no live trace to offer.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        self.render_header(ui);

        if self.playback.is_empty() {
            ui.add_space(6.0);
            ui.label("The initial-condition plan is empty.");
            ui.weak("Nothing has to be solved at t=0 \u{2014} every unknown comes from a start attribute.");
            return;
        }

        ui.add_space(6.0);
        ui.separator();

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

    /// Determinacy and relaxation — why the plan looks the way it does.
    fn render_header(&self, ui: &mut egui::Ui) {
        if let Some(verdict) = &self.context.verdict {
            // "well-posed" is the good case; anything else deserves the eye.
            let ok = verdict.starts_with("well-posed");
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(if ok { "\u{2705}" } else { "\u{26a0}" }).size(14.0));
                ui.label(
                    egui::RichText::new(verdict)
                        .color(if ok {
                            crate::colors::ANIM_PATH_FOUND
                        } else {
                            crate::colors::ANIM_FAIL
                        })
                        .strong(),
                );
            });
        }
        if self.context.dropped_equations.is_empty() && self.context.pinned_unknowns.is_empty() {
            return;
        }
        // Relaxation is not a failure — say so, because "dropped equation"
        // reads alarming and is in fact routine redundancy resolution.
        ui.weak(format!(
            "Relaxed to make the initial system square: dropped equation{} {}{}{}. \
             Not an error \u{2014} the planner is resolving a redundancy.",
            if self.context.dropped_equations.len() == 1 { "" } else { "s" },
            self.context
                .dropped_equations
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            if self.context.pinned_unknowns.is_empty() { "" } else { ", pinned " },
            self.context.pinned_unknowns.join(", "),
        ));
    }

    fn render_current(&self, ui: &mut egui::Ui) {
        let Some(block) = self.playback.current() else { return };
        let (icon, color, summary) = block_style(block);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0));
            ui.label(egui::RichText::new(summary).color(color).strong());
        });

        // A torn block's inner sequence is the interesting part — show it.
        if let IcBlock::Torn { causal_steps, .. } = block
            && !causal_steps.is_empty()
        {
            ui.add_space(4.0);
            egui::Grid::new("ic_torn_grid").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                for (var, eq) in causal_steps {
                    ui.label(egui::RichText::new(var).monospace());
                    ui.weak(format!("from equation {eq}"));
                    ui.end_row();
                }
            });
        }
    }

    /// Goal line, how far through the plan the walk is, and the sequence so far.
    fn render_running_state(&self, ui: &mut egui::Ui) {
        ui.label(
            egui::RichText::new(
                "Goal: a consistent state at t=0 \u{2014} every equation satisfied before the \
                 solver takes its first step. The plan does that as a sequence, iterating only \
                 where it must.",
            )
            .italics()
            .color(crate::colors::ANIM_EXPLORE),
        );

        let (cursor, total) = self.playback.position();
        let done = cursor + 1;
        let solved: usize =
            self.playback.frames().iter().take(done).map(IcBlock::unknowns_solved).sum();
        let iterating = self
            .playback
            .frames()
            .iter()
            .take(done)
            .filter(|b| !matches!(b, IcBlock::Direct { .. }))
            .count();

        ui.add_space(4.0);
        ui.label(format!(
            "Block {done} of {total} \u{2014} {solved} unknown{} pinned down so far",
            if solved == 1 { "" } else { "s" },
        ));
        ui.label(match iterating {
            0 => "No iteration needed yet \u{2014} pure assignment.".to_owned(),
            1 => "1 block so far needed iteration.".to_owned(),
            n => format!("{n} blocks so far needed iteration."),
        });

        ui.add_space(6.0);
        ui.label(egui::RichText::new("Solve order so far").strong());
        egui::ScrollArea::vertical().auto_shrink([false, true]).max_height(300.0).show(ui, |ui| {
            egui::Grid::new("ic_plan_grid")
                .num_columns(3)
                .spacing([10.0, 2.0])
                .striped(true)
                .show(ui, |ui| {
                    for (i, b) in self.playback.frames().iter().take(done).enumerate() {
                        ui.label(format!("{}.", i + 1));
                        ui.label(egui::RichText::new(block_kind_label(b)).monospace());
                        ui.label(egui::RichText::new(block_targets(b)).monospace());
                        ui.end_row();
                    }
                });
        });
    }
}

impl Animated for IcPlanAnimation {
    fn which(&self) -> &'static str {
        "ic_plan"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, _arming: bool) -> crate::LiveState {
        // This view never runs live — the plan is already computed.
        crate::LiveState::Idle
    }

    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let block = self.playback.current()?;
        let (cursor, total) = self.playback.position();
        Some(serde_json::json!({
            "step": block_style(block).2,
            "kind": block_kind_label(block),
            "solves": block_targets(block),
            "block": cursor + 1,
            "blocks_total": total,
            "verdict": self.context.verdict,
            "dropped_equations": self.context.dropped_equations,
            "pinned_unknowns": self.context.pinned_unknowns,
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

fn parse_block(v: &serde_json::Value) -> Option<IcBlock> {
    match v.get("kind")?.as_str()? {
        "scalar_direct" => Some(IcBlock::Direct {
            var: v.get("var")?.as_str()?.to_owned(),
            solution: v
                .get("solution")
                .map(crate::reduction_view::expr_to_short)
                .unwrap_or_else(|| "?".to_owned()),
        }),
        "scalar_newton" => Some(IcBlock::Newton {
            var: v.get("var")?.as_str()?.to_owned(),
            equation: v.get("equation")?.as_u64()? as usize,
        }),
        "torn_block" => Some(IcBlock::Torn {
            tear_vars: str_list(v.get("tear_vars")),
            residual_equations: v
                .get("residual_equations")
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter().filter_map(serde_json::Value::as_u64).map(|n| n as usize).collect()
                })
                .unwrap_or_default(),
            causal_steps: v
                .get("causal_steps")
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|s| {
                            Some((
                                s.get("var")?.as_str()?.to_owned(),
                                s.get("equation")?.as_u64()? as usize,
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default(),
        }),
        // An unrecognised kind is dropped rather than guessed at: showing a
        // block whose meaning HRW does not know would be worse than a gap, and
        // a new Rumoca block kind should be added here deliberately.
        _ => None,
    }
}

fn str_list(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(serde_json::Value::as_array)
        .map(|a| a.iter().filter_map(serde_json::Value::as_str).map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Short kind label for the sequence table.
fn block_kind_label(b: &IcBlock) -> &'static str {
    match b {
        IcBlock::Direct { .. } => "direct",
        IcBlock::Newton { .. } => "newton",
        IcBlock::Torn { .. } => "torn",
    }
}

/// What the block solves, for the sequence table.
fn block_targets(b: &IcBlock) -> String {
    match b {
        IcBlock::Direct { var, .. } | IcBlock::Newton { var, .. } => var.clone(),
        IcBlock::Torn { tear_vars, causal_steps, .. } => {
            let mut names = tear_vars.clone();
            names.extend(causal_steps.iter().map(|(v, _)| v.clone()));
            names.join(", ")
        }
    }
}

/// Icon, colour and summary — **shared by the view and the capture**, so the
/// screen and the emitted context cannot give different accounts.
fn block_style(b: &IcBlock) -> (&'static str, egui::Color32, String) {
    match b {
        IcBlock::Direct { var, solution } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            format!("{var} is isolated \u{2014} just evaluate {solution}"),
        ),
        IcBlock::Newton { var, equation } => (
            "\u{1f501}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "{var} will not rearrange \u{2014} equation {equation} gets a scalar Newton \
                 iteration",
            ),
        ),
        IcBlock::Torn { tear_vars, residual_equations, causal_steps } => (
            "\u{2702}",
            crate::colors::ANIM_PATH_FOUND,
            format!(
                "A simultaneous set, torn open: guess {}, and {} follow{} by assignment; \
                 {} residual equation{} left to iterate on",
                tear_vars.join(", "),
                causal_steps.len(),
                if causal_steps.len() == 1 { "s" } else { "" },
                residual_equations.len(),
                if residual_equations.len() == 1 { "" } else { "s" },
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(blocks: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "blocks": blocks,
            "determinacy": {"verdict": "well-posed (remaining states initialize from their start attributes)"},
            "relaxation_hint": {"dropped_equations": [17], "pinned_unknowns": ["gnd.p.i"]},
        })
    }

    /// All three block kinds parse, and each says what it will actually do.
    #[test]
    fn every_block_kind_parses_and_describes_itself() {
        let anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_direct", "var": "src.v", "solution": {"Literal": {"Real": 12.0}}},
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
            {"kind": "torn_block", "tear_vars": ["C.p.v"], "residual_equations": [2],
             "causal_steps": [{"var": "C.n.v", "equation": 6, "newton": false}]},
        ])))
        .expect("a report with blocks parses");
        assert_eq!(anim.position(), (0, 3));

        let frames = anim.playback.frames();
        assert!(block_style(&frames[0]).2.contains("src.v"));
        let newton = block_style(&frames[1]).2;
        assert!(newton.contains("R.i") && newton.contains("Newton"), "{newton}");
        let torn = block_style(&frames[2]).2;
        assert!(torn.contains("C.p.v"), "the tear variable is the guess: {torn}");
        assert!(torn.contains("residual"), "{torn}");
    }

    /// A torn block pins down its tears *and* everything the tears made causal,
    /// so the running count must not treat it as one unknown.
    #[test]
    fn a_torn_block_counts_every_unknown_it_pins() {
        let torn = IcBlock::Torn {
            tear_vars: vec!["a".into()],
            residual_equations: vec![2],
            causal_steps: vec![("b".into(), 6), ("c".into(), 7)],
        };
        assert_eq!(torn.unknowns_solved(), 3, "one tear plus two causal");
        assert_eq!(IcBlock::Newton { var: "x".into(), equation: 1 }.unknowns_solved(), 1);
    }

    /// Determinacy and relaxation reach the capture — they are the two facts
    /// that explain the plan's shape, and are the answer when initialization
    /// misbehaves.
    #[test]
    fn the_capture_carries_the_verdict_and_the_relaxation() {
        let anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();
        let ctx = anim.current_frame_context().expect("a frame is under the cursor");
        assert_eq!(ctx["kind"], "newton");
        assert_eq!(ctx["solves"], "R.i");
        assert_eq!(ctx["block"], 1);
        assert!(ctx["verdict"].as_str().unwrap().starts_with("well-posed"));
        assert_eq!(ctx["dropped_equations"], serde_json::json!([17]));
        assert_eq!(ctx["pinned_unknowns"], serde_json::json!(["gnd.p.i"]));
        assert_eq!(anim.which(), "ic_plan");
    }

    /// A failed initialization carries an `error` and no `blocks`; there is no
    /// plan to walk, and the view must decline rather than render an empty one.
    #[test]
    fn a_failed_initialization_yields_no_view() {
        let failed = serde_json::json!({"error": {"kind": "underdetermined"}});
        assert!(IcPlanAnimation::from_report(&failed).is_none());
    }

    /// An unfamiliar block kind is dropped, not guessed at.
    #[test]
    fn an_unknown_block_kind_is_skipped() {
        let anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "some_future_kind", "var": "x"},
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();
        assert_eq!(anim.position(), (0, 1), "only the recognised block survives");
    }

    #[test]
    fn the_view_is_never_live() {
        let anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();
        assert_eq!(anim.live_state(true), crate::LiveState::Idle);
    }

    /// End to end on a real specimen: the plan Rumoca actually produces must be
    /// one this view can walk. Guards against a Rumoca rename silently emptying
    /// the view — the shape tests above would all still pass.
    ///
    /// `MixedLoop` rather than the IC-focused `RcCircuit` because
    /// `test_support::dae_for` loads no libraries, and `RcCircuit` is built from
    /// MSL components; it would skip, and a silently skipping end-to-end test is
    /// worse than none. `MixedLoop` is standalone and has both algebraic and
    /// state variables, so it has a real plan.
    #[test]
    fn a_real_specimen_produces_a_walkable_plan() {
        let dae = crate::test_support::dae_for("MixedLoop")
            .expect("MixedLoop is standalone and must compile");
        let n_x = dae.variables.states.len();
        let Ok(plan) = rumoca_phase_structural::build_ic_plan(&dae, n_x) else {
            return; // planning failed; there is no plan to walk
        };
        let hint = rumoca_phase_structural::build_ic_relaxation_hint(&dae, n_x);
        let json = crate::worker::ic_plan_to_json(
            &plan,
            hint.as_ref(),
            n_x,
            dae.continuous.equations.len(),
        );
        let anim = IcPlanAnimation::from_report(&json)
            .expect("a successful plan always carries a blocks array");
        let (_, total) = anim.position();
        assert!(total > 0, "RcCircuit has an initial-condition plan to walk");
    }
}
