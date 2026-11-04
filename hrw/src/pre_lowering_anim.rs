//! **MLS §3.7.5 history operators, and what each one became** — the re-model of
//! the old `pre()`-lowering animation, against v0.10.0's DAE construction.
//!
//! # What was here, and why this is a re-model rather than a port
//!
//! This module used to step through `lower_pre_operator(&mut dae::Dae)`: a pass
//! that walked a finished DAE and rewrote every `pre(x)` occurrence, emitting a
//! `PreLoweringStep` per rewrite. **Rumoca 0.10.0 has no such pass.** `pre()` is
//! lowered *while the expression arena is being built*, as one arm of
//! `construction::expression`'s builtin dispatch beside `der`, `edge`, `change`,
//! `previous` and `hold`. A `Dae` is immutable now, so a pass that rewrote one in
//! place could not exist even in principle.
//!
//! # What replaced it, and why it needed no instrumentation
//!
//! The predecessor's doc comment predicted this re-model would have to read
//! construction through a new seam. **It does not**, and that was measured rather
//! than assumed: the information construction produces *survives into the DAE*,
//! as provenance on the expression nodes. Every node the history-operator
//! lowering synthesised carries
//! [`rumoca_ir_dae::DaeGeneration::PreValueLowering`], and every `pre()` read
//! lands on one of four coordinate lanes. So this view is built from the
//! finished artifact, with no `crates/` change at all.
//!
//! # The thing the old pass could not show
//!
//! `edge` and `change` are **not primitives** — MLS §3.7.5 defines them in terms
//! of `pre`, and the IR carries the expansion. Measured on a probe with one
//! `edge(b)`, one `change(b)` and two hand-written `pre()` reads:
//!
//! | site | nodes construction created, innermost first |
//! |---|---|
//! | `edge(b)` | `pre(b)`, `not pre(b)`, **`b and not pre(b)`** |
//! | `change(b)` | `pre(b)`, **`b <> pre(b)`** |
//! | `pre(counted)` | `pre(counted)` — one node, nothing synthesised |
//! | `pre(x)` | `pre(x)` — one node |
//!
//! **The two kinds are told apart by provenance, not by shape**: a `pre()` the
//! model wrote is `Source`, and a `pre()` the compiler synthesised while
//! expanding `edge` is `Generated(PreValueLowering)`. That distinction is the
//! reason this pane can claim `edge(b)` *became* `b and not pre(b)` rather than
//! merely noting that both appear.
//!
//! # And the lane, which is a semantic claim rather than a label
//!
//! `pre()` resolves to one of `PreDiscreteReal`, `PreDiscreteValue`, `PreState`
//! or `PreAlgebraic`. The last two exist because a continuous state or algebraic
//! gets **its own event-entry snapshot lane**, which is what makes
//! `y = f*pre(x); reinit(x, 0)` in one `when` body read the accumulated `x`
//! rather than the reinitialized one. `Coordinate::previous` in
//! `rumoca-phase-dae` carries that reasoning; the pane shows which lane each
//! read came from so the claim is checkable against a model.
//!
//! # What is still missing, and it is live stepping rather than the view
//!
//! The old view could run live, by re-running `to_dae_with_options_traced` over
//! the flat model. That function does not exist, and re-running `Dae::construct`
//! would report nothing without a tracing seam inside `lower_history_operator`.
//!
//! **So this view has no live path at all, and says so by not having one.** The
//! `start_live` that returned `None` unconditionally is gone, and so is
//! `app.rs`'s live-debug gate around it — a Debug button that arms a breakpoint,
//! waits and reports nothing is a false affordance, and
//! `playback::tests_animated_contract::only_the_views_with_a_live_path_report_a_live_session`
//! is what refused the alternative: a view handed a live path must delegate
//! `live_state` to its `Playback`, and one that delegates while holding only
//! recorded frames answers `Arming` whenever any session is armed anywhere.
//!
//! The frames here are playback of what the compile produced — the correct
//! category, not a re-execution presented as the compile. The seam live stepping
//! would need is recruited by its own `unbuilt:` tag in `docs/tech-debt.md`.

use eframe::egui;
use serde_json::Value;

use crate::LiveState;
use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames.
///
/// Matched to `alias_anim`'s rather than to the decision-making views': a frame
/// here is one node of an expansion, three at most, and it reads as a stream
/// rather than as a choice being made.
const FRAME_INTERVAL: f64 = 0.45;

/// What the pane says when the model uses no history operator.
///
/// **Named rather than inlined so a test can pin it.**
/// `doc_citations::every_absence_message_a_pane_shows_is_named_by_a_test`
/// exists because a pane that says the wrong thing when it has nothing to show
/// is well-formed and invisible to every other check — so the sentence is as
/// much a claim as a number is, and `the_empty_pane_says_what_is_absent`
/// holds this one.
const EMPTY_MESSAGE: &str = "No history operators in this model.";

/// One MLS §3.7.5 history-operator site, and the expression nodes it produced.
///
/// Built by `worker::pre_lowering_from_construction` from a finished DAE.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreLoweringSite {
    /// The source text Rumoca attached as the owner span: `edge(b)`,
    /// `change(b)`, `pre(x)`.
    ///
    /// **Read from the span, never matched by name.** The span is the compiler's
    /// own claim about which call owns these nodes; the text is only its label.
    pub call: String,
    /// 1-based source line of the site, or `None` for a span in another file.
    pub line: Option<usize>,
    /// Which of the four `pre` lanes the read resolves to, as Rumoca spells it.
    ///
    /// `None` when no node in the site is a `pre` coordinate reference, which
    /// should not happen for a history operator and is published as absence
    /// rather than guessed at.
    pub lane: Option<String>,
    /// `true` when construction synthesised these nodes (an `edge`/`change`
    /// expansion), `false` when the model wrote the `pre()` read itself.
    pub generated: bool,
    /// Each node in the order construction created it, innermost first, rendered
    /// as the term it is. **The last entry is the whole site's value.**
    ///
    /// The order is provable rather than assumed: a parent node cannot reference
    /// a child that does not exist yet, so within one site the node with the
    /// highest arena index is the root.
    pub steps: Vec<String>,
}

impl PreLoweringSite {
    /// The term this site finally evaluates to.
    ///
    /// Separate from indexing `steps` so the pane and a capture cannot disagree
    /// about which entry is the answer.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.steps.last().map(String::as_str)
    }
}

/// One step of the animation.
///
/// **Not named `PreLoweringStep`**, which was 0.9.20's type for one rewrite of a
/// pass that no longer exists; reusing the name would make this module's own
/// account of what went away read as a claim about what is here.
///
/// A `Node` carries indices rather than copies of the text, so the pane can show
/// a site's whole expansion with the cursor's step marked without every frame
/// carrying a clone of the list it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryStep {
    /// The model before any history operator is lowered.
    ///
    /// **The opening frame every animated view owes**, per
    /// `playback::tests_animated_contract::every_animated_view_renders_an_opening_frame`:
    /// a reader's first frame must be the state the algorithm was handed, not
    /// its first result. It also carries the site count, so the frame total is
    /// **one more than the number of nodes** — anything reporting a total of
    /// nodes must subtract it, the same arithmetic `alias_anim` documents.
    Start,
    /// Node `step` of site `site`.
    Node {
        /// Index into [`PreLoweringAnimation::sites`].
        site: usize,
        /// Index into that site's `steps`.
        step: usize,
    },
}

/// Playback over every history-operator lowering the compile performed.
pub struct PreLoweringAnimation {
    /// The sites, in construction order.
    sites: Vec<PreLoweringSite>,
    /// `[Start, Node…]` — one frame per node after the opening one, so stepping
    /// walks the expansions the way construction built them.
    playback: Playback<HistoryStep>,
}

impl PreLoweringAnimation {
    /// Build from the sites the compile produced.
    ///
    /// **An empty site list is a real answer**, not a failure: most models use no
    /// history operator at all, and the pane says so in those words rather than
    /// looking like a view that failed to load.
    #[must_use]
    pub fn from_sites(sites: Vec<PreLoweringSite>) -> Self {
        // **No opening frame when there is nothing to open.** An empty model
        // would otherwise report one frame, and a reader stepping to it would
        // see "0 sites to lower" presented as a step of an algorithm that never
        // ran. `is_empty` and the frame count must agree.
        let frames: Vec<HistoryStep> = if sites.is_empty() {
            Vec::new()
        } else {
            std::iter::once(HistoryStep::Start)
                .chain(sites.iter().enumerate().flat_map(|(site, s)| {
                    (0..s.steps.len()).map(move |step| HistoryStep::Node { site, step })
                }))
                .collect()
        };
        Self {
            sites,
            playback: Playback::recorded(frames, FRAME_INTERVAL),
        }
    }

    /// Whether the compile found any history-operator site.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    /// The sites, for a test or a capture that wants the whole list.
    #[must_use]
    pub fn sites(&self) -> &[PreLoweringSite] {
        &self.sites
    }

    /// The site under the cursor, and which of its nodes.
    ///
    /// `None` on the opening frame, which is about the model rather than about
    /// any one site.
    fn current_site(&self) -> Option<(&PreLoweringSite, usize)> {
        match self.playback.current()? {
            HistoryStep::Start => None,
            HistoryStep::Node { site, step } => Some((self.sites.get(*site)?, *step)),
        }
    }

    /// What the opening frame says: the whole job, before any of it is done.
    fn start_summary(&self) -> String {
        let expansions = self.sites.iter().filter(|s| s.generated).count();
        format!(
            "{} history-operator site(s): {expansions} expanded by construction, \
             {} written as pre() by the model",
            self.sites.len(),
            self.sites.len() - expansions,
        )
    }

    /// Draw the view.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if self.sites.is_empty() {
            ui.label(EMPTY_MESSAGE);
            ui.weak(
                "Nothing here uses pre(), edge() or change(), so DAE construction \
                 had no event history to lower. A model with no when-clause \
                 reaching backwards in time is the common case.",
            );
            return;
        }

        let live = LiveState::Idle;
        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }
        let _ = crate::animation_controls(ui, self.playback.controls(), live, false);

        ui.add_space(4.0);
        self.render_current(ui);
        ui.add_space(8.0);
        self.render_all_sites(ui);
    }

    /// The site under the cursor, with the current step marked.
    fn render_current(&self, ui: &mut egui::Ui) {
        let Some((site, step)) = self.current_site() else {
            // The opening frame. A clapper board rather than a flag: nothing at
            // the head of a replay may read as an ending.
            if self.playback.current() == Some(&HistoryStep::Start) {
                ui.horizontal_wrapped(|ui| {
                    ui.colored_label(crate::colors::ANIM_EXPLORE, "\u{1f3ac}");
                    ui.label(egui::RichText::new(self.start_summary()).strong());
                });
            }
            return;
        };

        // A synthesised expansion and a hand-written read are different claims,
        // so they get different icons and different headline text rather than one
        // phrasing that would have to be vague enough to cover both.
        let (icon, color, headline) = if site.generated {
            (
                "\u{1f500}",
                crate::colors::ANIM_PATH_FOUND,
                format!("{} expanded into {} node(s)", site.call, site.steps.len()),
            )
        } else {
            (
                "\u{23ee}",
                crate::colors::ANIM_EXPLORE,
                format!("{} reads the event-entry value", site.call),
            )
        };
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(color, icon);
            ui.label(egui::RichText::new(headline).strong());
        });
        if let Some(line) = site.line {
            ui.weak(format!("source line {line}"));
        }
        if let Some(lane) = &site.lane {
            ui.weak(format!("lane: {lane}"));
        }

        ui.add_space(4.0);
        for (i, term) in site.steps.iter().enumerate() {
            let marker = if i == step { "\u{25b6}" } else { " " };
            let text = egui::RichText::new(format!("{marker} {term}")).monospace();
            if i == step {
                ui.colored_label(color, text);
            } else {
                ui.weak(text);
            }
        }
    }

    /// Every site, so the cursor's position is legible against the whole run.
    fn render_all_sites(&self, ui: &mut egui::Ui) {
        let current = match self.playback.current() {
            Some(HistoryStep::Node { site, .. }) => Some(*site),
            _ => None,
        };
        ui.label(
            egui::RichText::new(format!("{} history-operator site(s)", self.sites.len())).strong(),
        );
        for (i, site) in self.sites.iter().enumerate() {
            let value = site.value().unwrap_or("<no nodes>");
            let line = site
                .line
                .map_or_else(String::new, |line| format!("  (line {line})"));
            let text =
                egui::RichText::new(format!("{}  \u{2192}  {value}{line}", site.call)).monospace();
            if current == Some(i) {
                ui.colored_label(crate::colors::ANIM_PATH_FOUND, text);
            } else {
                ui.weak(text);
            }
        }
    }
}

impl Animated for PreLoweringAnimation {
    fn which(&self) -> &'static str {
        "pre_lowering"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, _arming: bool) -> LiveState {
        // Never live: there is no pass to re-run, and no seam inside
        // construction to trace. See the module header.
        LiveState::Idle
    }

    /// `None` before a frame is under the cursor, which the trait defines as a
    /// real state rather than a failure — and here it is also the answer for a
    /// model with no history operator, where there is nothing to describe.
    fn current_frame_context(&self) -> Option<Value> {
        let Some((site, step)) = self.current_site() else {
            // The opening frame describes the model, not a site. Captured as its
            // own shape rather than omitted: a capture that returned `None` here
            // would read as "no frame", which is what an empty model reports.
            return match self.playback.current()? {
                HistoryStep::Start => Some(serde_json::json!({
                    "step": self.start_summary(),
                    "sites_total": self.sites.len(),
                })),
                HistoryStep::Node { .. } => None,
            };
        };
        Some(serde_json::json!({
            "call": site.call,
            "line": site.line,
            "lane": site.lane,
            // **Rumoca's own distinction, published as the field it is.** A pane
            // that said only "lowering" would make `edge(b)`'s synthesised
            // `pre(b)` indistinguishable from one the model wrote.
            "generated": site.generated,
            "step": step,
            "term": site.steps.get(step),
            "steps": site.steps,
            "value": site.value(),
            "site": match self.playback.current() {
                Some(HistoryStep::Node { site, .. }) => Some(*site),
                _ => None,
            },
            "sites_total": self.sites.len(),
        }))
    }

    /// Seek is delegated to [`Playback`], so every view agrees on what a frame
    /// index means and on refusing an out-of-range one.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(call: &str, generated: bool, steps: &[&str]) -> PreLoweringSite {
        PreLoweringSite {
            call: call.to_owned(),
            line: Some(7),
            lane: Some("PreDiscreteValue".to_owned()),
            generated,
            steps: steps.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// **One frame per node, across sites, plus the opening frame** — the
    /// flattening a capture's frame index depends on.
    ///
    /// The `+ 1` is pinned rather than derived, because it is the arithmetic
    /// `alias_anim` documents as the thing a total gets wrong: a view reporting
    /// "4 of 4 nodes" when there are four nodes and five frames is describing a
    /// list that does not exist.
    #[test]
    fn frames_walk_every_node_of_every_site_after_an_opening_frame() {
        let anim = PreLoweringAnimation::from_sites(vec![
            site(
                "edge(b)",
                true,
                &["pre(b)", "not pre(b)", "b and not pre(b)"],
            ),
            site("pre(x)", false, &["pre(x)"]),
        ]);
        let (_, total) = anim.position();
        assert_eq!(total, 5, "one opening frame, then three nodes, then one");
        assert!(!anim.is_empty());

        // The opening frame is about the model, so it carries a context of its
        // own rather than reporting a site.
        let context = anim
            .current_frame_context()
            .expect("the cursor starts on the opening frame");
        assert_eq!(context["sites_total"], 2);
        assert!(
            context["call"].is_null(),
            "the opening frame names no site: {context}",
        );
        assert!(
            context["step"]
                .as_str()
                .is_some_and(|s| s.contains("1 expanded") && s.contains("1 written")),
            "and it says what the run is about to do, split by who wrote each \
             site: {context}",
        );
    }

    /// A seek lands on the node it names, and refuses one past the end.
    #[test]
    fn seek_lands_on_the_named_node_and_refuses_a_missing_one() {
        let mut anim = PreLoweringAnimation::from_sites(vec![
            site(
                "edge(b)",
                true,
                &["pre(b)", "not pre(b)", "b and not pre(b)"],
            ),
            site("pre(x)", false, &["pre(x)"]),
        ]);
        // Frame 3, not 2: frame 0 is the opening frame, so the first site's
        // three nodes are frames 1, 2 and 3.
        assert!(anim.seek(3), "the third node of the first site exists");
        let context = anim.current_frame_context().expect("a frame is under it");
        assert_eq!(context["term"], "b and not pre(b)");
        assert_eq!(context["site"], 0);
        assert_eq!(context["generated"], true);

        assert!(
            anim.seek(4),
            "the fifth frame is the second site's one node"
        );
        let context = anim.current_frame_context().expect("a frame is under it");
        assert_eq!(context["call"], "pre(x)");
        assert_eq!(
            context["generated"], false,
            "a read the model wrote is not an expansion, and the capture must \
             carry the difference",
        );

        assert!(!anim.seek(5), "there is no sixth frame");
    }

    /// **No sites is a report, not a gap** — the non-vacuity guard for the rest.
    #[test]
    fn an_empty_run_reports_itself() {
        let anim = PreLoweringAnimation::from_sites(Vec::new());
        assert!(anim.is_empty());
        assert_eq!(anim.position(), (0, 0));
        assert!(
            anim.current_frame_context().is_none(),
            "no frame means no context, which the trait defines as a real state",
        );
    }

    /// **The empty pane names what is absent, and names it correctly.**
    ///
    /// A pane with nothing to show still makes a claim, and this one is easy to
    /// get wrong in a way nothing else would catch: the old placeholder said
    /// `pre()` lowering was *being rebuilt*, which was true of HRW and read as a
    /// statement about the compiler. The sentence must say that **the model** has
    /// no history operator — not that the view has no data, and not that Rumoca
    /// did not lower one.
    #[test]
    fn the_empty_pane_says_what_is_absent() {
        assert_eq!(
            EMPTY_MESSAGE, "No history operators in this model.",
            "the absence message is a claim about the model",
        );
        assert!(
            !EMPTY_MESSAGE.contains("rebuilt") && !EMPTY_MESSAGE.contains("not built"),
            "an absence message must not describe HRW's own state: a reader \
             cannot tell that apart from a claim about the compiler",
        );
    }

    /// The value is the last node, and `value()` is the only place that decides.
    #[test]
    fn the_sites_value_is_its_root_node() {
        let s = site(
            "edge(b)",
            true,
            &["pre(b)", "not pre(b)", "b and not pre(b)"],
        );
        assert_eq!(s.value(), Some("b and not pre(b)"));
        let empty = site("edge(b)", true, &[]);
        assert_eq!(empty.value(), None, "absence rather than a placeholder");
    }
}
