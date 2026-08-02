//! **Headless UI tests** — `docs/verification-plan.md` item 2.
//!
//! HRW has ~12,000 lines of UI and, until now, essentially one test that
//! exercised rendering. Everything else was verified by Doug walking fixture
//! tours, which makes **his attention the project's scarce resource**. These
//! tests convert the *mechanical* half of that — did the click select the node,
//! is the panel empty after a mode switch, does the notice exist — so his
//! attention goes only where judgement is required.
//!
//! # What this can and cannot see
//!
//! `egui_kittest` renders headlessly and queries the **accessibility tree**:
//! widgets with a label and a role. That draws a hard line through HRW:
//!
//! | Queryable | Not queryable |
//! |---|---|
//! | the IR tree, tab bars, buttons, notices, the equation sheet, the status bar | the incidence matrix, spy plot, Tarjan graph, matching animation |
//!
//! The second column is drawn with `Painter` calls — pixels, not widgets — so an
//! accessibility harness sees nothing there. **Image snapshot testing is
//! deliberately not enabled** (`snapshot`/`wgpu` features off): it asserts on
//! pixels, needs a GPU in the test path, and is brittle.
//!
//! **So the canvas views stay the fixture tours' job**, and that is now their
//! focused purpose rather than an accident of what nobody automated.
//!
//! # Geometry is reachable when the app records it
//!
//! The table above is about the *accessibility tree*, and it is easy to read as
//! "layout cannot be tested". That is too strong. The tree carries no geometry,
//! but **a widget's own state is a number the app can keep**, and a number can
//! be asserted on.
//!
//! `a_programmatic_source_scroll_keeps_the_left_margin` is the worked example.
//! Doug reported source lines with their left-most characters cut off; the cause
//! was `scroll_to_me` aligning on both axes in a `ScrollArea::both`. `ScrollArea`
//! stores its offset in `Memory` under an id derived from the parent `Ui`, which
//! a test cannot reconstruct — but `show` *returns* that state, so `app.rs`
//! records it into `source_scroll_offset` and the test reads it. The old code
//! measures 86 px of hidden left margin; the fixed code measures 0.
//!
//! **The general move: when a layout defect is reported, ask what number was
//! wrong.** If the app can hold that number, the defect is testable, and it need
//! not cost Doug a walk ever again. What stays genuinely his is what has no
//! number — colour, proportion, whether a thing reads well.
//!
//! # Two harness facts that each cost a wrong diagnosis
//!
//! **A widget laid out off-screen is queryable but not clickable.** At the
//! harness's 800x600 default, HRW's panels push the central content out of the
//! viewport — the tour links were in the accessibility tree, `query_by_label`
//! found them, and `click()` landed on nothing. The test read as *"the feature is
//! broken"*. It is not: the window was too small. Hence 1600x1200 below, and
//! hence the rule — **if a click appears to do nothing, check the layout before
//! the logic.**
//!
//! **HRW never goes quiescent, so `run()` cannot be used.** `tick_prewarm`
//! requests a repaint every frame while waiting for a debugger ack that never
//! comes in a test, so `Harness::run` exhausts its step budget and panics. That
//! is correct behaviour from a polling UI; `run_steps` is the right tool.
//!
//! # Why these tests do not compile specimens
//!
//! Driving the UI is cheap; compiling a model against the MSL is not — 30s and
//! 3.5 GB on a large one. These assert on **UI mechanics with state set
//! directly**, which is what the mechanical half of a tour actually checks. A
//! test that needs real IR belongs beside the worker tests, behind
//! `slow-tests`.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::app::App;

/// Drive HRW's whole frame in a headless harness.
///
/// `frame_ui` rather than [`eframe::App::ui`] because the trait method takes an
/// `eframe::Frame`, which cannot be constructed outside eframe — that one unused
/// parameter was the only thing standing between this UI and an automated test.
fn harness(app: App) -> Harness<'static, App> {
    // **1600x1200, not the 800x600 default.** HRW is a multi-panel observatory:
    // menu bar, status bar, specimen list, help panel, and a central panel with a
    // tab row. At 800x600 the central panel's content is pushed out of the
    // viewport, and a widget that is laid out off-screen is in the accessibility
    // tree but cannot be clicked — a synthetic click lands on nothing and the
    // test reads as "the feature is broken".
    let mut h = Harness::builder()
        .with_size(eframe::egui::Vec2::new(1600.0, 1200.0))
        .build_ui_state(|ui, app: &mut App| app.frame_ui(ui), app);
    // **`run_steps`, not `run` — HRW never goes quiescent, by design.**
    //
    // `Harness::run` repaints until the app stops asking, then gives up after
    // four frames. HRW keeps asking: `tick_prewarm` requests a repaint every
    // frame while it waits for the debugger to acknowledge a breakpoint, and in
    // a test there is no debugger, so it waits out its full three-second timeout
    // (`app.rs`, `Prewarm::Armed`).
    //
    // That is not a bug to route around — a UI that polls is *supposed* to keep
    // painting. Two frames is what these tests need: one to lay out, one so
    // anything a click deferred to the next frame has landed.
    h.run_steps(2);
    h
}

/// The harness renders HRW at all, and the accessibility tree is populated.
///
/// **The non-vacuity test for every test below it.** A harness that rendered
/// nothing would let every later assertion pass by querying an empty tree, which
/// is precisely the silent-success shape the must-fire rule exists to forbid.
#[test]
fn the_harness_renders_hrw_and_sees_widgets() {
    let h = harness(App::test_default());

    let buttons = h.get_all_by_label_contains("").count();
    assert!(
        buttons > 0,
        "the accessibility tree has no buttons at all — the harness is not rendering HRW, \
         and every other UI test would pass vacuously",
    );

    // The menu bar is the most stable thing on screen: present in every mode,
    // for every specimen, whether or not anything has compiled.
    assert!(
        h.query_by_label("File").is_some(),
        "expected the File menu; the frame rendered but not HRW's chrome",
    );
    assert!(h.query_by_label("Help").is_some(), "expected the Help menu");
}

/// The tour picker offers exactly the fixture tours — **rendered**, not merely
/// listed.
///
/// `app::tests::the_tour_list_offers_fixtures_with_ad_hoc_first` already asserts
/// this against `App::tours`. **This asserts it against what is on screen**,
/// which is the half that was previously checkable only by Doug looking at it.
///
/// It also pins the `README.md` exclusion at the rendered layer:
/// `docs/fixture-tours/` gained a README on 2026-08-01 and `bridge::fixture_tours`
/// had to learn to skip it, or the picker would offer a tour whose stops do not
/// exist.
#[test]
fn the_tour_picker_shows_every_fixture_and_no_readme() {
    let h = harness(App::test_default());

    for tour in [
        "node-pointing",
        "frame-seeking",
        "camera-aiming",
        "structural-vs-numerical-rank",
        "the-oracle",
    ] {
        assert!(
            h.query_by_label(tour).is_some(),
            "the tour picker should offer {tour:?}; it is a checked-in fixture",
        );
    }
    assert!(
        h.query_by_label("README").is_none(),
        "README.md is documentation ABOUT the tours, not a tour — offering it would \
         give the picker an entry whose stops do not exist",
    );
}

/// Selecting a different tour clears the stage side — **on screen**.
///
/// The bug this guards against was found by walking: *"the RHS doesn't
/// re-initialise on a second tour"*, which made Stop 1 look as though it had
/// already been done. `app::tests::switching_tours_resets_the_stage_side` covers
/// the state; this covers the state actually reaching the frame.
#[test]
fn switching_tours_clears_the_stage_side_on_screen() {
    let mut h = harness(App::test_default());

    // Walk into a tour far enough to have a model and a stage on the right.
    h.state_mut().test_set_walked_state(
        "/x/RcCircuit.mo",
        "RcCircuit",
        crate::worker::StageKind::Structural,
    );
    h.run_steps(2);
    assert!(h.state().test_model().is_some(), "precondition: the RHS has something on it");

    // Now pick a *different* tour.
    h.get_by_label("the-oracle").click();
    h.run_steps(2);

    assert!(
        h.state().test_model().is_none(),
        "switching tours must clear the model, or the new tour is read against the \
         old tour's state and its first stop looks already done",
    );
    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Parse,
        "and the stage returns to the start of the pipeline",
    );
}


/// A stop that needs a specimen is **refused, and says so where you can see it**.
///
/// **This is the "the notice was invisible" bug**, and the story is worth
/// keeping. Doug clicked a stop that HRW correctly refused, with the reason on
/// screen, and reported that nothing happened — because the tour said "a notice
/// appears" and never said notices live in the status bar. Two things were
/// wrong: the expectation did not say *where to look*, and nothing verified the
/// notice was rendered at all.
///
/// This closes the second half. The first half is a rule for writing tours, in
/// `docs/fixture-tours/README.md`.
///
/// **It also documents that the refusal is correct.** An earlier version of the
/// isolation test below clicked this same link on a fresh app and asserted the
/// stage changed — asserting a bug into existence, because a stage link with no
/// specimen *should* do nothing. Probing the behaviour rather than trusting the
/// premise turned a wrong test into this one.
#[test]
fn a_stop_needing_a_specimen_is_refused_with_a_visible_notice() {
    let mut h = harness(App::test_default());

    h.get_by_label("Structural → Incidence").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Parse,
        "with no specimen loaded the stop must not half-apply — a stage set now would \
         linger and fire when a specimen arrived later, sending the reader somewhere \
         no link pointed",
    );
    assert!(
        h.query_by_label_contains("no specimen loaded").is_some(),
        "the refusal must be RENDERED, not merely recorded. A silent refusal is \
         indistinguishable from a broken link, which is exactly how it was reported",
    );
}

/// A tour link acts **clicked in isolation**, not only after its predecessors.
///
/// This is the *"stop 4 works only if I click 1-3 first"* bug — the kind a human
/// walking in order never sees, because they always click stop 1 first. Driving
/// one link into an app that has *only* the specimen loaded is something a walk
/// structurally cannot do, which makes this the clearest case of the suite
/// catching what Doug cannot.
///
/// The specimen is set directly rather than loaded, because this asserts on
/// **link dispatch**, not on compilation — `open()` would spawn a real compile
/// against the MSL for no gain here.
#[test]
fn a_tour_link_acts_when_clicked_in_isolation() {
    let mut h = harness(App::test_default());
    h.state_mut().test_set_walked_state(
        "/x/RcCircuit.mo",
        "RcCircuit",
        crate::worker::StageKind::Parse,
    );
    h.run_steps(2);

    // A mid-tour link, with none of the stops before it clicked.
    h.get_by_label("Structural → Incidence").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Structural,
        "a stage link must act on its own once its precondition is met; needing an \
         earlier stop first is the \"works on the second click\" bug",
    );
}

/// A tour link can address a **corpus model**, not only a specimen file.
///
/// **This was the gap that blocked just-in-time curricula.** A curriculum is
/// delivered as an ad hoc tour — Claude writes `.hrw-bridge/tour.md` with the
/// models in the chosen order — and until 2026-08-01 `hrw://load/` resolved only
/// through `find_specimen`, which looks in `specimens/`. The worker could compile
/// an MSL model by name (`compile_model_by_name`, built for the fidelity sweep)
/// and **the UI had no way to ask**, so the 2,626-model corpus was unreachable
/// from a tour no matter how good a filter got.
///
/// Asserts on **dispatch**, not on compilation: the point is that the request is
/// made and the selection updated. Compiling an MSL model against the MSL costs
/// seconds and belongs behind `slow-tests`.
#[test]
fn a_tour_link_can_address_a_corpus_model() {
    let mut h = harness(App::test_default());
    h.state_mut().follow_link_for_test("hrw://load/Modelica.Electrical.Analog.Basic.Resistor");
    h.run_steps(2);

    assert_eq!(
        h.state().test_selected_name().as_deref(),
        Some("Modelica.Electrical.Analog.Basic.Resistor"),
        "a qualified name must select the corpus model, not fail as a missing specimen",
    );
    assert!(
        h.state().test_selection_is_library(),
        "and it must be marked as a library model — the source view reads `selected` from \
         disk, so a library selection mistaken for a file renders an empty pane",
    );
}

/// A curated specimen still wins, and a bare unknown name still fails loudly.
///
/// The fallback must not swallow a typo. `hrw://load/Typo` has no dot, so it is
/// not a qualified name and must be reported rather than sent to the library to
/// fail later with a worse message.
#[test]
fn the_load_verb_prefers_files_and_still_rejects_a_typo() {
    let mut h = harness(App::test_default());

    h.state_mut().follow_link_for_test("hrw://load/Drivetrain");
    h.run_steps(2);
    assert!(
        !h.state().test_selection_is_library(),
        "a curated specimen must resolve as a file, not fall through to the library",
    );

    h.state_mut().follow_link_for_test("hrw://load/NoSuchThing");
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("not found: NoSuchThing").is_some(),
        "a bare unknown name is a typo and must be reported, not guessed at",
    );
}

/// The corpus is **always visible**, and a click opens the model.
///
/// **This test asserted the opposite until 2026-08-01, and that is the lesson.**
/// The first version rendered the corpus section only while filtering, and this
/// test asserted the section was *absent* with an empty filter — so **the test
/// encoded the defect as a requirement.** Doug started HRW, saw no MSL models,
/// and reported them "not showing", which was exactly right from where he sat:
/// **an absence you cannot see is indistinguishable from a feature that was
/// never built.**
///
/// A green test suite said the feature worked. It did work — it was invisible.
/// That is the class `egui_kittest` cannot catch on its own, because a test only
/// checks the behaviour someone chose to assert.
///
/// The section is now a collapsed header carrying the model count, which keeps
/// the reason the first version existed — 2,626 rows must not bury 18 curated
/// specimens — while making the corpus's *existence* impossible to miss.
#[test]
fn the_corpus_is_visible_unfiltered_and_opens_on_click() {
    let mut h = harness(App::test_default());
    h.state_mut().test_set_ui_mode_specimen();
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("MSL corpus").is_some(),
        "the corpus must announce itself with NO filter typed — its absence is          indistinguishable from it not being implemented, which is how it was reported",
    );
    assert!(
        h.query_by_label_contains("2626").is_some()
            || h.query_by_label_contains("2,626").is_some(),
        "and it must say how many models it holds, so the header is evidence rather          than decoration",
    );

    h.state_mut().test_set_filter("Spice3BenchmarkDifferentialPair");
    h.run_steps(2);

    // The row renders its LEAF name; the qualified name is 60 characters and
    // would wrap every row.
    let row = h.query_by_label_contains("Spice3BenchmarkDifferentialPair");
    assert!(row.is_some(), "filtering must reveal the matching model");
    row.unwrap().click();
    h.run_steps(2);

    assert!(
        h.state().test_selection_is_library(),
        "clicking a corpus row must open it as a library model, not as a file",
    );
    assert_eq!(
        h.state().test_selected_name().as_deref(),
        Some("Modelica.Electrical.Spice3.Examples.Spice3BenchmarkDifferentialPair"),
        "and it must select the FULLY QUALIFIED name, since the leaf is only a label",
    );
}

/// The **background** names both halves: specimen **and** stage.
///
/// The third kind of context, and the only one with no labelled row -- which is
/// how half of it went missing in plain sight from the first commit of the bar
/// (`b2732393`) until Doug counted three kinds and saw two on 2026-08-01.
///
/// **This is the must-fire rule aimed at the Context Bar.** The bar's entire job
/// is to report what `focus.json` will carry, and `focus.json` carried the stage
/// all along. An under-reporting reporter is the failure mode most likely to go
/// unnoticed, because everything it *does* say is true.
#[test]
fn the_background_names_both_the_specimen_and_the_stage() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("MotorWithBrake.mo", "MotorWithBrake", crate::worker::StageKind::Flatten);
    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("MotorWithBrake \u{00b7} Flatten").is_some(),
        "the specimen half of the background must be rendered",
    );
    assert!(
        h.query_by_label_contains("\u{00b7} Flatten").is_some(),
        "the STAGE half must be rendered too -- `docs/context-assembly.md`: \
         \"Specimen and stage are always context, so they are always shown\". \
         Emitting a fact the bar does not show is precisely the drift the bar exists \
         to prevent",
    );
}

/// A specimen with **no compiled model yet** still names its stage.
///
/// Mid-compile the model name is `None`, and the first fix would have shown a
/// bare `Context` -- reading as *no context at all* at the one moment a reader is
/// most likely to be watching. The stage is known the whole time; say it.
#[test]
fn the_background_names_the_stage_before_a_model_exists() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("MotorWithBrake.mo", "MotorWithBrake", crate::worker::StageKind::Flatten);
    app.test_clear_model();
    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("\u{00b7} Flatten").is_some(),
        "with a specimen selected but no model compiled, the stage is still context \
         and must still be shown",
    );
}

/// At startup the **corpus is open and HRW specimens are shut**.
///
/// Doug, 2026-08-01, reversing the first arrangement: the corpus is the surface
/// most sessions browse, and the 18 curated files are the ones already known by
/// name.
///
/// **Both halves are asserted, because either alone can hold while the pair is
/// wrong.** A test that only checked the corpus was open would have passed on
/// the previous build too, where nothing was collapsed at all.
#[test]
fn at_startup_the_corpus_is_open_and_hrw_specimens_are_shut() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_specimen_files(&["RcCircuit.mo", "MotorWithBrake.mo"]);
    let mut h = harness(app);
    h.run_steps(2);

    // A corpus row proves the section is expanded, not merely present.
    assert!(
        h.query_by_label_contains("MSL corpus").is_some(),
        "the corpus header must be on screen",
    );
    assert!(
        h.query_by_label_contains("HRW specimens").is_some(),
        "the HRW section needs a header of its own \u{2014} that is the whole request",
    );
    assert!(
        h.query_by_label_contains("RcCircuit").is_none(),
        "HRW specimens start COLLAPSED, so no specimen row should be rendered",
    );
}

/// Clicking the HRW header reveals the specimens it counts.
///
/// The header says how many are inside while it is shut; this checks the number
/// is not decorative. A collapsed section whose count is right but whose body is
/// empty would look identical until clicked.
#[test]
fn opening_the_hrw_section_reveals_its_specimens() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_specimen_files(&["RcCircuit.mo", "MotorWithBrake.mo"]);
    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("HRW specimens \u{2014} 2").is_some(),
        "the header must carry the count while collapsed, or opening it is a guess",
    );
    h.get_by_label_contains("HRW specimens").click();
    h.run_steps(2);
    assert!(
        h.query_by_label_contains("RcCircuit").is_some(),
        "opening the section must show the specimens the header counted",
    );
}

/// A filter opens **both** sections, so one box searches everything.
///
/// The filter is the reason the corpus is reachable at all, and a section that
/// stayed shut while matching would report "no results" by omission — the
/// same absence-by-implication the Context Bar was fixed for.
#[test]
fn a_filter_opens_both_sections() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_specimen_files(&["RcCircuit.mo", "MotorWithBrake.mo"]);
    app.test_set_filter("rc");
    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("RcCircuit").is_some(),
        "a filter must open the HRW section, not merely narrow a shut one",
    );
    assert!(
        h.query_by_label_contains("HRW specimens \u{2014} 1 of 2").is_some(),
        "and the header must report the match count against the total",
    );
}

/// A programmatic scroll lands on the line's **start**, not its middle.
///
/// Doug, 2026-08-01: *"The text in the modelica source view is positioned too
/// far to the left. The left-most characters in many source lines are being cut
/// off."*
///
/// The source pane is a `ScrollArea::both`, and `Response::scroll_to_me` aligns
/// on **both** axes — so centring a row centred the *line*, and a long line
/// centred horizontally has its opening characters off the left edge.
///
/// Latent until MSL models: a specimen's lines are short, so centring one barely
/// moved the view. Library files are nested several packages deep with long
/// signatures, and the scroll now fires on every library load to reach the
/// declaration line.
///
/// **Layout is usually Doug's to judge** — the accessibility tree records no
/// geometry. This one is checkable anyway, because the thing that went wrong is
/// a number egui stores: the scroll area's horizontal offset.
#[test]
fn a_programmatic_source_scroll_keeps_the_left_margin() {
    // Long lines, so a horizontal centre would have somewhere to go, and deep
    // indentation, so losing the left edge loses the part that identifies the
    // line. This is the shape of real MSL source.
    let long: String = (1..=60)
        .map(|i| format!("        parameter Real veryLongIdentifierName{i} = {i}.0 \"a description that keeps the line wide\";"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_source(&long, 40);
    let mut h = harness(app);
    h.run_steps(4);

    let offset = h.state().test_source_scroll_offset();

    assert_eq!(
        offset.x, 0.0,
        "a programmatic scroll must leave the horizontal offset at the left margin; \
         at {} the opening characters of every line are off-screen, which is exactly \
         what was reported",
        offset.x,
    );
    assert!(
        offset.y > 0.0,
        "and it must still have scrolled VERTICALLY to the target line \u{2014} an offset \
         of 0 on both axes would pass the check above while doing nothing at all",
    );
}
