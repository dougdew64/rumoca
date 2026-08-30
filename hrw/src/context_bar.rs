//! **The Context Bar** — what is pointed at, what is being followed, and what is
//! always context: the state it owns, and the pane that draws it.
//!
//! Lifted out of `App::context_bar_ui` on 2026-08-19, in two steps: the rendering
//! first, then [`ContextBarState`] and the capture types behind it. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why the state followed the pane rather than leading it
//!
//! The rendering moved while [`ContextBarState`], [`PointedAt`] and [`PointKind`]
//! stayed in `app.rs`, which left this module importing its own state back from the
//! module it had just left — `app` → `context_bar` for the pane, `context_bar` →
//! `app` for the types it draws. Bringing the three types here makes the dependency
//! **one-directional**, which is the whole of what this step buys; the same
//! `Viewport` → `stage_view.rs` and `SourceViewState` → `specimen_source.rs`
//! precedent, arriving one iteration late because the pane was the expensive half.
//!
//! **It bought no new test, and that is recorded rather than dressed up.** Both
//! properties worth holding here were already asserted through `App::test_default()`
//! — the shared counter's recency ordering and the jump cursor's wrap-around — and
//! neither needed a worker or a compile to begin with. The justification is
//! `app-split-plan.md`'s *other* admissible one: what a session no longer has to
//! hold.
//!
//! # Why the middle of the function, again
//!
//! `context_bar_ui` was 255 lines calling **seven** `App` methods — the most of
//! anything left in `app.rs`, and the reason the coupling table put it last. Six of
//! the seven turn out to be free, and the census that shows it is about *position*
//! rather than count (`app-split-plan.md`):
//!
//! - `refresh_jump_matches` is the one genuine **barrier**: it rebuilds the match
//!   list that the Following row reports two lines later. It stays in `App`, called
//!   before this function.
//! - `jump_to_next_match`, `next_seq`, `emit_context` and `navigate_to` all sit in a
//!   trailing block *after* `ui.separator()`, where nothing downstream draws. Moving
//!   them to the caller costs **no frame at all** — the same statements run in the
//!   same order, one function boundary later. (`next_seq` has since come here as a
//!   method on [`ContextBarState`]; `App` still calls it from that same block.)
//! - `empty_context_hint` stays behind, and that decision is the parameter list:
//!   it reads `ui_mode`, `specimen_detail` and `viewing_log`, three pieces of state
//!   this pane otherwise never touches, purely to phrase one sentence. Moving it
//!   would have added three arguments that teach a reader nothing about what the
//!   bar *reports*.
//! - [`background_ui`] moves, because it is called from inside this body and draws
//!   rather than decides.
//!
//! So the empty-state branch stays in `App` — it is four lines of rendering around
//! that hint — and the ~200 lines that draw an *assembled* context left.
//!
//! # One press per frame
//!
//! The function used to accumulate five independent locals (`clear_point`,
//! `clear_thread`, `jump_forward`, `jump_back`, `go_to_class`) and act on all of
//! them below the rows. [`ContextBarPress`] collapses that to one report, which is
//! sound because every one of them is set by a distinct `small_button` or `link`:
//! egui delivers a pointer press to a **single** widget, so two could never be true
//! in the same frame. The old shape could express it; nothing could produce it.
//!
//! # Why the background is drawn by both branches
//!
//! [`background_ui`] is `pub(crate)` rather than private because `App`'s empty-state
//! branch calls it too. That sharing is deliberate and predates the split: the two
//! branches drifted apart the moment there were two of them, and the empty one
//! returned before ever reaching the background — so the bar showed *no* context in
//! the state a reader is in most of the time. Found 2026-08-01 by Doug, who counted
//! three kinds of context and saw two.

use std::collections::{BTreeMap, HashMap};

use eframe::egui;

use crate::bridge::{self, Seg};
use crate::identifier_index::IdentifierIndex;
use crate::worker::{DefInfo, StageBundle, StageKind};

/// Everything the **Context Bar** owns: what has been captured, and how the
/// reader moves through it.
///
/// Two cohorts, kept in one struct because the second exists only to serve the
/// first — you jump between *mentions of the identifier being followed*, so the
/// jump fields are meaningless without the capture.
///
/// # What stays on `App`
///
/// `tracked_identifier` does. It is one of the four fields the census found
/// genuinely shared: the source view underlines it, the tree highlights it, the
/// equation sheet marks its rows. The Context Bar *displays* the follow; it does
/// not own it.
///
/// `refresh_jump_matches` and `jump_to_next_match` stay too, and that is the
/// same rule pointed at behaviour rather than state: both read three pieces of
/// `App` (`tracked_identifier`, `stage`, and the current stage's IR; the second
/// also clears `viewing_log`), so moving them here would widen the signature to
/// carry state this module otherwise never touches — and they are already
/// covered by an `App`-level test that needs no worker and no compile.
#[derive(Default)]
pub(crate) struct ContextBarState {
    // ---- The capture ----
    /// What is pointed at, if anything.
    pub(crate) pointed_at: Option<PointedAt>,
    /// Why the last capture could not be written, if it could not. **Reported,
    /// never swallowed** — a capture that silently failed would have Claude
    /// answer about a screen nobody is looking at.
    pub(crate) point_error: Option<String>,
    /// A one-line summary of the followed identifier: how many mentions, across
    /// how many stages.
    pub(crate) tracking_summary: Option<(usize, usize)>,
    /// Bumped when the follow changes, so the capture can be re-emitted.
    pub(crate) track_seq: u64,
    /// Bumped on every capture, so `focus.json` carries a monotonic sequence and
    /// Claude can tell a stale read from a fresh one.
    pub(crate) context_seq: u64,

    // ---- Moving through the capture ----
    /// Where the followed identifier is mentioned, in render order.
    pub(crate) jump_matches: Vec<Vec<Seg>>,
    /// What [`Self::jump_matches`] was computed for, so it is rebuilt only when
    /// the question changes rather than every frame.
    pub(crate) jump_key: Option<(StageKind, String)>,
    /// Which mention the reader is on.
    pub(crate) jump_index: usize,
    /// A mention to scroll to next frame. **Lasts exactly one frame**: holding it
    /// longer would re-scroll every frame and pin the view.
    pub(crate) jump_target: Option<Vec<Seg>>,
    /// A row to flash so the reader can see which one the jump meant. Cleared as
    /// soon as they point at something themselves — they have just answered a
    /// different question.
    pub(crate) jump_highlight: Option<Vec<Seg>>,
}

impl ContextBarState {
    /// The next stamp from the **shared** context counter.
    ///
    /// One counter for both halves, so `seq` and `tracking.seq` are directly
    /// comparable and "which did the user touch last?" has an answer. Two
    /// independent counters looked comparable and were not: after twelve
    /// captures and one follow they read 12 and 1, which says nothing about
    /// recency — and a reader trusting the instructions would conclude the
    /// wrong thing. Found on the first real `explain`.
    ///
    /// It came here with the struct because it is the *only* one of the four
    /// `App` methods over this state that mentions nothing else: two lines, one
    /// field, and the invariant it protects is a property of the counter rather
    /// than of the application.
    pub(crate) fn next_seq(&mut self) -> u64 {
        self.context_seq += 1;
        self.context_seq
    }
}

/// The last deliberate capture, retained so the Context Bar can state it and so
/// re-emission preserves it.
///
/// `stage` is the stage the capture was **made** in, not the one currently on
/// screen. They diverge as soon as the user switches tabs, and the bar must
/// report the former — anything else describes context Claude does not have.
#[derive(Clone)]
pub(crate) struct PointedAt {
    /// Stamp from the shared context counter — comparable against
    /// `track_seq`, which is stamped from the same source.
    pub(crate) seq: u64,
    /// Human-readable description, exactly as emitted.
    pub(crate) target: String,
    /// Which of the three capture shapes this was.
    ///
    /// **All three must be recorded, not just `Node`.** Only node captures were
    /// retained at first, so clicking a stage tab — which emits a *stage*
    /// capture — rewrote `focus.json` while the bar went on displaying the
    /// previous node. The bar and the file disagreed, which is precisely the
    /// drift its governing rule forbids.
    pub(crate) kind: PointKind,
    pub(crate) stage: StageKind,
    pub(crate) request: bridge::AskRequest,
}

/// What a capture pointed at, kept so the focus can be rebuilt when the
/// followed identifier changes.
#[derive(Clone)]
pub(crate) enum PointKind {
    /// A specific IR node, addressed from the stage root.
    Node(Vec<Seg>),
    /// A whole stage's IR.
    Stage,
    /// The specimen as a whole.
    Specimen,
}

/// What the Context Bar was asked to do, for `App` to perform.
///
/// The **fourth instance** of the render-and-report pattern, after
/// [`crate::specimen_source`]'s `Option<String>`, [`crate::tour_panel`]'s
/// `TransportRequest` and [`crate::stage_tabs`]'s `TabClick` — and the first where
/// deferring costs provably nothing, because every one of these was already
/// performed below the last `ui` call in the function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContextBarPress {
    /// An arrow was clicked: move to the next mention of the followed identifier in
    /// this stage, forward or back.
    Jump { forward: bool },
    /// The × on the *Pointing at* row: drop the point, keeping the follow.
    ClearPoint,
    /// The × on the *Following* row: drop the follow, keeping the point.
    ClearThread,
    /// The declaring class was clicked: open it.
    GoToClass(String),
}

/// The bar, once something is assembled: what Claude can see right now.
///
/// The caller guarantees that `context.pointed_at` or `tracked_identifier` is
/// `Some` — the empty state is `App`'s branch, not this one — and that
/// `refresh_jump_matches` has already run for the current stage and follow.
///
/// ## The rule this obeys
///
/// **It renders what will be emitted — nothing more, nothing less.** If it
/// showed context Claude does not receive, or omitted context Claude does,
/// questions would be calibrated against a fiction. Built as a view of the
/// payload, it cannot drift, because there is nothing to drift from.
///
/// Hence three rows and no fourth: *pointing at* and *following* are the two
/// shapes of assembled context, and *always* is the standing context —
/// stage IRs, the DefId table, the libraries — that the old UI never
/// mentioned at all, leaving the user to underestimate what a question had
/// behind it.
///
/// Controls here are only those that **change** what is emitted. Navigation
/// is not context, so the declaring class is a link rather than a button.
/// See `docs/context-assembly.md`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn context_bar_ui(
    ui: &mut egui::Ui,
    context: &ContextBarState,
    tracked_identifier: &Option<String>,
    stage: StageKind,
    stages: &StageBundle,
    identifier_index: &Option<IdentifierIndex>,
    declaring_classes: &HashMap<String, String>,
    def_index: &BTreeMap<u64, DefInfo>,
    model: Option<&str>,
    tour: Option<&str>,
) -> Option<ContextBarPress> {
    let mut press: Option<ContextBarPress> = None;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Context").strong());
        if let Some(point) = &context.pointed_at {
            // Worth saying only when it **differs** from the background
            // stage; otherwise it repeats the line above as if it were a
            // second, independent fact.
            if point.stage != stage {
                ui.colored_label(
                    crate::colors::CONTEXT_POINT,
                    format!("\u{00b7} pointed at in {}", point.stage.name()),
                );
            }
        }
        // An emission failure must be stated here, not swallowed. Otherwise
        // the bar claims context Claude does not have — it would still be
        // holding the *previous* focus — which is the confident lie this
        // whole design exists to prevent.
        if let Some(err) = &context.point_error {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("\u{26a0} not emitted \u{2014} {err}"),
            );
        }
    });

    // A point or a thread implies a loaded specimen, so the stage is real here.
    always_ui(
        ui,
        model,
        Some(stage),
        tour,
        stage_ir_count(stages),
        def_index.len(),
    );

    if let Some(point) = &context.pointed_at {
        let request = point.request.as_str();
        let target = point.target.clone();
        ui.horizontal(|ui| {
            // **The category label carries the category's colour, not just its value.**
            // Doug is learning to read this bar at a glance, and a coloured value under
            // a grey label makes the eye find the value before it knows what kind of
            // thing it is.
            ui.colored_label(crate::colors::CONTEXT_POINT, "   Pointing at  ");
            ui.label(
                egui::RichText::new(&target)
                    .monospace()
                    .color(crate::colors::CONTEXT_POINT),
            );
            ui.weak(format!("({request})"));
            // Symmetric with Following. Without it the point could only be
            // *replaced*, never removed — so "explain only what I am
            // following" was unaskable, and the sole escape was reloading
            // the specimen, which recompiles and discards everything.
            if ui
                .small_button("\u{00d7}")
                .on_hover_text(
                    "Stop pointing at this \u{2014} leaves only what you are \
                         following in the context Claude has",
                )
                .clicked()
            {
                press = Some(ContextBarPress::ClearPoint);
            }
        });
    }

    if let Some(name) = tracked_identifier.clone() {
        ui.horizontal(|ui| {
            ui.colored_label(crate::colors::TRACKED_GOLD, "   Following    ");
            ui.label(
                egui::RichText::new(&name)
                    .monospace()
                    .color(crate::colors::TRACKED_GOLD),
            );
            // A synthesized name is checked FIRST, because it also carries a
            // source line — inherited from the variable it shadows — and
            // reporting that as "declared at line 41" sends the reader to a
            // declaration of a *different* variable. The emitted context had
            // the same defect; the two must agree, and both must be honest.
            //
            // Recognition uses Rumoca's own inverse, never a string match:
            // `generated_names.rs` owns the convention and says consumers
            // must not spell it out themselves.
            match rumoca_core::pre_slot_base(&name) {
                Some(base) => {
                    ui.weak(format!("\u{2014} generated: pre({base})"))
                        .on_hover_text(
                            "Synthesized by DAE pre-lowering, not declared anywhere. \
                                 A `when` equation needs a value to hold when no branch \
                                 fires, and a DAE has no way to say \u{201c}unchanged\u{201d} \
                                 \u{2014} so the previous value gets a variable of its own.",
                        );
                }
                None => match identifier_index
                    .as_ref()
                    .and_then(|idx| idx.variables.get(&name))
                    .map(|v| v.source_line)
                {
                    Some(line) => {
                        ui.weak(format!("\u{2014} declared at line {line}"));
                    }
                    None => match declaring_classes.get(&name) {
                        Some(class) => {
                            ui.weak("\u{2014} in");
                            if ui
                                .link(class)
                                .on_hover_text(format!(
                                    "Open {class} \u{2014} the type of the component this \
                                     variable belongs to. Use Back to return here.",
                                ))
                                .clicked()
                            {
                                press = Some(ContextBarPress::GoToClass(class.clone()));
                            }
                        }
                        None => {
                            ui.weak("\u{2014} not declared in this specimen")
                                .on_hover_text(
                                    "Neither the specimen nor a component type declares \
                                     this name, so a compiler phase created it. Ask \
                                     Claude to trace where it came from.",
                                );
                        }
                    },
                },
            }
            // What the question will actually have behind it.
            if let Some((mentions, stages)) = context.tracking_summary {
                ui.weak(format!(
                    "\u{00b7} {mentions} mention{} across {stages} stage{}",
                    if mentions == 1 { "" } else { "s" },
                    if stages == 1 { "" } else { "s" },
                ));
            }
            // Jump to where it lives in THIS stage.
            //
            // Replaces hunting for it by eye. "Reveal identifiers" tried to
            // solve this by expanding every path that leads to *any*
            // trackable name — which surfaces N nodes to reveal one, making
            // the haystack bigger. Here the target is already known: the
            // user said which identifier they are following, so the app
            // should not also make them find it.
            //
            // **That checkbox was removed 2026-08-04**, and this is what it
            // was superseded by. The supersession had been recorded here for
            // days while the control stayed on screen — worth noting, because
            // a comment saying "X failed" is not the same as deleting X, and
            // only Doug using it closed the gap.
            let n = context.jump_matches.len();
            if n == 0 {
                // Meaningful, not a failure — the same information as
                // `mentions: 0` in the emitted context. A variable absent
                // from Parse but present in Flatten is showing you the
                // flattening boundary.
                ui.weak(format!("\u{00b7} not in {}", stage.name()));
            } else {
                ui.weak(format!(
                    "\u{00b7} {} of {n} in {}",
                    context.jump_index + 1,
                    stage.name(),
                ));
                if ui
                    .small_button("\u{2190}")
                    .on_hover_text("Previous occurrence in this stage")
                    .clicked()
                {
                    press = Some(ContextBarPress::Jump { forward: false });
                }
                if ui
                    .small_button("\u{2192}")
                    .on_hover_text(
                        "Scroll the tree to where this identifier appears in this \
                             stage, opening whatever is collapsed above it",
                    )
                    .clicked()
                {
                    press = Some(ContextBarPress::Jump { forward: true });
                }
            }
            if ui
                .small_button("\u{00d7}")
                .on_hover_text("Stop following")
                .clicked()
            {
                press = Some(ContextBarPress::ClearThread);
            }
        });
    }

    ui.add_space(BAR_MARGIN);
    ui.separator();
    press
}

/// Breathing room above and below the Context Bar (Doug, 2026-08-30: *"add some margin
/// above and below the context bar"*).
///
/// **Applied inside the bar rather than at its three call sites**, so a fourth place
/// that draws it cannot forget — the bar reached the navigation branch by being added
/// there separately, and that is exactly the kind of omission a per-call-site
/// convention produces.
///
/// The bar sits between two `separator()`s; this is the gap between its rows and those
/// rules, not a replacement for them.
pub(crate) const BAR_MARGIN: f32 = 6.0;

/// How many stages have produced an IR — the count the Always row reports.
pub(crate) fn stage_ir_count(stages: &StageBundle) -> usize {
    stages
        .as_stage_pairs()
        .iter()
        .filter(|(_, v)| v.is_some())
        .count()
}

/// The **background**: specimen and stage, always context, always shown.
///
/// `docs/context-assembly.md`: *"Specimen and stage are always context, so they are
/// always shown."* One renderer for both branches of the bar — see the module doc
/// for what happened when there were two.
///
/// **`stage` is an `Option` because the bar is now always on screen** (Doug,
/// 2026-08-30), so "no specimen is loaded" became a state this has to render rather
/// than one its caller had already excluded.
///
/// It carried a `has_specimen: bool` for exactly this until earlier the same day, when
/// it was removed as unreachable — correctly, on the evidence then: the bar was hidden
/// without a specimen, so the arm could not be reached. **Making the bar unconditional
/// is what brought the state back**, and `Option<StageKind>` says it in the type rather
/// than in a parallel flag, matching `bridge::Ask::stage`, which is already `None` for
/// a navigated definition.
///
/// **The tour is background too.** `session.json` has carried the open tour's name
/// since 2026-08-19 for deixis, so Claude already had it while the bar did not say so —
/// the under-reporting that prompted this. What is *not* here is the current stop,
/// declined 2026-08-19 with four reasons; do not add it.
/// What the **Always** row says, as a string, so the wording is testable without a
/// frame.
///
/// Split out of the painter on 2026-08-30 for the reason `working-with-doug.md` gives
/// for the animation views: **move a computation out before adding one in.** Every
/// claim this row makes — that a stage ran, that a tour is open — is now checkable
/// directly, and the painter is a label.
pub(crate) fn always_summary(
    model: Option<&str>,
    stage: Option<StageKind>,
    tour: Option<&str>,
    stage_ir_count: usize,
    def_count: usize,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    match (model, stage) {
        (Some(model), Some(stage)) => parts.push(format!("{model} \u{00b7} {}", stage.name())),
        // Mid-compile, or a compile that yielded no model name: still name the stage.
        (None, Some(stage)) => parts.push(stage.name().to_owned()),
        // No specimen. A stage name here would be a claim that a phase ran.
        (Some(model), None) => parts.push(model.to_owned()),
        (None, None) => {}
    }
    if let Some(tour) = tour {
        parts.push(format!("tour: {tour}"));
    }
    // **Always present, including as zeroes.** They are the standing context Claude
    // reads without being pointed at anything, and a row that omitted them when empty
    // would be reporting nothing rather than reporting none.
    parts.push(format!("{stage_ir_count} stage IRs"));
    parts.push(format!("{def_count} DefIds"));
    parts.join(" \u{00b7} ")
}

/// The **Always** row: everything Claude has without you clicking anything.
///
/// # It was two things in two places until 2026-08-30, and Doug found both halves
///
/// > *"the 'Always' category only seems to appear when I follow an identifier. So, the
/// > 'Always' category seems to be part of 'Follow' instead of always-available
/// > context."*
///
/// > *"the specimen, stage and tour are listed, but no category is specified for
/// > them."*
///
/// **Those are one defect seen from two sides.** The session facts sat in a row labelled
/// `Always` that only rendered in the *assembled* branch — so a row asserting "always"
/// appeared only once something was pointed at or followed. Meanwhile specimen, stage
/// and tour rendered unlabelled beside the title, in the same category and looking like
/// a different one. A label claiming more than the mechanism delivers is the lens
/// `unattended-runs.md` leads with; this is that, in the UI.
///
/// **Now one row, one label, in every state**, carrying both halves. It is drawn
/// *first*, directly under the title, because that is the only position it can hold in
/// both branches — and a category that moves is one more thing to look for.
pub(crate) fn always_ui(
    ui: &mut egui::Ui,
    model: Option<&str>,
    stage: Option<StageKind>,
    tour: Option<&str>,
    stage_ir_count: usize,
    def_count: usize,
) {
    ui.horizontal(|ui| {
        ui.colored_label(crate::colors::CONTEXT_ALWAYS, "   Always       ");
        ui.colored_label(
            crate::colors::CONTEXT_ALWAYS,
            always_summary(model, stage, tour, stage_ir_count, def_count),
        )
        .on_hover_text(
            "True for the whole session, whatever you have clicked. Every pipeline \
             stage's full IR is on disk under .hrw-bridge/stages/, and the DefId table \
             resolves numeric ids to names. Claude reads these \u{2014} and the specimen, \
             stage and open tour \u{2014} without you pointing at anything.",
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The Always row states only what is true, and states the standing facts even
    /// when they are zero.**
    ///
    /// `App::stage` always holds *some* `StageKind`, so the obvious rendering prints
    /// `Parse` on a fresh launch — naming a phase that has not run, about a specimen
    /// that does not exist. **Making a pane unconditional is exactly when it starts
    /// inventing**, because it suddenly has frames to fill that it never had before.
    ///
    /// The counts go the other way: they are reported *as zeroes* rather than omitted,
    /// because the row's claim is "this is what Claude has without you clicking", and
    /// a row that fell silent when the answer was none would be reporting nothing
    /// instead of reporting none.
    #[test]
    fn always_summary_states_only_what_is_true() {
        let nothing = always_summary(None, None, None, 0, 0);
        assert!(
            !nothing.contains("Parse"),
            "no specimen is loaded, so naming a stage claims a phase ran: {nothing:?}",
        );
        assert!(
            nothing.contains("0 stage IRs") && nothing.contains("0 DefIds"),
            "the standing counts are the row's whole point and must be stated at zero, \
             not omitted: {nothing:?}",
        );

        let loaded = always_summary(Some("RcCircuit"), Some(StageKind::Parse), None, 4, 38855);
        assert!(
            loaded.contains("RcCircuit \u{00b7} Parse"),
            "a loaded specimen names its model and stage: {loaded:?}",
        );

        // Mid-compile: a stage is selected but no model name has arrived yet.
        let compiling = always_summary(None, Some(StageKind::Flatten), None, 1, 12);
        assert!(
            compiling.contains("Flatten"),
            "the stage is real even before the model name lands: {compiling:?}",
        );

        let toured = always_summary(None, None, Some("connect-expansion"), 0, 0);
        assert!(
            toured.contains("tour: connect-expansion"),
            "an open tour is standing context and is named: {toured:?}",
        );
    }
    use crate::identifier_index::IndexedVariable;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Everything the bar reads, plus what it reported, so one harness closure can
    /// drive it and the assertions can read both sides.
    struct Bar {
        context: ContextBarState,
        tracked: Option<String>,
        stage: StageKind,
        stages: StageBundle,
        index: Option<IdentifierIndex>,
        declaring: HashMap<String, String>,
        defs: BTreeMap<u64, DefInfo>,
        /// The last non-`None` report, kept separately from the per-frame return so a
        /// click observed on one frame is not erased by the next quiet redraw.
        reported: Option<ContextBarPress>,
    }

    /// Written by hand because [`StageKind`] has no `Default` — it is a position in
    /// the pipeline and there is no neutral one.
    impl Default for Bar {
        fn default() -> Self {
            Self {
                context: ContextBarState::default(),
                tracked: None,
                stage: StageKind::Flatten,
                stages: StageBundle::default(),
                index: None,
                declaring: HashMap::new(),
                defs: BTreeMap::new(),
                reported: None,
            }
        }
    }

    /// **Width is not decoration here.** The Following row is a single
    /// `ui.horizontal`, which clips rather than wraps, and a clipped widget stays in
    /// the accessibility tree — so a query finds the × and the click silently does
    /// nothing, which reads exactly like "the bar did not report the press".
    fn harness(bar: Bar) -> Harness<'static, Bar> {
        Harness::builder()
            .with_size(egui::Vec2::new(1600.0, 400.0))
            .build_ui_state(
                |ui, b: &mut Bar| {
                    let press = context_bar_ui(
                        ui,
                        &b.context,
                        &b.tracked,
                        b.stage,
                        &b.stages,
                        &b.index,
                        &b.declaring,
                        &b.defs,
                        Some("MotorWithBrake"),
                        None,
                    );
                    if press.is_some() {
                        b.reported = press;
                    }
                },
                bar,
            )
    }

    fn a_point() -> PointedAt {
        PointedAt {
            seq: 1,
            target: "equations[0]".to_owned(),
            kind: PointKind::Stage,
            stage: StageKind::Flatten,
            request: bridge::AskRequest::Explain,
        }
    }

    fn indexed(name: &str, line: u32) -> IndexedVariable {
        IndexedVariable {
            name: name.to_owned(),
            kind: "algebraic",
            source_byte_range: (0, 0),
            source_line: line,
            def_id: None,
            description: None,
        }
    }

    /// **A synthesized `pre` slot must be named as generated, even when the index
    /// carries a line for it.**
    ///
    /// This is the assertion the extraction exists for, and it is invisible from
    /// downstream: a pre-slot inherits the source line of the variable it shadows, so
    /// an implementation that consulted the index first would say "declared at line
    /// 41" and send the reader to the declaration of a *different* variable. Both
    /// branches render a plain grey label, so on screen the wrong one looks right.
    ///
    /// Must-fire: swap the two match arms and the first assertion fails.
    #[test]
    fn a_generated_pre_slot_is_named_generated_rather_than_declared() {
        let mut index = IdentifierIndex::default();
        // The trap, made real: the index *does* hold a line for the slot.
        index
            .variables
            .insert("__pre__.h".to_owned(), indexed("__pre__.h", 41));

        let mut h = harness(Bar {
            tracked: Some("__pre__.h".to_owned()),
            index: Some(index),
            ..Bar::default()
        });
        h.run_steps(2);

        assert!(
            h.query_by_label_contains("generated: pre(h)").is_some(),
            "a pre-slot is named by Rumoca's own inverse, not by the index",
        );
        assert!(
            h.query_by_label_contains("declared at line").is_none(),
            "and the inherited line must not be reported as this variable's declaration",
        );
    }

    /// **The two × buttons are not interchangeable.**
    ///
    /// They render identically and sit rows apart; only the report distinguishes
    /// them. Swapping the two variants would clear the follow when the reader asked
    /// to drop the point — and `App` would happily re-emit the wrong context.
    #[test]
    fn the_point_and_the_follow_are_cleared_by_different_reports() {
        let context = ContextBarState {
            pointed_at: Some(a_point()),
            ..Default::default()
        };

        let mut h = harness(Bar {
            context,
            tracked: Some("h".to_owned()),
            ..Bar::default()
        });
        h.run_steps(2);

        // Row order is Pointing at, then Following, so the first × is the point's.
        let crosses: Vec<_> = h.get_all_by_label_contains("\u{00d7}").collect();
        assert_eq!(crosses.len(), 2, "one × per assembled row");
        crosses[0].click();
        h.run_steps(2);
        assert_eq!(h.state().reported, Some(ContextBarPress::ClearPoint));

        let crosses: Vec<_> = h.get_all_by_label_contains("\u{00d7}").collect();
        crosses[1].click();
        h.run_steps(2);
        assert_eq!(h.state().reported, Some(ContextBarPress::ClearThread));
    }

    /// A name the index does not know, but a component type does, is a **link** — and
    /// the class it reports is the one `App` opens.
    #[test]
    fn the_declaring_class_is_reported_by_name() {
        let mut declaring = HashMap::new();
        declaring.insert("motor.w".to_owned(), "Modelica.Electrical.DC".to_owned());

        let mut h = harness(Bar {
            tracked: Some("motor.w".to_owned()),
            declaring,
            ..Bar::default()
        });
        h.run_steps(2);

        h.get_all_by_label_contains("Modelica.Electrical.DC")
            .next()
            .expect("the declaring class is a link")
            .click();
        h.run_steps(2);

        assert_eq!(
            h.state().reported,
            Some(ContextBarPress::GoToClass(
                "Modelica.Electrical.DC".to_owned()
            )),
        );
    }

    /// The non-vacuity guard the other three need: without it, a bar that reported a
    /// press on every frame would pass all of them.
    #[test]
    fn drawing_the_bar_without_clicking_reports_nothing() {
        let context = ContextBarState {
            pointed_at: Some(a_point()),
            ..Default::default()
        };

        let mut h = harness(Bar {
            context,
            tracked: Some("h".to_owned()),
            ..Bar::default()
        });
        h.run_steps(4);

        assert_eq!(h.state().reported, None);
    }
}
