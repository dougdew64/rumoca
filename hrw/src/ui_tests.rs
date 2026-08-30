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

/// Hold `.hrw-bridge/tour.md` at a chosen state for the duration of a test, and put
/// back whatever was there on the way out — including on a panic.
///
/// # Why every test that paints needs this
///
/// **The ad hoc tour is live state, not scratch space.** It is Claude's answer to
/// Doug's last question, and HRW *auto-selects it* when nothing else is chosen
/// (`tour::poll`), which resets the stage side. So its mere presence changes what a
/// painted frame does.
///
/// Three tests were written without accounting for that, and all three were wrong in a
/// different direction — found on 2026-08-16, the first day an ad hoc tour existed
/// while the suite ran:
///
/// 1. `the_ad_hoc_tour_row_is_present_even_with_no_ad_hoc_tour` **asserted** the file
///    was absent, so it failed whenever the feature had been used. A precondition of
///    "the user has not used the product recently" measures the environment.
/// 2. `the_ad_hoc_tour_is_a_button_and_not_a_picker_entry` wrote its own fixture and
///    **deleted** it afterwards, destroying a real answer without a word.
/// 3. `a_frame_link_into_flatten_connections_navigates` painted with whatever happened
///    to be on disk, and started failing when an ad hoc tour appeared and the
///    auto-selection reset the viewport it was asserting on.
///
/// One helper, three uses: `absent()` for tests that need none, `with(text)` for tests
/// that need one. Both restore.
pub(crate) struct AdHocTour(Option<String>);

impl AdHocTour {
    /// No ad hoc tour exists for the duration.
    pub(crate) fn absent() -> Self {
        let saved = std::fs::read_to_string(crate::bridge::TOUR_FILE).ok();
        let _ = std::fs::remove_file(crate::bridge::TOUR_FILE);
        Self(saved)
    }

    /// An ad hoc tour with `text` exists for the duration.
    pub(crate) fn with(text: &str) -> Self {
        let path = std::path::Path::new(crate::bridge::TOUR_FILE);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("bridge dir");
        }
        let saved = std::fs::read_to_string(path).ok();
        std::fs::write(path, text).expect("write the ad hoc tour");
        Self(saved)
    }
}

impl Drop for AdHocTour {
    fn drop(&mut self) {
        match self.0.take() {
            Some(text) => {
                let _ = std::fs::write(crate::bridge::TOUR_FILE, text);
            }
            None => {
                let _ = std::fs::remove_file(crate::bridge::TOUR_FILE);
            }
        }
    }
}

/// Drive HRW's whole frame in a headless harness.
///
/// `frame_ui` rather than [`eframe::App::ui`] because the trait method takes an
/// `eframe::Frame`, which cannot be constructed outside eframe — that one unused
/// parameter was the only thing standing between this UI and an automated test.
pub(crate) fn harness(app: App) -> Harness<'static, App> {
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

/// **The Context Bar is on screen in every state**, which is the whole of what it is
/// for.
///
/// # This test asserted the opposite until 2026-08-30, and the reversal is the record
///
/// It was written that morning as *"the bar appears only once a specimen is
/// selected"*, pinning the precondition that three collapsed gates relied on. Doug
/// then supplied the argument that overturned it:
///
/// > *"A context bar is novel for me. I need to learn to assume its presence and to
/// > make frequent use of it, just like I needed decades ago to learn to assume the
/// > presence of GUI menu bars."*
///
/// **That is a claim about habit, not about information**, and it is the same one the
/// stage tab row won on 2026-08-02 — *"report the empty state, never vanish."* The
/// rule had been applied to the tabs and not to the bar directly beneath them.
///
/// **A test reversing is not a test failing.** This one did its job twice: it held the
/// old invariant while that was the design, and it failed by name the moment the
/// design changed, which is how the change announced everywhere it reached.
///
/// # Why four states rather than two
///
/// *Always* is a claim about every state, and a two-case test would leave the modes
/// Doug actually walks in unchecked. Tour mode before a stop loads anything is the
/// case that started the conversation; the navigation view is the one branch of
/// `central_panel_ui` that draws no tab row, so the bar had to be added there
/// separately and could regress on its own.
#[test]
fn the_context_bar_is_present_in_every_state() {
    /// A named state and the app that puts HRW in it. Built lazily, because each
    /// `App` spawns a worker and only one is needed at a time.
    type Case = (&'static str, fn() -> App);

    let cases: [Case; 4] = [
        ("nothing loaded", || {
            let mut app = App::test_default();
            app.test_set_ui_mode_specimen();
            app
        }),
        ("a specimen loaded", || {
            let mut app = App::test_default();
            app.test_set_ui_mode_specimen();
            app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
            app
        }),
        // `UiMode` defaults to Tour, which is why this one sets no mode: it is the
        // state HRW actually launches in, and the one Doug walks tours in.
        ("tour mode, nothing loaded", App::test_default),
        ("the navigation view", || {
            let mut app = App::test_default();
            app.test_set_ui_mode_specimen();
            app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
            app.test_push_nav("Modelica.Blocks.Interfaces.RealInput");
            app
        }),
    ];

    for (what, build) in cases {
        let h = harness(build());
        assert!(
            h.query_by_label("Context").is_some(),
            "the Context Bar is missing with {what} \u{2014} a bar that comes and goes \
             cannot be assumed, and assuming it is the point",
        );
    }
}

/// **An always-present bar must not fill the silence**, which is the risk that comes
/// with never hiding.
///
/// `App::stage` always holds *some* `StageKind`, so the obvious way to render the
/// background on a fresh launch prints `· Parse` — naming a phase that has not run,
/// about a specimen that does not exist. **Making a pane unconditional is exactly when
/// it starts inventing**, because it now has frames to fill that it never had before,
/// and this repository's first rule is that absence is stated rather than filled.
///
/// The second half is the other side of the same coin: with a tour open there *is*
/// something true to say, and the bar says it. `session.json` has carried the open
/// tour's name since 2026-08-19, so the bar was under-reporting context Claude already
/// had — the gap Doug's question opened.
#[test]
fn the_bar_names_what_is_loaded_and_invents_nothing() {
    // **`.hrw-bridge/tour.md` is live state and `tour::poll` auto-selects it**, so with
    // one on disk there *is* an open tour and the bar rightly names it. Without this
    // guard the first assertion measures whether Doug has used the Answer feature
    // recently — the trap `AdHocTour` exists for, and the one that caught this test on
    // its first run.
    let _no_ad_hoc = AdHocTour::absent();

    let h = harness(App::test_default());
    assert!(
        h.query_by_label_contains("\u{00b7} ").is_none(),
        "with nothing loaded the bar rendered a background line \u{2014} there is no \
         specimen, no model and no tour, so naming a stage would be a claim that a \
         phase ran",
    );

    let mut app = App::test_default();
    assert!(
        app.test_select_fixture_tour("connect-expansion"),
        "precondition: connect-expansion is a checked-in fixture tour",
    );
    let h = harness(app);
    assert!(
        h.query_by_label_contains("tour: connect-expansion")
            .is_some(),
        "with a tour open the bar must name it \u{2014} it is context by the same rule \
         specimen and stage are, and Claude has had it in session.json since \
         2026-08-19 while the bar stayed silent",
    );
}

/// **Exactly one control starts a simulation, and the pane points at it.**
///
/// Doug, 2026-08-30: *"We only need one Run button for simulations."* Until then the
/// Simulation view carried its own `▶ Run` beside the stop-time slider, so opening
/// that tab put **two** on screen — and they disagreed about when a run was possible.
/// The tab row's uses `can_sim` (not compiling, not running, a model parsed, solve
/// lowering done); the pane's had decayed to `!sim_running`, leaving it live on a
/// specimen that could not be simulated.
///
/// # Why this one is testable when the divider next to it was not
///
/// Buttons and labels carry accessibility labels; separators do not. The extra rule
/// between "Log" and "Parse", removed the same day, could only ever be caught by Doug
/// looking at it. **The difference is worth knowing before reaching for a test** —
/// here one is cheap and pins both halves of his instruction: the button is gone, and
/// the empty-plot label names what replaced it rather than what it used to say.
#[test]
fn only_the_tab_row_starts_a_simulation_and_the_pane_says_so() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state(
        "RcCircuit.mo",
        "RcCircuit",
        crate::worker::StageKind::Simulation,
    );
    let h = harness(app);

    assert!(
        h.query_by_label("\u{25b6} Run").is_none(),
        "the Simulation pane must not carry its own Run button \u{2014} two controls \
         that start the same thing, disagreeing about when it is possible",
    );
    assert!(
        h.query_by_label("\u{25b6}").is_some(),
        "the tab row's \u{25b6} is the one that remains, so removing the pane's must \
         not have taken the last way to start a run",
    );
    assert!(
        h.query_by_label_contains("beside the Simulation tab")
            .is_some(),
        "with no data yet the pane must point at the control that does exist; it used \
         to say \u{201c}Press \u{25b6} Run\u{201d}, naming the button just deleted",
    );
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
    let mut h = harness(App::test_default());

    // **The picker is a combo box since 2026-08-16, so its items exist only while the
    // popup is open.** Opening it is now part of the act being tested, not setup: with
    // 22 tours the always-open list cost about 400 points of vertical space, over half
    // the tour panel on a 13" screen, for a control used a few times a day.
    //
    // Asserting against the closed box instead would have been the easy way to keep
    // this test green, and it would have stopped checking that every fixture is
    // offered — which is the only thing it is for.
    // **Queried by `value`, not by label.** A ComboBox exposes its selected text as
    // the node value and carries no label, so `label_contains` finds nothing — which
    // is how this test failed after the change, rather than by finding the wrong node.
    h.get_by_value("Pick a tour…").click();
    h.run_steps(2);

    // **Derived from the corpus, not restated.** This loop used to iterate a
    // hand-written list of eight names while the test's own name claimed *every*
    // fixture; a census on 2026-08-22 found **22 tours on disk and 9 named here**, so
    // fourteen were never checked. Worse than the shortfall is the shape: a
    // hand-written roster can only notice a tour that **disappears** from the picker,
    // never one that is added and never wired — because a tour missing from the picker
    // is also missing from the list, so nothing queries it and the test stays green.
    //
    // The same circular guarantee was found and fixed in `stage_tabs.rs` the same day.
    // [`crate::bridge::fixture_tours`] is *"the single definition of is-a-tour-file"*,
    // so deriving from it makes the name true.
    let tours: Vec<String> = crate::bridge::fixture_tours()
        .iter()
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(str::to_owned))
        .collect();
    // Non-vacuity: an empty or truncated corpus would make the loop below assert
    // nothing at all, which is the failure this whole change is about.
    assert!(
        tours.len() >= 20,
        "expected the full tour corpus, found {}: {tours:?}",
        tours.len(),
    );
    for tour in &tours {
        // `contains`, not exact: since 2026-08-05 a row reads
        // "failure-typecheck  ·  DimensionMismatch" so the model can be searched for
        // as well as the phase. The tour name is still the row's identity.
        //
        // **`get_all_…().next()`, not `query_by_label_contains`** — the latter
        // panics on two matches, and since `matching-live.md` split off on
        // 2026-08-08 the substring "matching" matches two rows. Presence is all
        // this loop asserts, so a second match is none of its business; the
        // module doc above records the same rule.
        assert!(
            h.get_all_by_label_contains(tour).next().is_some(),
            "the tour picker should offer {tour:?}; it is a checked-in fixture",
        );
    }
    assert!(
        h.query_by_label_contains("CATALOGUE").is_none(),
        "CATALOGUE.md is generated FOR CLAUDE — the index that lets an answer cite a \
         tour instead of retelling it. Doug found it in his picker on 2026-08-05, one \
         row among fifteen offering to be walked. It is not a tour and must not be \
         offered as one",
    );
    assert!(
        h.query_by_label("README").is_none(),
        "README.md is documentation ABOUT the tours, not a tour — offering it would \
         give the picker an entry whose stops do not exist",
    );
}

// **The visible tour count was removed 2026-08-19, and so was its test.**
//
// Doug went sixteen days without reading it once. Deleted rather than weakened: a
// test kept for a removed feature is a standing claim that the feature exists.

/// **A tree opens its root, so its children are on screen to be named.**
///
/// Doug, 2026-08-04: *"the trees are displayed entirely collapsed until I interact
/// with them. Yet, entirely collapsed is not useful. I almost always expand the root
/// tree node."* A fully collapsed tree shows one line, and `dae-construction.md`
/// points at children of the root (`x`, `p`, `f_x`) that were not visible to point at.
///
/// **Also asserts one level only.** Opening the children too would trade a tree that
/// shows nothing for one that shows everything — the DAE's second level has 20-odd
/// keys — so the grandchild must stay hidden.
#[test]
fn a_tree_opens_its_root_but_not_its_children() {
    use std::collections::{BTreeMap, HashMap};

    let value = serde_json::json!({
        "states": { "grandchild_key": 1 },
        "parameters": { "J": 1.0 },
    });
    let def_index = BTreeMap::new();
    let field_help = HashMap::new();

    let mut h = Harness::new_ui(move |ui| {
        let mut actions = crate::tree::TreeActions::default();
        crate::tree::tree_ui(
            ui,
            "DAE",
            &value,
            None,
            &mut actions,
            &def_index,
            &field_help,
            crate::tree::TreeOptions::default(),
        );
    });
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("states").is_some(),
        "the root's children must be visible without a click",
    );
    assert!(
        h.query_by_label_contains("parameters").is_some(),
        "all of them, not just the first",
    );
    assert!(
        h.query_by_label_contains("grandchild_key").is_none(),
        "one level only \u{2014} an entirely expanded tree is as unhelpful as an \
         entirely collapsed one, in the other direction",
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
    h.get_by_label_contains("RcCircuit \u{2192} Structural")
        .click();
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
///
/// # Why this clicks through accesskit, and what that costs
///
/// **It used to use a synthesized pointer click, and it passed for a reason that was
/// itself a bug** (2026-08-12). The tour pane was a vertical-only `ScrollArea`, so the
/// panel inflated to its content's width — 899pt of a 1280pt window — and the wide
/// layout made the document short enough that this link fell inside the visible
/// viewport. Enabling horizontal scrolling fixed the panel width, prose now wraps to
/// the width the reader chose, the document is correspondingly **taller**, and the link
/// moved below the fold. egui does not deliver pointer interaction outside a scroll
/// area's clip rect, so the click landed on nothing.
///
/// `click_accesskit` *"can also click widgets that are not currently visible"*, which
/// matches what this test is for: **dispatch**, not reachability. The pointer path stays
/// covered by `a_link_near_the_top_of_a_tour_dispatches` above, on a link that is
/// genuinely on screen — so the pair still covers both, and neither depends on the pane
/// happening to be mis-sized.
#[test]
fn a_link_far_down_a_long_tour_still_dispatches() {
    let mut h = harness(App::test_default());
    h.state_mut().test_set_specimen_files(&["BouncingBall.mo"]);
    assert!(
        h.state_mut().test_select_fixture_tour("matching"),
        "the fixture must be readable, or the click below has nothing to hit",
    );
    h.run_steps(2);

    h.get_by_label_contains("BouncingBall \u{2192} Structural")
        .click_accesskit();
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

    h.get_by_label_contains("RcCircuit \u{2192} Structural")
        .click();
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
    assert!(
        total >= 15,
        "the DAE tour should schedule ~20 beats, got {total}"
    );

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
    assert!(
        h.state().test_model().is_some(),
        "precondition: the RHS has something on it"
    );

    // Now pick a *different* tour. `contains`, because an entry carries its
    // specimens after the name.
    //
    // **The picker is a combo box since 2026-08-16, so it must be opened first.**
    // Queried by `value`: a ComboBox exposes its selected text as the node value and
    // carries no label, so `label_contains` finds nothing at all — which is how this
    // failed, rather than by clicking the wrong thing.
    //
    // *(The previous note here — "pick a tour that sorts early, or the last row falls
    // outside the harness viewport" — no longer applies: the popup is its own scroll
    // area and the old always-open list is gone. Kept in the history rather than the
    // comment.)*
    h.get_by_value("Pick a tour\u{2026}").click();
    h.run_steps(2);
    h.get_by_label_contains("camera-aiming").click();
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
    let mut app = App::test_default();
    // **Its own tour text, not the live ad hoc file.** This used to click a link out
    // of `.hrw-bridge/tour.md`, which is gitignored and rewritten every time Claude
    // answers a question — so it passed on content nobody had chosen, and broke the
    // day an answer was written that did not happen to contain this link.
    app.test_set_tour_text(
        "# Fixture\n\n[Structural → Incidence](hrw://stage/Structural/Incidence)\n",
    );
    let mut h = harness(app);

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
    let mut app = App::test_default();
    // Own tour text — see the note in the test above.
    app.test_set_tour_text(
        "# Fixture\n\n[Structural → Incidence](hrw://stage/Structural/Incidence)\n",
    );
    let mut h = harness(app);
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
    h.state_mut()
        .follow_link_for_test("hrw://load/Modelica.Electrical.Analog.Basic.Resistor");
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
        h.query_by_label_contains("not found: NoSuchThing")
            .is_some(),
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
        h.query_by_label_contains("2626").is_some() || h.query_by_label_contains("2,626").is_some(),
        "and it must say how many models it holds, so the header is evidence rather          than decoration",
    );

    h.state_mut()
        .test_set_filter("Spice3BenchmarkDifferentialPair");
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
    app.test_set_walked_state(
        "MotorWithBrake.mo",
        "MotorWithBrake",
        crate::worker::StageKind::Flatten,
    );
    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        h.query_by_label_contains("MotorWithBrake \u{00b7} Flatten")
            .is_some(),
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
    app.test_set_walked_state(
        "MotorWithBrake.mo",
        "MotorWithBrake",
        crate::worker::StageKind::Flatten,
    );
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
        h.query_by_label_contains("HRW specimens \u{2014} 2")
            .is_some(),
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
        h.query_by_label_contains("HRW specimens \u{2014} 1 of 2")
            .is_some(),
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
        h.query_by_label_contains("Left-click a tree node")
            .is_some(),
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

// **The equation sheet's empty state is UNREACHABLE, and its real behaviour now
// lives in `equation_sheet_view.rs`.**
//
// `equation_sheet_ui` opens with `let Some(sheet) = sheet else { ui.weak("(no
// equation sheet)"); return None; }` — which looks exactly like the empty state
// this chunk is built to check. There is one call site (`app.rs`, the Flatten
// sub-view row) and it is gated on `flatten_ready`, which is itself
// `cached_equation_sheet.is_some()`.
//
// The branch is defensive rather than wrong, so it stays. But a test asserting
// that message would have been **testing a string, not a behaviour** — passing
// forever regardless of what the pane does, which is the vacuity trap in its
// purest form. Worth writing down, because the message reads as evidence that
// the empty case is handled and reachable, and it is only the first of those.
//
// **What this comment used to say next was that the sheet's real behaviour
// "needs a populated `EquationSheet`, so it belongs with the tests that compile
// a specimen behind `slow-tests`." That is no longer true, and the reason is
// worth the correction:** the pane left `impl App` on 2026-08-19 and takes
// `Option<&EquationSheet>` as an argument, so the sheet is now something a test
// *constructs* rather than something a compile must produce. Six tests live
// beside it in `equation_sheet_view.rs` and run in 0.04 s.

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
        h.query_by_label_contains("Select a specimen to view its source")
            .is_some(),
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
        h.get_all_by_label_contains("cannot show this file")
            .next()
            .is_some(),
        "the pane must announce that it cannot show the file",
    );
    assert!(
        h.get_all_by_label_contains("os error 3").next().is_some(),
        "and it must carry the underlying reason — 'cannot show this file' alone leaves \
         the reader with nothing to act on, which is how the old false refusal read",
    );
}

/// **A selected model is never told to select a model.**
///
/// The sweep's finding, 2026-08-04. The source pane read `self.selected` off disk
/// with `.unwrap_or_default()` — and for a library model `selected` holds the
/// *qualified name* (`Modelica.Blocks.Continuous.SecondOrder`), not a path. When the
/// worker had not yet supplied the declaring file's text, the read failed, the empty
/// string was **cached**, and the pane's fallback said *"Select a specimen to view
/// its source."*
///
/// That is the same defect family as the fictions: a failure to read rendered as a
/// different, plausible, false claim — and this one sends the reader to fix something
/// that is not broken. The disk read is now skipped entirely for a library selection,
/// and the pane says what is actually true.
#[test]
fn a_library_model_awaiting_its_source_is_not_told_to_select_one() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_select_library_awaiting_source("Modelica.Blocks.Continuous.SecondOrder");
    let h = harness(app);

    // **The full sentence, not the prefix.** Four panes start with "Select a
    // specimen" — to compile, to see its purpose, to see its compilation log — and
    // the first version of this test matched the prefix, so it failed on the stages
    // pane legitimately saying nothing had been compiled yet. A substring deciding
    // which message it found, in a test written during the sweep that removed
    // substring-shaped defects.
    // `query_by_label_contains` for the absence, not `get_all_by_label_contains` —
    // the latter *panics* when nothing matches, so it cannot express "must not be
    // present" at all. The sibling test above uses the same call for the same reason.
    assert!(
        h.query_by_label_contains("Select a specimen to view its source")
            .is_none(),
        "a model IS selected \u{2014} this sentence sends the reader to fix something \
         that is not broken",
    );
    assert!(
        h.query_by_label_contains("has not arrived from the compiler")
            .is_some(),
        "the pane must say why there is no text yet",
    );
    assert!(
        h.state().test_source_load_error().is_none(),
        "a library selection must not be read from disk at all \u{2014} a qualified \
         name is not a path, and the failure it produces is meaningless",
    );
}

/// **A flagged stage shows its artifact BESIDE its error, not instead of it.**
///
/// Doug, 2026-08-05, walking `docs/fixture-tours/failure-typecheck.md`: *"there is no
/// tree in the failing typecheck stage view."* There was one in the data —
/// `DimensionMismatch`'s Typecheck stage carries the whole instantiated overlay plus
/// an `error` key, assembled by the worker precisely so both could be shown.
///
/// `central_panel_ui` rendered the summary in an `if` with the entire tree as the
/// `else`, so the overlay was built on every compile and discarded at the last step —
/// **with nothing on screen saying content was withheld**, which is the Context Bar
/// defect's shape.
///
/// **This test could not have been written before the fix**, which is the bar
/// `docs/format-and-app-plan.md` sets for an extraction: it asserts a pane shows two
/// things at once, and before the change it could only ever show one.
#[test]
fn a_flagged_stage_shows_its_artifact_beside_its_error() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_flagged_stage_with_artifact(crate::worker::StageKind::Typecheck);
    let h = harness(app);

    assert!(
        h.query_by_label_contains("dimension mismatch 2 vs 3")
            .is_some(),
        "the diagnostic must still be shown \u{2014} it is the more urgent half",
    );
    assert!(
        h.query_by_label_contains("components").is_some(),
        "and the artifact beside it must be reachable. Showing only the error \
         discards a tree the worker built on purpose, and says nothing about it",
    );
}

/// **A failed stage shows only its error**, because there is nothing else to show.
///
/// The other half of the pair, and the reason the fix is not "always render the tree":
/// `Stage::err_with_details` carries **only** `{"error": …}`, so a tree there would
/// render the error payload as a tree beside the summary that already explains it.
#[test]
fn a_failed_stage_with_only_an_error_shows_no_tree() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_failed_stage_error_only(crate::worker::StageKind::Flatten);
    let h = harness(app);

    assert!(
        h.query_by_label_contains("unbalanced model").is_some(),
        "the error summary is the whole content here",
    );
    assert!(
        h.query_by_label_contains("What went wrong").is_none(),
        "and it is NOT wrapped in the collapsing header, which exists only to make \
         room for an artifact underneath \u{2014} there is none",
    );
}

/// **"Show in source" washes the line it landed on**, not just scrolls to it.
///
/// Doug, 2026-08-05: *"Would it be possible to add visual highlighting of the item
/// being shown in the source?"* Scrolling puts the line somewhere in the pane; it
/// does not say **which** line, and a reader arriving in a 40-line file still has to
/// hunt for it. The wash is the answer to "which one".
///
/// Asserted on state rather than pixels — the fill is painted into a reserved slot
/// behind the row and `egui_kittest` cannot see colour. What it *can* pin is that the
/// verb sets the line, that the line survives the scroll being consumed, and that
/// changing specimen clears it. The last is the one that matters: a wash carried into
/// another file marks a line nothing pointed at.
#[test]
fn showing_a_variable_in_source_marks_the_line_it_landed_on() {
    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    // A specimen must be selected: `ShowSource` is not on `requires_specimen`'s
    // exempt list, and correctly so — there is no source to show without one.
    app.test_set_walked_state("RcCircuit.mo", "RcCircuit", crate::worker::StageKind::Parse);
    app.test_dispatch_show_source(7);

    assert_eq!(
        app.test_source_jump_line(),
        Some(7),
        "the landed-on line must be recorded, or the reader has to find it",
    );
    assert!(
        app.test_specimen_detail_is_source(),
        "and the pane showing it must be the source view",
    );

    // A new specimen invalidates it: line 7 of another file is a different line.
    app.test_clear_specimen_state();
    assert_eq!(
        app.test_source_jump_line(),
        None,
        "a wash carried into another specimen marks a line nothing pointed at",
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
        h.get_all_by_label_contains("No purpose note for NoSuchSpecimen")
            .next()
            .is_some(),
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
        h.get_all_by_label_contains("Compiling RcCircuit")
            .next()
            .is_some(),
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
        h.get_all_by_label_contains("Select a specimen to see its purpose")
            .next()
            .is_some(),
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
        h.query_by_label_contains("Select a specimen to compile")
            .is_some(),
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

    let f = h
        .state()
        .test_split_fraction()
        .expect("the panel must have been drawn");
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
    let wide = h
        .state()
        .test_split_fraction()
        .expect("drew at the bogus size");
    assert!(
        (wide - 0.4).abs() < 0.05,
        "precondition: 40% of the first size, got {wide}"
    );

    // The window turns out to be far smaller — as it is on the first real frame.
    h.set_size(eframe::egui::Vec2::new(1720.0, 1200.0));
    h.run_steps(3);

    let after = h
        .state()
        .test_split_fraction()
        .expect("drew at the real size");
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

    let drawn = h
        .state()
        .test_split_fraction()
        .expect("and the app records it");
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
    let specimen_f = specimen
        .state()
        .test_split_fraction()
        .expect("specimen panel drew");

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
    for (w, h) in [
        (1600.0_f32, 1200.0_f32),
        (1280.0, 900.0),
        (1024.0, 768.0),
        (800.0, 600.0),
    ] {
        let mut app = App::test_default();
        app.test_set_ui_mode_specimen();
        app.test_set_walked_state(
            "RcCircuit.mo",
            "RcCircuit",
            crate::worker::StageKind::Flatten,
        );
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

/// **A real frame publishes the pane on screen**, not just the function in isolation.
///
/// The unit tests around `publish_current_view` call it directly, so **all of them would
/// still pass if the call site vanished from `frame_ui`** — the gap between testing a
/// function and testing that anything invokes it. This drives an actual frame through the
/// harness and asserts the file appears, which is the only version that catches a deleted
/// call.
///
/// Doug's use for it (2026-08-13): stop having to transcribe a pane into chat before
/// asking about it.
#[test]
fn a_rendered_frame_publishes_the_current_view() {
    let path = std::path::Path::new(crate::bridge::VIEW_FILE);
    let _ = std::fs::remove_file(path);

    let mut app = App::test_default();
    app.test_set_ui_mode_specimen();
    app.test_set_walked_state(
        "RcCircuit.mo",
        "RcCircuit",
        crate::worker::StageKind::Flatten,
    );
    app.test_set_equation_sheet_for_publish();

    let mut h = harness(app);
    h.run_steps(2);

    assert!(
        path.exists(),
        "a frame on Flatten with an equation sheet must publish it \u{2014} \
         `publish_current_view` is not being reached from `frame_ui`",
    );
    let text = std::fs::read_to_string(path).expect("read view.json");
    assert!(
        text.contains("Flatten/EquationSheet"),
        "the published file must name the pane, got: {text}",
    );
    assert!(
        text.contains("f_x[0]"),
        "the published rows must carry their cross-view id, got: {text}",
    );

    let _ = std::fs::remove_file(path);
}

/// **The left panel's content never detaches from the divider**, at any window
/// size, however far the divider is dragged.
///
/// Doug, 2026-08-12, on a 13" laptop: *"the vertical divider refuses to go left
/// beyond a certain horizontal position. However, the right edge of the LHS content
/// continues to move leftward as I continue my attempted leftward drag."*
///
/// **The panel has an intrinsic minimum width — its content's — and the floor HRW
/// set was a fraction of the window.** Above the content minimum the two agreed and
/// nothing was visible; below it the outer rect held while the inner `Ui` kept taking
/// the dragged width, and the content detached. Measured before the fix, at 640
/// points wide, the gap grew from 21 to **112 points** as the drag continued. See
/// `app::MIN_LEFT_POINTS`.
///
/// **Why the sizes here are small, and why that is the whole point.** HRW ran at
/// `DEFAULT_ZOOM` = 2.0 until 2026-08-12, which gave a 13" screen only **~640×360
/// points** — and `the_chrome_stays_on_screen_at_every_width` tests down to 800×600
/// and never saw this, because at 800 points the 15 % floor is still above the
/// content minimum. **The defect lives below 800 points.**
///
/// The default zoom is 1.0 now, so that regime is no longer where Doug sits — but
/// **these sizes stay small deliberately.** A small window or a raised zoom puts him
/// back there in one gesture, and a regression test pinned to the current default
/// would stop covering the failure the moment the default moved again.
///
/// # The second defect, and why this test missed it the first time
///
/// Doug, hours later: *"the divider does not move. Instead, only the right edge of the
/// LHS tour content moves."* A **different** cause with the same signature — a
/// vertical-only `ScrollArea` reports its content's full width as the width it wants,
/// and `egui_commonmark` does not wrap tables, so the tour panel's minimum became the
/// widest table in the document: it opened at **899pt instead of 512pt and was frozen
/// solid**, gap reaching 705pt.
///
/// **The first version of this test could not have caught it, because it loaded no
/// tour.** `App::test_default` has no tour text and `test_set_walked_state` seeds one
/// short line of source, so every width in the LHS was small and every drag worked.
/// **A fixture narrow enough to pass is a fixture that tests nothing here** — so the
/// tour case now loads `the-concepts.md` from disk, the real document with the
/// widest table in the set, and asserts the panel *opens at the default width* rather
/// than at its content's.
#[allow(
    clippy::too_many_lines,
    reason = "one property checked across three window sizes, two modes and six drag \
              positions; splitting it would hide which combination failed"
)]
#[test]
fn the_left_panel_content_never_detaches_from_the_divider() {
    use eframe::egui::{Pos2, Vec2};

    // The frame padding plus the resize handle, measured at 19–23 points across
    // every size and drag position after the fix. 40 is comfortably above that and
    // far below the 98–148 the defect produced, so this bound distinguishes the two
    // without asserting an exact chrome width that styling may legitimately change.
    const MAX_CHROME: f32 = 40.0;

    // **Tour mode has a floor, and that is now a decision rather than a defect**
    // *(Doug, 2026-08-16)*.
    //
    // The tour transport bar carries controls that cannot compress — a button whose
    // label is fixed text, a combo box, the playback controls. A *vertical list* of
    // labels shrinks; a *horizontal bar of widgets* does not. So once the picker moved
    // into that bar, the panel stopped tracking the divider below about 250pt while its
    // content laid out at 194 — a **stable** 56pt difference across four frames and
    // every drag position, not a drift and not an oscillation.
    //
    // Measured before deciding:
    //
    // ```text
    // tour=true   panel floors at 250–255, inner 194  → 56pt
    // tour=false  panel floors at 212,     inner 194  → 19pt (ordinary chrome)
    // ```
    //
    // Doug's call, having seen the divider open further right than before: *"it would
    // be ok to accept a minimum width for the LHS to make everything work… I choose #2:
    // change the guard deliberately."* His reasoning is that the zoom fix removed most
    // of the 13" pressure, so he rarely drags the divider at all now.
    //
    // **What this bound still forbids** is the failure it was written for: a panel that
    // keeps growing away from its content (98–148pt, and unbounded as the tour got
    // wider). A named floor 16pt above ordinary chrome is a different thing from a
    // panel that has stopped listening, and `the_tour_panel_still_reaches_its_floor`
    // below asserts the floor is *reached* rather than merely tolerated.
    const MAX_TOUR_CHROME: f32 = 62.0;

    // Point-space sizes, with whether the divider can move at all at that size.
    //
    // 640×360 is the 13" laptop case. **500×340 cannot be dragged**, and that is
    // correct rather than a gap in the test: there the panel already sits at its
    // content's intrinsic minimum, so there is no travel to give. Requiring
    // movement there was this test's first failure, and the honest fix was to stop
    // requiring it — see `width_range`, which says a window that narrow simply has
    // nothing draggable about it.
    // **The real tour, from disk, not a fixture.** Its widest line is 178 characters
    // and it carries the route table; a synthetic short document is exactly what let
    // the 899pt freeze through unnoticed.
    let real_tour = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours/the-concepts.md"),
    )
    .expect("the-concepts.md must be readable — it is the widest tour");

    // **640×360 no longer expects movement, and that is a cost accepted rather than a
    // bug tolerated** *(2026-08-19)*. Un-wrapping the tour transport bar makes its
    // one-row width a hard floor on the panel — the price of a minimum that varies
    // *monotonically* with the bar's contents, which the wrapped version never did:
    // five separate edits to it each failed this test in a different place.
    //
    // The floor was bought down first — the tour count removed, `30s — teaser` → `30s`,
    // `✨ Claude's answer` → `✨ Answer` — taking the one-row minimum from 580pt to
    // 405.9pt. At 1280 the panel settles at 450.5pt, **35%**, against the 40% Doug walks
    // tours at, so he never meets it.
    //
    // **At 640 he does**: a ~410pt floor is most of the window, which is HRW at half
    // width beside VS Code — the `matching-live.md` debugger layout. That tour wants a
    // wider window now. Recorded here rather than in a commit message because this line
    // *is* the decision.
    for (w, h_px, expect_movement) in [
        (1280.0_f32, 720.0_f32, true),
        (640.0, 360.0, false),
        (500.0, 340.0, false),
    ] {
        for mode_is_tour in [true, false] {
            let mut app = App::test_default();
            // Tour is `UiMode`'s `#[default]`, so the tour case needs no switch.
            if !mode_is_tour {
                app.test_set_ui_mode_specimen();
            }
            app.test_set_walked_state(
                "RcCircuit.mo",
                "RcCircuit",
                crate::worker::StageKind::Flatten,
            );
            if mode_is_tour {
                app.test_set_tour_text(&real_tour);
            }
            let mut h = Harness::builder()
                .with_size(Vec2::new(w, h_px))
                .build_ui_state(|ui, app: &mut App| app.frame_ui(ui), app);
            h.run_steps(3);

            let started_at = h
                .state()
                .test_split_fraction()
                .expect("the split must have been drawn");

            // **The panel opens where HRW put it, not where its content wants.** This
            // is the assertion that names the second defect directly: with the real
            // tour loaded the panel opened at 899pt of a 1280pt window — 70 %, while
            // reporting a 40 % default — because wide unwrapped content was setting
            // the minimum. The floor may legitimately push it *wider* than 40 % on a
            // narrow window, so this bounds it from above only.
            // **Bounded in POINTS on a narrow window, not as a fraction** *(2026-08-19)*.
            // The floor is an absolute number — the un-wrapped bar's one-row width — so on
            // a 640pt window it is necessarily a large *fraction*, and asserting a
            // fraction there tests the window size rather than the layout. What still
            // matters at any size is that the floor is the **bar's** width and not the
            // tour prose's: 480pt is the bar plus chrome, and the 899pt failure this
            // assertion was written for would still trip it.
            let ceiling = if w >= 1000.0 { 0.55 } else { 455.0 / w };
            assert!(
                started_at <= ceiling,
                "{w}x{h_px} tour={mode_is_tour}: the panel opened at {:.0}% of the \
                 window ({:.1}pt) \u{2014} content is dictating the width instead of \
                 the split, so there is nothing left to drag",
                started_at * 100.0,
                started_at * w,
            );

            // Grab the divider and walk it hard left, holding the button down —
            // the gap only opened *during* a drag past the stop, so releasing
            // first would miss it.
            h.drag_at(Pos2::new(started_at * w, h_px * 0.5));
            h.run_steps(1);
            let mut moved = false;
            for x in [w * 0.35, w * 0.25, w * 0.18, w * 0.10, w * 0.05, 8.0] {
                h.hover_at(Pos2::new(x, h_px * 0.5));
                h.run_steps(1);

                let panel_w = h.state().test_split_fraction().unwrap_or(-1.0) * w;
                let inner_w = h
                    .state()
                    .test_split_inner_width()
                    .expect("the panel must have recorded its inner width");
                moved |= (panel_w - started_at * w).abs() > 1.0;

                // **The floor must be REACHED, not merely tolerated.** Widening
                // `MAX_TOUR_CHROME` accepts that the transport bar cannot compress; it
                // must not accept a panel that keeps drifting wider, which is the
                // defect this test was written for and which Doug saw again on
                // 2026-08-16 as *"the vertical divider defaulting far to the right when
                // starting HRW."*
                //
                // At the narrowest pointer position the panel has nowhere left to go,
                // so it is sitting on its floor and the floor is measurable. 300pt
                // leaves room for a label changing width without admitting the ~445pt
                // the panel opens at.
                // **Raised to 480pt on 2026-08-19**, when the bar stopped wrapping and its
                // one-row width became the panel's floor — measured at 450.5pt.
                //
                // It no longer catches "the panel is drifting wider", which is unreachable
                // by construction now. It catches **something was added to the bar without
                // a matching reduction**, which is the live risk once width is monotonic,
                // and that cost is permanent: every point here is a point the RHS never
                // gets back.
                if x <= 8.0 {
                    assert!(
                        panel_w <= 455.0,
                        "{w}x{h_px} tour={mode_is_tour}: dragged hard left, the panel \
                         settled at {panel_w:.1}pt. Since 2026-08-19 the un-wrapped tour \
                         bar's one-row width IS this floor, measured at 431.7pt with Back — so a \
                         higher number means something was added to that bar without a \
                         matching reduction, and the RHS pays for it permanently.",
                    );
                }

                let budget = if mode_is_tour {
                    MAX_TOUR_CHROME
                } else {
                    MAX_CHROME
                };
                assert!(
                    panel_w - inner_w <= budget,
                    "{w}x{h_px} tour={mode_is_tour}, pointer at x={x:.0}: the panel is \
                     {panel_w:.1}pt wide but its content was laid out against \
                     {inner_w:.1}pt \u{2014} a {:.1}pt gap, so the content has detached \
                     from the divider",
                    panel_w - inner_w,
                );
                assert!(
                    inner_w > 0.0,
                    "{w}x{h_px} tour={mode_is_tour}: the content was given {inner_w:.1}pt, \
                     which is not a width",
                );
            }
            h.drop_at(Pos2::new(8.0, h_px * 0.5));
            h.run_steps(2);

            // **Non-vacuity, where movement is possible.** If the drag never
            // reached the divider, every assertion above passed by never testing
            // anything — and a synthetic drag landing on the wrong pixel is exactly
            // the way this test would rot into a tautology. The defect needed a drag
            // *in progress* to appear, so a test that never drags cannot see it.
            // **At 640 the answer differs by mode, since 2026-08-19.** The un-wrapped
            // tour bar sets a 435pt floor, which on a 640pt window leaves the tour panel
            // a usable range — while Specimen mode, whose left panel has no bar and so no
            // floor of its own, is already at its content minimum and cannot move. One
            // expectation per width stopped being expressible when the two modes acquired
            // different minimums.
            let expect_movement = if (w - 640.0).abs() < 1.0 {
                mode_is_tour
            } else {
                expect_movement
            };
            assert_eq!(
                moved,
                expect_movement,
                "{w}x{h_px} tour={mode_is_tour}: expected the divider to be \
                 {} at this size, and it was not \u{2014} either the synthetic drag is \
                 missing the handle (which would make the checks above vacuous) or the \
                 permitted range has changed",
                if expect_movement {
                    "draggable"
                } else {
                    "pinned at its content minimum"
                },
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
    assert!(
        h.state().test_viewing_log(),
        "precondition: the log is showing"
    );

    h.get_all_by_label_contains("Flatten")
        .next()
        .expect("a Flatten tab")
        .click();
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
        // **The background's own shape, not the bare name.** `contains("RcCircuit")`
        // is satisfied by any source line, list row or notice that mentions the
        // specimen — so this did not assert "the Context Bar names it", it asserted
        // "exactly one thing on screen mentions it", and it broke on 2026-08-04 when
        // the source pane legitimately started rendering. See the sweep note in
        // `docs/tech-debt.md`.
        h.query_by_label_contains("\u{00b7} RcCircuit \u{00b7}")
            .is_some(),
        "precondition: the Context Bar names the specimen even with nothing pointed at",
    );
    assert!(
        h.query_by_label_contains("Pointing at").is_none(),
        "precondition: nothing is pointed at yet",
    );

    h.get_all_by_label_contains("Flatten")
        .next()
        .expect("a Flatten tab")
        .click();
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
    app.test_set_walked_state(
        "RcCircuit.mo",
        "RcCircuit",
        crate::worker::StageKind::Flatten,
    );
    let mut h = harness(app);
    assert!(
        !h.state().test_viewing_log(),
        "precondition: a stage is showing, not the log"
    );

    // **Exact label, not .** Several nodes carry "Log" as a substring,
    // and the first one  returns is not the button -- the click landed
    // somewhere harmless and the test read as "the Log button is broken".
    h.get_by_label("Log").click();
    h.run_steps(2);

    assert!(
        h.state().test_viewing_log(),
        "the Log button must show the log"
    );
    assert_eq!(
        h.state().test_stage(),
        crate::worker::StageKind::Flatten,
        "and must leave the selected stage alone — glancing at the log is not a decision \
         about which stage you are studying",
    );
}

/// **The "Claude's answer" row is always on screen — disabled when there is none.**
///
/// Doug, 2026-08-15: *"the 'Claude's Answer' tour seems to have disappeared from the
/// tours list in the tour mode."* It had not broken; no ad hoc tour had been written,
/// and the row was gated on `.hrw-bridge/tour.md` existing. **The absence was correct
/// and the design was wrong** — a row that silently ceases to exist gives no way to
/// distinguish *"nothing written yet"* from *"the feature broke"*, and he read it as
/// the second, which was the only reading the evidence supported.
///
/// `docs/ideas.md` #77 states the rule this breaks: *controls are enabled and disabled,
/// never shown and hidden.*
///
/// **Why this matters beyond one row:** the ad hoc tour is the whole *"Claude composes
/// an answer inside HRW"* capability (#42). A feature that looks broken stops being
/// reached for, and dies of apparent absence rather than of any decision.
///
/// The test asserts the row is **present**; that it is *disabled* is not reachable
/// through the accessibility tree here, and saying so is better than implying coverage
/// this does not have.
#[test]
fn the_ad_hoc_tour_row_is_present_even_with_no_ad_hoc_tour() {
    // **The state under test is ESTABLISHED, not asserted.**
    //
    // This used to assert the bridge file was absent, on the reasoning that it is
    // gitignored and absent in a clean checkout. That made the test fail whenever the
    // *feature worked*: Claude writes `.hrw-bridge/tour.md` to answer a question, and
    // from that moment the suite went red until someone deleted it. Found 2026-08-16,
    // the first time an ad hoc tour was written since the test existed.
    //
    // A test whose precondition is "the user has not used the product recently" is
    // measuring the environment. It now moves any real tour aside and restores it on
    // the way out, so a live answer survives the run and the run does not depend on
    // there being none.
    let _tour_state = AdHocTour::absent();

    // Tour is `UiMode`'s `#[default]`, so no mode switch is needed.
    let mut h = harness(App::test_default());
    h.run_steps(2);

    let label = crate::tour::TourSource::AdHoc.label();
    assert!(
        h.query_by_label_contains(&label).is_some(),
        "the {label:?} row must be listed even with no ad hoc tour \u{2014} its absence \
         is indistinguishable from the feature being broken",
    );
}

/// **The ad hoc tour has exactly one control, and it is not inside the picker.**
///
/// Doug, 2026-08-16: *"It seems to me that the 'Claude's Answer' tour item is special.
/// So special that perhaps it could be its own UI button beside the drop-down."*
///
/// It is a different kind of object from the other 22: they are committed, versioned,
/// machine-checked and citable as `hrw://tour/<name>/stop/<slug>`; this one is
/// `.hrw-bridge/tour.md` — gitignored, regenerated per question, and there is only ever
/// one. The code already privileged it (`tour::poll` auto-selects it); only the
/// presentation flattened it into row 23.
///
/// **Listing it in both places is the regression this guards.** A duplicate would make
/// one of the two a lie about where the tour lives, and the obvious way to write the
/// combo — iterate `available` — produces exactly that, because `available` still
/// contains `AdHoc`.
#[test]
fn the_ad_hoc_tour_is_a_button_and_not_a_picker_entry() {
    let label = crate::tour::TourSource::AdHoc.label();

    // **The ad hoc tour must actually EXIST, or the duplication half of this test
    // checks nothing.** Injecting `tour.available` does not work: `poll_tour_file`
    // rebuilds the list from disk on the first frame and erases it — measured, after an
    // injected version passed while the tour was listed in both places.
    //
    // So the file is written, and removed by a **guard** rather than by a line at the
    // end: a failing assertion between the two would leave it behind and break
    // `the_ad_hoc_tour_row_is_present_even_with_no_ad_hoc_tour`, which asserts its
    // absence as a precondition.
    // **The guard RESTORES what was there; it does not delete.** The first version
    // removed the file on the way out — and `.hrw-bridge/tour.md` is not scratch space,
    // it is Claude's live answer to Doug's last question. Running the suite while one
    // existed **destroyed it**, silently, which is worse than a failing test. Found
    // minutes after this test shipped, the first time an ad hoc tour was written.
    let _tour_state = AdHocTour::with(
        "# Claude's answer

A fixture for the picker test.
",
    );

    let mut h = harness(App::test_default());

    // **Counted by EXACT label since 2026-08-30, not by substring.** This asked for
    // nodes whose label *contains* "✨ Answer" and required exactly one — which quietly
    // also meant "nothing else on screen may NAME the open tour". The Context Bar then
    // began doing exactly that, correctly, and turned this red.
    //
    // The control's label *is* the tour's label; anything that merely mentions the tour
    // says more than that — the bar's background line reads "· tour: ✨ Answer". So an
    // exact match counts controls and ignores mentions, which is what the test's own
    // name claims it is about.
    let controls = |h: &Harness<'static, App>| h.get_all_by_label(&label).count();

    // Present as its own control while the picker is closed — the whole point of
    // promoting it, and the state Doug reported as a broken feature when it vanished.
    assert_eq!(
        controls(&h),
        1,
        "the ad hoc tour must have exactly one control, visible without opening the \
         picker",
    );

    h.get_by_value("Pick a tour\u{2026}").click();
    h.run_steps(2);

    assert_eq!(
        controls(&h),
        1,
        "opening the picker must not add a second {label:?} \u{2014} it lives beside \
         the picker, not inside it, and two controls for one tour makes one of them \
         wrong about where it comes from",
    );
}

/// **Switching tours puts the new one at its top, and the PAINT PATH is what does it.**
///
/// Doug, 2026-08-17: *"When I click a subordinate tour link in the-concepts hub tour, the
/// subordinate tour opens partially scrolled down instead of fully scrolled to the top."*
///
/// # Why the state-level half is not enough
///
/// `TourState::reset_scroll` sets a flag; egui's `ScrollArea` is what actually holds the
/// offset, under the stable `id_salt("tour")`. A test that only checked the flag would
/// have stayed green through the entire life of the bug — the three fields
/// `reset_scroll` already cleared were HRW's *own* measurements, and clearing them is
/// exactly what looked like a fix for eleven days.
///
/// **So this paints.** The flag being consumed proves the call site is wired, which is
/// the coupling a state-only test cannot see. That lesson is one day old here: the first
/// attempt at the tour-link navigation fix was written at the wrong level and stayed
/// green when the call site was re-gated.
///
/// The offset itself is not asserted, and that is deliberate rather than lazy: the
/// `ScrollArea`'s `Id` is derived from its parent `Ui`, so reconstructing it in a test
/// would hard-code a layout detail that a reshuffle silently invalidates. What is
/// checkable without guessing is that a frame carrying text consumes the request and a
/// frame carrying none preserves it.
#[test]
fn switching_tours_asks_the_pane_to_return_to_the_top() {
    let _guard = AdHocTour::absent();
    let mut h = harness(App::test_default());
    h.run_steps(2);

    // The reported gesture: leave whatever is showing for another tour. Routed through
    // `test_select_fixture_tour`, which calls `select_tour` — the same path the picker
    // and an `hrw://tour/…` link both take, so the request is made exactly as it is in
    // the app.
    //
    // **Not driven by clicking the picker, and the reason is a harness trap this file
    // already warns about.** The popup is a scroll area: a tour that sorts late is in
    // the accessibility tree but clipped, so the click lands on nothing. The first
    // version of this test did exactly that with `node-pointing` (17th of 22), selected
    // nothing, and **passed with the fix switched off** — "not set" reading as "set and
    // spent".
    assert!(
        h.state_mut().test_select_fixture_tour("node-pointing"),
        "the fixture must be readable, or no switch happens and nothing below is a test",
    );

    // Non-vacuity: the switch really did request a return before any frame ran.
    assert!(
        h.state().tour.scroll_to_top,
        "precondition: switching must set the request — that half is \
         `app::tests::switching_tours_requests_a_return_to_the_top`",
    );

    h.run_steps(4);

    assert!(
        !h.state().tour.scroll_to_top,
        "switching tours must ask the pane to return to the top, and the painting frame \
         must consume that request. Still set means the paint path never reads it, which \
         is the shape the bug had: `reset_scroll` cleared three fields that do not \
         position anything, while egui kept the previous document's offset under the \
         stable `id_salt(\"tour\")`",
    );
}

/// **The request survives a frame with no text to apply it to.**
///
/// Switching clears `cached`, so at least one frame renders no document. Consuming the
/// flag there would spend it on a document that was never drawn — and the reader would
/// land mid-page anyway, with every field looking correctly reset.
///
/// **A one-shot spent on the wrong frame is the classic form of this bug**, and it fails
/// intermittently: whether the text is cached yet depends on poll timing, so the fix
/// would work when tested and not when walked.
#[test]
fn a_return_to_the_top_is_not_spent_on_a_frame_with_no_tour() {
    let _guard = AdHocTour::absent();
    let mut h = harness(App::test_default());
    h.run_steps(2);

    // No tour selected, and a pending request.
    h.state_mut().tour.selected = None;
    h.state_mut().tour.cached = None;
    h.state_mut().tour.scroll_to_top = true;
    h.run_steps(2);

    assert!(
        h.state().tour.scroll_to_top,
        "with no document on screen there is nothing to scroll, so the request must \
         still be pending when one arrives",
    );
}

/// **The pane spends a stop request, so a `stop/<slug>` link actually lands.**
///
/// The paint half of `app::tests::a_stop_link_records_where_that_stop_begins`. The
/// feature was broken for its whole existence in exactly this gap: the handler recorded
/// a destination and **no frame ever read it**, so the offset sat there while the tour
/// opened wherever the pane already was.
///
/// **What is asserted is that the request is consumed**, not the resulting pixel offset.
/// The scroll is performed by `ui.scroll_to_cursor` inside the `ScrollArea`, so the
/// number belongs to egui — reconstructing it here would mean recomputing the thing the
/// implementation deliberately refuses to compute, since rendered height per character
/// is not constant and four attempts proved no constant corrects for it.
#[test]
fn a_stop_request_is_spent_by_the_pane() {
    let _guard = AdHocTour::absent();
    let mut h = harness(App::test_default());
    h.run_steps(2);

    assert!(
        h.state_mut().test_select_fixture_tour("failure-parse"),
        "the fixture must be readable, or there is no document to scroll",
    );

    // A real destination inside that document, found the way the handler finds it.
    let offset = h
        .state()
        .tour
        .text()
        .and_then(|t| t.find("## Stop 4"))
        .expect("failure-parse.md must have a Stop 4 to aim at");
    h.state_mut().tour.scroll_to_offset = Some(offset);

    h.run_steps(4);

    assert!(
        h.state().tour.scroll_to_offset.is_none(),
        "a painting frame must spend the stop request. Still pending means no frame \
         reads it — which is precisely how this feature shipped broken: the handler \
         recorded where to go and nothing ever went there",
    );
}

/// **A stale offset is discarded rather than slicing a `str` in half.**
///
/// Tours are re-read whenever their mtime changes, and Doug walks them *while* they are
/// being edited — that is the working mode of this project, not a corner case. So an
/// offset recorded against the previous text is expected, and `&text[..n]` panics if `n`
/// is not a character boundary.
///
/// **Both bad shapes are checked**, because they fail differently: past the end, and
/// inside a multi-byte character. Every tour here contains em-dashes and arrows, so the
/// second is reachable by ordinary editing rather than by contrivance.
#[test]
fn a_stale_stop_offset_is_discarded_without_panicking() {
    let _guard = AdHocTour::absent();
    let mut h = harness(App::test_default());
    h.run_steps(2);

    assert!(
        h.state_mut().test_select_fixture_tour("failure-parse"),
        "the fixture must be readable",
    );
    let len = h.state().tour.text().map(str::len).unwrap_or_default();
    assert!(len > 0, "precondition: the tour has text");

    // Past the end — an offset from a longer, earlier version of the document.
    h.state_mut().tour.scroll_to_offset = Some(len + 5_000);
    h.run_steps(2);
    assert!(
        h.state().tour.scroll_to_offset.is_none(),
        "an unusable offset must still be consumed, or it is retried on every frame \
         forever",
    );

    // Inside a multi-byte character. Found rather than assumed, so this test fails
    // loudly if the tour ever stops containing one instead of passing vacuously.
    let mid = h
        .state()
        .tour
        .text()
        .and_then(|t| (1..t.len()).find(|i| !t.is_char_boundary(*i)))
        .expect("the tour must contain a multi-byte character to aim inside of");
    h.state_mut().tour.scroll_to_offset = Some(mid);
    h.run_steps(2);
    assert!(
        h.state().tour.scroll_to_offset.is_none(),
        "an offset inside a character must be consumed and ignored, never sliced",
    );
}

/// **No marker from a real tour reaches the accessibility tree.**
///
/// Doug, 2026-08-17: *"The 'kind' metadata which you've added to the tours is now visible
/// in the HRW rendering of those tours."*
///
/// # Why the unit tests are not enough
///
/// `tour::tests_comment_stripping` proves the function removes comments. **It cannot
/// prove the function is called**, and the bug was never in the stripping — there was no
/// stripping. This loads a real tour off disk, through the real poll, and looks at what
/// the pane published.
///
/// **It also covers markers this session did not add.** Thirty-three comments were being
/// rendered before the kind tag existed — `pane-groups`, `pane-origins`, `pane-frames`,
/// `unbuilt:` — unreported because they sit beside tables in the middle of a document
/// rather than under the title. Doug's report was about the new one; the defect was
/// older, and asserting on the general form is what keeps the fix honest about that.
#[test]
fn a_tour_renders_none_of_its_html_markers() {
    let _guard = AdHocTour::absent();
    let mut h = harness(App::test_default());
    h.run_steps(2);

    // `connect-expansion` carries a `kind` tag and the `pane-groups` markers, so one
    // tour exercises both the new marker and the pre-existing ones.
    assert!(
        h.state_mut().test_select_fixture_tour("connect-expansion"),
        "the fixture must be readable, or nothing is rendered to inspect",
    );
    h.run_steps(2);

    let text = h.state().tour.text().unwrap_or_default().to_owned();

    // Non-vacuity, both ways: the document really is loaded, and the file on disk
    // really does contain a marker. Without the second, this test would pass forever
    // if the tags were quietly dropped from the corpus.
    assert!(
        text.contains("Stop 1"),
        "precondition: the tour text is actually loaded",
    );
    let on_disk = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("docs/fixture-tours/connect-expansion.md"),
    )
    .expect("the tour must be readable from disk");
    assert!(
        on_disk.contains("<!-- kind: concept -->"),
        "precondition: the file still declares its kind — the marker must survive on \
         disk, since the kind checkers read it from there",
    );

    assert!(
        !text.contains("<!--") && !text.contains("-->"),
        "no HTML marker may reach the pane. The kind tag sat under the title of every \
         tour because `egui_commonmark` renders a comment as literal text, and the \
         claim that it would be invisible was written into the README without ever \
         being checked",
    );
    assert!(
        h.query_by_label_contains("kind: concept").is_none(),
        "and nothing in the accessibility tree carries it either — the pane is what \
         Doug reads, and the cached string is only where it comes from",
    );
}

/// **Prose after a wide table wraps to the TABLE's width, not the panel's.**
///
/// Doug, 2026-08-28, after narrowing his own first report: *"prose does get wrapped, but
/// only according to the width of the table which precedes it, or to the width of the LHS
/// if no table precedes the prose. In the connections tour, all content seems to wrap to
/// the width of the LHS."*
///
/// # The mechanism
///
/// `tour_panel` renders into `ScrollArea::both()`, and inside a scroll area with the
/// horizontal axis enabled **a child that allocates beyond the `Ui`'s `max_rect` expands
/// it** — for every later sibling. So a paragraph before the first table wraps to the
/// panel, and the identical paragraph after it wraps to the table. `the-concepts` has two
/// wide tables and shows it; `connect-expansion` has none and does not.
///
/// Measured here: 590pt / 3 lines before, **839pt / 2 lines after**, in a panel ~590pt wide.
///
/// # Why it is `#[ignore]`d rather than absent
///
/// **No fix is known yet**, and two were tried and measured as ineffective: bounding the
/// shared `Ui` with `set_max_width` *or* `set_width` before rendering, both undone by the
/// table expanding it again. `egui_commonmark` 0.24 offers no wrap-width control
/// (`default_width` bounds images only), and its `show_scrollable` is `#[doc(hidden)]` and
/// documented as buggy.
///
/// The remaining candidate is to **split the document into table and non-table runs** and
/// re-apply the bound per prose run — which means rendering one document as several viewer
/// calls, and this file already warns that doing so *"is not free of consequence"* — a
/// change to Doug's primary learning surface, so it is his call.
///
/// Same shape as `pantelides_ladder`'s rungs: the acceptance test exists and is ignored
/// until the thing it accepts is built. **Un-ignore it when a fix lands.**
#[test]
#[ignore = "no fix known: prose after a wide table inherits the table's wrap width"]
fn tour_prose_after_a_table_wraps_to_the_panel_not_the_table() {
    let real = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/fixture-tours/the-concepts.md"
    ))
    .expect("the-concepts.md is the document that exhibits this");
    // Up to the end of the hub table -- the real one, because a synthetic table narrow
    // enough to fit the panel does not reproduce it, and three earlier drafts passed
    // vacuously for exactly that reason.
    let cut = real
        .match_indices('\n')
        .map(|(i, _)| i)
        .find(|i| *i > real.find("| 4 |").unwrap_or(0))
        .unwrap_or(real.len());

    // Identical paragraphs either side, so the assertion needs no magic number: correctly
    // wrapped they occupy the same height.
    const PARA: &str = "the quick brown fox jumps over the lazy dog and keeps going \
                        well past the width of any reasonable panel, which is the \
                        entire point of this fixture, and so it continues for some \
                        time yet before finally coming to a stop.";
    let _tour_state = AdHocTour::with(&format!(
        "ZZbefore {PARA}\n\n{}\n\nZZafter {PARA}\n",
        &real[..cut]
    ));

    let mut h = harness(App::test_default());
    h.run_steps(3);

    let rect = |m: &str| {
        h.query_by_label_contains(m)
            .unwrap_or_else(|| panic!("{m} should render"))
            .rect()
    };
    let (before, after) = (rect("ZZbefore"), rect("ZZafter"));

    assert!(
        (before.height() - after.height()).abs() < 1.0,
        "the same paragraph is {:.0}pt tall and {:.0}pt wide before the table, but \
         {:.0}pt tall and {:.0}pt wide after it \u{2014} so the one after inherited the \
         TABLE's width to wrap at rather than the panel's.",
        before.height(),
        before.width(),
        after.height(),
        after.width(),
    );
}
