//! Index-reduction stepping — the search, one observed record at a time.
//!
//! ## Why this phase is worth stepping through
//!
//! The [summary panel](crate::reduction_view) shows the whole search at once, which is
//! the right shape for *what happened*. What a table cannot give is the **accumulation**:
//! watching a residue come down round by round, seeing a lane find nothing and hand
//! over to the other, and meeting each refusal at the moment it was made rather than as
//! one row among forty.
//!
//! `GearWithBrake` logs 40 records and `Drivetrain` 29. Read as a table they are a wall;
//! read as a sequence they are a search with a plot.
//!
//! ## What kind of animation this is, and the one it is not
//!
//! **A replay of a recorded search, not a re-derivation.** The frames are
//! `worker::index_reduction_from_observation`'s published records, which came from
//! Rumoca's own observation of the one reduction this compile performed. Stepping moves
//! a cursor over them; nothing is recomputed, and there is no second implementation that
//! could drift from the first.
//!
//! **And there is also a live path, which is a different thing from the replay above.**
//! [`ReductionAnimation::start_live`] spawns a real run through
//! [`crate::live_trace::spawn_reduction_live`] and paces it against a breakpoint, so its
//! frames arrive as the algorithm produces them rather than being read off a finished
//! report. Both are legitimate, and `CLAUDE.md` says which is not: feeding an
//! already-completed report into the frame channel on a timer would be **a recording
//! wearing a live label**, the fiction this repository has removed twice.
//!
//! **The seam that makes it real is ours and is additive**: `watch_prepare_for_solve`
//! and the `ReductionWatcher` trait in `rumoca-phase-structural`, observation-only and
//! committed separately from `hrw/` so an upstream cherry-pick stays clean. Before it
//! existed, `start_live` returned `None` unconditionally and this pane's Debug button
//! armed a breakpoint that reported nothing — the same symptom PreLowering had, but for
//! the opposite reason: the seam here was merely *private*, not absent. `CLAUDE.md`
//! carries that distinction, because from the HRW side the two look identical.
//!
//! So this view is **not** exempt from
//! `playback::tests_animated_contract::only_the_views_with_a_live_path_report_a_live_session`:
//! `DISABLED_VIEWS` is empty, and a `start_live` that went back to returning `None`
//! would now fail that test by name rather than passing quietly.
//!
//! ## What replaced the funnel
//!
//! This module used to step through a **ten-step dummy-derivative funnel**:
//! `demote_exact_alias_component_states`, `reduce_constrained_dummy_derivatives` and
//! eight more, each mutating a `&mut Dae` in place with HRW's own instrumentation
//! emitting a frame per step. **Rumoca 0.10.0 does not run that algorithm** — the DAE is
//! immutable and reduction is two fixed-point lanes driven by a
//! candidate/attempt/selection protocol — so every name it animated stopped existing and
//! the shape it drew was not the shape of the search. The implementation is in git
//! history, which is the right place for the record of an algorithm Rumoca used to run.

use eframe::egui;
use serde_json::Value;

use crate::LiveState;
use crate::playback::{Animated, Playback};

/// Seconds between auto-advance frames.
///
/// Slower than `alias_anim`'s old 0.45s reveal and nearer the decision-making views:
/// every frame here is a choice or a measurement, not an item in a stream.
const FRAME_INTERVAL: f64 = 0.7;

/// One step of the search.
///
/// **`Start` is the opening frame — nothing tried, nothing selected.** Every view fed by
/// a Rumoca capture carries one, after Doug reported on 2026-08-23 that two views
/// "open to frame 1 with progress already having been made". A log's first record is
/// already a round *in progress*; the state before it is the system as reduction was
/// handed it, and that is what a reader needs first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReductionStep {
    /// Before any record. What it needs to say — the system's size and the residue the
    /// search began from — the animation reads off the payload it holds.
    Start,
    /// One observed record, as the stage published it.
    ///
    /// **The record itself, not an index into a vector**, because a live session has no
    /// vector: records arrive one at a time down a channel. Carrying it in the frame is
    /// what lets the recorded and live paths share every accessor below instead of
    /// having two of each.
    Record(Value),
}

/// Replay of the reduction search.
pub struct ReductionAnimation {
    /// The raw system's size, for the opening frame.
    ///
    /// `None` for a live session, which has no published payload to read it from.
    before: Option<(u64, u64, u64)>,
    /// The residue the first round reported, which is where the search began.
    opening_residue: Option<u64>,
    /// `[Start, Record(0), …]` — **one longer than the record count**, so anything
    /// reporting a total must subtract the opening frame.
    playback: Playback<ReductionStep>,
}

impl ReductionAnimation {
    /// Build from an index-reduction stage report.
    ///
    /// `None` when the payload carries no `search` array — a stage that was never
    /// reached. A stage that *was* reached always has one, even when the run refused
    /// the system: `CartesianPendulum`'s refusal is the most interesting log in the
    /// corpus, three alternations finding nothing.
    #[must_use]
    pub fn from_report(report: &Value) -> Option<Self> {
        let records: &Vec<Value> = report.get("search")?.as_array()?;
        // **No opening frame when there is nothing to open.** A lone `Start` would make
        // `is_empty` false for a system nothing was done to, and the pane would offer a
        // run through nothing instead of saying so.
        let frames: Vec<ReductionStep> = if records.is_empty() {
            Vec::new()
        } else {
            std::iter::once(ReductionStep::Start)
                .chain(records.iter().cloned().map(ReductionStep::Record))
                .collect()
        };
        let before = (|| {
            let b = report.get("before")?;
            Some((
                b["n_states"].as_u64()?,
                b["n_algebraics"].as_u64()?,
                b["n_equations"].as_u64()?,
            ))
        })();
        let opening_residue = records
            .iter()
            .find(|r| r["kind"] == "round")
            .and_then(|r| r["residue"].as_u64());
        Some(Self {
            before,
            opening_residue,
            playback: Playback::recorded(frames, FRAME_INTERVAL),
        })
    }

    /// Run reduction on a worker thread and step its records as they arrive.
    ///
    /// **A real live session since 2026-11-04**, when `watch_prepare_for_solve` landed
    /// in `rumoca-phase-structural`. It returned `None` unconditionally before that,
    /// because the only crate-external surface handed back a finished report and
    /// replaying one on a timer would have been a recording wearing a live label.
    ///
    /// `None` when the thread cannot be spawned, which is the answer `app.rs` already
    /// handles: it releases the breakpoint and falls back to the recorded view.
    ///
    /// # What a live session has that the recorded one does not
    ///
    /// The same records in the same order — so nothing here is *more true* than the
    /// replay. What it adds is a **breakpoint that fires inside the real reduction**:
    /// the thread waits for the debugger before the first step and pauses between
    /// pushes, so `docs/setup-windows.md`'s anchor can stop in `dae_transform.rs` with
    /// a candidate half-attempted and the locals live. That is the thing a trace cannot
    /// give, and the reason `CLAUDE.md` protects this path.
    #[must_use]
    pub fn start_live(
        dae: &std::sync::Arc<rumoca_ir_dae::Dae>,
        frame_delay: std::time::Duration,
    ) -> Option<Self> {
        // Every record is a frame: unlike the structural views, there is no sub-kind to
        // filter — the reduction's log is already exactly this view's subject.
        // **Each record is published the same way the stage publishes it**, through
        // `worker::reduction_record_to_json` and the compile's own source map. A second
        // rendering here would be a second vocabulary for one thing, and a reader
        // stepping live would see different words than the replay gives.
        let dae_for_records = std::sync::Arc::clone(dae);
        let (rx, done) = crate::live_trace::spawn_reduction_live(
            std::sync::Arc::clone(dae),
            frame_delay,
            "reduction-debug",
            move |record| {
                Some(ReductionStep::Record(
                    crate::worker::reduction_record_to_json(&record, dae_for_records.source_map()),
                ))
            },
        )?;
        Some(Self {
            // **No opening frame and no `before` size for a live session.** Both are
            // read off the published payload, which this path does not have — and
            // inventing a `Start` here would claim a residue nobody reported. The
            // recorded view is where the opening state comes from.
            before: None,
            opening_residue: None,
            playback: Playback::live(rx, done, FRAME_INTERVAL),
        })
    }

    /// Whether there is anything to step through.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// Records in the log.
    ///
    /// **Counted rather than derived from the frame count**, because the two paths
    /// differ by one: a recorded run opens on `Start` and a live one has no opening
    /// frame to read a size from. Counting the `Record` frames is right for both.
    #[must_use]
    pub fn n_records(&self) -> usize {
        self.records_through(usize::MAX).count()
    }

    /// The `Record` frames up to and including frame `cursor`.
    ///
    /// One helper for every accumulation below, so none of them can disagree about
    /// which frames count as "so far".
    fn records_through(&self, cursor: usize) -> impl Iterator<Item = &Value> {
        self.playback
            .frames()
            .iter()
            .take(cursor.saturating_add(1))
            .filter_map(|step| match step {
                ReductionStep::Start => None,
                ReductionStep::Record(record) => Some(record),
            })
    }

    /// The record under the cursor, or `None` at the opening frame.
    fn current_record(&self) -> Option<&Value> {
        match self.playback.current()? {
            ReductionStep::Start => None,
            ReductionStep::Record(record) => Some(record),
        }
    }

    /// How many rounds have begun at or before the cursor.
    ///
    /// The accumulation the stepping exists for: at the opening frame none has, and at
    /// frame *k* exactly those among the first *k* records.
    fn rounds_so_far(&self) -> usize {
        self.records_through(self.playback.cursor())
            .filter(|r| r["kind"] == "round")
            .count()
    }

    /// The residue most recently reported at or before the cursor.
    ///
    /// **Read from the log, never derived.** Subtracting selections from an opening
    /// number would understate progress whenever a `Held` selection moved nothing, and
    /// a reader would have no way to tell.
    fn residue_now(&self) -> Option<u64> {
        let seen: Vec<&Value> = self.records_through(self.playback.cursor()).collect();
        seen.into_iter()
            .rev()
            .find_map(|r| match r["kind"].as_str() {
                Some("round") => r["residue"].as_u64(),
                Some("selected") => r["residue_after"].as_u64(),
                Some("attempt") => r["outcome"]["residue"].as_u64(),
                _ => None,
            })
            .or(self.opening_residue)
    }

    /// Draw the stepper. Returns whether the Debug button was clicked.
    pub fn ui(&mut self, ui: &mut egui::Ui, arming: bool, debug_enabled: bool) -> bool {
        // **First, always.** Frames arrive on the worker thread and nothing else moves
        // them into the playback; `tick` only advances a *recorded* cursor. Omitting this
        // is what Doug reported on 2026-11-04 — the breakpoint fires in the real
        // reduction, Continue re-fires it, and the pane never moves, because the channel
        // fills while the view redraws the frame it started on.
        //
        // `connection_anim::ui` carries the same line and a comment saying it had
        // happened before. **I wrote this function without it anyway**, which is what a
        // rule living only next door to the code it governs costs — see
        // `playback::tests_animated_contract::every_live_capable_view_drains_its_channel`,
        // which is the version that cannot be skipped.
        self.playback.sync_live();

        // **Empty is not the same as not-yet-live.** A live session begins with no frames
        // at all — the worker is waiting at the startup gate for the reader to arrive — so
        // bailing out on `is_empty` alone hides the controls exactly when the reader needs
        // to see the session is armed. The second half of the same report.
        if self.playback.is_empty() && !self.playback.is_live() && !arming {
            // **Reached only by an empty `search` log**, and that is a stage that RAN: a
            // stage that was not reached has no `search` key, so `from_report` returns
            // `None` and this view never exists. Every specimen that reaches the stage
            // records at least one step (measured from the committed traces,
            // 2026-11-07). This said *"The stage was not reached"* until then, which is
            // the one case that cannot get here.
            ui.label("No reduction search in this report.");
            ui.weak(
                "The search log is empty, so there is no step to show \u{2014} the stage \
                 tab's note says what the stage reported.",
            );
            return false;
        }

        let live = self.live_state(arming);
        let dt = ui.input(|i| i.stable_dt) as f64;
        if self.playback.tick(dt, live) {
            ui.ctx().request_repaint();
        }
        let debug_clicked =
            crate::animation_controls(ui, self.playback.controls(), live, debug_enabled);

        ui.add_space(4.0);
        self.render_current(ui);
        ui.add_space(8.0);
        self.render_running_state(ui);
        debug_clicked
    }

    /// The frame under the cursor, in words.
    fn render_current(&self, ui: &mut egui::Ui) {
        let Some(record) = self.current_record() else {
            ui.label(egui::RichText::new("Before the search").strong());
            match (self.before, self.opening_residue) {
                (Some((states, algebraics, equations)), Some(residue)) => ui.label(format!(
                    "{states} state(s), {algebraics} algebraic(s), {equations} equation(s); \
                     residue {residue}"
                )),
                (Some((states, algebraics, equations)), None) => ui.label(format!(
                    "{states} state(s), {algebraics} algebraic(s), {equations} equation(s)"
                )),
                // Absent rather than zero: a size nobody published is not a size of 0.
                _ => ui.weak("The payload carries no size for the raw system."),
            };
            return;
        };

        let (icon, strong, summary) = step_style(record);
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(icon).size(16.0));
            let text = egui::RichText::new(summary);
            ui.label(if strong { text.strong() } else { text });
        });

        // The unmatched names are the evidence for a round's residue, so a round frame
        // shows them in full rather than as a count.
        if record["kind"] == "round" {
            for (label, key) in [
                ("equations", "unmatched_equations"),
                ("unknowns", "unmatched_unknowns"),
            ] {
                let names: Vec<String> = record[key]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .map(|v| match v["name"].as_str() {
                                Some(n) => n.to_owned(),
                                None => v.as_str().unwrap_or("(unnamed)").to_owned(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                if !names.is_empty() {
                    ui.add_space(2.0);
                    // No nested scroll area: the pane already scrolls.
                    ui.weak(format!("unmatched {label}: {}", names.join(", ")));
                }
            }
        }

        if let Some(at) = record["identity"]["declared_at"]["file"].as_str() {
            let line = record["identity"]["declared_at"]["line"]
                .as_u64()
                .map(|l| format!(":{l}"))
                .unwrap_or_default();
            ui.add_space(2.0);
            ui.weak(format!("at {at}{line}"));
        }
    }

    /// What has accumulated by this frame.
    fn render_running_state(&self, ui: &mut egui::Ui) {
        let (cursor, total) = self.playback.position();
        ui.weak(format!(
            "record {cursor} of {}  \u{2022}  {} round(s) begun{}",
            total.saturating_sub(1),
            self.rounds_so_far(),
            match self.residue_now() {
                Some(r) => format!("  \u{2022}  residue {r}"),
                None => String::new(),
            },
        ));
    }
}

/// An icon, whether the line is the frame's headline, and the line itself.
///
/// **Rumoca's own words for the record kinds and outcomes**, with a gloss only where
/// the bare name misleads — `held` especially, which is progress and reads as failure.
fn step_style(record: &Value) -> (&'static str, bool, String) {
    let lane = record["lane"].as_str().unwrap_or("?");
    match record["kind"].as_str().unwrap_or("?") {
        "round" => (
            "\u{25cf}",
            true,
            format!(
                "Round {} [{lane}] \u{2014} residue {}",
                record["round"], record["residue"],
            ),
        ),
        "candidates" => {
            let discovered = record["discovered"].as_u64().unwrap_or(0);
            let group = record["group"].as_str().unwrap_or("?");
            if discovered == 0 {
                (
                    "\u{25cb}",
                    false,
                    format!(
                        "[{lane}] {group}: nothing to try \u{2014} no candidate was \
                         admissible, which is not a refusal"
                    ),
                )
            } else {
                (
                    "\u{25cb}",
                    false,
                    format!("[{lane}] {group}: {discovered} candidate(s) found"),
                )
            }
        }
        "attempt" => {
            let form = record["identity"]["form"].as_str().unwrap_or("?");
            let outcome = record["outcome"]["kind"].as_str().unwrap_or("?");
            let residue = record["outcome"]["residue"]
                .as_u64()
                .map(|r| format!(" \u{2192} residue {r}"))
                .unwrap_or_default();
            let gloss = match outcome {
                "held" => " (the residue held and the system changed \u{2014} progress)",
                "raised" => " (the residue grew, so this was not taken)",
                "sorted" => " (this one sorted the system)",
                _ => "",
            };
            (
                "\u{2192}",
                false,
                format!("Tried {form} [{lane}]: {outcome}{residue}{gloss}"),
            )
        }
        "selected" => {
            let form = record["identity"]["form"].as_str().unwrap_or("?");
            let after = record["residue_after"]
                .as_u64()
                .map(|r| r.to_string())
                .unwrap_or_else(|| "sorted".to_owned());
            (
                "\u{2714}",
                true,
                format!(
                    "Selected {form} [{lane}]: residue {} \u{2192} {after}",
                    record["residue_before"],
                ),
            )
        }
        "retried_pristine" => (
            "\u{21ba}",
            false,
            format!("[{lane}] restarted from the unmodified system"),
        ),
        "stopped" => {
            let kind = record["outcome"]["kind"].as_str().unwrap_or("?");
            (
                "\u{25a0}",
                true,
                match record["outcome"]["residue"].as_u64() {
                    Some(r) => format!("Stopped: {kind}, residue {r}"),
                    None => format!("Stopped: {kind}"),
                },
            )
        }
        other => (
            "?",
            false,
            // **Named rather than skipped.** An upstream sixth record variant would
            // otherwise step past silently, and the search would read shorter than it was.
            format!("a record of kind `{other}`, which this view does not know how to show"),
        ),
    }
}

/// The frame's `step` value for the capture: the record's own kind.
fn step_name(record: Option<&Value>) -> String {
    match record {
        None => "start".to_owned(),
        Some(r) => r["kind"].as_str().unwrap_or("unknown").to_owned(),
    }
}

impl Animated for ReductionAnimation {
    fn which(&self) -> &'static str {
        "reduction"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> LiveState {
        // **Delegated, not hardcoded `Idle`.** `app.rs` gates this view with
        // `PendingLiveDebug::Reduction`, and
        // `playback::tests_animated_contract::only_the_views_with_a_live_path_report_a_live_session`
        // requires the gated set and the delegating set to be equal — a hardcoded
        // `Idle` would report "no session" during a real one, which is the defect
        // `connection_anim`'s own comment records. For a recorded cursor the playback
        // answers `Idle` anyway, so this is correct today and stays correct when
        // `start_live` becomes real.
        self.playback.live_state(arming)
    }

    /// What the frame under the cursor shows.
    ///
    /// Always `Some` for a non-empty log: the opening frame is a real state, not an
    /// absence, and it describes itself through the second branch below.
    fn current_frame_context(&self) -> Option<Value> {
        let record = self.current_record();
        let mut ctx = serde_json::json!({
            "step": step_name(record),
            "rounds_so_far": self.rounds_so_far(),
        });
        if let Some(residue) = self.residue_now() {
            ctx["residue"] = serde_json::json!(residue);
        }
        // **The record verbatim**, because Claude is the consumer of this capture and a
        // summary would be HRW's reading of it rather than Rumoca's statement.
        if let Some(record) = record {
            ctx["record"] = record.clone();
        } else if let Some((states, algebraics, equations)) = self.before {
            ctx["before"] = serde_json::json!({
                "n_states": states,
                "n_algebraics": algebraics,
                "n_equations": equations,
            });
        }
        Some(ctx)
    }

    /// Jump to frame `n`, for `hrw://…/frame/<n>`.
    ///
    /// `false` when the frame does not exist, so a link reports that instead of landing
    /// on a plausible neighbour. Frame 0 is the opening state, so the log's first
    /// record is frame 1 — which is also how the on-screen counter reads.
    fn seek(&mut self, n: usize) -> bool {
        self.playback.seek(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn report(search: Value) -> Value {
        json!({
            "before": { "n_states": 9, "n_algebraics": 88, "n_equations": 97 },
            "search": search,
        })
    }

    fn log() -> Value {
        json!([
            { "kind": "round", "lane": "direct", "round": 1, "residue": 8,
              "unmatched_equations": ["f_x[90] (SI.Angle phi)"],
              "unmatched_unknowns": [{ "name": "emf.w", "kind": "derivative" }] },
            { "kind": "candidates", "lane": "direct", "group": "direct_admissible",
              "discovered": 12 },
            { "kind": "attempt", "lane": "direct",
              "identity": { "form": "direct", "declared_at": { "file": "a.mo", "line": 15 } },
              "outcome": { "kind": "reduced", "residue": 6 } },
            { "kind": "selected", "lane": "direct",
              "identity": { "form": "direct", "declared_at": null },
              "residue_before": 8, "residue_after": 6 },
            { "kind": "stopped", "outcome": { "kind": "sorted" } },
        ])
    }

    #[test]
    fn a_payload_with_no_search_is_not_an_animation() {
        assert!(ReductionAnimation::from_report(&json!({ "before": {} })).is_none());
    }

    /// **The opening frame is the system before the search**, not the first record.
    #[test]
    fn the_run_opens_before_any_record() {
        let anim = ReductionAnimation::from_report(&report(log())).expect("an animation");
        assert_eq!(
            anim.position(),
            (0, 6),
            "five records plus the opening frame"
        );
        assert_eq!(anim.n_records(), 5);
        assert!(
            anim.current_record().is_none(),
            "frame 0 is before the search"
        );
        assert_eq!(
            anim.rounds_so_far(),
            0,
            "no round has begun at the opening frame"
        );

        let ctx = anim
            .current_frame_context()
            .expect("the opening frame describes itself");
        assert_eq!(ctx["step"], "start");
        assert_eq!(ctx["before"]["n_states"], 9);
        // The residue the search began from, so the opening frame is comparable with
        // every frame after it.
        assert_eq!(ctx["residue"], 8);
    }

    /// An empty log is **no frames**, not one.
    ///
    /// A lone `Start` would make `is_empty` false and offer a run through nothing.
    #[test]
    fn an_empty_log_has_no_frames() {
        let anim = ReductionAnimation::from_report(&report(json!([]))).expect("an animation");
        assert!(anim.is_empty());
        assert_eq!(anim.position(), (0, 0));
        assert_eq!(anim.n_records(), 0);
    }

    /// **The empty pane's words, rendered** — and they must not claim the stage was
    /// skipped, because an empty log comes from a stage that ran (a stage that did not
    /// has no `search` key, and `from_report` gives `None`). The message said *"The
    /// stage was not reached"* until 2026-11-07.
    #[test]
    fn an_empty_log_says_so_without_claiming_the_stage_was_skipped() {
        use egui_kittest::kittest::Queryable;
        let mut anim = ReductionAnimation::from_report(&report(json!([]))).expect("an animation");
        let harness = egui_kittest::Harness::new_ui(|ui| {
            let _ = anim.ui(ui, false, true);
        });
        harness.get_by_label("No reduction search in this report.");
        assert!(
            harness.query_by_label_contains("not reached").is_none(),
            "an empty log is from a stage that ran",
        );
    }

    /// **Every frame describes itself, including the opening one.**
    ///
    /// The behavioural half of
    /// `playback::tests_animated_contract::every_animation_context_names_the_step_on_screen`,
    /// which reads the source and so cannot see that the context is built in two
    /// branches.
    #[test]
    fn every_frame_describes_itself() {
        let mut anim = ReductionAnimation::from_report(&report(log())).expect("an animation");
        let expected = [
            "start",
            "round",
            "candidates",
            "attempt",
            "selected",
            "stopped",
        ];
        for (i, kind) in expected.iter().enumerate() {
            assert!(anim.seek(i), "frame {i} exists");
            let ctx = anim
                .current_frame_context()
                .unwrap_or_else(|| panic!("frame {i} must describe itself"));
            assert_eq!(ctx["step"], *kind, "frame {i}");
        }
        assert!(
            !anim.seek(6),
            "there is no frame 6, so a link must be refused"
        );
    }

    /// **The residue is read from the log, not derived from the opening number.**
    ///
    /// The frame after a `held` attempt must report the residue the *attempt* measured.
    /// Deriving it by subtracting selections would understate progress exactly where a
    /// `Held` selection moved nothing — `Drivetrain`'s round 3 — and the reader would
    /// have no way to tell.
    #[test]
    fn the_residue_comes_from_the_record_under_the_cursor() {
        let mut anim = ReductionAnimation::from_report(&report(json!([
            { "kind": "round", "lane": "direct", "round": 1, "residue": 8 },
            { "kind": "attempt", "lane": "direct", "identity": { "form": "direct" },
              "outcome": { "kind": "held", "residue": 8 } },
            { "kind": "round", "lane": "direct", "round": 2, "residue": 6 },
        ])))
        .expect("an animation");

        assert!(anim.seek(1));
        assert_eq!(anim.residue_now(), Some(8), "the first round's residue");
        assert!(anim.seek(2));
        assert_eq!(
            anim.residue_now(),
            Some(8),
            "the held attempt measured 8 again"
        );
        assert!(anim.seek(3));
        assert_eq!(anim.residue_now(), Some(6), "the second round reports 6");
        assert_eq!(anim.rounds_so_far(), 2);
    }

    /// `held` carries a gloss, because the bare word reads as failure.
    #[test]
    fn a_held_attempt_is_rendered_as_progress() {
        let record = json!({ "kind": "attempt", "lane": "direct",
            "identity": { "form": "direct" },
            "outcome": { "kind": "held", "residue": 4 } });
        let (_, _, summary) = step_style(&record);
        assert!(summary.contains("held"), "Rumoca's own word: {summary}");
        assert!(summary.contains("progress"), "and what it means: {summary}");
    }

    /// Zero candidates is rendered as *nothing to try*, never as a rejection.
    #[test]
    fn an_empty_candidate_group_says_nothing_to_try() {
        let record = json!({ "kind": "candidates", "lane": "holonomic",
            "group": "holonomic", "discovered": 0 });
        let (_, _, summary) = step_style(&record);
        assert!(summary.contains("nothing to try"), "{summary}");
        assert!(summary.contains("not a refusal"), "{summary}");
    }

    /// An unknown record kind is named on screen rather than stepped past.
    #[test]
    fn an_unknown_record_kind_is_named() {
        let (_, _, summary) = step_style(&json!({ "kind": "teleported" }));
        assert!(summary.contains("teleported"), "{summary}");
    }

    /// **A recorded run reports no live session**, which is the playback's answer and
    /// not a hardcoded one.
    ///
    /// *Named `the_view_is_never_live` until 2026-11-04, when `watch_prepare_for_solve`
    /// made a live session possible. The assertion is unchanged and the name was not: a
    /// recorded cursor is still `Idle`, but the **view** is no longer never-live, and a
    /// test whose name says otherwise is the kind of stale claim nothing else would
    /// catch.*
    ///
    /// `start_live` is deliberately not exercised here. It spawns a thread that waits
    /// at `LiveTrace::wait_for_debugger` and then runs a real reduction, which is a
    /// debugger-attached path rather than a unit-testable one — `CLAUDE.md`'s standing
    /// limit on what Claude can verify applies, and Doug pressing Debug is the
    /// verification.
    #[test]
    fn a_recorded_run_reports_no_live_session() {
        let anim = ReductionAnimation::from_report(&report(log())).expect("an animation");
        assert!(matches!(anim.live_state(false), LiveState::Idle));
    }

    /// **The stepper reads a real compile**, with every frame describing itself.
    ///
    /// Every test above builds its own log, so all of them would pass against a shape
    /// `worker::index_reduction_from_observation` no longer publishes. This is the only
    /// one that compares the reader against the writer.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn the_stepper_reads_a_real_compile() {
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
        let mut anim = ReductionAnimation::from_report(report).expect("an animation");

        let (_, total) = anim.position();
        assert_eq!(
            total,
            report["n_records"].as_u64().expect("a record count") as usize + 1,
            "one frame per record, plus the opening frame",
        );
        assert!(
            total > 10,
            "Drivetrain's search is 29 records; got {total} frames"
        );

        // Every frame must describe itself, and none may fall through to the unknown
        // arm — which is what would happen if upstream added a record variant.
        let mut kinds: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        for i in 0..total {
            assert!(anim.seek(i), "frame {i} exists");
            let ctx = anim
                .current_frame_context()
                .unwrap_or_else(|| panic!("frame {i} describes itself"));
            let step = ctx["step"].as_str().unwrap_or("").to_owned();
            assert_ne!(step, "unknown", "frame {i} has a record with no kind");
            if let Some(record) = anim.current_record() {
                let (_, _, summary) = step_style(record);
                assert!(
                    !summary.contains("does not know how to show"),
                    "frame {i} is a record kind this view cannot render: {summary}",
                );
            }
            kinds.insert(step);
        }
        for kind in [
            "start",
            "round",
            "candidates",
            "attempt",
            "selected",
            "stopped",
        ] {
            assert!(
                kinds.contains(kind),
                "no frame was a {kind:?}, so that arm is untested: {kinds:?}",
            );
        }
    }
}
