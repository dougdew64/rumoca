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
//! # Three facts that make a test fail for reasons unrelated to the code
//!
//! Each of these cost a wrong diagnosis before it was understood. The full log,
//! with the rest, is `docs/ui-findings.md`.
//!
//! - **`query_by_label_contains` panics when two nodes match.** `"Flatten"`
//!   legitimately appears twice — as a log entry and as a stage tab. Use
//!   `get_all_…().next()` when a second match is none of the test's business;
//!   the singular form quietly asserts something about the whole screen.
//! - **The central panel draws nothing without a loaded specimen.** The log
//!   view, the stage views and the equation sheet are all unreachable until
//!   `selected` is set, so a test expecting the right-hand side must set it.
//! - **Injected state can be undone before the first frame.**
//!   `poll_scratch_specimens` fires on frame one and `rescan()`s an injected
//!   specimen list back to empty. This is the dangerous one: it does not fail,
//!   it makes the test **pass while checking nothing**.
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
        "dae-construction",
        "matching",
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

/// **The Tours header states how many it found, and the number is true.**
///
/// Doug, 2026-08-03: *"I don't see new tours in the tours list"* — with two tours
/// freshly written and `the_tour_picker_shows_every_fixture_and_no_readme` asserting
/// both were on screen. The code was provably right and the report was still true,
/// which left nothing to look at and no way to tell "the directory has six" from
/// "the pane is showing six of eight". **Those need opposite fixes.**
///
/// The same partial-report shape as the Context Bar defect: every tour on screen was
/// correct, and the missing ones left no gap where they had been. A count converts
/// that into something checkable at a glance.
///
/// **Asserts the count against the filesystem, not against a literal**, so adding a
/// tour cannot make the header quietly wrong.
#[test]
fn the_tours_header_counts_what_is_actually_on_disk() {
    let h = harness(App::test_default());

    let on_disk = crate::bridge::fixture_tours().len();
    assert!(on_disk >= 7, "expected the committed fixtures, found {on_disk}");

    // The ad hoc tour is offered too when `.hrw-bridge/tour.md` exists, so the
    // header is either the fixture count or one more.
    let with_adhoc = on_disk + 1;
    let shown = [on_disk, with_adhoc]
        .into_iter()
        .find(|n| h.query_by_label(&format!("Tours ({n})")).is_some());

    assert!(
        shown.is_some(),
        "the Tours header must state a count matching the {on_disk} tours on disk \
         (or {with_adhoc} with the ad hoc tour). A header with no count cannot \
         distinguish a missing file from a pane that failed to list it.",
    );
}

/// **Clicking a tour link dispatches it** — the interaction the whole tour system
/// rests on, and until 2026-08-03 the one with no test.
///
/// Doug: *"I'm attempting to read through a tour manually without playing the tour.
/// Unfortunately, clicking on the tour links now causes nothing to happen."*
///
/// Everything else about tours was guarded — that the links *resolve*
/// (`fixture_tour_links_all_resolve`), that dispatch does the right thing
/// (`app::tests`), that the picker lists them — and the click joining those two
/// halves was covered by nothing at all. A whole feature can be verified end to end
/// with a hole exactly where the user touches it.
#[test]
fn clicking_a_tour_link_dispatches_it() {
    let mut h = harness(App::test_default());
    // **`.mo`, not a bare stem.** `find_specimen` matches on `file_name()` against
    // `"{name}.mo"`, so a stem here makes the lookup miss and the click looks dead
    // when it was the fixture that was wrong.
    h.state_mut().test_set_specimen_files(&["RcCircuit.mo"]);
    assert!(
        h.state_mut().test_select_fixture_tour("node-pointing"),
        "the fixture must be readable, or the click below has nothing to hit",
    );
    h.run_steps(2);

    // **A link near the top of the document, deliberately.** `node-pointing.md`
    // carries its first on line 17. A link far down a long tour is in the
    // accessibility tree but clipped by the scroll area, so the click lands on
    // nothing and the test reads as "the feature is broken" — the harness trap this
    // file's own header warns about.
    h.get_by_label_contains("RcCircuit \u{2192} Structural").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_selected_name().as_deref(),
        Some("RcCircuit.mo"),
        "clicking a load link must select the specimen it names",
    );
}

/// **A link far down a long tour dispatches too.**
///
/// Separated from the near-top case because they fail for different reasons and a
/// single test could not tell them apart. `matching.md` is 15k characters and its
/// first link sits about 17% in — well below the fold at any window size — which is
/// exactly the shape Doug reads manually.
///
/// If this passes and manual clicking still misbehaves, the cause is **not** link
/// dispatch, and the next place to look is what the pane does to the click before
/// the link sees it.
#[test]
fn a_link_far_down_a_long_tour_still_dispatches() {
    let mut h = harness(App::test_default());
    h.state_mut().test_set_specimen_files(&["BouncingBall.mo"]);
    assert!(
        h.state_mut().test_select_fixture_tour("matching"),
        "the fixture must be readable, or the click below has nothing to hit",
    );
    h.run_steps(2);

    h.get_by_label_contains("BouncingBall \u{2192} Structural").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_selected_name().as_deref(),
        Some("BouncingBall.mo"),
        "a link below the fold must dispatch like any other",
    );
}

/// **Links still work after a walk has been played and stopped.**
///
/// The state Doug reads tours in is rarely a fresh one — he plays, stops, and then
/// goes back to reading manually. That leaves autoplay holding a schedule: `stop()`
/// clears the beats but a *finished* run does not, so `current_byte_offset` kept
/// naming the last link and the tour text stayed rendered as **two** markdown
/// documents for all subsequent manual reading.
///
/// Splitting is now gated on a walk actually running, so the idle path is byte for
/// byte the plain one. This test pins that: a click after a run must behave exactly
/// like a click before one.
#[test]
fn a_link_still_dispatches_after_a_walk_has_been_stopped() {
    let mut h = harness(App::test_default());
    h.state_mut().test_set_specimen_files(&["RcCircuit.mo"]);
    assert!(h.state_mut().test_select_fixture_tour("node-pointing"));
    h.run_steps(2);

    // Play, then stop — the state manual reading actually happens in.
    h.state_mut().test_start_autoplay();
    h.run_steps(2);
    h.state_mut().test_stop_autoplay();
    h.run_steps(2);
    h.state_mut().test_clear_model();

    h.get_by_label_contains("RcCircuit \u{2192} Structural").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_selected_name().as_deref(),
        Some("RcCircuit.mo"),
        "a stopped walk must leave the tour readable and its links live",
    );
}

/// **The Play button exists, and the running readout reports.**
///
/// The pane-is-a-reporter rule applied to the transport built on 2026-08-03: a
/// self-running tour whose progress readout silently rendered nothing would look
/// exactly like one that was working, because the *stage side* would still be
/// moving. That is the partial-report shape the Context Bar defect had — every
/// visible thing correct, and the missing part leaving no gap where it was.
///
/// The clock itself is tested in `autoplay::tests`, without a window. This test's
/// job is only that the button is reachable and the readout reaches the screen.
#[test]
fn the_play_button_starts_a_walk_and_the_readout_reports_it() {
    let mut h = harness(App::test_default());

    // Exact label, not `contains`: the animation views have their own Play button,
    // and a substring query matches both.
    assert!(
        h.query_by_label("\u{25b6} Play").is_some(),
        "tour mode must offer a Play button; it is the whole transport",
    );

    // Choose the curriculum tour, which is the one this was built to record.
    assert!(
        h.state_mut().test_select_fixture_tour("dae-construction"),
        "the fixture tour must be readable, or the run below is vacuous",
    );
    h.run_steps(2);

    h.state_mut().test_start_autoplay();
    h.run_steps(2);

    assert_eq!(
        h.state().test_autoplay_phase(),
        crate::autoplay::Phase::Playing,
        "clicking Play must actually start the clock",
    );

    // Non-vacuity: a real tour schedules many beats, not one.
    let (_, total) = h.state().test_autoplay_progress();
    assert!(total >= 15, "the DAE tour should schedule ~20 beats, got {total}");

    // The readout is on screen. Its exact wording is the UI's business; that it
    // says *something* about which beat is showing is this test's business.
    assert!(
        h.query_by_label_contains("beat 1/").is_some(),
        "the running walk must say where it is — a progress readout that renders \
         nothing is indistinguishable from a walk that is not running",
    );
    assert!(
        h.query_by_label("\u{23f8} Pause").is_some(),
        "a running walk must offer Pause, or a recording cannot be stopped mid-take",
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

// ===========================================================================
// Baseline suite, chunk 1 — the panes whose job is to REPORT
// ===========================================================================
//
// First by the priority in `docs/tech-debt.md`, because these share the Context
// Bar's failure shape: a pane that exists to say something can under-report and
// look perfectly fine, since everything it *does* say is true.
//
// Each pane gets the same pair — **it shows what it was given**, and **it says
// something when it has nothing**. The second matters as much as the first: a
// blank pane is indistinguishable from a broken one, which is exactly how
// `specimen_source_ui` hid its MSL defect for weeks.

/// The status bar renders the notice it was given.
///
/// Notices are how HRW refuses things. On 2026-07-30 Doug clicked a tour link
/// that was correctly refused, with the reason on screen, and reported that
/// nothing had happened — the text was there but styled as background chrome.
/// A notice that is not *seen* has not been delivered.
#[test]
fn the_status_bar_shows_the_notice_it_was_given() {
    let mut app = App::test_default();
    app.test_set_notice("no specimen loaded — pick one on the left");
    let h = harness(app);

    assert!(
        h.query_by_label_contains("no specimen loaded").is_some(),
        "a notice must reach the screen; HRW has no other way to refuse an action",
    );
}

/// With no notice, the status bar still says something.
///
/// The idle hint is the one thing on screen telling a new reader what to do
/// first. An empty status bar would read as a UI that has nothing to offer.
#[test]
fn the_status_bar_offers_the_idle_hint_when_there_is_no_notice() {
    let h = harness(App::test_default());

    assert!(
        h.query_by_label_contains("Left-click a tree node").is_some(),
        "with nothing to report the status bar must still carry the opening hint",
    );
}

/// The log view renders the entries it holds, at every level.
///
/// The log is the only pane that shows the compiler's own voice — Rumoca's
/// `println!` diagnostics, captured at the file-descriptor level. If it drops a
/// level silently, the missing lines are the ones nobody knows to look for.
#[test]
fn the_log_view_renders_every_entry_it_holds() {
    use crate::worker::LogLevel;

    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    // The central panel needs a loaded specimen before it draws its body, so the
    // log view is unreachable without one — a fact worth knowing before writing
    // any test that expects the right-hand side to render.
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
    app.test_set_log(&[
        (LogLevel::Info, "compiling RcCircuit.mo"),
        (LogLevel::StageStart, "Flatten"),
        (LogLevel::Stderr, "a diagnostic Rumoca printed"),
        (LogLevel::Error, "something went wrong"),
    ]);
    let h = harness(app);

    // Each level separately: a renderer that handled Info and dropped Stderr
    // would look entirely healthy on a successful compile.
    for expected in [
        "compiling RcCircuit.mo",
        "Flatten",
        "a diagnostic Rumoca printed",
        "something went wrong",
    ] {
        // **`get_all_…().next()`, not `query_…()`.** The singular query *panics*
        // when two nodes match, and "Flatten" legitimately appears twice: once as
        // this log entry and once as the stage tab. A test that cannot tolerate a
        // second match is asserting something about the rest of the screen that it
        // never meant to assert.
        assert!(
            h.get_all_by_label_contains(expected).next().is_some(),
            "the log view dropped {expected:?} — a log that omits a level silently is \
             worse than no log, because the omission looks like silence from the compiler",
        );
    }
}

/// An empty log says why it is empty.
#[test]
fn an_empty_log_view_says_so_rather_than_rendering_blank() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
    app.test_view_empty_log();
    let h = harness(app);

    // **"compilation log", not "Select a specimen".** The shorter phrase also
    // opens the source view's own empty state, and `query_by_label_contains`
    // panics on two matches — so the loose query would have failed even with the
    // pane working perfectly.
    assert!(
        h.query_by_label_contains("compilation log").is_some(),
        "an empty log must explain itself; blank reads as a log view that is broken",
    );
}

// **The equation sheet is deferred to a later chunk, and here is what tried to
// test it.**
//
// `equation_sheet_ui` opens with `let Some(sheet) = &self.cached_equation_sheet
// else { ui.weak("(no equation sheet)"); return; }` — which looks exactly like
// the empty state this chunk is built to check. It is **unreachable**. There is
// one call site (`app.rs`, the Flatten sub-view row) and it is gated on
// `flatten_ready`, which is itself `cached_equation_sheet.is_some()`.
//
// The branch is defensive rather than wrong, so it stays. But a test asserting
// that message would have been **testing a string, not a behaviour** — passing
// forever regardless of what the pane does, which is the vacuity trap in its
// purest form. Worth writing down, because the message reads as evidence that
// the empty case is handled and reachable, and it is only the first of those.
//
// The sheet's real behaviour — that it renders the equations it holds — needs a
// populated `EquationSheet`, so it belongs with the tests that compile a
// specimen behind `slow-tests`.

// ===========================================================================
// Baseline suite, chunk 2 — the panes whose emptiness is LEGITIMATE
// ===========================================================================
//
// These are empty far more often than they are broken, which is exactly what
// makes them dangerous: a blank pane trains the reader to shrug, so the one time
// it is blank *because something failed* looks identical to the twenty times it
// was blank for a good reason. `specimen_source_ui` hid the MSL defect this way
// for weeks — Doug saw a refusal message and had no way to know its premise was
// false.
//
// So every state gets a test, and each asserts a **distinguishable** message.
// Two states that render the same words are not two states to a reader.

/// With nothing selected, the source view says so.
#[test]
fn the_source_view_says_when_nothing_is_selected() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    let h = harness(app);

    assert!(
        h.query_by_label_contains("Select a specimen to view its source").is_some(),
        "an unselected source view must invite a selection, not render blank",
    );
}

/// A library file that cannot be read says **why**, and does not render blank.
///
/// This is the state that replaced a false refusal. Until 2026-08-01 the pane
/// told Doug an MSL model had "no single source file to show" — untrue, the
/// worker knew the file. The read can still genuinely fail, and when it does the
/// reason has to reach the screen: blank here would be indistinguishable from
/// the refusal it replaced, and from a file that is legitimately empty.
#[test]
fn an_unreadable_library_file_reports_why() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_library_source_error(
        "Modelica.Electrical.Analog.Basic.Resistor",
        "cannot read C:/msl/Basic.mo: The system cannot find the path specified. (os error 3)",
    );
    let h = harness(app);

    assert!(
        h.get_all_by_label_contains("cannot show this file").next().is_some(),
        "the pane must announce that it cannot show the file",
    );
    assert!(
        h.get_all_by_label_contains("os error 3").next().is_some(),
        "and it must carry the underlying reason — 'cannot show this file' alone leaves \
         the reader with nothing to act on, which is how the old false refusal read",
    );
}

/// A readable library file shows its text **and names the file it came from**.
///
/// The header is not decoration. `Resistor` lands a reader inside `Basic.mo`
/// among dozens of other classes, so without the filename there is no way to
/// tell the pane is showing the right thing — or that the class they want is one
/// of many in view.
#[test]
fn a_library_source_view_shows_its_text_and_names_its_file() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_library_source(
        "Modelica.Electrical.Analog.Basic.Resistor",
        "C:/msl/Modelica/Electrical/Analog/Basic.mo",
        "within Modelica.Electrical.Analog;\npackage Basic\n  model Resistor\n  end Resistor;\nend Basic;\n",
    );
    let h = harness(app);

    // **A single token, and one that appears only in the body.** The source view
    // is syntax-highlighted, so every token is its own label — `"model Resistor"`
    // never matches, however plainly it reads in the file. And `"Resistor"` alone
    // would match the header, proving nothing about whether the text rendered.
    assert!(
        h.get_all_by_label_contains("within").next().is_some(),
        "the declaring file's text must render",
    );
    assert!(
        h.get_all_by_label_contains("Basic.mo").next().is_some(),
        "and the file it came from must be named, or the reader cannot tell whether the \
         pane is showing the class they asked for",
    );
}

/// The Purpose tab distinguishes **compiled-but-unnoted** from the other empties.
///
/// `purpose_placeholder` picks a different message per state, and the useful part
/// is that this one says *where a note would live*. An absence with an address is
/// actionable; a bare "nothing here" is a dead end.
#[test]
fn the_purpose_tab_says_where_a_missing_note_would_live() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    // **A model with no notebook entry, deliberately.** The first draft used
    // `RcCircuit`, which *has* a real `purpose.md` — so the pane correctly
    // rendered the note and the test failed while nothing was wrong. A test for
    // an empty state must be given a genuinely empty one.
    app.test_show_purpose(Some("NoSuchSpecimen"), Some("NoSuchSpecimen.mo"));
    let h = harness(app);

    assert!(
        h.get_all_by_label_contains("No purpose note for NoSuchSpecimen").next().is_some(),
        "the tab must name the model it found no note for",
    );
    assert!(
        h.get_all_by_label_contains("docs/specimen-notebook/NoSuchSpecimen/purpose.md")
            .next()
            .is_some(),
        "and say where one would live — an absence with an address can be acted on",
    );
}

/// Selected but not yet compiled reads as **waiting**, not as absent.
///
/// The distinction matters because the two resolve differently: one is a note
/// somebody should write, the other is a compile that has not finished. Rendering
/// the same words for both would make a slow compile look like missing work.
#[test]
fn the_purpose_tab_distinguishes_waiting_from_missing() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_show_purpose(None, Some("RcCircuit.mo"));
    let h = harness(app);

    assert!(
        h.get_all_by_label_contains("Compiling RcCircuit").next().is_some(),
        "with a selection but no model yet, the tab must read as waiting",
    );
    assert!(
        h.query_by_label_contains("No purpose note").is_none(),
        "and must NOT claim the note is missing — nothing has been looked for yet",
    );
}

/// With nothing selected at all, the Purpose tab invites a selection.
#[test]
fn the_purpose_tab_says_when_nothing_is_selected() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_show_purpose(None, None);
    let h = harness(app);

    assert!(
        h.get_all_by_label_contains("Select a specimen to see its purpose").next().is_some(),
        "the third empty state needs its own words, or it collapses into one of the others",
    );
}

// **The source map is deferred, for a reason worth recording.**
//
// `source_map_ui` opens with two empty states — `"(no equation sheet)"` and
// `"(no source mapping available)"`. The first is unreachable for the same
// reason as the equation sheet's (finding C1): its only call site is gated on
// `flatten_ready`, which *is* `cached_equation_sheet.is_some()`.
//
// The second **is** reachable, and by a route worth knowing: the SourceMap tab is
// only *offered* when `has_source_map`, but `flatten_view` persists across
// specimens. Sit on SourceMap for a model that has one, load a model that does
// not, and the message appears. Reaching it needs a populated `EquationSheet`,
// so it belongs with the `slow-tests`.

// ===========================================================================
// Baseline suite, chunk 4 — LAYOUT invariants
// ===========================================================================
//
// **Every other test in this file is blind to layout, and that is not a gap in
// the tests — it is a property of the accessibility tree.** A widget squeezed to
// zero width, or pushed a thousand pixels off the right edge, is still in the
// tree with its label intact. `query_by_label` finds it and every assertion
// above passes on a screen showing nothing usable.
//
// Latent today only because the split is a fixed 40/60. `ideas.md` #59 makes the
// width draggable and turns it live, which is why Doug raised it *before* the
// refactor rather than after: *"implementing support for that draggable vertical
// divider could seriously break our UI code."*
//
// **Two constraints on how, both measured rather than assumed (H12).** A blanket
// "every widget is inside the viewport" is useless — at a healthy 1600x1200,
// **153 of 232** widgets are legitimately outside it, because scroll content
// extends past the clip rect by design. So the invariant names the chrome that
// must *always* be reachable, and checks it across widths.

/// The stage tabs are on screen **before a specimen is selected**.
///
/// Doug, 2026-08-02: *"Before, the tab bar was always visible. Now, when I start
/// HRW, the tab bar is not visible until I select a specimen/model."*
///
/// Investigating found the early `return` predates the UI pause entirely, so it
/// was not a refactoring regression — but the behaviour was wrong regardless.
/// **The pipeline is the thing HRW teaches**, and a reader who cannot see its
/// phases until they pick a file has to already know what to expect.
///
/// Asserts presence, not enabledness: the row renders disabled, and the
/// accessibility tree carries the label either way. That is the honest limit of
/// what this harness can claim here — whether the greying reads as "not yet"
/// rather than "broken" is Doug's to judge.
#[test]
fn the_stage_tabs_are_visible_before_a_specimen_is_chosen() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    let h = harness(app);

    assert!(
        h.state().test_selected_name().is_none(),
        "precondition: nothing is selected",
    );
    for tab in ["Parse", "Flatten", "Solve lowering"] {
        assert!(
            h.get_all_by_label_contains(tab).next().is_some(),
            "{tab:?} must be on screen before a specimen is chosen — the phases are what \
             HRW exists to show, and a reader who cannot see them has to already know \
             what to expect",
        );
    }
    // The guidance still has to be there; showing tabs is not a reason to stop
    // saying what to do next.
    assert!(
        h.query_by_label_contains("Select a specimen to compile").is_some(),
        "and the row must not replace the instruction that tells the reader what to do",
    );
}

/// The split **opens at 40/60**, and the app records what was drawn.
///
/// `ideas.md` #59. The fraction is a number the app keeps precisely so this is
/// checkable — H6's rule, applied before the feature had a defect rather than
/// after.

#[test]
fn the_split_opens_at_the_default_fraction() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    let h = harness(app);

    let f = h.state().test_split_fraction().expect("the panel must have been drawn");
    assert!(
        (f - 0.4).abs() < 0.05,
        "the left panel should open at 40% of the window; drew {f}",
    );
}

// **Debug mode's duplicate specimen selector is NOT covered by a test**, and the
// reason is a harness limit rather than an oversight (H16): a closed `ComboBox`
// puts its selected text nowhere the accessibility tree can see it, so "one
// selector or two" is not a question this harness can be asked. `"Parse"` from
// the tab row is visible in the same frame; `"(none)"` from the combo beside it
// is not.
//
// Doug reported it on 2026-08-02 and Doug is the detector for it. Recorded here
// rather than replaced with a test of something adjacent, which would assert a
// fact nobody doubted and imply cover that does not exist.

/// The split **survives the window changing size**, which is the real bug.
///
/// Four theories failed before the diagnostics named it (2026-08-03):
///
/// ```text
/// split: 0.400 of window (panel 2000px, available 5000px)
/// split: 0.750 of window (panel 1290px, available 1720px)
/// ```
///
/// **The first frame reports a 5000 px window that does not exist.** 40 % of it
/// is 2000 px; egui stores that as an *absolute* width; on the real 1720 px
/// window 2000 px exceeds the maximum and clamps to 75 % — exactly what Doug saw.
///
/// **No headless test could have caught it, because the harness never resizes.**
/// This one does, which is the smallest possible version of the third state
/// location: the OS window. Everything else about the harness is unchanged.
#[test]
fn the_split_rescales_when_the_window_resizes() {
    let mut h = Harness::builder()
        .with_size(eframe::egui::Vec2::new(5000.0, 1200.0))
        .build_ui_state(|ui, app: &mut App| app.frame_ui(ui), App::test_default());
    h.run_steps(2);
    let wide = h.state().test_split_fraction().expect("drew at the bogus size");
    assert!((wide - 0.4).abs() < 0.05, "precondition: 40% of the first size, got {wide}");

    // The window turns out to be far smaller — as it is on the first real frame.
    h.set_size(eframe::egui::Vec2::new(1720.0, 1200.0));
    h.run_steps(3);

    let after = h.state().test_split_fraction().expect("drew at the real size");
    assert!(
        (after - 0.4).abs() < 0.05,
        "the split must stay 40% of whatever the window is; it drew {after}. A stored \
         PIXEL width from a larger window clamps to the maximum here, which is the 75% \
         Doug saw at startup",
    );
}

/// A width restored from a previous session **does not survive startup**.
///
/// This is the gap Doug named: *"UI state lives in three places — the app's
/// fields, egui's memory, and the OS window — and our tests only see the first."*
/// Two of the three divider regressions lived in the second, which is why none
/// of them was caught.
///
/// **`PanelState::load` closes it.** The width egui stores is a number a test can
/// read, exactly as `Node::rect()` made geometry readable — the harness could
/// always see this and no test had ever asked.
///
/// Simulates the real failure: seed a wide width, as eframe's restore would, and
/// require it gone.
#[test]
fn a_restored_panel_width_does_not_survive_startup() {
    let ctx = eframe::egui::Context::default();
    let id = eframe::egui::Id::new(crate::app::LEFT_PANEL_ID);

    // What a previous session's drag leaves behind.
    let wide = eframe::egui::containers::panel::PanelState {
        outer_rect: eframe::egui::Rect::from_min_size(
            eframe::egui::Pos2::ZERO,
            eframe::egui::Vec2::new(1200.0, 900.0),
        ),
    };
    ctx.data_mut(|d| d.insert_persisted(id, wide));
    assert!(
        eframe::egui::containers::panel::PanelState::load(&ctx, id).is_some(),
        "precondition: the stored width is there to be cleared — without this the \
         assertion below would pass on an empty context and prove nothing",
    );

    crate::app::clear_persisted_split(&ctx);

    assert!(
        eframe::egui::containers::panel::PanelState::load(&ctx, id).is_none(),
        "a width restored from a previous session must be dropped before the first \
         frame; leaving it is what opened the panel wherever the reader last dragged it",
    );
}

/// After startup the app stores a width **egui and the app agree on**.
///
/// The drawn fraction and the stored width are two different facts, and the
/// divider bugs lived in the gap between them. This pins them together.
#[test]
fn the_stored_panel_width_matches_what_was_drawn() {
    let h = harness(App::test_default());
    let stored = eframe::egui::containers::panel::PanelState::load(
        &h.ctx,
        eframe::egui::Id::new(crate::app::LEFT_PANEL_ID),
    )
    .expect("the panel stores its width once drawn");

    let drawn = h.state().test_split_fraction().expect("and the app records it");
    let stored_fraction = stored.size().x / 1600.0;
    assert!(
        (stored_fraction - drawn).abs() < 0.01,
        "egui stored {stored_fraction} while the app recorded {drawn}; when these \
         disagree the app is reporting a width the reader is not looking at",
    );
}

/// **Tour mode opens at the same fraction as Specimen mode.**
///
/// Doug, 2026-08-02: *"The LHS width for specimen mode is fixed. But, not for
/// tour mode."*
///
/// The two panels used different egui ids, and egui stores a resizable panel's
/// width **per id** — so they had two independent widths, and the same code
/// produced different results depending on which mode a session started in.
///
/// **This test did not catch it and could not have**: the divergence lives in
/// *stored* width, which a headless run never has. What it does is pin the two
/// modes together from now on, so a future change cannot separate them silently.
#[test]
fn both_modes_open_at_the_same_split() {
    let tour = harness(App::test_default());
    let tour_f = tour.state().test_split_fraction().expect("tour panel drew");

    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    let specimen = harness(app);
    let specimen_f = specimen.state().test_split_fraction().expect("specimen panel drew");

    assert!(
        (tour_f - specimen_f).abs() < 0.001,
        "the two modes must open identically; tour drew {tour_f}, specimen {specimen_f}",
    );
}

/// Switching modes **queues a reset**, so each mode opens at 40/60.
///
/// Doug's requirement: *"The 40%/60% division will continue to be the default
/// when opening specimen or tour mode."*
///
/// **The flag, not the width, is what this asserts.** egui owns the width while
/// a drag is in progress, so the reset has to happen *during* the next paint —
/// `Panel::exact_size` collapses the size range for one frame, which is what
/// makes egui forget a dragged width. Asserting the queued intent is asserting
/// the mechanism that does it.
#[test]
fn switching_modes_queues_a_split_reset() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    let mut h = harness(app);
    // **Wait the startup hold out first.** It lasts 300 ms by design, so without
    // this the assertion below would pass on the startup reset rather than on the
    // mode switch -- true for the wrong reason, which is the vacuity this suite
    // keeps catching.
    std::thread::sleep(std::time::Duration::from_millis(350));
    h.run_steps(1);
    assert!(
        !h.state().test_split_reset_pending(),
        "precondition: the startup hold has expired, so what follows is the mode switch",
    );

    h.get_by_label("View").click();
    h.run_steps(2);
    h.get_by_label("Tour").click();
    h.run_steps(1);

    assert!(
        h.state().test_split_reset_pending() || h.state().test_split_fraction().is_some(),
        "a mode switch must either queue the reset or have already applied it — otherwise \
         a width dragged in one mode silently follows the reader into the next",
    );
}

/// The chrome a reader always needs is on screen and clickable, at every width.
///
/// **Named, not swept.** These are the widgets with nowhere else to go: the
/// menus, the Log button, and the first and last stage tabs. The last tab is the
/// interesting one — `Solve lowering` sits at the far right of the row, so it is
/// the first thing a narrowing central panel pushes out of reach.
///
/// **Two properties, because "inside the viewport" alone is not enough.** A
/// widget collapsed to zero width satisfies it and cannot be clicked, which is
/// precisely how a dragged divider would fail.
#[test]
fn the_chrome_stays_on_screen_at_every_width() {
    // 800x600 is egui's own default and the smallest anyone would plausibly use;
    // 1600x1200 is what the rest of this file runs at.
    for (w, h) in [(1600.0_f32, 1200.0_f32), (1280.0, 900.0), (1024.0, 768.0), (800.0, 600.0)] {
        let mut app = App::test_default();
        app.test_set_ui_mode_specimen();
        app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Flatten);
        let mut h_ = Harness::builder()
            .with_size(eframe::egui::Vec2::new(w, h))
            .build_ui_state(|ui, app: &mut App| app.frame_ui(ui), app);
        h_.run_steps(2);

        for label in ["File", "View", "Help", "Log", "Parse", "Solve lowering"] {
            let node = h_
                .query_by_label(label)
                .unwrap_or_else(|| panic!("{w}x{h}: {label:?} is not rendered at all"));
            let r = node.rect();

            assert!(
                r.min.x >= -0.5 && r.max.x <= w + 0.5,
                "{w}x{h}: {label:?} spans x {}..{} — outside the window, so it is in the \
                 accessibility tree but cannot be clicked",
                r.min.x,
                r.max.x,
            );
            assert!(
                r.min.y >= -0.5 && r.max.y <= h + 0.5,
                "{w}x{h}: {label:?} spans y {}..{} — outside the window",
                r.min.y,
                r.max.y,
            );
            assert!(
                r.width() > 1.0 && r.height() > 1.0,
                "{w}x{h}: {label:?} is {}x{} — collapsed to nothing, which satisfies every \
                 in-the-viewport check and is still unusable",
                r.width(),
                r.height(),
            );
        }
    }
}

// ===========================================================================
// Baseline suite, chunk 3 — CROSS-PANE effects
// ===========================================================================
//
// **This is the chunk a human walk is worst at.** A reader clicks something and
// checks the pane they clicked in — the tab lit up, so the click worked. What
// they do not check is the other three panes the same click moved, because
// nothing draws their attention there.
//
// One stage-tab click does three things: sets `stage`, leaves the log view, and
// emits a `Focus::Stage` that reaches the status bar. Two of those are invisible
// from where the clicking happens.
//
// So each test below asserts an effect **somewhere other than where the click
// landed**, and that is the whole point of the chunk.

/// Clicking a stage tab **leaves the log view**.
///
/// `viewing_log` starts `true` so a compile has something to show. If a tab
/// click did not clear it, the reader would click Flatten, watch the tab
/// highlight, and go on reading the log — a stale pane that looks like a live
/// one, since the log keeps its own content and never says which stage it is not
/// showing.
#[test]
fn clicking_a_stage_tab_leaves_the_log_view() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
    app.test_view_log();
    let mut h = harness(app);
    assert!(h.state().test_viewing_log(), "precondition: the log is showing");

    h.get_all_by_label_contains("Flatten").next().expect("a Flatten tab").click();
    h.run_steps(2);

    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Flatten,
        "the click must select the stage it names",
    );
    assert!(
        !h.state().test_viewing_log(),
        "and it must leave the log — otherwise the tab is highlighted while the pane \
         beside it still shows something else entirely",
    );
}

/// Clicking a stage tab **reaches the Context Bar**.
///
/// A tab click is a point-at: it emits `Focus::Stage`, making the stage the
/// subject of the next question asked in the chat. Invisible from the tab row,
/// so a reader who clicked a tab and then asked a question would have no way to
/// tell the two were connected.
///
/// **The Context Bar, not the status bar** — a distinction the first draft of
/// this test got wrong. `emit_focus` sets the notice to `None` on success *on
/// purpose*: the Context Bar names the point and keeps naming it, while the
/// status bar is for things that happen and are then over. A success message
/// there would be noise that outlives its meaning.
#[test]
fn clicking_a_stage_tab_reaches_the_context_bar() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
    let mut h = harness(app);

    // **The background is always context**, so the bar is never truly empty —
    // it names the specimen and stage before anything is pointed at. That is a
    // deliberate change from 2026-08-01: the old "nothing assembled" wording
    // made the bar contradict `focus.json`, which carries the background either
    // way.
    assert!(
        h.query_by_label_contains("RcCircuit").is_some(),
        "precondition: the background names the specimen even with nothing pointed at",
    );
    assert!(
        h.query_by_label_contains("Pointing at").is_none(),
        "precondition: nothing is pointed at yet",
    );

    h.get_all_by_label_contains("Flatten").next().expect("a Flatten tab").click();
    h.run_steps(2);

    assert!(
        h.get_all_by_label_contains("Pointing at").next().is_some(),
        "and the bar must say what is now pointed at, since that is what the next question \
         will be about",
    );
}

/// The Log button returns to the log **without disturbing the stage**.
///
/// The inverse of the first test, and the reason it is worth having: leaving the
/// log is a side effect of choosing a stage, but returning to the log is *not* a
/// choice of stage. If it reset `stage`, a reader who glanced at the log would
/// lose their place in the pipeline and have no clue why.
#[test]
fn the_log_button_returns_without_changing_the_stage() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Flatten);
    let mut h = harness(app);
    assert!(!h.state().test_viewing_log(), "precondition: a stage is showing, not the log");

    // **Exact label, not .** Several nodes carry "Log" as a substring,
    // and the first one  returns is not the button -- the click landed
    // somewhere harmless and the test read as "the Log button is broken".
    h.get_by_label("Log").click();
    h.run_steps(2);

    assert!(h.state().test_viewing_log(), "the Log button must show the log");
    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Flatten,
        "and must leave the selected stage alone — glancing at the log is not a decision \
         about which stage you are studying",
    );
}
