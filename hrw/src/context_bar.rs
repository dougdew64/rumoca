//! **The Context Bar's assembled state** — what is pointed at, what is being
//! followed, and what is always context.
//!
//! Lifted out of `App::context_bar_ui` on 2026-08-19. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
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
//!   same order, one function boundary later.
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

use crate::app::ContextBarState;
use crate::identifier_index::IdentifierIndex;
use crate::worker::{DefInfo, StageBundle, StageKind};

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

/// The bar, once something is assembled.
///
/// The caller guarantees that `context.pointed_at` or `tracked_identifier` is
/// `Some` — the empty state is `App`'s branch, not this one — and that
/// `refresh_jump_matches` has already run for the current stage and follow.
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
    has_specimen: bool,
) -> Option<ContextBarPress> {
    let mut press: Option<ContextBarPress> = None;

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Context").strong());
        background_ui(ui, model, has_specimen, stage);
        if let Some(point) = &context.pointed_at {
            // Worth saying only when it **differs** from the background
            // stage; otherwise it repeats the line above as if it were a
            // second, independent fact.
            if point.stage != stage {
                ui.weak(format!("\u{00b7} pointed at in {}", point.stage.name()));
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

    if let Some(point) = &context.pointed_at {
        let request = point.request.as_str();
        let target = point.target.clone();
        ui.horizontal(|ui| {
            ui.weak("   Pointing at  ");
            ui.label(egui::RichText::new(&target).monospace());
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
            ui.weak("   Following    ");
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

    // Standing context — true for the whole session, and never previously
    // stated anywhere. Without it the user underestimates what Claude can
    // already see without doing anything.
    ui.horizontal(|ui| {
        ui.weak("   Always       ");
        let stage_count = stages
            .as_stage_pairs()
            .iter()
            .filter(|(_, v)| v.is_some())
            .count();
        ui.weak(format!(
            "{stage_count} stage IRs \u{00b7} {} DefIds",
            def_index.len(),
        ))
        .on_hover_text(
            "Every pipeline stage's full IR is on disk under .hrw-bridge/stages/, \
                 and the DefId table resolves numeric ids to names. Claude reads these \
                 without you pointing at anything.",
        );
    });
    ui.separator();
    press
}

/// The **background**: specimen and stage, always context, always shown.
///
/// `docs/context-assembly.md`: *"Specimen and stage are always context, so they are
/// always shown."* One renderer for both branches of the bar — see the module doc
/// for what happened when there were two.
pub(crate) fn background_ui(
    ui: &mut egui::Ui,
    model: Option<&str>,
    has_specimen: bool,
    stage: StageKind,
) {
    match (model, has_specimen) {
        (Some(model), _) => {
            ui.weak(format!("\u{00b7} {model} \u{00b7} {}", stage.name()));
        }
        // Mid-compile, or a compile that yielded no model name: still name the
        // stage rather than showing a bare "Context".
        (None, true) => {
            ui.weak(format!("\u{00b7} {}", stage.name()));
        }
        (None, false) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{PointKind, PointedAt};
    use crate::bridge;
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
                        true,
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
