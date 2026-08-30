//! **The stage tabs** — one tab per compilation phase, plus Simulation.
//!
//! Lifted out of `App::stage_tab_bar_ui` on 2026-08-19. See
//! [`docs/app-split-plan.md`](../docs/app-split-plan.md).
//!
//! # Why the *middle* of the function, and not the function
//!
//! `stage_tab_bar_ui` was 280 lines touching **12** of `App`'s fields and calling **two**
//! `App` methods — `open` (the Debug-mode specimen switcher) and `start_simulation` (the
//! inline ▶ button). Both are presses, so the callback pattern that carried
//! [`crate::specimen_source`] and [`crate::tour_panel`] out would have applied — but
//! **deferring either one changes what this frame draws**:
//!
//! - `App::open` sets `compiling`, clears the stage bundle and switches to the log view,
//!   and the tabs below it read all three. Reporting the press instead of performing it
//!   would draw one frame of the *previous* specimen's tabs, highlighted and enabled.
//! - `App::start_simulation` sets `sim_running`, and the spinner three lines later reads
//!   it.
//!
//! So the region rule applied instead (`app-split-plan.md`): **which contiguous span of the
//! body calls no `App` method?** Everything after the ▶ button — the tab row proper — and
//! that span is 163 of the 280 lines. What stays behind in `App::stage_tab_bar_ui` is the
//! chrome that genuinely needs the application: the specimen switcher, the Log button, and
//! the compile/nav spinners.
//!
//! # The ▶ button moved in on 2026-08-29, and the second bullet above is why it cost a frame
//!
//! Doug asked for it beside the Simulation tab rather than beside the Log button, where it
//! read as general chrome rather than as belonging to simulation. It now reports
//! [`TabClick::RunSimulation`] and `App` performs the run — the same render-and-report shape
//! as the tab clicks.
//!
//! **The consequence the bullet predicted is real and is accepted.** `sim_running` now
//! arrives as a parameter computed *before* the row draws, so on the click frame it is still
//! false and the spinner does not appear until the next frame — which egui paints
//! immediately after an interaction. This is exactly [`crate::tour_panel`]'s *"Play is
//! deferred by exactly one frame"*, and the alternative was handing this row `&mut App`.
//!
//! # Why `selectable_label` and not `selectable_value`
//!
//! egui offers two selection widgets:
//!
//! - `selectable_value(&mut val, variant, text)` — **always** highlights when
//!   `val == variant`. Good for radio-button groups.
//! - `selectable_label(is_selected, text)` — highlights when the bool is true, and the
//!   caller owns the condition.
//!
//! This row needs the second, because it must **suppress** highlighting while a freshly
//! selected specimen is compiling: the previous specimen's stage must not appear selected
//! over an empty, loading one. That is the `stage_selected` bool below — false while
//! compiling *or* while the log view is showing — and `selectable_value` cannot express
//! it, because it always highlights the current value.
//!
//! # Tab colouring
//!
//! Every label goes through [`tab_label`]: **red** if the stage errored (so a pipeline
//! failure reads off the row without opening anything), **green** if it produced its IR,
//! and the theme's default colour if it was never reached or is still compiling.
//!
//! # What the row reports rather than performs
//!
//! Selecting a stage tab has two consequences that belong to the application, not to a tab
//! row: leaving the log view, and capturing the stage for the chat (which is why there is
//! no separate 🔎 button — the stage's context is ready the instant you view it). The
//! Simulation tab shares the first and not the second, because a run/plot action is not an
//! IR capture. **One return value distinguishes them**, and `App` performs both.
//!
//! That subsumes the `stage_tab_clicked` local the function used to carry — a flag set by
//! every tab and acted on once below the row, so the follow-up logic was not duplicated in
//! eleven click handlers. `click: Option<TabClick>` does the same job and *is* the return
//! value, so the accumulate-then-act shape survives without a second variable.

use eframe::egui;

use crate::diagnostics;
use crate::worker::{Stage, StageBundle, StageKind};

/// What the tab row did, for `App` to finish.
///
/// The **third instance** of the render-and-report pattern (`specimen_source` returned
/// `Option<String>`, `tour_panel` an `Option<TransportRequest>`), and the first where
/// variants differ only in their *consequence*: the two tab clicks both leave the log
/// view and only [`Stage`](TabClick::Stage) asks for a capture, while
/// [`RunSimulation`](TabClick::RunSimulation) does neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TabClick {
    /// An IR stage tab was clicked. The new stage is already written through
    /// `selected_stage`; what remains is to leave the log view and capture the stage.
    Stage,
    /// The Simulation tab was clicked. Leave the log view — but do not capture, because a
    /// run/plot action is not an IR capture.
    Simulation,
    /// **The ▶ button was pressed** — start a run and *stay where you are*.
    ///
    /// The only variant that does **not** leave the log view, and that is the whole
    /// point of the button: its hover has always read *"stays on the current view"*, so
    /// a run can be watched in the log or studied against the IR while it completes —
    /// sweeping it in with the other two would silently change what pressing ▶ does.
    ///
    /// Moved here from `App`'s chrome row on 2026-08-29 at Doug's request — beside the
    /// Simulation tab rather than beside the Log button, where it read as general chrome
    /// rather than as belonging to simulation.
    RunSimulation,
}

/// Draw the stage tabs and the Simulation tab, reporting what was clicked.
///
/// `selected_stage` is `&mut` because selecting a tab *is* the mutation — the row owns
/// which stage is current. Everything else is read-only, and the two `sim_*` flags are
/// `bool` rather than the `Option`s they come from because the row only ever asks whether
/// the last run errored or produced data; passing the payloads would overstate its reach.
///
/// `viewing_log` is by value even though the row's click handlers used to clear it. Both
/// writes happened *after* the only read (`stage_selected`, computed before any tab is
/// drawn), so reporting the click and letting `App` clear the flag is behaviour-identical.
pub(crate) fn stage_tabs_ui(
    ui: &mut egui::Ui,
    stages: &StageBundle,
    selected_stage: &mut StageKind,
    compiling: bool,
    viewing_log: bool,
    sim_errored: bool,
    sim_has_data: bool,
    can_sim: bool,
    sim_running: bool,
) -> Option<TabClick> {
    ui.separator();
    let err = ui.visuals().error_fg_color;
    let ok = crate::colors::ok_color(ui.visuals().dark_mode);
    // While a freshly-selected specimen is still compiling, NO tab is
    // highlighted — the previous specimen's stage must not appear selected
    // over an empty/loading one. The highlight returns once results land
    // (`*selected_stage` = the furthest clean stage). Hence `selectable_label`
    // with an explicit `stage_selected && …` bool, not `selectable_value`
    // (which would always highlight the current stage).
    //
    // Selecting an IR stage tab ALSO captures that stage for the chat (no
    // separate 🔎 button) — so its context is ready the instant you view
    // it; the capture fires once below. Simulation is excluded: it's a
    // run/plot action, not an IR capture.
    let stage_selected = !compiling && !viewing_log;
    let mut click: Option<TabClick> = None;
    let tabs: &[(StageKind, &str, &Stage, Option<&str>)] = &[
        (StageKind::Parse, "Parse", &stages.parse, None),
        (StageKind::Resolve, "Resolve", &stages.resolve, None),
        (
            StageKind::Instantiate,
            "Instantiate",
            &stages.instantiate,
            None,
        ),
        (
            StageKind::Typecheck,
            "Typecheck",
            &stages.typecheck,
            Some(
                "The model-scoped instanced typecheck: it types the instantiated \
                     overlay (fills in type_ids, evaluates dimensions), so it runs AFTER \
                     Instantiate — not in Rumoca's nominal phase-3 slot. HRW can't use the \
                     pre-instantiation whole-tree typecheck; it fails on the full MSL.",
            ),
        ),
        (StageKind::Flatten, "Flatten", &stages.flatten, None),
        (
            StageKind::Dae,
            "DAE",
            &stages.dae,
            Some(
                "DAE construction (Rumoca phase 6): the flat equation list becomes a \
                     mathematical system. Variables are partitioned into states (x), \
                     algebraics (y), inputs (u), parameters (p) and discretes (z, m); \
                     equations into the MLS Appendix B partitions — f_x (continuous), \
                     f_z / f_m (discrete updates), f_c (conditions). The note reports the \
                     counts, and it is the count that decides everything downstream: \
                     matching cannot assign one equation per unknown unless they agree.",
            ),
        ),
        (
            StageKind::Structural,
            "Structural",
            &stages.structural,
            Some(
                "Structural analysis of the RAW DAE (Rumoca phase 7): maximum matching \
                     (equation↔unknown), BLT blocks (size>1 = algebraic loop), and tearing. \
                     A high-index system (rigid constraints) reports SINGULAR here — see the \
                     Index reduction tab for the reduced, solvable form. BLT spy-plot (drag \
                     to pan, scroll to zoom, click a block to capture) or the raw report tree.",
            ),
        ),
        (
            StageKind::IndexReduction,
            "Index reduction",
            &stages.index_reduction,
            Some(
                "Structural analysis of the DAE AFTER index reduction (Pantelides / \
                     dummy derivatives): the funnel differentiates constraints and demotes states \
                     so a high-index singular system becomes matchable. For an already-index-1 \
                     model this equals Structural. Same BLT spy-plot / tree.",
            ),
        ),
        (
            StageKind::Initialization,
            "Initialization",
            &stages.initialization,
            Some(
                "The consistent-initial-condition solve plan (build_ic_plan): the \
                     ordered blocks that compute a valid state at t=0 — direct symbolic solves, \
                     scalar Newton, torn/coupled loops — plus the relaxation hint (equations \
                     dropped / unknowns pinned) when the initial subsystem is singular, and a \
                     determinacy check that flags an OVER-determined init (more explicit initial \
                     conditions than states — conflicting/redundant ICs).",
            ),
        ),
        (
            StageKind::Events,
            "Events",
            &stages.events,
            Some(
                "The DAE's hybrid / event structure: the conditions (relations that \
                     trigger events), the discrete updates lowered from `when` clauses (f_z real, \
                     f_m valued), and the event partition (zero-crossing root conditions + scheduled \
                     time events). A smooth (continuous) model shows none.",
            ),
        ),
        (
            StageKind::SolveLowering,
            "Solve lowering",
            &stages.solve_lowering,
            Some(
                "The DAE lowered to a SolveModel (phase 8): the solvable form the \
                     simulator runs — residual programs, variable layout, mass matrix, Jacobian \
                     sparsity. This is the compile step just before simulation.",
            ),
        ),
    ];
    for &(kind, label, stage, hover) in tabs {
        let mut resp = ui.selectable_label(
            stage_selected && *selected_stage == kind,
            tab_label(label, stage, ok, err),
        );
        // A tab click is a point-at too — at the stage as a
        // whole. Appended to the tab's own explanation rather
        // than replacing it: what the stage *is* matters more
        // than what clicking does, and this is the row where a
        // reader is most likely to be learning the pipeline.
        let tip = match hover {
            Some(t) => format!("{t}\n\n{}", crate::POINT_AT_HOVER),
            None => crate::POINT_AT_HOVER.to_owned(),
        };
        resp = resp.on_hover_text(tip);
        if resp.clicked() {
            diagnostics::record_action("stage-tab", kind.name());
            *selected_stage = kind;
            click = Some(TabClick::Stage);
        }
    }
    // Simulation is a run/plot action, not an IR capture — hence its own
    // variant, which `App` answers without asking for a stage capture.
    ui.separator();
    // **▶ sits between the divider and the label** (Doug, 2026-08-29). It used to live
    // in `App`'s chrome row beside the Log button, where it read as general chrome; here
    // it reads as belonging to the tab it acts on. `can_sim` and `sim_running` arrive as
    // plain bools for the reason this signature's other `sim_*` flags do — the row only
    // asks whether a run may start and whether one is going, and passing the `Option`s
    // they come from would overstate its reach.
    if ui
        .add_enabled(can_sim, egui::Button::new("▶"))
        .on_hover_text("Run simulation (stays on the current view)")
        .on_disabled_hover_text("Compile a specimen first")
        .clicked()
    {
        click = Some(TabClick::RunSimulation);
    }
    if sim_running {
        ui.spinner();
    }
    let sim_label = {
        let text = egui::RichText::new("Simulation");
        if sim_errored {
            text.color(err)
        } else if sim_has_data {
            text.color(ok)
        } else {
            text
        }
    };
    if ui
        .selectable_label(
            stage_selected && *selected_stage == StageKind::Simulation,
            sim_label,
        )
        .on_hover_text(
            "Run the model (phase 9): compile → lower to a SolveModel → integrate \
                     (Auto: BDF for stiff, RK45 otherwise), then plot the state trajectories. Runs \
                     on the worker thread, so the UI stays live.",
        )
        .clicked()
    {
        *selected_stage = StageKind::Simulation;
        click = Some(TabClick::Simulation);
    }
    click
}

/// A stage-tab label, coloured by outcome so the whole pipeline's health reads off
/// the tab row without opening each stage: **red** if the stage errored, **green**
/// if it produced its IR (succeeded), and the normal colour for an
/// in-between/neutral status — "not reached" after an upstream failure, or no data
/// yet (before/while compiling).
///
/// `RichText` is egui's styled-text type: you create it with `RichText::new(…)`
/// and chain formatting methods (`.color()`, `.monospace()`, `.strong()`, etc.).
/// The resulting `RichText` can be passed anywhere a label/button expects text.
/// Here we use `.color()` to tint the tab label — the text itself is unchanged,
/// only its rendering color varies based on the stage's outcome.
fn tab_label(
    label: &str,
    stage: &Stage,
    ok_color: egui::Color32,
    err_color: egui::Color32,
) -> egui::RichText {
    let text = egui::RichText::new(label);
    if stage.note_is_error() {
        text.color(err_color)
    } else if stage.value.is_some() {
        text.color(ok_color)
    } else {
        // No color override — uses the theme's default text color. This
        // neutral state covers "not yet reached" (an upstream stage failed
        // or hasn't completed) and "still compiling".
        text
    }
}

/// The tests the extraction bought.
///
/// **None of these could have been written before the cut**, and the reason is not that
/// the row was untested — `ui_tests` already drives it through a real `App` and asserts
/// that a tab click selects the stage, leaves the log view and reaches the Context Bar.
/// The reason is that those assertions all run *downstream* of the row, so they can only
/// see the consequences `App` chose to apply. **What the row itself reported was not
/// observable**, and the distinction that matters most here lives exactly there: the
/// Simulation tab and a stage tab produce the *same* visible outcome except for a capture
/// that a test would have to reach into the bridge to see.
///
/// The whole input is a [`StageBundle`] and three flags. No worker, no channels, no
/// compile.
#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    /// Everything the row reads, plus what it reported, so one harness closure can drive
    /// it and the assertions can read both sides.
    struct Row {
        stages: StageBundle,
        stage: StageKind,
        compiling: bool,
        viewing_log: bool,
        sim_errored: bool,
        sim_has_data: bool,
        can_sim: bool,
        sim_running: bool,
        /// The last non-`None` report. Kept separately from the per-frame return so a
        /// click observed on frame one is not erased by frame two's quiet redraw.
        reported: Option<TabClick>,
    }

    /// Written by hand rather than derived, because [`StageKind`] has no `Default` — it
    /// is a position in the pipeline and there is no neutral one. `Parse` is the first,
    /// which is what "nothing has happened yet" means here.
    impl Default for Row {
        fn default() -> Self {
            Self {
                stages: StageBundle::default(),
                stage: StageKind::Parse,
                compiling: false,
                viewing_log: false,
                sim_errored: false,
                sim_has_data: false,
                // **Enabled by default, unlike the other flags.** A disabled ▶ is not
                // clickable, so a default of `false` would make every test that presses
                // it silently observe nothing — the shape `CLAUDE.md` records as a
                // synthetic click landing on nothing.
                can_sim: true,
                sim_running: false,
                reported: None,
            }
        }
    }

    /// **The `horizontal_wrapped` matters and is not decoration.** Both real call sites
    /// wrap the row in one, and without it the tabs stack vertically and fall off the
    /// bottom of the viewport — where they are still in the accessibility tree, so a
    /// query finds them, and still unclickable, so a click silently does nothing. That
    /// failure looks exactly like "the row did not report the press".
    fn harness(row: Row) -> Harness<'static, Row> {
        Harness::builder()
            .with_size(egui::Vec2::new(1600.0, 400.0))
            .build_ui_state(
                |ui, r: &mut Row| {
                    ui.horizontal_wrapped(|ui| {
                        let click = stage_tabs_ui(
                            ui,
                            &r.stages,
                            &mut r.stage,
                            r.compiling,
                            r.viewing_log,
                            r.sim_errored,
                            r.sim_has_data,
                            r.can_sim,
                            r.sim_running,
                        );
                        if click.is_some() {
                            r.reported = click;
                        }
                    });
                },
                row,
            )
    }

    /// The ▶ button **reports a run, and does not touch the selection**.
    ///
    /// It moved into this row on 2026-08-29, and the risk was never the drawing. The row
    /// already reports two variants that both mean *leave the log view*, and ▶ means the
    /// opposite — its hover has always promised it "stays on the current view". So the
    /// variant is pinned here, and `App` keeps it out of the branch that clears
    /// `viewing_log`.
    #[test]
    fn the_run_button_reports_a_run_and_selects_nothing() {
        let mut h = harness(Row {
            stage: StageKind::Flatten,
            ..Row::default()
        });
        h.run_steps(2);

        h.get_all_by_label_contains("▶")
            .next()
            .expect("a run button")
            .click();
        h.run_steps(2);

        assert_eq!(
            h.state().reported,
            Some(TabClick::RunSimulation),
            "pressing run must report RunSimulation \u{2014} the one variant `App` answers \
             WITHOUT clearing the log view",
        );
        assert_eq!(
            h.state().stage,
            StageKind::Flatten,
            "and it must not move the selection: running a model is not viewing a stage, \
             which is why the button sits beside the Simulation tab rather than being it",
        );
    }

    /// A stage tab **selects the stage and reports a stage click**.
    #[test]
    fn a_stage_tab_selects_and_reports_a_stage_click() {
        let mut h = harness(Row {
            stage: StageKind::Parse,
            ..Row::default()
        });
        h.run_steps(2);

        h.get_all_by_label_contains("Flatten")
            .next()
            .expect("a Flatten tab")
            .click();
        h.run_steps(2);

        assert_eq!(
            h.state().stage,
            StageKind::Flatten,
            "the row owns the selection, so the click must land in `selected_stage`",
        );
        assert_eq!(
            h.state().reported,
            Some(TabClick::Stage),
            "and it must be reported as a STAGE click — that is what tells `App` to \
             capture the stage for the chat",
        );
    }

    /// The Simulation tab **is reported as its own thing**, not as a stage.
    ///
    /// This is the assertion the extraction exists for. Both tabs select something and
    /// both leave the log view, so an implementation that reported `TabClick::Stage` for
    /// Simulation would look identical on screen — and would silently start capturing IR
    /// context for a run/plot action, which is the one thing the row's comments have
    /// always said it must not do.
    #[test]
    fn the_simulation_tab_is_reported_separately_from_a_stage() {
        let mut h = harness(Row::default());
        h.run_steps(2);

        h.get_all_by_label_contains("Simulation")
            .next()
            .expect("the Simulation tab")
            .click();
        h.run_steps(2);

        assert_eq!(
            h.state().stage,
            StageKind::Simulation,
            "precondition: the click selected Simulation",
        );
        assert_eq!(
            h.state().reported,
            Some(TabClick::Simulation),
            "Simulation must NOT report a stage click — `App` reads the variant to decide \
             whether to capture, and a run is not an IR capture",
        );
    }

    /// **Silence must be a failure, never a pass** — a row that is merely drawn reports
    /// nothing.
    ///
    /// Without this the two tests above are satisfied by a function that returns a click
    /// unconditionally: every frame would leave the log view, and every frame would ask
    /// for a capture.
    #[test]
    fn drawing_the_row_without_clicking_reports_nothing() {
        let mut h = harness(Row {
            stage: StageKind::Dae,
            sim_errored: true,
            sim_has_data: true,
            ..Row::default()
        });
        h.run_steps(3);

        assert_eq!(
            h.state().reported,
            None,
            "no click happened, so nothing may be reported",
        );
        assert_eq!(
            h.state().stage,
            StageKind::Dae,
            "and the selection must be untouched",
        );
    }

    /// Every stage in this list **renders a tab**, and the Log does not.
    ///
    /// # What it does and does not catch — corrected 2026-08-22
    ///
    /// This doc comment used to claim the hand-written roster meant *"adding a stage to
    /// the pipeline and forgetting its tab fails by name."* **It does not, and the
    /// reasoning was circular:** a stage added to `StageKind::COMPILATION` but not to the
    /// tab array is also absent from the list below, so nothing queries it and this test
    /// stays green. It catches a tab being **removed**, never one being **omitted**.
    ///
    /// A wrong negative is the error nobody catches, because acting on it means *not
    /// looking* — and a doc comment promising a guarantee that is not there is worse than
    /// no test, since it tells the next reader the case is covered.
    ///
    /// **The omission case is now held by
    /// [`no_compilation_stage_is_missing_from_the_tab_roster`]**, which derives its
    /// expectation from the enum instead of restating it. This test keeps the half it
    /// genuinely owns: that the labels reach the screen.
    ///
    /// The Log button stayed behind in `App::stage_tab_bar_ui` with the rest of the
    /// chrome, and this records that boundary: if it ever reappears here, the two rows
    /// would both draw it.
    #[test]
    fn every_compilation_stage_has_a_tab_and_the_log_does_not() {
        let mut h = harness(Row::default());
        h.run_steps(2);

        for label in [
            "Parse",
            "Resolve",
            "Instantiate",
            "Typecheck",
            "Flatten",
            "DAE",
            "Structural",
            "Index reduction",
            "Initialization",
            "Events",
            "Solve lowering",
            "Simulation",
        ] {
            assert!(
                h.query_by_label_contains(label).is_some(),
                "the pipeline's `{label}` stage must have a tab",
            );
        }
        assert!(
            h.query_by_label_contains("Log").is_none(),
            "the Log button belongs to the surrounding row, not to the tabs",
        );
    }

    /// **A stage added to the pipeline but not to the tab row fails by name.**
    ///
    /// # The gap this closes
    ///
    /// The tab row is a **hand-written array** — `let tabs: &[(StageKind, …)]` — and it
    /// is not derived from `StageKind::COMPILATION`, not compiler-enforced, and was not
    /// covered: the sibling test above restates the roster by hand, so it can only notice
    /// a tab that *disappears*. A stage wired into the enum, the bundle, the notebook and
    /// the bridge but missing here would simply have no tab, and **a pane that is absent
    /// leaves no gap where it was** — the same shape as the stranded `Animate` arm and the
    /// Flatten stranding that the `app.rs` arc found by reading siblings as a column.
    ///
    /// `CLAUDE.md` states the rule this enforces: *"New pipeline stages must be wired into
    /// ALL per-stage systems."* Two of the three systems it names already have derived
    /// guards — `stage_file_names_covers_all_pipeline_stages` and
    /// `manifest_stage_rosters_match_the_pipeline`. **The tab row was the one without.**
    ///
    /// # Why it reads the source instead of the rendered row
    ///
    /// The labels are not the stage names — `Dae` renders as *"DAE"*, `IndexReduction` as
    /// *"Index reduction"* — so a rendered check needs a name→label mapping, which would
    /// be a third hand-written list restating the second. Reading the array literal needs
    /// no mapping and states the property directly: **every variant appears in the row's
    /// definition.** `doc_citations` already scans Rust source for invariants that live in
    /// the text rather than the types.
    #[test]
    fn no_compilation_stage_is_missing_from_the_tab_roster() {
        let src =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/stage_tabs.rs"))
                .expect("stage_tabs.rs must be readable");
        let after = src
            .split_once("let tabs: &[(StageKind")
            .expect("the tab row must still be built from a `tabs` array literal here")
            .1;
        let roster = after
            .split_once("\n    ];")
            .expect("the `tabs` array literal must terminate")
            .0;

        let missing: Vec<&str> = StageKind::COMPILATION
            .iter()
            .filter(|kind| !roster.contains(&format!("StageKind::{kind:?}")))
            .map(|kind| kind.name())
            .collect();
        assert!(
            missing.is_empty(),
            "these pipeline stages have no tab, so they are unreachable in the UI: {missing:?}\n\n\
             Add a row to the `tabs` array in `stage_tabs.rs`. Every stage must be wired into \
             ALL per-stage systems -- the tab row, stage-file publishing and the notebook trace.",
        );

        // Non-vacuity: passing must mean the roster was found and read, never that the
        // split returned an empty slice that trivially contains nothing.
        let found = StageKind::COMPILATION
            .iter()
            .filter(|kind| roster.contains(&format!("StageKind::{kind:?}")))
            .count();
        assert_eq!(
            found,
            StageKind::COMPILATION.len(),
            "expected every stage to be found in the roster slice; the parse may have \
             captured the wrong region",
        );
    }
}
