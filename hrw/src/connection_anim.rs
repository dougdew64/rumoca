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

/// One connection set as the lane view shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneSet {
    /// The set's members, from `SetFormed`.
    pub variables: Vec<String>,
    /// The equations it produced — empty until its `EquationsGenerated` frame.
    pub equations: Vec<String>,
}

/// Every set of one kind — `"potential"`, `"flow"` or `"stream"` — built so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lane {
    pub kind: String,
    pub sets: Vec<LaneSet>,
}

/// **What the replay has built by a given frame, grouped by kind.**
///
/// # Why this exists
///
/// Doug, 2026-08-15, after stepping the replay: *"it is underwhelming… mostly a
/// text-based log of results which I can step through"*, and the specific thing
/// missing was that *"the connector variables are divided into sets: potential, flow
/// and sometimes also stream"* was nowhere on screen. One frame at a time shows one
/// set; nothing showed the **division**.
///
/// # This groups, it does not compute
///
/// Every value here is read out of frames Rumoca emitted — memberships from
/// `SetFormed`, equations from `EquationsGenerated`. **No set is inferred and no
/// count is derived by re-running anything.** That is the line agreed with Doug the
/// same day: *compute freely for presentation — layout, grouping for display — and
/// compute nothing that Rumoca also computes.*
///
/// Grouping frames by their own `kind` field is presentation. Deciding which
/// variables belong together would not be, and is exactly what the frames already
/// answer.
///
/// # A pure function, deliberately
///
/// `Lanes::upto` takes frames and a cursor and returns data. Nothing here touches
/// `egui`, so the interesting part is testable without a harness — the response to
/// custom-painted views being the least reachable surface in the project
/// (`CLAUDE.md`: *move a computation out before adding one in*).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lanes {
    pub lanes: Vec<Lane>,
}

impl Lanes {
    /// The sets built by the time the reader reaches `cursor`, inclusive.
    ///
    /// Sets appear in the lane for their kind in the order the pass formed them, and
    /// gain their equations when the following `EquationsGenerated` frame is reached
    /// — so stepping shows a set arrive, then pay out. Lanes appear in first-seen
    /// order rather than a fixed one, because a model with no flow variables should
    /// not be shown an empty flow lane implying it lost something.
    #[must_use]
    pub fn upto(frames: &[ConnectionFrame], cursor: usize) -> Self {
        let mut lanes: Vec<Lane> = Vec::new();
        for frame in frames.iter().take(cursor + 1) {
            match &frame.step {
                ConnectionStep::SetFormed {
                    kind, variables, ..
                } => {
                    let lane = match lanes.iter_mut().find(|l| l.kind == *kind) {
                        Some(l) => l,
                        None => {
                            lanes.push(Lane {
                                kind: (*kind).to_owned(),
                                sets: Vec::new(),
                            });
                            lanes.last_mut().expect("just pushed")
                        }
                    };
                    lane.sets.push(LaneSet {
                        variables: variables.clone(),
                        equations: Vec::new(),
                    });
                }
                ConnectionStep::EquationsGenerated {
                    kind, equations, ..
                } => {
                    // The most recent set of that kind is the one that just paid out:
                    // the pass emits `SetFormed` then `EquationsGenerated` per set, so
                    // "last of this kind" is the pairing, not a guess about identity.
                    if let Some(set) = lanes
                        .iter_mut()
                        .find(|l| l.kind == *kind)
                        .and_then(|l| l.sets.last_mut())
                    {
                        set.equations = equations.clone();
                    }
                }
                _ => {}
            }
        }
        Self { lanes }
    }

    /// Total sets across all lanes — the number the replay's own `Complete` frame
    /// reports, recomputed here only to be *compared* against it.
    #[must_use]
    pub fn set_count(&self) -> usize {
        self.lanes.iter().map(|l| l.sets.len()).sum()
    }
}

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

    /// Attach to a live session the **worker** is running.
    ///
    /// # Why this takes a receiver instead of spawning
    ///
    /// Every other animation's `start_live` spawns a thread here and re-runs its
    /// algorithm on copied data. This one cannot — connection expansion happens
    /// inside `compile_model_strict_reachable_*`, which needs the session and the
    /// resolved `ClassTree`, and both live on the worker.
    ///
    /// So the direction is reversed: the UI makes the channel, hands the producer to
    /// the worker in [`crate::worker::ToWorker::LiveDebugConnections`], and keeps the
    /// consumer. The animation's job is only to drain it.
    ///
    /// **`done` cannot be inferred from an empty channel** — a live pass between two
    /// breakpoint stops is silent for as long as the reader stands there — so the
    /// worker sets it, and the animation reads it to know the session ended rather
    /// than stalled.
    #[must_use]
    pub fn start_live(
        rx: std::sync::mpsc::Receiver<ConnectionFrame>,
        done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) -> Self {
        Self {
            playback: Playback::live(rx, done, FRAME_INTERVAL),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.playback.is_empty()
    }

    /// The replay as the bridge publishes it — **the frames, as recorded.**
    ///
    /// # Why this pane, and why it was the one left
    ///
    /// It is the **only view that shows connection sets**, which makes it the only
    /// evidence for `connect-expansion.md` Act 1 — *three nodes, of sizes 2, 2 and 3*.
    /// Every other claim in that tour became checkable when the equation sheet started
    /// publishing; this one stayed on Claude's word alone.
    ///
    /// # No summary is synthesized, and that is the whole design
    ///
    /// The obvious convenience would be a `sets: [{kind, size}]` array collected from the
    /// frames. It is deliberately absent. Projecting a summary means *deciding* what the
    /// replay proved, and a reader could not tell that decision from something Rumoca
    /// reported — the fiction this repository spent 2026-08-04 removing.
    ///
    /// The summary already exists **in the data, from the compiler**:
    /// `ConnectionStep::Complete` carries `sets` and `equations_added`, and
    /// `EquationsGenerated` carries `set_size` and `equations_added` per set. Those are
    /// Rumoca's counts. Anything that wants totals reads them; nothing needs Claude to
    /// add up frames and be believed.
    ///
    /// # Frame numbering
    ///
    /// `frame` is **1-based**, matching the on-screen counter *and*
    /// `hrw://stage/Flatten/Connections/frame/<n>`, so a number read here can be pasted
    /// into a tour link. `index` is the raw cursor. Both are published because getting
    /// this wrong lands a reader one frame early, silently.
    #[must_use]
    pub fn to_bridge_json(&self) -> serde_json::Value {
        let frames: Vec<serde_json::Value> = self
            .playback
            .frames()
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let mut row = serde_json::json!({
                    "id": format!("frame[{}]", i + 1),
                    "frame": i + 1,
                    "index": i,
                    "sets_so_far": f.sets_so_far,
                    "equations_so_far": f.equations_so_far,
                });
                let step = match &f.step {
                    ConnectionStep::Start { connect_statements } => serde_json::json!({
                        "step": "Start",
                        "connect_statements": connect_statements,
                    }),
                    ConnectionStep::SetFormed {
                        kind,
                        scope,
                        variables,
                    } => serde_json::json!({
                        "step": "SetFormed",
                        "kind": kind,
                        "scope": scope,
                        "size": variables.len(),
                        "variables": variables,
                    }),
                    ConnectionStep::EquationsGenerated {
                        kind,
                        set_size,
                        equations_added,
                        equations,
                    } => serde_json::json!({
                        "step": "EquationsGenerated",
                        "kind": kind,
                        "set_size": set_size,
                        "equations_added": equations_added,
                        // Named, not just counted: the equation IS the rule this
                        // replay exists to show.
                        "equations": equations,
                    }),
                    ConnectionStep::UnconnectedFlow { equations_added } => serde_json::json!({
                        "step": "UnconnectedFlow",
                        "equations_added": equations_added,
                    }),
                    ConnectionStep::Complete {
                        sets,
                        equations_added,
                    } => serde_json::json!({
                        "step": "Complete",
                        "sets": sets,
                        "equations_added": equations_added,
                    }),
                };
                if let (Some(row), Some(step)) = (row.as_object_mut(), step.as_object()) {
                    for (k, v) in step {
                        row.insert(k.clone(), v.clone());
                    }
                }
                row
            })
            .collect();

        serde_json::json!({
            // What the counter reads right now. A replay's "content" is the sequence
            // *and* where the reader is in it, so both are published.
            "cursor_frame": self.playback.cursor() + 1,
            "n_frames": frames.len(),
            "frames": frames,
        })
    }

    /// Render the controls, the step line, and the running state.
    ///
    /// Returns `true` on the frame the Debug button is clicked — the same contract as
    /// every other animated view, so `app.rs` owns the arming and this owns the row.
    #[must_use]
    pub fn ui(&mut self, ui: &mut egui::Ui, arming: bool, debug_enabled: bool) -> bool {
        // **First, always.** Frames arrive on the worker thread; nothing else moves
        // them into the playback, and `tick` only advances a *recorded* cursor. Omit
        // this and a live session behaves exactly as Doug saw it: the breakpoint
        // fires, Continue re-fires it, and the animation never moves — because the
        // channel fills while the view redraws the same frame it started on.
        //
        // Before the empty check too, since a live session's first frames arrive
        // before anything else has put a frame in `frames`.
        self.playback.sync_live();

        // **Empty is not the same as not-yet-live.** A live session begins with no
        // frames at all — the worker is waiting at the startup gate for the reader to
        // arrive — so bailing out on `is_empty` alone would hide the controls exactly
        // when the reader needs to see the session is armed.
        if self.playback.is_empty() && !self.playback.is_live() && !arming {
            ui.label("No connections in this model.");
            ui.weak(
                "Nothing to expand \u{2014} every equation in the flat model was written by hand, \
                 not generated from a connect().",
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

        // **The equations themselves, not just how many.** Doug, 2026-08-15, after
        // stepping this replay: *"it is underwhelming… mostly a text-based log of
        // results"* — and the sharpest instance was here, where the rule the whole
        // view exists to teach was rendered as an integer. A flow set of three
        // producing "1 equation" says nothing; producing
        // `flow sum equation: C.n.i + src.n.i + gnd.p.i = 0` **is** Kirchhoff's law.
        //
        // These are Rumoca's rendered origins, read back from the model after the
        // generating call — not text this view composed.
        if let ConnectionStep::EquationsGenerated { equations, .. } = &frame.step
            && !equations.is_empty()
        {
            ui.add_space(4.0);
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .max_height(200.0)
                .show(ui, |ui| {
                    for e in equations {
                        ui.label(
                            egui::RichText::new(e)
                                .monospace()
                                .color(crate::colors::MATCHED_MARKER),
                        );
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

        ui.add_space(8.0);
        self.render_lanes(ui);
    }

    /// The sets built so far, **one column per kind**.
    ///
    /// This is the answer to "the pane never shows that the variables are divided
    /// into potential, flow and stream sets": stepping now fills two columns side by
    /// side, and a set's equations appear beneath it as it pays out. A thin renderer
    /// over [`Lanes`], which is where the only logic lives.
    fn render_lanes(&self, ui: &mut egui::Ui) {
        let lanes = Lanes::upto(self.playback.frames(), self.playback.cursor());
        if lanes.lanes.is_empty() {
            return;
        }
        ui.separator();
        ui.label(
            egui::RichText::new(
                "Connection sets so far \u{2014} one column per kind. A set of n \
                 potentials pays out n-1 equations; a set of n flows pays out exactly one.",
            )
            .italics()
            .color(crate::colors::ANIM_EXPLORE),
        );
        ui.add_space(4.0);

        // `columns` rather than a manual split: egui divides the width evenly and
        // the lane count is small and data-driven.
        ui.columns(lanes.lanes.len(), |cols| {
            for (col, lane) in cols.iter_mut().zip(&lanes.lanes) {
                col.label(
                    egui::RichText::new(format!(
                        "{} \u{00b7} {} set(s)",
                        lane.kind,
                        lane.sets.len()
                    ))
                    .strong()
                    .color(kind_color(&lane.kind)),
                );
                egui::ScrollArea::vertical()
                    .id_salt(format!("lane-{}", lane.kind))
                    .auto_shrink([false, true])
                    .max_height(240.0)
                    .show(col, |col| {
                        for (i, set) in lane.sets.iter().enumerate() {
                            col.add_space(4.0);
                            col.label(
                                egui::RichText::new(format!(
                                    "set {} \u{2014} {} variable(s)",
                                    i + 1,
                                    set.variables.len()
                                ))
                                .strong(),
                            );
                            for v in &set.variables {
                                col.label(egui::RichText::new(format!("  {v}")).monospace());
                            }
                            // Absent until this set's equations frame is reached, and
                            // said rather than left blank: an empty gap reads as "this
                            // set produced nothing", which is a different claim.
                            if set.equations.is_empty() {
                                col.label(
                                    egui::RichText::new("  (equations not generated yet)")
                                        .weak()
                                        .italics(),
                                );
                            } else {
                                for e in &set.equations {
                                    col.label(
                                        egui::RichText::new(format!("  \u{2192} {e}"))
                                            .monospace()
                                            .color(crate::colors::MATCHED_MARKER),
                                    );
                                }
                            }
                        }
                    });
            }
        });
    }
}

/// Colour per connection kind, so the two columns are distinguishable at a glance.
fn kind_color(kind: &str) -> egui::Color32 {
    match kind {
        "flow" => crate::colors::EQ_CAT_FLOW_SUM,
        "potential" => crate::colors::EQ_CAT_CONNECTION,
        _ => crate::colors::ANIM_EXPLORE,
    }
}

impl Animated for ConnectionAnimation {
    fn which(&self) -> &'static str {
        "connection_expansion"
    }

    fn position(&self) -> (usize, usize) {
        self.playback.position()
    }

    fn live_state(&self, arming: bool) -> crate::LiveState {
        // **Was hardcoded to `Idle`** while this view was recorded-only, and that
        // stub outlived the reason for it: with a live path in place it reported
        // "no session" during a real one, so the playback controls stayed enabled
        // while the debugger owned the cursor, and `Finished` never arrived to
        // re-enable them afterwards.
        self.playback.live_state(arming)
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
            // Rendered below the summary line by `render_current`, not squeezed into
            // it: the equations are the content, and a one-line summary that swallows
            // them is the "text log of results" this replay was criticised for being.
            equations: _,
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
                equations: vec![
                    "connection equation: a = b".into(),
                    "connection equation: b = c".into(),
                ],
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
                equations: vec!["flow sum equation: a.i + b.i + c.i = 0".into()],
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
                equations: Vec::new(),
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
        assert_eq!(
            anim.live_state(false),
            crate::LiveState::Idle,
            "no frames and no session is Idle",
        );
        // **`Arming`, not `Idle`** — this used to assert `Idle` because
        // `live_state` was stubbed to it while the view was recorded-only. The flag
        // exists precisely so the controls disable during the breakpoint handshake,
        // when the view is still showing the recorded animation and cannot tell from
        // its own state that a session is starting.
        assert_eq!(anim.live_state(true), crate::LiveState::Arming);
    }
}

#[cfg(test)]
mod lane_tests {
    use super::*;

    fn set_formed(kind: &'static str, vars: &[&str]) -> ConnectionFrame {
        ConnectionFrame {
            step: ConnectionStep::SetFormed {
                kind,
                scope: String::new(),
                variables: vars.iter().map(|v| (*v).to_string()).collect(),
            },
            sets_so_far: 0,
            equations_so_far: 0,
        }
    }

    fn generated(kind: &'static str, eqs: &[&str]) -> ConnectionFrame {
        ConnectionFrame {
            step: ConnectionStep::EquationsGenerated {
                kind,
                set_size: 0,
                equations_added: eqs.len(),
                equations: eqs.iter().map(|e| (*e).to_string()).collect(),
            },
            sets_so_far: 0,
            equations_so_far: 0,
        }
    }

    /// The realistic shape: a flow set and its equation, then a potential set and
    /// its two — the order the pass actually emits for `RcCircuit`.
    fn chain() -> Vec<ConnectionFrame> {
        vec![
            set_formed("flow", &["a.i", "b.i", "c.i"]),
            generated("flow", &["flow sum equation: a.i + b.i + c.i = 0"]),
            set_formed("potential", &["a.v", "b.v", "c.v"]),
            generated(
                "potential",
                &[
                    "connection equation: a.v = b.v",
                    "connection equation: b.v = c.v",
                ],
            ),
        ]
    }

    /// **Sets land in the lane for their own kind, and never mix.**
    ///
    /// The division Doug reported as invisible: potential and flow are separate
    /// graphs over disjoint variables, and a view that merged them would be showing
    /// something the compiler never built.
    #[test]
    fn each_kind_gets_its_own_lane() {
        let lanes = Lanes::upto(&chain(), 3);
        assert_eq!(lanes.lanes.len(), 2, "one lane per kind: {lanes:?}");
        assert_eq!(lanes.set_count(), 2);

        let flow = lanes.lanes.iter().find(|l| l.kind == "flow").expect("flow");
        let potential = lanes
            .lanes
            .iter()
            .find(|l| l.kind == "potential")
            .expect("potential");
        assert!(
            flow.sets[0].variables.iter().all(|v| v.ends_with(".i")),
            "a flow lane holds only flow variables: {:?}",
            flow.sets[0].variables,
        );
        assert!(
            potential.sets[0]
                .variables
                .iter()
                .all(|v| v.ends_with(".v")),
            "a potential lane holds only potential variables: {:?}",
            potential.sets[0].variables,
        );
    }

    /// **A set shows its members before its equations exist, and gains them at the
    /// frame that generates them** — which is what makes stepping show the payout
    /// rather than a finished table.
    #[test]
    fn a_set_gains_its_equations_only_when_that_frame_is_reached() {
        let frames = chain();

        let at_set = Lanes::upto(&frames, 0);
        let flow = &at_set.lanes[0].sets[0];
        assert_eq!(flow.variables.len(), 3);
        assert!(
            flow.equations.is_empty(),
            "before its equations frame, a set has produced nothing yet",
        );

        let at_equations = Lanes::upto(&frames, 1);
        assert_eq!(
            at_equations.lanes[0].sets[0].equations.len(),
            1,
            "a flow set of three pays out exactly one equation",
        );
        assert_eq!(
            Lanes::upto(&frames, 3).lanes[1].sets[0].equations.len(),
            2,
            "a potential set of three pays out n-1 = 2",
        );
    }

    /// The cursor bounds what is shown: a reader at frame 1 has not seen the
    /// potential set, and a view that showed it would be reporting the future.
    #[test]
    fn lanes_never_show_past_the_cursor() {
        let frames = chain();
        assert_eq!(Lanes::upto(&frames, 1).lanes.len(), 1, "flow only so far");
        assert_eq!(Lanes::upto(&frames, 2).lanes.len(), 2, "potential appears");
    }

    /// A model with no connections produces no lanes — stated by absence rather
    /// than by an empty flow column implying something was lost.
    #[test]
    fn no_frames_means_no_lanes() {
        assert!(Lanes::upto(&[], 0).lanes.is_empty());
        assert_eq!(Lanes::upto(&[], 0).set_count(), 0);
    }
}

#[cfg(test)]
mod live_session_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn frame(n: usize) -> ConnectionFrame {
        ConnectionFrame {
            step: ConnectionStep::Start {
                connect_statements: n,
            },
            sets_so_far: 0,
            equations_so_far: 0,
        }
    }

    /// **A live session's frames reach the animation, and the cursor follows them.**
    ///
    /// The defect this exists for, reported by Doug on 2026-08-15 the first time the
    /// Connections Debug button was used: *"a breakpoint is set on live_trace.rs:173.
    /// That breakpoint is hit. When I click continue, I hit the breakpoint again. But
    /// the connections animation does not advance."*
    ///
    /// The worker half was correct — frames were being pushed and the anchor was
    /// firing. `ConnectionAnimation::ui` simply never called `Playback::sync_live`,
    /// which is the only thing that moves frames out of the channel; `tick` advances
    /// a *recorded* cursor and nothing else. Every other animated view called it as
    /// the first statement of `ui`, and this one was written without it.
    ///
    /// **The failure is silent and looks like a debugger problem**, which is what
    /// makes it worth a test: the breakpoint behaves perfectly, so the natural
    /// suspicion falls on the anchor, the adapter or the frame delay — none of which
    /// are at fault.
    #[test]
    fn live_frames_reach_the_animation_and_move_the_cursor() {
        let (tx, rx) = std::sync::mpsc::channel();
        let done = Arc::new(AtomicBool::new(false));
        let mut anim = ConnectionAnimation::start_live(rx, Arc::clone(&done));

        tx.send(frame(1)).expect("send");
        tx.send(frame(2)).expect("send");
        anim.playback.sync_live();

        assert!(!anim.is_empty(), "frames pushed by the worker must arrive");
        let published = anim.to_bridge_json();
        assert_eq!(
            published["n_frames"].as_u64(),
            Some(2),
            "both frames must be drained, not just the first",
        );
        assert_eq!(
            published["cursor_frame"].as_u64(),
            Some(2),
            "the cursor jumps to the newest arrival \u{2014} that is what makes a \
             debugger step visibly advance the view",
        );

        // A second stop delivers more, and the cursor keeps up.
        tx.send(frame(3)).expect("send");
        anim.playback.sync_live();
        assert_eq!(anim.to_bridge_json()["cursor_frame"].as_u64(), Some(3));
    }

    /// The session ending is reported, so the controls come back.
    ///
    /// Without it a finished live session leaves playback disabled for good —
    /// `docs/ideas.md` #74's defect, which cost a session to diagnose once already.
    #[test]
    fn a_finished_live_session_reports_itself() {
        let (tx, rx) = std::sync::mpsc::channel();
        let done = Arc::new(AtomicBool::new(false));
        let mut anim = ConnectionAnimation::start_live(rx, Arc::clone(&done));
        tx.send(frame(1)).expect("send");
        anim.playback.sync_live();

        assert_eq!(anim.live_state(false), crate::LiveState::Running);
        done.store(true, Ordering::Release);
        assert_eq!(
            anim.live_state(false),
            crate::LiveState::Finished,
            "the worker signals completion; the view must notice",
        );
    }
}
