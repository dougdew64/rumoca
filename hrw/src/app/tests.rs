//! **The unit tests for [`super`]** — every `#[cfg(test)]` block that used to be the
//! tail of `app.rs`, moved out on 2026-08-20. See
//! [`docs/app-split-plan.md`](../../docs/app-split-plan.md).
//!
//! # This is a size move, not a seam
//!
//! Nothing was refactored and no behaviour was touched: `app.rs` was 12,250 lines of
//! which **5,613 were test code**, every line from 6,638 to the end. Halving what a
//! session must hold to edit the file is the whole return. A later reader must not
//! mistake the step in the progress table for 5,613 lines of extraction work.
//!
//! # The mechanism, which needs no `#[path]` and no `mod.rs`
//!
//! Rust 2018 lets a *file* module own a subdirectory: `src/app.rs` declares
//! `#[cfg(test)] mod tests;` and the body lives in `src/app/tests.rs`. Three facts make
//! the move semantics-free, and each was checked rather than assumed:
//!
//! - **`super` still means `app`**, so every `use super::*;` is unchanged.
//! - **A child module sees its parent's private items** — the reason these tests can
//!   touch `App`'s private fields. That relationship is unchanged; it is the same one
//!   the block below already relies on.
//! - **An inherent `impl App` is legal in any module of the same crate**, so the
//!   test-only accessor block moves with the rest and its `pub(crate)` methods stay
//!   visible to `ui_tests` — a *sibling* of `app`, which is why they are `pub(crate)`
//!   rather than private in the first place.
//!
//! # Why the bulk block was FLATTENED into this file rather than nested
//!
//! The plan said "keep five blocks, moved verbatim". That was written before the
//! consequence was visible: this file *is* the module `app::tests`, so nesting the old
//! `mod tests { … }` inside it would rename ~140 test paths to `app::tests::tests::…`
//! and **falsify fifteen references** in `DECISIONS.md`, `docs/`, `arch_doc.rs`,
//! `doc_citations.rs` and `ui_tests.rs` that cite tests as `app::tests::<name>`. Those
//! are exactly the expired-comment class this refactor keeps finding.
//!
//! So the old `mod tests` body became this file's body — **every one of those paths is
//! byte-identical to what it was before the move**, which was verified by diffing
//! `cargo test --lib -- --list` across the change. The three smaller `tests_*` modules
//! stay nested (they gain a `tests::` prefix; nothing cites them) and the accessor
//! `impl` stays a sibling of them.
//!
//! Their `#[cfg(test)]` attributes are kept even though this file only exists under
//! `cfg(test)`. They are redundant, not false, and keeping them is what makes the diff
//! checkable as a pure move.

use super::*;

#[cfg(test)]
impl App {
    /// pub(crate) so the headless UI tests in a sibling module can build an App.
    pub(crate) fn test_default() -> Self {
        Self::test_with_sender().0
    }

    // ---- Test-only accessors for the headless UI suite -------------------
    //
    // `app::tests` reaches `App`'s private fields because it is a child module.
    // `ui_tests` is a **sibling**, so it cannot — and the fix is not to widen the
    // fields. Production encapsulation is unchanged; these exist only under
    // `cfg(test)` and say so by name.

    /// Whether the right-hand side is showing the log rather than a stage.
    pub(crate) fn test_viewing_log(&self) -> bool {
        self.viewing_log
    }

    /// Select a fixture lab by file stem, as clicking it in the picker would.
    ///
    /// Reads the file immediately rather than waiting for the poll interval, so a
    /// headless test does not have to sleep to see the lab it just chose.
    pub(crate) fn test_select_fixture_lab(&mut self, stem: &str) -> bool {
        let Some(path) = bridge::fixture_labs()
            .into_iter()
            .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(stem))
        else {
            return false;
        };
        self.select_lab(LabSource::Fixture(path));
        self.lab.polled_at = None;
        self.poll_lab_file();
        self.lab.text().is_some()
    }

    /// Start a self-running walk, as the Play button does.
    pub(crate) fn test_start_autoplay(&mut self) {
        self.start_autoplay();
    }

    /// Stop a run, as the Stop button does.
    pub(crate) fn test_stop_autoplay(&mut self) {
        self.lab.autoplay.stop();
        self.restore_mode_after_autoplay();
    }

    /// The autoplay clock's phase, for asserting a run is actually under way.
    pub(crate) fn test_autoplay_phase(&self) -> crate::autoplay::Phase {
        self.lab.autoplay.phase()
    }

    /// Beats done and beats total, as the readout shows them.
    pub(crate) fn test_autoplay_progress(&self) -> (usize, usize) {
        self.lab.autoplay.progress()
    }

    /// Put the right-hand side on the log, as it is while a compile runs.
    pub(crate) fn test_view_log(&mut self) {
        self.viewing_log = true;
    }

    /// The left panel's share of the window, as last drawn.
    pub(crate) fn test_split_fraction(&self) -> Option<f32> {
        // **What was drawn**, not what is remembered — the two differ while the panel
        // is pinned, and a layout test is asking about the screen.
        self.split.last_rendered
    }

    /// The width the LHS content was laid out against, for comparison with the
    /// panel's own width — a gap between them is the content detaching from the
    /// divider.
    pub(crate) fn test_split_inner_width(&self) -> Option<f32> {
        self.split.inner_width
    }

    /// Whether a reset back to the 40/60 default is queued for the next paint.
    pub(crate) fn test_split_reset_pending(&self) -> bool {
        self.split.resetting()
    }

    /// Put a **flagged** stage on screen: a real artifact carrying an error beside it.
    ///
    /// The state that showed only the error until 2026-08-05 — `Stage::recovered`'s
    /// value is the last good artifact *plus* an `error` key, and the pane rendered
    /// the summary in place of the tree. Doug found it walking `failure-typecheck.md`.
    pub(crate) fn test_set_flagged_stage_with_artifact(&mut self, kind: StageKind) {
        self.stage = kind;
        self.model = Some("Fixture".to_owned());
        self.selected = Some(PathBuf::from("Fixture.mo"));
        *self.stages.get_mut(kind) = crate::worker::Stage::recovered(
            serde_json::json!({
                "components": { "small": { "kind": "Real" } },
                "error": { "kind": "typecheck", "message": "dimension mismatch 2 vs 3" },
            }),
            "typecheck: 1 diagnostic(s)",
        );
    }

    /// Put a **failed** stage on screen whose value is nothing but an error.
    ///
    /// The contrast case for [`test_set_flagged_stage_with_artifact`]: there is no
    /// artifact, so the summary is the whole content and no tree should appear.
    pub(crate) fn test_set_failed_stage_error_only(&mut self, kind: StageKind) {
        self.stage = kind;
        self.model = Some("Fixture".to_owned());
        self.selected = Some(PathBuf::from("Fixture.mo"));
        *self.stages.get_mut(kind) = crate::worker::Stage::err_with_details(
            serde_json::json!({
                "kind": "todae",
                "message": "unbalanced model: 2 equations, 3 unknowns",
            }),
            "unbalanced model",
        );
    }

    /// Put lab text on screen **without touching the disk**.
    ///
    /// **Added 2026-08-05, because two tests were reading the live ad hoc lab.**
    /// `.hrw-bridge/lab.md` is gitignored and ephemeral by construction — Claude
    /// overwrites it every time he answers a question — and
    /// `a_stop_needing_a_specimen_is_refused_with_a_visible_notice` and
    /// `a_lab_link_acts_when_clicked_in_isolation` both clicked a link that happened
    /// to be in whatever answer was last written. **They passed for months on content
    /// no one had chosen**, and broke the moment an answer was written that did not
    /// contain that link.
    ///
    /// A test whose fixture is a scratch file is not testing what it says it tests.
    pub(crate) fn test_set_lab_text(&mut self, markdown: &str) {
        self.lab.selected = Some(LabSource::AdHoc);
        self.lab.cached = Some((markdown.to_owned(), std::time::SystemTime::now()));
        // Far in the future so `poll` does not immediately re-read the real file and
        // replace what this just set.
        self.lab.polled_at = Some(std::time::Instant::now());
        self.ui_mode = UiMode::Lab;
    }

    /// Drive the "Show in the Modelica source" verb, as the context menu does.
    pub(crate) fn test_dispatch_show_source(&mut self, line: u32) {
        self.dispatch_hrw_link(HrwLink::ShowSource(Some(line)));
    }

    /// The line a jump landed on, washed in the source view.
    pub(crate) fn test_source_jump_line(&self) -> Option<u32> {
        self.source.jump_line
    }

    /// Whether the source view is the visible specimen detail.
    pub(crate) fn test_specimen_detail_is_source(&self) -> bool {
        self.specimen_detail == SpecimenDetail::Source
    }

    /// Reset per-specimen state, as loading a different model does.
    pub(crate) fn test_clear_specimen_state(&mut self) {
        self.clear_specimen_state(false);
    }

    pub(crate) fn test_stage(&self) -> StageKind {
        self.stage
    }

    pub(crate) fn test_model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub(crate) fn test_selected_name(&self) -> Option<String> {
        self.selected.as_ref().map(|p| p.display().to_string())
    }

    pub(crate) fn test_selection_is_library(&self) -> bool {
        self.selected_is_library
    }

    pub(crate) fn test_set_filter(&mut self, s: &str) {
        self.model_list.filter = s.to_owned();
    }

    /// Populate the HRW specimen list without touching the filesystem.
    ///
    /// **Also parks the scratch poll**, which would otherwise fire on the first
    /// frame, see a bridge directory that disagrees with this injected list, and
    /// `rescan()` it straight back to empty from a `specimen_dir` no test sets.
    ///
    /// Not hypothetical: the first version of the divider tests passed
    /// **vacuously** because of it — "no specimen row is rendered" is true of a
    /// collapsed section and equally true of a list with nothing in it. The
    /// identical trap took the corpus test earlier the same day.
    /// Put a specimen's source on screen and aim a programmatic scroll at a line,
    /// without a compile.
    pub(crate) fn test_set_source(&mut self, text: &str, scroll_to_line: u32) {
        self.selected = Some(PathBuf::from("Fixture.mo"));
        self.source.text = Some(text.to_owned());
        self.source.highlight = None;
        self.source.scroll_target = Some(scroll_to_line);
        self.specimen_detail = SpecimenDetail::Source;
    }

    pub(crate) fn test_source_scroll_offset(&self) -> egui::Vec2 {
        self.source.scroll_offset
    }

    /// Show the Purpose tab, with the given model and selection.
    ///
    /// Both are inputs to `purpose_placeholder`, which picks a *different* message
    /// for each combination — so a test that sets only one is testing a state the
    /// pane does not distinguish.
    pub(crate) fn test_show_purpose(&mut self, model: Option<&str>, selected: Option<&str>) {
        self.specimen_detail = SpecimenDetail::Purpose;
        self.model = model.map(str::to_owned);
        self.selected = selected.map(PathBuf::from);
    }

    /// Put a library model on screen whose declaring file could not be read.
    pub(crate) fn test_set_library_source_error(&mut self, qualified: &str, why: &str) {
        self.selected = Some(PathBuf::from(qualified));
        self.selected_is_library = true;
        self.specimen_detail = SpecimenDetail::Source;
        self.source.text = None;
        self.source.library_error = Some(why.to_owned());
    }

    /// Put a library model on screen with its declaring file's text.
    pub(crate) fn test_set_library_source(&mut self, qualified: &str, uri: &str, text: &str) {
        self.selected = Some(PathBuf::from(qualified));
        self.selected_is_library = true;
        self.specimen_detail = SpecimenDetail::Source;
        self.model = Some(qualified.rsplit('.').next().unwrap_or(qualified).to_owned());
        self.source.library_uri = Some(uri.to_owned());
        self.source.library_error = None;
        self.source.text = Some(text.to_owned());
        self.source.highlight = None;
    }

    /// Select a **library** model whose text has not arrived from the worker yet.
    ///
    /// The state that used to make the source pane read a qualified name off disk,
    /// get an empty string, and print "Select a specimen to view its source" while a
    /// model was selected.
    pub(crate) fn test_select_library_awaiting_source(&mut self, qualified: &str) {
        self.selected = Some(PathBuf::from(qualified));
        self.selected_is_library = true;
        self.specimen_detail = SpecimenDetail::Source;
        self.source.text = None;
        self.source.library_error = None;
        self.source.load_error = None;
    }

    /// What the source pane would say, given the current state.
    pub(crate) fn test_source_load_error(&self) -> Option<&str> {
        self.source.load_error.as_deref()
    }

    /// Put a message in the status bar, as any refusal or result would.
    pub(crate) fn test_set_notice(&mut self, s: &str) {
        self.notice = Some(s.to_owned());
    }

    /// Fill the compilation log and open the log view.
    pub(crate) fn test_set_log(&mut self, lines: &[(crate::worker::LogLevel, &str)]) {
        self.log_entries = lines
            .iter()
            .enumerate()
            .map(|(i, (level, message))| LogEntry {
                elapsed_secs: i as f64 * 0.1,
                level: *level,
                message: (*message).to_owned(),
                depth: 0,
            })
            .collect();
        self.viewing_log = true;
    }

    /// Show the log view with nothing in it.
    pub(crate) fn test_view_empty_log(&mut self) {
        self.log_entries.clear();
        self.viewing_log = true;
    }

    pub(crate) fn test_set_specimen_files(&mut self, names: &[&str]) {
        self.model_list.files = names.iter().map(PathBuf::from).collect();
        self.model_list.polled_at = Some(std::time::Instant::now());
    }

    pub(crate) fn test_set_ui_mode_specimen(&mut self) {
        self.ui_mode = UiMode::Specimen;
    }

    /// Push a navigation entry, as "Go to definition" would.
    ///
    /// **The navigation view is the branch of `central_panel_ui` that draws no tab
    /// row**, so anything added to the stage side above can miss it silently — which
    /// is exactly what the Context Bar did until it was made unconditional.
    pub(crate) fn test_push_nav(&mut self, name: &str) {
        self.nav.push(NavEntry {
            name: name.to_owned(),
            value: serde_json::json!({}),
            def_index: BTreeMap::new(),
        });
    }

    /// Drive a link the way a lab click would, without a rendered hyperlink.
    pub(crate) fn follow_link_for_test(&mut self, url: &str) {
        if let Some(link) = parse_hrw_link(url) {
            self.dispatch_hrw_link(link);
        }
    }

    /// Seed a captured lab passage, as pressing 🎯 would leave it.
    ///
    /// **Sets the point directly rather than driving the button**, deliberately: the
    /// capture needs a real label selection, which a headless harness cannot make, and
    /// the copy round trip is already pinned by
    /// [`the_copy_catcher_runs_after_plugins_registered_before_it`]. What is untested
    /// without this is the last hop — whether the bar *renders* what was captured.
    pub(crate) fn test_point_at_lab_passage(&mut self, lab: &str, text: &str) {
        let seq = self.context.next_seq();
        self.context.pointed_at = Some(PointedAt {
            seq,
            target: text.to_owned(),
            kind: PointKind::LabPassage {
                lab: lab.to_owned(),
            },
            stage: None,
            request: bridge::AskRequest::Explain,
        });
    }

    /// Put the right-hand side into the state a walked-into lab would leave.
    pub(crate) fn test_set_walked_state(&mut self, specimen: &str, model: &str, stage: StageKind) {
        self.selected = Some(PathBuf::from(specimen));
        self.model = Some(model.to_owned());
        self.stage = stage;
        // **Seeded, because a walked state implies the source was read.** These
        // fixtures name files that do not exist (`RcCircuit.mo`), which was harmless
        // only while a failed read silently produced an empty string. Once the sweep
        // made that failure visible (2026-08-04) the pane began reporting it, which
        // is correct — and a fixture in a state the real app cannot reach is testing
        // something that does not happen.
        //
        // The text deliberately does **not** contain the model name: several tests
        // assert on the *Context Bar* by looking for the specimen name, and any
        // source on screen would give them a second match. That coupling is itself
        // fragile and is logged in the UI-testing debt.
        self.source.text = Some("// (fixture source)\n".to_owned());
        self.source.load_error = None;
    }

    /// Seed one equation-sheet row, so a harness frame has something to publish.
    ///
    /// Index 0 deliberately, so the assertion can look for the literal `f_x[0]` — the
    /// id form the other views use.
    pub(crate) fn test_set_equation_sheet_for_publish(&mut self) {
        self.viewport.flatten = FlattenView::Equations;
        self.cached_equation_sheet = Some(equation_sheet::EquationSheet {
            groups: vec![(
                equation_sheet::EquationCategory::Connection,
                vec![equation_sheet::FormattedEquation {
                    index: 0,
                    text: "0 = src.p.v - R.p.v".to_owned(),
                    origin: "connection equation".to_owned(),
                    category: equation_sheet::EquationCategory::Connection,
                    source_lines: vec![],
                }],
            )],
            n_equations: 1,
            ..equation_sheet::EquationSheet::default()
        });
    }

    /// Drop the model name, leaving the selection: the mid-compile state.
    pub(crate) fn test_clear_model(&mut self) {
        self.model = None;
    }

    fn test_with_sender() -> (Self, std::sync::mpsc::Sender<FromWorker>) {
        let (tx, _) = std::sync::mpsc::channel();
        let (from_tx, rx) = std::sync::mpsc::channel();
        let app = App {
            pending_passage: None,
            // No end-of-pass callback in a bare test App, so nothing ever fills it.
            copy_sink: Default::default(),
            worker: Worker {
                tx,
                rx,
                send_failed: false,
            },
            libraries_text: String::new(),
            library_status: String::new(),
            libraries_busy: false,
            // **An empty `dir`, unlike the real default.** A test must not scan
            // the developer's `specimens/`, or its results depend on what is
            // checked out.
            model_list: ModelListState {
                dir: String::new(),
                ..ModelListState::default()
            },
            selected_is_library: false,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Parse,
            def_index: BTreeMap::new(),
            nav: Vec::new(),
            nav_loading: None,
            nav_error: None,
            notice: None,
            ui_mode: UiMode::Lab,
            specimen_detail: SpecimenDetail::default(),
            show_settings: false,
            show_help: false,
            show_about: false,
            field_help: HashMap::new(),
            viewport: Viewport::default(),
            log_entries: Vec::new(),
            viewing_log: false,

            tracing_enabled: false,
            simulation: Stage::default(),
            sim_data: None,
            sim_running: false,
            sim_error: None,
            sim_t_end: 2.0,
            stage_views: StageViewCaches::default(),
            cached_equation_sheet: None,
            identifier_index: None,
            tracked_identifier: None,
            frames: CompileFrames::default(),
            cached_flat: None,
            compile_views: CompileViewCaches::default(),
            cached_dae: None,
            commonmark_cache: egui_commonmark::CommonMarkCache::default(),
            problem_lines: Vec::new(),
            split: SplitState::default(),
            context: ContextBarState::default(),
            source: SourceViewState::default(),
            lab: LabState::default(),
            aim_at_equation: None,
            seek_frame: None,
            cached_purpose_notes: HashMap::new(),
            known_variables: None,
            declaring_classes: HashMap::new(),
            pending_live_debug: None,
            live_breakpoint_armed: false,
            pending_stage: None,
            pending_sub_view: None,
            // **Done, so a test App can never arm the live-trace anchor.**
            //
            // This said "Tests drive `tick_prewarm` explicitly; nothing is armed
            // for them" and was **false**: `frame_ui` calls `tick_prewarm` on
            // every frame, so each `egui_kittest` harness armed a real breakpoint
            // in Doug's editor on its first paint — `.hrw-bridge/` is his running
            // VS Code's directory, not a fixture. He reported the breakpoint
            // twice; the first two fixes moved *tests* off the watched path and
            // missed this one, because no test body mentions the pre-warm at all.
            //
            // Isolation by construction beats isolation by convention: a source
            // check can only see what a test names, and this arrived through the
            // frame loop. `prewarm_arms_awaits_ack_then_removes` opts back in
            // deliberately, against a temp path.
            prewarm: Prewarm::Done,
        };
        (app, from_tx)
    }
}

/// Cycling, wrap-around, and the cache key that keeps "3 of 4" honest.
///
/// The index is per (stage, followed name). Carrying it across a stage
/// switch would leave it pointing into a list that no longer exists, and the
/// counter would be describing a different set of matches than the arrows
/// move through.
#[test]
fn jumping_cycles_within_the_current_stage_and_resets_across_stages() {
    let mut app = App::test_default();
    app.stages.flatten.value = Some(serde_json::json!({
        "variables": { "emf.w": 1 },
        "equations": [{ "text": "emf.w - der(emf.phi)" }, { "text": "emf.k * emf.w" }],
    }));
    app.stages.parse.value = Some(serde_json::json!({ "classes": { "M": { "name": "M" } } }));
    app.stage = StageKind::Flatten;
    app.tracked_identifier = Some("emf.w".to_owned());

    app.refresh_jump_matches();
    assert_eq!(
        app.context.jump_matches.len(),
        3,
        "one key plus two equations"
    );
    assert_eq!(app.context.jump_index, 0);

    // Forward through the list and around the end. Wrapping beats a dead
    // button: with a handful of matches, stopping is the worse surprise.
    app.jump_to_next_match(true);
    assert_eq!(app.context.jump_index, 1);
    app.jump_to_next_match(true);
    assert_eq!(app.context.jump_index, 2);
    app.jump_to_next_match(true);
    assert_eq!(
        app.context.jump_index, 0,
        "forward from the last match wraps"
    );
    app.jump_to_next_match(false);
    assert_eq!(app.context.jump_index, 2, "and back again from the first");

    // The jump must have asked the tree for something, and must have left
    // the log view — the matches live in a stage IR, so a jump with the log
    // showing would look broken.
    assert!(app.context.jump_target.is_some());
    assert!(!app.viewing_log);

    // Switching stage rebuilds the list and restarts the cycle.
    app.stage = StageKind::Parse;
    app.refresh_jump_matches();
    assert!(
        app.context.jump_matches.is_empty(),
        "emf.w does not exist before flattening"
    );
    assert_eq!(
        app.context.jump_index, 0,
        "a stale index would describe another stage's list"
    );

    // Nothing to jump to is not an error; the control simply does nothing.
    app.context.jump_target = None;
    app.jump_to_next_match(true);
    assert!(app.context.jump_target.is_none());

    // And with nothing followed at all, the list empties rather than lingering.
    app.stage = StageKind::Flatten;
    app.tracked_identifier = None;
    app.refresh_jump_matches();
    assert!(app.context.jump_matches.is_empty());
}

/// The empty-context hint must name a gesture that is actually available.
///
/// Regression for the exact state Doug hit: start HRW, switch to Specimen
/// mode, load a specimen. The first version of the hint said "left-click a
/// node to point at it, or right-click a variable name to follow it" and was
/// wrong twice over — the log view was showing so there was no tree and no
/// node, and the only clickable things on screen were source identifiers,
/// which are LEFT-click-to-follow.
///
/// A hint naming an unavailable gesture is worse than no hint, and it is the
/// same defect the Context Bar exists to prevent: a confident statement that
/// does not match the state.
#[test]
fn the_empty_hint_names_only_gestures_that_are_available() {
    let mut app = App::test_default();

    // The state Doug hit: specimen loaded, log showing, source on the left,
    // a compile finished so identifiers are underlined.
    app.ui_mode = UiMode::Specimen;
    app.specimen_detail = SpecimenDetail::Source;
    app.viewing_log = true;
    app.identifier_index = Some(identifier_index::IdentifierIndex::default());

    let hint = app.empty_context_hint();
    assert!(
        hint.contains("left-click an underlined identifier"),
        "must name the gesture that works here: {hint}",
    );
    assert!(
        !hint.contains("right-click"),
        "the source view has no context menu; naming one is the original bug: {hint}",
    );
    assert!(
        !hint.contains("a node to point at"),
        "no tree is showing, so there is no node to left-click: {hint}",
    );
    assert!(
        hint.contains("stage tab"),
        "the way to an IR view is a tab: {hint}"
    );

    // Before the compile lands there is no index, so nothing is underlined
    // and that gesture must not be offered.
    app.identifier_index = None;
    let hint = app.empty_context_hint();
    assert!(
        !hint.contains("underlined"),
        "nothing is clickable yet: {hint}"
    );

    // With a stage view open instead of the log, pointing is available.
    app.viewing_log = false;
    let hint = app.empty_context_hint();
    assert!(hint.contains("a node to point at"), "{hint}");
    assert!(!hint.contains("stage tab"), "a tab is already open: {hint}");
}

/// Recompiling the same specimen must not destroy the assembled context.
///
/// The workflow this broke: point at a node, ask for breakpoints, then
/// recompile to hit them — and the recompile wiped the very context that
/// motivated the breakpoints. Doug hit it the first time the breakpoints
/// actually fired.
///
/// Switching to a *different* specimen must still clear, because a key-path
/// addresses one model's IR and means nothing in another's.
#[test]
fn reselecting_the_same_specimen_keeps_the_context_but_switching_clears_it() {
    let (mut app, _tx) = App::test_with_sender();
    let motor = PathBuf::from("specimens/MotorWithBrake.mo");

    app.selected = Some(motor.clone());
    app.context.pointed_at = Some(PointedAt {
        seq: 1,
        target: "components.src.V".to_owned(),
        kind: PointKind::Stage,
        stage: Some(StageKind::Flatten),
        request: bridge::AskRequest::Explain,
    });
    app.tracked_identifier = Some("emf.w".to_owned());

    app.open(motor.clone());
    assert!(
        app.context.pointed_at.is_some(),
        "a reselect must keep the point"
    );
    assert_eq!(
        app.tracked_identifier.as_deref(),
        Some("emf.w"),
        "and the follow"
    );

    // The jump list belonged to the old IR and must not be reused, even
    // though the stage and followed name are unchanged — which is exactly
    // the key `refresh_jump_matches` caches on.
    assert!(
        app.context.jump_key.is_none(),
        "stale match list must be invalidated"
    );

    app.open(PathBuf::from("specimens/BouncingBall.mo"));
    assert!(
        app.context.pointed_at.is_none(),
        "a different specimen clears the point"
    );
    assert!(app.tracked_identifier.is_none(), "and the follow");
}

/// A retained point that no longer resolves is dropped, and says so.
///
/// Keeping it would leave the Context Bar naming a node that does not exist
/// and the emitted `node.subtree` as `null` — a confident claim about
/// nothing. A stage point cannot dangle, so it survives.
#[test]
fn a_retained_point_that_no_longer_resolves_is_dropped_out_loud() {
    let (mut app, _tx) = App::test_with_sender();
    app.stages.flatten.value = Some(serde_json::json!({ "variables": { "emf.w": 1 } }));

    // Addresses something the new IR does not have.
    app.context.pointed_at = Some(PointedAt {
        seq: 1,
        target: "variables.gone".to_owned(),
        kind: PointKind::Node(vec![Seg::Key("variables".into()), Seg::Key("gone".into())]),
        stage: Some(StageKind::Flatten),
        request: bridge::AskRequest::Explain,
    });
    app.revalidate_point_against_new_ir();
    assert!(
        app.context.pointed_at.is_none(),
        "a dangling point must not be kept"
    );
    let notice = app.notice.as_deref().unwrap_or_default();
    assert!(
        notice.contains("point dropped"),
        "the drop must be stated: {notice}"
    );
    assert!(
        notice.contains("variables.gone"),
        "and must name what was lost: {notice}"
    );

    // One that still resolves survives untouched.
    app.notice = None;
    app.context.pointed_at = Some(PointedAt {
        seq: 2,
        target: "variables.emf.w".to_owned(),
        kind: PointKind::Node(vec![Seg::Key("variables".into()), Seg::Key("emf.w".into())]),
        stage: Some(StageKind::Flatten),
        request: bridge::AskRequest::Explain,
    });
    app.revalidate_point_against_new_ir();
    assert!(
        app.context.pointed_at.is_some(),
        "a resolvable point survives a recompile"
    );
    assert!(app.notice.is_none(), "and says nothing");

    // A stage point cannot dangle — there is always a stage.
    app.context.pointed_at = Some(PointedAt {
        seq: 3,
        target: "stage".to_owned(),
        kind: PointKind::Stage,
        stage: Some(StageKind::Parse),
        request: bridge::AskRequest::Explain,
    });
    app.revalidate_point_against_new_ir();
    assert!(
        app.context.pointed_at.is_some(),
        "a stage point always resolves"
    );
}

/// Every live-debug variant must be recognised by the arming machinery.
///
/// Regression for the Debug button doing **nothing** on the `pre()`-lowering
/// view. `live_debug_poll` and `is_arming` compared variants by a
/// hand-written list of matching pairs — `(Matching, Matching) | (Tarjan,
/// Tarjan) | (Reduction, Reduction)` — so a fourth variant compiled cleanly
/// and silently never matched. No error, no arming badge, no session.
///
/// Iterating `ALL` rather than naming variants keeps this honest: a fifth
/// view added without touching the arming code still gets checked here,
/// which is the point. Derived `PartialEq` is what makes it pass, but the
/// test is what makes the next omission loud.
#[test]
fn every_live_debug_variant_is_recognised_while_arming() {
    for &variant in PendingLiveDebug::ALL {
        let mut app = App::test_default();
        assert!(
            !app.is_arming(variant),
            "{variant:?} must not arm on its own"
        );

        app.pending_live_debug = Some((std::time::Instant::now(), variant));
        assert!(
            app.is_arming(variant),
            "{variant:?} armed but not recognised"
        );

        // ...and must not be mistaken for any other view's session.
        for &other in PendingLiveDebug::ALL {
            if other != variant {
                assert!(
                    !app.is_arming(other),
                    "{variant:?} armed, but {other:?} also reported arming",
                );
            }
        }
    }
}

/// No view may offer a live Debug session before its data exists.
///
/// The gate answers three questions at once, and this is the one with a
/// wrong answer that is *silent*: an enabled Debug button on a bare app arms
/// a breakpoint for an algorithm that has nothing to run on, and the reader
/// sees an "Arming…" badge that never becomes a session.
///
/// **Iterating `ALL` is the point, as it is in
/// `every_live_debug_variant_is_recognised_while_arming`.** Until the gate
/// existed, this composition — "has the data" AND "not already busy" — was
/// written out once per view, so it could only have been checked six times
/// by name, and a seventh view would have been checked zero times.
///
/// It touches no bridge file: with `pending_live_debug` unset,
/// `live_debug_poll` returns before it looks for an ack.
#[test]
fn no_variant_enables_debug_without_its_data() {
    let ctx = egui::Context::default();
    for &variant in PendingLiveDebug::ALL {
        let mut app = App::test_default();
        let gate = app.live_debug_gate(&ctx, variant, |a| &a.stage_views.matching_anim);
        assert!(
            !gate.debug_enabled,
            "{variant:?} offered Debug with no data loaded"
        );
        assert!(!gate.arming, "{variant:?} reported arming unprompted");
        assert!(
            !gate.spawn_live,
            "{variant:?} asked for a live session nobody requested"
        );
    }
}

/// **The badge and the spawn must be true on the SAME frame.**
///
/// [`App::live_debug_gate`]'s doc calls its order load-bearing; this is the
/// assertion that claim was missing. `live_debug_poll` clears
/// `pending_live_debug` on the frame the ack lands, so a gate that polled
/// *before* asking `is_arming` would report `arming: false` on exactly that
/// frame — and the live animation does not exist yet, because the caller
/// constructs it from `spawn_live`, after the gate returns. The result is one
/// frame in which a view mid-handshake claims nothing is happening.
///
/// **Nothing observable fails when that regresses**: the session still
/// starts and the animation still runs. That is the must-fire rule's own
/// shape — the only way to catch it is to look at the frame directly.
///
/// **This test is why [`App::live_debug_gate_at`] exists.** Reaching this
/// frame means making an ack land, and `.hrw-bridge/breakpoint-ack.json` is
/// shared by the whole suite — which is why the two tests that do use it each
/// have to be one function covering several paths, to avoid racing
/// themselves. Its own file costs nothing and races nobody.
#[test]
fn the_arming_badge_survives_the_frame_its_ack_lands() {
    let ctx = egui::Context::default();
    let ack = std::env::temp_dir().join("hrw-gate-order-ack.json");
    let _ = std::fs::remove_file(&ack);

    let (mut app, _tx) = App::test_with_sender();
    app.pending_live_debug = Some((std::time::Instant::now(), PendingLiveDebug::Matching));

    // A positive verdict, so the poll ends the wait on this very call
    // rather than requesting another repaint.
    std::fs::write(&ack, r#"{"version":2,"breakpointPresent":true}"#).unwrap();

    let gate = app.live_debug_gate_at(
        &ctx,
        PendingLiveDebug::Matching,
        |a| &a.stage_views.matching_anim,
        &ack,
    );

    assert!(
        gate.spawn_live,
        "the ack landed, so this is the frame the algorithm thread starts"
    );
    assert!(
        gate.arming,
        "\u{2026}and the badge must still be lit on that frame \u{2014} \
             `is_arming` is read BEFORE `live_debug_poll` consumes the handshake, \
             and swapping those two lines blanks the badge for one frame with no \
             other symptom"
    );
    assert!(
        app.pending_live_debug.is_none(),
        "the poll did consume the handshake \u{2014} without that, the \
             assertion above would pass for the wrong reason"
    );

    let _ = std::fs::remove_file(&ack);
}

/// Every combination of the two primitives must be reachable, and the
/// emitted file must describe each one honestly.
///
/// Doug asked directly ("So there is now support for all combinations of
/// context?") after the point became clearable. Reading the code says yes;
/// this says yes and keeps saying it. Four states, and the two that used to
/// be wrong are the ones with no point: **follow-only** emitted
/// `kind: "stage"`, attributing a subject the user never chose, and
/// **neither** was unreachable at all because the point could not be cleared.
///
/// Reads `.hrw-bridge/focus.json` because the file is the artifact that
/// matters — asserting on app fields would pass even if emission were broken,
/// which is precisely the bar/file disagreement this design keeps hitting.
#[test]
fn every_point_and_follow_combination_emits_honestly() {
    fn emitted() -> Value {
        let path = std::path::Path::new(bridge::BRIDGE_DIR).join("focus.json");
        let text = std::fs::read_to_string(path).expect("focus.json should exist");
        serde_json::from_str(&text).expect("focus.json should be valid JSON")
    }
    fn a_point() -> PointedAt {
        PointedAt {
            seq: 1,
            target: "components.src.V".to_owned(),
            kind: PointKind::Stage,
            stage: Some(StageKind::Flatten),
            request: bridge::AskRequest::Explain,
        }
    }

    let (mut app, _tx) = App::test_with_sender();

    // 1. Neither. Nothing is claimed at all.
    app.emit_context();
    let doc = emitted();
    assert_eq!(
        doc["kind"],
        serde_json::json!("none"),
        "no point must not become a stage"
    );
    assert!(doc.get("tracking").is_none(), "nothing is being followed");
    // `request` belongs to the point. With no point, defaulting it to
    // "explain" would claim an intent the user never expressed — the same
    // species of phantom as the `kind: "stage"` this test was written for.
    assert!(
        doc["request"].is_null(),
        "no point means no request: {}",
        doc["request"]
    );

    // 2. Follow only — the state Doug wanted and could not reach.
    app.set_tracked_identifier("h".to_owned());
    let doc = emitted();
    assert_eq!(doc["kind"], serde_json::json!("none"));
    assert_eq!(doc["tracking"]["identifier"], serde_json::json!("h"));
    assert!(
        doc["request"].is_null(),
        "following carries no point-request either"
    );

    // 3. Both, independent of each other.
    app.context.pointed_at = Some(a_point());
    app.emit_context();
    let doc = emitted();
    assert_eq!(doc["kind"], serde_json::json!("stage"));
    assert_eq!(doc["tracking"]["identifier"], serde_json::json!("h"));
    assert_eq!(
        doc["request"],
        serde_json::json!("explain"),
        "a point does carry one"
    );

    // 4. Point only — reached by dropping the follow, which must not
    //    disturb the point.
    app.set_tracked_identifier("h".to_owned()); // toggles it off
    assert!(
        app.tracked_identifier.is_none(),
        "clicking the followed name again clears it"
    );
    let doc = emitted();
    assert_eq!(doc["kind"], serde_json::json!("stage"));
    assert!(doc.get("tracking").is_none());
    assert!(
        app.context.pointed_at.is_some(),
        "dropping the follow must not drop the point"
    );

    // ...and back to neither, by clearing the point.
    app.context.pointed_at = None;
    app.context.point_error = None;
    app.context.context_seq = app.context.next_seq();
    app.emit_context();
    let doc = emitted();
    assert_eq!(doc["kind"], serde_json::json!("none"));
    assert!(doc.get("tracking").is_none());
}

/// Following is context, so changing it must re-emit — and must not destroy
/// the point. That independence is the property the Context Bar's honesty
/// rests on.
#[test]
fn following_re_emits_without_losing_the_point() {
    let (mut app, _tx) = App::test_with_sender();
    app.context.pointed_at = Some(PointedAt {
        seq: 3,
        target: "components.src.V".to_owned(),
        kind: PointKind::Node(vec![Seg::Key("components".into())]),
        stage: Some(StageKind::Flatten),
        request: bridge::AskRequest::Explain,
    });

    app.set_tracked_identifier("h".to_owned());
    assert_eq!(app.tracked_identifier.as_deref(), Some("h"));
    assert!(
        app.context.pointed_at.is_some(),
        "ambient following must not clear a deliberate capture"
    );
    assert_eq!(
        app.context.track_seq, 1,
        "the thread's own recency counter advanced"
    );

    // Un-following also re-emits, and still leaves the point alone.
    app.set_tracked_identifier("h".to_owned());
    assert!(app.tracked_identifier.is_none());
    assert!(app.context.pointed_at.is_some());
    assert_eq!(app.context.track_seq, 2);
}

/// One counter for both halves, so the two stamps are comparable.
///
/// Two independent counters *looked* comparable and were not: after twelve
/// captures and one follow they read 12 and 1, and the emitted instructions
/// told the reader to compare them. Found on the first real `explain`.
#[test]
fn point_and_thread_stamps_are_comparable() {
    let (mut app, _tx) = App::test_with_sender();

    app.emit_focus(Focus::Stage);
    let after_point = app.context.pointed_at.as_ref().unwrap().seq;

    app.set_tracked_identifier("h".to_owned());
    assert!(
        app.context.track_seq > after_point,
        "following happened later, so its stamp must be higher \
             (point {after_point}, thread {})",
        app.context.track_seq,
    );

    app.emit_focus(Focus::Stage);
    assert!(
        app.context.pointed_at.as_ref().unwrap().seq > app.context.track_seq,
        "pointing happened later, so now the point's stamp must be higher"
    );
}

/// Every capture shape is recorded, not just node captures.
///
/// Clicking a stage tab emits a *stage* capture. That path used to write
/// the file without updating `pointed_at`, so the emitted context changed
/// while the Context Bar kept displaying the previous node — the exact
/// drift the bar's rule forbids.
#[test]
fn stage_and_specimen_captures_are_recorded_too() {
    let (mut app, _tx) = App::test_with_sender();

    app.emit_focus(Focus::Stage);
    let point = app
        .context
        .pointed_at
        .as_ref()
        .expect("a stage capture is still a point");
    assert!(matches!(point.kind, PointKind::Stage));
    assert!(point.target.contains("stage"));

    app.emit_focus(Focus::Specimen);
    assert!(matches!(
        app.context.pointed_at.as_ref().unwrap().kind,
        PointKind::Specimen
    ));
}

/// The bar reports the stage the capture was *made* in. Switching tabs
/// afterwards must not change what it claims Claude has.
#[test]
fn the_point_remembers_its_own_stage() {
    let (mut app, _tx) = App::test_with_sender();
    app.stage = StageKind::Flatten;
    app.context.pointed_at = Some(PointedAt {
        seq: 1,
        target: "x".to_owned(),
        kind: PointKind::Stage,
        stage: Some(StageKind::Flatten),
        request: bridge::AskRequest::Explain,
    });

    app.stage = StageKind::Structural;
    assert_eq!(
        app.context.pointed_at.as_ref().unwrap().stage,
        Some(StageKind::Flatten),
        "the captured stage is a property of the capture, not of the view"
    );
}

/// The pre-warm state machine: arm → await ack → remove, and — critically —
/// abandon *without consuming the ack* if a Debug click takes over.
///
/// Both paths live in one test because they share a single
/// request/ack pair; as separate tests they would race each other (the same
/// reason `bridge`'s arm/remove/ack test is combined).
///
/// **Driven through a temp path, because the pre-warm arms for real.** Against
/// `.hrw-bridge/` this test put a breakpoint in Doug's editor on every run —
/// found 2026-08-15 by an ack timestamped inside a full gate run reading
/// `"action":"add"`, after a first fix that had moved only `bridge.rs`'s tests
/// off the watched path. See `App::tick_prewarm_at`.
#[test]
fn prewarm_arms_awaits_ack_then_removes() {
    let ctx = egui::Context::default();
    let (mut app, _tx) = App::test_with_sender();
    let request = std::env::temp_dir().join("hrw-prewarm-request.json");
    let ack_buf = request.with_file_name("breakpoint-ack.json");
    let ack = ack_buf.as_path();
    let _ = std::fs::remove_file(&request);
    let _ = std::fs::remove_file(ack);

    // **A test App starts Done, so nothing arms behind a test's back.** Pinned
    // here rather than assumed: `frame_ui` ticks the pre-warm every frame, so
    // any other default would have every UI harness arming a real breakpoint.
    assert_eq!(
        app.prewarm,
        Prewarm::Done,
        "a test App must not arm the anchor on its own",
    );
    // Opt in deliberately — this is the one test that drives the state
    // machine, and it does so against a temp path.
    app.prewarm = Prewarm::NotStarted;

    // First tick writes the arm request and begins waiting for the ack.
    app.tick_prewarm_at(&ctx, &request);
    assert!(
        matches!(app.prewarm, Prewarm::Awaiting(_)),
        "first tick should arm and wait, got {:?}",
        app.prewarm
    );

    // Without an ack it keeps waiting (the 3s timeout has not elapsed).
    app.tick_prewarm_at(&ctx, &request);
    assert!(
        matches!(app.prewarm, Prewarm::Awaiting(_)),
        "should still be waiting"
    );

    // The extension acks; the next tick removes the breakpoint and finishes.
    std::fs::write(ack, r#"{"acked":true}"#).unwrap();
    app.tick_prewarm_at(&ctx, &request);
    assert_eq!(
        app.prewarm,
        Prewarm::Done,
        "ack should complete the pre-warm"
    );
    assert!(!ack.exists(), "pre-warm consumes its own ack");

    // --- Abandon path: a Debug click owns the handshake mid-pre-warm. ---
    app.prewarm = Prewarm::NotStarted;
    app.tick_prewarm_at(&ctx, &request);
    assert!(matches!(app.prewarm, Prewarm::Awaiting(_)));

    std::fs::write(ack, r#"{"acked":true}"#).unwrap();
    app.pending_live_debug = Some((std::time::Instant::now(), PendingLiveDebug::Reduction));
    app.tick_prewarm_at(&ctx, &request);

    assert_eq!(
        app.prewarm,
        Prewarm::Done,
        "should abandon, not keep polling"
    );
    assert!(
        ack.exists(),
        "abandoning must NOT consume the ack — the Debug click is waiting for it"
    );

    let _ = std::fs::remove_file(ack);
    let _ = std::fs::remove_file(&request);
}

/// **Quitting HRW releases an armed live-trace breakpoint** (`docs/tech-debt.md`).
///
/// Every other removal site is reactive to an in-app event, so closing the
/// window used to leave the breakpoint registered in VS Code — a breakpoint
/// the user never set, which then stops any later debug session reaching
/// `live_trace.rs`.
///
/// Drives `release_live_breakpoint_at_exit` rather than `on_exit`, because
/// `eframe` owns the call to the latter and no test can reach it. Both
/// branches are checked: **doing nothing when nothing is armed matters as
/// much as acting when something is**, since an unconditional removal would
/// write a request on every quit and undo `#71`'s rule that HRW must not
/// assert state it cannot see.
#[test]
fn quitting_releases_an_armed_live_breakpoint() {
    let (mut app, _tx) = App::test_with_sender();
    let request = std::path::Path::new(bridge::BREAKPOINT_REQUEST_FILE);

    // --- Nothing armed: no request, no claim. ---
    let _ = std::fs::remove_file(request);
    app.live_breakpoint_armed = false;
    assert!(
        !app.release_live_breakpoint_at_exit(),
        "with nothing armed there is nothing to release"
    );
    assert!(
        !request.exists(),
        "a quit with no breakpoint must not write a removal request"
    );

    // --- Armed: the removal is issued and the flag drops. ---
    app.live_breakpoint_armed = true;
    assert!(
        app.release_live_breakpoint_at_exit(),
        "an armed breakpoint must be released on the way out"
    );
    assert!(
        !app.live_breakpoint_armed,
        "releasing it must also drop the claim that it is armed"
    );

    let text = std::fs::read_to_string(request)
        .expect("the removal request must reach the bridge as a file");
    let json: serde_json::Value =
        serde_json::from_str(&text).expect("the request must be valid JSON");
    assert_eq!(
        json["action"], "remove",
        "an exit must ask for removal, not arm another one: {text}"
    );
    assert!(
        json["breakpoints"][0]["path"]
            .as_str()
            .unwrap_or_default()
            .contains("live_trace"),
        "the request must name the live-trace anchor: {text}"
    );

    // **The removal request is left in place, deliberately.** `.hrw-bridge/`
    // is the live directory of Doug's VS Code, and `extension.ts` consumes a
    // request by reading it and unlinking it — so a request deleted before
    // its watcher fires is a request that never happened. Deleting a
    // *removal* here would discard the very instruction that releases the
    // anchor. `bridge::tests::DisarmAnchor` documents the full account.
    // Every test that inspects this file clears it on entry, so leaving it
    // is safe for the suite.
}

/// **A finished live session must not release the anchor** (`docs/ideas.md`
/// #74).
///
/// A safety net in [`App::live_debug_poll`] used to fire the moment a
/// session stopped being busy, on the reasoning that an armed breakpoint
/// with nothing in flight has nothing left to stop for. It was an LLDB
/// SIGSTOP workaround, and it silently destroyed the feature:
/// **`cppvsdbg` will not re-bind a breakpoint at a location whose
/// breakpoint left its active set earlier in the same debug session.** The
/// second Debug press armed `live_trace.rs:173`, VS Code drew it hollow,
/// and the algorithm ran to completion without stopping — no error
/// anywhere.
///
/// **The absence of a request is the whole assertion**, which is exactly
/// the shape the must-fire rule exists for: nothing observable fails when
/// this regresses, so the test has to look for the silence directly.
///
/// **It asserts unconditionally now, and that is a simplification worth
/// noting.** While the release was merely *gated* on a `cfg!(not(windows))`
/// constant, this test had to branch too — and its first draft branched on
/// **the constant itself**, which made it assert whatever the constant
/// already said: forcing the gate to `true` took the other branch and
/// **passed**. It was rewritten to branch on `cfg!(windows)` and verified
/// must-fire. Deleting the gate rather than keeping it removes the branch
/// from both, and with it the whole class of mistake.
#[test]
fn a_finished_live_session_keeps_the_anchor_armed() {
    let (mut app, _tx) = App::test_with_sender();
    let request = std::path::Path::new(bridge::BREAKPOINT_REQUEST_FILE);
    let ctx = egui::Context::default();

    let _ = std::fs::remove_file(request);
    app.live_breakpoint_armed = true;
    // No handshake in flight, so nothing here should touch the bridge.
    app.pending_live_debug = None;

    let action = app.live_debug_poll(
        &ctx,
        PendingLiveDebug::Matching,
        std::path::Path::new(bridge::BREAKPOINT_ACK_FILE),
    );
    assert!(
        matches!(action, LiveDebugAction::None),
        "a finished session with no pending handshake spawns nothing"
    );

    assert!(
        !request.exists(),
        "the session ending must not release the anchor \u{2014} cppvsdbg will \
             not re-bind a location it has released, so the next Debug press would \
             arm a breakpoint that never binds"
    );
    assert!(
        app.live_breakpoint_armed,
        "the breakpoint is still armed, and HRW must keep saying so \u{2014} \
             dropping the flag here would make the next exit skip its release"
    );

    let _ = std::fs::remove_file(request);
}

/// **An ack and a timeout are different outcomes, and only one of them arms
/// a breakpoint** (`docs/ideas.md` #71).
///
/// Both used to be one branch: `if acked || timed_out` set
/// `live_breakpoint_armed = true` and spawned the thread. With no extension
/// installed — the state of any fresh clone, since `out/` is gitignored —
/// HRW recorded a breakpoint that did not exist, ran the algorithm to
/// completion, and **said nothing**. The claim also left the screen: the
/// context capture emits `breakpoint_armed`.
///
/// # The four verdicts, and why they are four tests
///
/// [`bridge::check_breakpoint_ack_at`] reports one of four things, and
/// [`App::live_debug_poll`] owes each a different pair of answers — *does a
/// breakpoint exist*, and *what is the user told*. This test holds `Armed`;
/// its siblings hold the other three:
/// [`a_disabled_breakpoint_spawns_and_names_the_cause`] (`NotArmed`),
/// [`a_stale_bridge_reply_claims_nothing_and_names_its_fix`]
/// (`Unreportable`) and [`a_timed_out_arm_claims_nothing_and_says_so`]
/// (`Pending`, which is #71's own path).
///
/// **They were one `#[test]` until 2026-08-20**, and that function's doc
/// said why: *"Both paths share the single
/// `.hrw-bridge/breakpoint-ack.json` … as separate tests they would race for
/// that file."* **That sentence was a coupling measurement, and the
/// `ack_path` parameter added to [`App::live_debug_poll`] expired it** —
/// each verdict now drives a file nobody else reads, so a failure names the
/// verdict rather than a line number, and no path inherits the state the
/// previous one left behind.
#[test]
fn an_armed_verdict_starts_the_run_and_stays_quiet() {
    let ctx = egui::Context::default();
    let (mut app, _tx) = App::test_with_sender();
    let ack = std::env::temp_dir().join("hrw-verdict-armed-ack.json");
    let _ = std::fs::remove_file(&ack);

    // `#75`: the evidence is the *verdict*, not the reply. This payload used
    // to read `{"acked":true}`, which now means "cannot say" — see
    // `a_stale_bridge_reply_claims_nothing_and_names_its_fix`.
    std::fs::write(&ack, r#"{"version":2,"breakpointPresent":true}"#).unwrap();
    app.pending_live_debug = Some((std::time::Instant::now(), PendingLiveDebug::Reduction));

    let action = app.live_debug_poll(&ctx, PendingLiveDebug::Reduction, &ack);

    assert!(
        matches!(action, LiveDebugAction::SpawnLive),
        "an ack should start the algorithm thread"
    );
    assert!(
        app.live_breakpoint_armed,
        "a positive verdict is the evidence that a breakpoint exists"
    );
    assert_eq!(
        app.notice, None,
        "the happy path must stay quiet \u{2014} a notice on every Debug click \
             would train the eye to ignore the one that matters"
    );

    let _ = std::fs::remove_file(&ack);
}

/// **The bridge replied that nothing is armed, so HRW may not say otherwise**
/// — the `NotArmed` verdict of the four in
/// [`an_armed_verdict_starts_the_run_and_stays_quiet`].
///
/// The reachable case is a disabled breakpoint: one click of VS Code's
/// "Disable All Breakpoints" and the pre-`#75` code armed nothing, acked
/// true, and ran to completion in silence.
///
/// **The run still starts.** Refusing to run would be a worse answer than
/// running unstepped — the recorded animation is still worth watching, and
/// it is what the user asked for.
#[test]
fn a_disabled_breakpoint_spawns_and_names_the_cause() {
    let ctx = egui::Context::default();
    let (mut app, _tx) = App::test_with_sender();
    let ack = std::env::temp_dir().join("hrw-verdict-disabled-ack.json");
    let _ = std::fs::remove_file(&ack);

    std::fs::write(
            &ack,
            r#"{"version":2,"breakpointPresent":false,"reason":"a breakpoint exists at live_trace.rs:173 but is DISABLED"}"#,
        )
        .unwrap();
    app.pending_live_debug = Some((std::time::Instant::now(), PendingLiveDebug::Reduction));

    let action = app.live_debug_poll(&ctx, PendingLiveDebug::Reduction, &ack);

    assert!(
        matches!(action, LiveDebugAction::SpawnLive),
        "it must still spawn \u{2014} refusing to run would be a worse answer \
             than running unstepped"
    );
    assert!(
        !app.live_breakpoint_armed,
        "the bridge said nothing is armed, so HRW must not claim otherwise"
    );
    let notice = app
        .notice
        .as_deref()
        .expect("a run that will not stop must say so before it starts");
    assert!(
        notice.contains("DISABLED"),
        "the bridge's reason must reach the user, not be replaced by a generic \
             failure \u{2014} the cause is the whole value here, got: {notice}"
    );

    let _ = std::fs::remove_file(&ack);
}

/// **A reply in the pre-`#75` format cannot say what it armed, and that is
/// its own answer** — the `Unreportable` verdict of the four in
/// [`an_armed_verdict_starts_the_run_and_stays_quiet`].
///
/// Not hypothetical: on 2026-08-08 this machine ran a build twelve days
/// behind its source, because `git pull` runs no `tsc`. Reading it as armed
/// would reinstate #71's fiction; reading it as a plain failure would blame
/// the wrong thing. It gets its own message, and that message names the fix.
#[test]
fn a_stale_bridge_reply_claims_nothing_and_names_its_fix() {
    let ctx = egui::Context::default();
    let (mut app, _tx) = App::test_with_sender();
    let ack = std::env::temp_dir().join("hrw-verdict-stale-ack.json");
    let _ = std::fs::remove_file(&ack);

    std::fs::write(&ack, r#"{"acked":true}"#).unwrap();
    app.pending_live_debug = Some((std::time::Instant::now(), PendingLiveDebug::Reduction));

    let action = app.live_debug_poll(&ctx, PendingLiveDebug::Reduction, &ack);

    assert!(
        matches!(action, LiveDebugAction::SpawnLive),
        "a reply is a reply \u{2014} the wait ends here, however uninformative it was"
    );
    assert!(
        !app.live_breakpoint_armed,
        "an ack that cannot say what it armed is not evidence of anything"
    );
    let notice = app
        .notice
        .as_deref()
        .expect("a stale bridge must announce itself rather than fail silently");
    assert!(
        notice.contains("npm run build"),
        "the notice must name the fix \u{2014} the whole point is that a stale \
             extension is otherwise invisible, got: {notice}"
    );

    let _ = std::fs::remove_file(&ack);
}

/// **Nothing acked and the wait expired: the run starts, and HRW claims
/// nothing** — the `Pending` verdict of the four in
/// [`an_armed_verdict_starts_the_run_and_stays_quiet`], and the path
/// `docs/ideas.md` #71 is named after.
///
/// The timeout still spawns, which is deliberate: a wedged extension must
/// not deadlock the UI. What it may not do is *pass for success* — with no
/// extension installed, the old `if acked || timed_out` branch recorded a
/// breakpoint that did not exist and said nothing about it.
#[test]
fn a_timed_out_arm_claims_nothing_and_says_so() {
    let ctx = egui::Context::default();
    let (mut app, _tx) = App::test_with_sender();
    // Nothing ever writes this file: the absence of a reply IS the input.
    let ack = std::env::temp_dir().join("hrw-verdict-timeout-ack.json");
    let _ = std::fs::remove_file(&ack);

    // Backdate the arm past the timeout. `expect` rather than a fallback:
    // silently landing on "not yet timed out" would make this test vacuous,
    // which is the failure mode the must-fire rule exists to refuse.
    let long_ago = std::time::Instant::now()
        .checked_sub(LIVE_DEBUG_ACK_TIMEOUT * 2)
        .expect("the process must have been running longer than the ack timeout");
    app.pending_live_debug = Some((long_ago, PendingLiveDebug::Reduction));

    let action = app.live_debug_poll(&ctx, PendingLiveDebug::Reduction, &ack);

    assert!(
        matches!(action, LiveDebugAction::SpawnLive),
        "the timeout must still spawn \u{2014} a missing extension may not deadlock the UI"
    );
    assert!(
        !app.live_breakpoint_armed,
        "nothing acked, so nothing is armed \u{2014} this is the claim #71 was about"
    );
    let notice = app
        .notice
        .as_deref()
        .expect("a timed-out handshake must say so; silence is the defect");
    assert!(
        notice.contains("Bridge"),
        "the notice must name the bridge as the suspect, got: {notice}"
    );
    assert!(
        notice.contains("vscode-extension"),
        "the notice must point at the fix, got: {notice}"
    );
}

/// `src.V` is not declared in the specimen — it is a parameter of `src`'s
/// type. Resolving the component gives the class that declares it, which
/// turns "not declared in this specimen" into a navigable answer.
#[test]
fn declaring_classes_resolves_a_component_type() {
    use crate::equation_sheet::{ClassifiedVariable, EquationSheet};

    let stages = StageBundle {
        resolve: Stage::ok(serde_json::json!({
            "components": {
                "src": { "type_def_id": 6005 },
                "plain": { "type_def_id": 4047 },
            }
        })),
        ..Default::default()
    };
    let mut def_index = BTreeMap::new();
    def_index.insert(
        6005u64,
        DefInfo {
            name: "Modelica.Electrical.Analog.Sources.ConstantVoltage".to_owned(),
            kind: DefKind::Class,
            class_type: Some("model".to_owned()),
            file_name: None,
            line: None,
        },
    );
    // A non-class definition must not be offered as a declaring class.
    def_index.insert(
        4047u64,
        DefInfo {
            name: "Modelica.Units.SI.Voltage".to_owned(),
            kind: DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        },
    );

    let var = |name: &str| ClassifiedVariable {
        name: name.to_owned(),
        kind: "parameter",
        unit: None,
        description: None,
        start: None,
        derivative_evidence: None,
    };
    let sheet = EquationSheet {
        variables: vec![var("src.V"), var("plain.x"), var("h"), var("nosuch.y")],
        ..Default::default()
    };

    let map = App::build_declaring_classes(&stages, &def_index, Some(&sheet));
    assert_eq!(
        map.get("src.V").map(String::as_str),
        Some("Modelica.Electrical.Analog.Sources.ConstantVoltage")
    );
    assert!(
        !map.contains_key("plain.x"),
        "a non-class DefId is not a declaring class"
    );
    assert!(
        !map.contains_key("h"),
        "an unqualified name has no component to resolve"
    );
    assert!(
        !map.contains_key("nosuch.y"),
        "unknown components resolve to nothing"
    );
}

/// Tracking is a toggle from every view, and derivative mentions resolve to
/// the base variable before being stored.
/// The point must be clearable, so "explain only what I am following" is
/// askable.
///
/// Found by Doug in testing: the Following row had a clear button and the
/// Pointing at row did not, so a point could only ever be *replaced*. The
/// sole escape was reloading the specimen, which recompiles and discards
/// everything.
///
/// The emitted `kind` is the load-bearing half. Clearing must NOT fall back
/// to `Focus::Stage`: "pointing at the Typecheck stage as a whole" is a
/// claim the user makes by clicking a tab, and attributing it to someone who
/// pointed at nothing is the confident lie this design exists to prevent.
#[test]
fn clearing_the_point_emits_nothing_not_the_current_stage() {
    let empty = BTreeMap::new();
    let stages: [(&str, Option<&Value>); 0] = [];
    let ask = Ask {
        seq: 3,
        request: bridge::AskRequest::Explain,
        specimen: None,
        model: Some("MotorWithBrake"),
        // A stage is still reported — it is where the user is looking —
        // but it must not become the subject.
        stage: Some(StageKind::Typecheck),
        libraries: vec![],
        def_index: &empty,
        parse_value: None,
        resolve_value: None,
        focus: Focus::Nothing,
        tracking: Some(bridge::Tracking {
            seq: 4,
            name: "emf.w",
            declared_line: None,
            declaring_class: None,
            stage_values: &stages,
        }),
        view: bridge::View {
            ui_mode: "Specimen",
            stage_view: None,
            specimen_detail: None,
            viewing_log: false,
            animation: None,
        },
        failure: None,
    };
    let doc = bridge::build_for_test(&ask);

    assert_eq!(
        doc["kind"],
        serde_json::json!("none"),
        "absence must be stated, not implied"
    );
    assert!(doc.get("node").is_none(), "there is no node to describe");
    assert!(doc.get("cross_stage").is_none());
    assert_eq!(doc["tracking"]["identifier"], serde_json::json!("emf.w"));
    assert!(
        doc["instructions"]
            .as_str()
            .is_some_and(|i| i.contains("kind: \"none\"")),
        "the file must explain what `none` means to whoever reads it",
    );
}

#[test]
fn set_tracked_identifier_toggles_and_strips_der() {
    let (mut app, _tx) = App::test_with_sender();

    app.set_tracked_identifier("h".to_owned());
    assert_eq!(app.tracked_identifier.as_deref(), Some("h"));

    // Clicking the same name again clears it.
    app.set_tracked_identifier("h".to_owned());
    assert_eq!(app.tracked_identifier, None);

    // A derivative mention tracks the variable it differentiates...
    app.set_tracked_identifier("der(h)".to_owned());
    assert_eq!(app.tracked_identifier.as_deref(), Some("h"));
    // ...and so clicking `h` elsewhere is recognised as the same thing.
    app.set_tracked_identifier("h".to_owned());
    assert_eq!(app.tracked_identifier, None, "der(h) and h are one target");
}

/// The source view scrolls on *change* only. Without this the view would be
/// re-centred every frame while an identifier stayed tracked, pinning it and
/// making the scrollbar unusable.
#[test]
fn source_scroll_is_armed_only_when_the_tracked_identifier_changes() {
    let (mut app, _tx) = App::test_with_sender();
    assert_eq!(app.source.scrolled_for, None);

    app.set_tracked_identifier("h".to_owned());
    assert_ne!(
        app.tracked_identifier, app.source.scrolled_for,
        "a newly tracked identifier must still be pending a scroll"
    );

    // Simulate the source view having scrolled to it.
    app.source.scrolled_for = app.tracked_identifier.clone();
    assert_eq!(
        app.tracked_identifier, app.source.scrolled_for,
        "once scrolled, no further scroll is armed for the same identifier"
    );
}

#[test]
fn read_purpose_extracts_hint() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = dir.join("specimens/BouncingBall.mo");
    let purpose = read_purpose(&path);
    assert!(
        purpose.is_some(),
        "BouncingBall should have a // purpose: comment"
    );
    let text = purpose.unwrap();
    assert!(!text.is_empty());
    assert!(
        text.to_lowercase().contains("event"),
        "purpose should mention events: {text}"
    );
}

#[test]
fn read_purpose_returns_none_for_missing_file() {
    let purpose = read_purpose(Path::new("/nonexistent/specimen.mo"));
    assert!(purpose.is_none());
}

#[test]
fn every_specimen_has_a_purpose_comment() {
    let dir = std::path::PathBuf::from(format!("{}/specimens", env!("CARGO_MANIFEST_DIR")));
    let entries: Vec<_> = std::fs::read_dir(&dir)
        .expect("read specimens dir")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "mo"))
        .collect();
    assert!(!entries.is_empty(), "no .mo files found in specimens/");
    for entry in entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_str().unwrap();
        assert!(
            read_purpose(&path).is_some(),
            "specimen {name} is missing a // purpose: comment"
        );
    }
}

fn make_app_with_stages(ok_through: StageKind) -> App {
    let ok_stage = Stage::ok(serde_json::json!({}));
    let empty = Stage::default();
    let stages_in_order = [
        StageKind::Parse,
        StageKind::Resolve,
        StageKind::Instantiate,
        StageKind::Typecheck,
        StageKind::Flatten,
        StageKind::Structural,
        StageKind::IndexReduction,
        StageKind::Initialization,
        StageKind::Events,
        StageKind::SolveLowering,
    ];
    let cutoff = stages_in_order
        .iter()
        .position(|&s| s == ok_through)
        .unwrap_or(0);
    let mut bundle = StageBundle::default();
    for (i, &kind) in stages_in_order.iter().enumerate() {
        let stage = if i <= cutoff {
            ok_stage.clone()
        } else {
            empty.clone()
        };
        match kind {
            StageKind::Parse => bundle.parse = stage,
            StageKind::Resolve => bundle.resolve = stage,
            StageKind::Instantiate => bundle.instantiate = stage,
            StageKind::Typecheck => bundle.typecheck = stage,
            StageKind::Flatten => bundle.flatten = stage,
            StageKind::Dae => bundle.dae = stage,
            StageKind::Structural => bundle.structural = stage,
            StageKind::IndexReduction => bundle.index_reduction = stage,
            StageKind::Initialization => bundle.initialization = stage,
            StageKind::Events => bundle.events = stage,
            StageKind::SolveLowering => bundle.solve_lowering = stage,
            StageKind::Simulation => {}
        }
    }
    App {
        stages: bundle,
        ..App::test_default()
    }
}

#[test]
fn last_successful_stage_selects_furthest_ok() {
    let app = make_app_with_stages(StageKind::Flatten);
    assert_eq!(app.last_successful_stage(), StageKind::Flatten);
}

#[test]
fn last_successful_stage_falls_back_to_parse() {
    let app = App {
        stages: StageBundle::default(),
        ..App::test_default()
    };
    assert_eq!(app.last_successful_stage(), StageKind::Parse);
}

#[test]
fn last_successful_stage_skips_errored() {
    let mut app = make_app_with_stages(StageKind::Structural);
    app.stages.structural = Stage::recovered(serde_json::json!({}), "singular");
    assert_eq!(app.last_successful_stage(), StageKind::Flatten);
}

#[test]
fn previous_stage_value_parse_is_none() {
    let mut app = make_app_with_stages(StageKind::SolveLowering);
    app.stage = StageKind::Parse;
    assert!(app.previous_stage_value().is_none());
}

#[test]
fn previous_stage_value_instantiate_returns_resolve() {
    let mut app = make_app_with_stages(StageKind::SolveLowering);
    app.stage = StageKind::Instantiate;
    assert!(app.previous_stage_value().is_some());
}

#[test]
fn stage_name_exhaustive() {
    let all = [
        StageKind::Parse,
        StageKind::Resolve,
        StageKind::Instantiate,
        StageKind::Typecheck,
        StageKind::Flatten,
        StageKind::Structural,
        StageKind::IndexReduction,
        StageKind::Initialization,
        StageKind::Events,
        StageKind::SolveLowering,
        StageKind::Simulation,
    ];
    for kind in all {
        let name = kind.name();
        assert!(!name.is_empty(), "{kind:?} has an empty name");
    }
}

#[test]
fn parse_library_paths_splits_lines() {
    let mut app = App::test_default();
    app.libraries_text = "/path/one\n/path/two\n".to_owned();
    let paths = app.parse_library_paths();
    assert_eq!(
        paths,
        vec![PathBuf::from("/path/one"), PathBuf::from("/path/two")]
    );
}

#[test]
fn parse_library_paths_trims_whitespace() {
    let mut app = App::test_default();
    app.libraries_text = "  /trimmed  \n".to_owned();
    let paths = app.parse_library_paths();
    assert_eq!(paths, vec![PathBuf::from("/trimmed")]);
}

#[test]
fn parse_library_paths_skips_blank_lines() {
    let mut app = App::test_default();
    app.libraries_text = "/first\n\n  \n/last\n".to_owned();
    let paths = app.parse_library_paths();
    assert_eq!(paths, vec![PathBuf::from("/first"), PathBuf::from("/last")]);
}

#[test]
fn parse_library_paths_empty_text() {
    let app = App::test_default();
    assert!(app.parse_library_paths().is_empty());
}

/// A successful point says nothing in the status bar.
///
/// The Context Bar names the point persistently; a second, transient
/// description of the same thing could only go stale and disagree with it.
/// Two places claiming to describe what Claude has is the failure mode this
/// design keeps running into, and the weaker one is the one to drop.
#[test]
fn a_successful_point_is_silent() {
    let msg = status_line(
        1,
        "equations.3.lhs",
        "explain",
        Ok(PathBuf::from("/tmp/focus.json")),
    );
    assert_eq!(
        msg, None,
        "the Context Bar states this; the status bar must not repeat it"
    );
}

/// The debugger request still speaks, because it asks for something next.
/// An instruction is not a confirmation.
#[test]
fn a_debug_point_still_tells_the_user_what_to_do() {
    let msg = status_line(
        2,
        "def_id",
        "debug-where-set",
        Ok(PathBuf::from("/tmp/f.json")),
    )
    .expect("debug requests carry an instruction");
    assert!(
        msg.contains("debugger"),
        "debug request should mention debugger: {msg}"
    );
    assert!(
        msg.contains("context #2"),
        "should carry the shared counter: {msg}"
    );
}

/// A failure must still be stated. This is the one case the Context Bar
/// cannot cover alone — it renders the point either way, so silence would
/// leave it describing context that was never written.
#[test]
fn a_failed_emission_is_never_silent() {
    let err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
    let msg = status_line(1, "x", "explain", Err(err)).expect("failures are always reported");
    assert!(
        msg.contains("not emitted"),
        "should say it was not emitted: {msg}"
    );
    assert!(msg.contains("denied"), "should carry the cause: {msg}");
}

/// Switching stages drops **every stage-keyed** view, including ones added later.
///
/// **This test used to re-implement the invalidation inline**, clearing five
/// fields by hand and then asserting they were clear — so it verified its own
/// copy of the logic, not the app's. The real block could have been deleted
/// and this would still have passed. Calling `reset_for` is the point of
/// extracting it.
///
/// The assertion is `built_for` plus a *whole-struct* check, so a view added
/// tomorrow is covered without touching this test.
///
/// **The word "every" was load-bearing and wrong until 2026-08-20.** Three replays
/// on this list were not stage-keyed at all, and dropping them here was the defect
/// [`the_compile_replays_survive_a_stage_switch`] now guards against. The two tests
/// are a pair: this one says what a stage switch *must* clear, that one says what it
/// **must not**, and a field moved between the two families fails one of them.
#[test]
fn report_cache_invalidated_on_stage_switch() {
    let mut app = App::test_default();
    app.stage_views.built_for = Some(StageKind::Structural);
    app.stage_views.spy_plot = Some(None);
    app.stage_views.incidence = Some(None);
    app.stage_views.reduction = Some(None);
    app.stage_views.matching_anim = Some(None);
    app.stage_views.tarjan_anim = Some(None);

    let reset = app.stage_views.reset_for(StageKind::IndexReduction);

    assert!(reset, "a different stage must report that it reset");
    assert_eq!(app.stage_views.built_for, Some(StageKind::IndexReduction));
    // Every view is back to `None`. Listed rather than compared as a whole
    // struct, because the view types do not implement `PartialEq` and
    // deriving it on production types to satisfy a test would be backwards.
    //
    // **This list going stale is survivable, unlike the two it replaced**:
    // `reset_for` assigns a whole `Self`, so a view added tomorrow is cleared
    // whether or not anyone remembers to add a line here. Missing it costs
    // coverage, not correctness.
    assert!(
        app.stage_views.spy_plot.is_none()
            && app.stage_views.incidence.is_none()
            && app.stage_views.reduction.is_none()
            && app.stage_views.matching_anim.is_none()
            && app.stage_views.tarjan_anim.is_none()
            && app.stage_views.tearing_anim.is_none()
            && app.stage_views.alias_anim.is_none()
            && app.stage_views.before_incidence.is_none(),
        "a stage switch must leave no view built for the previous stage",
    );
}

/// A stage switch must **not** touch the replays the compile produced.
///
/// The mirror of [`report_cache_invalidated_on_stage_switch`], and the guard for the
/// defect that split `CompileViewCaches` out of `StageViewCaches` on 2026-08-20.
///
/// **What it was like before.** `reduction_anim`, `connection_anim` and
/// `ic_plan_anim` sat in `StageViewCaches`, so the whole-struct assignment above
/// dropped them — and since `reset_for` is called only from the report sub-view row,
/// the rule in force was *"a replay restarts if you happened to pass through
/// Structural or Index Reduction in between."* Paused on frame 12 of the
/// index-reduction replay, clicking Structural to compare and clicking back put you
/// on frame 0, because [`crate::playback::Playback::recorded`] starts there.
///
/// **It goes through the paint path, and that is the whole design of it.** The first
/// draft set the four cache fields directly, called `reset_for`, and asserted they
/// survived — which cannot fail, because they are in a different struct now. It
/// asserted the refactor rather than the behaviour. This one drives the real
/// `frame_ui`, so the stage switch reaches `report_sub_view_row_ui` the way a click
/// does, and it fails at **runtime** rather than at compile time if a replay moves
/// back into `StageViewCaches` — the loud failure, since it names the cursor.
///
/// **Initialization ▸ IC Plan is the cheapest of the four to stage**, because
/// `IcPlanAnimation::from_report` takes plain JSON while the other three want
/// captured compiler frames. Three blocks, so a cursor at 2 is distinguishable from
/// a reset to 0; the structural report exists only so the sub-view row draws at all
/// (it needs `current_stage().value`), which is what calls `reset_for`.
#[test]
fn a_replay_keeps_its_place_across_a_stage_switch() {
    use crate::ui_tests::{AdHocLab, harness};
    use crate::worker::Stage;

    // Auto-selecting an ad hoc lab resets the stage side. See the note on
    // `AdHocLab` — its presence is environment, not code.
    let _lab_state = AdHocLab::absent();

    let mut app = App::test_default();
    app.stages.initialization = Stage {
        value: Some(serde_json::json!({
            "blocks": [
                { "kind": "scalar_direct", "var": "a" },
                { "kind": "scalar_direct", "var": "b" },
                { "kind": "scalar_direct", "var": "c" },
            ]
        })),
        ..Stage::default()
    };
    // Any non-empty report: the row only asks whether the stage has a value.
    app.stages.structural = Stage {
        value: Some(serde_json::json!({ "blocks": [] })),
        ..Stage::default()
    };
    app.stage = StageKind::Initialization;
    app.viewport.init = InitView::IcPlan;
    // **A specimen must be selected or `central_panel_ui` returns before the stage
    // view** — and `nav` must stay *empty*, which reads backwards until you know
    // what it is: `nav` is the go-to-definition stack, so a non-empty one means the
    // reader has drilled into a class and the pane shows that class instead of the
    // stage. Both were found by probe rather than by reading, and the failure in
    // each case was the same one — "the replay is missing", when the truth was "the
    // pane never ran".
    app.selected = Some(std::path::PathBuf::from("specimens/RcCircuit.mo"));

    let mut h = harness(app);
    h.run_steps(2);

    // Part-way through the walk, which is the state worth preserving.
    assert!(
        h.state_mut()
            .on_screen_animation_mut()
            .expect("precondition: the IC plan replay is on screen")
            .seek(2),
        "precondition: a three-block plan has a frame 2",
    );

    // Off to a report stage and back — the exact round trip that used to reset it.
    h.state_mut().stage = StageKind::Structural;
    h.run_steps(2);
    h.state_mut().stage = StageKind::Initialization;
    h.run_steps(2);

    assert_eq!(
        h.state()
            .on_screen_animation()
            .expect("the replay must still be on screen")
            .position(),
        // 4, not 3: a three-block plan is four frames, because frame 0 is the
        // opening state where nothing has been solved (2026-08-23). The cursor
        // is unchanged — this test is about the cursor surviving the round trip.
        (2, 4),
        "a replay built from the compile outlives a visit to another stage — \
             nothing it was built from changed. Before 2026-08-20 this returned cursor 0: \
             the report sub-view row cleared it on the way through, and a reader who had \
             walked to block 2 was silently put back at the start",
    );
}

/// A sub-view left behind on a report stage must not take over another stage's pane.
///
/// **The third instance of the stranded-sub-view class**, after the alias view
/// (2026-08-19) and the redirect list it exposed — and the worst-looking of the
/// three. `viewport.structural` deliberately survives a stage change, because it is
/// a camera; `clamp_structural_sub_view` returns early on every non-report stage, by
/// design. So the only thing standing between a stranded `Animate` and another
/// stage's pane is the `report_ready` guard on its dispatch branch, and that branch
/// was **the one arm of eight without it**.
///
/// **What it looked like**: on Index Reduction choose **Animate ▶**, then click
/// **Events**, **Initialization** or **Flatten**. The Events tab is highlighted, the
/// sub-view row offers Tree / pre() lowering — and the pane below draws the
/// *index-reduction* replay, because that arm sits above all three of theirs in the
/// chain. Not "absence filled" this time but **presence substituted**: a correct
/// animation of the wrong phase, under another phase's tab.
///
/// Found while splitting `CompileViewCaches` out, by a probe that was asking a
/// different question — the IC plan cache was `None` when it should have been built.
#[test]
fn a_stranded_structural_sub_view_does_not_take_over_another_stage() {
    use crate::ui_tests::{AdHocLab, harness};
    use crate::worker::Stage;

    let _lab_state = AdHocLab::absent();

    let mut app = App::test_default();
    app.stages.initialization = Stage {
        value: Some(serde_json::json!({
            "blocks": [{ "kind": "scalar_direct", "var": "a" }]
        })),
        ..Stage::default()
    };
    app.selected = Some(std::path::PathBuf::from("specimens/RcCircuit.mo"));
    app.stage = StageKind::Initialization;
    app.viewport.init = InitView::IcPlan;
    // The reader's last report-stage sub-view, still set — which is correct, and is
    // exactly the state the dispatch has to ignore.
    app.viewport.structural = StructuralView::Animate;

    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.state().compile_views.ic_plan_anim.is_some(),
        "the stage the reader is ON decides the pane: Initialization ▸ IC Plan must \
             render even when a report stage's sub-view is still selected behind it",
    );
    assert!(
        h.state().compile_views.reduction_anim.is_none(),
        "and the index-reduction replay must not have been built at all — building \
             it is the symptom that it was drawn under the Initialization tab",
    );
}

#[test]
fn report_cache_preserved_for_same_stage() {
    let mut app = App::test_default();
    app.stage_views.built_for = Some(StageKind::Structural);
    app.stage_views.spy_plot = Some(None);
    app.stage = StageKind::Structural;

    // Same stage — nothing should be dropped, and `reset_for` says so.
    let reset = app.stage_views.reset_for(StageKind::Structural);

    assert!(!reset, "the same stage must not report a reset");
    assert!(
        app.stage_views.spy_plot.is_some(),
        "rebuilding a view that was already correct wastes the work the cache exists \
             to avoid — and on a large model that work is seconds, not milliseconds",
    );
}

/// The model list renders **without an `App`**, and reports a click instead
/// of acting on it.
///
/// This is the payoff of the whole extraction, and the reason the signature
/// was narrowed rather than left as `&mut self`. A pane that takes `&mut App`
/// can be *called* in a test but not *isolated* in one: every assertion is
/// entangled with 85 other fields, and a failure never tells you which.
///
/// **`ModelListState` is the entire input here.** No worker, no channels, no
/// compile — the harness drives one struct.
#[test]
fn the_model_list_renders_and_reports_without_an_app() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let state = ModelListState {
        dir: String::new(),
        files: vec![
            PathBuf::from("RcCircuit.mo"),
            PathBuf::from("MotorWithBrake.mo"),
        ],
        // Park the scratch poll, or frame one rescans an empty `dir` and
        // wipes the list — finding R2, the trap that made two UI tests pass
        // while checking nothing.
        polled_at: Some(std::time::Instant::now()),
        filter: "rc".to_owned(),
        ..ModelListState::default()
    };

    // The closure outlives this scope, so the observed navigation goes into a
    // shared cell rather than a captured local.
    let nav_seen = std::rc::Rc::new(std::cell::RefCell::new(None::<String>));
    let sink = std::rc::Rc::clone(&nav_seen);
    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(1600.0, 1200.0))
        .build_ui_state(
            |ui, s: &mut ModelListState| {
                let out = s.ui(ui, None, false, false);
                if let Some(ModelListNav::Select(p)) = out.nav {
                    *sink.borrow_mut() = Some(p.display().to_string());
                }
            },
            state,
        );
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("RcCircuit").is_some(),
        "the filtered specimen must render with no `App` in sight",
    );
    assert!(
        h.query_by_label_contains("MotorWithBrake").is_none(),
        "and the filter must still exclude the other one — a pane that renders \
             everything regardless would pass the assertion above too",
    );

    h.get_all_by_label_contains("RcCircuit")
        .next()
        .expect("the row")
        .click();
    h.run_steps(2);

    assert_eq!(
        nav_seen.borrow().as_deref(),
        Some("RcCircuit.mo"),
        "a click must be REPORTED as a navigation, not applied — the list does not \
             own the stages, the log or the context bar that opening a specimen resets",
    );
}

/// Every animation pane **says when it has nothing to show**.
///
/// Finding C6: six animation panes, testable all along and never tested. The
/// earlier reading assumed they were out of reach because they sit near
/// `Painter` calls — checked, and wrong: their controls, step labels and
/// state text are ordinary widgets (H7).
///
/// These are the **most** empty-prone panes in HRW. Most models have no
/// algebraic loop to tear, no alias eliminations, no `pre()` lowering. A
/// reader meets the empty state far more often than the animation, so a pane
/// that rendered blank would train them to read "nothing here" as normal —
/// and the one time it meant "this failed" would look identical.
///
/// Asserts the empty state only. The populated case needs a real report and
/// belongs with the `slow-tests`.
#[test]
fn every_animation_pane_reports_having_nothing_to_show() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    // `App::test_default` has no compiled stages, so every animation's source
    // report is absent — which is the state under test.
    type Pane = fn(&mut App, &mut egui::Ui);
    let panes: [(&str, Pane, &str); 5] = [
        // **Was "no DAE available for tearing", which was usually untrue.** The
        // DAE is normally present; what is absent is the *tearing*, because the
        // compiler stopped at matching. `structural_unavailable` says which
        // (2026-08-04) — a pane that names the wrong cause is worse than one that
        // names none, because it sends the reader looking in the wrong place.
        ("tearing", |a, ui| a.tearing_anim_ui(ui), "No tearing"),
        (
            "alias",
            |a, ui| a.alias_anim_ui(ui),
            "no alias eliminations in this report",
        ),
        (
            "ic_plan",
            |a, ui| a.ic_plan_anim_ui(ui),
            "no initial-condition plan in this report",
        ),
        (
            "connection",
            |a, ui| a.connection_anim_ui(ui),
            "no connections in this model",
        ),
        (
            "pre_lowering",
            |a, ui| a.pre_lowering_anim_ui(ui),
            "no pre() lowering in this model",
        ),
    ];

    for (name, render, expected) in panes {
        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 700.0))
            .build_ui_state(
                move |ui, app: &mut App| render(app, ui),
                App::test_default(),
            );
        h.run_steps(2);

        assert!(
            h.query_by_label_contains(expected).is_some(),
            "the {name} animation renders blank with no report. A blank pane and a                  broken one are the same picture, and this is a pane readers meet empty                  most of the time",
        );
    }
}

/// **A pane whose animation EXISTS and holds nothing says so too.**
///
/// The sibling of [`every_animation_pane_reports_having_nothing_to_show`], which covers
/// the other state — no report at all, so no animation is built and `app.rs` speaks.
/// This one covers the state where the report is there, the animation was built, and it
/// is empty, so the view speaks for itself.
///
/// # Why the two states need different messages, and therefore different tests
///
/// They mean different things, and the wording is the only thing carrying the
/// difference. *"(no alias eliminations in this **report**)"* means HRW found nothing
/// to read. *"No alias eliminations in this **model**."* means the compiler reported
/// an empty list — a fact about the model. The first is about HRW's reach; the second
/// is a claim about Rumoca's output.
///
/// **That distinction is the charter's absence rule in miniature**, and until
/// 2026-08-24 only the first of each pair was tested. `BouncingBall` and
/// `SingleInertia` both reach the second one for real: their initialization stage
/// carries `blocks: []`.
///
/// # What this does not check
///
/// Whether the wording *should* distinguish them more clearly than *report* versus
/// *model* does. That is a pane claim and Doug's to rule on; it is recorded in
/// `docs/ui-findings.md` rather than changed here.
///
/// # The ic_plan fixture gained a verdict on 2026-08-25 (finding C19)
///
/// It used to be a bare `blocks: []`, which reaches the pane's **no-verdict** arm —
/// a state neither real specimen produces. `BouncingBall` and `SingleInertia` both
/// carry `determinacy.verdict`, so the fixture now does too, and the bare form is a
/// second case rather than the only one. Both arms that *speak* are covered here;
/// the arm that deliberately stays silent — a verdict HRW cannot paraphrase — is
/// pinned by `ic_plan_anim::tests::the_empty_plan_gloss_is_shown_only_for_the_verdict_it_paraphrases`,
/// because it asserts an absence and this harness queries for presence.
#[test]
fn an_animation_built_from_an_empty_report_says_the_model_has_none() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    type Pane = fn(&mut App, &mut egui::Ui);
    // (name, the stage to sit on, its report, the pane, what it must say)
    let cases: [(&str, StageKind, serde_json::Value, Pane, &str); 3] = [
        (
            "alias",
            StageKind::IndexReduction,
            serde_json::json!({"reduction": {"eliminations": []}}),
            |a, ui| a.alias_anim_ui(ui),
            "No alias eliminations in this model",
        ),
        (
            // What `BouncingBall` and `SingleInertia` actually produce.
            "ic_plan (start-attribute verdict)",
            StageKind::Initialization,
            serde_json::json!({
                "blocks": [],
                "determinacy": {
                    "verdict": "well-posed (remaining states initialize from their start attributes)"
                },
            }),
            |a, ui| a.ic_plan_anim_ui(ui),
            "Nothing has to be solved at t=0",
        ),
        (
            // No determinacy at all: no header either, so the body must say so.
            "ic_plan (no verdict)",
            StageKind::Initialization,
            serde_json::json!({"blocks": []}),
            |a, ui| a.ic_plan_anim_ui(ui),
            "no determinacy verdict",
        ),
    ];

    for (name, stage, report, render, expected) in cases {
        let mut app = App::test_default();
        app.selected = Some(PathBuf::from("/test/specimen.mo"));
        app.stage = stage;
        *app.stages.get_mut(stage) = Stage::ok(report);

        let mut h = Harness::builder()
            .with_size(egui::Vec2::new(900.0, 700.0))
            .build_ui_state(move |ui, app: &mut App| render(app, ui), app);
        h.run_steps(2);

        assert!(
            h.query_by_label_contains(expected).is_some(),
            "the {name} pane built an animation from an empty list and then said \
             nothing about it. An empty report and an unreadable one are different \
             facts, and the message is the only thing that carries the difference",
        );
    }
}

/// The source map **says when the model has no source mapping**.
///
/// Finding C12, and it is reachable by a route worth knowing: the SourceMap
/// sub-view is only *offered* when the sheet has source lines, but
/// `Viewport::flatten` survives a specimen change. Sit on SourceMap for a
/// model that has one, load a model that does not, and this is what you see.
///
/// **Deferred as needing a compile, and that was wrong.** `EquationSheet`
/// derives `Default` and its fields are public, so the state is one struct
/// literal away — the deferral was an assumption about the type, not a fact
/// about it. Checking cost less than the deferral did.
///
/// Its sibling `"(no equation sheet)"` stays unreachable (C1): the only call
/// site is gated on `flatten_ready`, which *is* `cached_equation_sheet
/// .is_some()`.
#[test]
fn the_source_map_reports_a_model_with_no_mapping() {
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;

    let mut app = App::test_default();
    // A sheet that exists but carries no source lines: the exact state a
    // persisting sub-view lands the reader in.
    app.cached_equation_sheet = Some(equation_sheet::EquationSheet::default());
    app.viewport.flatten = FlattenView::SourceMap;

    let mut h = Harness::builder()
        .with_size(egui::Vec2::new(900.0, 700.0))
        .build_ui_state(|ui, a: &mut App| a.source_map_ui(ui), app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("no source mapping available")
            .is_some(),
        "a sheet with no source lines must say so. Rendering blank here is worse than              elsewhere: the reader arrived on this sub-view by inertia, not by choosing              it, so a blank pane looks like the tab they picked is broken",
    );
}

#[test]
fn drain_worker_libraries_ok_updates_status() {
    let (mut app, tx) = App::test_with_sender();
    app.libraries_busy = true;
    tx.send(FromWorker::Libraries(Ok(3))).unwrap();
    app.drain_worker();
    assert!(!app.libraries_busy);
    assert!(app.library_status.contains("3"));
}

#[test]
fn drain_worker_libraries_err_updates_status() {
    let (mut app, tx) = App::test_with_sender();
    app.libraries_busy = true;
    tx.send(FromWorker::Libraries(Err("boom".into()))).unwrap();
    app.drain_worker();
    assert!(!app.libraries_busy);
    assert!(app.library_status.contains("boom"));
}

#[test]
fn drain_worker_compile_progress_updates_stages_for_current_specimen() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({"parsed": true})),
        ..Default::default()
    };
    tx.send(FromWorker::CompileProgress { path, stages })
        .unwrap();
    app.drain_worker();
    assert!(app.stages.parse.value.is_some());
}

#[test]
fn drain_worker_compile_progress_ignored_for_stale_specimen() {
    let (mut app, tx) = App::test_with_sender();
    app.selected = Some(PathBuf::from("/test/current.mo"));
    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({"parsed": true})),
        ..Default::default()
    };
    tx.send(FromWorker::CompileProgress {
        path: PathBuf::from("/test/stale.mo"),
        stages,
    })
    .unwrap();
    app.drain_worker();
    assert!(app.stages.parse.value.is_none());
}

/// **A partial result drops the views built from the previous compile's reports.**
///
/// Found by the 2026-08-23 column read of `drain_worker`, ruled on 2026-08-24. The
/// arm replaced `self.stages` with new partial reports and invalidated nothing, and
/// `StageViewCaches::reset_for` keys on the *stage* — which does not change
/// mid-compile — so a cached view survived until `Compiled` landed.
///
/// **The pane then drew the previous compile's matrix over the current compile's
/// report.** Real data, correctly computed, attributed to the wrong run: the fiction
/// class `CLAUDE.md` names when it requires everything on screen to be traceable to
/// what Rumoca did on *this* run. And the tab colours advanced with the pipeline while
/// the pane held still, so two things on screen described the same instant differently
/// — during Recompile, whose entire purpose is to show what an edit changed.
///
/// # Why `compile_views` is deliberately NOT dropped here
///
/// The asymmetry is the point, and it is by source rather than by convenience.
/// `stage_views` is built from stage reports, which a progress message delivers.
/// `compile_views` holds replays built from frames, which arrive **only** with
/// `Compiled` — dropping them here would blank every animation for the whole compile
/// with nothing to rebuild from. The second assertion pins that, because a fix that
/// invalidated both would pass a test checking only the first.
#[test]
fn drain_worker_compile_progress_drops_views_built_from_the_previous_report() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());

    // A view cached from the previous compile, keyed to the stage on screen.
    app.stage = StageKind::Structural;
    app.stage_views.built_for = Some(StageKind::Structural);
    app.stage_views.incidence = Some(None);
    // And a replay from that compile's frames, which this message carries none of.
    app.compile_views.ic_plan_anim = Some(None);

    tx.send(FromWorker::CompileProgress {
        path,
        stages: StageBundle {
            structural: Stage::ok(serde_json::json!({"incidence": {"n_eq": 1, "n_var": 1}})),
            ..Default::default()
        },
    })
    .unwrap();
    app.drain_worker();

    assert!(
        app.stage_views.incidence.is_none(),
        "the view built from the previous compile's report survived a new report, so \
         the pane draws one run's matrix over another run's data",
    );
    assert!(
        app.stage_views.built_for.is_none(),
        "the key must go too, or the next paint reuses the cache for this same stage",
    );
    assert!(
        app.compile_views.ic_plan_anim.is_some(),
        "the frame-built replays must survive: frames arrive only with `Compiled`, so \
         dropping them here blanks every animation for the whole compile with nothing \
         to rebuild from",
    );
}

/// **A partial result must not claim the compile finished.**
///
/// `CompileProgress` carries a documented contract in its own arm: *"Compilation is
/// still running, so DON'T touch `compiling`, `stage`, `def_index`, or the bridge —
/// the final `Compiled` owns those."* Nothing checked it, and every field it names is
/// one the UI reads to decide whether work is still happening. Setting `compiling`
/// here would stop the spinner and re-enable the buttons **mid-pipeline**, which looks
/// exactly like a finished compile that produced half a model.
///
/// Written during the 2026-08-23 column read of this router, as the companion to the
/// finding recorded in `docs/unattended-runs.md`: the same arm replaces `self.stages`
/// without invalidating the view caches built from the previous report, which is a
/// question about what a pane shows and therefore Doug's to rule on rather than
/// Claude's to change.
#[test]
fn drain_worker_compile_progress_does_not_claim_the_compile_finished() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.compiling = true;
    app.stage = StageKind::Flatten;

    tx.send(FromWorker::CompileProgress {
        path,
        stages: StageBundle {
            parse: Stage::ok(serde_json::json!({"parsed": true})),
            ..Default::default()
        },
    })
    .unwrap();
    app.drain_worker();

    assert!(
        app.compiling,
        "a partial result cleared `compiling`; the spinner stops and the buttons \
         re-enable while the pipeline is still running",
    );
    assert_eq!(
        app.stage,
        StageKind::Flatten,
        "a partial result moved the displayed stage out from under the reader",
    );
    assert!(
        app.def_index.is_empty(),
        "`def_index` is the final compile's to publish",
    );
    assert!(
        app.stages.parse.value.is_some(),
        "the stages it IS responsible for must still land, or this test would pass \
         against an arm that ignored the message entirely",
    );
}

/// **A superseded navigation result must not clear the indicator for the one still
/// in flight.**
///
/// From the 2026-08-23 column read of `drain_worker`'s six arms. Three of them refuse
/// to act on a result that is no longer awaited — `CompileProgress`, `Compiled` and
/// `Simulated` each compare the message's `path` against `selected`, and `DefTree` was
/// the arm that did not.
///
/// Nothing gates navigation while a fetch runs, so clicking a second class before
/// the first returns is ordinary use, and the worker is FIFO: the first result
/// arrives and used to clear `nav_loading` outright, so the pane stopped saying
/// "opening B…" while B was still being fetched.
///
/// The pair of assertions is the point. Checking only that the stale arrival leaves
/// the indicator alone would pass on an arm that never cleared it at all.
#[test]
fn drain_worker_def_tree_keeps_the_indicator_for_a_request_still_in_flight() {
    let (mut app, tx) = App::test_with_sender();
    app.nav_loading = Some("Modelica.Electrical.Analog.Basic.Resistor".to_owned());

    // The earlier request lands after the reader has moved on to another class.
    tx.send(FromWorker::DefTree {
        name: "Modelica.Blocks.Sources.Constant".to_owned(),
        result: Ok((serde_json::json!({"class": "Constant"}), BTreeMap::new())),
    })
    .unwrap();
    app.drain_worker();

    assert_eq!(
        app.nav_loading.as_deref(),
        Some("Modelica.Electrical.Analog.Basic.Resistor"),
        "a superseded result cleared the indicator for the request still in flight, so \
         the pane reported nothing loading during a load",
    );
    assert_eq!(app.nav.len(), 1, "the earlier class was still asked for");

    // And the awaited one does clear it.
    tx.send(FromWorker::DefTree {
        name: "Modelica.Electrical.Analog.Basic.Resistor".to_owned(),
        result: Ok((serde_json::json!({"class": "Resistor"}), BTreeMap::new())),
    })
    .unwrap();
    app.drain_worker();
    assert!(app.nav_loading.is_none(), "the awaited result clears it");
    assert_eq!(app.nav.len(), 2);
}

#[test]
fn drain_worker_compiled_clears_caches_and_updates_state() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.compiling = true;
    app.stage_views.spy_plot = Some(None);
    app.stage_views.incidence = Some(None);
    app.stage_views.built_for = Some(StageKind::Parse);
    app.live_breakpoint_armed = false;

    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({})),
        ..Default::default()
    };
    tx.send(FromWorker::Compiled {
        path,
        model: Some("TestModel".into()),
        stages,
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: crate::worker::StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    })
    .unwrap();
    app.drain_worker();

    assert!(!app.compiling);
    assert_eq!(app.model.as_deref(), Some("TestModel"));
    assert!(app.stage_views.spy_plot.is_none());
    assert!(app.stage_views.incidence.is_none());
    assert!(app.stage_views.built_for.is_none());
    assert!(app.pending_live_debug.is_none());
}

#[test]
fn drain_worker_compiled_stale_path_ignored() {
    let (mut app, tx) = App::test_with_sender();
    app.selected = Some(PathBuf::from("/test/current.mo"));
    app.compiling = true;

    tx.send(FromWorker::Compiled {
        path: PathBuf::from("/test/stale.mo"),
        model: Some("StaleModel".into()),
        stages: StageBundle::default(),
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: crate::worker::StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    })
    .unwrap();
    app.drain_worker();

    assert!(app.compiling);
    assert!(app.model.is_none());
}

#[test]
fn drain_worker_compiled_applies_pending_stage() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.compiling = true;
    app.pending_stage = Some(StageKind::Flatten);

    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({})),
        resolve: Stage::ok(serde_json::json!({})),
        ..Default::default()
    };
    tx.send(FromWorker::Compiled {
        path,
        model: Some("TestModel".into()),
        stages,
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: crate::worker::StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    })
    .unwrap();
    app.drain_worker();

    assert_eq!(
        app.stage,
        StageKind::Flatten,
        "pending_stage should override last_successful_stage"
    );
    assert!(
        app.pending_stage.is_none(),
        "pending_stage should be consumed"
    );
    assert!(
        !app.viewing_log,
        "viewing_log should be cleared after compilation"
    );
}

#[test]
fn drain_worker_compiled_falls_back_without_pending_stage() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.compiling = true;
    app.pending_stage = None;

    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({})),
        resolve: Stage::ok(serde_json::json!({})),
        ..Default::default()
    };
    tx.send(FromWorker::Compiled {
        path,
        model: Some("TestModel".into()),
        stages,
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: crate::worker::StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    })
    .unwrap();
    app.drain_worker();

    assert_eq!(
        app.stage,
        StageKind::Resolve,
        "should fall back to last_successful_stage"
    );
}

#[test]
fn drain_worker_compiled_preserves_log_view() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.compiling = true;
    app.viewing_log = true;

    let stages = StageBundle {
        parse: Stage::ok(serde_json::json!({})),
        resolve: Stage::ok(serde_json::json!({})),
        ..Default::default()
    };
    tx.send(FromWorker::Compiled {
        path,
        model: Some("TestModel".into()),
        stages,
        def_index: BTreeMap::new(),
        equation_sheet: None,
        identifier_index: None,
        index_reduction_frames: Vec::new(),
        matching_frames: Vec::new(),
        tarjan_frames: Vec::new(),
        tearing_frames: Vec::new(),
        reduced_frames: crate::worker::StructuralFrames::default(),
        pre_lowering_frames: Vec::new(),
        connection_frames: Vec::new(),
        flat: None,
        dae: None,
        library_source: None,
    })
    .unwrap();
    app.drain_worker();

    assert!(app.viewing_log, "should not yank user off the Log tab");
    assert_eq!(
        app.stage,
        StageKind::Resolve,
        "stage should still be updated for when user clicks away"
    );
}

#[test]
fn drain_worker_simulated_ok_stores_data() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.sim_running = true;

    tx.send(FromWorker::Simulated {
        path,
        result: Ok(SimData {
            times: vec![0.0, 1.0],
            names: vec!["x".into()],
            data: vec![vec![0.0, 1.0]],
            n_states: 1,
            has_discontinuities: false,
            solver_steps: vec![],
        }),
    })
    .unwrap();
    app.drain_worker();

    assert!(!app.sim_running);
    assert!(app.sim_data.is_some());
    assert!(app.sim_error.is_none());
}

/// **`CopyCatcher` sees a copy issued by a plugin registered BEFORE it.**
///
/// # The property, and why it is the one to pin
///
/// egui's `LabelSelectionState` pushes `CopyText` from its own `on_end_pass`, and it is
/// registered at `context.rs:737`. Anything that hopes to observe that copy must run
/// **later in the plugin order** — and I got that wrong twice in one evening, in two
/// different ways, both silent:
///
/// 1. reading `ctx.output()` on the next frame — `end_pass` has already taken it;
/// 2. `Context::on_end_pass`, which registers into egui's `CallbackPlugin` — added at
///    `context.rs:733`, **four lines before** the selection plugin, so it runs first
///    and finds nothing.
///
/// Neither failure had a symptom of its own: the button simply did nothing, and Doug
/// could only report the cosmetics around it. **So the ordering gets a test rather than
/// a third careful reading.**
///
/// The emitter stands in for `LabelSelectionState`: registered first, copying from its
/// own `on_end_pass`. If `CopyCatcher` ever runs before it again, the sink is empty and
/// this fails by name.
#[test]
fn the_copy_catcher_runs_after_plugins_registered_before_it() {
    struct Emitter;
    impl egui::Plugin for Emitter {
        fn debug_name(&self) -> &'static str {
            "test_emitter"
        }
        fn on_end_pass(&mut self, ui: &mut egui::Ui) {
            ui.copy_text("selected prose".to_owned());
        }
    }

    let sink: CopySink = CopySink::default();
    let mut h = egui_kittest::Harness::new_ui(|_ui| {});
    // Registered in the same relative order egui uses: the copier first.
    h.ctx.add_plugin(Emitter);
    h.ctx.add_plugin(CopyCatcher(std::sync::Arc::clone(&sink)));
    h.run_steps(2);

    assert_eq!(
        sink.lock().expect("sink").take().as_deref(),
        Some("selected prose"),
        "a copy pushed by an earlier plugin's on_end_pass must be visible to ours \u{2014} \
         if it is not, the catcher is running too early and the capture silently does \
         nothing, which is exactly how this shipped twice",
    );
}

/// **A stray copy is drained rather than banked, and a 🎯 press collects only its own.**
///
/// # The bug this pins never had a symptom of its own
///
/// The first version read `ctx.output()` on the frame *after* the press. `end_pass`
/// does `std::mem::take(&mut viewport.output)`, so the `CopyText` was already gone and
/// the capture silently never happened. Doug reported the two cosmetic faults beside it
/// — the button appearing on a caret, and vanishing when clicked — and **this one he
/// could not have reported**, because a capture that does not happen looks exactly like
/// one he did not make.
///
/// # Why draining unconditionally is the load-bearing half
///
/// An ordinary Ctrl+C also produces `CopyText`, and the callback catches every one. If
/// the sink were only emptied while a press was pending, a copy made an hour earlier
/// would still be sitting there and the next 🎯 would capture *it* — a point attaching
/// itself to an older gesture, which is exactly what `PendingPassage`'s expiry exists to
/// prevent, arriving by the back door.
#[test]
fn a_stray_copy_never_becomes_the_next_capture() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();

    // A copy made with nothing pending — Ctrl+C on any label.
    *app.copy_sink.lock().expect("sink") = Some("something else entirely".to_owned());
    app.collect_pending_passage();
    assert!(
        app.copy_sink.lock().expect("sink").is_none(),
        "a copy with no capture pending must be drained, not banked for the next press",
    );
    assert!(
        app.context.pointed_at.is_none(),
        "and it must not become a point on its own \u{2014} Doug ruled that only the \
         button captures",
    );

    // Now a real press, whose own copy arrives next frame.
    app.pending_passage = Some(PendingPassage {
        lab: "connect-expansion".to_owned(),
        frames_left: 3,
    });
    *app.copy_sink.lock().expect("sink") = Some("two separate graphs".to_owned());
    app.collect_pending_passage();

    let point = app
        .context
        .pointed_at
        .as_ref()
        .expect("the press must have captured");
    assert_eq!(point.target, "two separate graphs");
    assert!(
        point.stage.is_none(),
        "prose is not in a phase, and the whole type change was to be able to say so",
    );
    match &point.kind {
        PointKind::LabPassage { lab } => assert_eq!(lab, "connect-expansion"),
        _ => panic!("expected a lab passage"),
    }
}

/// **A 🎯 press that collects nothing expires, and says why.**
///
/// egui's `has_selection()` is `selection.is_some()`, and a click that merely places a
/// caret makes it `Some` — nothing public distinguishes a caret from a selection, so
/// the button appears for both. Pressing it on a caret produces no copy, and this is
/// the path that must not end in silence: the Context Bar would go on showing the
/// PREVIOUS point, and Doug would ask about a passage Claude never received.
#[test]
fn a_press_with_nothing_selected_expires_and_says_so() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.pending_passage = Some(PendingPassage {
        lab: "connect-expansion".to_owned(),
        frames_left: 2,
    });

    app.collect_pending_passage();
    assert!(
        app.pending_passage.is_some(),
        "one quiet frame is the round trip, not a failure",
    );

    app.collect_pending_passage();
    assert!(
        app.pending_passage.is_none(),
        "but it must give up rather than stay armed for the next unrelated copy",
    );
    let notice = app.notice.clone().unwrap_or_default();
    assert!(
        notice.contains("cursor position is not a selection"),
        "the notice must name the likeliest cause, not just report failure: {notice:?}",
    );
    assert!(
        app.context.pointed_at.is_none(),
        "and nothing may be captured",
    );
}

/// **A run in progress is announced once, in words, by the Simulation pane.**
///
/// Doug removed the pane's spinner on 2026-08-30 — the same ruling as the two Run
/// buttons an hour earlier, and the same division of labour: **the tab row carries
/// the spinner, the pane says what is happening.** He kept the wording, so the two
/// no longer duplicate each other.
///
/// # What this can and cannot see, which is why it asserts a sentence
///
/// `ui.spinner()` paints and carries no accessibility label, so **neither the spinner
/// that went nor the one that stayed is queryable** — the same blind spot as the
/// extra divider removed the same day. This pins the half that is visible: that a
/// running simulation still says so here. It is not a check that the spinner is gone,
/// and no headless test can be.
#[test]
fn a_running_simulation_is_announced_in_the_pane() {
    use egui_kittest::kittest::Queryable;

    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", StageKind::Simulation);
    app.sim_running = true;

    let h = crate::ui_tests::harness(app);
    assert!(
        h.query_by_label_contains("simulating").is_some(),
        "a run in progress must still be announced in the pane \u{2014} the tab row's \
         spinner is a bare painted widget with no words, so dropping this sentence \
         would leave the Simulation view silent about what it is doing",
    );
}

#[test]
fn drain_worker_simulated_err_stores_error() {
    let (mut app, tx) = App::test_with_sender();
    let path = PathBuf::from("/test/specimen.mo");
    app.selected = Some(path.clone());
    app.sim_running = true;

    tx.send(FromWorker::Simulated {
        path,
        result: Err("solver diverged".into()),
    })
    .unwrap();
    app.drain_worker();

    assert!(!app.sim_running);
    assert!(app.sim_data.is_none());
    assert!(app.sim_error.as_deref() == Some("solver diverged"));
}

#[test]
fn drain_worker_log_appends_entry() {
    let (mut app, tx) = App::test_with_sender();
    tx.send(FromWorker::Log(LogEntry {
        elapsed_secs: 0.0,
        level: crate::worker::LogLevel::Info,
        message: "test log".into(),
        depth: 0,
    }))
    .unwrap();
    app.drain_worker();
    assert_eq!(app.log_entries.len(), 1);
    assert!(app.log_entries[0].message.contains("test log"));
}

/// **Every link verb states whether it needs a specimen, and opening a document does not.**
///
/// # This test is NOT what stops a new verb being forgotten — read `requires_specimen`
///
/// It used to claim it was, and the claim was false in the way that matters: a test over a
/// **hand-written list of siblings cannot see a new sibling.** It listed three exempt verbs
/// and one requiring one, passed for 26 days, and was green the day `OpenDoc` shipped
/// defaulting to the wrong answer — the second time Doug reported *"the link does nothing"*
/// from this one function. **The enforcement is now the exhaustive `match` in
/// `requires_specimen`, which does not compile until a new variant is ruled on.**
///
/// **So what this test is still for** is the part a compiler cannot check: that the
/// rulings are the *right* ones. An exhaustive match forces an answer; it cannot tell a
/// correct answer from a confident wrong one, and `OpenDoc` returning `true` would have
/// compiled perfectly. The cases below are the ones whose answer was got wrong in
/// practice, plus a contrast so the whole thing cannot pass vacuously.
#[test]
fn link_verbs_declare_whether_they_need_a_specimen() {
    // Navigation between FILES: no model is involved, so a reader with nothing loaded
    // must still be able to follow a citation. Both historical failures are in this
    // list — `OpenLab` (2026-08-05) and `OpenDoc` (2026-08-31).
    for link in [
        HrwLink::OpenLab {
            lab: "failure-parse".to_owned(),
            stop: None,
        },
        HrwLink::OpenLab {
            lab: "failure-parse".to_owned(),
            stop: Some("station-1-the-failure-itself".to_owned()),
        },
        HrwLink::OpenDoc("upstream-issues.md".to_owned()),
        HrwLink::OpenSource("crates/rumoca-core/src/lib.rs#VarName".to_owned()),
        HrwLink::OpenNotebook("structural-vs-numerical-rank.nb".to_owned()),
        HrwLink::ArmBreakpoint("augment-entry".to_owned()),
        HrwLink::LoadSpecimen("RcCircuit".to_owned()),
    ] {
        assert!(
            !link.requires_specimen(),
            "{} must be followable with nothing loaded",
            link.describe(),
        );
    }
    // And the contrast, so this is not vacuously true: a verb that acts *on* a
    // loaded model genuinely does need one.
    assert!(
        HrwLink::SwitchStage(StageKind::Structural, None).requires_specimen(),
        "switching stages without a specimen would half-apply",
    );
    assert!(
        HrwLink::Follow("resistor.R".to_owned()).requires_specimen(),
        "following an identifier with no model has no referent",
    );
}

/// **A verb that opens another application gets autoplay's longer beat.**
///
/// Same shape as the test above and the same reason for existing: the exhaustive match in
/// `leaves_hrw` guarantees an *answer* per verb, never a *correct* one. The two cases that
/// separate a right answer from a plausible one are here — `OpenDoc` spawns `code` and so
/// hops out, while `OpenLab` opens in HRW's own panel and does not, which is the pair a
/// reader skimming "the Open* verbs" would get wrong.
#[test]
fn a_verb_that_opens_another_application_gets_a_longer_beat() {
    for link in [
        HrwLink::OpenDoc("upstream-issues.md".to_owned()),
        HrwLink::OpenSource("crates/rumoca-core/src/lib.rs#VarName".to_owned()),
        HrwLink::OpenNotebook("structural-vs-numerical-rank.nb".to_owned()),
        HrwLink::OpenInSystemModeler("RcCircuit".to_owned()),
    ] {
        assert!(
            link.leaves_hrw(),
            "{} puts another window in front of the reader",
            link.describe(),
        );
    }
    for link in [
        HrwLink::OpenLab {
            lab: "the-concepts".to_owned(),
            stop: None,
        },
        HrwLink::LoadSpecimen("RcCircuit".to_owned()),
        HrwLink::SwitchStage(StageKind::Structural, None),
    ] {
        assert!(!link.leaves_hrw(), "{} happens inside HRW", link.describe(),);
    }
}

/// **The lab-citation forms parse.** Missing when the form shipped, which is
/// exactly the gap a must-fire test exists to close: `fixture_lab_links_all_resolve`
/// only checks links found in *fixture* labs, and `lab_citations_…` only checks
/// that cited names exist — **neither exercises the parser itself**, so a form that
/// failed to parse would show up as "the link does nothing" with nothing failing.
/// *(Placed with its own `#[test]`, and the one below restored: inserting this
/// between `#[test]` and `fn parse_hrw_link_load_specimen` stole the attribute and
/// silently un-tested that function — the fourth recorded instance of the trap
/// `CLAUDE.md` warns about. **`dead_code = "deny"`, adopted this morning, is what
/// caught it**; without that lint the suite would have gone green one test short.)*
#[test]
fn parse_hrw_link_lab_and_stop() {
    assert_eq!(
        parse_hrw_link("hrw://lab/failure-parse"),
        Some(HrwLink::OpenLab {
            lab: "failure-parse".to_owned(),
            stop: None
        }),
    );
    assert_eq!(
        parse_hrw_link(
            "hrw://lab/failure-parse/station/station-4-the-distinction-this-specimen-anchors"
        ),
        Some(HrwLink::OpenLab {
            lab: "failure-parse".to_owned(),
            stop: Some("station-4-the-distinction-this-specimen-anchors".to_owned()),
        }),
    );
    // An empty name names nothing, as with `notebook/`.
    assert_eq!(parse_hrw_link("hrw://lab/"), None);
}

#[test]
fn parse_hrw_link_load_specimen() {
    let link = parse_hrw_link("hrw://load/BouncingBall");
    assert!(matches!(link, Some(HrwLink::LoadSpecimen(ref s)) if s == "BouncingBall"));
}

/// **An unknown breakpoint anchor must NOT parse** (`docs/ideas.md` #73).
///
/// This is the whole reason the name is validated in the parser rather than
/// at dispatch: `fixture_lab_links_all_resolve` walks every link in every
/// lab, so a typo — or an anchor whose locating fragment was edited away —
/// fails the suite. Accepting the link and reporting the problem at click
/// time would move the discovery into the middle of a walk, which is exactly
/// where a lab must not surprise its reader.
#[test]
fn parse_hrw_link_breakpoint_validates_the_anchor_name() {
    assert!(
        matches!(
            parse_hrw_link("hrw://breakpoint/decision"),
            Some(HrwLink::ArmBreakpoint(ref n)) if n == "decision"
        ),
        "`decision` is a real anchor and must parse",
    );
    assert_eq!(
        parse_hrw_link("hrw://breakpoint/desicion"),
        None,
        "a misspelled anchor must fail the link checker, not the walker",
    );
    assert_eq!(parse_hrw_link("hrw://breakpoint/"), None);
}

/// **Arming a breakpoint needs no specimen.**
///
/// It targets Rumoca's source, and `matching-live.md` deliberately has the
/// reader place breakpoints in a session before the model finishes
/// compiling. Requiring one would refuse the link at exactly the moment the
/// lab tells them to click it.
#[test]
fn arming_a_breakpoint_does_not_require_a_specimen() {
    assert!(
        !HrwLink::ArmBreakpoint("decision".to_owned()).requires_specimen(),
        "the anchor is in matching.rs, not in the model",
    );
}

#[test]
fn parse_hrw_link_switch_stage() {
    let link = parse_hrw_link("hrw://stage/Structural");
    assert!(matches!(
        link,
        Some(HrwLink::SwitchStage(StageKind::Structural, None))
    ));
}

#[test]
fn parse_hrw_link_load_and_switch() {
    let link = parse_hrw_link("hrw://load/GearWithBrake/Parse");
    assert!(
        matches!(link, Some(HrwLink::LoadAndSwitch(ref s, StageKind::Parse, None)) if s == "GearWithBrake")
    );
}

#[test]
fn parse_hrw_link_invalid_stage() {
    assert!(parse_hrw_link("hrw://stage/Bogus").is_none());
}

#[test]
fn parse_hrw_link_not_hrw_scheme() {
    assert!(parse_hrw_link("https://example.com").is_none());
}

/// `hrw://doc/` keeps the whole tail, because documents nest.
///
/// The one thing this form does that no other link form does: it **rejoins** the
/// remaining segments instead of matching a fixed count. `splitn(5, '/')` was already
/// raised from 4 once, when `stage/…/node/<n>` silently glommed into one segment and the
/// link did nothing on click — the worst outcome in a lab, since the screen says
/// nothing. A subdirectory path is the same hazard in a new place, so it is pinned here.
#[test]
fn parse_hrw_link_doc_keeps_a_nested_path() {
    assert!(
        matches!(parse_hrw_link("hrw://doc/upstream-issues.md"), Some(HrwLink::OpenDoc(ref n)) if n == "upstream-issues.md")
    );
    assert!(
        matches!(parse_hrw_link("hrw://doc/compiler-phases/the-chain-of-problems.md"), Some(HrwLink::OpenDoc(ref n)) if n == "compiler-phases/the-chain-of-problems.md")
    );
    // A verb with no object is not a link. Resolution is `bridge::resolve_doc`'s job
    // at click time, matching `hrw://notebook/`; only emptiness is refused here.
    assert!(parse_hrw_link("hrw://doc/").is_none());
    assert!(parse_hrw_link("hrw://doc").is_none());
}

/// `hrw://src/` keeps a deep path AND the `#symbol` riding on its tail.
///
/// The tail is where this could quietly break: `splitn(5, '/')` has already been raised
/// from 4 once, when `stage/…/node/<n>` glommed into one segment and the link did nothing
/// on click. A source path is the deepest form in the grammar — six segments before the
/// `#` — so it is the one most likely to hit that cap next.
#[test]
fn parse_hrw_link_src_keeps_a_deep_path_and_its_symbol() {
    let deep = "crates/rumoca-phase-flatten/src/connections/mod.rs";
    assert!(
        matches!(parse_hrw_link(&format!("hrw://src/{deep}")), Some(HrwLink::OpenSource(ref t)) if t == deep)
    );
    assert!(
        matches!(parse_hrw_link(&format!("hrw://src/{deep}#union")), Some(HrwLink::OpenSource(ref t)) if *t == format!("{deep}#union"))
    );
    // Resolution is `bridge::resolve_source`'s job at click time, matching every other
    // Open* verb; only emptiness is refused by the grammar.
    assert!(parse_hrw_link("hrw://src/").is_none());
    assert!(parse_hrw_link("hrw://src").is_none());
}

/// **A width the panel had no choice about must not become a remembered fraction.**
///
/// Doug, 2026-08-16: the divider jumps to ~75 % when the window is maximized after
/// being normalized. The numbers `observe` recorded named the cause exactly, and
/// this test is built from them rather than from invented ones:
///
/// ```text
/// split: 0.400 of window (panel 461px, available 1152px)   <- startup, correct
/// split: 0.750 of window (panel 200px, available  267px)   <- the jump
/// ```
///
/// At `avail = 267` the legal range collapses to a single point — the maximum is
/// 267 × 0.75 = 200.25 and the 210pt floor is above it. So 0.750 was arithmetic
/// forced by the window being narrow, and remembering it as a **proportion**
/// applied it to the maximized window.
///
/// The property: after a narrow window pins the panel, the reader's own split
/// survives, so restoring the window restores what they chose.
#[test]
fn a_pinned_panel_width_does_not_overwrite_the_chosen_split() {
    let mut split = SplitState::default();

    // A real choice on a roomy window is learned.
    //
    // **480/1200 rather than 461/1152 since 2026-08-19**: `MIN_LEFT_POINTS` rose to
    // 465 when the lab bar stopped wrapping, so 461 is now *below the floor* and is
    // correctly read as pinned rather than chosen. The ratio is still 0.40 — the
    // property under test is unchanged, only the window it needs to fit in.
    split.observe(480.0, 1200.0);
    let chosen = split.fraction.expect("a chosen split is remembered");
    assert!(
        (chosen - 0.4).abs() < 0.01,
        "expected ~0.40 from 461/1152, got {chosen}",
    );

    // The narrow window pins the panel: at 267 points the range is a single value.
    let (min_w, max_w) = SplitState::width_range(267.0);
    assert!(
        max_w - min_w < 1.0,
        "precondition: at 267pt the legal range must be degenerate ({min_w}..{max_w}) \
             — if the constants change, this test is no longer about the reported bug",
    );

    split.observe(200.0, 267.0);
    assert_eq!(
        split.fraction,
        Some(chosen),
        "a width the panel had no choice about became the remembered split, so \
             maximizing the window would put the divider at 75%",
    );
}

#[test]
fn extract_hrw_links_from_markdown() {
    let md = "Click [here](hrw://load/Foo) or [there](hrw://stage/Parse) end.";
    let links = extract_hrw_links(md);
    assert_eq!(links, vec!["hrw://load/Foo", "hrw://stage/Parse"]);
}

#[test]
fn extract_hrw_links_deduplicates() {
    let md = "[a](hrw://load/X) and [b](hrw://load/X) again.";
    let links = extract_hrw_links(md);
    assert_eq!(links.len(), 1);
}

/// **Every `hrw://` link in every committed lab survives extraction AND parsing.**
///
/// Doug, 2026-08-16: *"There are two Act 2 links which don't cause any action when
/// clicked."* `fixture_lab_links_all_resolve` was green, because it parses URLs it
/// is handed. Nothing checked the step *before* that — that `extract_hrw_links`,
/// which decides where a URL **ends**, hands over the same string the author wrote.
///
/// A hook is registered under the extracted string and fired by the exact URL in
/// the document, so an extractor that stops one character early registers a hook
/// that can never fire: the link renders, the cursor changes over it, the click
/// lands, and nothing happens. **A link checker that starts from the parser cannot
/// see this**, which is why it went unnoticed while a test claimed the links
/// resolved.
#[test]
fn every_lab_link_survives_extraction_and_parsing() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
    let mut checked = 0usize;
    let mut broken: Vec<String> = Vec::new();

    for entry in std::fs::read_dir(&dir)
        .expect("fixture-labs exists")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable lab");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let extracted = extract_hrw_links(&text);

        // Every URL as it appears in a markdown target, taken independently of
        // the extractor so the two cannot agree on a shared mistake.
        for (i, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(at) = rest.find("](hrw://") {
                let after = &rest[at + 2..];
                let Some(close) = after.find(')') else { break };
                let url = &after[..close];
                rest = &after[close..];

                checked += 1;
                if !extracted.iter().any(|e| e == url) {
                    broken.push(format!(
                        "{name}:{}: `{url}` is written in the document but \
                             `extract_hrw_links` produced something else, so the hook \
                             registered for it can never fire",
                        i + 1
                    ));
                } else if parse_hrw_link(url).is_none() {
                    broken.push(format!("{name}:{}: `{url}` does not parse", i + 1));
                }
            }
        }
    }

    assert!(
        checked >= 40,
        "only {checked} markdown links were inspected across the labs; the \
             extraction is broken, not the labs",
    );
    assert!(
        broken.is_empty(),
        "{} lab link(s) are clickable but inert:\n  {}",
        broken.len(),
        broken.join("\n  "),
    );
}

/// **A fired hook is consumed, so a link below it is still reachable.**
///
/// The regression behind Doug's report that two Station 2 links "don't cause any
/// action when clicked". `egui_commonmark` never clears a hook it sets, and
/// `drain_hrw_hooks` only *read* it — so the first link clicked anywhere in a lab
/// was re-dispatched every frame forever, and being first in document order it
/// masked every link below it.
///
/// Two assertions, and the second is the one that was broken:
///
/// 1. the fired link is returned;
/// 2. it does not fire again, and a *later* link fires normally afterwards.
#[test]
fn a_fired_link_hook_is_consumed_and_does_not_mask_later_links() {
    let mut cache = egui_commonmark::CommonMarkCache::default();
    let links = vec![
        "hrw://load/RcCircuit".to_owned(),
        "hrw://stage/Flatten/Connections/frame/7".to_owned(),
    ];
    register_hrw_hooks(&mut cache, &links);

    // Simulate a click on the FIRST link, the way the renderer does.
    cache.link_hooks_mut().insert(links[0].clone(), true);
    assert!(
        matches!(
            drain_hrw_hooks(&mut cache, &links),
            Some(HrwLink::LoadSpecimen(_))
        ),
        "the clicked link must dispatch",
    );
    assert!(
        drain_hrw_hooks(&mut cache, &links).is_none(),
        "a hook that stays fired re-dispatches on every frame and masks every \
             link below it \u{2014} this is the bug",
    );

    // Now the SECOND link, which was unreachable before the fix.
    cache.link_hooks_mut().insert(links[1].clone(), true);
    assert!(
        matches!(
            drain_hrw_hooks(&mut cache, &links),
            Some(HrwLink::SeekFrame(StageKind::Flatten, _, 6)),
        ),
        "a link later in the document must dispatch once the one above it is \
             consumed",
    );
    assert!(
        drain_hrw_hooks(&mut cache, &links).is_none(),
        "and it must be consumed too",
    );

    // Re-registering must not resurrect a consumed hook.
    register_hrw_hooks(&mut cache, &links);
    assert!(
        drain_hrw_hooks(&mut cache, &links).is_none(),
        "registration runs every frame; it must not re-fire what was consumed",
    );
}

#[test]
fn stage_kind_from_slug_round_trips() {
    for kind in StageKind::ALL {
        let slug = kind.slug();
        assert_eq!(StageKind::from_slug(slug), Some(*kind));
    }
}

/// A scratch specimen is listed, marked, and findable by name (ideas #42).
///
/// The point of the split is that Claude can write "here is the smallest model
/// that shows the thing you asked about" and have it appear in HRW without
/// touching the curated corpus — whose portable-subset, `// purpose:` and
/// System-Modeler-round-trip properties a disposable probe would degrade.
#[test]
fn a_scratch_specimen_is_listed_and_marked() {
    // **The test establishes its own precondition** rather than depending on a probe
    // happening to be on disk. It used to return early when one was absent, so in a
    // clean checkout it asserted nothing — and nothing said so. The guard restores
    // whatever was there on drop, including on a panic; see `ScratchSpecimen`.
    let probe_file = crate::test_support::ScratchSpecimen::probe();
    let probe = probe_file.path().to_path_buf();

    let mut app = App::test_default();
    app.model_list.dir = DEFAULT_SPECIMEN_DIR.to_owned();
    app.model_list.rescan();

    assert!(
        app.model_list.files.contains(&probe),
        "scratch specimens join the list"
    );
    // Scratch sorts BEFORE the curated corpus, matching the lab list: the
    // just-written thing is the one most likely wanted next, and burying it under
    // 18 curated specimens made the common case the awkward one.
    //
    // **Stated as the property, not as "the probe is files[0]"** — that form was
    // true only while `ScratchProbe.mo` was the ONLY scratch file, and it failed
    // the moment a second one existed (2026-08-22: two probes written to answer a
    // question about connector type checking, both sorting ahead of it).
    // `.hrw-bridge/specimens/` is live state the suite does not control, the same
    // class as `.hrw-bridge/lab.md`, and the fix is to assert what the feature
    // promises rather than what one directory happened to contain.
    let last_scratch = app
        .model_list
        .files
        .iter()
        .rposition(|p| app.model_list.scratch.contains(p))
        .expect("the probe is itself scratch, so at least one exists");
    if let Some(first_curated) = app
        .model_list
        .files
        .iter()
        .position(|p| !app.model_list.scratch.contains(p))
    {
        assert!(
            last_scratch < first_curated,
            "scratch specimens lead the list: {:?}",
            app.model_list.files.iter().take(5).collect::<Vec<_>>(),
        );
    }
    assert!(
        app.model_list.scratch.contains(&probe),
        "and are marked as scratch"
    );
    assert_eq!(
        app.find_specimen("ScratchProbe"),
        Some(probe),
        "and are reachable by name, so `hrw://load/ScratchProbe` works",
    );

    // The curated corpus is untouched and still unmarked.
    let curated = app
        .model_list
        .files
        .iter()
        .find(|p| p.file_name().and_then(|n| n.to_str()) == Some("BouncingBall.mo"))
        .expect("BouncingBall is curated");
    assert!(!app.model_list.scratch.contains(curated));
}

/// A scratch specimen may not shadow a curated one.
///
/// Loading a different model than the name says is the "makes Claude guess"
/// failure: Claude would reason confidently about source Doug is not looking at.
/// So the collision is reported and the scratch file skipped, rather than either
/// one silently winning.
#[test]
fn a_scratch_specimen_cannot_shadow_a_curated_one() {
    // **The sharp case for the guard**: a `BouncingBall.mo` left in the scratch
    // directory shadows a curated specimen, which is the "makes Claude guess" failure
    // this very test exists to prevent. The old form removed it on the last line, so a
    // failing assertion left it behind; `Drop` runs while unwinding and does not.
    let clash_file = crate::test_support::ScratchSpecimen::with(
        "BouncingBall.mo",
        "model BouncingBall end BouncingBall;\n",
    );
    let clash = clash_file.path().to_path_buf();

    let mut app = App::test_default();
    app.model_list.dir = DEFAULT_SPECIMEN_DIR.to_owned();
    app.model_list.rescan();

    assert!(
        app.model_list
            .shadowed
            .iter()
            .any(|n| n == "BouncingBall.mo"),
        "the collision is reported: {:?}",
        app.model_list.shadowed,
    );
    assert!(
        !app.model_list.scratch.contains(&clash),
        "and the scratch file is skipped"
    );
    // The curated one still wins, and is what the name resolves to.
    let found = app.find_specimen("BouncingBall").expect("still findable");
    assert!(
        found.starts_with(DEFAULT_SPECIMEN_DIR),
        "curated wins: {found:?}"
    );
}

#[test]
fn find_specimen_matches_by_filename() {
    let mut app = App::test_default();
    app.model_list.files = vec![
        PathBuf::from("/specimens/BouncingBall.mo"),
        PathBuf::from("/specimens/Drivetrain.mo"),
    ];
    assert_eq!(
        app.find_specimen("BouncingBall"),
        Some(PathBuf::from("/specimens/BouncingBall.mo"))
    );
}

#[test]
fn find_specimen_returns_none_for_missing() {
    let mut app = App::test_default();
    app.model_list.files = vec![PathBuf::from("/specimens/BouncingBall.mo")];
    assert!(app.find_specimen("NonExistent").is_none());
}

#[test]
fn find_specimen_does_not_match_substring() {
    let mut app = App::test_default();
    app.model_list.files = vec![PathBuf::from("/specimens/BouncingBall.mo")];
    assert!(app.find_specimen("Bouncing").is_none());
}

/// A link can point at the source, with or without a line.
///
/// Closes the second quiet lab hole: two labs *quoted* a source line because
/// nothing could point at one.
#[test]
fn a_link_can_point_at_a_source_line() {
    assert_eq!(
        parse_hrw_link("hrw://source/9"),
        Some(HrwLink::ShowSource(Some(9)))
    );
    assert_eq!(
        parse_hrw_link("hrw://source"),
        Some(HrwLink::ShowSource(None))
    );
    // A non-numeric line is malformed, not line 0 and not "the whole file".
    assert!(parse_hrw_link("hrw://source/nine").is_none());
}

/// **A high-index model must never have its source blamed.**
///
/// This is the design condition, not a nicety. `MotorWithBrake` is structurally
/// singular, has an unmatched unknown, and has a source line for it — and it is a
/// perfectly good model that index reduction solves by demoting a state. Painting
/// its `connect()` as a problem would teach the opposite of the lesson the
/// Structural/IndexReduction contrast exists to teach.
///
/// `CapacitorLoop` is the case where blame is real: states 1 → 1, nothing demoted,
/// still singular, so nothing downstream can save it.
#[test]
fn only_an_unrescuable_model_gets_its_source_blamed() {
    // Structural failed, index reduction rescued it → no blame.
    let mut app = App::test_default();
    // Struct literals rather than `Stage::ok`/`recovered`: those constructors are
    // the worker's own, and the UI consumes stages read-only.
    let stage = Stage::ok;
    app.stages.structural = stage(serde_json::json!({ "error": { "kind": "singular" } }));
    app.stages.index_reduction = stage(serde_json::json!({ "blocks": [] }));
    app.compute_problem_lines();
    assert!(
        app.problem_lines.is_empty(),
        "a high-index model that index reduction fixed must not be blamed",
    );

    // Structural failed AND index reduction failed → blame, with the line.
    app.stages.index_reduction = stage(serde_json::json!({
        "error": {
            "kind": "singular",
            "unmatched_unknown_locations": [
                { "unknown": "gnd.p.i", "location": { "line": 9 } }
            ]
        }
    }));
    app.compute_problem_lines();
    assert_eq!(app.problem_lines.len(), 1);
    assert_eq!(app.problem_lines[0].0, 9);
    assert!(
        app.problem_lines[0].1.contains("ill-posed"),
        "the hover must say why: {}",
        app.problem_lines[0].1,
    );

    // An unknown with no source provenance contributes no blamed line rather than
    // a bogus one — manufactured and solver-vector variables have no source.
    app.stages.index_reduction = stage(serde_json::json!({
        "error": {
            "kind": "singular",
            "unmatched_unknown_locations": [
                { "unknown": "__solver_y_3", "location": null }
            ]
        }
    }));
    app.compute_problem_lines();
    assert!(
        app.problem_lines.is_empty(),
        "no span means no line to blame"
    );
}

/// A link can address a sub-view, on both the load and the switch forms.
///
/// Closes the quiet lab hole logged 2026-07-29: links reached a stage tab and
/// no further, so every animation had to be handed off in prose ("same tab →
/// now click **Incidence**"). The first lab had two working links and four
/// such hand-offs.
#[test]
fn a_link_can_address_a_sub_view() {
    assert_eq!(
        parse_hrw_link("hrw://stage/Structural/MatchingAnim"),
        Some(HrwLink::SwitchStage(
            StageKind::Structural,
            Some(SubView::Structural(StructuralView::MatchingAnim)),
        )),
    );
    assert_eq!(
        parse_hrw_link("hrw://load/MotorWithBrake/IndexReduction/AliasAnim"),
        Some(HrwLink::LoadAndSwitch(
            "MotorWithBrake".to_owned(),
            StageKind::IndexReduction,
            Some(SubView::Structural(StructuralView::AliasAnim)),
        )),
    );
    // The bare forms still work — a sub-view is optional, not required.
    assert_eq!(
        parse_hrw_link("hrw://stage/Flatten"),
        Some(HrwLink::SwitchStage(StageKind::Flatten, None)),
    );
}

/// **The noun/verb parity audit, as a test.**
///
/// #42's design principle is that `hrw://` must express any noun `focus.json` can
/// describe — same vocabulary, opposite directions. `SubView::from_slug`'s doc
/// comment asserted "the slugs are exactly the names the capture emits", and until
/// 2026-07-29 **nothing checked it**: an unverified claim about verification, which
/// is the failure this project keeps finding in its own records.
///
/// Doug asked for the audit to be run "as often as necessary". A manual audit rots;
/// this one runs in the 7-second loop. It fails when a view variant is added to one
/// side only — which is exactly what happens when a new feature introduces a noun.
#[test]
fn every_capture_view_name_round_trips_as_a_link_slug() {
    // (stage the sub-view belongs to, its capture name, the expected parse)
    let mut checked = 0usize;

    for v in StructuralView::ALL {
        // Structural sub-views are reachable under both stages that share the enum.
        for stage in [StageKind::Structural, StageKind::IndexReduction] {
            let name = structural_view_name(*v);
            assert_eq!(
                SubView::from_slug(stage, name),
                Some(SubView::Structural(*v)),
                "capture emits {name:?} for {v:?} under {stage:?}, but hrw:// cannot parse it \
                     back — the two vocabularies have drifted",
            );
            checked += 1;
        }
    }
    for v in FlattenView::ALL {
        let name = flatten_view_name(*v);
        assert_eq!(
            SubView::from_slug(StageKind::Flatten, name),
            Some(SubView::Flatten(*v)),
            "Flatten: capture emits {name:?}, hrw:// cannot parse it",
        );
        checked += 1;
    }
    for v in EventsView::ALL {
        let name = events_view_name(*v);
        assert_eq!(
            SubView::from_slug(StageKind::Events, name),
            Some(SubView::Events(*v)),
            "Events: capture emits {name:?}, hrw:// cannot parse it",
        );
        checked += 1;
    }
    for v in InitView::ALL {
        let name = init_view_name(*v);
        assert_eq!(
            SubView::from_slug(StageKind::Initialization, name),
            Some(SubView::Init(*v)),
            "Initialization: capture emits {name:?}, hrw:// cannot parse it",
        );
        checked += 1;
    }

    assert!(
        checked >= 26,
        "expected every view variant covered, checked {checked}"
    );
}

/// **Every noun the capture can describe is reachable by a link.**
///
/// The whole of #42's design principle, in one assertion per noun. Written as an
/// exhaustive match on `Focus` and a field-by-field walk of `Tracking`, so *adding a
/// noun to the capture fails this test until a verb exists for it* — which is the
/// only way the principle stays true rather than becoming a paragraph nobody checks.
///
/// Two gaps stood open until 2026-07-29: `Focus::Node` (the capture's richest noun,
/// produced by every left-click) and the follow. Both are closed here.
#[test]
fn every_capture_noun_is_reachable_by_a_link() {
    // `Focus`, exhaustively. The match is the point: a new variant will not compile
    // until it is considered here.
    let unreachable: Vec<&str> = [
        (
            "Focus::Node",
            parse_hrw_link("hrw://stage/Structural/Tree/node/error.unmatched_unknowns[0]")
                .is_some(),
        ),
        (
            "Focus::Stage",
            parse_hrw_link("hrw://stage/Structural").is_some(),
        ),
        (
            "Focus::Specimen",
            parse_hrw_link("hrw://load/CapacitorLoop").is_some(),
        ),
        // `Focus::Nothing` is the absence of a point; there is nothing to navigate
        // to, and a verb for it would mean "un-point", which no lab has wanted.
        (
            "Tracking::name",
            parse_hrw_link("hrw://follow/emf.phi").is_some(),
        ),
        // The rest of `Tracking` is derived from the name (declaring class, source
        // line, per-stage mentions), so setting the name sets all of it.
        (
            "view.stage_view",
            parse_hrw_link("hrw://stage/Structural/MatchingAnim").is_some(),
        ),
        (
            "view.animation.frame",
            parse_hrw_link("hrw://stage/Structural/MatchingAnim/frame/1").is_some(),
        ),
        (
            "specimen source line",
            parse_hrw_link("hrw://source/9").is_some(),
        ),
    ]
    .into_iter()
    .filter_map(|(noun, reachable)| (!reachable).then_some(noun))
    .collect();

    assert!(
        unreachable.is_empty(),
        "the capture can describe these, and no link can reach them: {unreachable:?}",
    );
}

/// Every stage a capture can name is reachable by a link, and back.
///
/// The other half of parity: `focus.json` carries a `stage`, so `hrw://stage/<X>`
/// must accept every one of them. A stage added to `StageKind` without a slug would
/// be describable but not navigable.
#[test]
fn every_stage_round_trips_between_capture_and_link() {
    for kind in StageKind::ALL {
        let slug = kind.slug();
        assert_eq!(
            StageKind::from_slug(slug),
            Some(*kind),
            "{kind:?} emits slug {slug:?} which does not parse back",
        );
        assert_eq!(
            parse_hrw_link(&format!("hrw://stage/{slug}")),
            Some(HrwLink::SwitchStage(*kind, None)),
            "hrw://stage/{slug} must navigate to {kind:?}",
        );
    }
}

/// Every link in every **fixture lab** resolves against the current parser.
///
/// A fixture lab is kept and versioned — unlike an ad hoc lab, which is gitignored
/// and regenerated per question. The ephemerality rule was never about labs; it was
/// about *explanation*, which rots because nothing checks it. A fixture lab has a
/// pass/fail criterion, and **this test is what makes that true**: without something
/// executing it, a saved lab is stored prose with extra steps, and would drift from
/// the app exactly as `end_to_end_lab.md`'s 7x7 matrix did.
///
/// Checks the links only. Whether the camera *looks* right is Doug's half — that is
/// the whole reason the fixture exists.
/// The picker names each lab by what it *is*, not by where it lives.
#[test]
fn lab_labels_name_what_the_lab_is() {
    assert!(
        LabSource::AdHoc.label().contains("Answer"),
        "the ad hoc lab is named by its role; its filename is an implementation \
             detail nobody should need to know",
    );
    let fixture = LabSource::Fixture(PathBuf::from("/x/docs/fixture-labs/camera-aiming.md"));
    assert_eq!(fixture.label(), "camera-aiming");
    assert_eq!(
        fixture.path(),
        PathBuf::from("/x/docs/fixture-labs/camera-aiming.md")
    );
    assert_eq!(
        LabSource::AdHoc.path(),
        PathBuf::from(crate::bridge::LAB_FILE)
    );
}

/// Switching labs re-initialises the right-hand side; re-selecting does not.
///
/// Doug: clicking a link in one lab and then choosing a second lab left the first
/// lab's specimen on screen. A lab is a self-contained sequence whose first stop
/// loads a specimen, so the leftover state invites reading the new lab's stops
/// against the old lab's model — and makes Station 1 look already done.
///
/// The reset reuses `open`'s own field list via `clear_specimen_state`, rather than
/// a second copy that would drift from it.
#[test]
fn switching_labs_resets_the_stage_side() {
    let a = LabSource::Fixture(PathBuf::from("/x/a.md"));
    let b = LabSource::Fixture(PathBuf::from("/x/b.md"));

    let mut app = App::test_default();
    app.select_lab(a.clone());
    // Simulate having walked a stop: a specimen loaded, a stage reached.
    app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
    app.model = Some("RcCircuit".to_owned());
    app.stage = StageKind::Structural;

    // Re-selecting the SAME lab must not throw away work in progress.
    app.select_lab(a.clone());
    assert_eq!(
        app.selected,
        Some(PathBuf::from("/x/RcCircuit.mo")),
        "reselect keeps state"
    );
    assert_eq!(app.model.as_deref(), Some("RcCircuit"));

    // A different lab starts clean.
    app.select_lab(b.clone());
    assert_eq!(app.selected, None, "the specimen is cleared");
    assert_eq!(app.model, None, "and so is the model");
    assert_eq!(
        app.stage,
        StageKind::Parse,
        "and the stage returns to the start"
    );
    assert_eq!(app.lab.selected, Some(b));
}

/// The list offers the fixtures, ad hoc first when one exists.
///
/// Doug asked for in-app selection so a fixture lab no longer has to be copied over
/// `.hrw-bridge/lab.md` before starting HRW. Ad hoc goes first because it answers
/// the question just asked; burying it under the fixtures would make the common case
/// the awkward one.
#[test]
fn the_lab_list_offers_fixtures_with_ad_hoc_first() {
    let mut app = App::test_default();
    app.poll_lab_file();

    assert!(
        app.lab
            .available
            .iter()
            .any(|t| matches!(t, LabSource::Fixture(_))),
        "the checked-in fixture labs should be listed: {:?}",
        app.lab
            .available
            .iter()
            .map(LabSource::label)
            .collect::<Vec<_>>(),
    );

    // **A README is not a lab.** `docs/fixture-labs/` gained one on
    // 2026-08-01 under the two-audience convention (`DECISIONS.md`), and the
    // enumeration takes every `.md` in the directory — so without the
    // exclusion in `bridge::fixture_labs` the picker offers a lab called
    // "README" whose stops do not exist. Pinned here because the next
    // directory README would reintroduce it silently.
    let labels: Vec<String> = app.lab.available.iter().map(LabSource::label).collect();
    assert!(
        !labels.iter().any(|l| l.eq_ignore_ascii_case("README")),
        "README.md must not be offered as a lab: {labels:?}",
    );
    if app.lab.available.contains(&LabSource::AdHoc) {
        assert_eq!(app.lab.available[0], LabSource::AdHoc, "ad hoc sorts first");
        assert_eq!(
            app.lab.selected,
            Some(LabSource::AdHoc),
            "and is selected by default"
        );
    }

    // Selecting a fixture drops the previous text immediately rather than leaving
    // it on screen until the next poll.
    let fixture = app
        .lab
        .available
        .iter()
        .find(|t| matches!(t, LabSource::Fixture(_)))
        .cloned()
        .expect("a fixture exists");
    app.select_lab(fixture.clone());
    assert!(app.lab.cached.is_none(), "old text cleared on switch");
    app.lab.polled_at = None;
    app.poll_lab_file();
    assert_eq!(app.lab.selected, Some(fixture));
    assert!(app.lab.cached.is_some(), "the chosen fixture is loaded");
}

/// **The chain overview sorts first in the picker, and the separator follows it.**
///
/// Doug, 2026-08-17: *"I really want to be able to navigate backward from a
/// subordinate lab to the top-level lab so that I can then navigate downward to
/// another subordinate lab."* Two things answer that — a back-link inside each phase
/// lab (checked by `doc_citations::every_lab_the_overview_links_to_links_back`) and
/// the hub sitting at the top of the picker, which is this.
///
/// **Asserted against `available`, not against a literal list**, so a new lab cannot
/// slip in above the overview: the check is *position relative to everything else*.
///
/// **And the non-vacuity guard matters here more than usual.** Alphabetically
/// `the-concepts` already sorts after most labs, so a test that merely looked for
/// it in the list would pass with the hoist deleted. This asserts index 0 *and* that
/// there is something below it to be above.
#[test]
fn the_chain_overview_sorts_first_in_the_picker() {
    let mut app = App::test_default();
    app.poll_lab_file();

    let (ordered, hoisted) = app.lab.picker_order();
    let labels: Vec<String> = ordered.iter().map(|s| s.label()).collect();

    assert_eq!(
        hoisted, 1,
        "exactly one lab is the chain overview; the picker draws its separator at \
             this index, so 0 would silently mean 'no hub on disk' and 2 would mean the \
             predicate matched something else. Order was: {labels:?}",
    );
    assert!(
        ordered[0].is_overview(),
        "the-concepts.md is the hub the nine phase labs hang off and must be the \
             first row in the picker; it was {:?}",
        labels.first(),
    );
    assert!(
        ordered.len() > 1,
        "the hoist is vacuous with nothing beneath it — the corpus should hold every \
             phase lab as well: {labels:?}",
    );
    // The tail keeps the enumeration's own order. Not sorted here, deliberately: the
    // hoist is the only reordering, and asserting more would pin `bridge::fixture_labs`
    // twice.
    assert!(
        ordered[1..].iter().all(|s| !s.is_overview()),
        "the overview appears once, at the top, and not again below the separator: \
             {labels:?}",
    );
}

/// **The lab catalogue is current.**
///
/// `docs/fixture-labs/CATALOGUE.md` is generated by
/// `cargo run -p hrw --example gen_lab_catalogue` and is **written for Claude** —
/// it is how a question gets answered by citing a lab that already demonstrates
/// the thing, instead of by writing a new one that retells it without its checked
/// expectations (`docs/ideas.md` #63).
///
/// **A stale catalogue is worse than none.** It would send Claude to cite a lab
/// about the wrong specimen with full confidence, and #63 already records that
/// citing a lab makes its claims your own. Every field in it is derived, so
/// "stale" only ever means "not regenerated" — which is exactly what this catches.
///
/// Adding, renaming or re-stopping a lab fails here with the command to run.
#[test]
fn lab_catalogue_is_current() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
    let path = dir.join("CATALOGUE.md");
    let Ok(on_disk) = std::fs::read_to_string(&path) else {
        panic!("no CATALOGUE.md — run: cargo run -p hrw --example gen_lab_catalogue");
    };
    // The same function the example calls, not a reimplementation of it: a
    // checker that duplicates what it checks is the drift `fidelity-plan.md`
    // warns about, and is why `catalogue` lives in the library.
    let fresh = crate::lab::catalogue();
    assert_eq!(
        on_disk, fresh,
        "CATALOGUE.md is out of date \u{2014} run: \
             cargo run -p hrw --example gen_lab_catalogue",
    );
}

/// **Every `hrw://lab/…` citation names a lab that exists, at a stop that exists.**
///
/// `fixture_lab_links_all_resolve` checks the *grammar* — `lab/x/station/y` parses
/// whether or not `x` or `y` are real. That is the gap this closes, and it is the
/// gap that makes the slug design worth anything: a **renamed heading fails here
/// loudly**, which is the whole reason stops are addressed by slug rather than by
/// ordinal. An ordinal would still resolve after an insertion and point at the
/// wrong stop, exactly as `worker.rs:3434` did in `tech-debt.md` inside a day.
///
/// Added 2026-08-05 with the link form (`docs/ideas.md` #63).
#[test]
fn lab_citations_name_a_real_lab_and_a_real_stop() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let labs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .collect();

    let mut checked = 0usize;
    for path in &labs {
        let text = std::fs::read_to_string(path).unwrap();
        for raw in text.split("hrw://lab/").skip(1) {
            let cite: String = raw
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '`')
                .collect();
            // **A documented form is not a citation.** `CATALOGUE.md` and any lab
            // explaining the link form write `hrw://lab/<name>/station/<slug>`
            // literally, and the angle brackets are what say "placeholder". Without
            // this the checker demanded a lab called `<name>`.
            if cite.contains('<') {
                continue;
            }
            let mut parts = cite.split('/');
            let name = parts.next().unwrap_or_default();
            let target = dir.join(format!("{name}.md"));
            assert!(
                target.exists(),
                "{} cites lab `{name}`, which does not exist",
                path.display(),
            );
            checked += 1;

            // `stop/<slug>` — the slug must match a heading in the cited lab.
            if parts.next() == Some("station")
                && let Some(slug) = parts.next()
            {
                let cited = std::fs::read_to_string(&target).unwrap();
                let slugs: Vec<String> = crate::autoplay::parse_stations(&cited)
                    .iter()
                    .map(|s| crate::autoplay::station_slug(&s.heading))
                    .collect();
                assert!(
                    slugs.iter().any(|s| s == slug),
                    "{} cites `{name}` stop `{slug}`, which is not a heading there. \
                         Available: {slugs:?}",
                    path.display(),
                );
            }
        }
    }
    // Non-vacuity is NOT asserted: no lab cites another yet. This test exists so
    // that the first one to do so is checked, and it would pass silently on a
    // corpus with no citations at all — which is correct here and would not be if
    // citations were expected. Stated rather than left as an accident.
    let _ = checked;
}

/// Node paths in the **node-pointing** fixture resolve against the real IR.
///
/// `fixture_lab_links_all_resolve` checks only the grammar — a path can parse
/// perfectly and point at nothing. A fixture lab with a made-up path is a broken
/// test that *looks* fine, which is the worst kind, so the paths are checked against
/// the specimen's own trace.
///
/// Station 5 is deliberately unresolvable (it belongs to `CapacitorLoop`, which fails
/// structurally); the lab expects a notice there, so it is excluded by name.
#[test]
fn node_pointing_fixture_paths_exist_in_the_real_ir() {
    let trace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("docs/specimen-notebook/RcCircuit/trace/structural.json");
    let Ok(text) = std::fs::read_to_string(&trace) else {
        return; // trace not generated in this checkout
    };
    let ir: Value = serde_json::from_str(&text).unwrap();

    let lab = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs/node-pointing.md");
    let Ok(md) = std::fs::read_to_string(&lab) else {
        return;
    };

    // The one path the lab expects to fail, by design.
    const DELIBERATELY_ABSENT: &str = "error.unmatched_unknowns[0]";
    let mut checked = 0usize;
    for link in extract_hrw_links(&md) {
        let Some(raw) = link.split("/node/").nth(1) else {
            continue;
        };
        let path = bridge::parse_path(raw).expect("fixture paths must be well-formed");
        if raw == DELIBERATELY_ABSENT {
            assert!(
                bridge::navigate(&ir, &path).is_none(),
                "Station 5 relies on {raw} being absent from RcCircuit; if it now exists \
                     the lab tests nothing",
            );
            continue;
        }
        assert!(
            bridge::navigate(&ir, &path).is_some(),
            "{raw} is in the fixture lab but not in RcCircuit's structural IR",
        );
        checked += 1;
    }
    assert!(
        checked >= 4,
        "expected the fixture's real paths to be checked, saw {checked}"
    );
}

/// A fixture lab's referenced files exist.
///
/// The cross-platform lab points at a Wolfram notebook, and a stop referencing a
/// file that is not there tests nothing while looking fine — the same failure as a
/// made-up node path. Fixture notebooks are therefore **versioned beside their
/// lab**, not written to the gitignored bridge directory: an *ad hoc* notebook is
/// ephemeral like an ad hoc lab, but a fixture has expected outcomes, and a test
/// that vanishes on a fresh checkout is not a test.
#[test]
fn fixture_labs_reference_files_that_exist() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut checked = 0usize;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();

        // Relative markdown links, excluding schemes and anchors.
        for target in text.split("](").skip(1).filter_map(|t| t.split(')').next()) {
            if target.starts_with('#') || target.contains("://") {
                continue;
            }
            assert!(
                dir.join(target).exists(),
                "{} references {target}, which does not exist",
                path.display(),
            );
            checked += 1;
        }

        // And `hrw://notebook/<name>` targets, which are references too — the link
        // parses whatever the name, so grammar alone proves nothing about the file.
        for link in extract_hrw_links(&text) {
            let Some(name) = link.strip_prefix("hrw://notebook/") else {
                continue;
            };
            assert!(
                bridge::resolve_notebook(name).is_some(),
                "{} opens notebook {name}, which does not resolve",
                path.display(),
            );
            checked += 1;
        }

        // And `hrw://src/<path>#<symbol>` targets — **the check that makes the
        // code-grounding agreement pay.** A lab claim naming `generate_equality_equations`
        // is worth more than one about "graphs" precisely because this can refute it, so a
        // symbol renamed out of the source breaks the suite instead of Doug's click.
        for link in extract_hrw_links(&text) {
            let Some(target) = link.strip_prefix("hrw://src/") else {
                continue;
            };
            assert!(
                bridge::resolve_source(target).is_some(),
                "{} cites {target}, which does not resolve — either the path is not a \
                 file under the workspace, or the `#symbol` is no longer defined in it",
                path.display(),
            );
            checked += 1;
        }

        // And `hrw://doc/<name>` targets, for the same reason one file type later.
        // The relative-link clause above cannot cover these: a doc link no longer IS a
        // relative link, so converting one moves it out of that clause's reach — the
        // exact way the notebook conversion did in 2026-07-30, which is what the
        // non-vacuity guard below was added for.
        for link in extract_hrw_links(&text) {
            let Some(name) = link.strip_prefix("hrw://doc/") else {
                continue;
            };
            assert!(
                bridge::resolve_doc(name).is_some(),
                "{} opens document {name}, which does not resolve under hrw/docs/",
                path.display(),
            );
            checked += 1;
        }
    }
    // Non-vacuity. The first version of this test asserted only on relative links,
    // and converting the notebook link to `hrw://notebook/` left it with nothing to
    // check — it failed rather than passing empty, which is the behaviour to keep.
    assert!(
        checked > 0,
        "expected at least one file reference across the fixtures"
    );
}

/// Every `hrw://` link in every fixture lab parses.
///
/// **Enumerates through `bridge::fixture_labs`, not its own `read_dir`.** It had
/// a private copy until 2026-08-01, which is a second definition of "what is a
/// fixture lab" — and the two drifted the moment the directory gained a
/// `README.md`: the app correctly stopped offering it as a lab while this test
/// still scanned it and failed on the bare `hrw://` in its prose. *A check that
/// exists twice is a check that drifts*, which is the same lesson F1 and F7 both
/// produced.
#[test]
fn fixture_lab_links_all_resolve() {
    let mut labs = 0usize;
    let mut links = 0usize;
    for path in bridge::fixture_labs() {
        labs += 1;
        let text = std::fs::read_to_string(&path).unwrap();
        let found = extract_hrw_links(&text);
        assert!(
            !found.is_empty(),
            "a fixture lab with no links tests nothing: {}",
            path.display(),
        );
        for link in found {
            assert!(
                parse_hrw_link(&link).is_some(),
                "unresolvable link in {}: {link}",
                path.display(),
            );
            links += 1;
        }
    }
    assert!(
        labs > 0 && links > 0,
        "expected at least one fixture lab with links"
    );
}

/// **Every link form round-trips: `parse` → `describe` → `parse` yields the same link.**
///
/// # Why this is worth a test, and why it could not be written before
///
/// `HrwLink` is four hand-maintained lists over twelve variants — `parse_hrw_link`
/// (17 arms), `describe` (17), `dispatch_hrw_link` (12) and `requires_specimen`. Nothing
/// held them in correspondence. On 2026-08-22 a column read of that cluster found
/// `LoadAndSwitch` and `LoadSpecimen` missing a guard their sibling in the left panel had
/// carried all along, which is the shape this cluster fails in.
///
/// **The round-trip states the correspondence as a property rather than case by case.**
/// It is what makes the two encodings each other's inverse instead of two functions that
/// happen to agree today — and it catches an asymmetry in *either* direction, which no
/// per-URL test can.
///
/// # What it adds over the test that already existed
///
/// [`a_recorded_link_round_trips_to_the_same_link`] makes the same parity claim over
/// **ten hand-picked literals**, and it would catch the `SeekFrame` break below. **The
/// difference is that its coverage is a fixed sample and this one's is the corpus.** A
/// census on 2026-08-22 counted 70 `LoadAndSwitch`, 14 `PointAtNode`, 9 `SeekFrame` and
/// 5 `AimAtEquation` across the labs — real payloads, including quoted node keys, that
/// no literal list contains. **A lab introducing a new form is covered here the day it
/// lands, and never by a literal list.**
///
/// A synthetic companion for the two forms labs do not use — `SwitchStage` with no
/// sub-view, and `ShowSource` with a line — was written and then **deleted**: both are
/// already in that test's literals, and a duplicate makes one defect fail two tests
/// while saying the same thing twice.
///
/// Unparseable links are skipped deliberately — [`fixture_lab_links_all_resolve`] owns
/// that claim, for the same reason.
///
/// **`SeekFrame` is the case it earns its keep on.** `describe` emits `n + 1` and the
/// parser does `checked_sub(1)`, because links are 1-based to match "Frame 3/11" on
/// screen while the value is 0-based. That off-by-one is invisible to every other check
/// and would round-trip wrongly the moment either side changed alone.
#[test]
fn every_lab_link_round_trips_through_describe() {
    let mut checked = 0usize;
    for path in bridge::fixture_labs() {
        let text = std::fs::read_to_string(&path).expect("readable lab");
        for url in extract_hrw_links(&text) {
            // Resolution is `fixture_lab_links_all_resolve`'s claim, not this one.
            let Some(link) = parse_hrw_link(&url) else {
                continue;
            };
            let described = format!("hrw://{}", link.describe());
            let again = parse_hrw_link(&described);
            assert_eq!(
                again.as_ref(),
                Some(&link),
                "a link did not survive parse -> describe -> parse\n  lab:     {}\n  \
                 original: {url}\n  describe: {described}\n  reparsed: {again:?}",
                path.display(),
            );
            checked += 1;
        }
    }
    // Non-vacuity: passing must mean the forms were exercised, never that the extractor
    // returned nothing.
    assert!(
        checked > 20,
        "only {checked} links round-tripped \u{2014} the corpus or the extractor is broken",
    );
}

/// **Every sub-view a lab link names is AVAILABLE for the specimen it names.**
///
/// Doug, 2026-08-12, walking `connect-expansion.md`: *"Act 2 … contains a link for
/// RcCircuit → Structural → Summary, and that link actually navigates to RcCircuit
/// → Structural → Incidence."*
///
/// **`fixture_lab_links_all_resolve` passed every one of these, and was right to.**
/// It checks the *grammar* — `Structural/Summary` is a real stage and a real
/// sub-view, so it parses. What it cannot know is that **`Summary` exists on the
/// Structural stage only for a singular system**: on `RcCircuit` the app refuses
/// it, says so in the status bar, and leaves the sub-view wherever it was. The
/// reader sees the stage change and the wrong view, with the explanation in a pane
/// they were not told to look at (`fixture-labs/README.md`'s second rule, which
/// this is the second instance of).
///
/// **Six such links existed across three labs; one walk found one of them.**
///
/// # How it checks without compiling
///
/// Singularity comes from the **committed manifest** —
/// `docs/specimen-notebook/<Model>/trace/manifest.json`, whose per-stage `note` is
/// the same string the app reads — and the verdict from
/// `App::structural_view_available_from_stage`, the same function the app calls.
/// Neither is a reimplementation, so neither can drift.
///
/// **`Animate` and `AliasAnim` are skipped, loudly.** Their availability also
/// depends on frames captured at compile time, which a trace cannot settle, so the
/// predicate returns `None` and this test counts them as unchecked rather than
/// assuming they pass. No lab links to either today; if one does, the count below
/// says so instead of the link going silently unverified.
#[test]
fn every_lab_sub_view_link_is_available_for_its_specimen() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0usize;
    let mut skipped_frame_dependent = 0usize;
    let mut skipped_conditional_non_report = 0usize;
    let mut no_manifest: Vec<String> = Vec::new();
    let mut broken: Vec<String> = Vec::new();

    for path in bridge::fixture_labs() {
        let lab = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_owned();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        for link in extract_hrw_links(&text) {
            // Only the form that names a specimen AND a sub-view can be checked
            // here: a bare `hrw://stage/...` carries no specimen, so which model
            // is loaded when it is clicked depends on the walk.
            let Some(HrwLink::LoadAndSwitch(model, stage, Some(sub))) = parse_hrw_link(&link)
            else {
                continue;
            };
            let SubView::Structural(view) = sub else {
                // **NOT "always present" — that is what this comment used to say, and it
                // was false.** Source Map exists only when the equation sheet carries
                // source spans, Connections only when the model has `connect()`
                // statements, `pre() lowering` only when a trace was captured, IC plan
                // only when initialization produced one. `sub_view_rows`' three
                // predicates answer all four — but from *live compile state*, and the
                // committed manifest carries none of it: no source-span flag, no
                // connection-frame count. So these are **skipped loudly**, exactly as
                // `Animate` and `AliasAnim` are, rather than assumed to pass.
                //
                // `Tree` and `EquationSheet` are the two that could be settled from the
                // manifest today, and both are available on any model that compiles at
                // all, so settling them would assert nothing. What would make this real is
                // a manifest field for each conditional tab — filed as a follow-up in
                // `docs/app-split-plan.md`, not built here.
                if !matches!(
                    sub,
                    SubView::Flatten(FlattenView::Tree)
                        | SubView::Flatten(FlattenView::Equations)
                        | SubView::Events(EventsView::Tree)
                        | SubView::Init(InitView::Tree)
                ) {
                    skipped_conditional_non_report += 1;
                }
                continue;
            };

            let manifest = root
                .join("docs/specimen-notebook")
                .join(&model)
                .join("trace/manifest.json");
            let Ok(raw) = std::fs::read_to_string(&manifest) else {
                no_manifest.push(format!("{lab}: {link} (no trace for {model})"));
                continue;
            };
            let json: serde_json::Value = match serde_json::from_str(&raw) {
                Ok(v) => v,
                Err(e) => {
                    no_manifest.push(format!("{lab}: {link} (unreadable manifest: {e})"));
                    continue;
                }
            };
            // The manifest keys stages by the trace's own snake_case names.
            let key = match stage {
                StageKind::Structural => "structural",
                StageKind::IndexReduction => "index_reduction",
                _ => continue,
            };
            let note = json["stages"][key]["note"].as_str();
            let is_singular = App::note_says_singular(note);
            let is_index_reduction = stage == StageKind::IndexReduction;

            match App::structural_view_available_from_stage(view, is_index_reduction, is_singular) {
                None => skipped_frame_dependent += 1,
                Some(true) => checked += 1,
                Some(false) => {
                    checked += 1;
                    broken.push(format!(
                        "{lab}: {link}\n      {model}'s {} note is {} \u{2014} so \
                             {} is not offered there, and the click will land on whichever \
                             sub-view was already showing",
                        key,
                        note.map_or_else(
                            || "absent (not singular)".to_owned(),
                            |n| format!("{:?}", n.chars().take(60).collect::<String>())
                        ),
                        structural_view_name(view),
                    ));
                }
            }
        }
    }

    assert!(
        no_manifest.is_empty(),
        "a lab names a specimen with no committed trace, so its links cannot be \
             checked at all:\n  {}",
        no_manifest.join("\n  "),
    );
    // **Non-vacuity.** Six of these links were broken when the test was written,
    // so a run that checks none of them has stopped working rather than found
    // nothing to complain about.
    assert!(
        checked >= 10,
        "only {checked} sub-view links were checked ({skipped_frame_dependent} skipped as \
             frame-dependent) \u{2014} the extraction is broken, not the labs",
    );
    assert!(
        broken.is_empty(),
        "{} lab link(s) name a sub-view the specimen does not offer:\n  {}",
        broken.len(),
        broken.join("\n  "),
    );
    // **The gap, counted rather than assumed away.** These are the Flatten/Events/Init
    // links whose availability depends on live compile state a committed trace does not
    // carry — the same treatment `Animate` and `AliasAnim` get above. It was an unchecked
    // `continue` under a comment claiming they are "always present" until 2026-08-21.
    //
    // A bound rather than an equality: a new lab link of this shape should not fail a
    // test about the checker, but a *sweep* of them means the blind spot has grown enough
    // to be worth the manifest field. `RcCircuit/Flatten/Connections` is the one today.
    assert!(
        skipped_conditional_non_report <= 4,
        "{skipped_conditional_non_report} lab links name a conditional non-report \
         sub-view, and nothing checks any of them \u{2014} the manifest needs a field per \
         conditional tab before this grows further (docs/app-split-plan.md)",
    );
}

/// **Every link that names a sub-view defers it, rather than applying it.**
///
/// This is the bug Doug found by clicking the fixture lab in order: Station 5's
/// `hrw://stage/IndexReduction/Animate/frame/2` showed the Index Reduction
/// *Summary* the first time, and the replay only on a second click.
///
/// Cause: the centre panel resets the sub-view whenever a report stage is entered
/// — forcing `Summary` for Index Reduction — and that reset runs *after* link
/// dispatch. A sub-view applied during dispatch is therefore overwritten. The
/// second click works because the stage no longer changes, so the reset is skipped.
///
/// `pending_sub_view` exists precisely to survive that reset, and `LoadAndSwitch`
/// already used it. The three sibling verbs did not. **The symptom to remember is
/// "works on the second click" — it almost always means set-then-overwritten.**
#[test]
fn every_sub_view_link_defers_through_pending_sub_view() {
    let animate = SubView::Structural(StructuralView::Animate);

    for (label, link) in [
        (
            "switch",
            HrwLink::SwitchStage(StageKind::IndexReduction, Some(animate)),
        ),
        (
            "seek",
            HrwLink::SeekFrame(StageKind::IndexReduction, animate, 2),
        ),
        (
            "aim",
            HrwLink::AimAtEquation(StageKind::IndexReduction, animate, 0),
        ),
    ] {
        let mut app = App::test_default();
        app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
        // A sub-view the reset would clobber, so the test cannot pass by accident.
        app.viewport.structural = StructuralView::SpyPlot;
        app.dispatch_hrw_link(link);

        assert_eq!(
            app.stage,
            StageKind::IndexReduction,
            "{label}: stage switched"
        );
        assert_eq!(
            app.pending_sub_view,
            Some(animate),
            "{label}: the sub-view must be DEFERRED so the stage-entry reset cannot \
                 overwrite it",
        );
        assert_eq!(
            app.viewport.structural,
            StructuralView::SpyPlot,
            "{label}: and must NOT be applied during dispatch",
        );
    }
}

/// A seek aimed at a view with no animation gives up instead of lingering armed.
///
/// Without a budget it would sit pending until the reader wandered into an animated
/// view and then fire there — a link taking effect somewhere it was never pointed.
/// Station 6 of the frame-seeking fixture is exactly this case.
#[test]
fn a_seek_that_never_lands_expires() {
    let mut app = App::test_default();
    app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
    // Incidence has no animation, ever.
    app.dispatch_hrw_link(HrwLink::SeekFrame(
        StageKind::Structural,
        SubView::Structural(StructuralView::Incidence),
        3,
    ));
    assert!(app.seek_frame.is_some(), "armed on dispatch");

    for _ in 0..SEEK_ATTEMPTS {
        app.apply_pending_seek();
    }
    assert!(
        app.seek_frame.is_none(),
        "the seek must expire rather than stay armed for a later view",
    );
}

/// The compile outcome names the first failing stage, or how far it got.
///
/// Doug found the gap by reloading a lab and asking what the trail said an hour
/// later: it still read `compiling: true, model: null`, because the trail ended at
/// "specimen sent to the worker" and nothing recorded the finish. The app block was
/// accurate for the last *action* and increasingly wrong about *now*.
///
/// The **first** failing stage is what gets reported, because everything after it
/// says "not reached" and carries no information.
#[test]
fn the_compile_outcome_names_the_first_failure() {
    let ok = Stage::ok(serde_json::json!({}));
    // `err_with_details` = Outcome::Failed: the value holds the error payload,
    // not IR, so nothing downstream can consume it.
    let failed = Stage::err_with_details(serde_json::json!({}), "boom");

    // A clean run reports how far it reached.
    let mut app = App::test_default();
    app.model = Some("RcCircuit".to_owned());
    app.stages = StageBundle {
        parse: ok.clone(),
        resolve: ok.clone(),
        instantiate: ok.clone(),
        typecheck: ok.clone(),
        flatten: ok.clone(),
        dae: ok.clone(),
        structural: ok.clone(),
        index_reduction: ok.clone(),
        initialization: ok.clone(),
        events: ok.clone(),
        solve_lowering: ok.clone(),
    };
    let outcome = app.compile_outcome();
    assert!(outcome.starts_with("RcCircuit: reached "), "{outcome}");

    // A failure names the stage, and names the FIRST one.
    let mut app = App::test_default();
    app.model = Some("UnbalancedShaft".to_owned());
    app.stages = StageBundle {
        parse: ok.clone(),
        resolve: ok.clone(),
        instantiate: ok.clone(),
        typecheck: ok.clone(),
        flatten: failed.clone(),
        // A later stage also "fails" (not reached); it must not be the one named.
        structural: failed.clone(),
        ..StageBundle::default()
    };
    let outcome = app.compile_outcome();
    assert_eq!(
        outcome, "UnbalancedShaft: FAILED at Flatten",
        "the first failure is the diagnostic; later stages are just not reached",
    );
}

/// A link's trail entry is the link, so the trail can be read against the lab.
///
/// Doug asked whether Claude can see him click a lab link. It could not — the
/// action trail showed the specimen load and nothing after, so a report of "several
/// bugs in the node-pointing lab" had to be reconstructed by asking. Now every
/// followed link is recorded, and recorded as its **canonical URL** rather than a
/// `Debug` dump, so it lines up with the lab's own text at a glance.
///
/// Round-tripped rather than pinned to literals: `describe` and `parse_hrw_link`
/// must agree, which is the same parity rule as everywhere else.
#[test]
fn a_recorded_link_round_trips_to_the_same_link() {
    for url in [
        "hrw://load/CapacitorLoop",
        "hrw://stage/Structural",
        "hrw://stage/Structural/Incidence",
        "hrw://load/RcCircuit/Structural/Tree",
        "hrw://source",
        "hrw://source/9",
        "hrw://stage/Structural/TarjanAnim/equation/13",
        "hrw://stage/Structural/MatchingAnim/frame/41",
        "hrw://stage/Structural/Tree/node/incidence.rows[0].equation_text",
        "hrw://follow/C.v",
    ] {
        let link = parse_hrw_link(url).unwrap_or_else(|| panic!("{url} should parse"));
        assert_eq!(
            format!("hrw://{}", link.describe()),
            url,
            "a recorded link must read back as the link that was clicked",
        );
    }
}

/// `SubView::slug` is `from_slug`'s inverse, for every variant.
///
/// The missing-inverse gap again: `from_slug` existed alone, which is exactly how
/// the stage vocabulary drifted into four copies. `slug` dispatches to the same
/// functions the capture uses, so the two vocabularies are equal by construction.
#[test]
fn every_sub_view_slug_round_trips() {
    let cases: Vec<(StageKind, SubView)> = StructuralView::ALL
        .iter()
        .map(|v| (StageKind::Structural, SubView::Structural(*v)))
        .chain(
            FlattenView::ALL
                .iter()
                .map(|v| (StageKind::Flatten, SubView::Flatten(*v))),
        )
        .chain(
            EventsView::ALL
                .iter()
                .map(|v| (StageKind::Events, SubView::Events(*v))),
        )
        .chain(
            InitView::ALL
                .iter()
                .map(|v| (StageKind::Initialization, SubView::Init(*v))),
        )
        .collect();

    for (stage, sub) in cases {
        assert_eq!(
            SubView::from_slug(stage, sub.slug()),
            Some(sub),
            "{sub:?} writes slug {:?} which does not parse back under {stage:?}",
            sub.slug(),
        );
    }
}

/// A node link marks the row it pointed at, and the mark outlives the scroll.
///
/// Doug walked the node-pointing fixture and reported the node was not highlighted.
/// He was right twice over: the lab asserted a highlight, and **there was none** —
/// `scroll_if_jump_target` only ever scrolled. The lab was right about what should
/// happen, though: a row scrolled to the centre of a screen of near-identical rows,
/// unmarked, leaves the reader guessing which one was meant.
///
/// `jump_target` lasts exactly one frame, so highlighting on that alone would flash
/// for 16ms. `jump_highlight` persists until Doug does something of his own.
#[test]
fn a_node_link_marks_the_row_until_doug_moves_on() {
    let path = bridge::parse_path("incidence.rows[0].equation_text").expect("well-formed");

    let mut app = App::test_default();
    // These verbs now require a loaded specimen — clicking a stop out of order is
    // refused rather than half-applied. Give it one so dispatch proceeds.
    app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
    app.dispatch_hrw_link(HrwLink::PointAtNode(
        StageKind::Structural,
        Some(SubView::Structural(StructuralView::Tree)),
        path.clone(),
    ));
    assert_eq!(
        app.context.jump_target.as_ref(),
        Some(&path),
        "scrolls to it"
    );
    assert_eq!(
        app.context.jump_highlight.as_ref(),
        Some(&path),
        "and marks it"
    );

    // The scroll is consumed after one frame; the mark is not.
    app.context.jump_target = None;
    assert_eq!(
        app.context.jump_highlight.as_ref(),
        Some(&path),
        "the mark must outlive the one-frame scroll, or it flashes and tells nobody",
    );

    // A point of Doug's own answers a different question, so the mark goes.
    app.emit_node_focus(vec![Seg::Key("blocks".into())], bridge::AskRequest::Explain);
    assert!(
        app.context.jump_highlight.is_none(),
        "Doug pointing at something supersedes the link's mark",
    );
}

/// A link can point at a node, using the capture's own spelling of the path.
///
/// This closes the last parity gap: `Focus::Node` is the capture's richest noun and
/// no link could express it. The path grammar is not re-stated here — that is
/// round-tripped in `bridge::tests` — this checks the link layer consumes it.
#[test]
fn a_link_can_point_at_a_node() {
    let Some(HrwLink::PointAtNode(stage, sub, path)) =
        parse_hrw_link("hrw://stage/Structural/Tree/node/error.unmatched_unknowns[0]")
    else {
        panic!("should parse");
    };
    assert_eq!(stage, StageKind::Structural);
    assert_eq!(sub, Some(SubView::Structural(StructuralView::Tree)));
    assert_eq!(bridge::describe_path(&path), "error.unmatched_unknowns[0]");

    // The tree root is a legitimate target.
    assert!(matches!(
        parse_hrw_link("hrw://stage/Flatten/Tree/node/"),
        Some(HrwLink::PointAtNode(StageKind::Flatten, _, _)),
    ));
    // A malformed path fails the whole link rather than pointing somewhere near.
    assert!(parse_hrw_link("hrw://stage/Structural/Tree/node/a..b").is_none());
}

/// **A frame link built by `frame_link` seeks the frame it names.**
///
/// Binds the formatter to the parser so the 0-based/1-based seam has exactly one
/// crossing. `examples/frame_index` printed 0-based indices and told the author
/// they worked verbatim in `hrw://…/frame/<n>`; the parser subtracts one, so
/// every link written from that output pointed one step early. **Nothing could
/// have caught it** — the link parses, resolves, and lands on a real frame that
/// is simply the wrong one, which is the whole failure mode `frame_index` was
/// built to remove.
#[test]
fn a_frame_link_round_trips_through_the_parser() {
    for index in [0usize, 1, 6, 15, 40] {
        let uri = frame_link("Structural", "MatchingAnim", index);
        assert_eq!(
            parse_hrw_link(&uri),
            Some(HrwLink::SeekFrame(
                StageKind::Structural,
                SubView::Structural(StructuralView::MatchingAnim),
                index,
            )),
            "{uri} must seek frame {index}, not its neighbour",
        );
    }
    // Frame 0 is reachable — as `frame/1`. The `checked_sub` rejects `frame/0`,
    // which under 1-based numbering names no frame at all.
    assert_eq!(
        frame_link("Structural", "MatchingAnim", 0),
        "hrw://stage/Structural/MatchingAnim/frame/1"
    );
}

/// **A link into a NON-report stage actually changes the sub-view.**
///
/// Doug, 2026-08-16, after a first fix that addressed a different bug: *"Clicking
/// on the frame 7 and frame 13 links is still not causing navigation."*
///
/// `pending_sub_view` was applied inside `report_sub_view_row_ui`, which runs only
/// when the stage is **Structural or Index Reduction** and its report exists. For
/// Flatten, Events and Initialization the pending sub-view was set by the link and
/// **never taken**, and `apply_pending_seek` never ran — so the link was discarded
/// in silence, the seek budget expired over five paints, and nothing said why.
///
/// The whole class hid behind the default: `Flatten/EquationSheet` is where Flatten
/// already opens, so links to it appeared to work. `Flatten/Connections` never did.
///
/// **This test alone is NOT sufficient, and saying so is the point.** It drives
/// `apply_pending_view_and_seek` directly, so it proves the method works — not
/// that anything calls it for a non-report stage, which is precisely what was
/// broken. Verified: re-gating the call site behind `report_ready` leaves this
/// test **green**.
///
/// `ui_tests::a_frame_link_into_flatten_connections_navigates` covers the call
/// site by painting. Both are kept because they fail for different reasons: this
/// one localises a regression in the method, that one catches the method being
/// bypassed.
#[test]
fn a_link_into_a_non_report_stage_applies_its_sub_view() {
    for (stage, sub, expected) in [
        (
            StageKind::Flatten,
            SubView::Flatten(FlattenView::Connections),
            FlattenView::Connections,
        ),
        (
            StageKind::Flatten,
            SubView::Flatten(FlattenView::SourceMap),
            FlattenView::SourceMap,
        ),
    ] {
        let (mut app, _tx) = App::test_with_sender();
        app.stage = stage;
        app.viewport.flatten = FlattenView::Equations;
        // **The compile must have produced these tabs, and until 2026-08-21 it did not
        // have to.** The link guard was `_ => true` for every non-report sub-view, so this
        // test passed against an `App` with no sheet and no connection frames — asserting
        // that a link is *applied* while the app was, separately, unable to refuse one that
        // should not be. Both halves are now real: `give_flatten_every_tab` is what makes
        // the link legitimate, and `a_link_to_a_flatten_sub_view_this_model_lacks_is_refused`
        // is the other side of the same predicate.
        give_flatten_every_tab(&mut app);
        app.pending_sub_view = Some(sub);

        app.apply_pending_view_and_seek();

        assert_eq!(
            app.viewport.flatten, expected,
            "a link naming {:?} left the viewport on {:?} \u{2014} Flatten is not a \
                 report stage, so this used to be dropped in silence",
            sub, app.viewport.flatten,
        );
        assert!(
            app.pending_sub_view.is_none(),
            "the request must be consumed, or it re-applies every frame",
        );
    }
}

/// **A frame link into Flatten actually navigates, through a real paint.**
///
/// Doug, 2026-08-16, on a fix that addressed a different bug: *"Clicking on the frame
/// 7 and frame 13 links is still not causing navigation."*
///
/// `pending_sub_view` was consumed inside `report_sub_view_row_ui`, which runs only
/// for **Structural** and **Index Reduction**. For Flatten, Events and Initialization
/// the request was set and never taken, and the frame seek never ran — silently, since
/// an expired seek budget is the normal end of a seek whose animation is still
/// building.
///
/// # Why this has to paint
///
/// The unit test beside `apply_pending_view_and_seek` drives that method directly and
/// **stays green when the call site is re-gated behind `report_ready`** — measured, not
/// assumed. The defect was never in the method; it was in which code path reaches it.
/// A test that cannot see the call site cannot see this bug, which is the same
/// wrong-level mistake `CLAUDE.md` records for the first scroll-area defect.
///
/// So this drives `frame_ui` and asserts the viewport moved.
#[test]
fn a_frame_link_into_flatten_connections_navigates() {
    use crate::ui_tests::{AdHocLab, harness};

    // **No ad hoc lab for the duration.** HRW auto-selects one when nothing else
    // is chosen, and selecting a lab resets the stage side — so this test passed
    // or failed depending on whether Claude had answered a question recently. It
    // started failing the first time one existed, which is the environment
    // changing rather than the code.
    let _lab_state = AdHocLab::absent();

    let mut app = App::test_default();
    // A specimen must be selected or the link is refused by design — the "no specimen
    // loaded" guard Doug saw when he clicked the link first.
    app.selected = Some(std::path::PathBuf::from("specimens/RcCircuit.mo"));
    app.stage = StageKind::Flatten;
    app.viewport.flatten = FlattenView::Equations;
    // Since 2026-08-21 the link guard asks whether this model has a Connections tab, so
    // the fixture must give it one. `RcCircuit` really does — four `connect()` statements
    // — which is why this test names it.
    give_flatten_every_tab(&mut app);

    app.dispatch_hrw_link(HrwLink::SeekFrame(
        StageKind::Flatten,
        SubView::Flatten(FlattenView::Connections),
        6,
    ));
    assert_eq!(
        app.pending_sub_view,
        Some(SubView::Flatten(FlattenView::Connections)),
        "precondition: the link records the request",
    );

    let mut h = harness(app);
    h.run_steps(2);

    assert_eq!(
        h.state().viewport.flatten,
        FlattenView::Connections,
        "painting must apply the sub-view a link asked for; Flatten is not a report \
             stage, and this used to be dropped without a notice",
    );
    assert!(
        h.state().pending_sub_view.is_none(),
        "the request must be consumed, or it re-applies on every frame",
    );
}

/// The same defect for **Events** and **Initialization**, the other two stages
/// whose sub-views were unreachable by link.
///
/// Each is given the thing its tab needs — a captured `pre()`-lowering trace, an
/// initial-condition plan — because since 2026-08-21 the link guard asks. Without them the
/// link is refused, which is the subject of
/// `a_link_to_a_flatten_sub_view_this_model_lacks_is_refused`'s siblings below.
#[test]
fn links_into_events_and_initialization_apply_their_sub_views() {
    let (mut app, _tx) = App::test_with_sender();
    app.stage = StageKind::Events;
    app.viewport.events = EventsView::Tree;
    give_pre_lowering_trace(&mut app);
    app.pending_sub_view = Some(SubView::Events(EventsView::PreLowering));
    app.apply_pending_view_and_seek();
    assert_eq!(app.viewport.events, EventsView::PreLowering);

    let (mut app, _tx) = App::test_with_sender();
    app.stage = StageKind::Initialization;
    app.viewport.init = InitView::Tree;
    give_ic_plan(&mut app);
    app.pending_sub_view = Some(SubView::Init(InitView::IcPlan));
    app.apply_pending_view_and_seek();
    assert_eq!(app.viewport.init, InitView::IcPlan);
    assert!(parse_hrw_link("hrw://stage/Structural/MatchingAnim/frame/0").is_none());
}

/// Give `app` a Flatten compile that offers **every** tab: a sheet, source spans, and a
/// connection frame.
///
/// One helper because three tests need the same precondition, and the interesting failure
/// is a test that forgets one of the three and reads as though the guard is broken.
fn give_flatten_every_tab(app: &mut App) {
    app.cached_equation_sheet = Some(crate::equation_sheet::EquationSheet {
        source_lines: vec![crate::equation_sheet::SourceLine {
            line_number: 1,
            text: "Real x;".to_owned(),
            equation_indices: Vec::new(),
            category: None,
        }],
        ..Default::default()
    });
    app.frames.connection = vec![rumoca_phase_flatten::connections::trace::ConnectionFrame {
        step: rumoca_phase_flatten::connections::trace::ConnectionStep::Start {
            connect_statements: 1,
        },
        sets_so_far: 0,
        equations_so_far: 0,
    }];
}

/// Give `app` a captured `pre()`-lowering trace, so the Events row has its tab.
fn give_pre_lowering_trace(app: &mut App) {
    app.frames.pre_lowering = vec![rumoca_phase_dae::PreLoweringFrame {
        step: rumoca_phase_dae::PreLoweringStep::Start {
            pass: 1,
            equations: 1,
        },
        slots_so_far: Vec::new(),
    }];
}

/// Give `app` an initial-condition plan, so the Initialization row has its tab.
///
/// Written as the JSON `App::has_ic_plan` reads rather than as a typed report, for the
/// reason that method's doc gives: the pane's question is about what the *report* carries.
fn give_ic_plan(app: &mut App) {
    app.stages.initialization.value = Some(serde_json::json!({
        "blocks": [{ "kind": "assignment", "unknowns": ["x"] }]
    }));
}

/// **A link naming a Flatten sub-view this model does not have is refused, not applied.**
///
/// Doug, 2026-08-12, on the report stages: *"Act 2 … contains a link for RcCircuit →
/// Structural → Summary, and that link actually navigates to RcCircuit → Structural →
/// Incidence."* `App::structural_view_available` was built to close that, and the guard
/// that consults it read `SubView::Structural(v) => …, _ => true` — so **the same defect
/// stayed open for Flatten, Events and Initialization for nine days.** Source Map exists
/// only when the sheet carries source spans; Connections only when the model has
/// `connect()` statements.
///
/// The symptom is the one Doug described: the stage changes, the sub-view does not, and
/// the explanation is in a pane he was not told to look at.
///
/// **`Tree` is not refused** and must not be — it is what the stage falls back to, so a
/// link naming it always shows what it names. Same rule as `StructuralView::Tree`.
#[test]
fn a_link_to_a_flatten_sub_view_this_model_lacks_is_refused() {
    for (missing, sub) in [
        ("source map", SubView::Flatten(FlattenView::SourceMap)),
        ("connections", SubView::Flatten(FlattenView::Connections)),
    ] {
        let (mut app, _tx) = App::test_with_sender();
        app.stage = StageKind::Flatten;
        // A sheet, so the row draws — but neither of the two conditional tabs.
        app.cached_equation_sheet = Some(crate::equation_sheet::EquationSheet::default());
        app.viewport.flatten = FlattenView::Equations;
        app.pending_sub_view = Some(sub);

        app.apply_pending_view_and_seek();

        assert_eq!(
            app.viewport.flatten,
            FlattenView::Equations,
            "a link naming a {missing} view this model has no tab for must be refused, \
             not applied to a pane that will report its own absence",
        );
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|n| n.contains("not here")),
            "and the refusal must be said out loud: {:?}",
            app.notice,
        );
    }

    // The tree is always reachable, so a link naming it is honoured on a model with
    // nothing else — the clause that keeps the guard from being "refuse when empty".
    let (mut app, _tx) = App::test_with_sender();
    app.stage = StageKind::Flatten;
    app.cached_equation_sheet = Some(crate::equation_sheet::EquationSheet::default());
    app.viewport.flatten = FlattenView::Equations;
    app.pending_sub_view = Some(SubView::Flatten(FlattenView::Tree));
    app.apply_pending_view_and_seek();
    assert_eq!(
        app.viewport.flatten,
        FlattenView::Tree,
        "Tree is what the stage falls back to, so it is available on every model",
    );
}

/// **A sub-view link that arrives while the app is on another stage is passed through, not
/// refused** — because availability is a question about the stage on screen.
///
/// `HrwLink::LoadAndSwitch` sets `pending_sub_view` and leaves the stage change to the
/// compile landing, so for the duration of the compile the request names Flatten while
/// `self.stage` is still whatever the reader was looking at. Asking "does *this model's*
/// Flatten offer Connections?" then is asking about an empty compile, and the honest answer
/// is that there is nothing to report: `apply_sub_view` drops a stage-mismatched request
/// anyway.
///
/// **Without this the guard would notice-and-drop a live lab link** —
/// `hrw://load/RcCircuit/Flatten/Connections` in `connect-expansion.md` — on every walk,
/// with a message naming a stage the reader was not on. Found by reasoning about the
/// ordering before writing the guard, not by a walk.
#[test]
fn a_sub_view_link_for_another_stage_is_not_refused() {
    let (mut app, _tx) = App::test_with_sender();
    // The reader is still on Structural; the link names Flatten, as during a load.
    app.stage = StageKind::Structural;
    app.pending_sub_view = Some(SubView::Flatten(FlattenView::Connections));

    app.apply_pending_view_and_seek();

    assert!(
        app.notice.is_none(),
        "a link for a stage the app is not on must produce no complaint: {:?}",
        app.notice,
    );
    assert_eq!(
        app.viewport.flatten,
        FlattenView::Equations,
        "and it must not be applied either \u{2014} `apply_sub_view` matches the stage",
    );
}

/// **A self-running walk puts the mode back when it ends.**
///
/// Doug, 2026-08-03: *"at the completion of the lab, the mode is being switched
/// from lab mode to specimen mode."*
///
/// The stop that does it is not wrong. `hrw://source/<line>` *must* switch to
/// Specimen mode, because that is the only place the source renders, and a reader
/// clicking it wants to be taken there. But `matching.md` ends Station 3 with one, so
/// an unattended run finished with the lab nowhere on screen — and the last two
/// stops played to nobody.
///
/// **A walk is a round trip.** Only the mode is restored: the stage and the
/// specimen are the *result* of the walk and worth keeping.
#[test]
fn a_finished_walk_returns_to_the_mode_it_started_in() {
    let mut app = App::test_default();
    app.ui_mode = UiMode::Lab;
    app.selected = Some(PathBuf::from("/x/RcCircuit.mo"));
    app.lab.mode_before_autoplay = Some(UiMode::Lab);

    app.dispatch_hrw_link(HrwLink::ShowSource(Some(9)));
    assert_eq!(
        app.ui_mode,
        UiMode::Specimen,
        "precondition: a source stop legitimately leaves Lab mode",
    );

    app.restore_mode_after_autoplay();
    assert_eq!(app.ui_mode, UiMode::Lab, "the walk must put the mode back");
    assert!(
        app.lab.mode_before_autoplay.is_none(),
        "and consume the record"
    );

    // Idempotent: a second call (Stop after Finished) must not fight the user.
    app.ui_mode = UiMode::Specimen;
    app.restore_mode_after_autoplay();
    assert_eq!(
        app.ui_mode,
        UiMode::Specimen,
        "with nothing recorded there is nothing to restore, and a stray call must \
             not drag the user out of the mode they chose",
    );
}

/// **A new run does not scroll back from where the last one stopped.**
///
/// Doug's sequence, 2026-08-03: watch `matching` to the middle, Stop, select
/// `frame-seeking`, select `matching` again, press Play — and *"the matching lab
/// rescrolls very visibly from the stopped position back up to the top before the
/// lab begins playing."*
///
/// **The pane was believed correct at the time, and it was not** — re-selecting a
/// lab did *not* put it at the top, which Doug reported on 2026-08-17 and
/// `switching_labs_asks_the_pane_to_return_to_the_top` now holds. The sentence that
/// stood here ("the pane itself was correct") is corrected rather than deleted,
/// because believing it is what kept the search inside HRW's own bookkeeping and
/// away from the `ScrollArea` that actually holds the offset.
///
/// **The bookkeeping was also wrong, and this test is about that half.**
/// `lab_link_y` and `lab_prev_link_y` are pixel positions
/// measured in one document at one beat, and nothing cleared them — so the first
/// frame of the new run interpolated *from* the stopped position and travelled
/// back over the full window.
///
/// Cleared at both boundaries: selecting a lab, and starting a run. Either alone
/// would have fixed Doug's sequence; both are needed because a run can also be
/// restarted on the *same* lab without a selection change.
#[test]
fn starting_a_walk_forgets_where_the_last_one_stopped() {
    let mut app = App::test_default();

    // Stand in for a run stopped half way down a lab.
    app.lab.lab_link_y = Some(4_000.0);
    app.lab.lab_prev_link_y = Some(3_800.0);
    app.lab.lab_measured_beat = Some(12);

    // Boundary 1: choosing a lab. Positions from another document are not
    // merely stale, they are measured against a different length of text.
    app.reset_for_new_lab();
    assert_eq!(
        app.lab.lab_link_y, None,
        "a new lab forgets the old positions"
    );
    assert_eq!(app.lab.lab_prev_link_y, None);
    assert_eq!(app.lab.lab_measured_beat, None);

    // Boundary 2: pressing Play, which also covers replaying the *same* lab
    // with no selection change in between — the case boundary 1 cannot see.
    //
    // The staging order matters. `test_select_fixture_lab` routes through
    // `select_lab`, which itself resets — so setting the stale values before it
    // would let this assertion pass on boundary 1's work and prove nothing about
    // Play at all.
    assert!(
        app.test_select_fixture_lab("matching"),
        "the fixture must be readable, or Play below does nothing",
    );
    app.lab.lab_link_y = Some(4_000.0);
    app.lab.lab_prev_link_y = Some(3_800.0);
    app.test_start_autoplay();
    assert_eq!(
        app.lab.lab_link_y, None,
        "pressing Play must start from the pane's own position; interpolating \
             from the last run's makes the text scroll backwards before it begins",
    );
    assert_eq!(app.lab.lab_prev_link_y, None);

    // Non-vacuity: the run really did start, so this is not passing because
    // nothing happened.
    assert_eq!(app.test_autoplay_phase(), crate::autoplay::Phase::Playing);
}

/// **A `stop/<slug>` link records where that stop actually begins.**
///
/// The first half of the citation feature: `hrw://lab/<name>/station/<slug>` exists so
/// an answer can send Doug to *a stop* rather than to a lab he then has to scan.
///
/// **It was written and never read for as long as it existed.** The handler set
/// `scroll_to_offset` and no frame ever consumed it, so a stop link opened the lab
/// and landed wherever the pane happened to be. Two things hid it: the corpus holds
/// exactly **one** such link, and the symptom is indistinguishable from the
/// stale-scroll bug fixed the same day — both look like *"it opened in the wrong
/// place"*.
///
/// This asserts the offset **points at the heading it names**, not merely that it is
/// `Some`. An offset that resolves to the wrong place would scroll confidently to the
/// wrong stop, which is worse than not scrolling at all.
#[test]
fn a_stop_link_records_where_that_stop_begins() {
    let mut app = App::test_default();

    // The corpus's only stop link, from `failure-resolve.md`.
    app.dispatch_hrw_link(HrwLink::OpenLab {
        lab: "failure-parse".to_owned(),
        stop: Some("station-4-the-distinction-this-specimen-anchors".to_owned()),
    });

    let offset = app
        .lab
        .scroll_to_offset
        .expect("a stop link must record where to land");
    let text = app.lab.text().expect("the lab must be loaded");
    assert!(
        text[offset..].starts_with("## Station 4"),
        "the offset must name the heading the slug asked for, or the pane scrolls \
             confidently to the wrong stop. It landed on: {:?}",
        &text[offset..(offset + 40).min(text.len())],
    );
    assert!(
        app.notice.is_none(),
        "a stop that resolves must not also report a problem: {:?}",
        app.notice,
    );
}

/// **A stop that is gone says so, and does not silently land at the top.**
///
/// The handler's own message — *"opened at the top"* — was a promise the code did not
/// keep until 2026-08-17, since nothing scrolled anywhere. Now that a stop link does
/// move the pane, the failure branch matters more, not less: *"it opened at the top"*
/// and *"it opened at the stop I asked for"* have to be distinguishable, or a renamed
/// heading reads as a lab whose first stop is the one you wanted.
#[test]
fn a_stop_link_naming_nothing_reports_it_rather_than_landing_anywhere() {
    let mut app = App::test_default();

    app.dispatch_hrw_link(HrwLink::OpenLab {
        lab: "failure-parse".to_owned(),
        stop: Some("no-such-stop-anywhere".to_owned()),
    });

    assert!(
        app.lab.scroll_to_offset.is_none(),
        "an unresolved stop must record no destination",
    );
    let notice = app.notice.as_deref().unwrap_or_default();
    assert!(
        notice.contains("no-such-stop-anywhere") && notice.contains("opened at the top"),
        "the reader must be told which stop is missing and where they ended up \
             instead; got {notice:?}",
    );
    // Non-vacuity: the lab itself did open, so this is the *stop* failing rather
    // than the whole link.
    assert!(
        app.lab.text().is_some(),
        "the lab still opens — only the stop within it was not found",
    );
}

/// **Switching labs requests a return to the top.**
///
/// The state half of Doug's 2026-08-17 report; the paint half — that a rendering
/// frame actually spends the request — is
/// `ui_tests::switching_labs_asks_the_pane_to_return_to_the_top`. **Both are
/// needed, and this one alone would have been the wrong test**: the bug lived
/// precisely in the gap, where `reset_scroll` diligently cleared three fields and
/// none of them positions the view.
#[test]
fn switching_labs_requests_a_return_to_the_top() {
    let mut app = App::test_default();

    // A switch that really happens. `test_select_fixture_lab` routes through
    // `select_lab`, so this exercises the same path the picker and an
    // `hrw://lab/…` link both take.
    assert!(
        app.test_select_fixture_lab("node-pointing"),
        "the fixture must be readable, or the switch below does nothing",
    );
    assert!(
        app.lab.scroll_to_top,
        "arriving at a lab must ask the pane to start at the beginning",
    );

    // Spent by a paint, which this test does not do — so clear it by hand and
    // confirm a *second* switch asks again. A one-shot that only ever fires once
    // would fix the first navigation of a session and no other.
    app.lab.scroll_to_top = false;
    assert!(
        app.test_select_fixture_lab("camera-aiming"),
        "the second fixture must be readable too",
    );
    assert!(
        app.lab.scroll_to_top,
        "every switch asks, not just the first",
    );

    // **Re-selecting the lab already showing must NOT ask.** `select_lab` returns
    // false there deliberately, to keep a reader's place in a lab they are partway
    // through — and yanking them to the top would be the same defect wearing the
    // opposite sign.
    app.lab.scroll_to_top = false;
    app.test_select_fixture_lab("camera-aiming");
    assert!(
        !app.lab.scroll_to_top,
        "re-picking the lab already open is not a switch, and must leave the \
             reader where they were",
    );
}

/// **Non-vacuity for the test above**: the scenario is real, not hypothetical.
///
/// A lab with no mode-switching stop would make the round trip untestable and
/// the fix unnecessary. `matching.md` has one, near its end, which is why the bug
/// showed up as "at the completion of the lab".
#[test]
fn a_fixture_lab_really_does_contain_a_mode_switching_stop() {
    let found = bridge::fixture_labs().into_iter().any(|p| {
        std::fs::read_to_string(&p)
            .map(|t| t.contains("hrw://source/"))
            .unwrap_or(false)
    });
    assert!(
        found,
        "no fixture lab contains a `hrw://source/` stop, so nothing exercises \
             the mode round trip — either a lab lost one or this guard is stale",
    );
}

/// **Every stage can be pointed into, including the five with no sub-views.**
///
/// Parse, Resolve, Instantiate, Typecheck and DAE render one generic tree and have
/// no `SubView` variants, so the four-segment `node` form cannot name a node in any
/// of them — the richest noun in the link vocabulary was unavailable on the stages
/// with the least else to point at. Found 2026-08-03 when the DAE lab's
/// `hrw://stage/Dae/Tree/node/x` links all failed to parse.
///
/// **Checks the property, not the five known names**: a tree-only stage added later
/// fails here rather than quietly inheriting the hole.
#[test]
fn a_node_link_reaches_every_stage_including_the_tree_only_ones() {
    let mut tree_only = 0usize;
    for &kind in StageKind::ALL {
        let uri = format!("hrw://stage/{}/node/x", kind.slug());
        let parsed = parse_hrw_link(&uri);
        assert!(
            matches!(&parsed, Some(HrwLink::PointAtNode(k, None, _)) if *k == kind),
            "{uri} must point into {}, got {parsed:?}",
            kind.name(),
        );

        // Round-trip, so the form a capture *writes* is one a lab can read back.
        let Some(link) = parsed else { unreachable!() };
        assert_eq!(link.describe(), format!("stage/{}/node/x", kind.slug()));

        if SubView::from_slug(kind, "Tree").is_none() {
            tree_only += 1;
            // And the four-segment form is still refused for these — a link
            // naming a sub-view the stage does not have is malformed, not
            // silently downgraded to "somewhere in the stage".
            assert!(
                parse_hrw_link(&format!("hrw://stage/{}/Tree/node/x", kind.slug())).is_none(),
                "{} has no Tree sub-view; naming one must fail",
                kind.name(),
            );
        }
    }
    assert!(
        tree_only >= 5,
        "expected at least the five tree-only stages, found {tree_only} — if a stage \
             gained sub-views that is fine, but check this test still covers the case",
    );
}

/// A stop clicked out of order says so, instead of doing nothing.
///
/// Doug clicked a lab's fourth stop first. Nothing happened: with no specimen the
/// stage area returns early, so the link set state nothing consumed. Silence is the
/// one outcome a lab cannot survive, because there is no way to tell it from a
/// broken link.
///
/// The state is **not** left pending. Setting it and returning would be worse than
/// doing nothing — it would fire when a specimen arrived later, sending the reader
/// somewhere no link had pointed.
#[test]
fn a_stop_needing_a_specimen_refuses_without_one() {
    let needs = [
        HrwLink::SwitchStage(StageKind::Structural, None),
        HrwLink::ShowSource(Some(9)),
        HrwLink::Follow("C.v".to_owned()),
        HrwLink::PointAtNode(
            StageKind::Structural,
            Some(SubView::Structural(StructuralView::Tree)),
            vec![Seg::Key("blocks".into())],
        ),
        HrwLink::SeekFrame(
            StageKind::Structural,
            SubView::Structural(StructuralView::MatchingAnim),
            0,
        ),
        HrwLink::AimAtEquation(
            StageKind::Structural,
            SubView::Structural(StructuralView::TarjanAnim),
            0,
        ),
    ];
    for link in needs {
        assert!(link.requires_specimen(), "{link:?} needs a specimen");
        let mut app = App::test_default();
        app.dispatch_hrw_link(link);
        assert!(app.notice.is_some(), "it must say so");
        assert!(
            app.pending_stage.is_none(),
            "and leave nothing armed to fire later"
        );
        assert!(app.pending_sub_view.is_none());
        assert!(app.seek_frame.is_none());
        assert!(app.aim_at_equation.is_none());
        assert!(app.context.jump_target.is_none());
    }

    // The three that stand alone are unaffected.
    for link in [
        HrwLink::LoadSpecimen("RcCircuit".to_owned()),
        HrwLink::LoadAndSwitch("RcCircuit".to_owned(), StageKind::Structural, None),
        HrwLink::OpenNotebook("x.nb".to_owned()),
    ] {
        assert!(!link.requires_specimen(), "{link:?} makes sense on its own");
    }
}

/// Sub-view availability depends on the model, not only the stage.
///
/// Doug found the cross-platform lab linking to `Structural/Summary` on
/// `ProportionalLoop`. The slug is valid for the stage, so `SubView::from_slug`
/// accepts it — but Summary only has a tab when a model is **singular**, and
/// ProportionalLoop is not. The link selected a view with no tab and the panel
/// rendered the singular summary for a non-singular model.
///
/// One predicate now answers this for both the tab bar and the link guard, so a
/// tab that exists and a link that is honoured cannot disagree.
#[test]
fn a_sub_view_is_available_only_when_its_tab_is() {
    let clean = Stage::ok(serde_json::json!({}));
    // `recovered` = Outcome::Flagged, which is what the worker really builds for
    // a singular structural analysis: real IR *plus* an error, and the pipeline
    // carries on into index reduction.
    let singular = Stage::recovered(serde_json::json!({ "error": {} }), "singular");

    // A non-singular Structural stage: no Summary, but the pattern views are there.
    let mut app = App::test_default();
    app.stage = StageKind::Structural;
    app.stages.structural = clean.clone();
    assert!(
        !app.structural_view_available(StructuralView::Summary),
        "no Summary here"
    );
    assert!(app.structural_view_available(StructuralView::SpyPlot));
    assert!(app.structural_view_available(StructuralView::TearingAnim));
    assert!(app.structural_view_available(StructuralView::Incidence));

    // Singular: Summary appears, and the views needing a full matching vanish.
    app.stages.structural = singular;
    assert!(app.structural_view_available(StructuralView::Summary));
    assert!(!app.structural_view_available(StructuralView::SpyPlot));
    assert!(!app.structural_view_available(StructuralView::TearingAnim));
    // ...except Matching, whose *failure* is the point on a singular system (#44).
    assert!(app.structural_view_available(StructuralView::MatchingAnim));
    assert!(app.structural_view_available(StructuralView::Tree));

    // Index Reduction always has a Summary, and the reduction replay only with frames.
    app.stage = StageKind::IndexReduction;
    app.stages.index_reduction = clean;
    assert!(app.structural_view_available(StructuralView::Summary));
    assert!(
        !app.structural_view_available(StructuralView::Animate),
        "no frames yet"
    );
}

/// **A selected sub-view with no tab is corrected AND reported**, never quietly
/// tolerated.
///
/// The backstop for the 2026-08-19 alias defect, and it guards the *class*: three
/// separate doors write `viewport.structural`, each with its own guard, and nothing
/// checked the result. Adding a door without its guard is what happened when the
/// Aliases tab arrived and the stage-change default was not updated with it.
///
/// **The notice is half the test.** After the fix in `report_sub_view` there is no
/// known path here, so a silent clamp would hide the very regression this exists to
/// catch — the "silence must be a failure" rule applied to a guard whose success is
/// invisible.
#[test]
fn a_sub_view_with_no_tab_is_clamped_and_reported() {
    let mut app = App::test_default();
    app.stage = StageKind::Structural;
    app.stages.structural = Stage::ok(serde_json::json!({}));
    // Structural never offers the alias replay: `structural_view_available` requires
    // the Index Reduction stage for it, whatever the report holds.
    app.viewport.structural = StructuralView::AliasAnim;

    app.clamp_structural_sub_view();

    assert_eq!(
        app.viewport.structural,
        StructuralView::Tree,
        "Tree is the one view every report stage offers, singular or not",
    );
    let notice = app.notice.as_deref().expect(
        "reaching this means an upstream guard is missing \u{2014} it must be said, \
             not silently corrected",
    );
    assert!(
        notice.contains("AliasAnim"),
        "the notice must name the view that was stranded, not the fallback: {notice}",
    );
    assert!(
        !notice.contains("Tree"),
        "naming the fallback would read as though the tree were the problem: {notice}",
    );
}

/// And the guard is **a no-op on a healthy selection** — otherwise the test above is
/// satisfied by a method that clamps everything to Tree and notifies every frame.
#[test]
fn a_sub_view_that_has_a_tab_is_left_alone() {
    let mut app = App::test_default();
    app.stage = StageKind::Structural;
    app.stages.structural = Stage::ok(serde_json::json!({}));
    app.viewport.structural = StructuralView::Incidence;

    app.clamp_structural_sub_view();

    assert_eq!(app.viewport.structural, StructuralView::Incidence);
    assert!(
        app.notice.is_none(),
        "nothing went wrong, so nothing may be reported",
    );
}

/// A **non-report stage** is left alone entirely.
///
/// `viewport.structural` keeps its value while the reader is on Flatten or Events —
/// that is the point of a viewport surviving the stage change — so clamping there
/// would destroy the camera the reader set and notify about a view nobody is looking
/// at.
#[test]
fn the_clamp_does_not_touch_a_non_report_stage() {
    let mut app = App::test_default();
    app.stage = StageKind::Flatten;
    app.viewport.structural = StructuralView::AliasAnim;

    app.clamp_structural_sub_view();

    assert_eq!(app.viewport.structural, StructuralView::AliasAnim);
    assert!(app.notice.is_none());
}

/// The System Modeler verb parses, needs a name, and stands alone.
///
/// It needs no specimen *loaded* — like the load verbs, it makes sense on its own,
/// which matters because the adjudicator case is often "open this in SM and see that
/// it refuses", reached without walking a lab first.
#[test]
fn the_system_modeler_verb_stands_alone() {
    assert_eq!(
        parse_hrw_link("hrw://systemmodeler/IncompatibleConnect"),
        Some(HrwLink::OpenInSystemModeler(
            "IncompatibleConnect".to_owned()
        )),
    );
    assert!(
        parse_hrw_link("hrw://systemmodeler/").is_none(),
        "a bare verb names nothing"
    );
    assert!(
        !HrwLink::OpenInSystemModeler("X".to_owned()).requires_specimen(),
        "opening a specimen in another tool does not need one loaded here",
    );
    // Round-trips into the action trail like every other verb.
    let link = parse_hrw_link("hrw://systemmodeler/CapacitorLoop").unwrap();
    assert_eq!(
        format!("hrw://{}", link.describe()),
        "hrw://systemmodeler/CapacitorLoop"
    );
}

/// The notebook verb parses a real name and refuses an empty one.
///
/// `hrw://notebook/` alone names nothing. Accepting it meant a **prose mention** of
/// the verb inside a code span parsed as a link to an unnamed file — which the
/// fixture reference test duly reported as a missing notebook called "". Two small
/// faults met there: the extractor did not stop at a backtick, and the grammar
/// accepted an empty name.
#[test]
fn the_notebook_verb_needs_a_name() {
    assert_eq!(
        parse_hrw_link("hrw://notebook/structural-vs-numerical-rank.nb"),
        Some(HrwLink::OpenNotebook(
            "structural-vs-numerical-rank.nb".to_owned()
        )),
    );
    assert!(
        parse_hrw_link("hrw://notebook/").is_none(),
        "a bare verb names nothing"
    );
    assert!(parse_hrw_link("hrw://notebook").is_none());
}

/// A verb written in prose, inside a code span, is not a link.
///
/// Documentation about `hrw://` belongs in labs and doc comments, and writing it in
/// backticks is how one writes it. The extractor must not turn that into a hook.
#[test]
fn a_code_span_mention_is_not_extracted_as_a_link() {
    let md = "Use the [notebook verb](hrw://notebook/x.nb). \
                  Writing `hrw://notebook/` in prose must not register a hook.";
    let links = extract_hrw_links(md);
    assert_eq!(
        links,
        vec!["hrw://notebook/x.nb", "hrw://notebook/"],
        "the code-span mention stops at the backtick rather than swallowing it",
    );
    // ...and the truncated mention does not parse, so nothing acts on it.
    assert!(parse_hrw_link("hrw://notebook/").is_none());
}

/// A link can set the follow, independently of what is pointed at.
///
/// The two composition primitives are independent by design — point-only,
/// follow-only and both are all normal states — so `follow` deliberately does not
/// touch the stage.
#[test]
fn a_link_can_set_the_follow() {
    assert_eq!(
        parse_hrw_link("hrw://follow/emf.phi"),
        Some(HrwLink::Follow("emf.phi".to_owned())),
    );

    let mut app = App::test_default();
    app.stage = StageKind::Events;
    app.dispatch_hrw_link(HrwLink::Follow("load.w".to_owned()));
    assert_eq!(app.stage, StageKind::Events, "following does not navigate");
}

/// A link's frame number is the one on screen — the two must not be off by one.
///
/// Doug walked the fixture lab and found the link and the counter disagreeing.
/// The fixture had even *documented* the discrepancy ("frames are 0-based in links,
/// 1-based in the display"), which is writing a bug down instead of fixing it.
///
/// The rule this pins: **each verb matches how its own thing is displayed.** Frames
/// read "Frame 3/11" from one, so frame links count from one; equations read
/// `f_x[46]` from zero, so equation links count from zero. Uniformity between the
/// two verbs would force one to disagree with the screen, which is the drift that
/// actually costs something.
#[test]
fn a_frame_link_and_the_frame_counter_agree() {
    for shown in [1usize, 2, 7, 41] {
        let link = format!("hrw://stage/Structural/MatchingAnim/frame/{shown}");
        let Some(HrwLink::SeekFrame(_, _, cursor)) = parse_hrw_link(&link) else {
            panic!("{link} should parse");
        };
        // What the label would render for that cursor.
        let label = crate::frame_label(cursor, 100, crate::LiveState::Idle);
        assert!(
            label.starts_with(&format!("Frame {shown}/")),
            "{link} should land on a view reading \"Frame {shown}/…\", got {label:?}",
        );
    }
}

/// The frame-seek verb parses everywhere an animation lives.
#[test]
fn a_link_can_seek_to_a_frame() {
    assert_eq!(
        parse_hrw_link("hrw://stage/Structural/MatchingAnim/frame/7"),
        Some(HrwLink::SeekFrame(
            StageKind::Structural,
            SubView::Structural(StructuralView::MatchingAnim),
            6, // 1-based link, 0-based cursor
        )),
    );
    // The non-structural animated views too — one per stage that has one.
    for (stage, view) in [
        ("Events", "PreLowering"),
        ("Initialization", "IcPlan"),
        ("Flatten", "Connections"),
    ] {
        let link = format!("hrw://stage/{stage}/{view}/frame/3");
        assert!(
            matches!(parse_hrw_link(&link), Some(HrwLink::SeekFrame(_, _, 2))),
            "{link} should seek",
        );
    }
    // Links are **1-based**, matching the on-screen counter, so `frame/1` is the
    // first frame and `frame/0` does not exist.
    assert!(matches!(
        parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/1"),
        Some(HrwLink::SeekFrame(_, _, 0)),
    ));
    assert!(
        parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/0").is_none(),
        "there is no frame zero when the counter starts at one",
    );
    // Garbage still fails rather than defaulting.
    assert!(parse_hrw_link("hrw://stage/Structural/TarjanAnim/frame/last").is_none());
    assert!(parse_hrw_link("hrw://stage/Structural/Tree/frame/1").is_some());
    assert!(parse_hrw_link("hrw://stage/Events/TarjanAnim/frame/1").is_none());
}

/// The camera-aiming verb parses, and only where it makes sense.
#[test]
fn a_link_can_aim_at_an_equation() {
    assert_eq!(
        parse_hrw_link("hrw://stage/Structural/TarjanAnim/equation/13"),
        Some(HrwLink::AimAtEquation(
            StageKind::Structural,
            SubView::Structural(StructuralView::TarjanAnim),
            13,
        )),
    );
    // Works on the other stage that shares the sub-view enum.
    assert!(matches!(
        parse_hrw_link("hrw://stage/IndexReduction/MatchingAnim/equation/0"),
        Some(HrwLink::AimAtEquation(StageKind::IndexReduction, _, 0)),
    ));

    // A sub-view the stage does not have still fails, rather than aiming blindly.
    assert!(parse_hrw_link("hrw://stage/Events/TarjanAnim/equation/1").is_none());
    // A non-numeric index fails rather than silently becoming 0.
    assert!(parse_hrw_link("hrw://stage/Structural/TarjanAnim/equation/x").is_none());
    // The shorter forms still parse — raising `splitn` to 5 must not break them.
    assert!(matches!(
        parse_hrw_link("hrw://stage/Structural/Incidence"),
        Some(HrwLink::SwitchStage(StageKind::Structural, Some(_))),
    ));
    assert!(matches!(
        parse_hrw_link("hrw://load/CapacitorLoop/Structural/Summary"),
        Some(HrwLink::LoadAndSwitch(_, StageKind::Structural, Some(_))),
    ));
}

/// A sub-view slug is resolved **against its stage**, so the same word means
/// different things in different stages and a wrong pairing does not navigate.
#[test]
fn sub_view_slugs_are_stage_scoped() {
    // `Tree` exists under four stages and means a different enum in each.
    assert_eq!(
        SubView::from_slug(StageKind::Flatten, "Tree"),
        Some(SubView::Flatten(FlattenView::Tree)),
    );
    assert_eq!(
        SubView::from_slug(StageKind::Events, "Tree"),
        Some(SubView::Events(EventsView::Tree)),
    );

    // A slug from the wrong stage must not resolve — better a dead link than
    // one that navigates somewhere the author did not mean.
    assert!(SubView::from_slug(StageKind::Flatten, "MatchingAnim").is_none());
    assert!(SubView::from_slug(StageKind::Events, "IcPlan").is_none());
    // Stages with no sub-views reject every slug.
    assert!(SubView::from_slug(StageKind::Parse, "Tree").is_none());
    // And a malformed link is None rather than a partial navigation.
    assert!(parse_hrw_link("hrw://stage/Structural/NoSuchView").is_none());
}

/// **Every sub-view name the capture emits is addressable by a link, and vice
/// versa.** This is #42's design principle as an assertion: `hrw://` should
/// express any noun `focus.json` can describe, so the two directions share one
/// vocabulary. Without this test the two lists drift, and a lab would point at
/// a view whose capture name had been renamed.
#[test]
fn link_slugs_and_capture_names_are_the_same_vocabulary() {
    let cases: &[(StageKind, &[&str])] = &[
        (
            StageKind::Structural,
            &[
                structural_view_name(StructuralView::Summary),
                structural_view_name(StructuralView::SpyPlot),
                structural_view_name(StructuralView::Incidence),
                structural_view_name(StructuralView::MatchingAnim),
                structural_view_name(StructuralView::TarjanAnim),
                structural_view_name(StructuralView::TearingAnim),
                structural_view_name(StructuralView::AliasAnim),
                structural_view_name(StructuralView::Animate),
                structural_view_name(StructuralView::Tree),
            ],
        ),
        (
            StageKind::Flatten,
            &[
                flatten_view_name(FlattenView::Equations),
                flatten_view_name(FlattenView::SourceMap),
                flatten_view_name(FlattenView::Connections),
                flatten_view_name(FlattenView::Tree),
            ],
        ),
        (
            StageKind::Events,
            &[
                events_view_name(EventsView::Tree),
                events_view_name(EventsView::PreLowering),
            ],
        ),
        (
            StageKind::Initialization,
            &[
                init_view_name(InitView::Tree),
                init_view_name(InitView::IcPlan),
            ],
        ),
    ];
    for (stage, names) in cases {
        for name in *names {
            assert!(
                SubView::from_slug(*stage, name).is_some(),
                "capture emits {name:?} for {stage:?} but no link can address it",
            );
        }
    }
}

/// An ad hoc lab written to the bridge round-trips, and its links parse.
///
/// Replaces `lab_document_hrw_links_are_valid`, which checked the links in
/// `end_to_end_lab.md` — a document HRW no longer shows. Its prose was
/// retired 2026-07-29 and lab mode now renders whatever Claude writes to
/// `.hrw-bridge/lab.md`, so the subject of that test no longer existed.
///
/// Touches the shared bridge directory, so it needs `--test-threads=1` like
/// the other bridge tests.
#[test]
fn an_ad_hoc_lab_round_trips_through_the_bridge() {
    let saved = std::fs::read_to_string(bridge::LAB_FILE).ok();

    let lab = "# Station 1

Open [the Structural tab](hrw://stage/Structural).

                    # Station 2

Now [load MotorWithBrake](hrw://load/MotorWithBrake/IndexReduction).
";
    std::fs::create_dir_all(bridge::BRIDGE_DIR).unwrap();
    std::fs::write(bridge::LAB_FILE, lab).unwrap();

    let (text, _mtime) = bridge::read_lab().expect("a written lab is readable");
    assert!(text.contains("Station 1"), "{text}");

    let links = extract_hrw_links(&text);
    assert_eq!(links.len(), 2, "both links found: {links:?}");
    for link in &links {
        assert!(parse_hrw_link(link).is_some(), "unparseable link: {link}");
    }

    // Absence is the normal state, not an error.
    std::fs::remove_file(bridge::LAB_FILE).unwrap();
    assert!(bridge::read_lab().is_none(), "no lab file means no lab");

    if let Some(prev) = saved {
        std::fs::write(bridge::LAB_FILE, prev).unwrap();
    }
}

/// Every `hrw://` link in every specimen `purpose.md` parses.
///
/// **Counts the files it checked and asserts the count is right.** Until
/// 2026-07-29 this looked for `narrative.md`, and when those were renamed to
/// `purpose.md` the `continue` swallowed every directory — the test passed by
/// checking nothing. A silent-skip test is worse than no test, so the count
/// is now part of the assertion.
#[test]
fn purpose_note_hrw_links_are_valid() {
    let notebook_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/specimen-notebook");
    let mut checked = 0usize;
    let mut notes = std::collections::BTreeSet::new();
    for entry in std::fs::read_dir(&notebook_dir).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_dir() {
            continue;
        }
        let purpose = path.join("purpose.md");
        assert!(
            purpose.exists(),
            "every specimen dir needs a purpose.md: {}",
            path.display()
        );
        notes.insert(
            path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default()
                .to_owned(),
        );
        checked += 1;
        let text = std::fs::read_to_string(&purpose).unwrap();
        for link in extract_hrw_links(&text) {
            assert!(
                parse_hrw_link(&link).is_some(),
                "invalid hrw link in {}: {link}",
                purpose.display()
            );
        }
    }
    // Tied to the corpus rather than to a magic number. The literal `14` here
    // failed the moment four diagnostic specimens were added (2026-07-29) — it was
    // guarding the right property with the wrong constant, so it reported a
    // *correct* corpus as broken. Every `specimens/*.mo` must have a note, and
    // every note must belong to a specimen; both directions matter, because an
    // orphaned note is prose about a model that no longer exists.
    let specimen_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("specimens");
    let specimens: std::collections::BTreeSet<String> = std::fs::read_dir(&specimen_dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("mo"))
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .collect();

    assert_eq!(
        notes,
        specimens,
        "every specimen needs a purpose note and every note needs a specimen; \
             missing notes: {:?}; orphaned notes: {:?}",
        specimens.difference(&notes).collect::<Vec<_>>(),
        notes.difference(&specimens).collect::<Vec<_>>(),
    );
    assert_eq!(checked, specimens.len());
}

#[test]
fn open_resets_all_specimen_state() {
    let mut app = App::test_default();
    // Populate fields with non-default values to detect missed resets.
    app.model = Some(String::from("OldModel"));
    app.sim_data = Some(crate::worker::SimData {
        times: vec![0.0],
        names: vec![],
        data: vec![],
        n_states: 0,
        has_discontinuities: false,
        solver_steps: vec![],
    });
    app.sim_error = Some("stale error".into());
    app.sim_running = true;
    app.def_index.insert(
        1,
        crate::worker::DefInfo {
            name: "x".into(),
            kind: crate::worker::DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        },
    );
    app.cached_equation_sheet = Some(crate::equation_sheet::EquationSheet {
        node_lines: std::collections::HashMap::new(),
        flat_node_lines: std::collections::HashMap::new(),
        groups: vec![],
        n_equations: 0,
        variables: vec![],
        n_states: 0,
        n_algebraics: 0,
        n_parameters: 0,
        n_constants: 0,
        n_discrete: 0,
        n_inputs: 0,
        n_outputs: 0,
        source_lines: vec![],
    });
    app.identifier_index = Some(crate::identifier_index::IdentifierIndex::default());
    app.tracked_identifier = Some("h".into());
    app.source.text = Some("old source".into());
    app.viewport.highlighted_eq_row = Some(0);
    app.viewport.highlighted_source_line = Some(0);
    app.nav.push(NavEntry {
        name: "x".into(),
        value: serde_json::Value::Null,
        def_index: BTreeMap::new(),
    });
    app.nav_loading = Some("y".into());
    app.nav_error = Some("z".into());
    app.pending_stage = Some(StageKind::Resolve);
    app.viewing_log = false;

    app.open(PathBuf::from("specimens/BouncingBall.mo"));

    assert!(app.compiling, "compiling should be true");
    assert!(app.model.is_none(), "model should be cleared");
    assert!(app.sim_data.is_none(), "sim_data should be cleared");
    assert!(app.sim_error.is_none(), "sim_error should be cleared");
    assert!(!app.sim_running, "sim_running should be false");
    assert!(app.def_index.is_empty(), "def_index should be cleared");
    assert!(
        app.cached_equation_sheet.is_none(),
        "cached_equation_sheet should be cleared"
    );
    assert!(
        app.identifier_index.is_none(),
        "identifier_index should be cleared"
    );
    assert!(
        app.tracked_identifier.is_none(),
        "tracked_identifier should be cleared"
    );
    assert!(app.source.text.is_none(), "cached_source should be cleared");
    assert!(
        app.viewport.highlighted_eq_row.is_none(),
        "highlighted_eq_row should be cleared"
    );
    assert!(
        app.viewport.highlighted_source_line.is_none(),
        "highlighted_source_line should be cleared"
    );
    assert!(app.nav.is_empty(), "nav should be cleared");
    assert!(app.nav_loading.is_none(), "nav_loading should be cleared");
    assert!(app.nav_error.is_none(), "nav_error should be cleared");
    assert!(
        app.pending_stage.is_none(),
        "pending_stage should be cleared"
    );
    assert!(app.viewing_log, "viewing_log should be true");
}

/// **The snapshot names the sub-tab, not just the stage.**
///
/// `stage_tab` said `Flatten` and stopped there, so Claude could not tell the
/// equation sheet from the source map from the connections replay — the gap that had
/// Doug about to transcribe a pane by hand on 2026-08-13.
#[test]
fn the_diagnostic_snapshot_names_the_sub_view() {
    let mut app = App::test_default();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", StageKind::Flatten);

    app.viewport.flatten = FlattenView::Equations;
    assert_eq!(app.diagnostic_snapshot()["sub_view"], "EquationSheet");

    app.viewport.flatten = FlattenView::Connections;
    assert_eq!(app.diagnostic_snapshot()["sub_view"], "Connections");

    // A tree-only stage has no sub-tab, and must say so rather than invent one.
    app.stage = StageKind::Parse;
    assert!(
        app.diagnostic_snapshot()["sub_view"].is_null(),
        "a stage with one view reports null, not a fabricated name",
    );
}

/// **Leaving a published view removes the file; it never leaves the old one.**
///
/// A `view.json` describing a pane the reader has left is indistinguishable from a
/// current one by content alone, and Claude would answer confidently about the wrong
/// pane. That is the failure this whole file's rules exist to prevent, so the empty
/// case is the one worth a test.
///
/// **Touches the real bridge path**, as the `focus.json` tests already do — the
/// directory is a compile-time constant. It restores the file's absence afterwards.
#[test]
fn leaving_a_published_view_removes_the_file() {
    let mut app = App::test_default();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", StageKind::Flatten);
    app.viewport.flatten = FlattenView::Equations;
    app.cached_equation_sheet = Some(equation_sheet::EquationSheet {
        n_equations: 1,
        ..equation_sheet::EquationSheet::default()
    });

    app.publish_current_view();
    let path = std::path::Path::new(bridge::VIEW_FILE);
    assert!(path.exists(), "the equation sheet must be published");
    let text = std::fs::read_to_string(path).expect("read view.json");
    assert!(
        text.contains("Flatten/EquationSheet"),
        "the file must name the pane it describes, got: {text}",
    );

    // Move to a stage with no publisher.
    app.stage = StageKind::Parse;
    app.publish_current_view();
    assert!(
        !path.exists(),
        "a view with no publisher must remove the file, not leave the previous \
             pane's content behind",
    );
}

/// The publish is skipped when nothing moved, so the file is not rewritten per frame.
#[test]
fn republishing_the_same_view_is_a_no_op() {
    let mut app = App::test_default();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", StageKind::Parse);

    app.publish_current_view();
    let first = app.viewport.last_published_view.clone();
    assert_eq!(first.as_deref(), Some("Parse"));

    // A second call with nothing changed must return before touching the disk.
    app.publish_current_view();
    assert_eq!(app.viewport.last_published_view, first);
}

#[cfg(test)]
mod tests_incidence_row_link {
    use super::*;

    /// **`hrw://stage/<Stage>/Incidence/equation/<n>` marks the row.**
    ///
    /// # Why this verb was extended rather than a new one added
    ///
    /// `equation` reached only the canvas views, where it aims a camera. The Incidence
    /// view's rows *are* equations and nothing could link to one — so a lab could open
    /// the matrix and then had to describe the row in prose. On `Drivetrain`'s 97 rows
    /// that is the difference between pointing and gesturing, and the index-reduction
    /// lab hit it: it hand-copied a five-row table the pane already draws.
    ///
    /// **A separate `row` verb would put two words on one job.** The lab template's own
    /// rule 2 forbids exactly that, and "point at equation N" is what both views are
    /// being asked for — the canvas moves a camera, the matrix marks a row.
    #[test]
    fn an_equation_link_marks_the_incidence_row() {
        let mut app = App::test_default();
        // **A stage link is refused with no model loaded** — the guard behind Doug's
        // "no specimen loaded" report on 2026-08-16. Without this the test asserts
        // against a dispatch that never ran.
        app.test_set_walked_state(
            "/x/RcCircuit.mo",
            "RcCircuit",
            crate::worker::StageKind::Structural,
        );
        assert!(
            app.viewport.highlighted_eq_row.is_none(),
            "precondition: nothing is marked before the link is dispatched",
        );

        let link = parse_hrw_link("hrw://stage/Structural/Incidence/equation/4")
            .expect("the link form must parse");
        app.dispatch_hrw_link(link);

        assert_eq!(
            app.viewport.highlighted_eq_row,
            Some(4),
            "the row a lab points at must be marked; without this the link opens the \
             matrix and says nothing about which row it meant",
        );
        assert_eq!(app.stage, crate::worker::StageKind::Structural);
    }

    /// **The canvas aim still happens, so the verb did not change meaning for the views
    /// that already used it.**
    ///
    /// Extending a verb is only safe if the old consumers keep working; a test that
    /// checked the new behaviour alone would pass while `matching.md`'s camera links
    /// silently stopped aiming.
    #[test]
    fn extending_the_verb_did_not_break_the_camera_aim() {
        let mut app = App::test_default();
        app.test_set_walked_state(
            "/x/RcCircuit.mo",
            "RcCircuit",
            crate::worker::StageKind::Structural,
        );
        let link = parse_hrw_link("hrw://stage/Structural/TarjanAnim/equation/2")
            .expect("the canvas form must parse");
        app.dispatch_hrw_link(link);

        assert_eq!(
            app.aim_at_equation,
            Some(2),
            "the deferred camera aim must still be set \u{2014} the canvas views have \
             used this verb since before the Incidence view had any link at all",
        );
    }
}

#[cfg(test)]
mod tests_lab_in_diagnostics {
    use super::*;

    /// **The diagnostic capture names the lab that is open.**
    ///
    /// Doug, 2026-08-19: *"I'd like to enjoy the convenience of deixis when asking
    /// questions about statements which you've made in labs. Currently, it seems that
    /// I have to copy / paste those lab statements."*
    ///
    /// The capture reported `ui_mode: "Lab"` and nothing else about the lab, so
    /// **which document he was reading was unrecoverable** — and pasting was the only
    /// way to ask a question about it. With the name published, *"the Newton paragraph"*
    /// resolves, because the labs are on disk and can be read once the document is
    /// known.
    ///
    /// **Absence stays distinguishable from silence**, which is why the null case is
    /// asserted too: no lab open must read as `null`, not as a missing key that could
    /// equally mean the field was never written.
    #[test]
    fn the_capture_names_the_open_lab() {
        let mut app = App::test_default();

        let none_open = app.diagnostic_snapshot();
        assert_eq!(
            none_open.get("lab"),
            Some(&serde_json::Value::Null),
            "with no lab open the key must be present and null \u{2014} a missing key \
             cannot be told apart from a field that was never published",
        );

        assert!(
            app.test_select_fixture_lab("index-reduction"),
            "the fixture must be readable, or nothing below is testing a selection",
        );

        let open = app.diagnostic_snapshot();
        let name = open
            .get("lab")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        assert!(
            name.contains("index-reduction"),
            "the capture must name the open lab so a question about it can be answered \
             without pasting; got {name:?}",
        );
    }
}

#[cfg(test)]
mod tests_lab_back {
    use super::*;

    /// **Back returns to the lab a link came from, at the offset it was left at.**
    ///
    /// Doug, 2026-08-19: *"while in the index reduction lab, I can click a link to
    /// navigate to the blt-ordering lab, but then I cannot navigate back."* Authored
    /// back-links solved the hub case in August and cannot solve this one: the
    /// hub-to-lab edge has a canonical parent, cross-references do not.
    #[test]
    fn back_returns_to_the_previous_lab_where_it_was_left() {
        let mut app = App::test_default();
        assert!(app.test_select_fixture_lab("index-reduction"));

        // Stand in for having read part way down before following a link.
        app.lab.current_scroll_y = 812.0;
        assert!(app.test_select_fixture_lab("blt-ordering"));
        assert_eq!(
            app.lab.history.len(),
            1,
            "following a link must record where it came from, or Back has nothing to pop",
        );

        app.lab_back();

        let back_on = app
            .lab
            .selected
            .as_ref()
            .and_then(|s| match s {
                LabSource::Fixture(p) => p.file_stem().and_then(|n| n.to_str()),
                LabSource::AdHoc => None,
            })
            .unwrap_or_default()
            .to_owned();
        assert_eq!(back_on, "index-reduction");
        assert_eq!(
            app.lab.restore_scroll_y,
            Some(812.0),
            "and to the place in it the reader had reached \u{2014} landing at the top of \
             a document you were halfway down is most of the friction, not a detail",
        );
        assert!(
            app.lab.history.is_empty(),
            "the entry is consumed, not reusable"
        );
    }

    /// **Back does not record its own navigation.**
    ///
    /// The classic history bug named in `ideas.md` #78: a Back that pushes the lab it is
    /// leaving ping-pongs between two documents, and nothing further up the stack is ever
    /// reachable.
    #[test]
    fn back_does_not_push_what_it_is_leaving() {
        let mut app = App::test_default();
        assert!(app.test_select_fixture_lab("index-reduction"));
        assert!(app.test_select_fixture_lab("blt-ordering"));
        app.lab_back();

        assert!(
            app.lab.history.is_empty(),
            "a stack that grows when you go back is one you can never reach the bottom of",
        );
        // And Back with nothing to return to is a no-op rather than a panic.
        app.lab_back();
        assert!(app.lab.history.is_empty());
    }

    /// **Re-selecting the lab already open records nothing**, or the stack fills with
    /// entries that go nowhere and Back looks enabled while doing nothing visible.
    #[test]
    fn reselecting_the_open_lab_does_not_grow_the_history() {
        let mut app = App::test_default();
        assert!(app.test_select_fixture_lab("index-reduction"));
        assert!(app.test_select_fixture_lab("index-reduction"));
        assert!(app.lab.history.is_empty());
    }
}

/// **A `▶ Look` link to the specimen already loaded must switch, not recompile.**
///
/// Doug, 2026-08-22: *"the Look links always seem to compile specimens, even if those
/// specimens are already loaded and compiled."* Walking one lab paid a full compile
/// per stop, because every stop's Look link names the same specimen.
///
/// The rule is not new — the left panel's `ModelListNav::Select` arm has always
/// skipped the recompile, and `ModelListNav::Reload` exists for when you *want* one.
/// `LoadAndSwitch` was simply the arm that never got the guard.
#[cfg(test)]
mod tests_look_link_reuses_the_loaded_compile {
    use super::*;

    #[test]
    fn a_look_link_to_the_loaded_specimen_switches_without_recompiling() {
        let mut app = App::test_default();
        app.model_list.dir = DEFAULT_SPECIMEN_DIR.to_owned();
        app.model_list.rescan();
        let path = app
            .find_specimen("RcCircuit")
            .expect("RcCircuit is a curated specimen");
        app.selected = Some(path);
        app.compiling = false;
        app.stage = StageKind::Parse;

        app.dispatch_hrw_link(HrwLink::LoadAndSwitch(
            "RcCircuit".to_owned(),
            StageKind::Flatten,
            None,
        ));

        assert!(
            !app.compiling,
            "a Look link to the specimen already loaded started a compile",
        );
        assert_eq!(
            app.stage,
            StageKind::Flatten,
            "and it must still switch the stage, or the link does nothing",
        );
        assert!(
            app.pending_stage.is_none(),
            "no compile is coming, so a deferred stage would fire on the NEXT one",
        );
    }

    /// The complement, and what keeps the test above from passing vacuously: a link to
    /// a **different** specimen still compiles, and defers the stage until it lands.
    #[test]
    fn a_look_link_to_a_different_specimen_still_compiles() {
        let mut app = App::test_default();
        app.model_list.dir = DEFAULT_SPECIMEN_DIR.to_owned();
        app.model_list.rescan();
        let other = app
            .find_specimen("BouncingBall")
            .expect("BouncingBall is a curated specimen");
        app.selected = Some(other);
        app.compiling = false;

        app.dispatch_hrw_link(HrwLink::LoadAndSwitch(
            "RcCircuit".to_owned(),
            StageKind::Flatten,
            None,
        ));

        assert!(
            app.compiling,
            "a Look link to a different specimen must still compile it",
        );
        assert_eq!(
            app.pending_stage,
            Some(StageKind::Flatten),
            "and the stage must wait for the compile rather than showing an empty one",
        );
    }
}
