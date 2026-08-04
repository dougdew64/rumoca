//! Animated tearing stepper — replays how an algebraic loop is broken open.
//!
//! ## Why this phase is worth animating
//!
//! BLT leaves behind blocks that cannot be ordered: *n* equations in *n*
//! unknowns that must be solved together. Tearing attacks such a block by
//! guessing a small set of **tear variables**, which turns most of the block
//! back into a sequence of assignments; only the leftover **residual
//! equations** go to the nonlinear solver. Fewer tears means a smaller
//! Newton iteration, so the quality of the guess is the whole game.
//!
//! Rumoca's heuristic is greedy: repeatedly tear the variable appearing in the
//! most still-unsolved equations, then propagate — assign every equation that
//! this leaves with exactly one unknown. The report shows which variables ended
//! up torn. It cannot show *why* each was chosen, because the reason is a count
//! that exists only while the loop runs:
//!
//! - **appearances** — how many unsolved equations the winner appeared in.
//! - **competitors** — how many unknowns an equation still had when it became
//!   causal, which is what makes "exactly one left" the trigger it is.
//!
//! Those two numbers are the content of this view. Reading a
//! `TearingReport` tells you the answer; watching the replay tells you that the
//! algorithm is a greedy count-and-pick, and shows you the moment a single tear
//! cascades into several free assignments.
//!
//! ## Where the frames come from
//!
//! Unlike index reduction, tearing is not re-run by the pipeline — it happens
//! inside `build_structural_report`, whose result HRW already holds. So this
//! view rebuilds the situation from the DAE: incidence, matching, BLT blocks,
//! and then `tear_algebraic_loop_with_trace` on each coupled block, using
//! `block_local_incidence` to translate the block into the 0..n index space the
//! algorithm works in. Recorded and live playback run the *same* walk
//! ([`walk_blocks`]); the only difference is where the frames are delivered.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use eframe::egui;

use rumoca_phase_structural::{LiveTrace, TearingFrame, TearingStep};

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. Slower than matching's cursor walk:
/// each frame here is a decision about the system, not a probe.
const FRAME_INTERVAL: f64 = 0.7;

/// The names behind one coupled block's local indices.
///
/// `TearingFrame` speaks in block-local `usize`s — that is the space the
/// algorithm works in. Rendering needs the names back, and they are per block,
/// so they are held here rather than on the frames (which would repeat every
/// name on every frame).
#[derive(Debug, Clone, Default)]
pub struct BlockNames {
    /// Equation labels, in block-local order.
    pub equations: Vec<String>,
    /// Unknown names, in block-local order.
    pub unknowns: Vec<String>,
}

/// One frame, tagged with the coupled block it belongs to.
///
/// A model may hold several algebraic loops and each is torn independently. The
/// replay walks them in order, so a frame has to say which block it is about.
#[derive(Debug, Clone)]
pub struct BlockFrame {
    /// Index into the animation's `blocks`.
    pub block: usize,
    pub frame: TearingFrame,
}

/// Replay of tearing — recorded or live.
pub struct TearingAnimation {
    playback: Playback<BlockFrame>,
    /// Names per coupled block, parallel to `BlockFrame::block`. Populated on
    /// construction for a recorded replay; for a live session it is built up
    /// front too, since walking the blocks is cheap and only the *frames* need
    /// to arrive from the debugger thread.
    blocks: Vec<BlockNames>,
}

/// Walk every coupled block of `dae`, tearing each with an observer attached.
///
/// Returns the per-block names; the frames go to `emit`, tagged with the block
/// index. Shared by recorded and live playback so the two can never diverge.
///
/// Scalar BLT blocks are skipped — a 1x1 block is already causal and there is
/// nothing to tear. A model with no algebraic loop therefore produces no
/// frames at all, which the view reports as "no algebraic loops".
pub fn walk_blocks(dae: &rumoca_ir_dae::Dae, emit: &dyn Fn(usize, TearingFrame)) -> Vec<BlockNames> {
    use rumoca_phase_structural::BltBlock;

    let inc = rumoca_phase_structural::build_incidence(dae);
    let (match_eq, match_var) =
        rumoca_phase_structural::matching::maximum_matching(inc.n_eq, inc.n_var, &inc.eq_unknowns);
    // The dependency graph is equation -> equation: an edge where one equation
    // needs an unknown another equation solves. That is why it is keyed by
    // `match_var` (unknown -> equation) rather than `match_eq`.
    let adj = rumoca_phase_structural::incidence::build_dependency_graph(
        &inc.eq_unknowns,
        &match_var,
        inc.n_eq,
    );
    let blt = rumoca_phase_structural::blt::build_blt_blocks(&inc, &match_eq, &adj);

    let mut names = Vec::new();
    for block in &blt {
        let BltBlock::AlgebraicLoop { equations, unknowns } = block else {
            continue;
        };
        let index = names.len();
        names.push(BlockNames {
            equations: equations.iter().map(|e| e.to_string()).collect(),
            unknowns: unknowns.iter().map(|u| u.to_string()).collect(),
        });
        let local = rumoca_phase_structural::block_local_incidence(&inc, equations, unknowns);
        let sink = |f: &TearingFrame| emit(index, f.clone());
        let _ = rumoca_phase_structural::tear_algebraic_loop_with_trace(
            unknowns.len(),
            &local,
            Some(&sink),
        );
    }
    names
}

impl TearingAnimation {
    /// Record every coupled block's tearing from a finished DAE.
    /// Build from **tearing captured during the compile**, with block names read
    /// from the structural report.
    ///
    /// [`Self::record`] re-derives four things to get here — incidence, matching,
    /// Tarjan and then the tearing itself — so the loops a reader watches being torn
    /// were torn by a run that produced nothing, while the tear variables on screen
    /// came from one nobody saw. This is the last of the recorded animations to stop
    /// doing that (2026-08-04).
    ///
    /// # The one real hazard: which blocks the segments belong to
    ///
    /// `segments` holds one entry per **coupled** block, in tearing order. The
    /// report's `blocks` array holds **every** block, scalar ones included — and
    /// scalar blocks are never torn. Zipping the two directly would attach loop *i*'s
    /// reasoning to whatever block sat at index *i*, which on a model with a scalar
    /// block before a coupled one is silently the wrong loop.
    ///
    /// So the report is filtered to `kind == "coupled"` first, and the count is
    /// checked: a mismatch returns `None` rather than guessing an alignment.
    /// `TwoLoops` (two coupled blocks) is the specimen that makes this testable
    /// rather than merely asserted.
    pub fn from_captured(
        report: &serde_json::Value,
        segments: &[Vec<TearingFrame>],
    ) -> Option<Self> {
        if segments.is_empty() {
            return None;
        }
        let coupled: Vec<&serde_json::Value> = report
            .get("blocks")?
            .as_array()?
            .iter()
            .filter(|b| b.get("kind").and_then(serde_json::Value::as_str) == Some("coupled"))
            .collect();

        // **Refuses rather than aligns.** A count mismatch means the capture and the
        // report describe different runs, and an animation that quietly pairs them
        // would be a wrong picture with no symptom.
        if coupled.len() != segments.len() {
            return None;
        }

        let names = |b: &serde_json::Value, key: &str| -> Vec<String> {
            b.get(key)
                .and_then(serde_json::Value::as_array)
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default()
        };
        let blocks: Vec<BlockNames> = coupled
            .iter()
            .map(|b| BlockNames {
                equations: names(b, "equations"),
                unknowns: names(b, "unknowns"),
            })
            .collect();

        // Frames carry the block they belong to, so the flat playback the view
        // expects is rebuilt by tagging each segment with its index.
        let frames: Vec<BlockFrame> = segments
            .iter()
            .enumerate()
            .flat_map(|(block, seg)| {
                seg.iter().map(move |frame| BlockFrame { block, frame: frame.clone() })
            })
            .collect();

        Some(Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
            blocks,
        })
    }

    /// **Re-runs tearing over its own freshly-built BLT. Test-only, enforced by the
    /// compiler** — see [`crate::matching_anim::MatchingAnimation::from_incidence`]
    /// for why a `cfg` replaced a source-text grep on 2026-08-04.
    #[cfg(test)]
    pub fn record(dae: &rumoca_ir_dae::Dae) -> Self {
        // `walk_blocks` takes `&dyn Fn`, so the accumulator needs interior
        // mutability — the same shape the phase's observer contract forces.
        let frames = std::cell::RefCell::new(Vec::new());
        let blocks = walk_blocks(dae, &|block, frame| {
            frames.borrow_mut().push(BlockFrame { block, frame });
        });
        Self {
            playback: Playback::recorded(frames.into_inner(), FRAME_INTERVAL),
            blocks,
        }
    }

    /// Start a live debug session: the same walk, on a thread that parks until
    /// the debugger attaches, pushing frames as they are produced.
    pub fn start_live(
        dae: rumoca_ir_dae::Dae,
        on_complete: impl FnOnce() + Send + 'static,
    ) -> Option<Self> {
        // Names are needed by the *renderer* from the first frame onward, so
        // they are computed here rather than waiting on the thread.
        let blocks = walk_blocks(&dae, &|_, _| {});

        let (lt, rx) = LiveTrace::new();
        let lt = lt.with_frame_delay(std::time::Duration::from_millis(20));
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);

        thread::Builder::new()
            .name("tearing-debug".to_owned())
            .spawn(move || {
                lt.wait_for_debugger();
                walk_blocks(&dae, &|block, frame| lt.push(BlockFrame { block, frame }));
                on_complete();
                done_for_thread.store(true, Ordering::Release);
            })
            .ok()?;

        Some(Self { playback: Playback::live(rx, done, FRAME_INTERVAL), blocks })
    }

    /// The tear variables this replay decided on, by name, across every block.
    ///
    /// Exists so the re-derivation can be compared against Rumoca's own report: HRW
    /// re-runs tearing to animate it, and until 2026-07-30 nothing checked that its
    /// answer matched the compiler's. See `docs/fidelity-plan.md` F1.
    pub fn tear_variable_names(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, names) in self.blocks.iter().enumerate() {
            // The final frame of each block carries the complete tear set.
            if let Some(last) = self.playback.frames().iter().rfind(|f| f.block == i) {
                for &v in &last.frame.tears_so_far {
                    out.push(name_of(&names.unknowns, v));
                }
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Render the controls, the step line, and the running state.
    ///
    /// Returns `true` on the frame the Debug button is clicked.
    #[must_use]
    pub fn ui(&mut self, ui: &mut egui::Ui, arming: bool, debug_enabled: bool) -> bool {
        self.playback.sync_live();
        let live = self.playback.live_state(arming);

        if self.playback.is_empty() && !arming {
            ui.label("No algebraic loops in this model \u{2014} nothing to tear.");
            ui.weak(
                "Every BLT block is 1x1, so the system solves as a straight sequence of \
                 assignments.",
            );
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

        if let Some(bf) = self.playback.current() {
            let names = self.blocks.get(bf.block).cloned().unwrap_or_default();
            ui.add_space(4.0);
            render_step(ui, bf, &names);
            ui.add_space(8.0);
            render_running_state(ui, bf, &names);
        }

        debug_clicked
    }
}

impl Animated for TearingAnimation {
    fn which(&self) -> &'static str {
        "tearing"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        self.playback.live_state(arming)
    }

    /// The sentence on screen plus the two running sets the greedy loop carries:
    /// which variables have been torn, and which equations have gone causal.
    /// Together they are the algorithm's entire state.
    fn current_frame_context(&self) -> Option<serde_json::Value> {
        let bf = self.playback.current()?;
        let names = self.blocks.get(bf.block).cloned().unwrap_or_default();
        Some(serde_json::json!({
            "block": bf.block,
            "block_size": names.unknowns.len(),
            "step": step_summary(bf, &names),
            "torn_so_far": bf.frame.tears_so_far.iter()
                .map(|&v| name_of(&names.unknowns, v)).collect::<Vec<_>>(),
            "causal_so_far": bf.frame.causal_so_far.iter()
                .map(|&(e, v)| format!("{} solves {}",
                    name_of(&names.equations, e), name_of(&names.unknowns, v)))
                .collect::<Vec<_>>(),
        }))
    }

    /// Seek is delegated to [`Playback`], so all eight views agree on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

/// Look a block-local index back up, degrading to the index itself rather than
/// panicking — a live session renders frames before it is certain the name
/// table matches, and a missing name must not take the app down.
fn name_of(names: &[String], i: usize) -> String {
    names.get(i).cloned().unwrap_or_else(|| format!("#{i}"))
}

/// The one-line description of a step — **shared by the view and the capture**,
/// so the screen and the emitted context cannot give different accounts of one
/// frame.
fn step_summary(bf: &BlockFrame, names: &BlockNames) -> String {
    step_style(bf, names).2
}

/// Icon, colour and summary. Icons are only ever codepoints this app already
/// renders elsewhere.
fn step_style(bf: &BlockFrame, names: &BlockNames) -> (&'static str, egui::Color32, String) {
    match &bf.frame.step {
        TearingStep::Start { n } => (
            "\u{1f3ac}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "An algebraic loop: {n} equations in {n} unknowns, none solvable on its own",
            ),
        ),
        TearingStep::Torn { variable, appearances, remaining_equations } => (
            "\u{2702}",
            crate::colors::ANIM_PATH_FOUND,
            // The greedy choice. `appearances` is the reason it won, and saying
            // it out loud is the difference between watching and being told.
            format!(
                "Tore {} \u{2014} it appeared in {appearances} of the {remaining_equations} \
                 unsolved equations, more than any other unknown",
                name_of(&names.unknowns, *variable),
            ),
        ),
        TearingStep::Causal { equation, variable, competitors } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            // The cascade. `competitors` says how crowded the equation was
            // before the tear knocked the others out.
            format!(
                "{} now solves {} \u{2014} it is the last unknown left there (it had {} before \
                 the tears)",
                name_of(&names.equations, *equation),
                name_of(&names.unknowns, *variable),
                competitors + 1,
            ),
        ),
        TearingStep::Complete { tears, residuals } => (
            "\u{2705}",
            crate::colors::ANIM_PATH_FOUND,
            format!(
                "Done: {tears} tear{} leaves {residuals} residual equation{} for the nonlinear \
                 solver",
                if *tears == 1 { "" } else { "s" },
                if *residuals == 1 { "" } else { "s" },
            ),
        ),
        TearingStep::NoProgress => (
            "\u{26a0}",
            crate::colors::ANIM_EXPLORE,
            "No progress \u{2014} no unknown appears in any unsolved equation; the heuristic \
             gives up and the whole block goes to the solver"
                .to_owned(),
        ),
    }
}

fn render_step(ui: &mut egui::Ui, bf: &BlockFrame, names: &BlockNames) {
    let (icon, color, summary) = step_style(bf, names);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(icon).size(16.0));
        ui.label(egui::RichText::new(&summary).color(color).strong());
    });
}

/// Goal line plus the running state — what has been torn, and how much of the
/// block that has made causal. This is the "am I winning?" panel: a good tear
/// is one that buys many causal assignments.
fn render_running_state(ui: &mut egui::Ui, bf: &BlockFrame, names: &BlockNames) {
    let n = names.unknowns.len();
    ui.label(
        egui::RichText::new(
            "Goal: tear as few variables as possible \u{2014} each tear is one more unknown the \
             nonlinear solver must iterate on.",
        )
        .italics()
        .color(crate::colors::ANIM_EXPLORE),
    );

    let torn: Vec<String> =
        bf.frame.tears_so_far.iter().map(|&v| name_of(&names.unknowns, v)).collect();
    let causal = bf.frame.causal_so_far.len();

    ui.add_space(4.0);
    ui.label(if torn.is_empty() {
        "Nothing torn yet.".to_owned()
    } else {
        format!(
            "Torn so far ({}): {}",
            torn.len(),
            torn.join(", "),
        )
    });
    ui.label(format!(
        "{causal} of {n} equations made causal \u{2014} {} still tangled",
        n.saturating_sub(causal),
    ));

    if bf.frame.causal_so_far.is_empty() {
        return;
    }
    ui.add_space(6.0);
    ui.label(egui::RichText::new("Solve sequence so far").strong());
    egui::Grid::new("tearing_causal_grid")
        .num_columns(3)
        .spacing([10.0, 2.0])
        .striped(true)
        .show(ui, |ui| {
            for (i, &(eq, var)) in bf.frame.causal_so_far.iter().enumerate() {
                ui.label(format!("{}.", i + 1));
                ui.label(egui::RichText::new(name_of(&names.equations, eq)).monospace());
                ui.label(
                    egui::RichText::new(format!("\u{2192} {}", name_of(&names.unknowns, var)))
                        .monospace(),
                );
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> BlockNames {
        BlockNames {
            equations: vec!["f_x[0]".into(), "f_x[1]".into(), "f_x[2]".into()],
            unknowns: vec!["command".into(), "error".into(), "measurement".into()],
        }
    }

    fn bf(step: TearingStep, tears: &[usize], causal: &[(usize, usize)]) -> BlockFrame {
        BlockFrame {
            block: 0,
            frame: TearingFrame {
                step,
                tears_so_far: tears.to_vec(),
                causal_so_far: causal.to_vec(),
            },
        }
    }

    /// The two counts that only exist mid-run are the reason this view exists,
    /// so both must reach the screen rather than merely the frame.
    #[test]
    fn the_reasons_reach_the_summary() {
        let torn = bf(
            TearingStep::Torn { variable: 0, appearances: 3, remaining_equations: 3 },
            &[0],
            &[],
        );
        let s = step_summary(&torn, &names());
        assert!(s.contains("command"), "{s}");
        assert!(s.contains('3'), "the appearance count is the reason: {s}");

        let causal =
            bf(TearingStep::Causal { equation: 1, variable: 1, competitors: 1 }, &[0], &[(1, 1)]);
        let s = step_summary(&causal, &names());
        assert!(s.contains("f_x[1]") && s.contains("error"), "{s}");
        // competitors + 1 = the crowd before the tears knocked them out.
        assert!(s.contains('2'), "the competitor count is the reason: {s}");
    }

    /// Every step renders something, including the give-up path.
    #[test]
    fn every_step_renders() {
        for step in [
            TearingStep::Start { n: 3 },
            TearingStep::Torn { variable: 0, appearances: 3, remaining_equations: 3 },
            TearingStep::Causal { equation: 0, variable: 1, competitors: 0 },
            TearingStep::Complete { tears: 1, residuals: 1 },
            TearingStep::NoProgress,
        ] {
            assert!(!step_summary(&bf(step, &[], &[]), &names()).is_empty());
        }
    }

    /// A missing name degrades to the index instead of panicking. A live
    /// session can render a frame before its block's names are certain, and an
    /// out-of-range index must never take the app down (the specimen-view
    /// crash of 2026-07-27 is the cautionary tale).
    #[test]
    fn an_unknown_index_degrades_rather_than_panics() {
        let s = step_summary(
            &bf(TearingStep::Torn { variable: 99, appearances: 1, remaining_equations: 1 }, &[], &[]),
            &BlockNames::default(),
        );
        assert!(s.contains("#99"), "{s}");
    }

    /// The capture carries the same sentence the view draws, plus both running
    /// sets — together the algorithm's whole state at that frame.
    #[test]
    fn the_capture_gets_the_step_and_both_running_sets() {
        let anim = TearingAnimation {
            playback: Playback::recorded(
                vec![bf(
                    TearingStep::Causal { equation: 1, variable: 1, competitors: 1 },
                    &[0],
                    &[(1, 1)],
                )],
                FRAME_INTERVAL,
            ),
            blocks: vec![names()],
        };
        let ctx = anim.current_frame_context().expect("a frame is under the cursor");
        assert_eq!(ctx["torn_so_far"], serde_json::json!(["command"]));
        assert_eq!(ctx["causal_so_far"], serde_json::json!(["f_x[1] solves error"]));
        assert_eq!(ctx["block_size"], serde_json::json!(3));
        assert_eq!(anim.which(), "tearing");
    }

    #[test]
    fn a_model_with_no_loops_is_empty() {
        let anim = TearingAnimation {
            playback: Playback::recorded(Vec::new(), FRAME_INTERVAL),
            blocks: Vec::new(),
        };
        assert!(anim.is_empty());
        assert!(anim.current_frame_context().is_none());
        assert_eq!(anim.live_state(false), crate::LiveState::Idle);
    }

    /// End to end on a real coupled block: `walk_blocks` must find
    /// `ProportionalLoop`'s 3x3 loop and produce a Start, at least one Torn,
    /// and a Complete — the proof that the block-local translation lines up
    /// with what the algorithm expects.
    #[test]
    fn walk_blocks_traces_a_real_algebraic_loop() {
        let Some(dae) = crate::test_support::dae_for("ProportionalLoop") else {
            return; // specimen unavailable in this checkout
        };
        let anim = TearingAnimation::record(&dae);
        assert!(!anim.is_empty(), "ProportionalLoop has a coupled block");
        assert_eq!(anim.blocks.len(), 1, "exactly one algebraic loop");
        assert_eq!(anim.blocks[0].unknowns.len(), 3, "a 3x3 loop");

        let steps: Vec<&TearingStep> =
            anim.playback.frames().iter().map(|bf| &bf.frame.step).collect();
        assert!(matches!(steps.first(), Some(TearingStep::Start { n: 3 })), "{steps:?}");
        assert!(
            steps.iter().any(|s| matches!(s, TearingStep::Torn { .. })),
            "something must get torn: {steps:?}",
        );
        assert!(matches!(steps.last(), Some(TearingStep::Complete { .. })), "{steps:?}");
    }
}
