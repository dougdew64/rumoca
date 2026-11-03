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
//! **This view stopped rebuilding the situation in the v0.10.0 rebase.** It
//! used to reconstruct the whole analysis from the DAE — incidence, matching,
//! the dependency graph, BLT — and then re-tear each coupled block, which
//! needed four APIs that are all private on 0.10.0 and, worse, meant the loops
//! a reader watched being torn were torn by a run that produced nothing.
//!
//! The steps now come from `inspect_structural_report`, which observes the
//! tearing the compiler performed, and the block names come from that same
//! call's report. A live session still re-runs — that is what the Debug button
//! asks for — but through `observe_sort`, so it re-runs the same code over the
//! same DAE.
//!
//! # The hazard this view exists to avoid, and where it is handled now
//!
//! Tearing speaks in **block-local** indices, so a step drawn against the
//! wrong block labels real tear variables with the wrong equations. Two things
//! could go wrong, and both are now settled upstream rather than inferred
//! here: a coupled block that is never torn (a non-square SCC is rejected
//! before the greedy search starts) used to shift every later block by one,
//! which `TearingStep::BlockStart`'s stated `ordinal` removes; and one block
//! yields **up to two passes** under different tear priorities, only one of
//! which is adopted, which `TearingStep::PlanSelected` names.

use eframe::egui;

use rumoca_phase_structural::{StructuralRecord, TearingRecord, TearingStep};

use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames. Slower than matching's cursor run:
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
/// replay runs them in order, so a frame has to say which block it is about.
#[derive(Debug, Clone)]
pub struct BlockFrame {
    /// Index into the animation's `blocks`.
    pub block: usize,
    pub frame: TearingRecord,
}

/// Replay of tearing — recorded or live.
pub struct TearingAnimation {
    playback: Playback<BlockFrame>,
    /// Names per coupled block, parallel to `BlockFrame::block`. Populated on
    /// construction for a recorded replay; for a live session it is built up
    /// front too, since running the blocks is cheap and only the *frames* need
    /// to arrive from the debugger thread.
    blocks: Vec<BlockNames>,
}

/// The coupled blocks' names, read from the structural report's JSON.
///
/// Shared by the recorded and live constructors so the two cannot label the
/// same blocks differently.
///
/// **The report's `blocks` array holds every block, scalar ones included, and
/// scalar blocks are never torn.** Filtering to `kind == "coupled"` first is
/// what keeps loop *i*'s reasoning off whatever block happened to sit at index
/// *i* — on a model with a scalar block before a coupled one that is silently
/// the wrong loop.
///
/// **A name this parser cannot read is refused, not dropped.**
/// `filter_map(|v| v.as_str())` silently removed any non-string member until
/// the 2026-08-04 sweep, so a coupled block of four equations could be drawn
/// and labelled as a block of three. Tearing is *about* which variables in a
/// block get torn, so a block with a member missing does not merely look
/// smaller — it makes the tear look like a different choice over a different
/// system. The caller (`App::structural_unavailable`) already knows how to say
/// a tearing animation is not available.
fn block_names_from_report(report: &serde_json::Value) -> Option<Vec<BlockNames>> {
    let names = |b: &serde_json::Value, key: &str| -> Option<Vec<String>> {
        let Some(raw) = b.get(key) else {
            // Absent is legitimate: a block need not list both sides.
            return Some(Vec::new());
        };
        raw.as_array()?
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<String>>>()
    };
    report
        .get("blocks")?
        .as_array()?
        .iter()
        .filter(|b| b.get("kind").and_then(serde_json::Value::as_str) == Some("coupled"))
        .map(|b| {
            Some(BlockNames {
                equations: names(b, "equations")?,
                unknowns: names(b, "unknowns")?,
            })
        })
        .collect()
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
        segments: &[Vec<TearingRecord>],
    ) -> Option<Self> {
        if segments.is_empty() {
            return None;
        }
        let blocks = block_names_from_report(report)?;

        // **Refuses rather than aligns.** A count mismatch means the capture and the
        // report describe different runs, and an animation that quietly pairs them
        // would be a wrong picture with no symptom.
        if blocks.len() != segments.len() {
            return None;
        }

        // **And the alignment is now checked against a stated ordinal, not
        // assumed from position.** Each segment opens with the `BlockStart`
        // Rumoca emitted for that coupled block, so segment *i* must declare
        // ordinal *i*. Before `BlockStart` existed, a coupled block that was
        // never torn — a non-square SCC, rejected before the greedy search
        // begins — contributed no segment, and the count check above passes
        // happily while every later block is off by one.
        if !segments.iter().enumerate().all(|(index, segment)| {
            matches!(
                segment.first().map(|record| &record.step),
                Some(TearingStep::BlockStart { ordinal, .. }) if *ordinal == index
            )
        }) {
            return None;
        }

        // Frames carry the block they belong to, so the flat playback the view
        // expects is rebuilt by tagging each segment with its index.
        let frames: Vec<BlockFrame> = segments
            .iter()
            .enumerate()
            .flat_map(|(block, seg)| {
                seg.iter().map(move |frame| BlockFrame {
                    block,
                    frame: frame.clone(),
                })
            })
            .collect();

        Some(Self {
            playback: Playback::recorded(frames, FRAME_INTERVAL),
            blocks,
        })
    }

    /// Start a live debug session: `sort`'s own tearing, on a thread that
    /// parks until the debugger attaches, pushing steps as they are produced.
    ///
    /// **Takes the report as well as the DAE since the v0.10.0 rebase**,
    /// because the renderer needs the block names from the first step onward
    /// and they are no longer recoverable by rebuilding the BLT here. They
    /// come from the same report the recorded path uses, so a live session and
    /// a replay label the blocks identically.
    ///
    /// Each step is tagged with the block whose `BlockStart` most recently
    /// went past — the ordinal the trace states, never a running count of
    /// pass groups.
    pub fn start_live(
        report: &serde_json::Value,
        dae: std::sync::Arc<rumoca_ir_dae::Dae>,
        frame_delay: std::time::Duration,
    ) -> Option<Self> {
        let blocks = block_names_from_report(report)?;
        let mut current = None;
        let (rx, done) = crate::live_trace::spawn_structural_live(
            dae,
            frame_delay,
            "tearing-debug",
            move |record| {
                let StructuralRecord::Tearing(frame) = record else {
                    return None;
                };
                if let TearingStep::BlockStart { ordinal, .. } = frame.step {
                    current = Some(ordinal);
                }
                // A step before any `BlockStart` cannot be placed, so it is
                // dropped rather than guessed onto block 0.
                Some(BlockFrame {
                    block: current?,
                    frame,
                })
            },
        )?;

        Some(Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
            blocks,
        })
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
        TearingStep::BlockStart {
            equations,
            unknowns,
            ..
        } => (
            "\u{1f3ac}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "An algebraic loop: {equations} equations in {unknowns} unknowns, none solvable \
                 on its own",
            ),
        ),
        // **The criterion lives here, not on `Torn`.** Rumoca plans the same
        // loop twice under two different greedy criteria and keeps the smaller
        // tear set, so "why that variable?" is answered by which pass is
        // running — which is why the old single-`Start` vocabulary could not
        // state it.
        TearingStep::PassStart { priority, n } => (
            "\u{1f3b2}",
            crate::colors::ANIM_EXPLORE,
            format!(
                "Planning the {n}-equation loop by {}",
                match priority {
                    rumoca_phase_structural::TearPriorityKind::CausalUnlocks =>
                        "cutting whichever unknown frees the most equations to be solved directly",
                    rumoca_phase_structural::TearPriorityKind::VariableDegree =>
                        "cutting whichever unknown appears in the most unsolved equations",
                },
            ),
        ),
        TearingStep::Torn {
            variable,
            appearances,
            remaining_equations,
        } => (
            "\u{2702}",
            crate::colors::ANIM_PATH_FOUND,
            // **No superlative, and that is a correction.** This read "more
            // than any other unknown", which is the `VariableDegree`
            // criterion stated as though it were the only one. Under
            // `CausalUnlocks` the winner maximises how many equations the cut
            // frees, and the most-appearances unknown routinely loses — so the
            // old sentence was false on whichever pass ran first. The count is
            // still worth showing; the claim about why it won belongs to
            // `PassStart`, which knows the criterion.
            format!(
                "Tore {} \u{2014} it appears in {appearances} of the {remaining_equations} \
                 unsolved equations",
                name_of(&names.unknowns, *variable),
            ),
        ),
        TearingStep::Causal {
            equation,
            variable,
            competitors,
        } => (
            "\u{2b07}",
            crate::colors::MATCHED_MARKER,
            // **`competitors` counts EQUATIONS, not unknowns** — how many
            // equations could have solved for this variable, with ties broken
            // toward the equation having fewer total unknowns. This used to
            // render it as `competitors + 1` behind the words "it had N before
            // the tears", which describes the equation's unknown count: a
            // different quantity entirely, so the number shown was unrelated
            // to the sentence containing it. Rumoca's own meaning is published
            // instead, and only when there was actually a contest.
            format!(
                "{} now solves {} \u{2014} it is the last unknown left there{}",
                name_of(&names.equations, *equation),
                name_of(&names.unknowns, *variable),
                if *competitors > 1 {
                    format!(
                        " ({competitors} equations could have solved for it; the one with fewer \
                         remaining unknowns won)"
                    )
                } else {
                    String::new()
                },
            ),
        ),
        // **Which plan the compiler kept.** Without this the pane shows two
        // tear sequences for one loop and says nothing about which survived,
        // so a reader would take the last one drawn — and that is whichever
        // pass ran second, not whichever won.
        TearingStep::PlanSelected {
            priority,
            tears,
            residuals,
        } => (
            "\u{1f3c1}",
            crate::colors::ANIM_PATH_FOUND,
            format!(
                "Adopted the plan from the {} pass: {tears} tear{} and {residuals} residual \
                 equation{}",
                match priority {
                    rumoca_phase_structural::TearPriorityKind::CausalUnlocks => "free-the-most",
                    rumoca_phase_structural::TearPriorityKind::VariableDegree => "most-appearances",
                },
                if *tears == 1 { "" } else { "s" },
                if *residuals == 1 { "" } else { "s" },
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
        // **Widened, so the old sentence had to go.** It said "no unknown
        // appears in any unsolved equation", which is one of three ways a pass
        // now ends with no plan — the others being a tear set that did not
        // shrink the problem and tear/residual counts that disagreed. Naming
        // only the first would assert a cause this step does not carry, and a
        // reader cannot tell which case they are looking at.
        TearingStep::NoProgress => (
            "\u{26a0}",
            crate::colors::ANIM_EXPLORE,
            "This pass found no usable plan \u{2014} either nothing was left to cut or cutting \
             did not shrink the problem. If the other pass also reports this, the whole block \
             goes to the solver untorn"
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

    let torn: Vec<String> = bf
        .frame
        .tears_so_far
        .iter()
        .map(|&v| name_of(&names.unknowns, v))
        .collect();
    let causal = bf.frame.causal_so_far.len();

    ui.add_space(4.0);
    ui.label(if torn.is_empty() {
        "Nothing torn yet.".to_owned()
    } else {
        format!("Torn so far ({}): {}", torn.len(), torn.join(", "),)
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
            frame: TearingRecord {
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
            TearingStep::Torn {
                variable: 0,
                appearances: 3,
                remaining_equations: 3,
            },
            &[0],
            &[],
        );
        let s = step_summary(&torn, &names());
        assert!(s.contains("command"), "{s}");
        assert!(s.contains('3'), "the appearance count is the reason: {s}");

        // **`competitors` counts EQUATIONS that could have solved for the
        // variable**, not unknowns in the equation. This asserted `competitors
        // + 1` behind the words "the crowd before the tears knocked them out",
        // which described the equation's unknown count — a different quantity,
        // so the number shown was unrelated to the sentence containing it.
        //
        // Both cases are pinned, because the pair is the claim: a contest is
        // reported with its size, and no contest is reported at all rather
        // than as a contest of one.
        let contested = bf(
            TearingStep::Causal {
                equation: 1,
                variable: 1,
                competitors: 3,
            },
            &[0],
            &[(1, 1)],
        );
        let s = step_summary(&contested, &names());
        assert!(s.contains("f_x[1]") && s.contains("error"), "{s}");
        assert!(
            s.contains('3'),
            "three equations could have solved for it, and that is the reason: {s}"
        );

        let uncontested = bf(
            TearingStep::Causal {
                equation: 1,
                variable: 1,
                competitors: 1,
            },
            &[0],
            &[(1, 1)],
        );
        let s = step_summary(&uncontested, &names());
        assert!(
            !s.contains("could have solved"),
            "one candidate is not a contest, and reporting it as one would invent \
             a tie that was never broken: {s}"
        );
    }

    /// Every step renders something, including the give-up path.
    #[test]
    fn every_step_renders() {
        for step in [
            TearingStep::BlockStart {
                ordinal: 0,
                equations: 3,
                unknowns: 3,
            },
            TearingStep::PassStart {
                priority: rumoca_phase_structural::TearPriorityKind::CausalUnlocks,
                n: 3,
            },
            TearingStep::PlanSelected {
                priority: rumoca_phase_structural::TearPriorityKind::VariableDegree,
                tears: 1,
                residuals: 1,
            },
            TearingStep::Torn {
                variable: 0,
                appearances: 3,
                remaining_equations: 3,
            },
            TearingStep::Causal {
                equation: 0,
                variable: 1,
                competitors: 0,
            },
            TearingStep::Complete {
                tears: 1,
                residuals: 1,
            },
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
            &bf(
                TearingStep::Torn {
                    variable: 99,
                    appearances: 1,
                    remaining_equations: 1,
                },
                &[],
                &[],
            ),
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
                    TearingStep::Causal {
                        equation: 1,
                        variable: 1,
                        competitors: 1,
                    },
                    &[0],
                    &[(1, 1)],
                )],
                FRAME_INTERVAL,
            ),
            blocks: vec![names()],
        };
        let ctx = anim
            .current_frame_context()
            .expect("a frame is under the cursor");
        assert_eq!(ctx["torn_so_far"], serde_json::json!(["command"]));
        assert_eq!(
            ctx["causal_so_far"],
            serde_json::json!(["f_x[1] solves error"])
        );
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

    /// End to end on a real coupled block: the compile must find
    /// `ProportionalLoop`'s 3x3 loop and report a `BlockStart`, at least one
    /// `Torn`, and the `PlanSelected` naming the plan it kept — the proof that
    /// the block-local translation lines up with what the algorithm expects.
    ///
    /// **Reads the compile rather than re-tearing.** This called
    /// `TearingAnimation::record`, which rebuilt incidence, matching, the
    /// dependency graph and the BLT and then tore every block again; that
    /// constructor is gone and the steps come from `inspect_structural_report`.
    /// `ProportionalLoop` is MSL-free, so no library roots are loaded.
    #[test]
    fn a_real_algebraic_loop_is_traced_end_to_end() {
        let specimen =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("specimens/ProportionalLoop.mo");
        let Ok(compiled) = crate::worker::compile_specimen(&specimen, Vec::new()) else {
            return; // specimen unavailable in this checkout
        };
        let crate::worker::FromWorker::Compiled {
            stages,
            tearing_frames,
            ..
        } = compiled
        else {
            panic!("ProportionalLoop must reach structural analysis");
        };
        let report = stages
            .get(crate::worker::StageKind::Structural)
            .value
            .clone()
            .expect("the Structural stage published a report");

        let anim = TearingAnimation::from_captured(&report, &tearing_frames)
            .expect("ProportionalLoop has a coupled block with tearing steps");
        assert_eq!(anim.blocks.len(), 1, "exactly one algebraic loop");
        assert_eq!(anim.blocks[0].unknowns.len(), 3, "a 3x3 loop");

        let steps: Vec<&TearingStep> = anim
            .playback
            .frames()
            .iter()
            .map(|bf| &bf.frame.step)
            .collect();
        // The block announces itself first — that ordinal is what keeps a
        // block's tears off another block's rows.
        assert!(
            matches!(
                steps.first(),
                Some(TearingStep::BlockStart {
                    ordinal: 0,
                    unknowns: 3,
                    ..
                })
            ),
            "{steps:?}"
        );
        assert!(
            steps.iter().any(|s| matches!(s, TearingStep::Torn { .. })),
            "something must get torn: {steps:?}",
        );
        // **`PlanSelected`, not `Complete`.** One loop yields up to two passes
        // and each successful pass reports its own `Complete`; only the
        // selection says which plan the compiler kept, so it is the last word.
        assert!(
            steps
                .iter()
                .any(|s| matches!(s, TearingStep::PlanSelected { .. })),
            "the adopted plan must be named: {steps:?}",
        );
    }
}
