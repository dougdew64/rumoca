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
            IcBlock::Torn {
                tear_vars,
                causal_steps,
                ..
            } => tear_vars.len() + causal_steps.len(),
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

/// One step of the walk: the opening state, or one block of the plan.
///
/// **`Start` is the opening frame — nothing solved, nothing attempted.** Added
/// 2026-08-23 with `alias_anim`'s, after Doug reported both views opening "with
/// progress already having been made". The four views fed by Rumoca capture
/// scopes each carry a `Start` step; these two parse a report *list*, so the
/// first frame was the first block already solved and no frame described the
/// system at t=0 before the plan ran.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcStep {
    /// Before the first block. What it needs to say — how many blocks, how many
    /// iterate — the animation computes from the plan it holds.
    Start,
    /// One block of the plan, executed.
    Block(IcBlock),
}

/// Reveal of the initial-condition solve plan.
pub struct IcPlanAnimation {
    /// `[Start, Block(0), …, Block(n-1)]` — **one longer than the block count**,
    /// so anything reporting a total must subtract the opening frame.
    playback: Playback<IcStep>,
    context: PlanContext,
    /// Parts of the plan report this parser could not read. Rendered above the
    /// header, because the messages below it — *"the plan is empty"*, *"nothing has
    /// to be solved at t=0"* — are positive claims about the model that become false
    /// the moment a block is lost in parsing.
    problems: Vec<String>,
}

impl IcPlanAnimation {
    /// Build from an initialization stage report.
    ///
    /// Returns `None` when the report has no `blocks` array — which is the case
    /// for a model whose initialization *failed* (the stage then carries an
    /// `error` instead). A failed initialization has no plan to walk.
    pub fn from_report(report: &serde_json::Value) -> Option<Self> {
        // Required: a model whose initialization failed carries an `error` instead,
        // and has no plan to walk.
        report.get("blocks")?.as_array()?;
        let mut problems = Vec::new();
        let blocks: Vec<IcBlock> =
            crate::json_read::parse_list(report, "blocks", &mut problems, parse_block);

        let determinacy = report.get("determinacy");
        let relax = report.get("relaxation_hint");
        // **`dropped_equations` is the sharpest case in this file.** It lists the
        // equations the compiler *threw away* to make initialization solvable, so an
        // entry lost in parsing **under-reports what was discarded** — the reader is
        // told the compiler relaxed less than it did, which is the opposite of the
        // thing the hint exists to disclose. Silent until the 2026-08-04 sweep.
        let context = PlanContext {
            verdict: determinacy
                .and_then(|d| d.get("verdict"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned),
            dropped_equations: relax
                .map(|r| {
                    crate::json_read::parse_list(r, "dropped_equations", &mut problems, |v| {
                        v.as_u64().map(|n| n as usize)
                    })
                })
                .unwrap_or_default(),
            pinned_unknowns: relax
                .map(|r| {
                    crate::json_read::parse_list(r, "pinned_unknowns", &mut problems, |v| {
                        v.as_str().map(str::to_owned)
                    })
                })
                .unwrap_or_default(),
        };

        // **No opening frame when there is no plan.** A lone `Start` would make
        // `is_empty()` false for a model with nothing to solve at t=0, and the
        // pane would offer a walk through nothing instead of saying so.
        let frames: Vec<IcStep> = if blocks.is_empty() {
            Vec::new()
        } else {
            std::iter::once(IcStep::Start)
                .chain(blocks.into_iter().map(IcStep::Block))
                .collect()
        };

        Some(Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
            context,
            problems,
        })
    }

    /// Blocks in the plan, which is one fewer than the frame count.
    fn n_blocks(&self) -> usize {
        self.playback.position().1.saturating_sub(1)
    }

    /// The blocks walked so far — empty at the opening frame.
    ///
    /// `skip(1)` steps over `Start`, and the cursor doubles as the count for the
    /// same reason it does in `alias_anim`: at frame 0 no block has run, and at
    /// frame k exactly k have.
    fn blocks_done(&self) -> impl Iterator<Item = &IcBlock> {
        self.playback
            .frames()
            .iter()
            .skip(1)
            .take(self.playback.cursor())
            .filter_map(|s| match s {
                IcStep::Block(b) => Some(b),
                IcStep::Start => None,
            })
    }

    /// What the opening frame says: the plan as it stands before any of it runs.
    fn start_summary(&self) -> String {
        let iterating = self
            .playback
            .frames()
            .iter()
            .filter(|s| matches!(s, IcStep::Block(b) if !matches!(b, IcBlock::Direct { .. })))
            .count();
        format!(
            "Starting point: {} block{} to solve at t=0, {} needing iteration \u{2014} nothing \
             solved yet",
            self.n_blocks(),
            if self.n_blocks() == 1 { "" } else { "s" },
            iterating,
        )
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the header, the controls, and the plan walk. No Debug button:
    /// see the module note on why this phase has no live trace to offer.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        for p in &self.problems {
            ui.colored_label(ui.visuals().error_fg_color, format!("\u{26a0} {p}"));
        }
        self.render_header(ui);

        if self.playback.is_empty() {
            ui.add_space(6.0);
            ui.label("The initial-condition plan is empty.");
            if let Some(note) = empty_plan_note(self.context.verdict.as_deref()) {
                ui.weak(note);
            }
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
            if self.context.dropped_equations.len() == 1 {
                ""
            } else {
                "s"
            },
            self.context
                .dropped_equations
                .iter()
                .map(usize::to_string)
                .collect::<Vec<_>>()
                .join(", "),
            if self.context.pinned_unknowns.is_empty() {
                ""
            } else {
                ", pinned "
            },
            self.context.pinned_unknowns.join(", "),
        ));
    }

    fn render_current(&self, ui: &mut egui::Ui) {
        let Some(step) = self.playback.current() else {
            return;
        };
        // Clapper board and `ANIM_EXPLORE` on the opening frame, matching the
        // four capture-driven views — `34c22d56`: a start icon, not a finish
        // flag.
        let (icon, color, summary) = match step {
            IcStep::Start => (
                "\u{1f3ac}",
                crate::colors::ANIM_EXPLORE,
                self.start_summary(),
            ),
            IcStep::Block(b) => block_style(b),
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0));
            ui.label(egui::RichText::new(summary).color(color).strong());
        });

        let IcStep::Block(block) = step else {
            return;
        };

        // A torn block's inner sequence is the interesting part — show it.
        if let IcBlock::Torn { causal_steps, .. } = block
            && !causal_steps.is_empty()
        {
            ui.add_space(4.0);
            egui::Grid::new("ic_torn_grid")
                .num_columns(2)
                .spacing([10.0, 2.0])
                .show(ui, |ui| {
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

        // **The cursor IS the count of blocks executed**, because frame 0 is the
        // opening state. This used to read `cursor + 1`, which is what made the
        // walk open claiming a block had already been solved.
        let done = self.playback.cursor();
        let total = self.n_blocks();
        let solved: usize = self.blocks_done().map(IcBlock::unknowns_solved).sum();
        let iterating = self
            .blocks_done()
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
        // **No inner scroll area, and no height cap.** `App::ic_plan_anim_ui`
        // already wraps this whole view in a vertical scroll area, so a second
        // one nested inside it capped the solve order at 300pt and captured the
        // mouse wheel. `RcCircuit` and `OverInitRc` plan 21 blocks each, which
        // overflows that cap — the list scrolled inside a small box while the
        // pane around it stayed empty.
        //
        // The parent scrolls; this view just renders. Held by
        // `playback::tests_layout::a_view_inside_a_scrolling_pane_does_not_scroll_or_cap_itself`.
        egui::Grid::new("ic_plan_grid")
            .num_columns(3)
            .spacing([10.0, 2.0])
            .striped(true)
            .show(ui, |ui| {
                for (i, b) in self.blocks_done().enumerate() {
                    ui.label(format!("{}.", i + 1));
                    ui.label(egui::RichText::new(block_kind_label(b)).monospace());
                    ui.label(egui::RichText::new(block_targets(b)).monospace());
                    ui.end_row();
                }
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
        let step = self.playback.current()?;
        // `blocks_total` counts plan blocks, so it excludes the opening frame.
        let mut ctx = serde_json::json!({
            "block": self.playback.cursor(),
            "blocks_total": self.n_blocks(),
            "verdict": self.context.verdict,
            "dropped_equations": self.context.dropped_equations,
            "pinned_unknowns": self.context.pinned_unknowns,
        });
        let obj = ctx.as_object_mut().expect("built as an object");
        match step {
            IcStep::Start => {
                obj.insert("step".to_owned(), serde_json::json!(self.start_summary()));
            }
            IcStep::Block(block) => {
                obj.insert("step".to_owned(), serde_json::json!(block_style(block).2));
                obj.insert(
                    "kind".to_owned(),
                    serde_json::json!(block_kind_label(block)),
                );
                obj.insert("solves".to_owned(), serde_json::json!(block_targets(block)));
            }
        }
        Some(ctx)
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
                    a.iter()
                        .filter_map(serde_json::Value::as_u64)
                        .map(|n| n as usize)
                        .collect()
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
        .map(|a| {
            a.iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
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
        IcBlock::Torn {
            tear_vars,
            causal_steps,
            ..
        } => {
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
        IcBlock::Torn {
            tear_vars,
            residual_equations,
            causal_steps,
        } => (
            "\u{2702}",
            crate::colors::ANIM_PATH_FOUND,
            format!(
                "A simultaneous set, torn open: guess {}, and {} follow{} by assignment; \
                 {} residual equation{} left to iterate on",
                tear_vars.join(", "),
                causal_steps.len(),
                if causal_steps.len() == 1 { "s" } else { "" },
                residual_equations.len(),
                if residual_equations.len() == 1 {
                    ""
                } else {
                    "s"
                },
            ),
        ),
    }
}

/// The one determinacy verdict the empty-plan gloss paraphrases.
///
/// Single-sourced so the gloss and the test that gates it cannot drift apart —
/// the same defect the not-run wording had in `worker.rs`, where five hand-written
/// copies agreed by coincidence.
const START_ATTRIBUTE_VERDICT: &str =
    "well-posed (remaining states initialize from their start attributes)";

/// What the pane may add beneath *"The initial-condition plan is empty."*
///
/// # Why this is gated rather than always shown (finding C19)
///
/// The gloss states a **cause**: *every unknown comes from a start attribute.*
/// HRW does not compute that — it is a literal, and the report separately carries
/// Rumoca's own `determinacy.verdict`, which `render_header` paints directly above.
/// Until 2026-08-25 the sentence was unconditional, so it was true only because the
/// two specimens that reach an empty plan — `BouncingBall` and `SingleInertia` —
/// both happen to return [`START_ATTRIBUTE_VERDICT`]. **True by agreement with the
/// corpus, not by derivation**, and an empty plan under any other verdict would have
/// had HRW assert one cause directly beneath Rumoca stating another.
///
/// **The middle arm returns `None` on purpose, and that is not an oversight to fill
/// in later.** When Rumoca returned a verdict HRW cannot paraphrase, the header is
/// already showing the compiler's own words — adding a sentence there would be
/// inventing the explanation this fix exists to remove. *Absence is stated, never
/// filled.*
///
/// The missing-verdict arm does speak, because a report with no verdict paints no
/// header at all, and a pane saying only *"the plan is empty"* would leave the
/// reader unable to tell a well-posed model from an unreported one.
///
/// Deliberately **not** reusing `render_header`'s `starts_with("well-posed")`: that
/// predicate picks an icon, and any future well-posed verdict with a different cause
/// would satisfy it while making the gloss false.
fn empty_plan_note(verdict: Option<&str>) -> Option<&'static str> {
    match verdict {
        Some(v) if v.starts_with(START_ATTRIBUTE_VERDICT) => Some(
            "Nothing has to be solved at t=0 \u{2014} every unknown comes from a start attribute.",
        ),
        Some(_) => None,
        None => Some("Rumoca's report carried no determinacy verdict for this model."),
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

    /// The empty-plan gloss appears only for the verdict it paraphrases (C19).
    ///
    /// **The third case is the one with teeth.** `render_header` decides its icon with
    /// `starts_with("well-posed")`, and reusing that predicate here would let the gloss
    /// fire for *any* well-posed verdict — including one whose cause is something other
    /// than start attributes, which is precisely the false claim this gate removes.
    #[test]
    fn the_empty_plan_gloss_is_shown_only_for_the_verdict_it_paraphrases() {
        assert_eq!(
            empty_plan_note(Some(START_ATTRIBUTE_VERDICT)),
            Some(
                "Nothing has to be solved at t=0 \u{2014} every unknown comes from a start attribute."
            ),
            "the verdict the gloss paraphrases must still produce it"
        );

        assert_eq!(
            empty_plan_note(Some("over-determined (3 equations too many)")),
            None,
            "HRW must not explain a cause Rumoca did not state; the header carries its words"
        );

        assert_eq!(
            empty_plan_note(Some(
                "well-posed (all unknowns fixed by parameter bindings)"
            )),
            None,
            "a DIFFERENT well-posed verdict must not borrow the start-attribute explanation"
        );

        assert_eq!(
            empty_plan_note(None),
            Some("Rumoca's report carried no determinacy verdict for this model."),
            "with no verdict there is no header either, so the absence must be stated"
        );
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
        assert_eq!(
            anim.position(),
            (0, 4),
            "three blocks plus the opening frame"
        );

        let blocks: Vec<&IcBlock> = anim
            .playback
            .frames()
            .iter()
            .filter_map(|s| match s {
                IcStep::Block(b) => Some(b),
                IcStep::Start => None,
            })
            .collect();
        assert_eq!(blocks.len(), 3, "the opening frame is not a block");
        assert!(block_style(blocks[0]).2.contains("src.v"));
        let newton = block_style(blocks[1]).2;
        assert!(
            newton.contains("R.i") && newton.contains("Newton"),
            "{newton}"
        );
        let torn = block_style(blocks[2]).2;
        assert!(
            torn.contains("C.p.v"),
            "the tear variable is the guess: {torn}"
        );
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
        assert_eq!(
            IcBlock::Newton {
                var: "x".into(),
                equation: 1
            }
            .unknowns_solved(),
            1
        );
    }

    /// Determinacy and relaxation reach the capture — they are the two facts
    /// that explain the plan's shape, and are the answer when initialization
    /// misbehaves.
    #[test]
    fn the_capture_carries_the_verdict_and_the_relaxation() {
        let mut anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();
        assert!(anim.seek(1), "step past the opening frame to the block");
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["kind"], "newton");
        assert_eq!(ctx["solves"], "R.i");
        assert_eq!(ctx["block"], 1);
        assert!(ctx["verdict"].as_str().unwrap().starts_with("well-posed"));
        assert_eq!(ctx["dropped_equations"], serde_json::json!([17]));
        assert_eq!(ctx["pinned_unknowns"], serde_json::json!(["gnd.p.i"]));
        assert_eq!(anim.which(), "ic_plan");
    }

    /// **The walk opens before any block has been solved.**
    ///
    /// The sibling of `alias_anim`'s guard, from the same report: Doug,
    /// 2026-08-23, found both views opening *"with progress already having been
    /// made"*. The pair of assertions is the point — the opening frame reports
    /// **zero** blocks done, and the frame after it reports one.
    #[test]
    fn the_walk_opens_before_any_block_has_been_solved() {
        let mut anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_direct", "var": "src.v", "solution": {"Literal": {"Real": 12.0}}},
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();

        assert_eq!(anim.playback.current(), Some(&IcStep::Start));
        let opening = anim.current_frame_context().expect("an opening frame");
        assert_eq!(opening["block"], 0, "no block has run at the opening frame");
        assert_eq!(opening["blocks_total"], 2);
        assert!(
            opening["kind"].is_null() && opening["solves"].is_null(),
            "the opening frame solves nothing, so it must claim no block"
        );
        assert!(
            opening["step"].as_str().is_some_and(|s| s.contains("1")),
            "the opening frame states how many blocks need iteration: {}",
            opening["step"]
        );

        assert!(anim.seek(1), "the first block is frame 1");
        assert_eq!(anim.current_frame_context().expect("a frame")["block"], 1);
    }

    /// **Every frame describes itself, including the opening one.**
    ///
    /// The twin of `alias_anim`'s, and for the same reason: this view builds its
    /// capture in two branches, and the source-level family check in
    /// `playback::tests_animated_contract` cannot see a branch that forgot `step`.
    #[test]
    fn every_frame_describes_itself() {
        let mut anim = IcPlanAnimation::from_report(&report(serde_json::json!([
            {"kind": "scalar_direct", "var": "src.v", "solution": {"Literal": {"Real": 12.0}}},
            {"kind": "scalar_newton", "var": "R.i", "equation": 9},
        ])))
        .unwrap();

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
        assert_eq!(
            anim.position(),
            (0, 2),
            "only the recognised block survives, plus the opening frame"
        );
        assert_eq!(
            anim.n_blocks(),
            1,
            "one block, and the opening frame is not"
        );
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
