//! The observatory shell — the top-level egui application.
//!
//! ## Immediate-mode UI in a nutshell
//!
//! egui is an **immediate-mode** GUI: every frame, the framework calls our
//! [`App::ui()`] method, and we rebuild the entire UI from scratch — buttons,
//! labels, panels, trees, plots, everything. There is no retained widget tree
//! that persists between frames; instead, all durable state lives in the [`App`]
//! struct itself (stage results, navigation stack, view flags, etc.), and the UI
//! code reads/writes those fields each frame to decide what to show and how to
//! react to clicks. This means:
//!
//! - **Layout is code, not data.** You won't find XML/HTML templates — the UI
//!   is specified by Rust function calls (`ui.label(…)`, `ui.button(…)`, etc.)
//!   executed every frame.
//! - **State mutations happen in-line.** A button click is detected the same
//!   frame the button is drawn; a `if button.clicked() { self.flag = true; }`
//!   both renders the button and handles the event, in one pass.
//! - **The App struct IS the model.** Any value that must survive across frames
//!   (the selected specimen, compilation results, which panel is open) must be
//!   a field on `App`. Local variables inside `ui()` are gone after the frame.
//!
//! Eframe shell (charter §4.2.1, §4.4): a file picker over the specimen
//! directory, a library-path (source-root) configuration for dependency
//! resolution, and the generic serde-value tree inspector showing each stage's
//! IR for the selected model.

use std::path::{Path, PathBuf};

use eframe::egui;

use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap, HashSet};

// The bridge module handles communication with Claude Code (the AI assistant
// running in a terminal alongside this app): we write JSON "focus" files that
// Claude reads to understand what the user is looking at.
use crate::bridge::{self, Ask, Focus, Seg};
use crate::diagnostics;
use crate::equation_sheet;
use crate::equation_sheet_view::SheetClick;
use crate::identifier_index;
// Canvas provides a pan/zoom camera for custom-painted views (spy-plot,
// incidence matrix). It tracks the transform and handles drag/scroll input.
use crate::LiveState;
use crate::alias_anim;
use crate::artifact_pane;
use crate::connection_anim;
use crate::field_help;
use crate::ic_plan_anim;
use crate::incidence_view;
use crate::log_view;
use crate::matching_anim;
use crate::matrix_panes;
use crate::nav_view;
use crate::playback::Animated;
use crate::pre_lowering_anim;
use crate::reduction_anim;
use crate::reduction_view;
use crate::sub_view_rows;
use crate::tarjan_anim;
use crate::tearing_anim;
use crate::tree;
// The worker module runs compilation and simulation on a background thread so
// the UI never blocks. Communication is via channels: we send `ToWorker`
// commands and receive `FromWorker` results. `Stage` holds one pipeline stage's
// output (its serde_json::Value IR + optional error note).
use crate::worker::{
    DefInfo, DefKind, FromWorker, LogEntry, SimData, Stage, StageBundle, StageKind, ToWorker,
    Worker, discontinuity_segments,
};

/// Initial UI zoom (fonts + spacing). Adjustable live via Settings or Ctrl +/−;
/// egui's `zoom_factor` is the idiomatic knob.
///
/// # Why this is 1.0, and why it was 2.0
///
/// **Zoom multiplies the display's own scaling — it does not replace it.** egui:
/// `pixels_per_point = zoom_factor * native_pixels_per_point`. So on a Windows
/// laptop at 150 % display scaling, a zoom of 2.0 is an *effective 3.0*, and a
/// 1920-pixel panel gives HRW **640 points** of layout width instead of 1280.
///
/// 2.0 was almost certainly right where it was written and wrong once the platform
/// moved. It predates the WSL2 → native-Windows port (`docs/architecture.md`,
/// 2026-07-27), and under WSLg a hi-dpi panel is commonly reported as
/// `native_pixels_per_point = 1.0` — so the 2.0 *was* the DPI scaling. Native
/// Windows reports the real value, and the compensation started double-counting.
///
/// **The cost was measured, not guessed** (2026-08-12). At 640 points the lab panel
/// cannot go below ~33 % of the window, because its content needs ~210 points, so
/// the lab and the stage view could not both be usable on Doug's 13" laptop —
/// `docs/ideas.md` #77. The same 640-point regime hid the divider defect that
/// [`MIN_LEFT_POINTS`] fixes, since HRW's own width tests stop at 800 points.
///
/// At 1.0 the UI renders at whatever size the display asks for, which is the
/// behaviour a reader expects from every other application on the machine.
///
/// # A known limitation, stated because Doug works across machines
///
/// On a display whose scaling is *under*-reported (the WSLg case above), 1.0 gives
/// small text and the Settings slider is the remedy — **but that choice does not
/// currently survive a restart.** `App::new` calls `set_zoom_factor` on every
/// startup, and `zoom_factor` is part of egui's persisted `Options`, so the stored
/// value is overwritten before it is ever read. Deliberate for now: startup is
/// deterministic, which is the same property `clear_persisted_split` protects. If a
/// per-machine zoom needs to stick, the fix is to apply this default only when no
/// value was persisted.
const DEFAULT_ZOOM: f32 = 1.0;

/// How often lab mode stats `.hrw-bridge/lab.md`. A quarter second is well
/// under human notice and keeps filesystem work out of the paint path.
pub(crate) const LAB_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

/// How many paints a pending frame seek keeps trying for before giving up.
///
/// Two would do — the target view needs one paint to build its animation — but a small
/// margin costs nothing and covers a view that defers construction one frame further.
const SEEK_ATTEMPTS: u8 = 5;

/// How often the scratch specimen directory is re-listed. Slower than the lab poll:
/// a specimen appearing a second late is imperceptible, and a rescan re-reads every
/// specimen's `// purpose:` line.
pub(crate) const SCRATCH_POLL_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(1000);

/// How long the breakpoint handshake waits for the VS Code extension's ack
/// before giving up and running anyway.
///
/// **The fallback exists so a missing or wedged extension cannot deadlock the
/// UI** — it is not evidence that a breakpoint was set. See
/// [`App::live_debug_poll`], which must not confuse the two, and `docs/ideas.md`
/// #71 for what happened when it did.
///
/// Named and shared with the pre-warm so the two waits cannot drift apart.
const LIVE_DEBUG_ACK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

// **THE END OF A LIVE SESSION DOES NOT RELEASE THE ANCHOR BREAKPOINT**, and the
// code that used to do it is gone rather than gated (`docs/ideas.md` #74).
//
// Its only stated purpose was to stop LLDB delivering SIGSTOP/SIGCHLD when the
// algorithm thread terminated with a breakpoint still armed — a workaround
// written 2026-07-24 under CodeLLDB, before HRW's migration to Windows and
// `cppvsdbg`. Windows has no SIGSTOP, and nothing tests the LLDB path, so what
// remained was untested code for a configuration nobody runs.
//
// **And it was actively harmful.** `cppvsdbg` will not re-bind a breakpoint at
// a location whose breakpoint left its active set earlier in the same debug
// session — by removal *or* by being disabled. So the teardown made every Debug
// press after the first arm a breakpoint VS Code drew hollow, with the algorithm
// running to completion and nothing on screen saying why.
//
// **Leaving it armed between runs is safe.** `live_trace_breakpoint` is
// unreachable outside a live session: its only callers are
// `LiveTrace::wait_for_debugger` and `LiveTrace::push`, and `push` reaches it
// only when a `frame_delay` is set — which only the animations' `start_live`
// paths do. An ordinary compile never touches it.
//
// **Three releases remain**, each ending the reason the breakpoint existed
// rather than merely pausing it: a session that failed to spawn, a specimen
// change, and app exit (`App::release_live_breakpoint_at_exit`).

/// Default specimen directory: `specimens/` next to this crate's manifest.
pub(crate) const DEFAULT_SPECIMEN_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/specimens");

/// Default library source roots: the staged reference MSL 4.1.0, one path per
/// line. Editable at runtime.
const DEFAULT_LIBRARIES: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/Modelica 4.1.0\n",
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/ModelicaServices 4.1.0\n",
    env!("CARGO_MANIFEST_DIR"),
    "/vendor/msl/Complex.mo",
);

// ---- Layout constants ----
// Named fractions for panel sizes and splits, replacing inline magic numbers.

/// Fraction of available width used by the left panel (lab text or specimen
/// list) in Lab and Specimen modes.
const LEFT_PANEL_WIDTH_FRACTION: f32 = 0.4;

/// How far the divider may be dragged, as a fraction of the window.
///
/// **Both edges are clamped, and that is not fussiness.** A divider draggable to
/// zero hides a panel *with no handle left to drag back* — the reader would have
/// to know the layout resets on a mode switch to recover, which is exactly the
/// kind of thing nobody knows at the moment they need it.
/// **One id for both left panels**, so there is one stored width and one
/// behaviour.
///
/// egui remembers a resizable panel's width per id, so `"lab_panel"` and
/// `"specimen_panel"` had **two independent widths** — the same code producing
/// different results depending on which mode a session happened to start in, and
/// on what had been dragged in each. Doug, 2026-08-02: *"The LHS width for
/// specimen mode is fixed. But, not for lab mode. Make lab mode the same as
/// specimen mode."*
///
/// **Not reproduced headlessly.** Both modes measure 0.400 in the harness, with
/// an empty lab, a short one, and one wide enough to force a scrollbar. The two
/// ids were the only divergence left, and a stored width is exactly the kind of
/// state a headless run never has — so this removes the difference rather than
/// demonstrating it. If the symptom survives, the cause is elsewhere and this
/// note is the record of what was already ruled out.
pub(crate) const LEFT_PANEL_ID: &str = "left_panel";

/// Drop any left-panel width restored from a previous session.
///
/// **Called once, from `App::new`, and that timing is the whole point.** eframe
/// restores egui's memory wholesale when it creates the `Context`
/// (`winit_integration::create_egui_context`), and that runs *before* the app
/// factory — so `cc.egui_ctx` already holds whatever width the reader last
/// dragged, and clearing it here lands exactly once, deterministically.
///
/// # Why this replaced a timer
///
/// The first fix held the default for 300 ms to outlast "whatever overwrites the
/// width after frame one". That worked and was a guess. Reading egui's source
/// settled it: `PanelState` is stored with `get_persisted`, so the width is part
/// of what eframe writes to disk.
///
/// **The symptom's direction confirms it.** Doug saw the panel too *wide*. A
/// late window maximize predicts too *narrow* — frame one at the smaller size
/// would store 40 % of a small window, a smaller fraction once maximized. Only a
/// restored, previously-dragged width explains *wide*.
pub(crate) fn clear_persisted_split(ctx: &egui::Context) {
    ctx.data_mut(|d| {
        d.remove::<egui::containers::panel::PanelState>(egui::Id::new(LEFT_PANEL_ID));
    });
}

const MODE_SWITCH_RESET: std::time::Duration = std::time::Duration::from_millis(50);

const MIN_LEFT_FRACTION: f32 = 0.15;
const MAX_LEFT_FRACTION: f32 = 0.75;

/// The narrowest the left panel may be, **in points** — a floor under the
/// fractional one.
///
/// **A fraction is the wrong unit for a minimum, and that was a real defect**
/// (Doug, 2026-08-12: *"the vertical divider refuses to go left beyond a certain
/// horizontal position. However, the right edge of the LHS content continues to
/// move leftward"*).
///
/// The panel has an intrinsic minimum width set by its own content — the lab-list
/// rows and the autoplay controls. Measured across three window sizes it sits at
/// **189–205 points** and does not move with the window, because content does not
/// care how big the screen is. `MIN_LEFT_FRACTION` *did* move with the window, and
/// the two only agreed by coincidence:
///
/// ```text
/// window 1280pt   15% floor = 192pt   content min ~192pt   agree, no symptom
/// window  640pt   15% floor =  96pt   content min ~189pt   DISAGREE
/// ```
///
/// With the floor far below that minimum, the panel's **outer** rect holds at what
/// the content needs while the **inner** `Ui` keeps taking the dragged width, so the
/// content detaches from the divider and a gap opens — measured growing from 21 to
/// **112 points** at 640pt wide. *(That the two rects diverge is measured; the exact
/// egui path by which they do is not, and is not needed for the fix.)*
///
/// **Why this was invisible for three weeks.** HRW ran at [`DEFAULT_ZOOM`] = 2.0
/// until 2026-08-12, so a 13" laptop gave it only ~640 points and the 15 % floor
/// landed under the content minimum. On a large display 15 % is comfortably above
/// it, which is why every earlier session and
/// `the_chrome_stays_on_screen_at_every_width` — which tests down to 800×600
/// *points* — never saw it. The zoom is 1.0 now, so a 13" laptop gets ~1280 points
/// and this floor is no longer the binding one there; **the defect it prevents is
/// still reachable**, by a small window or a raised zoom, and the regression test
/// covers 640 and 500 points regardless of what the default happens to be.
///
/// 210 rather than 205: above every measured minimum, so the fractional floor and
/// this one cannot straddle the content minimum again. If content grows past it,
/// `the_left_panel_content_never_detaches_from_the_divider` fails rather than the
/// gap silently returning.
const MIN_LEFT_POINTS: f32 = 435.0;

/// The draggable LHS/RHS split (`docs/ideas.md` #59).
///
/// # Who owns the width
///
/// **egui does, while the reader is dragging.** A `Panel` remembers its width
/// under its own id, so forcing a width every frame would fight the drag. This
/// struct holds two things instead: the last width *observed*, so a test can
/// assert on it (H6 — a layout property is checkable when the app records the
/// number), and a one-frame **reset request**.
///
/// # Why the reset is a flag rather than an assignment
///
/// `Panel::exact_size` collapses the size range to a point, which overrides
/// egui's remembered width for that frame — so the reset has to happen *during*
/// rendering, not when the mode changes. The flag carries the intent from the
/// mode switch to the next paint.
struct SplitState {
    /// The left panel's share of the window as of the last frame that drew it.
    /// `None` until something has been drawn.
    fraction: Option<f32>,
    /// The window width the stored panel width was computed for.
    ///
    /// A change here means the *window* resized, not that the reader dragged —
    /// and a stored pixel width is meaningless across that.
    last_avail: Option<f32>,
    /// The fraction the panel was **actually drawn at** on the last frame.
    ///
    /// Distinct from [`Self::fraction`], which is the split the *reader chose* and is
    /// deliberately not updated while the panel is pinned at a limit — see `observe`.
    /// The two agree except on a window too narrow to honour a choice, which is exactly
    /// the case the 2026-08-16 maximize bug lived in, so collapsing them back into one
    /// field would reintroduce it.
    ///
    /// Written every frame and read by tests and diagnostics: *what is on screen* is a
    /// different question from *what will be restored*.
    last_rendered: Option<f32>,
    /// How many more split changes to report to the log view.
    ///
    /// Startup is the interesting window and it is short; after that a resize is
    /// the reader's own doing and needs no commentary.
    reports_left: u8,
    /// The `available_width()` seen *inside* the panel closure — the width the LHS
    /// content was actually laid out against.
    ///
    /// **Recorded because the outer and inner widths can disagree**, and when they
    /// do the content visibly detaches from the divider ([`MIN_LEFT_POINTS`]). The
    /// outer width alone cannot see that: it was correct throughout the defect.
    /// Same reasoning as [`Self::fraction`] — a layout property is only checkable
    /// once the app records the number — and
    /// `the_left_panel_content_never_detaches_from_the_divider` is what reads it.
    inner_width: Option<f32>,
    /// Hold the default until this instant. `None` once settled, and **`None` at
    /// startup** — nothing is held there any more.
    ///
    /// The first fix for Doug's *"when HRW starts, too much horizontal space is
    /// given to the LHS"* held the default for 300 ms, to outlast whatever
    /// overwrote the width after frame one. [`clear_persisted_split`] removes the
    /// cause instead, before the first frame, so `default_size` applies for the
    /// reason it exists rather than being forced past a timer.
    ///
    /// **A countdown, not a flag, and the difference is the whole bug.** Doug,
    /// 2026-08-02: *"HRW starts in lab mode. And in lab mode, the LHS has too
    /// much width. If I switch to specimen mode, then the LHS has the desired
    /// 40%. If I switch back to lab mode, then the LHS then has the same 40%."*
    ///
    /// A one-frame reset worked for **mode switches** and not for **startup** —
    /// and specimen mode looked correct only because it is *only ever reached by
    /// a mode switch*, so it was always being reset. The asymmetry was the clue:
    /// nothing is wrong with either mode, only with the first frame.
    ///
    /// Something overwrites the width after frame one. The window is created
    /// unmaximized and maximizes shortly after, and eframe restores persisted
    /// egui memory around the same point — either would land after a single
    /// forced frame and leave the panel wherever it was left. Rather than guess
    /// which, **outlast both**: a handful of frames is imperceptible and does not
    /// depend on knowing the cause.
    reset_until: Option<std::time::Instant>,
}

impl Default for SplitState {
    fn default() -> Self {
        Self {
            fraction: None,
            last_avail: None,
            last_rendered: None,
            inner_width: None,
            reports_left: 6,
            reset_until: None,
        }
    }
}

impl SplitState {
    /// Configure a left panel: draggable, clamped, and reset when asked.
    ///
    /// Takes `avail` rather than reading it, because the caller is already
    /// inside the `Ui` whose width matters.
    fn configure(&mut self, ctx: &egui::Context, panel: egui::Panel, avail: f32) -> egui::Panel {
        let want = self.fraction.unwrap_or(LEFT_PANEL_WIDTH_FRACTION);

        // **Rescale whenever the window changes size.**
        //
        // This is the fix for the bug four other theories missed, and the
        // diagnostics named it exactly (2026-08-03):
        //
        // ```text
        // split: 0.400 of window (panel 2000px, available 5000px)
        // split: 0.750 of window (panel 1290px, available 1720px)
        // ```
        //
        // **The first frame reports a 5000 px window that does not exist.** 40 %
        // of it is 2000 px, egui stores that as an *absolute* width, and on the
        // real 1720 px window 2000 px exceeds the maximum — so it clamps to
        // 75 %, which is exactly what Doug saw.
        //
        // egui remembers a width; HRW means a *fraction*. Rewriting the stored
        // width whenever `avail` moves makes the fraction authoritative, which
        // also gives window resizing the behaviour a reader expects: the split
        // keeps its proportions instead of holding a pixel count.
        //
        // A drag at a stable width is untouched — `avail` has not changed, so
        // nothing is rewritten, and `observe` reads the new fraction back.
        let resized = self
            .last_avail
            .is_none_or(|last| (last - avail).abs() > 1.0);
        // **One range, computed once, used by both the stored width and the panel**
        // — they used to be two copies of the same expression, and a floor added to
        // one and not the other is the next version of this bug.
        let (min_w, max_w) = Self::width_range(avail);

        if resized || self.resetting() {
            let id = egui::Id::new(LEFT_PANEL_ID);
            let width = (want * avail).clamp(min_w, max_w);
            let rect = egui::containers::panel::PanelState::load(ctx, id).map_or_else(
                || egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, avail)),
                |s| {
                    let r = s.outer_rect;
                    egui::Rect::from_min_size(r.min, egui::vec2(width, r.height()))
                },
            );
            ctx.data_mut(|d| {
                d.insert_persisted(id, egui::containers::panel::PanelState { outer_rect: rect });
            });
        }
        self.last_avail = Some(avail);

        panel
            .resizable(true)
            .default_size(want * avail)
            .size_range(min_w..=max_w)
    }

    /// The left panel's permitted width range in points, for a window of `avail`.
    ///
    /// Two floors, and the **larger wins**: a fraction of the window
    /// ([`MIN_LEFT_FRACTION`]) and an absolute one ([`MIN_LEFT_POINTS`], the width
    /// the content actually needs). Taking the max is what keeps the divider's stop
    /// and the content's stop at the same place — see [`MIN_LEFT_POINTS`] for the
    /// measurements.
    ///
    /// **The minimum is then capped by the maximum**, because on a narrow enough
    /// window the absolute floor exceeds [`MAX_LEFT_FRACTION`] of it (210 pt against
    /// 75 % of 250 pt) and an inverted range is not a range. There the panel simply
    /// cannot be resized, which is honest: nothing about that width is draggable.
    fn width_range(avail: f32) -> (f32, f32) {
        let max_w = avail * MAX_LEFT_FRACTION;
        let min_w = (avail * MIN_LEFT_FRACTION).max(MIN_LEFT_POINTS).min(max_w);
        (min_w, max_w)
    }

    /// Record what was actually drawn, so the split is a number a test can read.
    ///
    /// Returns a one-line report **when the split is not where HRW put it**, for
    /// the log view.
    ///
    /// **Provisional.** This exists because five attempts at the opening width
    /// failed and the sixth succeeded the moment somebody looked at the numbers.
    /// Whether it stays is logged in `docs/tech-debt.md` — *"Remove the split
    /// reporting once the LHS width has proven itself"* — with the trigger being
    /// a stretch of ordinary use without a surprise, not a date. Three theories about the opening width have now been wrong,
    /// and each was a guess about numbers nobody had looked at. This makes them
    /// visible: `avail`, the width egui chose, and the resulting fraction.
    ///
    /// **Bounded, because an unbounded per-frame log is noise that buries the
    /// thing it was added to show.** Only changes are reported, and only the
    /// first few.
    fn observe(&mut self, width: f32, avail: f32) -> Option<String> {
        if avail <= 0.0 {
            return None;
        }
        let f = width / avail;
        let moved = self.fraction.is_none_or(|old| (old - f).abs() > 0.001);

        // **A pinned width is not a chosen one, so no fraction is learned from it.**
        //
        // Doug, 2026-08-16: *"the vertical divider bar positions far to the right
        // (~75%) when I maximize the HRW window from the normalized window size."* The
        // recorded observations named it exactly:
        //
        // ```text
        // split: 0.400 of window (panel 461px, available 1152px)   <- startup, correct
        // split: 0.750 of window (panel 200px, available  267px)   <- the jump
        // ```
        //
        // At `avail = 267` the permitted range **collapses to a point**: the maximum is
        // 267 × 0.75 = 200.25 and the 210pt floor sits above it, so the panel has
        // exactly one legal width. 0.750 was arithmetic, not a decision — and storing
        // it as a *proportion* then applied it to a maximized window, which is 75 % of
        // something much larger.
        //
        // **The floor is absolute and the memory is proportional, and that is the
        // category error.** `MIN_LEFT_POINTS` says "the content needs 210 points",
        // which is a different claim at every window size, so it must be re-derived per
        // frame rather than remembered as a ratio. Every bug in this area has been this
        // same disagreement — `SplitState` means a fraction, egui stores a width — and
        // this is the first one where the *floor itself* was the thing being
        // misremembered.
        //
        // Skipping the update keeps the last width the reader actually chose, so
        // restoring the window restores their split. `configure` still clamps to the
        // legal range every frame, so the pinned panel continues to render correctly
        // while narrow.
        self.last_rendered = Some(f);
        let (min_w, max_w) = Self::width_range(avail);
        let pinned = width <= min_w + 1.0 || width >= max_w - 1.0;
        if !pinned {
            self.fraction = Some(f);
        }
        // **Only when it is wrong.** The log view is the *compile* log, and a
        // routine startup measurement in it would break the one thing that view
        // promises: empty means nothing has compiled. Reporting only the anomaly
        // keeps that true and puts a line on screen exactly when there is
        // something to explain — which is also better instrumentation, since a
        // log that always says something is a log nobody reads.
        if !moved {
            return None;
        }
        let msg = format!(
            "split: {:.3} of window (panel {:.0}px, available {:.0}px)",
            f, width, avail,
        );
        // **Recorded before the budget is consulted, which the code below used to get
        // backwards.** The comment beneath has always said *"always to the diagnostics
        // file, only anomalies to the log view"* — and the `reports_left == 0` check sat
        // above this line, so once six observations had gone by, nothing was recorded
        // anywhere.
        //
        // That budget was sized for diagnosing *startup*, and startup spends it. Doug,
        // 2026-08-16: the divider jumps to ~70 % when the window is **maximized**, which
        // happens long after the sixth observation — so the one instrument that could
        // name the cause was already switched off by the time the bug occurred. Five
        // theories about the opening width were wrong before somebody looked at these
        // numbers; this is what keeps them lookable at.
        crate::diagnostics::record_action("split", msg.clone());
        if self.reports_left == 0 {
            return None;
        }
        self.reports_left -= 1;
        // The log view is the *compile* log and is cleared when a specimen loads, which
        // is how the first attempt at this instrument destroyed its own evidence: Doug
        // had to open a specimen to reach the log, and opening one wiped the startup
        // lines. The session file survives that, and Claude can read it directly — which
        // is why the recording above is unconditional and only this message is rationed.
        ((f - LEFT_PANEL_WIDTH_FRACTION).abs() > 0.02).then_some(msg)
    }

    /// Whether the default is currently being held.
    fn resetting(&self) -> bool {
        self.reset_until
            .is_some_and(|t| std::time::Instant::now() < t)
    }

    /// Hold the default for `window` from now.
    fn request_reset(&mut self, window: std::time::Duration) {
        self.reset_until = Some(std::time::Instant::now() + window);
    }

    /// Called once per frame, after both panels have had their chance at the
    /// reset. Clearing it inside either would leave the other still holding a
    /// dragged width on the frame a mode switch happens.
    fn end_frame(&mut self) {
        if !self.resetting() {
            self.reset_until = None;
        }
    }
}

/// Fraction of the left panel's height reserved for the specimen file list
/// (the top third). The remaining two-thirds show source or purpose note.
const SPECIMEN_LIST_HEIGHT_FRACTION: f32 = 1.0 / 3.0;

/// Fraction of available height used by the trajectory plot when solver
/// diagnostics are shown below it on the Simulation tab.
const TRAJECTORY_PLOT_HEIGHT_FRACTION: f32 = 0.65;

/// The general rule for how context gets assembled — the empty bar's hover.
///
/// The visible line names one gesture that works *right now*; this names the
/// rule behind all of them, which holds regardless of what is on screen. Both
/// are needed: the state-specific hint gets you moving, and the rule explains
/// why the same click does different things in different places — the thing
/// Doug found genuinely confusing when he first met it.
const EMPTY_CONTEXT_RULE: &str = "Context is assembled from two things: one node you POINT AT, \
and one identifier you FOLLOW. Which one a left-click gives you depends on what the view shows. \
Where things appear as names \u{2014} the specimen source, the variable grid \u{2014} left-click \
follows them. Where the view shows IR nodes \u{2014} trees, stage tabs, incidence rows \u{2014} \
left-click points at them, and right-click offers Follow for names the model knows. Hover \
anything clickable and it will say which.";

/// Where the text egui copies is caught, before egui throws it away.
///
/// # Why a plugin callback and not `ctx.output()` next frame
///
/// **That was the first attempt and it could never have worked.** `Context::end_pass`
/// does `std::mem::take(&mut viewport.output)`, so the `CopyText` command is gone
/// before the next frame begins. Reading it there found nothing, every time, and the
/// only visible symptom was a capture that silently did not happen — Doug reported the
/// two *cosmetic* faults beside it and this one had no symptom of its own at all.
///
/// `Context::on_end_pass` runs **in registration order**, after egui's own
/// `LabelSelectionState` has pushed the command and before the take. So the callback
/// sees it, stashes it here, and `App` collects it on the next frame.
type CopySink = std::sync::Arc<std::sync::Mutex<Option<String>>>;

/// Catches [`egui::OutputCommand::CopyText`] at the end of a pass, into a [`CopySink`].
///
/// # Why a plugin of our own, and not `Context::on_end_pass`
///
/// **That convenience wrapper runs too early, and the reason is one line of egui.**
/// `Context::on_end_pass` registers into egui's `CallbackPlugin` — which `Context::new`
/// adds *first*, at `context.rs:733`, **before** `LabelSelectionState` at `737`. Plugins
/// run in registration order, so a callback registered that way fires before the
/// selection plugin has pushed the copy, and finds nothing. Every time.
///
/// A plugin of a distinct type is added when `App::new` calls `add_plugin`, which is
/// after all four built-ins — so this runs last, sees the command, and still leaves it
/// in the queue for the backend to put on the clipboard.
///
/// **Three attempts, three orderings, and only this one is right.** Reading
/// `ctx.output()` on the next frame missed it because `end_pass` takes the output;
/// `on_end_pass` missed it because of the line above. Both failed silently, which is
/// what made the button look like a click problem for an evening.
struct CopyCatcher(CopySink);

impl egui::Plugin for CopyCatcher {
    fn debug_name(&self) -> &'static str {
        "hrw_copy_catcher"
    }

    fn on_end_pass(&mut self, ui: &mut egui::Ui) {
        let copied = ui.ctx().output(|o| {
            o.commands.iter().rev().find_map(|c| match c {
                egui::OutputCommand::CopyText(t) if !t.trim().is_empty() => Some(t.clone()),
                _ => None,
            })
        });
        if let Some(text) = copied
            && let Ok(mut slot) = self.0.lock()
        {
            *slot = Some(text);
        }
    }
}

/// A 🎯 press waiting for egui to hand back the text the reader selected.
///
/// # Why this needs a state machine at all
///
/// **egui does not expose a label selection's text to the application.** `has_selection()`
/// is public; the text is not. What *is* reachable is the copy: when egui performs one it
/// pushes [`egui::OutputCommand::CopyText`] into `ctx.output()`, which anyone can read.
///
/// So the button does not read the selection — it *asks egui to copy*, by pushing an
/// [`egui::Event::Copy`] into the input queue, and then collects the text egui emits in
/// response. That round trip costs frames, hence this: press on frame N, egui's
/// selection plugin acts on N+1, the text is in `output` when frame N+2 begins.
///
/// # Why it gives up rather than waiting
///
/// The button is only enabled while something is selected, but a selection can vanish
/// between press and collection — and then no `CopyText` ever arrives. Without a bound
/// this would sit armed for the rest of the session and fire on the reader's *next*
/// unrelated Ctrl+C, capturing something they never pointed at. **A capture that
/// attaches itself to the wrong gesture is worse than one that does not happen**, so it
/// expires and says so.
struct PendingPassage {
    /// The lab open when 🎯 was pressed, not when the text arrives — they cannot
    /// differ today, and recording the former is what keeps that true if they ever can.
    lab: String,
    /// Frames left before giving up. Three is two more than the round trip needs.
    frames_left: u8,
}

/// The fonts HRW installs: egui's bundled set, with **every** font made a fallback for
/// **both** families.
///
/// A glyph that lives in only one family otherwise shows as a tofu box in the other —
/// the → and ← arrows are in Hack (monospace) but not Ubuntu-Light (proportional), so
/// before this they were boxes in every ordinary label.
///
/// **What it cannot do is conjure a codepoint no bundled font has**, and that limit was
/// paid for on 2026-08-30: the scratch-specimen marker was U+270E (LOWER RIGHT
/// PENCIL), which is in none of them, so widening the fallbacks could never have helped
/// and Doug saw a box for it. Extracted from `App::new` that day so a test can ask this
/// exact font set whether a glyph exists — see
/// [`crate::model_list::tests_absence::the_scratch_marker_glyph_actually_renders`].
pub(crate) fn hrw_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let all: Vec<String> = fonts.font_data.keys().cloned().collect();
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        for name in &all {
            if !list.contains(name) {
                list.push(name.clone());
            }
        }
    }
    fonts
}

/// The algorithm **frame sets** one compile produced, grouped because they are one
/// thing.
///
/// Each is captured **during the compile that produced the IR on screen** — via the
/// capture scopes in `rumoca-phase-{flatten,dae,structural}` — rather than by
/// re-running an algorithm when its tab is opened. See `matching::start_capture` for
/// why that distinction matters: a re-derivation agrees with the compiler and still
/// depicts a search that produced nothing.
///
/// *(Reworded 2026-08-04: this said "the algorithm **replays** one compile produced",
/// which was true in the playback sense and reads as the forbidden one. See the
/// two-senses note in `CLAUDE.md` — the word is load-bearing in both directions now,
/// so this type, which holds the good kind, avoids it.)*
#[derive(Default)]
struct CompileFrames {
    /// Index reduction's demotions and differentiations.
    index_reduction: Vec<rumoca_phase_structural::dae_prepare::IndexReductionFrame>,
    /// Matching, Tarjan and tearing over the **raw** DAE — the Structural tab.
    structural: crate::worker::StructuralFrames,
    /// The same three over the **index-reduced** DAE — the Index Reduction tab.
    ///
    /// Two sets because they are two systems: on `Drivetrain` the reduced one has
    /// 20 equations to the raw one's 97, so frames from either address rows the
    /// other does not have. `structural_frames_for_stage` picks between them, in
    /// one place, because choosing at each call site is the mistake already made
    /// once here.
    reduced: crate::worker::StructuralFrames,
    /// `pre()` lowering, on the Events stage (idea #40).
    pre_lowering: Vec<rumoca_phase_dae::PreLoweringFrame>,
    /// Connection expansion (MLS §9).
    connection: Vec<rumoca_phase_flatten::connections::trace::ConnectionFrame>,
}

use crate::compile_caches::CompileViewCaches;
use crate::context_bar::{self, ContextBarPress, ContextBarState, PointKind, PointedAt};
use crate::lab::{LabSource, LabState};
use crate::lab_panel::{self, TransportRequest};
use crate::model_list::{ModelListNav, ModelListState};
use crate::report_sub_view;
use crate::specimen_purpose;
use crate::specimen_source::{self, SourceViewState};
use crate::stage_caches::StageViewCaches;
use crate::stage_tabs;
use crate::stage_view::{
    EventsView, FlattenView, InitView, StructuralView, Viewport, events_view_name,
    flatten_view_name, init_view_name, structural_view_name, sub_view_name_for,
};
use crate::ui_state::{NavEntry, SpecimenDetail, UiMode};

/// The entire application state. In immediate-mode UI, this struct IS the
/// application — every frame, `ui()` reads and writes these fields to decide
/// what to render and how to react. Fields are grouped by concern:
///
/// 1. **Worker** — the background thread that runs the compiler and simulator.
/// 2. **Library config** — MSL source roots the compiler needs for `import`.
/// 3. **Specimen list** — the directory of `.mo` files the user picks from.
/// 4. **Compilation results** — one `Stage` per pipeline phase, plus selection state.
/// 5. **Navigation** — the "go to definition" stack for drilling into classes.
/// 6. **Bridge** — communication with Claude Code (the AI assistant in the terminal).
/// 7. **View toggles** — Settings/Help/About windows.
/// 8. **Field help** — generic documentation for IR fields (the fast, no-AI tier).
/// 9. **Custom views** — spy-plot/incidence cameras for the Structural stages.
/// 10. **Log** — the compilation event log.
/// 11. **Simulation** — on-demand model execution and plotting.
pub struct App {
    // ---- 1. Worker thread ----
    // Compilation and simulation run off the UI thread so the app stays
    // responsive. We send commands (`ToWorker`) and poll results (`FromWorker`)
    // each frame via `drain_worker()`.
    worker: Worker,

    // ---- 2. Library configuration ----
    // Modelica models import classes from external libraries (mainly MSL).
    // These fields hold the library source-root paths the user has configured
    // and the status of the last library-load operation.
    libraries_text: String,
    library_status: String,
    libraries_busy: bool,

    // ---- 3. The model list ----
    /// Everything the left panel's model list owns. See [`ModelListState`].
    model_list: ModelListState,
    /// True when [`Self::selected`] names a **library model** rather than a file
    /// on disk. See [`Self::open_library_model`] for why the two share a field.
    ///
    /// **Stays on `App`, not in [`ModelListState`]**: it qualifies `selected`,
    /// which is one of the four genuinely shared fields, and every reader of one
    /// needs the other.
    selected_is_library: bool,

    // ---- 4. Current selection + compilation results ----
    // When the user clicks a specimen, `selected` records the path and
    // `compiling` goes true. The worker streams `CompileProgress` messages
    // (partial results) and finally `Compiled` (all stages done). Each `Stage`
    // holds the JSON IR + an optional note/error for that pipeline phase.
    selected: Option<PathBuf>,
    compiling: bool,
    /// The Modelica model name extracted from the parsed file (e.g.
    /// "BouncingBall"). `None` until parsing succeeds.
    model: Option<String>,
    /// All ten pipeline-stage results. Each stage holds a `Stage` (an optional
    /// JSON IR tree + optional note/error). Populated progressively during
    /// compilation via `FromWorker::CompileProgress` and finalized by
    /// `FromWorker::Compiled`.
    stages: StageBundle,
    /// Which stage tab is currently selected (determines what the center panel
    /// shows). Updated when the user clicks a tab or after compilation finishes
    /// (auto-selects the furthest successful stage).
    stage: StageKind,
    // Resolved identity of every DefId referenced in the current model's IR.
    // Shared across all stages; populated by the final `Compiled` message.
    def_index: BTreeMap<u64, DefInfo>,

    // ---- 5. "Go to definition" navigation stack ----
    // Empty means we're showing the specimen's own stages; non-empty means we
    // navigated into a library class (e.g. Resistor's resolved definition).
    // Renders as a breadcrumb trail: model > class1 > class2.
    nav: Vec<NavEntry>,
    nav_loading: Option<String>,
    nav_error: Option<String>,
    /// A 🎯 press awaiting the text egui will copy for it. See [`PendingPassage`].
    pending_passage: Option<PendingPassage>,
    /// Text caught from egui's copy, by the callback registered in `App::new`.
    copy_sink: CopySink,

    /// A transient one-line notice for the status bar.
    ///
    /// **No longer the bridge's confirmation channel.** Renamed from
    /// `bridge_status` when the Context Bar took that job: the bar names the
    /// point persistently, so a second, staler description of the same thing
    /// could only disagree with it. What is left is genuinely transient and
    /// belongs nowhere else — "specimen not found", "diagnostic written to …",
    /// a stage-file write failure.
    notice: Option<String>,

    // ---- 7. Panels and windows toggled from the menu bar ----
    ui_mode: UiMode,
    specimen_detail: SpecimenDetail,
    // egui's `Window::open(&mut bool)` pattern: the bool controls visibility,
    // and the window's close button flips it back to false.
    show_settings: bool,
    show_help: bool,
    show_about: bool,

    // ---- 8. Generic field help ----
    // `field_help` is a compile-time HashMap<field_name, explanation> loaded
    // from a generated help table, delivered as hover tooltips on tree nodes.
    field_help: HashMap<String, String>,

    // ---- 9. How the reader is looking at the current stage ----
    /// Sub-view selections, cameras and highlights. See [`Viewport`].
    viewport: Viewport,

    // ---- 10. Compilation log ----
    // Timestamped events streamed from the worker thread (phase start/end,
    // timing, diagnostics). `viewing_log` is true when the Log button (left of
    // the stage tabs) is active; auto-selected when a specimen is opened so
    // the user sees compilation progress before any stage IR is ready.
    log_entries: Vec<LogEntry>,
    viewing_log: bool,
    tracing_enabled: bool,

    // ---- 11. On-demand simulation ----
    // Simulation is NOT a compiler stage — it's triggered by the user pressing
    // "Run". `simulation` is an always-empty `Stage` placeholder so that
    // `current_stage()` can return a `&Stage` for all `StageKind` variants
    // (the Simulation view is actually rendered by `simulation_pane()`, not
    // the generic tree inspector). `sim_data` holds the time-series output
    // from the solver once the run completes.
    simulation: Stage,
    sim_data: Option<SimData>,
    sim_running: bool,
    sim_error: Option<String>,
    sim_t_end: f64,

    // ---- 12. Cached structural views ----
    // Avoids re-parsing `from_report` JSON every frame. Invalidated when
    // `stages` changes (in `drain_worker` on `Compiled`) or when the
    // active stage switches between Structural and IndexReduction (each
    // has its own report data). Outer Option is the cache state (None =
    // not yet computed); inner Option is the parse result (None = report
    // had no data for this view).
    /// Every view derived from the current stage's report. See
    /// [`StageViewCaches`] for why these eight live together and the other nine
    /// `cached_*` fields do not — and [`CompileViewCaches`] for the three that used to
    /// be here and had no business being.
    stage_views: StageViewCaches,
    cached_equation_sheet: Option<equation_sheet::EquationSheet>,
    identifier_index: Option<identifier_index::IdentifierIndex>,
    tracked_identifier: Option<String>,
    /// **Every algorithm replay the last compile captured**, in one place.
    ///
    /// Six `Vec`s of frames sat directly on `App` until 2026-08-04, when adding the
    /// fifth and sixth pushed the field-count ratchet over its bound and it asked
    /// the question it exists to ask. The answer was not "raise the number": these
    /// six are one thing — **the observations a single compile produced** — and they
    /// arrive together on `FromWorker::Compiled`, are replaced together, and are read
    /// only by the animations.
    ///
    /// Grouping them takes `App` from 60 fields to 55, which is the ratchet working
    /// as designed rather than being appeased.
    frames: CompileFrames,
    cached_flat: Option<rumoca_ir_flat::Model>,
    /// **The replays built from `frames`**, which live exactly as long as `frames` does.
    ///
    /// One field where `pre_lowering_anim` used to sit alone, because it never was alone:
    /// `reduction_anim`, `connection_anim` and `ic_plan_anim` have the same lifetime and
    /// were filed under [`StageViewCaches`], which dropped them whenever a *report* stage
    /// was entered. See [`CompileViewCaches`] for what that did and why nobody designed
    /// it.
    compile_views: CompileViewCaches,
    cached_dae: Option<rumoca_ir_dae::Dae>,

    // ---- 13. Markdown rendering ----
    // Caches parsed markdown for `egui_commonmark`. Shared across lab and
    // purpose-note rendering so heading IDs and image state persist across frames.
    commonmark_cache: egui_commonmark::CommonMarkCache,
    /// Source lines the compiler blamed, as `(1-based line, why)`.
    ///
    /// Populated **only when the model is genuinely ill-posed** — see
    /// [`Self::compute_problem_lines`]. Recomputed once per compile, never per frame.
    problem_lines: Vec<(u32, String)>,
    /// The draggable LHS/RHS split. See [`SplitState`].
    split: SplitState,
    /// Everything the Context Bar owns. See [`ContextBarState`].
    context: ContextBarState,
    /// Everything the source view owns. See [`SourceViewState`].
    source: SourceViewState,
    /// Everything the lab panel owns. See [`LabState`].
    ///
    /// Polled rather than watched: stat-ing once per frame would be cheap but
    /// puts filesystem work in the paint path, which the debugging conventions
    /// rule out. A few polls a second is indistinguishable to a reader, and a
    /// lab Claude writes mid-conversation still appears without a restart.
    ///
    /// `pub(crate)` for `ui_tests` only: a headless test has to be able to say "an ad
    /// hoc lab exists" without writing `.hrw-bridge/lab.md`, which is shared with a
    /// running HRW and asserted *absent* by another test. Injecting the state is the
    /// race-free way to give a check a subject — and without one,
    /// `the_ad_hoc_lab_is_a_button_and_not_a_picker_entry` passed while the ad hoc
    /// lab was listed in both places.
    pub(crate) lab: LabState,
    /// A pending camera aim from `hrw://…/equation/<n>`, consumed by whichever canvas
    /// view paints next. `None` means no link asked for one.
    ///
    /// Held on the app rather than pushed straight into a `Canvas` because the target
    /// is an *equation index*, and turning that into a world position needs the view's
    /// own layout — which only exists at paint time.
    aim_at_equation: Option<usize>,
    /// A pending frame seek from `hrw://…/frame/<n>`: the target frame and how many
    /// more paints to keep trying for.
    ///
    /// **The budget is not decoration.** The animation for a newly-selected sub-view is
    /// not built until that view paints, so the first attempt after navigation always
    /// misses. Retrying unboundedly would be worse: a seek aimed at a view that *never*
    /// has an animation (Station 6 of the frame-seeking fixture) would sit armed until the
    /// reader wandered into an animated view, and then fire there — a link taking effect
    /// somewhere it was never pointed at.
    seek_frame: Option<(usize, u8)>,
    /// Specimen purpose notes, memoised by **model name** — misses included, so an
    /// unnoted model does not re-stat an absent file every frame. Read and rendered by
    /// [`crate::specimen_purpose`].
    ///
    /// **The doc block that used to sit here described a field that had already left.**
    /// It read *"when the scratch specimen directory was last polled … moved to
    /// `ModelListState::polled_at` on 2026-08-02"* — a `///` block whose own field was
    /// deleted, adopted by the next one down, which then carried a plain `//` comment
    /// nobody could see in rustdoc. Found 2026-08-21 while extracting this pane; it is
    /// the class the doc-comment sweep exists for, and the destination
    /// (`model_list.rs`) carries its own doc, so the orphan was residue rather than
    /// information.
    cached_purpose_notes: HashMap<String, Option<String>>,
    // Every variable name in the compiled model — ground truth for which tree
    // leaves name something trackable. Rebuilt per compile alongside the
    // equation sheet, which is where the full classification lives.
    known_variables: Option<HashSet<String>>,
    // Variable name -> the class that declares it, when that is not the
    // specimen. See `build_declaring_classes`.
    declaring_classes: HashMap<String, String>,

    // ---- 14. Pending stage from hrw:// link ----
    // When an hrw://load/Specimen/Stage link fires, the stage can't be applied
    // immediately — compilation is async and drain_worker will auto-select the
    // last successful stage. This field defers the stage switch until the
    // Compiled message arrives.
    pending_stage: Option<StageKind>,
    /// Sub-view requested by an `hrw://load/…/<Stage>/<SubView>` link, applied
    /// alongside `pending_stage` once the compile lands. Separate from the stage
    /// because it must be applied *after* the default-sub-view logic, which would
    /// otherwise overwrite it.
    pending_sub_view: Option<SubView>,

    // ---- 15. Deferred live debug spawn (ack handshake) ----
    // When the Debug button is clicked, `arm_live_trace_breakpoint` writes a
    // breakpoint request to `.hrw-bridge/breakpoint-request.json`. The algorithm
    // thread is NOT spawned immediately — the VS Code extension must process the
    // file and register the breakpoint with LLDB first. Each UI frame, we poll
    // `bridge::check_breakpoint_ack()` for the ack file the extension writes
    // after arming. The thread launches once the ack arrives (or after a
    // 3-second timeout if the extension isn't running). The `Instant` records
    // the request time for the timeout; the enum says which algorithm to spawn.
    pending_live_debug: Option<(std::time::Instant, PendingLiveDebug)>,
    // True while a `live_trace_breakpoint` is armed by the Debug button.
    //
    // **It stays true across runs, which is the point.** The two session-end
    // clears that used to exist — the algorithm thread's `on_complete` callback
    // and a safety net in `live_debug_poll` — are gone, because releasing the
    // anchor is what stopped the *next* Debug press working (`docs/ideas.md`
    // #74).
    //
    // What clears it is the set of events that end the breakpoint's reason to
    // exist: a `start_live` that failed to spawn, a specimen change, and app
    // exit.
    live_breakpoint_armed: bool,

    // ---- 16. Breakpoint pre-warm ----
    // See `Prewarm` and `tick_prewarm`. Runs once, early, so the debugger's
    // first (slow) resolution of live_trace.rs does not happen on the critical
    // path of the first Debug click.
    prewarm: Prewarm,
}

/// `PartialEq` so "is the pending session this view's?" is `==`.
///
/// It used to be a hand-written list of matching pairs — `(Matching, Matching) |
/// (Tarjan, Tarjan) | (Reduction, Reduction)` — in two places. Adding a fourth
/// variant compiled cleanly and silently never matched, so the Debug button did
/// nothing at all: no error, no arming badge, no session. Derived equality
/// cannot go stale that way.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PendingLiveDebug {
    Matching,
    Tarjan,
    Reduction,
    /// Idea #40. Unlike the other three it replays a phase of *DAE
    /// construction*, so it re-runs from the flat model rather than the DAE.
    PreLowering,
    /// Tearing. Re-runs from the DAE, like Reduction, because tearing needs the
    /// BLT blocks and those are rebuilt from the incidence each time.
    Tearing,
    /// Connection expansion — **the only one the worker runs**, and the only one
    /// that does not re-run its phase.
    ///
    /// The others spawn a thread here on copied data. This pass lives inside
    /// `compile_model_strict_reachable_*`, needing the session and the resolved
    /// `ClassTree`, so the request goes to the worker instead
    /// (`ToWorker::LiveDebugConnections`) and the reader steps the real compile.
    Connections,
}

impl PendingLiveDebug {
    /// Every variant, so a test can check the arming machinery handles all of
    /// them without naming them. Add new variants here.
    ///
    /// Test-only: nothing in the app iterates the variants, because each view
    /// names its own. That is exactly why the omission this guards against was
    /// silent.
    #[cfg(test)]
    const ALL: &'static [PendingLiveDebug] = &[
        PendingLiveDebug::Matching,
        PendingLiveDebug::Tarjan,
        PendingLiveDebug::Reduction,
        PendingLiveDebug::PreLowering,
        PendingLiveDebug::Tearing,
        PendingLiveDebug::Connections,
    ];
}

enum LiveDebugAction {
    None,
    SpawnLive,
}

/// One frame of the live-debug handshake, answered for a single animated view.
///
/// # Why this exists
///
/// Six views — Matching, Tarjan, Reduction, Tearing, Connections and
/// `pre()` lowering — opened with the *same* eighteen-line prologue: ask
/// [`App::is_arming`], fold the cached animation's [`LiveState`], gate the
/// button on [`App::has_live_debug_data`], then advance
/// [`App::live_debug_poll`]. Six copies of one protocol, with nothing
/// requiring them to agree — a seventh view could have been written with the
/// steps in the wrong order, or with one missing, and it would have compiled.
///
/// **The point is not the lines saved, it is that the protocol now has one
/// implementation.** A view gets these three answers or it does not get them.
///
/// What is *not* here is deliberate: constructing the live animation, building
/// the recorded fallback, and drawing genuinely differ per view — see
/// [`App::live_debug_gate`] for the seam and what stayed behind it.
struct LiveDebugGate {
    /// This view's Debug press is still waiting for the bridge's ack, so its
    /// controls show the "Arming…" badge and stay disabled.
    arming: bool,
    /// The Debug button may be pressed this frame: the view has the data its
    /// algorithm needs, and no session is already running.
    debug_enabled: bool,
    /// True on the single frame the ack lands (or the wait times out), which is
    /// when the caller should start its algorithm.
    spawn_live: bool,
}

/// State of the one-shot breakpoint pre-warm.
///
/// ## The problem this solves
///
/// The first time a debugger is asked for a breakpoint in a given source file,
/// it must resolve that file to a compilation unit and load its line table.
/// For `hrw.exe` — a 21 MB `.text` section with a correspondingly large PDB —
/// that first resolution takes well over a second. Every later breakpoint in
/// the same file is fast, because the module is already warm.
///
/// That cold cost lands in exactly the wrong place. The Debug button's
/// handshake gives the debugger only ~500 ms (`LiveTrace::wait_for_debugger`)
/// between the extension's ack and the algorithm thread reaching the anchor,
/// and the ack means "VS Code has been told", not "the debugger has installed
/// it". So the *first* Debug click of a session missed its breakpoint and the
/// algorithm ran to completion; the second click worked, and every one after.
///
/// Rather than lengthen the wait — a guess that would be paid on every live
/// debug start forever — this moves the cold resolution off the critical path:
/// arm the anchor once at startup, wait for the ack, then remove it. Nothing is
/// left armed, but the debugger has loaded the line table by the time the user
/// clicks Debug, so the existing 500 ms is ample.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Prewarm {
    /// Nothing sent yet. The arm request goes out on the first UI frame.
    NotStarted,
    /// Arm request written; waiting for the extension's ack before removing it.
    /// The `Instant` bounds the wait for the case where no extension is running
    /// (HRW launched outside VS Code, or the bridge extension disabled).
    Awaiting(std::time::Instant),
    /// Finished — either warmed, timed out, or deliberately abandoned.
    Done,
}

impl App {
    /// Advance the one-shot breakpoint pre-warm. Called once per UI frame.
    ///
    /// See [`Prewarm`] for why this exists. The sequence is arm → await ack →
    /// remove, and it must be a sequence rather than an arm/remove pair: both
    /// requests are written to the *same* file
    /// (`.hrw-bridge/breakpoint-request.json`), so removing before the extension
    /// has read the arm request would simply overwrite it.
    fn tick_prewarm(&mut self, ctx: &egui::Context) {
        self.tick_prewarm_at(ctx, std::path::Path::new(bridge::BREAKPOINT_REQUEST_FILE));
    }

    /// [`tick_prewarm`](Self::tick_prewarm), driving the handshake through
    /// `request_path` instead of the bridge's own file.
    ///
    /// # Why this seam exists
    ///
    /// **The pre-warm arms the anchor for real**, and `.hrw-bridge/` is watched by
    /// Doug's running VS Code, so a test driving this state machine puts a
    /// breakpoint in his editor — which the extension will later refuse to remove,
    /// because it only clears breakpoints it added and a window reload empties that
    /// list (`bridge::arm_live_trace_breakpoint_to`).
    ///
    /// This was missed by the first pass at that fix, which moved only `bridge.rs`'s
    /// own tests to a temp path. The evidence was an ack written *during* a full
    /// gate run reading `"action":"add"` — a test suite arming a breakpoint while
    /// claiming it no longer could. The ack file is derived beside `request_path`
    /// for the same reason: consuming the live one would steal the reply the
    /// running app is waiting for.
    fn tick_prewarm_at(&mut self, ctx: &egui::Context, request_path: &std::path::Path) {
        let ack_path = request_path.with_file_name("breakpoint-ack.json");
        match self.prewarm {
            Prewarm::NotStarted => {
                // No specimen is loaded yet, so pass no model name — this arms
                // the anchor purely to force line-table resolution.
                if bridge::arm_live_trace_breakpoint_to(request_path, None).is_ok() {
                    self.prewarm = Prewarm::Awaiting(std::time::Instant::now());
                    ctx.request_repaint();
                } else {
                    // No bridge directory (or it is not writable). Live debug
                    // will not work either way; nothing to warm.
                    self.prewarm = Prewarm::Done;
                }
            }
            Prewarm::Awaiting(started) => {
                // A Debug click owns the handshake from here on. Abandon rather
                // than consume the ack it is waiting for — `check_breakpoint_ack`
                // deletes the file it reads, so polling here would make the
                // Debug click miss its own ack and fall back to its 3s timeout.
                if self.pending_live_debug.is_some() || self.live_breakpoint_armed {
                    self.prewarm = Prewarm::Done;
                    return;
                }
                // The pre-warm only needs to know the extension *replied* — it
                // is warming the debugger's line resolution, not arming
                // anything, and it removes the breakpoint either way. Which
                // verdict came back is the Debug click's business.
                let replied = bridge::check_breakpoint_ack_at(&ack_path).replied();
                let timed_out = started.elapsed() >= LIVE_DEBUG_ACK_TIMEOUT;
                if replied || timed_out {
                    // Remove it again: pre-warming must not leave a breakpoint
                    // armed. Resolution stays cached in the debugger regardless.
                    let _ = bridge::remove_live_trace_breakpoint_to(request_path);
                    self.prewarm = Prewarm::Done;
                } else {
                    ctx.request_repaint();
                }
            }
            Prewarm::Done => {}
        }
    }

    /// Build and begin a self-running walk of the lab currently showing.
    ///
    /// Only links that **parse** become beats. A lab that names a verb in prose
    /// would otherwise contribute a beat that dispatches nothing and stalls the run
    /// on a blank screen — and since `parse_hrw_link` is the same gate
    /// `fixture_lab_links_all_resolve` applies, a scheduled run and a checked lab
    /// cannot disagree about what a link is.
    fn start_autoplay(&mut self) {
        let Some(text) = self.lab.text().map(str::to_owned) else {
            self.notify("no lab is showing \u{2014} pick one first");
            return;
        };
        let mut stops = crate::autoplay::parse_stations(&text);
        for stop in &mut stops {
            stop.links.retain(|l| parse_hrw_link(&l.url).is_some());
        }
        let beats = crate::autoplay::schedule(&stops, self.lab.autoplay_total, |l| {
            // An external hop needs longer on screen. The ruling is per verb and lives
            // on the verb — see `HrwLink::leaves_hrw`, which is exhaustive so a new
            // form cannot default into the wrong beat length unnoticed.
            parse_hrw_link(l).is_some_and(|link| link.leaves_hrw())
        });
        if beats.is_empty() {
            self.notify("this lab has no stops to play");
            return;
        }
        // Remember where we started, so the run can put it back. A stop may
        // legitimately leave Lab mode — `hrw://source/<line>` must, since the
        // source only renders in Specimen mode — and `matching.md` ends Station 3 with
        // exactly that, so the walk used to finish with the lab off screen.
        self.lab.mode_before_autoplay = Some(self.ui_mode);

        // **Start from where the pane is, not from where the last run stopped.**
        //
        // These positions are measured per document per beat, and a stopped run
        // leaves its last one behind. The first frame of a new run interpolates
        // *from* that value, so pressing Play scrolled to the old spot and then
        // travelled back — visibly, over the full travel window, before the lab
        // had begun. Clearing them makes both ends of the first interpolation zero,
        // so a lab already at the top simply does not move.
        self.lab.reset_scroll();

        if let Some(first) = self.lab.autoplay.start(beats) {
            self.dispatch_beat(first);
        }
    }

    /// End a run and restore what it borrowed.
    ///
    /// **A walk is a round trip.** Called both when the last beat elapses and when
    /// Stop is pressed, because a viewer who stops halfway is no more interested in
    /// being left in Specimen mode than one who watches to the end.
    ///
    /// Only the *mode* is restored, not the stage or the specimen: those are the
    /// result of the walk and worth keeping on screen. It is the **frame** the lab
    /// was being read in that has to come back.
    fn restore_mode_after_autoplay(&mut self) {
        if let Some(mode) = self.lab.mode_before_autoplay.take()
            && self.ui_mode != mode
        {
            self.ui_mode = mode;
            self.split.request_reset(MODE_SWITCH_RESET);
        }
    }

    /// Advance a running lab by one frame.
    ///
    /// **Pauses itself when the window loses focus.** An external stop brings
    /// Wolfram Desktop or System Modeler to the front, and a clock that kept
    /// running behind another window would advance HRW while nobody was watching
    /// it — the recording would return to a lab that had moved on without them.
    /// Clicking back into HRW resumes. That makes an external hop as long as the
    /// viewer wants rather than as long as the schedule guessed.
    fn tick_autoplay(&mut self, ctx: &egui::Context) {
        if !self.lab.autoplay.is_running() {
            return;
        }
        self.lab.autoplay.set_focused(ctx.input(|i| i.focused));

        // `stable_dt` rather than `unstable_dt`: a single slow frame should not
        // jump the walk forward by its own hitch.
        let dt = std::time::Duration::from_secs_f32(ctx.input(|i| i.stable_dt).min(0.25));
        if let Some(next) = self.lab.autoplay.tick(dt, self.compiling) {
            self.dispatch_beat(next);
        }
        // The last beat has elapsed: put the mode back before the reader notices it
        // moved. A stop that switched to Specimen mode (`hrw://source/<line>`) would
        // otherwise leave the walk ending with no lab on screen.
        if self.lab.autoplay.phase() == crate::autoplay::Phase::Finished {
            self.restore_mode_after_autoplay();
        }
        // A timed run must keep painting even when nothing else asks it to.
        ctx.request_repaint();
    }

    /// Apply one beat: dispatch its link, if it has one.
    ///
    /// A prose-only beat is not a no-op — it is a deliberate pause on the stop's
    /// caption, which is how a title card and a section break get their moment.
    fn dispatch_beat(&mut self, beat: crate::autoplay::Beat) {
        let Some(link) = beat.link.as_deref().and_then(parse_hrw_link) else {
            return;
        };
        self.dispatch_hrw_link(link);
    }

    /// Whether this view has the data its algorithm needs — gates the Debug button.
    ///
    /// **Every variant is named, and the wildcard arm is deliberately absent.**
    /// This used to end in `_ => matches!(&self.stage_views.incidence, …)`, so a
    /// seventh view would have compiled cleanly and silently been told to look
    /// for an incidence matrix it may have no use for — the *same* silent-omission
    /// shape that `every_live_debug_variant_is_recognised_while_arming` was
    /// written for after the `pre()`-lowering Debug button did nothing. Listing
    /// the two makes the next view a compile error instead of a wrong answer.
    fn has_live_debug_data(&self, variant: PendingLiveDebug) -> bool {
        match variant {
            PendingLiveDebug::Reduction | PendingLiveDebug::Tearing => self.cached_dae.is_some(),
            // The flat model, not the DAE: `pre()` lowering runs inside DAE
            // construction, so the DAE is already past it.
            PendingLiveDebug::PreLowering => self.cached_flat.is_some(),
            // A specimen with a compiled model is all the worker needs; the
            // session it will re-compile through lives there, not here.
            PendingLiveDebug::Connections => self.selected.is_some() && self.model.is_some(),
            // Both search the incidence matrix the Structural report carries.
            PendingLiveDebug::Matching | PendingLiveDebug::Tarjan => {
                matches!(&self.stage_views.incidence, Some(Some(_)))
            }
        }
    }

    /// Arm a live debug session.
    ///
    /// Called when the Debug button reports a click. That button is rendered by
    /// `animation_controls`, so it sits on the same row as the playback
    /// controls; it cannot arm the session itself because that needs app state
    /// (the bridge, the model name, `pending_live_debug`), so it returns the
    /// click and the caller lands here.
    fn start_live_debug(&mut self, variant: PendingLiveDebug) {
        let _ = bridge::arm_live_trace_breakpoint(self.model.as_deref());
        self.pending_live_debug = Some((std::time::Instant::now(), variant));
    }

    /// Advance the live-debug handshake. Renders nothing.
    ///
    /// Returns `SpawnLive` on the frame the ack lands (or the timeout expires),
    /// at which point the caller should start the algorithm thread.
    ///
    /// **It no longer takes the `LiveState`, because there is no session-end
    /// safety net.** One used to fire the moment a session stopped being busy,
    /// on the reasoning that an armed breakpoint with nothing in flight has
    /// nothing left to stop for. That is true only until the next Debug press —
    /// and releasing the anchor is what made that press fail, silently
    /// (`docs/ideas.md` #74). With the release gone, the state was the only
    /// thing this function read it for.
    ///
    /// # Why the ack file arrives as a parameter
    ///
    /// [`bridge::check_breakpoint_ack_at`] exists so a test can exercise the
    /// consume-and-delete behaviour without touching the live bridge directory,
    /// and this function used to throw that seam away by calling the
    /// default-path wrapper. **The consequence was a property that could not be
    /// asserted at all**: reaching the frame the ack lands on means writing a
    /// real `.hrw-bridge/breakpoint-ack.json`, which every other test in the
    /// suite shares — so the tests that do it must run as one function to avoid
    /// racing for that file, and [`Self::live_debug_gate`]'s ordering could not
    /// be tested from the gate at all. Taking the path forwards the seam that
    /// was already one layer down.
    fn live_debug_poll(
        &mut self,
        ctx: &egui::Context,
        variant: PendingLiveDebug,
        ack_path: &Path,
    ) -> LiveDebugAction {
        if let Some((armed_at, v)) = self.pending_live_debug
            && v == variant
        {
            let ack = bridge::check_breakpoint_ack_at(ack_path);
            let timed_out = armed_at.elapsed() >= LIVE_DEBUG_ACK_TIMEOUT;
            if ack.replied() || timed_out {
                self.pending_live_debug = None;

                // **Only a verdict of `Armed` means a breakpoint exists.**
                // Recording the timeout as `true` was a claim HRW had no
                // grounds for — and it did not stay on screen: the context
                // capture emits `breakpoint_armed`, so the fiction reached
                // Claude's reasoning too. `docs/ideas.md` #71.
                //
                // A late ack (after the timeout) therefore leaves a real
                // breakpoint that HRW no longer tracks. That is the honest
                // trade: **HRW must not claim state it cannot see**, and the
                // "HRW: Clear Armed Breakpoints" command exists for exactly
                // this. Pretending otherwise bought tidy bookkeeping with a
                // false statement.
                //
                // The *reply* itself is no longer evidence — `#75`. The
                // extension acks every request it reads, including ones that
                // armed nothing, so `replied()` ends the wait while `is_armed()`
                // decides the claim.
                self.live_breakpoint_armed = ack.is_armed();

                // **The silence was the defect.** The animation runs to the end
                // either way, and a learner cannot tell "the bridge is not
                // installed" from "this phase has nothing to stop at" — the
                // second being a perfectly plausible thing to believe about a
                // compiler phase. Each case now names itself.
                match &ack {
                    bridge::BreakpointAck::Armed => {}
                    bridge::BreakpointAck::Pending => self.notify(
                        "no reply from the HRW Debugger Bridge \u{2014} running without a \
                         breakpoint. Is the extension built and installed? See \
                         hrw/vscode-extension/README.md",
                    ),
                    bridge::BreakpointAck::NotArmed(why) => self.notify(format!(
                        "the breakpoint was NOT armed, so this run will not stop: {why}"
                    )),
                    bridge::BreakpointAck::Unreportable => self.notify(
                        "the HRW Debugger Bridge replied in an old format and cannot say \
                         whether it armed anything \u{2014} running without a breakpoint. \
                         Rebuild it: cd hrw/vscode-extension && npm run build, then reload \
                         the VS Code window.",
                    ),
                }
                return LiveDebugAction::SpawnLive;
            }
            // No status text here — the control row already shows the
            // "Arming…" badge via `LiveState::badge`.
            ctx.request_repaint();
        }
        LiveDebugAction::None
    }

    /// Whether a live debug session is being armed for this particular view.
    ///
    /// The animations cannot work this out themselves: during the handshake the
    /// view still holds the *recorded* animation, so its own `is_live()` is
    /// false. Without this, the controls stayed enabled for the several frames
    /// between the Debug click and the algorithm thread spawning.
    fn is_arming(&self, variant: PendingLiveDebug) -> bool {
        self.pending_live_debug.is_some_and(|(_, v)| v == variant)
    }

    /// Advance the live-debug handshake for one animated view. Renders nothing.
    ///
    /// **The body is [`Self::live_debug_gate_at`]**; this is the default-path
    /// wrapper the six views call, in the same shape as
    /// [`bridge::check_breakpoint_ack`] over `check_breakpoint_ack_at`. What is
    /// written here is what a *caller* needs; the reason the seam exists is on
    /// the `_at` form.
    ///
    /// The prologue every animated view shares, in the order it must happen:
    /// **arming, then the button gate, then the poll.** The order is load-bearing
    /// — `live_debug_poll` clears `pending_live_debug` on the frame the ack
    /// lands, so asking `is_arming` after it would report `false` on the very
    /// frame the badge is still wanted.
    ///
    /// # Why the cache arrives as a `fn` pointer
    ///
    /// The [`LiveState`] fold reads the view's own animation cache, whose type
    /// differs per view; the poll needs `&mut self`. Taking `&Option<Option<A>>`
    /// directly would hold an immutable borrow of `self` across the mutable
    /// call. A `fn(&Self) -> _` accessor is applied and dropped inside, so each
    /// caller writes `|a| &a.stage_views.tarjan_anim` and the borrows never
    /// overlap. It is a `fn` rather than a closure so it costs no allocation
    /// and cannot capture — the accessor is a field path, nothing more.
    ///
    /// # What stayed with the callers
    ///
    /// Everything after `spawn_live`. Starting the live session differs in kind
    /// and not just in detail — five views spawn a thread over copied data while
    /// [`PendingLiveDebug::Connections`] hands a producer to the worker — and the
    /// recorded fallback and the drawing differ again. Folding those in would
    /// mean a parameter per difference, which is the duplication back in another
    /// shape.
    fn live_debug_gate<A: crate::playback::Animated>(
        &mut self,
        ctx: &egui::Context,
        variant: PendingLiveDebug,
        cache: fn(&Self) -> &Option<Option<A>>,
    ) -> LiveDebugGate {
        self.live_debug_gate_at(ctx, variant, cache, Path::new(bridge::BREAKPOINT_ACK_FILE))
    }

    /// [`Self::live_debug_gate`] against an arbitrary ack file.
    ///
    /// Named for [`bridge::check_breakpoint_ack_at`], which is where the seam
    /// originates and what it is for: **the ordering below is the whole point of
    /// this method existing, and it cannot be observed without controlling the
    /// ack file.** `live_debug_poll` clears `pending_live_debug` on the frame the
    /// ack lands, so a test that wants to see `arming` and `spawn_live` true
    /// *together* — the frame where the badge is still wanted and the thread is
    /// about to start — must be able to make the ack land on demand, in a file
    /// no other test is reading.
    ///
    /// The default-path wrapper above is what the six views call; nothing in the
    /// paint path knows this parameter exists.
    fn live_debug_gate_at<A: crate::playback::Animated>(
        &mut self,
        ctx: &egui::Context,
        variant: PendingLiveDebug,
        cache: fn(&Self) -> &Option<Option<A>>,
        ack_path: &Path,
    ) -> LiveDebugGate {
        let arming = self.is_arming(variant);
        let live = cache(self).as_ref().and_then(|o| o.as_ref()).map_or(
            if arming {
                LiveState::Arming
            } else {
                LiveState::Idle
            },
            |a| a.live_state(arming),
        );
        let debug_enabled = self.has_live_debug_data(variant) && !live.is_busy();
        let spawn_live = matches!(
            self.live_debug_poll(ctx, variant, ack_path),
            LiveDebugAction::SpawnLive
        );
        LiveDebugGate {
            arming,
            debug_enabled,
            spawn_live,
        }
    }

    /// Create the application. Called once by eframe at startup.
    ///
    /// Three things happen here that are worth understanding:
    ///
    /// 1. **Font fallback trick**: egui has two font families (Proportional and
    ///    Monospace), each with its own list of fonts. Some glyphs (like arrows
    ///    and ) only exist in the monospace font (Hack), so proportional labels
    ///    would show blank squares ("tofu") for them. The fix: add every loaded
    ///    font as a fallback for BOTH families, so egui searches all fonts when
    ///    a glyph is missing from the primary.
    ///
    /// 2. **Zoom factor**: rather than changing individual font sizes, we scale
    ///    the entire UI (fonts + widget spacing) via `set_zoom_factor`. This
    ///    keeps the Settings slider and Ctrl+/- keyboard shortcuts working
    ///    consistently.
    ///
    /// 3. **Worker spawn**: the background thread is created here with a clone
    ///    of the egui context. The worker uses that context to call
    ///    `request_repaint()` after sending results, which wakes the UI thread
    ///    to process them — without this, egui would only repaint on user input
    ///    and results could sit unseen in the channel.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Before anything else: install the panic hook, so a crash during
        // startup still writes a file. Everything below this line — font
        // loading, the worker spawn — can fail, and did once.
        diagnostics::init();
        diagnostics::record_action("session", "HRW started");

        // Before the first frame: drop a panel width restored from a previous
        // session, so the split opens where HRW says rather than where the
        // reader last left it. See `clear_persisted_split`.
        clear_persisted_split(&cc.egui_ctx);

        cc.egui_ctx.set_fonts(hrw_font_definitions());

        // **Catch the copy before egui discards it**, with a plugin of our own so it
        // runs AFTER egui's `LabelSelectionState` rather than before it. See
        // [`CopyCatcher`] for why the obvious `Context::on_end_pass` wrapper cannot
        // work — it registers into a plugin egui adds four lines earlier.
        //
        // It observes without consuming: an ordinary Ctrl+C still reaches the
        // clipboard, because the command stays in the queue for the backend.
        let sink: CopySink = CopySink::default();
        cc.egui_ctx
            .add_plugin(CopyCatcher(std::sync::Arc::clone(&sink)));

        // Scale the whole UI (fonts + spacing) via egui's zoom, not by mutating
        // individual text styles — so the Settings slider and Ctrl +/− both work.
        cc.egui_ctx.set_zoom_factor(DEFAULT_ZOOM);

        // Spawn the worker thread. It gets a clone of the egui context so it
        // can wake the UI when results are ready (see `drain_worker`).
        let worker = Worker::spawn(cc.egui_ctx.clone());
        let mut app = App {
            worker,
            libraries_text: DEFAULT_LIBRARIES.to_owned(),
            library_status: String::new(),
            libraries_busy: false,
            model_list: ModelListState::default(),
            selected_is_library: false,
            selected: None,
            compiling: false,
            model: None,
            stages: StageBundle::default(),
            stage: StageKind::Resolve,
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
            field_help: field_help::load(),
            pending_passage: None,
            // The same handle the end-of-pass callback writes into.
            copy_sink: sink,
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
            pending_stage: None,
            pending_sub_view: None,
            pending_live_debug: None,
            live_breakpoint_armed: false,
            prewarm: Prewarm::NotStarted,
        };
        // Scan the specimen directory and pre-load libraries at startup so the
        // Resolve phase works immediately when the user selects a specimen
        // (Resolve needs the MSL classes to resolve `import` references).
        app.model_list.rescan();
        app.load_libraries();
        app
    }

    /// Parse the multi-line library text field into a list of paths, one per
    /// non-empty line. The text field is editable in the Settings window.
    fn parse_library_paths(&self) -> Vec<PathBuf> {
        self.libraries_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect()
    }

    /// Send the current library paths to the worker thread for loading.
    /// Library loading is async — the result arrives as `FromWorker::Libraries`.
    fn load_libraries(&mut self) {
        let roots = self.parse_library_paths();
        self.libraries_busy = true;
        self.library_status = format!("loading {} source root(s)…", roots.len());
        self.worker.send(ToWorker::SetLibraries(roots));
    }

    /// Find a specimen by model name (e.g. "BouncingBall" → `specimens/BouncingBall.mo`).
    fn find_specimen(&self, name: &str) -> Option<PathBuf> {
        let with_ext = format!("{name}.mo");
        self.model_list
            .files
            .iter()
            .find(|p| p.file_name().and_then(|f| f.to_str()) == Some(with_ext.as_str()))
            .cloned()
    }

    /// Open (compile) a specimen. Called when the user clicks a file in the
    /// left panel or follows an `hrw://load/` link.
    ///
    /// **Why everything resets:** each specimen is an independent compilation.
    /// Leftover stage data from the previous specimen would be confusing (e.g.
    /// showing BouncingBall's events tab while compiling Drivetrain). So we
    /// clear every stage, the navigation stack, the simulation results, and the
    /// field-help selection before sending the new compile command.
    ///
    /// **Why `viewing_log` starts `true`:** compilation takes a moment, and
    /// showing the log view lets the user watch progress (phase start/end
    /// messages, timing) instead of staring at "compiling..." text. Once
    /// compilation finishes and the user clicks a stage tab, `viewing_log`
    /// flips to false.
    fn open(&mut self, path: PathBuf) {
        // Reselecting the SAME specimen keeps the assembled context.
        //
        // Recompiling used to wipe the point and the follow unconditionally,
        // which made one workflow impossible: assembling context, asking for
        // breakpoints, then recompiling to hit them — because the recompile
        // destroyed the very context that motivated the breakpoints. Doug hit
        // this the first time the breakpoints actually worked.
        //
        // Clearing is right when the specimen *changes*: a key-path addresses
        // one model's IR and means nothing in another's. For a reselect the IR
        // is normally identical, so the point still resolves and the followed
        // name still exists. "Normally" is not "always" — the file may have been
        // edited between loads — so the retained point is *validated* against
        // the new IR when it arrives, not assumed to survive.
        let same_specimen = self.selected.as_deref() == Some(path.as_path());

        // Mark as compiling — this disables stage tab highlighting and shows a
        // spinner.
        self.compiling = true;
        self.clear_specimen_state(same_specimen);
        // Start with the log view so the user sees compilation progress.
        self.log_entries.clear();
        self.viewing_log = true;
        // Send the compile command to the worker thread. Results will arrive
        // asynchronously via `FromWorker::CompileProgress` and `FromWorker::Compiled`.
        // Recorded before the state changes, so the ring buffer reads in the
        // order the user acted. See `diagnostics.rs`.
        self.context.jump_highlight = None;
        diagnostics::record_action("specimen", path.display().to_string());
        self.worker.send(ToWorker::Compile(path.clone()));
        self.selected = Some(path);
        // Cleared here, not only set in `open_library_model`: a specimen opened
        // after a corpus model would otherwise still look like one, and the
        // source view would refuse to read a file that is right there.
        self.selected_is_library = false;
    }

    /// Open a model from a **loaded library** by qualified name — the corpus
    /// counterpart of [`Self::open`].
    ///
    /// # Why the selection is a `PathBuf` holding a name
    ///
    /// `selected` identifies *what is loaded* and is compared for equality in
    /// three staleness checks, which discard results for a model the user has
    /// already navigated away from. A library model has no file of its own — it
    /// lives inside a package file that may declare many classes, so the file
    /// path would not identify it — and **the worker already established the
    /// convention**: `compile_target` puts the qualified name in the result's
    /// `path` field for a library target. Following it here keeps one identity
    /// scheme and leaves the staleness checks working unchanged.
    ///
    /// `selected_is_library` records which kind it is, rather than sniffing the
    /// string for dots. A model *file* could contain a dot; a flag cannot lie.
    pub(crate) fn open_library_model(&mut self, qualified: &str) {
        let id = PathBuf::from(qualified);
        let same = self.selected.as_deref() == Some(id.as_path());
        self.compiling = true;
        self.clear_specimen_state(same);
        self.log_entries.clear();
        self.viewing_log = true;
        self.context.jump_highlight = None;
        diagnostics::record_action("corpus-model", qualified.to_owned());
        self.worker
            .send(ToWorker::CompileLibraryModel(qualified.to_owned()));
        self.selected = Some(id);
        self.selected_is_library = true;
    }

    /// Clear everything that belonged to the previously loaded specimen.
    ///
    /// Shared by [`Self::open`] and by switching labs, because both need "forget the
    /// last model" and **two copies of a thirty-field reset would drift**. That is not
    /// hypothetical: on 2026-07-30 clearing the jump highlight in one of two capture
    /// paths and not the other produced a real bug within the hour.
    ///
    /// `keep_context` is true when the *same* specimen is being reloaded: a key-path
    /// addresses one model's IR and means nothing in another's, but on a reselect the IR
    /// is normally identical, so the point still resolves. It is validated against the
    /// new IR on arrival rather than assumed to survive.
    fn clear_specimen_state(&mut self, keep_context: bool) {
        // Clear all previous results. Every field that could hold stale data
        // from the last specimen is reset to its default.
        self.model = None;
        self.stages = StageBundle::default();
        self.sim_data = None;
        self.sim_error = None;
        self.sim_running = false;
        self.def_index = BTreeMap::new();
        self.cached_equation_sheet = None;
        self.known_variables = None;
        self.declaring_classes.clear();
        // A point addresses IR that no longer exists once the stages are
        // replaced — but only when the specimen changed. See `same_specimen`.
        if !keep_context {
            self.context.pointed_at = None;
            self.tracked_identifier = None;
        }
        self.context.point_error = None;
        self.context.tracking_summary = None;
        self.identifier_index = None;
        // Whatever the followed name matched belonged to the old IR, so the
        // jump list must be rebuilt rather than reused.
        self.context.jump_matches = Vec::new();
        self.context.jump_key = None;
        self.context.jump_index = 0;
        self.context.jump_target = None;
        self.source.scrolled_for = None;
        self.source.text = None;
        // Cleared with the text, or the previous model's read failure would be
        // reported over the new model's pane — and would also suppress the retry,
        // since it doubles as the retry guard.
        self.source.load_error = None;
        // Cleared *with* the text it labels, or the header would name the previous
        // model's library file over an empty pane for the whole compile. The two
        // are set together in the `Compiled` handler for the same reason.
        self.source.library_uri = None;
        self.source.library_error = None;
        self.source.highlight = None;
        // The wash names a line in *this* file. Carried into another specimen it
        // would mark a line nothing pointed at, which is worse than no mark.
        self.source.jump_line = None;
        self.viewport.highlighted_eq_row = None;
        self.viewport.highlighted_source_line = None;
        self.nav.clear();
        self.nav_loading = None;
        self.nav_error = None;
        // Clean up any in-flight or active live debug session. Without this,
        // switching specimens while arming or running leaves the breakpoint
        // armed on the old specimen and the polling state dangling.
        if self.live_breakpoint_armed {
            let _ = bridge::remove_live_trace_breakpoint();
        }
        self.live_breakpoint_armed = false;
        self.pending_live_debug = None;
        // Clear any stale pending_stage — a plain LoadSpecimen link should not
        // carry over a stage override from a previous LoadAndSwitch click.
        // LoadAndSwitch calls open() first, then sets pending_stage.
        self.pending_stage = None;
        self.pending_sub_view = None;
    }

    /// Ask the bridge to remove the live-trace breakpoint, if HRW believes one is
    /// armed. Returns whether a removal was issued.
    ///
    /// **Separate from [`eframe::App::on_exit`] so it can be tested at all** —
    /// `eframe` owns the call to `on_exit`, so nothing in the test harness can
    /// reach it. Extracting the body is the only way the shutdown path gets a
    /// regression guard, and per `docs/format-and-app-plan.md` an extraction is
    /// justified by exactly that: a test that could not have been written before
    /// it.
    ///
    /// The boolean is what makes the test non-vacuous. Without it, a test could
    /// only assert on a request file, which a *different* code path may also have
    /// written.
    pub(crate) fn release_live_breakpoint_at_exit(&mut self) -> bool {
        if !self.live_breakpoint_armed {
            return false;
        }
        let _ = bridge::remove_live_trace_breakpoint();
        self.live_breakpoint_armed = false;
        true
    }

    /// Fetch a class by qualified name for navigation (async; pushed on arrival).
    /// The worker resolves the name to a class definition and returns it as a
    /// `FromWorker::DefTree` message; `drain_worker` pushes it onto `self.nav`.
    fn navigate_to(&mut self, name: String) {
        self.nav_loading = Some(name.clone());
        self.nav_error = None;
        self.worker.send(ToWorker::OpenDef(name));
    }

    /// Drain all pending messages from the worker thread's channel.
    ///
    /// This is the **channel drain pattern**: the worker sends results via an
    /// `mpsc` channel, and we poll it each frame with `try_recv()` in a loop.
    /// `try_recv` is non-blocking — it returns `Ok(msg)` if a message is
    /// waiting, or `Err` if the channel is empty. The `while` loop processes
    /// ALL pending messages in one frame (the worker may have sent several
    /// between repaints), then falls through so the UI can render.
    ///
    /// Two compilation message types work together:
    /// - **`CompileProgress`**: sent after each pipeline phase completes. It
    ///   carries partial results so the tab colors update incrementally (you
    ///   see Parse go green, then Resolve, etc.) while compilation continues.
    ///   We must NOT touch `compiling` or `stage` here — the pipeline isn't done.
    /// - **`Compiled`**: sent once when the full pipeline finishes. This is
    ///   where we clear `compiling`, set `stage` to the furthest clean result,
    ///   publish all IRs to the bridge, and fit the custom-view cameras.
    ///
    /// Both messages carry the specimen `path` so we can detect stale results:
    /// if the user switched specimens while compilation was running, the old
    /// results arrive for a path that no longer matches `self.selected`, and
    /// we skip them with `continue`.
    fn drain_worker(&mut self) {
        while let Ok(msg) = self.worker.rx.try_recv() {
            match msg {
                FromWorker::Libraries(result) => {
                    self.libraries_busy = false;
                    self.library_status = match result {
                        Ok(n) => format!("loaded {n} library document(s)"),
                        Err(e) => format!("library load failed — {e}"),
                    };
                }
                FromWorker::Log(entry) => {
                    // Mirrored into the crash buffer here rather than snapshotted
                    // per frame: entries arrive one at a time, and cloning the
                    // whole log 60 times a second to carry it would not be
                    // affordable. See `diagnostics.rs`.
                    diagnostics::record_log(entry.level.label(), &entry.message);
                    self.log_entries.push(entry);
                }
                FromWorker::CompileProgress { path, stages } => {
                    // A partial result mid-compile: apply the stages known so far so
                    // the tab colours advance in step with the pipeline. Compilation
                    // is still running, so DON'T touch `compiling`, `stage`,
                    // `def_index`, or the bridge — the final `Compiled` owns those.
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale (specimen switched)
                    }
                    self.stages = stages;
                    // **Each cache tracks its own input, and this one's input just
                    // changed.** `stage_views` holds views built from stage *reports*,
                    // which is exactly what a progress message delivers; `compile_views`
                    // holds replays built from *frames*, which arrive only with
                    // `Compiled`. Dropping the latter here would blank the animations
                    // for the whole compile with nothing to rebuild them from, so the
                    // two are treated differently **because their sources differ**.
                    //
                    // Without this, a recompile drew the PREVIOUS compile's matrix over
                    // the current compile's report: `reset_for` keys on the stage, which
                    // does not change mid-compile, so the cached view survived until
                    // `Compiled`. The data was real and correctly computed — and
                    // attributed to the wrong run, which is the fiction class
                    // `CLAUDE.md` names: *traceable to something Rumoca actually did on
                    // THIS run*. Meanwhile the tab colours advanced with the pipeline, so
                    // the pane and the tabs told different stories about the same instant
                    // — during the one gesture, Recompile, whose whole purpose is to see
                    // what an edit changed.
                    //
                    // No absence is manufactured by rebuilding early: `report_ready`
                    // already refuses to draw a stage whose value has not arrived, so a
                    // stage the pipeline has not reached renders nothing rather than
                    // claiming it holds nothing. Doug ruled it 2026-08-24 on the
                    // principles — accuracy first, and inconsistency costs learning.
                    self.stage_views.invalidate_all();
                }
                FromWorker::Compiled {
                    path,
                    model,
                    stages,
                    def_index,
                    equation_sheet,
                    identifier_index,
                    index_reduction_frames,
                    matching_frames,
                    tarjan_frames,
                    tearing_frames,
                    reduced_frames,
                    dae,
                    pre_lowering_frames,
                    connection_frames,
                    flat,
                    library_source,
                } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result
                    }
                    self.compiling = false;
                    self.model = model;
                    self.stages = stages;
                    self.def_index = def_index;
                    // Ground truth for what the tree may offer to track. Built
                    // from the equation sheet's classification, which lists
                    // every variable in the compiled model — including
                    // library-origin ones, which the identifier index omits
                    // because they have no specimen source line.
                    self.known_variables = equation_sheet
                        .as_ref()
                        .map(|s| s.variables.iter().map(|v| v.name.clone()).collect());
                    self.declaring_classes = Self::build_declaring_classes(
                        &self.stages,
                        &self.def_index,
                        equation_sheet.as_ref(),
                    );
                    self.cached_equation_sheet = equation_sheet;
                    self.identifier_index = identifier_index;
                    self.frames = CompileFrames {
                        index_reduction: index_reduction_frames,
                        structural: crate::worker::StructuralFrames {
                            matching: matching_frames,
                            tarjan: tarjan_frames,
                            tearing: tearing_frames,
                        },
                        reduced: reduced_frames,
                        pre_lowering: pre_lowering_frames,
                        connection: connection_frames,
                    };
                    self.cached_flat = flat;
                    self.cached_dae = dae;
                    // **The two cache families, dropped together.** New frames mean the
                    // replays built from them are stale; new reports mean even the stage
                    // already on screen must be rebuilt — hence `stage_views`' key goes
                    // too. Each is a whole-struct assignment, so a view added to either
                    // family tomorrow is covered here by construction. Until 2026-08-20
                    // the first of these was a hand-written `self.cached_pre_lowering_anim
                    // = None` — a list of one, which is how a list of two goes wrong.
                    self.compile_views.invalidate_all();
                    self.stage_views.invalidate_all();
                    // **A library model's source, seeded into the same cache a
                    // specimen fills from disk.** Everything downstream —
                    // highlighting, clickable identifiers, blamed lines, the
                    // scroll-to-declaration — then works without knowing
                    // where the text came from, which is the point: Doug asked
                    // for the MSL source view to be *as functional as* a
                    // specimen's, not for a second reduced one.
                    if let Some(lib) = library_source {
                        // A read failure is *reported*, never rendered as blank.
                        match lib.text {
                            Ok(text) => {
                                self.source.library_error = None;
                                self.source.text = Some(text);
                            }
                            Err(why) => {
                                self.source.library_error = Some(why);
                                self.source.text = None;
                            }
                        }
                        // Landing at line 1 of a package file shows a header and
                        // nothing asked for; `Resistor` starts 1,498 lines into
                        // `Basic.mo`. The line is the compiler's, not a search.
                        self.source.scroll_target = lib.decl_line;
                        self.source.library_uri = Some(lib.uri);
                        self.source.highlight = None;
                    } else {
                        self.source.library_uri = None;
                        self.source.library_error = None;
                    }
                    if self.live_breakpoint_armed {
                        let _ = bridge::remove_live_trace_breakpoint();
                    }
                    self.live_breakpoint_armed = false;
                    self.pending_live_debug = None;
                    // A point retained across a reselect must still address
                    // something. Validated rather than assumed: the source may
                    // have been edited between loads, and a bar naming a node
                    // that no longer exists — over an emitted `subtree: null` —
                    // is exactly the confident lie this design forbids.
                    self.revalidate_point_against_new_ir();
                    self.viewport.spy.request_fit();
                    self.viewport.incidence.request_fit();
                    self.viewport.matching_anim.request_fit();
                    self.viewport.tarjan_anim.request_fit();
                    self.viewport.before_incidence.request_fit();
                    // Land on the furthest stage that completed cleanly,
                    // unless a pending_stage was requested via an hrw://load/…/Stage link.
                    // Always update `self.stage` so the correct stage is ready when the
                    // user clicks away from Log, but don't force `viewing_log = false` —
                    // if the user is watching the log, let them stay there.
                    if let Some(pending) = self.pending_stage.take() {
                        self.stage = pending;
                        self.viewing_log = false;
                    } else {
                        self.stage = self.last_successful_stage();
                    }
                    // Which source lines the compiler blamed. Once per compile.
                    self.compute_problem_lines();
                    // Publish every stage's full IR so Claude can diff any pair.
                    if let Err(e) = bridge::write_stages(&self.stages.as_stage_pairs()) {
                        self.notify(format!("write_stages failed: {e}"));
                    }
                    // **Close the loop on the load.** Without this the trail ended at
                    // "specimen sent to the worker", so the app block stayed frozen at
                    // `compiling: true, model: null` — accurate for the last *action*,
                    // and increasingly wrong about *now* the longer Doug paused. He
                    // caught exactly that by reloading a lab and asking what the trail
                    // said an hour later.
                    //
                    // Says where the pipeline got to, so a failure is legible without
                    // opening the stage files: the failing stage is the diagnostic.
                    diagnostics::record_action("compiled", self.compile_outcome());
                }
                FromWorker::Simulated { path, result } => {
                    if self.selected.as_deref() != Some(path.as_path()) {
                        continue; // stale result for a different specimen
                    }
                    self.sim_running = false;
                    // Same gap as the compile: without this the trail ends at "started
                    // simulating" and the app block stays frozen mid-run.
                    match result {
                        Ok(data) => {
                            diagnostics::record_action(
                                "simulated",
                                format!(
                                    "{}: {} variables, {} points",
                                    self.model.as_deref().unwrap_or("<unnamed>"),
                                    data.names.len(),
                                    data.times.len(),
                                ),
                            );
                            self.sim_data = Some(data);
                            self.sim_error = None;
                        }
                        Err(e) => {
                            // The failure text itself, because a simulation that will
                            // not run is exactly the case Doug would be asking about.
                            diagnostics::record_action(
                                "simulated",
                                format!(
                                    "{}: FAILED \u{2014} {e}",
                                    self.model.as_deref().unwrap_or("<unnamed>"),
                                ),
                            );
                            self.sim_data = None;
                            self.sim_error = Some(e);
                        }
                    }
                }
                FromWorker::DefTree { name, result } => {
                    // **Only the awaited request clears the indicator.** The three
                    // arms above all refuse to act on a result that is no longer the
                    // one in flight — `CompileProgress`, `Compiled` and `Simulated`
                    // each compare the message's `path` against `selected` — and this
                    // arm was the one that did not.
                    //
                    // Nothing gates navigation while a fetch is running, so clicking a
                    // second class before the first returns is ordinary use. The worker
                    // is FIFO, so the first result then arrived and cleared "opening
                    // B…" while B was still in flight: the pane said nothing was
                    // loading during a load. Found by the 2026-08-23 column read of
                    // this router's six arms.
                    //
                    // The entry is still pushed. Both classes were genuinely asked for
                    // and `nav` is a stack of what was opened, so dropping the earlier
                    // one would lose a navigation the reader made.
                    if self.nav_loading.as_deref() == Some(name.as_str()) {
                        self.nav_loading = None;
                    }
                    match result {
                        Ok((value, def_index)) => {
                            self.nav.push(NavEntry {
                                name,
                                value,
                                def_index,
                            });
                            self.nav_error = None;
                        }
                        Err(e) => self.nav_error = Some(format!("open “{name}” failed: {e}")),
                    }
                }
            }
        }
    }

    /// **The captured frames for the system the current tab is showing.**
    ///
    /// The matching, Tarjan and tearing views render under **two** stages, over two
    /// different DAEs: Structural analyses the raw system, Index Reduction the
    /// reduced one. On `Drivetrain` those differ by 97 equations against 20, so a
    /// frame set from the wrong tab addresses rows the matrix does not have.
    ///
    /// One accessor rather than the choice repeated at three call sites: this is
    /// exactly the decision I got wrong once already by fixing it for tearing and
    /// not carrying the reasoning to the other two.
    fn structural_frames_for_stage(&self) -> &crate::worker::StructuralFrames {
        if self.stage == StageKind::IndexReduction {
            &self.frames.reduced
        } else {
            &self.frames.structural
        }
    }

    /// **Why an algorithm view has nothing to show** — stated, never filled in.
    ///
    /// Doug, 2026-08-04: *"if during matching the compiler discovers that the system
    /// is singular, it would be helpful to know that the compiler returned before
    /// building BLT blocks … it would be helpful if the parts of the UI which depend
    /// upon the BLT blocks made clear that no BLT blocks are available because no
    /// attempt was made by the compiler to create those BLT blocks."*
    ///
    /// He was right, and it reversed an argument I had just made for keeping the
    /// re-deriving fallbacks. Measured on `CapacitorLoop`: the compiler matches 13 of
    /// 14 equations, declares the system singular, and **returns before building any
    /// BLT blocks** — and the Tarjan tab then built its own matching and BLT and drew
    /// a **non-empty SCC decomposition of blocks that were never created.**
    ///
    /// That is a fiction in the same sense as the "DAE pipeline" log entry removed
    /// earlier the same day, and worse: the log was mislabelled, this was fabricated.
    /// **A view with nothing to show that shows something anyway teaches a false
    /// model of the compiler**, and nothing on screen says so.
    ///
    /// The absence is also the more useful thing to know. It teaches the chain's
    /// contract: BLT decomposition and tearing are *entitled* to a matched system, and
    /// a phase that refuses when it has not got one is doing its job.
    fn structural_unavailable(&self, what: &str) -> String {
        let err = self
            .stages
            .get(self.stage)
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
            .and_then(|e| e.get("message"))
            .and_then(serde_json::Value::as_str);
        match err {
            Some(msg) => format!(
                "No {what} to show \u{2014} structural analysis stopped before this \
                 step.\n\n{msg}\n\nMatching runs first. When it cannot match every \
                 equation the system is singular, so BLT decomposition and tearing are \
                 never attempted. Nothing is missing from HRW: the compiler did not \
                 get this far.",
            ),
            None => format!(
                "No {what} was recorded for this model. The compiler produced none, so \
                 there is nothing to replay.",
            ),
        }
    }

    /// Look up the `Stage` for the currently selected tab. Delegates to
    /// `StageBundle::get()` for the ten real stages; Simulation returns the
    /// always-empty placeholder (the Simulation view is the plot pane, rendered
    /// specially — not the generic tree inspector).
    fn current_stage(&self) -> &Stage {
        match self.stage {
            StageKind::Simulation => &self.simulation,
            other => self.stages.get(other),
        }
    }

    /// The previous stage's IR, aligned to the *current* stage's tree root, for
    /// the "changed by this stage" green highlight. `None` for Parse (no
    /// previous). Where structures differ (e.g. Resolve's class tree vs the
    /// Instantiate overlay) few paths align, so little highlights — as intended.
    fn previous_stage_value(&self) -> Option<&serde_json::Value> {
        match self.stage {
            StageKind::Parse => None,
            // Parse's root is the StoredDefinition; the class is under classes.<model>.
            StageKind::Resolve => self
                .stages
                .parse
                .value
                .as_ref()?
                .get("classes")?
                .get(self.model.as_deref()?),
            StageKind::Instantiate => self.stages.resolve.value.as_ref(),
            StageKind::Typecheck => self.stages.instantiate.value.as_ref(),
            StageKind::Flatten => self.stages.typecheck.value.as_ref(),
            // **No diff for DAE.** Its predecessor is the flat model, whose
            // shape shares almost no paths with the DAE's partitioned form —
            // the highlight would light up everything and mean nothing. The
            // interesting comparison is not path-wise; it is the partition
            // itself, which is what the stage exists to show.
            StageKind::Dae => None,
            // The structural report is a different shape from the flat model —
            // no path-aligned previous, so nothing to highlight.
            StageKind::Structural => None,
            // Diff the reduced report against the raw one: for an already-index-1
            // model they're identical (nothing highlights); for a reduced
            // high-index model the raw report is absent (it was singular).
            StageKind::IndexReduction => self.stages.structural.value.as_ref(),
            // The IC plan is its own shape (a solve sequence) — no path-aligned prior.
            StageKind::Initialization => None,
            // The event partitions are their own shape — no path-aligned prior.
            StageKind::Events => None,
            // The SolveModel is a different shape from the DAE — no path-aligned prior.
            StageKind::SolveLowering => None,
            // Simulation is a plot, not IR — no tree, no prior.
            StageKind::Simulation => None,
        }
    }

    /// The furthest-along stage that produced clean IR (value present, no error
    /// note) — where the tabs should land after a compile. Falls back to Parse.
    fn last_successful_stage(&self) -> StageKind {
        let ok = |s: &Stage| s.value.is_some() && !s.note_is_error();
        if ok(&self.stages.solve_lowering) {
            StageKind::SolveLowering
        } else if ok(&self.stages.events) {
            StageKind::Events
        } else if ok(&self.stages.initialization) {
            StageKind::Initialization
        } else if ok(&self.stages.index_reduction) {
            StageKind::IndexReduction
        } else if ok(&self.stages.structural) {
            StageKind::Structural
        } else if ok(&self.stages.flatten) {
            StageKind::Flatten
        } else if ok(&self.stages.typecheck) {
            StageKind::Typecheck
        } else if ok(&self.stages.instantiate) {
            StageKind::Instantiate
        } else if ok(&self.stages.resolve) {
            StageKind::Resolve
        } else {
            StageKind::Parse
        }
    }

    fn library_strings(&self) -> Vec<String> {
        self.parse_library_paths()
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    }

    /// Emit a bridge focus file describing what the user wants to ask about, and
    /// record a one-line status. The reasoning happens in the Claude Code chat.
    ///
    /// **What "capture" means:** when the user clicks a tree node, a stage tab,
    /// or a specimen, HRW writes a JSON file (the "focus file") to a known
    /// location on disk. Claude Code (running in a terminal alongside the GUI)
    /// watches that location. The focus file contains: what was clicked (a
    /// key-path like `["equations", 3, "lhs"]`), which stage, the full IR of
    /// the current and adjacent stages, and the def_index for resolving
    /// cross-references. Claude reads this to understand the user's question
    /// context without the user having to copy-paste IR.
    ///
    /// Build an `Ask` from the current specimen state (shared fields).
    fn base_ask<'a>(
        &'a self,
        seq: u64,
        request: bridge::AskRequest,
        focus: Focus<'a>,
        stage_values: &'a [(&'a str, Option<&'a Value>)],
    ) -> Ask<'a> {
        Ask {
            seq,
            request,
            specimen: self.selected.as_deref(),
            model: self.model.as_deref(),
            stage: Some(self.stage),
            libraries: self.library_strings(),
            def_index: &self.def_index,
            parse_value: self.stages.parse.value.as_ref(),
            resolve_value: self.stages.resolve.value.as_ref(),
            focus,
            tracking: self.tracking_context(stage_values),
            view: self.view_context(),
            failure: self.failure_context(),
        }
    }

    /// The first pipeline stage that failed, for the capture (ideas #45 step 3).
    ///
    /// **First, not current.** A failure cascades — every later stage reports "not
    /// reached" — so the earliest error is the cause and the rest are consequences.
    /// Naming whichever stage Doug happens to be looking at would often name a
    /// consequence, which is exactly the wrong thing to hand a question like "why
    /// doesn't this work?".
    ///
    /// Returns `None` for a clean compile, so the section is absent rather than
    /// present-and-empty: a `pipeline_failure` key that exists always would make
    /// "nothing failed" indistinguishable from "the field was not populated".
    fn failure_context(&self) -> Option<bridge::PipelineFailure<'_>> {
        // `COMPILATION`, not `ALL`: `ALL` ends with `Simulation`, which is a tab rather
        // than a compilation stage, and `StageBundle::get()` panics on it.
        let first = StageKind::COMPILATION
            .iter()
            .copied()
            .find(|&k| self.stages.get(k).note_is_error())?;
        let stage = self.stages.get(first);
        let after = StageKind::COMPILATION
            .iter()
            .copied()
            .skip_while(|&k| k != first)
            .skip(1)
            .filter(|&k| self.stages.get(k).value.is_none())
            .map(StageKind::name)
            .collect();
        Some(bridge::PipelineFailure {
            stage: first.name(),
            note: stage.note.as_deref().unwrap_or(""),
            error: stage.value.as_ref().and_then(|v| v.get("error")),
            not_reached: after,
        })
    }

    /// The ambient half of the emitted context — what is being followed.
    ///
    /// Always included when something is tracked, whatever the user pointed at.
    /// Following is not a mode you enter to ask a question; it is standing
    /// context that any question is asked *within*. See
    /// `docs/context-assembly.md`.
    fn tracking_context<'a>(
        &'a self,
        stage_values: &'a [(&'a str, Option<&'a Value>)],
    ) -> Option<bridge::Tracking<'a>> {
        let name = self.tracked_identifier.as_deref()?;
        Some(bridge::Tracking {
            seq: self.context.track_seq,
            name,
            declared_line: self
                .identifier_index
                .as_ref()
                .and_then(|idx| idx.variables.get(name))
                .map(|v| v.source_line),
            declaring_class: self.declaring_classes.get(name).map(String::as_str),
            stage_values,
        })
    }

    /// Put a split measurement in the log view.
    ///
    /// **The log, not a `dbg!`.** Doug is the only one who can see the real
    /// window, so the number has to reach *him* — three theories about the
    /// opening width have been wrong, each a guess about a figure nobody had
    /// looked at.
    fn log_split(&mut self, message: String) {
        self.log_entries.push(crate::worker::LogEntry {
            elapsed_secs: 0.0,
            level: crate::worker::LogLevel::Info,
            message,
            depth: 0,
        });
    }

    /// Emit a stage or specimen capture, and record it as the point.
    ///
    /// Recording matters as much as emitting: this path used to write the file
    /// without updating `pointed_at`, so a stage-tab click replaced the emitted
    /// context while the Context Bar carried on showing the previous node.
    fn emit_focus(&mut self, focus: Focus) {
        // `Nothing` is not a capture. It is what the Context Bar emits *after*
        // the point is cleared, which goes through `emit_context`. Guarded
        // rather than asserted: a UI path must not abort the app to report a
        // programming error.
        if matches!(focus, Focus::Nothing) {
            return;
        }
        let seq = self.context.next_seq();
        // A point of Doug's own supersedes the link's — the highlight answers "which
        // row did that link mean?", and he has just answered a different question.
        self.context.jump_highlight = None;
        diagnostics::record_action("point-at", format!("in {}", self.stage.name()));
        // Name what was captured, so the status confirms the *right* thing was
        // written — not just that some focus was.
        let target = match &focus {
            Focus::Node { key_path, .. } => bridge::describe_path(key_path),
            Focus::Stage => format!("stage “{}”", self.stage.name()),
            Focus::Specimen => format!(
                "specimen “{}”",
                self.selected
                    .as_deref()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("?"),
            ),
            // Excluded by the guard above. `return` rather than a panic keeps
            // that safe even if a future caller forgets.
            Focus::Nothing => return,
            // **Lab passages do not come through here.** They are captured by
            // `capture_lab_passage`, which has the lab name and the selected text
            // and needs none of the stage machinery this function is built on.
            Focus::LabPassage { .. } => return,
        };
        let stage_values = self.stages.as_stage_pairs();
        let kind = match &focus {
            Focus::Node { key_path, .. } => PointKind::Node(key_path.clone()),
            Focus::Stage => PointKind::Stage,
            Focus::Specimen => PointKind::Specimen,
            Focus::Nothing => return,
            Focus::LabPassage { .. } => return,
        };
        let ask = self.base_ask(seq, bridge::AskRequest::Explain, focus, &stage_values);
        let result = bridge::write(&ask);
        self.context.pointed_at = Some(PointedAt {
            seq,
            target: target.clone(),
            kind,
            stage: Some(self.stage),
            request: bridge::AskRequest::Explain,
        });
        self.context.point_error = result.as_ref().err().map(std::string::ToString::to_string);
        // `None` on success: the Context Bar already names the point, and it
        // does so persistently. See `status_line`.
        self.notice = status_line(seq, &target, "explain", result);
    }

    /// What HRW is showing, for the capture.
    ///
    /// One builder for all four `Ask` construction sites, so the emitted view
    /// can never depend on which path produced the capture — a point made by
    /// clicking a node and the same point re-emitted when the followed
    /// identifier changes must describe the same screen.
    ///
    /// Overlaps `diagnostic_snapshot` deliberately rather than sharing with it.
    /// The two answer different questions — *what was on screen when this
    /// context was assembled* versus *what was the app doing when it died* —
    /// and merging them would tie the capture's shape to the crash log's.
    fn view_context(&self) -> bridge::View<'_> {
        bridge::View {
            ui_mode: self.ui_mode.label(),
            stage_view: match self.stage {
                StageKind::Structural | StageKind::IndexReduction => {
                    Some(structural_view_name(self.viewport.structural))
                }
                StageKind::Flatten => Some(flatten_view_name(self.viewport.flatten)),
                StageKind::Events => Some(events_view_name(self.viewport.events)),
                StageKind::Initialization => Some(init_view_name(self.viewport.init)),
                _ => None,
            },
            specimen_detail: (self.ui_mode == UiMode::Specimen).then_some({
                match self.specimen_detail {
                    SpecimenDetail::Source => "Source",
                    SpecimenDetail::Purpose => "Purpose",
                }
            }),
            viewing_log: self.viewing_log,
            animation: self.animation_view(),
        }
    }

    /// The animation the current stage tab is showing, if any.
    ///
    /// One place that answers "which animation is on screen?" — the capture and
    /// the crash log both used to carry their own copy of this match, so a new
    /// view (idea #40's `pre()` lowering) would have needed adding in three
    /// places and would have been forgotten in one.
    ///
    /// Reported only for the *current* tab: the caches hold several animations
    /// at once, and naming a stale one would say the user was looking at
    /// something they were not.
    fn on_screen_animation(&self) -> Option<&dyn Animated> {
        match self.stage {
            StageKind::Structural => match self.viewport.structural {
                StructuralView::MatchingAnim => {
                    Some(self.stage_views.matching_anim.as_ref()?.as_ref()?)
                }
                StructuralView::TarjanAnim => {
                    Some(self.stage_views.tarjan_anim.as_ref()?.as_ref()?)
                }
                StructuralView::TearingAnim => {
                    Some(self.stage_views.tearing_anim.as_ref()?.as_ref()?)
                }
                StructuralView::AliasAnim => Some(self.stage_views.alias_anim.as_ref()?.as_ref()?),
                _ => None,
            },
            StageKind::IndexReduction => {
                Some(self.compile_views.reduction_anim.as_ref()?.as_ref()?)
            }
            StageKind::Events if self.viewport.events == EventsView::PreLowering => {
                Some(self.compile_views.pre_lowering_anim.as_ref()?.as_ref()?)
            }
            StageKind::Initialization if self.viewport.init == InitView::IcPlan => {
                Some(self.compile_views.ic_plan_anim.as_ref()?.as_ref()?)
            }
            StageKind::Flatten if self.viewport.flatten == FlattenView::Connections => {
                Some(self.compile_views.connection_anim.as_ref()?.as_ref()?)
            }
            _ => None,
        }
    }

    /// One line saying how far the pipeline got, for the action trail.
    ///
    /// Reports the **first failing stage** when there is one, because that is the
    /// diagnostic — everything after it is "not reached" and says nothing. On success
    /// it reports the furthest stage reached, which is how a partial pipeline (a model
    /// that compiles but will not lower) is told apart from a complete one.
    fn compile_outcome(&self) -> String {
        let model = self.model.as_deref().unwrap_or("<unnamed>");
        for kind in StageKind::COMPILATION {
            let stage = self.stages.get(*kind);
            if stage.note_is_error() {
                return format!("{model}: FAILED at {}", kind.name());
            }
        }
        format!("{model}: reached {}", self.last_successful_stage().name())
    }

    /// Whether a structural sub-view has a tab right now.
    ///
    /// **One predicate, used by the tab bar and by the link guard**, so a link cannot
    /// select a view that has no tab. Doug hit exactly that: the cross-platform lab
    /// linked to `Structural/Summary`, which exists only when a model is *singular* —
    /// `ProportionalLoop` is not, so the link selected a view with no tab and the panel
    /// rendered the singular summary for a non-singular model.
    ///
    /// Availability depends on the *model*, not just the stage, which is why
    /// `SubView::from_slug` cannot catch it: that validates a slug against a stage, and
    /// this is a question about what the compile produced.
    fn structural_view_available(&self, v: StructuralView) -> bool {
        let is_index_reduction = self.stage == StageKind::IndexReduction;
        let is_singular = Self::note_says_singular(self.stages.get(self.stage).note.as_deref());
        match v {
            // The only two whose availability depends on what the compile
            // *captured* rather than on which stage it is and whether the system
            // was singular. Everything else defers to the pure predicate below,
            // so a checker can reach it without running a compile.
            StructuralView::Animate => {
                is_index_reduction && !self.frames.index_reduction.is_empty()
            }
            StructuralView::AliasAnim => is_index_reduction && self.has_alias_eliminations(),
            other => Self::structural_view_available_from_stage(
                other,
                is_index_reduction,
                is_singular,
            )
            .expect("Animate and AliasAnim are the only frame-dependent views, both handled above"),
        }
    }

    /// Whether a stage note reports a structurally singular system.
    ///
    /// One function, because the *labs* are now checked against the same rule the
    /// app applies, and the note's wording varies: `RcCircuit` carries `null`,
    /// `Drivetrain` carries `"singular"`, and `BenchActuator` carries a sentence
    /// beginning *"structural analysis failed: structurally singular system: 47
    /// matched out of 48…"*. A substring test is right here for the same reason it
    /// is wrong for identity (`docs/identity-and-provenance.md`): this asks a
    /// *question about prose Rumoca wrote*, not which thing a name refers to.
    fn note_says_singular(note: Option<&str>) -> bool {
        note.is_some_and(|n| n.contains("singular"))
    }

    /// Sub-view availability decided by **stage and singularity alone** — `None` for
    /// the two views that additionally depend on captured frames.
    ///
    /// **Extracted 2026-08-12 so a lab link can be checked without a compile.**
    /// Doug, walking `connect-expansion.md`: *"Act 2 … contains a link for RcCircuit
    /// → Structural → Summary, and that link actually navigates to RcCircuit →
    /// Structural → Incidence."* The link parsed, so
    /// `fixture_lab_links_all_resolve` passed it; `Summary` is simply **not
    /// available on the Structural stage of a non-singular model**, so the app
    /// refused it, said so in the status bar, and left the sub-view where it was.
    /// Six such links existed across three labs and one walk found one of them.
    ///
    /// `every_lab_sub_view_link_is_available_for_its_specimen` calls **this**
    /// function against each specimen's committed manifest note, so the check cannot
    /// drift from the behaviour — the reimplementation hazard
    /// `docs/fidelity-plan.md` warns about.
    fn structural_view_available_from_stage(
        v: StructuralView,
        is_index_reduction: bool,
        is_singular: bool,
    ) -> Option<bool> {
        match v {
            // Summary is the singular-system explanation, plus Index Reduction's report.
            StructuralView::Summary => Some(is_index_reduction || is_singular),
            // These need a complete matching to mean anything.
            StructuralView::SpyPlot | StructuralView::TarjanAnim | StructuralView::TearingAnim => {
                Some(!is_singular || is_index_reduction)
            }
            // Always available: the incidence pattern, the matching *search* (whose
            // failure is the point on a singular system), and the raw tree.
            StructuralView::Incidence | StructuralView::MatchingAnim | StructuralView::Tree => {
                Some(true)
            }
            StructuralView::Animate | StructuralView::AliasAnim => None,
        }
    }

    /// Force the structural sub-view back to something this stage actually offers.
    ///
    /// # The class of bug this closes, rather than the instance
    ///
    /// Three doors set `viewport.structural`, and each has its own guard: the tab row only
    /// draws tabs [`Self::structural_view_available`] approved, the `hrw://` link guard in
    /// [`Self::apply_pending_view_and_seek`] refuses a view the model has no tab for, and
    /// the stage-change default in [`crate::report_sub_view`] redirects the
    /// Index-Reduction-only views. **Nothing checked the result**, so a door added without
    /// its guard — which is exactly what happened when the Aliases tab was added and the
    /// stage-change default was not updated with it (2026-08-19) — produced a selected
    /// view with no tab, and the panel below rendered it against the wrong stage's report.
    ///
    /// The symptom is *silent and plausible*: the alias view read the Structural report,
    /// found no eliminations there, and said *"(no alias eliminations in this report)"*
    /// about `RcCircuit`, which has several. **Absence filled rather than stated.**
    ///
    /// # Why it notifies rather than clamping quietly
    ///
    /// After the 2026-08-19 fix there is no known path here, so reaching it means a guard
    /// somewhere upstream has gone missing — the thing a silent correction would hide. The
    /// notice puts it in `session.json`'s action trail, where a bug report can start from
    /// it. **Tree is the fallback because it is the one view available on every report
    /// stage**, singular or not.
    fn clamp_structural_sub_view(&mut self) {
        if !matches!(
            self.stage,
            StageKind::Structural | StageKind::IndexReduction
        ) {
            return;
        }
        let v = self.viewport.structural;
        if !self.structural_view_available(v) {
            // Named *before* the clamp, or the notice reports the fallback rather than
            // the view that was stranded — which is the only useful part of it.
            let stranded = structural_view_name(v);
            self.viewport.structural = StructuralView::Tree;
            self.notify(format!(
                "{} has no {stranded} view for this model \u{2014} showing the tree \
                 instead. This is an HRW bug; please report it.",
                self.stage.name(),
            ));
        }
    }

    /// Show a notice **and record it**.
    ///
    /// Every notice is something HRW is telling Doug went wrong — an unresolvable node
    /// path, a frame that does not exist, a specimen not found. Those are exactly the
    /// events Claude needs when Doug reports "there seem to be several bugs", and until
    /// 2026-07-30 they existed only on screen for a few seconds.
    ///
    /// One function so the shown text and the recorded text cannot drift apart.
    fn notify(&mut self, message: impl Into<String>) {
        let message = message.into();
        diagnostics::record_action("notice", message.clone());
        self.notice = Some(message);
    }

    /// Act on a followed `hrw://` link.
    ///
    /// Extracted from `ui` 2026-07-29 so the ordering rules below can be tested — they
    /// are subtle, and one of them was already wrong.
    ///
    /// **Every sub-view request goes through `pending_sub_view`, never
    /// `apply_sub_view`.** The centre panel resets the sub-view whenever a report stage
    /// is entered (forcing `Summary` for Index Reduction, `SpyPlot` for a non-singular
    /// Structural), and that reset runs *after* this. Applying a sub-view here would be
    /// silently overwritten, and the symptom is the one Doug hit: the link appears to do
    /// nothing the first time and works the second, because the second click no longer
    /// changes the stage and so skips the reset.
    ///
    /// `LoadAndSwitch` already did this correctly and its comment said why; the three
    /// sibling verbs did not, which is the whole bug.
    fn dispatch_hrw_link(&mut self, action: HrwLink) {
        // **Record every followed link.** Doug clicking a lab stop is the single most
        // informative thing that happens in a session, and it was invisible: when he
        // reported bugs in a fixture lab, `session.json` showed the specimen load and
        // nothing after. Now the trail names each stop in order, so a bug report can
        // start from what was actually clicked rather than from a reconstruction.
        //
        // Deliberately **not** written to `focus.json`. That file is the noun Doug
        // *assembles*; overwriting it on every click would destroy what he is pointing
        // at and break the composition primitives. This is a different question —
        // "what did I do", not "what should you look at".
        diagnostics::record_action("lab-link", action.describe());

        // A link that needs a specimen, with none loaded, is refused rather than
        // half-applied. Setting `pending_stage` here and returning would be worse than
        // doing nothing: it would linger and fire when a specimen arrived later, sending
        // the reader somewhere no link had pointed — the same trap the frame-seek budget
        // exists to close.
        if action.requires_specimen() && self.selected.is_none() {
            self.notify(
                "no specimen loaded \u{2014} this stop needs one. Start at the lab's first \
                 stop, which loads it.",
            );
            return;
        }
        match action {
            HrwLink::OpenLab { lab, stop } => {
                // **One lab citing another**, so a composed answer can send Doug to
                // an existing demonstration rather than retelling it. `docs/ideas.md`
                // #63.
                let found = bridge::fixture_labs()
                    .into_iter()
                    .find(|p| p.file_stem().and_then(|s| s.to_str()) == Some(lab.as_str()));
                let Some(path) = found else {
                    // **Named, not silent.** A link that does nothing is the worst
                    // outcome in a lab, because nothing on screen says why — the
                    // reason the parser caps `splitn` at 5 rather than 4.
                    self.notice = Some(format!("no fixture lab named `{lab}`"));
                    return;
                };
                self.select_lab(LabSource::Fixture(path));
                self.lab.polled_at = None;
                self.poll_lab_file();
                if let Some(slug) = stop {
                    match self.lab.text().map(|t| {
                        crate::autoplay::parse_stations(t)
                            .into_iter()
                            .find(|s| crate::autoplay::station_slug(&s.heading) == slug)
                    }) {
                        Some(Some(found_station)) => {
                            self.lab.scroll_to_offset = Some(found_station.heading_offset);
                        }
                        // The lab opened but the stop is gone — say which, because
                        // "it opened at the top" is indistinguishable from a lab
                        // whose first stop is the one that was asked for.
                        _ => {
                            self.notice = Some(format!(
                                "`{lab}` has no stop `{slug}` \u{2014} opened at the top"
                            ));
                        }
                    }
                }
            }
            HrwLink::LoadSpecimen(name) => {
                // **One verb, three sources** — deliberately not a second verb.
                //
                // The corpus list shows curated specimens, scratch probes and the
                // 2,626 MSL models in one widget (`docs/ideas.md` #52), so from a
                // lab's point of view they are all just models. A separate
                // `hrw://model/` verb would split one gesture in two and need
                // merging later, which is the mistake Test mode was.
                //
                // Files first: a curated specimen and a library model could in
                // principle share a name, and the repo's own copy should win.
                if let Some(path) = self.find_specimen(&name) {
                    // Same rule as `LoadAndSwitch` above and as the left panel:
                    // reselecting what is already loaded reveals it, and only
                    // "Recompile" recompiles.
                    if self.selected.as_ref() == Some(&path) {
                        self.viewing_log = false;
                    } else {
                        self.open(path);
                    }
                } else if name.contains('.') {
                    // A qualified name — ask the library. The worker reports
                    // clearly if no such model is loaded, so nothing is guessed
                    // at here.
                    self.open_library_model(&name);
                } else {
                    self.notify(format!(
                        "not found: {name} - no specimen by that name, and it is not a \
                         qualified model name (those contain dots)"
                    ));
                }
            }
            HrwLink::ShowSource(line) => {
                // The source lives in Specimen mode's Source detail, so getting there
                // means setting both — a lab should not have to tell Doug which mode
                // to be in.
                self.ui_mode = UiMode::Specimen;
                self.split.request_reset(MODE_SWITCH_RESET);
                self.specimen_detail = SpecimenDetail::Source;
                self.viewing_log = false;
                self.source.scroll_target = line;
                // Scrolling says "somewhere here"; the wash says "this one".
                self.source.jump_line = line;
            }
            HrwLink::SwitchStage(kind, sub) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = sub;
            }
            HrwLink::LoadAndSwitch(name, kind, sub) => {
                if let Some(path) = self.find_specimen(&name) {
                    // **Already loaded means switch, not recompile** — the rule the
                    // left panel's `ModelListNav::Select` arm has always followed, and
                    // which this arm did not. Doug, 2026-08-22: every `▶ Look` link in
                    // a stop recompiled the specimen the previous link had just
                    // compiled, so walking one lab paid a full compile per stop.
                    //
                    // **Nothing is lost by skipping it.** The workflow `open`'s comment
                    // protects — assemble context, arm a breakpoint, recompile to hit
                    // it — belongs to `ModelListNav::Reload` ("Recompile" in the
                    // context menu), which always compiles and is unaffected.
                    //
                    // **Deliberately no mtime check**, though an edited specimen would
                    // now show its previous compile. The left panel has never checked
                    // either, and a link path stricter than the panel would put the
                    // same inconsistency in a new place. If staleness needs closing it
                    // should close for both, in one change.
                    if self.selected.as_ref() == Some(&path) {
                        self.viewing_log = false;
                        self.stage = kind;
                        // No compile is coming, so nothing would ever apply a deferred
                        // stage; clear it rather than leave one to fire on the next.
                        self.pending_stage = None;
                        self.pending_sub_view = sub;
                    } else {
                        self.open(path);
                        self.pending_stage = Some(kind);
                        self.pending_sub_view = sub;
                    }
                } else {
                    self.notify(format!("specimen not found: {name}"));
                }
            }
            HrwLink::AimAtEquation(kind, sub, equation) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = Some(sub);
                // Deferred: turning an equation index into a world position needs the
                // view's own layout, which exists only at paint time.
                self.aim_at_equation = Some(equation);
                // **And the same verb points at a row of the incidence matrix.**
                //
                // `equation` used to reach only the canvas views, where it aims a
                // camera. The Incidence view has rows *named for equations* and no way
                // to link to one, so a lab could say "open the matrix" and then had to
                // describe the row in prose — which on a 97-row model is the difference
                // between pointing and gesturing.
                //
                // Doug, 2026-08-18, on a lab that hand-copied a five-row table the
                // pane already draws: *"HRW is your platform. Use it."*
                //
                // **One verb, not two.** A separate `row` would put two words on one
                // job — the thing the lab template's own rule 2 forbids — when "point
                // at equation N" is what both views are being asked for. Each does it
                // in its own idiom: the canvas moves a camera, the matrix marks a row.
                self.viewport.highlighted_eq_row = Some(equation);
            }
            HrwLink::PointAtNode(kind, sub, path) => {
                self.stage = kind;
                self.viewing_log = false;
                // `None` leaves whatever sub-view the stage is already showing, which
                // for a tree-only stage is its only one.
                self.pending_sub_view = sub;
                // `jump_target` already forces ancestors open and scrolls, and is
                // consumed on the frame it is honoured — the same one-shot discipline
                // as the camera aim and the frame seek.
                self.context.jump_target = Some(path.clone());
                self.context.jump_highlight = Some(path);
            }
            HrwLink::OpenNotebook(name) => match bridge::resolve_notebook(&name) {
                Some(path) => {
                    if let Err(e) = open_with_os(&path) {
                        self.notify(format!("could not open {name}: {e}"));
                    }
                }
                None => self.notify(format!(
                    "no notebook {name} \u{2014} the link names one that is not here",
                )),
            },
            HrwLink::OpenDoc(name) => match bridge::resolve_doc(&name) {
                Some(path) => {
                    // **`code`, not the OS association**, which on Doug's machine is
                    // Chrome. See `HrwLink::OpenDoc`. Failure is reported rather than
                    // falling back to the browser: a silent fallback would reproduce
                    // exactly the behaviour he asked to be rid of.
                    if let Err(e) = open_in_vscode(&path, None) {
                        self.notify(format!(
                            "could not open {name} in VS Code ({e}) \u{2014} run \
                             `cargo run -p hrw --example check_machine`, which rules on \
                             whether the CLI this spawns is reachable here",
                        ));
                    }
                }
                None => self.notify(format!(
                    "no document {name} \u{2014} the link names one that is not under hrw/docs/",
                )),
            },
            HrwLink::OpenSource(target) => match bridge::resolve_source(&target) {
                Some(found) => {
                    if let Err(e) = open_in_vscode(&found.path, found.line) {
                        self.notify(format!(
                            "could not open {target} in VS Code ({e}) \u{2014} run \
                             `cargo run -p hrw --example check_machine`, which rules on \
                             whether the CLI this spawns is reachable here",
                        ));
                    }
                }
                // **Two failures, one message, and it says which.** A path outside the
                // workspace and a symbol that no longer exists are different findings,
                // and the second is the one that will actually happen — code moves.
                None => self.notify(format!(
                    "cannot resolve {target} \u{2014} either the path is not a file under \
                     the workspace, or the `#symbol` after it is not defined in that file",
                )),
            },
            HrwLink::OpenInSystemModeler(name) => match self.find_specimen(&name) {
                Some(path) => {
                    if let Err(e) = open_with_os(&path) {
                        self.notify(format!("could not open {name} in System Modeler: {e}"));
                    }
                }
                None => self.notify(format!("specimen not found: {name}")),
            },
            HrwLink::ArmBreakpoint(name) => {
                // **Refused while the Debug handshake is in flight.** Both use the
                // single `breakpoint-request.json` / `breakpoint-ack.json` pair,
                // so arming here mid-handshake would overwrite the request the
                // Debug button is waiting on and hand it someone else's ack —
                // `#71`'s failure in a new dress.
                if self.pending_live_debug.is_some() {
                    self.notify(
                        "a Debug session is still arming \u{2014} wait for it to stop, \
                         then click this again",
                    );
                } else if let Some((file, line, what)) =
                    crate::matching_ledger::anchor_by_name(&name)
                {
                    match bridge::arm_source_breakpoint(file, line) {
                        // Deliberately not "armed": the ack is not read here, and
                        // `#75` is the entry about claiming a breakpoint exists on
                        // evidence that does not say so. The red dot in VS Code's
                        // gutter is the confirmation, and it is already on screen.
                        Ok(()) => self.notify(format!(
                            "asked the bridge for a breakpoint at {file}:{line} \u{2014} {what}",
                        )),
                        Err(e) => self.notify(format!("could not arm {file}:{line}: {e}")),
                    }
                } else {
                    // Unreachable via a parsed link — `parse_hrw_link` rejects an
                    // unknown anchor — but stated rather than silently ignored.
                    self.notify(format!("no breakpoint anchor named {name}"));
                }
            }
            HrwLink::Follow(name) => {
                // Following is independent of what is pointed at: a stop may set one,
                // the other, or both. So this deliberately does not touch the stage.
                self.set_tracked_identifier(name);
            }
            HrwLink::SeekFrame(kind, sub, frame) => {
                self.stage = kind;
                self.viewing_log = false;
                self.pending_sub_view = Some(sub);
                // Deferred for the same reason, plus one more: the animation for this
                // sub-view may not be built until it paints.
                self.seek_frame = Some((frame, SEEK_ATTEMPTS));
            }
        }
    }

    /// The on-screen animation, mutably — the seeking twin of
    /// [`Self::on_screen_animation`].
    ///
    /// Two functions rather than one generic over mutability because the immutable one
    /// serves the **capture**, and a capture must never move what it describes. Keeping
    /// them separate makes that a type-level fact rather than a convention.
    ///
    /// The duplicated match is deliberate and small; if a third caller appears, the
    /// sub-view-to-animation mapping should become a table instead.
    fn on_screen_animation_mut(&mut self) -> Option<&mut dyn Animated> {
        match self.stage {
            StageKind::Structural => match self.viewport.structural {
                StructuralView::MatchingAnim => {
                    Some(self.stage_views.matching_anim.as_mut()?.as_mut()?)
                }
                StructuralView::TarjanAnim => {
                    Some(self.stage_views.tarjan_anim.as_mut()?.as_mut()?)
                }
                StructuralView::TearingAnim => {
                    Some(self.stage_views.tearing_anim.as_mut()?.as_mut()?)
                }
                StructuralView::AliasAnim => Some(self.stage_views.alias_anim.as_mut()?.as_mut()?),
                _ => None,
            },
            StageKind::IndexReduction => {
                Some(self.compile_views.reduction_anim.as_mut()?.as_mut()?)
            }
            StageKind::Events if self.viewport.events == EventsView::PreLowering => {
                Some(self.compile_views.pre_lowering_anim.as_mut()?.as_mut()?)
            }
            StageKind::Initialization if self.viewport.init == InitView::IcPlan => {
                Some(self.compile_views.ic_plan_anim.as_mut()?.as_mut()?)
            }
            StageKind::Flatten if self.viewport.flatten == FlattenView::Connections => {
                Some(self.compile_views.connection_anim.as_mut()?.as_mut()?)
            }
            _ => None,
        }
    }

    /// Apply a pending `hrw://…/frame/<n>` seek, if the on-screen animation can take it.
    ///
    /// Called from the paint path *after* the animation views have had a chance to
    /// build themselves, which is why it is deferred rather than applied at link
    /// dispatch.
    fn apply_pending_seek(&mut self) {
        let Some((target, attempts)) = self.seek_frame else {
            return;
        };
        // Probe first: `on_screen_animation_mut` borrows `self`, so the pending flag
        // has to be settled before the borrow starts.
        if self.on_screen_animation().is_none() {
            // Not built yet — or this view never has one. Spend an attempt rather than
            // waiting forever; see `seek_frame` on why an unbounded retry is a bug.
            // `saturating_sub` then test for zero, so exactly `SEEK_ATTEMPTS` paints
            // are spent. `checked_sub` needed one *extra* call to clear, because it
            // only returns `None` when already at zero — an off-by-one my own expiry
            // test caught.
            let left = attempts.saturating_sub(1);
            self.seek_frame = (left > 0).then_some((target, left));
            return;
        }
        self.seek_frame = None;
        let ok = self
            .on_screen_animation_mut()
            .is_some_and(|a| a.seek(target));
        if !ok {
            let (_, total) = self
                .on_screen_animation()
                .map_or((0, 0), Animated::position);
            // Report the number Doug typed, not the internal cursor: the link is
            // 1-based to match the on-screen counter, so quoting `target` raw would be
            // off by one — the very bug this change fixes.
            self.notify(format!(
                "no frame {} in this replay \u{2014} it has {total}",
                target + 1,
            ));
        }
    }

    fn animation_view(&self) -> Option<bridge::AnimationView<'static>> {
        let anim = self.on_screen_animation()?;
        let (frame, frame_count) = anim.position();
        Some(bridge::AnimationView {
            which: anim.which(),
            frame,
            frame_count,
            live_state: anim.live_state(false).name(),
            // What the user is looking at, not merely where they are.
            frame_context: anim.current_frame_context(),
        })
    }

    /// Publish the pane on screen to `.hrw-bridge/view.json`, when it changes.
    ///
    /// # What this closes
    ///
    /// The diagnostic snapshot already carried `stage_tab`, so Claude knew Doug was on
    /// Flatten — but not **which Flatten sub-tab**, and not the pane's contents. That
    /// single gap is what had him about to type out an equation sheet by hand
    /// (2026-08-13). A transcription is friction *and* a place for an error neither of
    /// us would catch.
    ///
    /// # Only on change, and only the current view
    ///
    /// Writing every view on every compile would add to the bridge's existing 1.5 MB
    /// per compile for no benefit, since almost all of it is never read. Comparing
    /// against [`Viewport::last_published_view`] means the file is written when the
    /// reader moves and not otherwise.
    ///
    /// # Views without a publisher state their absence
    ///
    /// A pane with no `to_bridge_json` **removes** the file. Leaving the previous view's
    /// content would be a stale report indistinguishable from a current one — and
    /// `view.json` naming a pane the reader has left is worse than no file at all.
    /// Adding a view here is: give its data type a `to_bridge_json`, then add an arm.
    fn publish_current_view(&mut self) {
        let sub = sub_view_name_for(self.stage, &self.viewport);
        let key = match sub {
            Some(name) => format!("{}/{name}", self.stage.slug()),
            None => self.stage.slug().to_owned(),
        };
        if self.viewport.last_published_view.as_deref() == Some(key.as_str()) {
            return;
        }

        // One arm per publishable view. The body is the renderer's own input, never a
        // description of it — see `EquationSheet::to_bridge_json`.
        let body = match self.stage {
            StageKind::Flatten if self.viewport.flatten == FlattenView::Equations => self
                .cached_equation_sheet
                .as_ref()
                .map(equation_sheet::EquationSheet::to_bridge_json),
            // The only pane that shows connection sets, and therefore the only
            // evidence for `connect-expansion.md` Station 1.
            StageKind::Flatten if self.viewport.flatten == FlattenView::Connections => self
                .compile_views
                .connection_anim
                .as_ref()
                .and_then(Option::as_ref)
                .map(connection_anim::ConnectionAnimation::to_bridge_json),
            // The painter-drawn view no accessibility tree can reach. Both report
            // stages share it; `stage_views` already holds whichever was built.
            StageKind::Structural | StageKind::IndexReduction
                if self.viewport.structural == StructuralView::Incidence =>
            {
                self.stage_views
                    .incidence
                    .as_ref()
                    .and_then(Option::as_ref)
                    .map(incidence_view::IncidenceMatrix::to_bridge_json)
            }
            _ => None,
        };

        let kind = body.as_ref().map(|_| key.as_str());
        if let Err(e) = bridge::write_view(kind, body.as_ref()) {
            // Reported, not swallowed: a bridge write that fails silently is a file
            // Claude would read as "this pane is empty".
            self.context.point_error = Some(format!("view.json: {e}"));
        }
        diagnostics::record_action("view", key.clone());
        self.viewport.last_published_view = Some(key);
    }

    /// The application state a crash file should carry.
    ///
    /// Built every frame and handed to [`crate::diagnostics::set_snapshot`], so
    /// the panic hook — which cannot borrow `App` — still has it. Every field
    /// here is one I had to reconstruct from Doug's description of what he
    /// clicked while diagnosing the 2026-07-28 crash; the list is that session's
    /// findings turned into code rather than a guess at what might be useful.
    ///
    /// Deliberately *not* an interpretation. It reports what the state is, not
    /// what it means — the same rule the bridge follows (`DECISIONS.md`,
    /// 2026-07-28). `stages` lists which IRs exist rather than judging the
    /// compile good or bad, because "Flatten produced a note but no value" is a
    /// fact and "compilation partly failed" is a conclusion.
    fn diagnostic_snapshot(&self) -> Value {
        let anim = self.animation_diagnostic();
        json!({
            "specimen": self.selected.as_ref().map(|p| p.display().to_string()),
            "model": self.model,
            "ui_mode": format!("{:?}", self.ui_mode),
            // **Which lab is open**, so a question about "this stop" can be answered.
            //
            // Doug, 2026-08-19: *"I'd like to enjoy the convenience of deixis when asking
            // questions about statements which you've made in labs. Currently, it seems
            // that I have to copy / paste those lab statements."* The capture said
            // `ui_mode: "Lab"` and nothing more, so **which document he was reading was
            // unrecoverable** and pasting was the only way to ask about it.
            //
            // The name, not the text: the labs are on disk, so naming the document is
            // enough to read the exact wording from it. Publishing the prose would
            // duplicate a file that is already the source of truth, and a duplicate can
            // disagree with it.
            //
            // **Deliberately not the scroll position.** Publishing which *stop* is on
            // screen was considered and recommended against the same day (`CLAUDE.md`):
            // it answers a question Doug did not ask — he points at statements, not stops
            // — and costs a per-heading render on a pane in constant use.
            "lab": self.lab.selected.as_ref().map(LabSource::label),
            "specimen_detail": format!("{:?}", self.specimen_detail),
            "stage_tab": self.stage.name(),
            // **Which sub-tab of that stage**, which `stage_tab` alone does not say.
            // `null` means the stage has only one view, not that the field was omitted.
            "sub_view": sub_view_name_for(self.stage, &self.viewport),
            // Whether `.hrw-bridge/view.json` currently describes this pane. A reader
            // comparing the two can tell a stale file from a current one.
            "view_published": self.viewport.last_published_view,
            "viewing_log": self.viewing_log,
            "compiling": self.compiling,
            // Non-empty means the view is showing a library class, not the
            // specimen — which changes what every other field refers to.
            "navigation": self.nav.iter().map(|n| n.name.clone()).collect::<Vec<_>>(),
            "context": {
                "seq": self.context.context_seq,
                "pointing_at": self.context.pointed_at.as_ref().map(|p| json!({
                    "seq": p.seq,
                    "target": p.target,
                    "kind": match &p.kind {
                        PointKind::Node(path) => format!("node {}", bridge::describe_path(path)),
                        PointKind::Stage => "stage".to_owned(),
                        PointKind::Specimen => "specimen".to_owned(),
                        // Named with its lab, because "lab passage" alone would
                        // leave the hook reporting a quotation with no document.
                        PointKind::LabPassage { lab } => format!("lab passage in {lab}"),
                    },
                    // Null for a lab passage, which is not in a stage. Stated rather
                    // than filled with whichever tab happened to be selected.
                    "stage": p.stage.map(StageKind::name),
                    "request": format!("{:?}", p.request),
                })),
                "following": self.tracked_identifier.as_ref().map(|name| json!({
                    "identifier": name,
                    "seq": self.context.track_seq,
                    "mentions": self.context.tracking_summary.map(|(m, _)| m),
                    "stages_with_mentions": self.context.tracking_summary.map(|(_, s)| s),
                })),
                "last_emission_error": self.context.point_error,
                "status_line": self.notice,
            },
            "animation": anim,
            "live_trace": {
                "breakpoint_armed": self.live_breakpoint_armed,
                "awaiting_ack": self.pending_live_debug.is_some(),
            },
            "simulation": {
                "running": self.sim_running,
                "error": self.sim_error,
                "has_data": self.sim_data.is_some(),
                "t_end": self.sim_t_end,
            },
            "stages": self.stages.as_stage_pairs().iter()
                .map(|(name, value)| json!({ "stage": name, "has_ir": value.is_some() }))
                .collect::<Vec<_>>(),
            "counts": {
                "log_entries": self.log_entries.len(),
                "specimen_files": self.model_list.files.len(),
                "resolved_def_ids": self.def_index.len(),
                "known_variables": self.known_variables.as_ref().map(HashSet::len),
            },
        })
    }

    /// Which animation is on screen and where its cursor stands.
    ///
    /// Reported only for the animation belonging to the *current* stage tab:
    /// the caches can hold several at once, and listing a stale one would
    /// suggest the user was looking at something they were not.
    fn animation_diagnostic(&self) -> Value {
        let Some(anim) = self.on_screen_animation() else {
            return Value::Null;
        };
        let (frame, frame_count) = anim.position();
        json!({
            "which": anim.which(),
            "frame": frame,
            "frame_count": frame_count,
            "live_state": anim.live_state(false).name(),
            // A crash mid-animation is one of the harder ones to reproduce, so
            // the file carries what was being shown, not just the index.
            "showing": anim.current_frame_context(),
        })
    }

    /// Capture the node the user acted on — scoped to the navigated class when
    /// navigating, else to the current specimen stage (with cross-stage diff).
    /// `request` is "explain" (Ask Claude) or "debug-where-set" (the debugger).
    ///
    /// The two code paths handle different contexts:
    /// - **Navigated class** (nav stack non-empty): we're viewing a library
    ///   class reached via "Go to definition". No Parse stage exists here
    ///   (it's not a specimen), so no cross-stage diff is possible.
    /// - **Specimen stage** (nav stack empty): we're viewing the specimen's own
    ///   IR. The capture includes the Parse and Resolve values so Claude can
    ///   diff across stages (e.g. "what did Typecheck change vs Instantiate?").
    fn emit_node_focus(&mut self, key_path: Vec<Seg>, request: bridge::AskRequest) {
        // Same rule as `emit_focus`: a point of Doug's own supersedes the link's mark.
        // There are **two** capture paths — this one for nodes, `emit_focus` for stage
        // and specimen — and clearing in only one is exactly the omission the test for
        // this caught.
        self.context.jump_highlight = None;
        let seq = self.context.next_seq();
        let target = bridge::describe_path(&key_path);
        let request_str = request.as_str();

        let status = if let Some(entry) = self.nav.last() {
            let ask = Ask {
                seq,
                request,
                specimen: None,
                model: Some(&entry.name),
                stage: None,
                libraries: self.library_strings(),
                def_index: &entry.def_index,
                parse_value: None,
                resolve_value: None,
                focus: Focus::Node {
                    key_path,
                    stage_value: &entry.value,
                },
                // A navigated library class is outside the specimen pipeline,
                // so there are no stages to sweep for the followed identifier.
                tracking: None,
                view: self.view_context(),
                failure: self.failure_context(),
            };
            status_line(seq, &target, request_str, bridge::write(&ask))
        } else {
            let stage_value = self.current_stage().value.clone();
            match &stage_value {
                Some(value) => {
                    let focus = Focus::Node {
                        key_path: key_path.clone(),
                        stage_value: value,
                    };
                    let stage_values = self.stages.as_stage_pairs();
                    let ask = self.base_ask(seq, request, focus, &stage_values);
                    let result = bridge::write(&ask);
                    // Retained so the Context Bar can state it, and so a later
                    // change of what is followed re-emits without losing it.
                    self.context.pointed_at = Some(PointedAt {
                        seq,
                        target: target.clone(),
                        kind: PointKind::Node(key_path),
                        stage: Some(self.stage),
                        request,
                    });
                    self.context.point_error =
                        result.as_ref().err().map(std::string::ToString::to_string);
                    status_line(seq, &target, request_str, result)
                }
                None => Some("(no IR for this stage to point at)".to_owned()),
            }
        };
        self.notice = status;
    }

    fn start_simulation(&mut self) {
        if let (Some(path), Some(model)) = (self.selected.clone(), self.model.clone()) {
            self.sim_running = true;
            self.sim_data = None;
            self.sim_error = None;
            // **`selected_is_library` decides how `path` is read.** For a library
            // model it holds the qualified name rather than a file, which is why
            // pressing Run on an MSL model reported a missing file until 2026-08-04.
            self.worker.send(ToWorker::Simulate {
                path,
                model,
                t_end: self.sim_t_end,
                is_library: self.selected_is_library,
            });
        }
    }

    /// The Simulation view — a stop-time slider and an `egui_plot` pane of the state
    /// trajectories. **It starts nothing.** Running the model is on-demand rather than
    /// a compile stage, and the one control that starts it is the tab row's ▶
    /// (`stage_tabs.rs`), which dispatches `ToWorker::Simulate` through
    /// `App::start_simulation`; the plot appears when `FromWorker::Simulated` lands
    /// (see `drain_worker`).
    fn simulation_pane(&mut self, ui: &mut egui::Ui) {
        use egui_plot::{Corner, Legend, Line, Plot, PlotPoints};

        // **This pane had its own ▶ Run until 2026-08-30, and now has none.** Doug:
        // *"We only need one Run button for simulations."* Two were on screen together
        // whenever this tab was open — this one and the tab row's ▶ — and they did not
        // even agree on when a run was possible: the row's uses `can_sim` (not
        // compiling, not running, a model parsed, solve lowering done), while this one
        // had decayed to `!sim_running` alone, so it stayed live on a specimen that
        // could not be simulated. One button, one answer.
        ui.horizontal(|ui| {
            ui.add(
                egui::Slider::new(&mut self.sim_t_end, 0.1..=20.0)
                    .step_by(0.1)
                    .text("stop time"),
            );
            // **The spinner is the tab row's; the words are this pane's** (Doug,
            // 2026-08-30, the same ruling as the two Run buttons). Both drew on
            // `sim_running`, so a run put two spinners on screen whenever this tab was
            // open — verified before removing it, since `ui.spinner()` carries no
            // accessibility label and the claim had been a code read.
            //
            // The sentence stays: the tab row's spinner is a bare painted widget with
            // no words, so dropping this too would leave the view silent about what it
            // is doing. `a_running_simulation_is_announced_in_the_pane` pins it.
            if self.sim_running {
                ui.weak("simulating…");
            }
        });
        if let Some(e) = &self.sim_error {
            egui::ScrollArea::horizontal()
                .id_salt("sim_err")
                .show(ui, |ui| {
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        egui::RichText::new(e).monospace(),
                    );
                });
        }
        ui.separator();
        match &self.sim_data {
            Some(data) => {
                // **A trajectory HRW cannot faithfully draw is reported, not quietly
                // rendered** (2026-08-25). A simulation can succeed and still contain
                // an infinity: the solver's finiteness guards watch states and the
                // projection path, and an algebraic output that goes singular mid-run
                // is neither. Measured with a three-equation model, and until this
                // line existed the pane drew it in silence.
                let non_finite = data.non_finite_series();
                if !non_finite.is_empty() {
                    let listed: Vec<String> = non_finite
                        .iter()
                        .map(|(name, n)| format!("{name} ({n})"))
                        .collect();
                    ui.colored_label(
                        ui.visuals().error_fg_color,
                        format!(
                            "\u{26a0} non-finite values in: {} \u{2014} the solver returned \
                             these, so the curve below is not the whole trajectory",
                            listed.join(", "),
                        ),
                    );
                }
                let has_diagnostics = !data.solver_steps.is_empty();
                let link_group = ui.id().with("sim_time_axis");

                let mut trajectory_plot = Plot::new("sim_plot")
                    .legend(Legend::default().position(Corner::LeftTop))
                    .x_axis_label("time");
                if has_diagnostics {
                    trajectory_plot = trajectory_plot
                        .link_axis(link_group, [true, false])
                        .link_cursor(link_group, [true, false])
                        .height(ui.available_height() * TRAJECTORY_PLOT_HEIGHT_FRACTION);
                }
                let tracked = self.tracked_identifier.as_deref();
                trajectory_plot.show(ui, |plot_ui| {
                    for (i, (name, series)) in data.names.iter().zip(&data.data).enumerate() {
                        let segments = if data.has_discontinuities {
                            discontinuity_segments(series)
                        } else {
                            std::iter::once(0..series.len()).collect()
                        };
                        let is_tracked = tracked == Some(name.as_str());
                        let color = if is_tracked {
                            crate::colors::TRACKED_GOLD
                        } else {
                            series_color(i)
                        };
                        let width = if is_tracked { 3.0 } else { 1.0 };
                        for seg in segments {
                            let pts: PlotPoints = data.times[seg.clone()]
                                .iter()
                                .zip(&series[seg])
                                .map(|(&t, &y)| [t, y])
                                .collect();
                            plot_ui.line(Line::new(name.clone(), pts).color(color).width(width));
                        }
                    }
                });

                if has_diagnostics {
                    ui.separator();
                    ui.strong("Solver diagnostics");

                    Plot::new("solver_diagnostics")
                        .legend(Legend::default().position(Corner::LeftTop))
                        .link_axis(link_group, [true, false])
                        .link_cursor(link_group, [true, false])
                        .x_axis_label("time")
                        .y_axis_label("step size h  /  BDF order k")
                        .show(ui, |plot_ui| {
                            let h_pts: PlotPoints =
                                data.solver_steps.iter().map(|s| [s.t, s.h]).collect();
                            plot_ui.line(
                                Line::new("step size h", h_pts)
                                    .color(crate::colors::SOLVER_STEP_SIZE),
                            );

                            let order_pts: PlotPoints = data
                                .solver_steps
                                .iter()
                                .map(|s| [s.t, s.order as f64])
                                .collect();
                            plot_ui.line(
                                Line::new("BDF order k", order_pts)
                                    .color(crate::colors::SOLVER_BDF_ORDER),
                            );
                        });
                }
            }
            None if !self.sim_running => {
                ui.weak(
                    "Press ▶ beside the Simulation tab to simulate this specimen and \
                     plot its state trajectories.",
                );
            }
            None => {}
        }
    }
}

/// A notice for the status bar after a bridge write — **only when something
/// needs saying**.
///
/// ## Why success is silent now
///
/// The status bar used to confirm every capture ("captured equations.3.lhs —
/// now ask me about it in the chat"). The Context Bar makes that redundant, and
/// worse than redundant: the bar states the point *persistently and by name*,
/// while the status line stated it once and then went stale, so the two could
/// disagree about what Claude had. Two places claiming to describe the same
/// thing is exactly the failure this design keeps hitting — a transient
/// confirmation is the weaker of the two, so it goes.
///
/// A failure still returns text, because a failure is precisely the case the
/// Context Bar cannot show on its own: it renders the point either way, and
/// silence would leave it describing context that was never written. (The bar
/// *also* flags it via `point_error`; this is the second, transient channel for
/// the moment it happens.)
///
/// `debug-where-set` still speaks on success, because it asks the user to do
/// something next — say "debug" in the chat — and an instruction is not a
/// confirmation.
fn status_line(
    seq: u64,
    target: &str,
    request: &str,
    result: std::io::Result<std::path::PathBuf>,
) -> Option<String> {
    match result {
        Err(e) => Some(format!("\u{26a0} not emitted \u{2014} {e}")),
        Ok(_) if request == "debug-where-set" => Some(format!(
            "🐞 pointing at  {target}  for the debugger \u{2014} say \u{201c}debug\u{201d} \
             in chat and I'll set the breakpoint  (context #{seq})"
        )),
        Ok(_) => None,
    }
}

// The `eframe::App` trait is what eframe calls each frame. `ui()` is the
// single entry point: it receives a `&mut egui::Ui` and must build the
// entire window's contents from scratch. This is the heart of the
// immediate-mode pattern — no persistent widget tree, just code that runs
// every frame.
impl App {
    fn floating_windows(&mut self, ui: &mut egui::Ui) {
        egui::Window::new("Using HRW")
            .open(&mut self.show_help)
            .collapsible(false)
            .default_width(440.0)
            .show(ui.ctx(), |ui| {
                ui.strong("Inspect");
                ui.label("Pick a specimen (left), choose Parse/Resolve, and expand the IR tree.");
                ui.add_space(6.0);
                ui.strong("Assemble context");
                ui.label(
                    "Two ways to build the subject of a question, and the Context Bar shows \
                     both. Point at one node \u{2014} left-click it, or right-click for more \
                     actions. Follow one identifier \u{2014} right-click a variable name and \
                     choose Follow, and HRW reports where it appears in every stage, and \
                     where it does not.",
                );
                ui.add_space(2.0);
                ui.label(
                    "Then ask in the Claude Code chat \u{2014} Claude reads what you assembled. \
                     Shortcut: just type \u{201c}explain\u{201d}, with no need to phrase a question.",
                );
                ui.add_space(6.0);
                ui.strong("Diff stages");
                ui.label(
                    "Every point publishes all stages\u{2019} full IR, so Claude can compare any \
                     two on request. Point at anything, then ask in the chat — e.g. \u{201c}what did \
                     Typecheck change vs Instantiate?\u{201d} (the resolved type_ids) or \u{201c}diff Parse and \
                     Resolve here\u{201d} (def_ids filled in) — and Claude reads the two stages and reports \
                     the differences. (A node captured on Parse/Resolve also carries its own \
                     before/after inline, so \u{201c}explain\u{201d} alone shows what Resolve changed.)",
                );
                ui.add_space(6.0);
                ui.strong("Structural (spy-plot)");
                ui.label(
                    "On the Structural stage, the BLT block structure is drawn as a spy-plot: \
                     diagonal blocks in evaluation order — scalar solves are single cells, coupled \
                     algebraic loops are boxes. Drag to pan, scroll to zoom, hover a block to see its \
                     equations/unknowns/tearing, and click it to capture it for \u{201c}explain\u{201d}. Toggle to \
                     Tree for the raw report.",
                );
                ui.add_space(6.0);
                ui.strong("Navigate");
                ui.label(
                    "Some fields hold a DefId that resolves to a class — the tree shows it inline \
                     (e.g. \u{201c}type_def_id: 27579 → model …\u{201d}). Right-click that field and choose \
                     \u{201c}Go to …\u{201d} to open that class\u{2019}s own IR. Use Back to step up one level, or \
                     Specimen to return to the top.",
                );
                ui.add_space(6.0);
                ui.strong("Debugger");
                ui.label(
                    "Right-click a field and choose 🐞 \u{201c}Show this being set\u{201d}, then tell Claude \
                     \u{201c}debug\u{201d} in the chat. Claude sets a breakpoint on the running debug session; \
                     right-click the specimen and choose Recompile to hit it.",
                );
            });
        egui::Window::new("About HRW Observatory")
            .open(&mut self.show_about)
            .collapsible(false)
            .resizable(false)
            .show(ui.ctx(), |ui| {
                ui.strong("HRW Observatory");
                ui.label("An egui instrument for studying the Rumoca Modelica compiler pipeline.");
                ui.separator();
                ui.label(format!("HRW v{}", env!("CARGO_PKG_VERSION")));
                ui.label(format!(
                    "Built against Rumoca {} · git {}",
                    env!("HRW_RUMOCA_VERSION"),
                    env!("HRW_RUMOCA_REV"),
                ));
                ui.label("Rumoca is linked as a library; compilation runs on a worker thread.");
            });
        // Settings uses the "deferred action" pattern: the `.open()` borrow
        // prevents calling `self.load_libraries()` inside the closure, so we
        // collect intent and act after the closure.
        let mut load_libraries = false;
        let mut rescan_specimens = false;
        egui::Window::new("Settings")
            .open(&mut self.show_settings)
            .collapsible(false)
            .default_width(560.0)
            .show(ui.ctx(), |ui| {
                ui.strong("Display");
                let mut zoom = ui.ctx().zoom_factor();
                if ui
                    .add(
                        egui::Slider::new(&mut zoom, 0.75..=3.0)
                            .step_by(0.05)
                            .text("Font / UI scale"),
                    )
                    .changed()
                {
                    ui.ctx().set_zoom_factor(zoom);
                }
                ui.separator();
                ui.strong("Specimen directory");
                ui.horizontal(|ui| {
                    let changed = ui
                        .add(
                            egui::TextEdit::singleline(&mut self.model_list.dir)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace),
                        )
                        .changed();
                    if ui.button("⟳").on_hover_text("Rescan directory").clicked() || changed {
                        rescan_specimens = true;
                    }
                });
                ui.separator();
                ui.strong("Library source roots");
                ui.label("One package directory (or single .mo) per line:");
                ui.add(
                    egui::TextEdit::multiline(&mut self.libraries_text)
                        .desired_rows(4)
                        .desired_width(f32::INFINITY)
                        .font(egui::TextStyle::Monospace),
                );
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(!self.libraries_busy, egui::Button::new("Load libraries"))
                        .clicked()
                    {
                        load_libraries = true;
                    }
                    if self.libraries_busy {
                        ui.spinner();
                    }
                    ui.weak(&self.library_status);
                });
            });
        if load_libraries {
            self.load_libraries();
        }
        if rescan_specimens {
            self.model_list.rescan();
        }
    }

    fn equation_sheet_ui(&mut self, ui: &mut egui::Ui) {
        let has_incidence = self
            .stage_views
            .incidence
            .as_ref()
            .is_some_and(|c| c.is_some())
            || self.stages.get(StageKind::Structural).value.is_some();

        let click = crate::equation_sheet_view::equation_sheet_ui(
            ui,
            self.cached_equation_sheet.as_ref(),
            has_incidence,
            self.tracked_identifier.as_deref(),
            self.viewport.highlighted_eq_row,
        );

        match click {
            Some(SheetClick::Equation(new_val)) => {
                self.viewport.highlighted_eq_row = new_val;
                if new_val.is_some() {
                    self.stage = StageKind::Structural;
                    self.viewport.structural = StructuralView::Incidence;
                }
            }
            Some(SheetClick::Variable(name)) => {
                self.set_tracked_identifier(name);
                // The whole point of reverse tracking is *seeing* the declaration,
                // and the specimen source view only renders in Specimen mode — in
                // Lab or Debug mode the click sets tracking and appears to do
                // nothing. So reveal the source, the same way clicking an equation
                // already navigates to the incidence matrix.
                if self.tracked_identifier.is_some() {
                    self.ui_mode = UiMode::Specimen;
                    self.split.request_reset(MODE_SWITCH_RESET);
                    self.specimen_detail = SpecimenDetail::Source;
                }
            }
            None => {}
        }
    }

    /// For each variable, the class that declares it — when that is not the
    /// specimen.
    ///
    /// A flattened name like `src.V` has no declaration in the specimen source:
    /// `src` is a component of the model, and `V` is a parameter of *its* type,
    /// `Modelica.Electrical.Analog.Sources.ConstantVoltage`. Without this, "where
    /// did this come from?" had no answer for such names — most of a real model,
    /// since most components are library instances.
    ///
    /// ## What this resolves, and what it does not
    ///
    /// Only the **first** path segment is resolved, against the model's own
    /// components in the Resolve IR. For `src.V` that is exact: `V` really is
    /// declared in `src`'s type. For a deeper name like `gear.flange_a.tau` it
    /// yields `gear`'s type, which *contains* the declaration rather than being
    /// it — resolving further would mean walking into library class IRs that are
    /// only loaded on demand. The UI therefore says "component `src` is a …"
    /// rather than "`V` is declared in …", which is true in both cases.
    fn build_declaring_classes(
        stages: &StageBundle,
        def_index: &BTreeMap<u64, DefInfo>,
        sheet: Option<&equation_sheet::EquationSheet>,
    ) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let (Some(resolve), Some(sheet)) = (stages.get(StageKind::Resolve).value.as_ref(), sheet)
        else {
            return out;
        };
        let Some(components) = resolve.get("components").and_then(|c| c.as_object()) else {
            return out;
        };
        for var in &sheet.variables {
            let Some((head, _)) = var.name.split_once('.') else {
                continue;
            };
            let class = components
                .get(head)
                .and_then(|c| c.get("type_def_id"))
                .and_then(serde_json::Value::as_u64)
                .and_then(|id| def_index.get(&id))
                .filter(|info| matches!(info.kind, DefKind::Class))
                .map(|info| info.name.clone());
            if let Some(class) = class {
                out.insert(var.name.clone(), class);
            }
        }
        out
    }

    /// The Matching animation view (Structural Analysis tab).
    ///
    /// Extracted from `ui` during the 2026-07-28 sweep. It and the five animated
    /// views below it — Tarjan, Reduction, Tearing, Connections, `pre()` lowering
    /// — once opened with the same eighteen-line live-debug prologue written out
    /// by hand six times. That prologue is now [`Self::live_debug_gate`], and the
    /// [`playback::Animated`] trait it is generic over is the "trait over the
    /// animation types" this comment used to say was still owed.
    ///
    /// **What differs between the six is what is left here**, and the differences
    /// are real rather than accidental: this view starts its live session from
    /// the incidence matrix, aims a camera at it, and has a split "before/after"
    /// header the scrolling views do not.
    fn matching_anim_ui(&mut self, ui: &mut egui::Ui, ir_split: bool) {
        if self.stage_views.incidence.is_none() {
            self.stage_views.incidence = Some(
                self.stages
                    .get(self.stage)
                    .value
                    .as_ref()
                    .and_then(incidence_view::IncidenceMatrix::from_report),
            );
        }
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::Matching, |a| {
            &a.stage_views.matching_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let Some(Some(mat)) = &self.stage_views.incidence
        {
            // The delay is chosen from whether a breakpoint was actually acked:
            // a stepped session needs egui to finish painting inside the sleep,
            // a free-running one must not crawl. See `crate::live_frame_delay`.
            let delay = crate::live_frame_delay(self.live_breakpoint_armed);
            let live = matching_anim::MatchingAnimation::start_live(mat, delay);
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.stage_views.matching_anim = Some(live);
            self.viewport.matching_anim.request_fit();
        }
        if self.stage_views.matching_anim.is_none() {
            let inc = self.stage_views.incidence.as_ref().unwrap();
            self.stage_views.matching_anim = Some(
                // Frames from the compile, and from the SYSTEM this tab is showing:
                // Structural animates the raw DAE, Index Reduction the reduced one.
                inc.as_ref().and_then(|m| {
                    matching_anim::MatchingAnimation::from_captured_frames(
                        m,
                        &self.structural_frames_for_stage().matching,
                    )
                }),
            );
        }
        if let Some(Some(anim)) = &mut self.stage_views.matching_anim {
            if ir_split {
                ui.label(
                    egui::RichText::new("Before (raw DAE)")
                        .strong()
                        .color(crate::colors::ANIM_FAIL),
                );
                ui.weak("Matching animation unavailable (structurally singular \u{2014} only a partial matching exists)");
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("After (reduced)")
                        .strong()
                        .color(crate::colors::ANIM_PATH_FOUND),
                );
            }
            debug_clicked = anim.ui(
                ui,
                &mut self.viewport.matching_anim,
                self.tracked_identifier.as_deref(),
                gate.arming,
                gate.debug_enabled,
            );
        } else {
            // Was "(no incidence data)" — which is often false and always unhelpful.
            // The real reason is usually that the compiler stopped earlier.
            ui.label(self.structural_unavailable("matching search"));
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Matching);
        }
    }

    /// The BLT / Tarjan animation view. See [`Self::matching_anim_ui`].
    fn tarjan_anim_ui(&mut self, ui: &mut egui::Ui, ir_split: bool) {
        if self.stage_views.incidence.is_none() {
            self.stage_views.incidence = Some(
                self.stages
                    .get(self.stage)
                    .value
                    .as_ref()
                    .and_then(incidence_view::IncidenceMatrix::from_report),
            );
        }
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::Tarjan, |a| {
            &a.stage_views.tarjan_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let Some(Some(mat)) = &self.stage_views.incidence
        {
            let delay = crate::live_frame_delay(self.live_breakpoint_armed);
            let live = tarjan_anim::TarjanAnimation::start_live(mat, delay);
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.stage_views.tarjan_anim = Some(live);
            self.viewport.tarjan_anim.request_fit();
        }
        if self.stage_views.tarjan_anim.is_none() {
            let inc = self.stage_views.incidence.as_ref().unwrap();
            self.stage_views.tarjan_anim = Some(
                // Both searches from the compile, and from the system this tab shows.
                inc.as_ref().and_then(|m| {
                    let f = self.structural_frames_for_stage();
                    tarjan_anim::TarjanAnimation::from_captured_frames(m, &f.matching, &f.tarjan)
                }),
            );
        }
        // Consume a pending camera aim from `hrw://…/equation/<n>`. Taken here rather than
        // at link-dispatch time because turning an equation index into a world position
        // needs this view's own layout, which exists only at paint time.
        if let Some(target) = self.aim_at_equation
            && let Some(Some(anim)) = &self.stage_views.tarjan_anim
        {
            self.aim_at_equation = None;
            if !anim.aim_at_equation(&mut self.viewport.tarjan_anim, target) {
                self.notify(format!(
                "no equation {target} in this model \u{2014} the link names one that is not here",
            ));
            }
        }
        if let Some(Some(anim)) = &mut self.stage_views.tarjan_anim {
            if ir_split {
                ui.label(
                    egui::RichText::new("Before (raw DAE)")
                        .strong()
                        .color(crate::colors::ANIM_FAIL),
                );
                ui.weak("BLT animation unavailable (structurally singular \u{2014} no full matching for block decomposition)");
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("After (reduced)")
                        .strong()
                        .color(crate::colors::ANIM_PATH_FOUND),
                );
            }
            debug_clicked = anim.ui(
                ui,
                &mut self.viewport.tarjan_anim,
                self.tracked_identifier.as_deref(),
                gate.arming,
                gate.debug_enabled,
            );
        } else {
            // The dependency graph exists whenever matching succeeded; when this pane
            // is empty it is nearly always because the compiler never built BLT blocks.
            ui.label(self.structural_unavailable("BLT block decomposition"));
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Tarjan);
        }
    }

    /// The index-reduction animation view. See [`Self::matching_anim_ui`].
    fn reduction_anim_ui(&mut self, ui: &mut egui::Ui) {
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::Reduction, |a| {
            &a.compile_views.reduction_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let Some(dae) = &self.cached_dae
        {
            let delay = crate::live_frame_delay(self.live_breakpoint_armed);
            let live = reduction_anim::ReductionAnimation::start_live(dae.clone(), delay);
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.compile_views.reduction_anim = Some(live);
        }
        if self.compile_views.reduction_anim.is_none() {
            let frames = &self.frames.index_reduction;
            self.compile_views.reduction_anim = Some(if frames.is_empty() {
                None
            } else {
                Some(reduction_anim::ReductionAnimation::from_frames(
                    frames.clone(),
                ))
            });
        }
        if let Some(Some(anim)) = &mut self.compile_views.reduction_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, gate.arming, gate.debug_enabled))
                .inner;
        } else {
            ui.weak("(no index-reduction trace for this model)");
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Reduction);
        }
    }

    /// The tearing replay, on the Structural and Index Reduction stages.
    ///
    /// Unlike the other animated views this one is not built from the stage's
    /// JSON report: tearing works in each coupled block's own 0..n index space,
    /// and the report has already translated back to names. So the view walks
    /// the DAE again (`tearing_anim::walk_blocks`) and re-runs the algorithm
    /// with an observer attached.
    ///
    /// Which DAE depends on the tab. The Structural tab tears the raw DAE; the
    /// Index Reduction tab tears the *reduced* one, because that is the system
    /// its report describes -- and for a high-index model the raw DAE has no
    /// full matching, hence no blocks, hence nothing to tear.
    fn tearing_anim_ui(&mut self, ui: &mut egui::Ui) {
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::Tearing, |a| {
            &a.stage_views.tearing_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let Some(dae) = self.tearing_dae()
        {
            let delay = crate::live_frame_delay(self.live_breakpoint_armed);
            let live = tearing_anim::TearingAnimation::start_live(dae, delay);
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.stage_views.tearing_anim = Some(live);
        }
        if self.stage_views.tearing_anim.is_none() {
            // **Captured first, re-derived only as a fallback.** `from_captured`
            // returns `None` when the capture is absent or disagrees with the
            // report, and `record` then re-runs the walk — a faithful picture beats
            // an empty one, and refusing to *guess an alignment* is what the
            // `None` is for.
            // **Only under Structural.** The tearing view also renders on the
            // Index Reduction tab, where `tearing_dae` tears the *reduced* DAE that
            // HRW builds itself — the captured frames came from
            // `build_structural_report` on the raw one, so using them there would
            // animate a different system than the tab is showing.
            // Both the report and the frames must come from the same system: the
            // Structural tab tears the raw DAE, Index Reduction the reduced one.
            // Pairing one tab's report with the other's frames is the mismatch this
            // whole set of captures exists to make impossible.
            // **No `record` fallback.** Re-walking the DAE here would tear blocks the
            // compiler never built — see `structural_unavailable`.
            self.stage_views.tearing_anim = Some(
                self.stages
                    .get(self.stage)
                    .value
                    .as_ref()
                    .and_then(|report| {
                        tearing_anim::TearingAnimation::from_captured(
                            report,
                            &self.structural_frames_for_stage().tearing,
                        )
                    }),
            );
        }
        if let Some(Some(anim)) = &mut self.stage_views.tearing_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, gate.arming, gate.debug_enabled))
                .inner;
        } else {
            ui.label(self.structural_unavailable("tearing"));
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Tearing);
        }
    }

    /// The DAE the current stage tab should tear -- see [`Self::tearing_anim_ui`].
    ///
    /// Index reduction is re-run here rather than carried from the worker
    /// because it is a pure function of the DAE, and re-running it keeps the
    /// two tabs from having to agree about a second cached artifact.
    fn tearing_dae(&self) -> Option<rumoca_ir_dae::Dae> {
        let dae = self.cached_dae.as_ref()?;
        if self.stage == StageKind::IndexReduction {
            let mut reduced = dae.clone();
            crate::worker::index_reduce_in_place(&mut reduced);
            Some(reduced)
        } else {
            Some(dae.clone())
        }
    }

    /// The connection-expansion replay, with live debug driven by the **worker**.
    ///
    /// Follows the same six-step lifecycle as every other animated view, with one
    /// difference at step 3: instead of spawning an algorithm thread here, it hands
    /// the channel's producer to the worker and lets the real compile drive it. See
    /// [`PendingLiveDebug::Connections`].
    fn connection_anim_ui(&mut self, ui: &mut egui::Ui) {
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::Connections, |a| {
            &a.compile_views.connection_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let (Some(path), Some(model)) = (self.selected.clone(), self.model.clone())
        {
            // The UI owns the consumer; the worker gets the producer. Reversed from
            // every other view, because the data this pass needs cannot come here.
            let (trace, rx) = rumoca_phase_structural::live_trace::LiveTrace::new();
            let trace = trace.with_frame_delay(crate::live_frame_delay(self.live_breakpoint_armed));
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            self.worker.send(ToWorker::LiveDebugConnections {
                path,
                model,
                trace,
                done: std::sync::Arc::clone(&done),
            });
            self.compile_views.connection_anim = Some(Some(
                connection_anim::ConnectionAnimation::start_live(rx, done),
            ));
        }

        if self.compile_views.connection_anim.is_none() {
            let frames = &self.frames.connection;
            self.compile_views.connection_anim = Some(if frames.is_empty() {
                None
            } else {
                Some(connection_anim::ConnectionAnimation::from_frames(
                    frames.clone(),
                ))
            });
        }
        if let Some(Some(anim)) = &mut self.compile_views.connection_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, gate.arming, gate.debug_enabled))
                .inner;
        } else {
            ui.weak("(no connections in this model)");
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::Connections);
        }
    }

    /// The alias-elimination reveal, on the Index Reduction stage.
    ///
    /// Simpler than the replay views: no live-debug lifecycle, because this
    /// phase has no search to trace (see `alias_anim`'s module note).
    fn alias_anim_ui(&mut self, ui: &mut egui::Ui) {
        if self.stage_views.alias_anim.is_none() {
            self.stage_views.alias_anim = Some(
                self.stages
                    .get(self.stage)
                    .value
                    .as_ref()
                    .and_then(alias_anim::AliasAnimation::from_report),
            );
        }
        if let Some(Some(anim)) = &mut self.stage_views.alias_anim {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui));
        } else {
            ui.weak("(no alias eliminations in this report)");
        }
    }

    /// Whether the current report has alias eliminations worth a tab.
    ///
    /// Read straight from the stage JSON rather than the cache: the tab bar is
    /// drawn before the view is, so the cache may not exist yet.
    fn has_alias_eliminations(&self) -> bool {
        self.stages
            .get(self.stage)
            .value
            .as_ref()
            .and_then(|v| v.get("reduction")?.get("eliminations")?.as_array())
            .is_some_and(|a| !a.is_empty())
    }

    /// The initial-condition plan walk, on the Initialization stage.
    fn ic_plan_anim_ui(&mut self, ui: &mut egui::Ui) {
        if self.compile_views.ic_plan_anim.is_none() {
            self.compile_views.ic_plan_anim = Some(
                self.stages
                    .initialization
                    .value
                    .as_ref()
                    .and_then(ic_plan_anim::IcPlanAnimation::from_report),
            );
        }
        if let Some(Some(anim)) = &mut self.compile_views.ic_plan_anim {
            egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui));
        } else {
            ui.weak("(no initial-condition plan in this report)");
        }
    }

    /// Whether the initialization report has a plan worth a tab. See
    /// [`Self::has_alias_eliminations`] on why this reads the JSON directly.
    fn has_ic_plan(&self) -> bool {
        self.stages
            .initialization
            .value
            .as_ref()
            .and_then(|v| v.get("blocks")?.as_array())
            .is_some_and(|a| !a.is_empty())
    }

    /// Whether the compile captured a `pre()`-lowering trace worth a tab.
    ///
    /// A method rather than the inline `!self.frames.pre_lowering.is_empty()` for the same
    /// reason [`Self::flatten_content`] exists: **the sub-view row and the `hrw://` link
    /// guard must ask the same question**, and two spellings of it are two places to
    /// change.
    fn has_pre_lowering_trace(&self) -> bool {
        !self.frames.pre_lowering.is_empty()
    }

    /// What the Flatten compile produced, in the form its sub-view row and its link guard
    /// both read.
    ///
    /// **One builder, two consumers**, which is the whole point: the row draws a tab iff
    /// `sub_view_rows::flatten_view_available` approves it, and
    /// [`Self::apply_pending_view_and_seek`] refuses a link the same predicate rejects.
    /// Before 2026-08-21 the row built this literal inline and the link guard had no
    /// opinion at all — a `_ => true` arm — so a link could select a tab that was not
    /// drawn.
    fn flatten_content(&self) -> sub_view_rows::FlattenContent {
        sub_view_rows::FlattenContent {
            equation_sheet: self.cached_equation_sheet.is_some(),
            // **Spans, not the sheet.** A sheet is built from the DAE whether or not the
            // specimen's text could be read, and a source map with no lines is an empty
            // pane rather than a missing one.
            source_map: self
                .cached_equation_sheet
                .as_ref()
                .is_some_and(|s| !s.source_lines.is_empty()),
            connections: !self.frames.connection.is_empty(),
        }
    }

    /// The `pre()`-lowering replay (idea #40), on the Events stage.
    ///
    /// Mirrors `reduction_anim_ui` beat for beat — the six-step live-debug
    /// sequence is the same for every animated view. The one difference is what
    /// gets re-run for a live session: the flat model rather than the DAE,
    /// because this pass happens *inside* DAE construction and the DAE HRW holds
    /// is already past it.
    fn pre_lowering_anim_ui(&mut self, ui: &mut egui::Ui) {
        let gate = self.live_debug_gate(ui.ctx(), PendingLiveDebug::PreLowering, |a| {
            &a.compile_views.pre_lowering_anim
        });
        let mut debug_clicked = false;
        if gate.spawn_live
            && let Some(flat) = &self.cached_flat
        {
            let delay = crate::live_frame_delay(self.live_breakpoint_armed);
            let live = pre_lowering_anim::PreLoweringAnimation::start_live(flat.clone(), delay);
            if live.is_none() {
                let _ = bridge::remove_live_trace_breakpoint();
                self.live_breakpoint_armed = false;
            }
            self.compile_views.pre_lowering_anim = Some(live);
        }
        if self.compile_views.pre_lowering_anim.is_none() {
            let frames = &self.frames.pre_lowering;
            self.compile_views.pre_lowering_anim = Some(if frames.is_empty() {
                None
            } else {
                Some(pre_lowering_anim::PreLoweringAnimation::from_frames(
                    frames.clone(),
                ))
            });
        }
        if let Some(Some(anim)) = &mut self.compile_views.pre_lowering_anim {
            debug_clicked = egui::ScrollArea::vertical()
                .auto_shrink(false)
                .show(ui, |ui| anim.ui(ui, gate.arming, gate.debug_enabled))
                .inner;
        } else {
            ui.weak("(no pre() lowering in this model)");
        }
        if debug_clicked {
            self.start_live_debug(PendingLiveDebug::PreLowering);
        }
    }

    /// What the **stage** tree is told about the loaded specimen: the tracked name and the
    /// four indexes built from the compile.
    ///
    /// **`jump_to` and `highlight` are deliberately left `None`** — they address a *node*
    /// rather than describing the model, so they belong to whichever tree is being drawn,
    /// and the one caller fills them in.
    ///
    /// # It had two callers for a day, and that was the defect
    ///
    /// The navigated-class tree ([`crate::nav_view`]) was handed the same five fields, and
    /// **every one of them describes the specimen** — so a library class reached by "Go to
    /// definition" was underlined, offered for Follow, and could cite *"declared at line
    /// N"* naming a line of the specimen's file. Doug ruled on 2026-08-20 that the
    /// navigated tree is annotated *from the class or not at all*; since the five plus
    /// `jump_to` and `highlight` are the whole of [`tree::TreeOptions`], `nav_view_ui`
    /// stopped taking one rather than blanking each field. **The pane it must not reach
    /// can no longer name this method's return type**, which is why this doc no longer has
    /// to warn about it.
    ///
    /// Kept as a method though it now has one caller: it names the answer to *"what does a
    /// tree know about the specimen?"*, and the caller is a router already too long.
    fn specimen_tree_options(&self) -> tree::TreeOptions<'_> {
        tree::TreeOptions {
            tracked: self.tracked_identifier.as_deref(),
            known_variables: self.known_variables.as_ref(),
            declaring_classes: Some(&self.declaring_classes),
            variable_lines: self.identifier_index.as_ref().map(|i| &i.variables),
            // **Each stage gets ITS OWN map, or none.** Flatten numbers equations
            // differently from the DAE, so sharing one map would resolve confidently and
            // wrongly. Index reduction, initialization and structural carry no spans at
            // all and get `None`, which also keeps them from paying for a path string per
            // row.
            path_lines: self
                .cached_equation_sheet
                .as_ref()
                .and_then(|s| match self.stage {
                    StageKind::Dae => Some(&s.node_lines),
                    StageKind::Flatten => Some(&s.flat_node_lines),
                    _ => None,
                }),
            jump_to: None,
            highlight: None,
        }
    }

    /// The central panel: stage tab bar, status banners, sub-tab bars, and the
    /// per-stage view dispatch. The bulk of the frame.
    ///
    /// Extracted from [`Self::ui`] on 2026-07-29. This was the payoff of
    /// `FrameIntent`: the body records what the user asked for into `intent`
    /// and the caller acts on it after the panel closures end, so the whole
    /// block moves with a single out-parameter.
    ///
    /// The early `return` near the top (no specimen selected) exits this method,
    /// the panel closure then ends normally, and `ui`'s deferred-action block
    /// still runs — the same order as when this was a closure body.
    fn central_panel_ui(&mut self, ui: &mut egui::Ui, intent: &mut FrameIntent) {
        if self.nav.is_empty() {
            // ---- Specimen stage view ----
            // No specimen yet → no stages to show, so don't render the tab
            // row (a highlighted tab before any compile is misleading).
            // In Debug mode the specimen list is hidden, so show the
            // dropdown here so the user can pick their first specimen.
            // **The tab row is always on screen, disabled until there is
            // something to show.** Doug, 2026-08-02: *"Before, the tab bar was
            // always visible. Now, when I start HRW, the tab bar is not visible
            // until I select a specimen/model."*
            //
            // Checked rather than assumed: the early `return` below predates the
            // UI pause entirely — it is in `b1777e6a` and older — so this was not
            // a refactoring regression. It was long-standing behaviour that
            // became conspicuous when the startup screen changed.
            //
            // The original reasoning is preserved and narrowed. *"A highlighted
            // tab before any compile is misleading"* is true of a **highlighted**
            // tab; it is not an argument for hiding the row. **The pipeline is
            // the thing HRW teaches**, and a reader who cannot see its phases
            // until they pick a file has to already know what to expect. Greyed
            // tabs say "these exist, and are not ready yet" — which is exactly
            // the state, and is the same rule every other pane here follows:
            // report the empty state, never vanish.
            if self.selected.is_none() {
                // **Not disabled from out here.** The row carries Debug mode's
                // specimen switcher, which is the one control that *must* work
                // when nothing is selected — it is how a specimen gets chosen.
                // `stage_tab_bar_ui` disables the tabs itself, after the switcher.
                ui.horizontal_wrapped(|ui| self.stage_tab_bar_ui(ui, intent));

                // **The Context Bar is on screen even with nothing loaded** (Doug,
                // 2026-08-30), and the reason is not that it has much to say here —
                // it is that a bar which comes and goes cannot become a habit:
                //
                // > *"A context bar is novel for me. I need to learn to assume its
                // > presence and to make frequent use of it, just like I needed decades
                // > ago to learn to assume the presence of GUI menu bars."*
                //
                // Same argument the tab row won on 2026-08-02, three comment-paragraphs
                // above: *"report the empty state, never vanish."* That rule was applied
                // to the tabs and not to the bar below them, which is the inconsistency
                // this closes.
                ui.separator();
                self.context_bar_ui(ui);

                if self.ui_mode == UiMode::Debug {
                    // **Nothing here.** Debug mode's switcher lives in the tab
                    // row above and is drawn unconditionally now, so a second
                    // copy here put *two specimen selectors on screen* — one
                    // disabled, one not — which is what Doug reported on
                    // 2026-08-02. The row's copy already reads "(none)" and
                    // already lists every specimen.
                } else if self.ui_mode == UiMode::Lab {
                    // **Lab mode has no specimen list to select from**, so telling Doug
                    // to select one is advice he cannot take — the same species as the
                    // Purpose tab telling him to select a specimen he had just selected.
                    // In Lab mode the specimen arrives from a stop.
                    ui.weak("Walk a lab \u{2014} its first stop loads a specimen.");
                    ui.weak("Pick one from the list on the left.");
                } else {
                    ui.weak("Select a specimen to compile.");
                }
                return;
            }
            // `horizontal_wrapped` lays widgets left-to-right, wrapping to a
            // second line when they don't fit. With 11 stage tabs, a narrow
            // window needs this (vs `horizontal` which would clip).
            ui.horizontal_wrapped(|ui| self.stage_tab_bar_ui(ui, intent));

            if let Some(err) = &self.nav_error {
                ui.colored_label(ui.visuals().error_fg_color, err);
            }
            ui.separator();

            self.context_bar_ui(ui);

            if self.viewing_log {
                if log_view::ui(ui, &self.log_entries, &mut self.tracing_enabled) {
                    self.worker.send(ToWorker::SetTracing(self.tracing_enabled));
                }
            } else if self.stage == StageKind::Simulation {
                self.simulation_pane(ui);
            } else {
                // Stage note (in its own scope so its borrow of `self` ends
                // before the value section, which may borrow `self` mutably for
                // the spy-plot canvas).
                {
                    let stage = self.current_stage();
                    // Stages with structured error data show their own summary
                    // below; Structural/IndexReduction with singular/index-1
                    // notes show a status banner. Skip the generic note for both.
                    let has_error_summary = stage.note_is_error()
                        && stage.value.as_ref().and_then(|v| v.get("error")).is_some();
                    let has_custom_banner = matches!(
                        self.stage,
                        StageKind::Structural | StageKind::IndexReduction
                    ) && stage
                        .note
                        .as_deref()
                        .is_some_and(|n| n.contains("singular") || n.contains("index-1"));
                    if let Some(note) = &stage.note
                        && !has_custom_banner
                        && !has_error_summary
                    {
                        let color = if stage.note_is_error() {
                            ui.visuals().error_fg_color
                        } else {
                            ui.visuals().weak_text_color()
                        };
                        egui::ScrollArea::horizontal()
                            .id_salt("note")
                            .show(ui, |ui| {
                                ui.colored_label(color, egui::RichText::new(note).monospace());
                            });
                        ui.separator();
                    }
                }

                // **Three rows, drawn unconditionally, at most one of which appears** —
                // each begins with `stage == <its own stage>` and the app shows one
                // stage. Each returns its own gate, which the pane dispatch below
                // re-tests, so a tab that exists and a pane that draws cannot disagree.
                // See `sub_view_rows` for why Flatten is the odd member of the three.
                let flatten_ready = sub_view_rows::flatten_row_ui(
                    ui,
                    self.stage,
                    self.flatten_content(),
                    &mut self.viewport.flatten,
                );
                let events_ready = sub_view_rows::events_row_ui(
                    ui,
                    self.stage,
                    self.has_pre_lowering_trace(),
                    &mut self.viewport.events,
                );
                let init_ready = sub_view_rows::init_row_ui(
                    ui,
                    self.stage,
                    self.has_ic_plan(),
                    &mut self.viewport.init,
                );

                // The report stages (Structural + Index reduction) offer a custom
                // BLT spy-plot alongside the generic tree; every other stage is
                // tree-only.
                let report_stage = matches!(
                    self.stage,
                    StageKind::Structural | StageKind::IndexReduction
                );
                let report_ready = report_stage && self.current_stage().value.is_some();
                if report_ready {
                    // **The four questions are asked here rather than in the row**, so
                    // that a tab which exists and a link which is honoured cannot
                    // disagree: `structural_view_available` is the one predicate, and
                    // `apply_pending_view_and_seek` below consults the very same method.
                    // Two of the four additionally depend on what the compile *captured*,
                    // which is state the row never otherwise touches.
                    let available = report_sub_view::TabAvailability {
                        summary: self.structural_view_available(StructuralView::Summary),
                        animate: self.structural_view_available(StructuralView::Animate),
                        aliases: self.structural_view_available(StructuralView::AliasAnim),
                        spy_plot: self.structural_view_available(StructuralView::SpyPlot),
                    };
                    report_sub_view::report_sub_view_row_ui(
                        ui,
                        self.stage,
                        &self.stages,
                        &mut self.stage_views,
                        &mut self.viewport.structural,
                        available,
                    );
                }
                // **Unconditional, and that is the fix.** This used to live inside the
                // row above, so it ran only for Structural and Index Reduction — see
                // `apply_pending_view_and_seek`. Every Flatten, Events and
                // Initialization link naming a non-default sub-view, and every frame
                // seek into them, was discarded in silence.
                self.apply_pending_view_and_seek();
                // **Last, so it sees every door.** The row above, the stage-change default
                // inside it and the link guard on the line above are the three ways the
                // structural sub-view gets set; this checks the result of all three rather
                // than trusting each. It is a no-op on every known path — see the method.
                self.clamp_structural_sub_view();

                // Whether the Index Reduction tab shows a Before/After split for
                // comparative views. True when index reduction was actually needed
                // (the note mentions "singular").
                let ir_split = report_ready
                    && self.stage == StageKind::IndexReduction
                    && self
                        .stages
                        .get(self.stage)
                        .note
                        .as_deref()
                        .is_some_and(|n| n.contains("singular"));

                if report_ready && self.viewport.structural == StructuralView::SpyPlot {
                    matrix_panes::spy_plot_pane_ui(
                        ui,
                        self.stages.get(self.stage).value.as_ref(),
                        matrix_panes::MatrixPane {
                            cache: &mut self.stage_views.spy_plot,
                            camera: &mut self.viewport.spy,
                        },
                        &mut intent.canvas_capture,
                        self.tracked_identifier.as_deref(),
                        ir_split,
                    );
                } else if report_ready && self.viewport.structural == StructuralView::Incidence {
                    matrix_panes::incidence_pane_ui(
                        ui,
                        self.stages.get(self.stage).value.as_ref(),
                        matrix_panes::MatrixPane {
                            cache: &mut self.stage_views.incidence,
                            camera: &mut self.viewport.incidence,
                        },
                        // **The Before pane exists exactly when the split is on**, so the
                        // pane cannot be handed a camera it must not draw into. See the
                        // module: the spy plot next door keeps a `bool` because it has no
                        // Before pane at all.
                        ir_split.then_some(matrix_panes::MatrixPane {
                            cache: &mut self.stage_views.before_incidence,
                            camera: &mut self.viewport.before_incidence,
                        }),
                        &mut intent.canvas_capture,
                        self.tracked_identifier.as_deref(),
                        self.viewport.highlighted_eq_row,
                    );
                } else if report_ready && self.viewport.structural == StructuralView::MatchingAnim {
                    self.matching_anim_ui(ui, ir_split);
                } else if report_ready && self.viewport.structural == StructuralView::TarjanAnim {
                    self.tarjan_anim_ui(ui, ir_split);
                } else if report_ready && self.viewport.structural == StructuralView::TearingAnim {
                    self.tearing_anim_ui(ui);
                } else if report_ready && self.viewport.structural == StructuralView::AliasAnim {
                    self.alias_anim_ui(ui);
                } else if report_ready && self.viewport.structural == StructuralView::Summary {
                    if self.stage == StageKind::Structural {
                        crate::error_summary::structural_singular_summary(
                            ui,
                            &self.stages.structural,
                        );
                    } else {
                        let cached = self.stage_views.reduction.get_or_insert_with(|| {
                            self.stages
                                .get(self.stage)
                                .value
                                .as_ref()
                                .and_then(reduction_view::ReductionView::from_report)
                        });
                        if let Some(view) = cached {
                            view.ui(ui, self.tracked_identifier.as_deref());
                        } else {
                            ui.weak("(no reduction data in this report)");
                        }
                    }
                } else if report_ready && self.viewport.structural == StructuralView::Animate {
                    // **`report_ready` was missing here and nowhere else in this chain**
                    // (fixed 2026-08-20). `Animate` is an Index-Reduction-only sub-view,
                    // and `viewport.structural` deliberately survives a stage change —
                    // it is a camera. Nothing clamps it off a report stage either:
                    // `clamp_structural_sub_view` returns early on every other stage, by
                    // design. So an unguarded branch here meant *this* pane won on
                    // Events, Initialization and Flatten too, since it sits above all
                    // three of theirs: leave Index Reduction ▸ Animate for Events and
                    // the index-reduction replay was drawn under the Events tab.
                    self.reduction_anim_ui(ui);
                } else if events_ready && self.viewport.events == EventsView::PreLowering {
                    self.pre_lowering_anim_ui(ui);
                } else if init_ready && self.viewport.init == InitView::IcPlan {
                    self.ic_plan_anim_ui(ui);
                } else if flatten_ready && self.viewport.flatten == FlattenView::Equations {
                    self.equation_sheet_ui(ui);
                } else if flatten_ready && self.viewport.flatten == FlattenView::SourceMap {
                    self.source_map_ui(ui);
                } else if flatten_ready && self.viewport.flatten == FlattenView::Connections {
                    self.connection_anim_ui(ui);
                } else {
                    // **The arm with no gate** — every other member of this chain asks
                    // whether its own sub-view is selected, and this one draws when none
                    // of them claimed the frame. It is also the only pane most stages
                    // ever show. See `artifact_pane` for the "beside, not instead of"
                    // rule its error summary follows.
                    let stage = self.current_stage();
                    let notice = artifact_pane::artifact_pane_ui(
                        ui,
                        stage,
                        self.stage,
                        artifact_pane::ArtifactChrome {
                            label: self.model.as_deref().unwrap_or("model"),
                            prev: self.previous_stage_value(),
                            identifier_count: self.known_variables.as_ref().map(HashSet::len),
                            compiling: self.compiling,
                        },
                        artifact_pane::ArtifactTree {
                            opts: self.specimen_tree_options(),
                            def_index: &self.def_index,
                            field_help: &self.field_help,
                            jump_target: self.context.jump_target.as_deref(),
                            jump_highlight: self.context.jump_highlight.as_deref(),
                        },
                        &mut intent.tree,
                    );
                    // The stage borrow ends with the call, so the notice can finally be
                    // posted — the same deferred pattern as `FrameIntent`.
                    if let Some(msg) = notice {
                        self.context.jump_target = None;
                        self.notify(msg);
                    }
                }
            } // end: non-Simulation stage rendering
        } else {
            // ---- Navigation view (a class reached via "Go to definition") ----
            // The whole branch lives in `nav_view`; it shares nothing with the stage view
            // above but the tree widget. The two buttons cannot touch `self.nav` from
            // inside a panel closure, so they report — the `FrameIntent` pattern, one
            // pane's worth.
            //
            // **The bar comes first here too**, because "always" has to mean always or
            // it is back to being something to check for. There is no tab row in this
            // branch, so it sits at the top rather than under one.
            self.context_bar_ui(ui);
            match nav_view::nav_view_ui(
                ui,
                &self.nav,
                nav_view::NavChrome {
                    model: self.model.as_deref(),
                    loading: self.nav_loading.as_deref(),
                    error: self.nav_error.as_deref(),
                },
                &self.field_help,
                &mut intent.tree,
            ) {
                Some(nav_view::NavCommand::Home) => intent.go_home = true,
                Some(nav_view::NavCommand::Back) => intent.go_back = true,
                None => {}
            }
        }
    }

    /// Work out which source lines the compiler blamed, from the stage errors.
    ///
    /// **Only fills in when the model is genuinely ill-posed**, meaning the Structural
    /// stage failed *and* index reduction could not rescue it. That condition is the
    /// whole point rather than caution: `MotorWithBrake` is structurally singular too,
    /// with unmatched unknowns and source lines to match, and it is a perfectly good
    /// model — high-index, not broken. Index reduction demotes a state and it solves.
    /// Painting its `connect()` line as a problem would teach something false.
    ///
    /// `CapacitorLoop` is the other case: states 1 → 1, nothing demoted, still
    /// singular. Nothing downstream can save it, so the blame is real.
    ///
    /// Reads the **index reduction** payload rather than the structural one, because
    /// that is the stage whose failure means "unrescuable". Both name the same unknown
    /// here, but the reduced system is the one actually stuck.
    fn compute_problem_lines(&mut self) {
        self.problem_lines.clear();
        let structural_failed = self
            .stages
            .structural
            .value
            .as_ref()
            .is_some_and(|v| v.get("error").is_some());
        if !structural_failed {
            return;
        }
        // Index reduction succeeding means high-index at worst, never ill-posed.
        let Some(err) = self
            .stages
            .index_reduction
            .value
            .as_ref()
            .and_then(|v| v.get("error"))
        else {
            return;
        };
        let Some(locs) = err
            .get("unmatched_unknown_locations")
            .and_then(|v| v.as_array())
        else {
            return;
        };
        for entry in locs {
            // No source provenance means a manufactured or solver-vector variable.
            // There is no line to blame, and inventing one would be worse than silence.
            let Some(line) = entry
                .get("location")
                .and_then(|l| l.get("line"))
                .and_then(serde_json::Value::as_u64)
            else {
                continue;
            };
            let unknown = entry
                .get("unknown")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("?");
            self.problem_lines.push((
                line as u32,
                format!(
                    "No equation determines `{unknown}`, and index reduction could not fix \
                     it \u{2014} this model is ill-posed, not merely high-index. See the \
                     Structural and Index Reduction tabs."
                ),
            ));
        }
    }

    /// Point the current stage's sub-view selector at `sub`, if one was requested.
    ///
    /// Each stage keeps its own selector enum, so this is a small dispatch rather
    /// than one assignment. A `SubView` can only have been built by
    /// `SubView::from_slug` against a stage, so a mismatch here would mean the link
    /// named one stage and the app is on another — possible if the user clicked away
    /// mid-compile. Ignoring it is right: better to leave the view where the user put
    /// it than to yank it somewhere the link no longer describes.
    fn apply_sub_view(&mut self, sub: Option<SubView>) {
        match (sub, self.stage) {
            (Some(SubView::Structural(v)), StageKind::Structural | StageKind::IndexReduction) => {
                self.viewport.structural = v;
            }
            (Some(SubView::Flatten(v)), StageKind::Flatten) => self.viewport.flatten = v,
            (Some(SubView::Events(v)), StageKind::Events) => self.viewport.events = v,
            (Some(SubView::Init(v)), StageKind::Initialization) => self.viewport.init = v,
            _ => {}
        }
    }

    /// Re-read `.hrw-bridge/lab.md` if it changed since the last read.
    ///
    /// Polled rather than watched: a `stat` every [`LAB_POLL_INTERVAL`] is
    /// simpler than a filesystem watcher, has no platform quirks, and a lab
    /// appearing a quarter-second late is imperceptible. Re-reads only when the
    /// mtime differs, so an unchanged lab costs one `stat` per poll and no
    /// markdown re-parse.
    /// Re-read the lab list and the selected lab's text, at most once per
    /// [`LAB_POLL_INTERVAL`].
    ///
    /// So a lab Claude writes mid-conversation appears without restarting HRW.
    fn poll_lab_file(&mut self) {
        if self.lab.poll() {
            self.reset_for_new_lab();
        }
    }

    /// Switch the Lab panel to `source`, discarding the previous text.
    ///
    /// Clears `cached_lab` rather than letting the poll notice: without this the old
    /// lab stays on screen until the next mtime comparison, and a reader who just
    /// clicked a different lab would see the previous one for up to a poll interval.
    fn select_lab(&mut self, source: LabSource) {
        // **Switching labs re-initialises the right-hand side.** A lab is a
        // self-contained sequence starting from its own first stop, which normally
        // loads a specimen. Leaving the previous lab's model on screen invites reading
        // the new lab's stops against the old lab's state — and worse, makes Station 1
        // look as though it has already been done.
        //
        // Only on an actual change: re-clicking the lab already showing should not
        // throw away a specimen the reader is partway through.
        // **Remember where we were, before the selection changes.** Every switch is
        // undoable — picker, `hrw://lab/…` link, or the Answer button — because a reader
        // who lands somewhere unintended wants out regardless of how they arrived.
        if let Some(previous) = self.lab.selected.clone()
            && previous != source
        {
            self.lab.history.push((previous, self.lab.current_scroll_y));
        }
        if self.lab.select(source) {
            self.reset_for_new_lab();
        }
    }

    /// **Return to the lab the reader came from, at the offset they left it.**
    ///
    /// Pops rather than pushes, which is the whole of the *"do not record your own
    /// navigation"* problem `ideas.md` #78 warns about: a Back that pushed the lab it is
    /// leaving would ping-pong between two documents and never reach the rest of the
    /// stack.
    ///
    /// **No Forward, deliberately.** The back destination is unreachable — nothing in
    /// `blt-ordering` links to `index-reduction`, which is why Back exists at all. The
    /// forward destination is one click away, because the link that went there sits in the
    /// prose being returned to. Forward would duplicate an available navigation and bring
    /// the suppression-flag bug Back does not have. Purely additive if the friction ever
    /// appears: the same stack, pushed instead of dropped.
    fn lab_back(&mut self) {
        let Some((previous, offset)) = self.lab.history.pop() else {
            return;
        };
        if self.lab.select(previous) {
            self.reset_for_new_lab();
        }
        // After `reset_for_new_lab`, which requests the top.
        self.lab.restore_scroll_y = Some(offset);
        self.lab.polled_at = None;
        self.poll_lab_file();
    }

    /// Re-initialise the right-hand side for a lab that just became current.
    ///
    /// **Stays on `App` deliberately.** A lab is a self-contained sequence
    /// starting from its own first stop, which normally loads a specimen, so
    /// switching labs must clear the stage side — but *stages, selection and the
    /// log are not the lab panel's to touch*. [`LabState`] reports that the
    /// selection changed; deciding what that invalidates is the application's job.
    fn reset_for_new_lab(&mut self) {
        self.clear_specimen_state(false);
        self.selected = None;
        self.stage = StageKind::Parse;
        self.viewing_log = false;
        self.compiling = false;
        // Scroll positions are measured in the *previous* document and mean nothing
        // in this one. Keeping them let a new run interpolate from wherever the last
        // one was stopped.
        self.lab.reset_scroll();
    }

    /// The top menu bar (File, View, Help).
    ///
    /// Extracted from `ui` during the 2026-07-28 sweep — self-contained, and
    /// `ui` had grown past the point where a reader could see its shape.
    fn menu_bar_ui(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("bar").show(ui, |ui| {
            // `MenuBar` creates a horizontal bar with dropdown menus.
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Rescan specimens").clicked() {
                        self.model_list.rescan();
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Settings…").clicked() {
                        self.show_settings = true;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Quit").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
                ui.menu_button("View", |ui| {
                    if ui
                        .selectable_label(self.ui_mode == UiMode::Lab, UiMode::Lab.label())
                        .clicked()
                    {
                        self.ui_mode = UiMode::Lab;
                        self.split.request_reset(MODE_SWITCH_RESET);
                        ui.close();
                    }
                    if ui
                        .selectable_label(
                            self.ui_mode == UiMode::Specimen,
                            UiMode::Specimen.label(),
                        )
                        .clicked()
                    {
                        self.ui_mode = UiMode::Specimen;
                        self.split.request_reset(MODE_SWITCH_RESET);
                        ui.close();
                    }
                    if ui
                        .selectable_label(self.ui_mode == UiMode::Debug, UiMode::Debug.label())
                        .clicked()
                    {
                        self.ui_mode = UiMode::Debug;
                        self.split.request_reset(MODE_SWITCH_RESET);
                        ui.close();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Using HRW…").clicked() {
                        self.show_help = true;
                        ui.close();
                    }
                    if ui.button("About HRW…").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                    ui.separator();
                    // For problems that do NOT kill the app. A crash writes its own
                    // file; a wrong-looking view or a hang writes nothing, and the
                    // evidence needed to diagnose it is identical. The path goes into
                    // the status bar because a file nobody can find is a file nobody
                    // sends.
                    if ui
                        .button("Write diagnostic snapshot")
                        .on_hover_text(
                            "Write the current app state, recent actions, and log tail to \
                     .hrw-bridge/diagnostics/ for Claude to read. Use when something \
                     looks wrong but HRW has not crashed — crashes write their own file.",
                        )
                        .clicked()
                    {
                        let msg = match diagnostics::write_on_demand() {
                            Ok(path) => format!("diagnostic written: {}", path.display()),
                            Err(e) => format!("diagnostic FAILED: {e}"),
                        };
                        self.notify(msg);
                        ui.close();
                    }
                });
            });
        });
    }

    /// The specimen's Modelica source pane — see
    /// [`specimen_source::specimen_source_ui`], which holds the body.
    ///
    /// **The follow stays here on purpose.** The pane hands back the identifier that
    /// was clicked and `App` acts on it, because `set_tracked_identifier` is shared
    /// with the tree and the equation sheet — pushing it into the pane would give the
    /// pane a policy it does not own, and would cost it the `&mut self` it just shed.
    fn specimen_source_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(name) = specimen_source::specimen_source_ui(
            ui,
            &mut self.source,
            &self.model,
            &self.selected,
            self.selected_is_library,
            &self.tracked_identifier,
            &self.identifier_index,
            &self.problem_lines,
        ) {
            self.set_tracked_identifier(name);
        }
    }

    /// Re-emit the focus file from whatever context is currently assembled.
    ///
    /// The single write path, so the file always reflects **both** halves at
    /// once: the point retained in `pointed_at` and the thread in
    /// `tracked_identifier`. Called when either changes.
    ///
    /// This is what stops ambient following from destroying deliberate
    /// pointing. Following used to emit nothing at all; the naive fix — writing
    /// a fresh focus on every track change — would have overwritten the node
    /// the user meant to ask about. Retaining the point and re-emitting both
    /// keeps them independent, which is the property `docs/context-assembly.md`
    /// asks for.
    fn emit_context(&mut self) {
        let stage_values = self.stages.as_stage_pairs();
        // Summarised now (on a click), never per frame — it walks every stage.
        // Computed before the borrow below, since `tracking` borrows self.
        let summary = self
            .tracking_context(&stage_values)
            .as_ref()
            .map(bridge::summarize_tracking);
        self.context.tracking_summary = summary;

        let tracking = self.tracking_context(&stage_values);
        let Some(point) = self.context.pointed_at.clone() else {
            // Following with nothing pointed at is a normal state; emit the
            // thread on its own rather than withholding context.
            let ask = Ask {
                seq: self.context.context_seq,
                // NOT `Focus::Stage`. See `Focus::Nothing`.
                request: bridge::AskRequest::Explain,
                specimen: self.selected.as_deref(),
                model: self.model.as_deref(),
                stage: Some(self.stage),
                libraries: self.library_strings(),
                def_index: &self.def_index,
                parse_value: self.stages.parse.value.as_ref(),
                resolve_value: self.stages.resolve.value.as_ref(),
                focus: Focus::Nothing,
                tracking,
                view: self.view_context(),
                failure: self.failure_context(),
            };
            self.context.point_error = bridge::write(&ask).err().map(|e| e.to_string());
            return;
        };

        // Rebuild whichever shape was captured. Handling only `Node` here would
        // silently drop stage and specimen captures on the next follow-change,
        // reintroducing the disagreement between bar and file.
        // **A lab passage is emitted before the stage lookup, because it has no
        // stage.** Everything below rebuilds a capture out of some stage's IR; a
        // passage of prose is the first point that is not in a compile at all.
        if let PointKind::LabPassage { lab } = &point.kind {
            let ask = Ask {
                seq: point.seq,
                request: point.request,
                specimen: self.selected.as_deref(),
                model: self.model.as_deref(),
                // Genuinely none. The reader was looking at prose, not at a phase.
                stage: None,
                libraries: self.library_strings(),
                def_index: &self.def_index,
                parse_value: self.stages.parse.value.as_ref(),
                resolve_value: self.stages.resolve.value.as_ref(),
                focus: Focus::LabPassage {
                    lab,
                    text: &point.target,
                },
                tracking,
                view: self.view_context(),
                failure: self.failure_context(),
            };
            self.context.point_error = bridge::write(&ask).err().map(|e| e.to_string());
            return;
        }

        // Every remaining shape is a location in a stage's IR, so a stage is required
        // and its absence is a bug rather than a state — the three IR capture sites all
        // record one.
        let Some(point_stage) = point.stage else {
            self.context.point_error =
                Some("capture recorded no stage, so it cannot be re-emitted".to_owned());
            return;
        };
        let stage_value = self.stages.get(point_stage).value.clone();
        let focus = match (&point.kind, &stage_value) {
            (PointKind::Node(key_path), Some(value)) => Focus::Node {
                key_path: key_path.clone(),
                stage_value: value,
            },
            // The stage's IR is not available, so the node cannot be described.
            // Skip re-emission rather than dropping the point: the file still
            // holds what Claude has, and the bar still describes that file, so
            // the two stay in agreement. Discarding the capture here would
            // destroy deliberate context over a transient condition — a
            // recompile clears the point through `reset`, which is the place
            // that knows the old IR is genuinely gone.
            (PointKind::Node(_), None) => return,
            (PointKind::Stage, _) => Focus::Stage,
            (PointKind::Specimen, _) => Focus::Specimen,
            // Returned above, before the stage lookup this match sits inside.
            (PointKind::LabPassage { .. }, _) => return,
        };
        let ask = Ask {
            seq: point.seq,
            request: point.request,
            specimen: self.selected.as_deref(),
            model: self.model.as_deref(),
            // The stage the capture was MADE in, not the one now on screen.
            // A bar reading "Structural" for a point captured in Flatten would
            // be describing context Claude does not have.
            stage: point.stage,
            libraries: self.library_strings(),
            def_index: &self.def_index,
            parse_value: self.stages.parse.value.as_ref(),
            resolve_value: self.stages.resolve.value.as_ref(),
            focus,
            tracking,
            view: self.view_context(),
            failure: self.failure_context(),
        };
        self.context.point_error = bridge::write(&ask).err().map(|e| e.to_string());
    }

    /// Arm a lab-passage capture: ask egui to copy the selection, and wait for it.
    ///
    /// Pushing [`egui::Event::Copy`] is the only way to get at a label selection's
    /// text — see [`PendingPassage`] for why the application cannot simply read it.
    /// **Ctrl+C deliberately does not do this**, on Doug's ruling: a copy made to paste
    /// somewhere else must not silently change what Claude has.
    fn arm_lab_passage_capture(&mut self, ctx: &egui::Context, lab: String) {
        // **Recorded at the PRESS, not only at the capture**, so the action trail
        // distinguishes the two failures. Without it, "nothing happened" was
        // indistinguishable from "the click never arrived" — and it was the latter for
        // a whole evening, because `session.json` is written only when an action is
        // recorded, so a press that reached nothing left the file untouched and every
        // reading of it described HRW's state at startup.
        diagnostics::record_action("point-at-selection", format!("in {lab}"));
        ctx.input_mut(|i| i.events.push(egui::Event::Copy));
        self.pending_passage = Some(PendingPassage {
            lab,
            frames_left: 3,
        });
    }

    /// Collect the copied text, if egui has produced it yet.
    ///
    /// **Reads `output` without draining it**, so an ordinary Ctrl+C still reaches the
    /// clipboard: the command stays in the queue for the backend to act on. This only
    /// looks.
    fn collect_pending_passage(&mut self) {
        // **Drained unconditionally, even with nothing pending.** Otherwise an ordinary
        // Ctrl+C would leave text sitting in the sink, and the next 🎯 press would
        // collect *that* instead of the passage just selected — a capture attaching
        // itself to an older gesture, which is the failure `PendingPassage`'s expiry
        // exists to prevent and would reintroduce by the back door.
        let copied = self.copy_sink.lock().ok().and_then(|mut s| s.take());
        let Some(pending) = &mut self.pending_passage else {
            return;
        };
        match copied {
            Some(text) => {
                let lab = pending.lab.clone();
                self.pending_passage = None;
                self.capture_lab_passage(lab, text);
            }
            None => {
                pending.frames_left = pending.frames_left.saturating_sub(1);
                if pending.frames_left == 0 {
                    self.pending_passage = None;
                    // **Said out loud, because the bar will show the PREVIOUS point.**
                    // Silence here would read as "the capture worked", and the reader
                    // would ask about a passage Claude never received.
                    //
                    // **The likeliest cause is a caret rather than a selection**, and
                    // the message leads with it: egui's `has_selection` is
                    // `selection.is_some()`, and a click that only places a cursor
                    // makes it `Some`. Nothing public distinguishes the two, so the
                    // button appears for both and this is where the difference shows.
                    self.notify(
                        "\u{26a0} nothing captured \u{2014} a cursor position is not a \
                         selection. Drag across the text you mean, then press \u{1f3af}.",
                    );
                }
            }
        }
    }

    /// Make a selected lab passage the point, and emit it.
    fn capture_lab_passage(&mut self, lab: String, text: String) {
        // **Collapsed to one line for the bar and the log.** The passage itself goes to
        // `focus.json` in full; a paragraph rendered into a one-row bar would push the
        // × button off the edge, and the bar's job is to say WHAT is held, not hold it.
        let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let shown: String = if flat.chars().count() > 60 {
            format!("{}\u{2026}", flat.chars().take(60).collect::<String>())
        } else {
            flat
        };
        let target = format!("\u{201c}{shown}\u{201d}");

        diagnostics::record_action("point-at-lab-passage", format!("in {lab}"));
        self.context.jump_highlight = None;
        let seq = self.context.next_seq();
        self.context.pointed_at = Some(PointedAt {
            seq,
            // The bar shows the abbreviation; `emit_context` sends `target` as the
            // passage text, so this MUST be the full prose, not the elision.
            target: text,
            kind: PointKind::LabPassage { lab },
            // No stage. A passage of prose is not in one.
            stage: None,
            request: bridge::AskRequest::Explain,
        });
        self.emit_context();
        self.notify(format!(
            "\u{1f3af} pointing at {target} \u{2014} now ask about it"
        ));
    }

    /// Drop a retained point if the recompiled IR no longer contains it.
    ///
    /// Runs once per compile, after the new stages land. Only `PointKind::Node`
    /// can dangle — a stage or specimen point names something that exists by
    /// construction, and **a lab passage is not in the compile at all**, so
    /// recompiling a specimen cannot invalidate it. The arm below says so; this
    /// sentence said "a stage or specimen point" until the variant existed.
    ///
    /// The **follow is deliberately not validated**: it is a name, not an
    /// address, and a name matching nothing is already reported honestly as
    /// `mentions: 0` in every stage. Dropping it would discard a deliberate
    /// choice in order to answer a question the emitted context answers better.
    ///
    /// Re-emits when anything survived, because the focus file still describes
    /// the *previous* compile's IR until it is rewritten — same node, different
    /// values — and a stale subtree is worse than an absent one.
    fn revalidate_point_against_new_ir(&mut self) {
        let dangling = match &self.context.pointed_at {
            Some(point) => match &point.kind {
                PointKind::Node(key_path) => point.stage.is_none_or(|s| {
                    !self
                        .stages
                        .get(s)
                        .value
                        .as_ref()
                        .is_some_and(|value| bridge::node_exists(value, key_path))
                }),
                // **A lab passage cannot dangle on a recompile**, which is the whole
                // reason it is listed beside the two that name something existing by
                // construction: it points at prose in a document, and compiling a
                // specimen does not touch the labs.
                PointKind::Stage | PointKind::Specimen | PointKind::LabPassage { .. } => false,
            },
            None => false,
        };
        if dangling {
            let target = self
                .context
                .pointed_at
                .take()
                .map(|p| p.target)
                .unwrap_or_default();
            // Said out loud. Silently dropping it would leave the user believing
            // Claude still has a node that has vanished from the bar.
            self.notify(format!(
                "point dropped \u{2014} {target} no longer exists in the recompiled IR",
            ));
            diagnostics::record_action("point-dropped", target);
        }
        if self.context.pointed_at.is_some() || self.tracked_identifier.is_some() {
            self.emit_context();
        }
    }

    /// Rebuild the jump match list if the stage or the followed name changed.
    ///
    /// Cheap on the common path — a tuple comparison — because the walk itself
    /// is not: it visits every node of the stage IR and lexes every code-bearing
    /// string. Same discipline as `tracking_summary`, which is computed on a
    /// click and never per frame.
    fn refresh_jump_matches(&mut self) {
        let key = self
            .tracked_identifier
            .as_ref()
            .map(|name| (self.stage, name.clone()));
        if key == self.context.jump_key {
            return;
        }
        self.context.jump_matches = match (&key, self.current_stage().value.as_ref()) {
            (Some((_, name)), Some(value)) => bridge::mention_paths(value, name),
            _ => Vec::new(),
        };
        // Restart the cycle: an index carried over from another stage would
        // point at a match that no longer exists, and "3 of 4" would be a lie
        // about a different list.
        self.context.jump_index = 0;
        self.context.jump_key = key;
    }

    /// Advance to the next match and ask the tree to scroll to it.
    ///
    /// Wraps around rather than stopping at the end — with a handful of matches,
    /// a dead button at the last one is a worse surprise than returning to the
    /// first.
    fn jump_to_next_match(&mut self, forward: bool) {
        if self.context.jump_matches.is_empty() {
            return;
        }
        let n = self.context.jump_matches.len();
        self.context.jump_index = if forward {
            (self.context.jump_index + 1) % n
        } else {
            (self.context.jump_index + n - 1) % n
        };
        self.context.jump_target = Some(self.context.jump_matches[self.context.jump_index].clone());
        // The matches live in a stage's IR, so the tree has to be on screen for
        // the jump to land. Leaving the log showing would make the button look
        // broken.
        self.viewing_log = false;
        diagnostics::record_action(
            "jump",
            format!(
                "match {} of {n} for {} in {}",
                self.context.jump_index + 1,
                self.tracked_identifier.as_deref().unwrap_or("?"),
                self.stage.name(),
            ),
        );
    }

    /// How to leave the empty context, given what is **actually on screen**.
    ///
    /// The first version of this line was generic — "left-click a node to point
    /// at it, or right-click a variable name to follow it" — and Doug met it
    /// immediately after loading a specimen, where it was wrong twice over: the
    /// log view was showing, so there was no tree and no node to left-click;
    /// and the only clickable things in sight were source identifiers, which
    /// are **left**-click-to-follow, not right-click.
    ///
    /// A hint that names an unavailable gesture is worse than no hint. It is
    /// also the same defect as everything else this bar exists to prevent — a
    /// confident statement that does not match the state — so it is built from
    /// the state rather than written once.
    ///
    /// Each branch is a fact about what is rendered right now:
    /// - Source identifiers are underlined and clickable only in Specimen mode
    ///   showing Source, and only once a compile has produced the identifier
    ///   index (it is `None` while compilation is still running).
    /// - With the log showing there is no IR view at all, so the way forward is
    ///   a stage tab, not a node.
    fn empty_context_hint(&self) -> String {
        let mut ways: Vec<&str> = Vec::new();
        if self.ui_mode == UiMode::Specimen
            && self.specimen_detail == SpecimenDetail::Source
            && self.identifier_index.is_some()
        {
            ways.push("left-click an underlined identifier in the source to follow it");
        }
        ways.push(if self.viewing_log {
            "open a stage tab to inspect its IR"
        } else {
            "left-click a node to point at it"
        });
        // **Not "nothing assembled".** The background is always assembled and
        // always emitted, so that wording made the bar contradict `focus.json`
        // for every reader who had not yet clicked anything -- the majority
        // state. What is missing is a *selection*, which is what this now says.
        //
        // **No leading em dash since 2026-08-30.** It was punctuation for sitting
        // beside the background line; the hint is hover text now, introduced by
        // "Right now:", and a dash after a colon reads as a stutter.
        ways.join(", or ")
    }

    /// The **stage tab row**: one tab per compilation phase, plus Simulation and
    /// the Log.
    ///
    /// Lifted out of `central_panel_ui` on 2026-08-02, when that was 760 lines
    /// and the largest thing left in the file.
    ///
    /// **What is left here is the chrome that genuinely needs the application**, after
    /// the tabs left for [`crate::stage_tabs`] on 2026-08-19: the Debug-mode specimen
    /// switcher, the Log button, the inline ▶ button, and the two status spinners.
    ///
    /// **Still `&mut self`, and that is now a finding rather than an admission.** The
    /// two `App` methods this row calls — `open` and `start_simulation` — are the reason
    /// the whole function could not move: both set state that the widgets *below* them
    /// read in the same frame, so reporting the press instead of performing it would
    /// draw one stale frame. Everything downstream of them reads state and was
    /// extractable; the region rule in `docs/app-split-plan.md` is exactly this
    /// distinction.
    ///
    /// Guarded by three headless tests from the baseline suite's chunk 3: a tab
    /// click selects the stage, leaves the log view, and reaches the Context Bar.
    fn stage_tab_bar_ui(&mut self, ui: &mut egui::Ui, intent: &mut FrameIntent) {
        // Specimen switcher dropdown — only in Debug mode, where
        // the specimen list is hidden.
        if self.ui_mode == UiMode::Debug {
            let current_name = self
                .selected
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|n| n.to_str())
                .unwrap_or("(none)");
            let combo = egui::ComboBox::from_id_salt("specimen_switcher")
                .selected_text(current_name)
                .width(120.0);
            let mut switch_to = None;
            combo.show_ui(ui, |ui| {
                for path in &self.model_list.files {
                    let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("?");
                    let is_selected = self.selected.as_deref() == Some(path.as_path());
                    if ui.selectable_label(is_selected, name).clicked() {
                        switch_to = Some(path.clone());
                    }
                }
            });
            if let Some(path) = switch_to {
                self.open(path);
            }
            ui.separator();
        }
        // **The tabs go inert without a specimen; the switcher above does
        // not.** Drawn either way so the pipeline's phases are visible before
        // anything is loaded, but a click must not set a stage that would
        // linger and fire when a specimen arrived later. Disabling here
        // rather than at the call site is what keeps the switcher usable —
        // `Ui::disable` applies to everything drawn after it.
        if self.selected.is_none() {
            ui.disable();
        }

        if ui.selectable_label(self.viewing_log, "Log").clicked() {
            self.viewing_log = true;
        }
        // **The ▶ button and its spinner moved into the tab row on 2026-08-29**
        // (`stage_tabs.rs`), to sit between the last divider and the Simulation label
        // rather than here beside the Log button. What stays behind is `can_sim`: it
        // reads four `App` fields the row has no business holding and travels as one
        // bool — not compiling, not already simulating, a model was parsed, and solve
        // lowering succeeded, since the simulator needs the `SolveModel` IR.
        //
        // **AND THE SEPARATOR THAT USED TO SIT HERE WENT WITH IT.** It had divided Log
        // from ▶; once ▶ left it fell against `stage_tabs_ui`'s own leading divider and
        // drew a SECOND rule between "Log" and "Parse". Doug reported it 2026-08-30.
        // **Nothing could have caught it** — a separator carries no accessibility
        // label, so a headless harness cannot see one, let alone count two. This is
        // the layout class where his report is the verification.
        let can_sim = !self.compiling
            && !self.sim_running
            && self.model.is_some()
            && self.stages.solve_lowering.value.is_some();
        // ---- The tabs themselves ----
        //
        // Everything from here down left for `stage_tabs.rs` on 2026-08-19: it is the
        // longest contiguous span of this function that calls no `App` method, which is
        // what made it extractable at all. The two presses above (`open`,
        // `start_simulation`) cannot be deferred, because the row below reads the state
        // they set in the same frame — see that module's header.
        //
        // The row reports rather than performs, so both consequences of a tab click are
        // answered here: leaving the log view, which both variants share, and capturing
        // the stage for the chat, which only an IR stage asks for.
        let click = stage_tabs::stage_tabs_ui(
            ui,
            &self.stages,
            &mut self.stage,
            self.compiling,
            self.viewing_log,
            self.sim_error.is_some(),
            self.sim_data.is_some(),
            can_sim,
            self.sim_running,
        );
        match click {
            // **▶ does not leave the log view, and that is deliberate.** Its hover has
            // always promised it "stays on the current view", so a run can be watched in
            // the log or read against the IR while it completes. Folding it in with the
            // tab clicks below would have changed what the button does while only moving
            // where it sits.
            Some(stage_tabs::TabClick::RunSimulation) => self.start_simulation(),
            Some(click) => {
                self.viewing_log = false;
                // Guarded on a specimen because the capture describes *this run's* stage;
                // with nothing loaded there is no stage to describe.
                if click == stage_tabs::TabClick::Stage && self.selected.is_some() {
                    intent.want_stage_ask = true;
                }
            }
            None => {}
        }
        // The compiled-model name is deliberately NOT shown here — the
        // stage tab row is short on horizontal space, and the same
        // identity is already visible in the specimen list and the
        // tree breadcrumb. `self.model` itself is still maintained;
        // it feeds the Claude bridge focus file, live-debug arming,
        // capture gating, and the purpose-note lookup.
        if self.compiling {
            ui.spinner();
        }
        if let Some(n) = &self.nav_loading {
            ui.weak(format!("opening {n}…"));
            ui.spinner();
        }
    }

    /// Apply a sub-view and a frame seek requested by an `hrw://` link.
    ///
    /// # Why this is its own method, called for EVERY stage
    ///
    /// Doug, 2026-08-16: *"Clicking on the frame 7 and frame 13 links is still not
    /// causing navigation."*
    ///
    /// This logic used to live inside
    /// [`report_sub_view_row_ui`](crate::report_sub_view::report_sub_view_row_ui), which its own
    /// doc comment describes as *"only ever reached when `report_ready` — the stage is
    /// a report stage **and** it produced a value"*. Report stages are **Structural**
    /// and **Index Reduction**, and nothing else.
    ///
    /// So for **Flatten, Events and Initialization**, `pending_sub_view` was set by the
    /// link and then never taken, and `apply_pending_seek` never ran at all. Every link
    /// naming a non-default sub-view of those stages, and every frame seek into them,
    /// silently did nothing — the seek budget expired over five paints and gave up
    /// without a notice, because expiry is the *expected* end of a seek whose animation
    /// is still building.
    ///
    /// **It looked like it worked, because the default is the common case.**
    /// `Flatten/EquationSheet` is where Flatten already opens, so a link to it appears
    /// to navigate. `Flatten/Connections` does not, and neither does any frame in it.
    ///
    /// # Ordering is the reason it is a separate call rather than moved earlier
    ///
    /// It must run **after** the report row's default-sub-view reset, which forces
    /// Summary when a report stage is entered singular — a link saying "show the
    /// matching animation" has to win over that. So the caller runs the report row
    /// first when there is one, then this, for all stages alike.
    fn apply_pending_view_and_seek(&mut self) {
        // Set when a link names a sub-view this model has no tab for. Collected
        // here and posted after the borrows end, as `FrameIntent` does.
        let mut bad_sub_view: Option<String> = None;

        if let Some(sub) = self.pending_sub_view.take() {
            // Refuse a sub-view this model does not have a tab for, rather than
            // selecting it and rendering something misleading — the same rule as
            // aiming at an equation that is not there. The link named a real
            // slug; whether it is *available* depends on what the compile
            // produced, which only this point knows.
            // **Every sub-view family answers now, and the question is asked of the stage
            // the app is ON.** This was `SubView::Structural(v) => …, _ => true` until
            // 2026-08-21, which said *"every non-report sub-view is always present"* —
            // false of Source Map and Connections, both of which exist only for some
            // models. Same wildcard shape `CLAUDE.md` records from the live-debug cluster.
            //
            // **The stage is matched here rather than left to `apply_sub_view`, and that
            // is not tidiness.** `HrwLink::LoadAndSwitch` sets `pending_sub_view` while the
            // compile is still in flight and `self.stage` is still the *previous* stage —
            // so a question about "this model's Flatten tabs" asked then is answered from
            // an empty compile, and a link to `Flatten/Connections` would be refused with a
            // notice naming a stage the app is not showing. `apply_sub_view` drops a
            // stage-mismatched request anyway, so passing it through is the honest answer:
            // **availability is a property of the stage on screen, and there is nothing to
            // report about the others.** The structural arm gained the same protection —
            // `structural_view_available` reads `self.stage` internally, so before this it
            // could refuse a Structural link while the app sat on Flatten.
            let available = match (sub, self.stage) {
                (SubView::Structural(v), StageKind::Structural | StageKind::IndexReduction) => {
                    self.structural_view_available(v)
                }
                (SubView::Flatten(v), StageKind::Flatten) => {
                    sub_view_rows::flatten_view_available(v, self.flatten_content())
                }
                (SubView::Events(v), StageKind::Events) => {
                    sub_view_rows::events_view_available(v, self.has_pre_lowering_trace())
                }
                (SubView::Init(v), StageKind::Initialization) => {
                    sub_view_rows::init_view_available(v, self.has_ic_plan())
                }
                // Stage mismatch — see above. `apply_sub_view` matches the same pairs and
                // ignores it, so this changes nothing except which message is *not* shown.
                _ => true,
            };
            if available {
                self.apply_sub_view(Some(sub));
            } else {
                bad_sub_view = Some(format!(
                    "{} has no {} view for this model \u{2014} the link names one \
                     that is not here",
                    self.stage.name(),
                    sub.slug(),
                ));
            }
        }
        // Only now is the sub-view settled, so only now does looking up "the
        // on-screen animation" mean the one the link named. Applying this
        // before the block above would seek whichever animation happened to be
        // showing beforehand.
        self.apply_pending_seek();
        if let Some(msg) = bad_sub_view.take() {
            self.notify(msg);
        }
    }

    /// The **lab panel**: the picker at the top, the lab's markdown below.
    ///
    /// Lifted out of `frame_ui` on 2026-08-02.
    ///
    /// Returns the `hrw://` link the reader clicked, if any. **Returned rather
    /// than dispatched**, because a lab link can load a specimen, change stage
    /// and move the camera — the panel has no business doing any of that, and
    /// `frame_ui` acts on it before the central panel renders so the whole frame
    /// sees one consistent state.
    ///
    /// Re-reads the lab file immediately after a pick rather than waiting for
    /// the poll: *"a click that appears to do nothing for a quarter second reads
    /// as a broken button."*
    fn lab_panel_ui(&mut self, ui: &mut egui::Ui) -> Option<HrwLink> {
        self.poll_lab_file();
        let lab_text = self.lab.text().map(str::to_owned);
        let lab_links = lab_text
            .as_deref()
            .map(extract_hrw_links)
            .unwrap_or_default();
        register_hrw_hooks(&mut self.commonmark_cache, &lab_links);
        let avail = ui.available_width();
        let mut switch_to: Option<LabSource> = None;
        let ctx = ui.ctx().clone();
        let shown = self
            .split
            .configure(&ctx, egui::Panel::left(LEFT_PANEL_ID), avail)
            .show(ui, |ui| {
                self.split.inner_width = Some(ui.available_width());
                // **The lab picker lives in the transport bar**, not in a section of
                // its own. Doug, 2026-08-16: the "Labs (23)" header and its divider
                // stopped making sense once the list became one combo box and one
                // button — a titled bar around two controls is chrome announcing
                // chrome. Both moved into `autoplay_controls_ui`, which already owns
                // a bar and already sits directly above the prose.
                switch_to = self.autoplay_controls_ui(ui, &lab_text);
                ui.separator();

                lab_panel::lab_prose_ui(ui, &mut self.lab, &mut self.commonmark_cache, &lab_text);
            });
        if let Some(msg) = self.split.observe(shown.response.rect.width(), avail) {
            self.log_split(msg);
        }
        if let Some(source) = switch_to {
            self.select_lab(source);
            // Re-read now rather than waiting up to a poll interval: a click that
            // appears to do nothing for a quarter second reads as a broken button.
            self.lab.polled_at = None;
            self.poll_lab_file();
        }
        drain_hrw_hooks(&mut self.commonmark_cache, &lab_links)
    }

    /// The **Specimen left panel** — the specimen list above, and beneath it either the
    /// Modelica source or the purpose note.
    ///
    /// Lifted out of `frame_ui` on 2026-08-21, the twin of [`Self::lab_panel_ui`] and
    /// the reason the seam was visible: the two mode panels are one two-member list, and
    /// one member had been a method call since 2026-08-02 while the other was a hundred
    /// and thirteen lines of body. See
    /// [`docs/app-split-plan.md`](../docs/app-split-plan.md).
    ///
    /// Returns the `hrw://` link the reader clicked in a purpose note, if any —
    /// returned rather than dispatched for [`Self::lab_panel_ui`]'s reason: a link can
    /// load a specimen and move the camera, and `frame_ui` acts on it before the central
    /// panel renders so the whole frame sees one consistent state.
    ///
    /// **The list's navigation stays here**, because only the caller knows what is
    /// already selected — the list cannot tell a re-click from a switch.
    fn specimen_panel_ui(&mut self, ui: &mut egui::Ui) -> Option<HrwLink> {
        let avail = ui.available_width();
        let ctx = ui.ctx().clone();
        let mut link_action: Option<HrwLink> = None;
        let shown = self
            .split
            .configure(&ctx, egui::Panel::left(LEFT_PANEL_ID), avail)
            .show(ui, |ui| {
                self.split.inner_width = Some(ui.available_width());
                let panel_height = ui.available_height();
                let list_height = panel_height * SPECIMEN_LIST_HEIGHT_FRACTION;

                // -- Top third: specimen list --
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), list_height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let sel = self.selected.clone();
                        let out = self.model_list.ui(
                            ui,
                            sel.as_deref(),
                            self.compiling,
                            self.model.is_some(),
                        );
                        match out.nav {
                            Some(ModelListNav::OpenLibrary(name)) => {
                                self.open_library_model(&name);
                            }
                            Some(ModelListNav::Reload(path)) => self.open(path),
                            // **A click on the specimen already loaded reveals it
                            // rather than recompiling.** The list cannot know that;
                            // only the caller knows what is selected.
                            Some(ModelListNav::Select(path)) => {
                                if self.selected.as_ref() == Some(&path) {
                                    self.viewing_log = false;
                                } else {
                                    self.open(path);
                                }
                            }
                            None => {}
                        }
                        if out.point_at_specimen {
                            self.emit_focus(Focus::Specimen);
                        }
                    },
                );

                ui.add_space(10.0);
                section_header_toggle(
                    ui,
                    &mut self.specimen_detail,
                    &[
                        (SpecimenDetail::Source, "Source"),
                        (SpecimenDetail::Purpose, "Purpose"),
                    ],
                );
                ui.add_space(4.0);

                // -- Bottom two-thirds: source or purpose --
                match self.specimen_detail {
                    SpecimenDetail::Source => self.specimen_source_ui(ui),
                    SpecimenDetail::Purpose => link_action = self.specimen_purpose_ui(ui),
                }
            });
        if let Some(msg) = self.split.observe(shown.response.rect.width(), avail) {
            self.log_split(msg);
        }
        link_action
    }

    /// The specimen's purpose note — see [`specimen_purpose::purpose_ui`], which holds
    /// the body.
    ///
    /// **Resolving the note and draining the link hooks stay here**, the same split
    /// [`Self::lab_panel_ui`] uses for the lab's prose: the pane renders a document
    /// and the caller decides what a click on it means.
    ///
    /// **The guard this replaced was dead, and saying so is the point.** The old code
    /// wrote `if hrw_link_action.is_none()`, defending against a lab link the same
    /// frame — but the lab and specimen panels are arms of a `ui_mode` comparison and
    /// cannot both run. A condition that can never be false reads as a real interaction
    /// between two panels; there is none.
    fn specimen_purpose_ui(&mut self, ui: &mut egui::Ui) -> Option<HrwLink> {
        let model = self.model.as_deref();
        let note = specimen_purpose::purpose_note(&mut self.cached_purpose_notes, model);
        // Empty when there is no note, and both hook calls are then no-op loops — so the
        // pane's two arms need no gate of their own here.
        let links = note.map(extract_hrw_links).unwrap_or_default();
        register_hrw_hooks(&mut self.commonmark_cache, &links);
        specimen_purpose::purpose_ui(
            ui,
            &mut self.commonmark_cache,
            note,
            model,
            self.selected.as_deref(),
        );
        drain_hrw_hooks(&mut self.commonmark_cache, &links)
    }

    /// The lab transport bar — see [`lab_panel::autoplay_controls_ui`], which
    /// holds the body and the rationale.
    ///
    /// **The presses stay here on purpose.** Back, Play and Stop each reach past the
    /// lab into the application — the lab file, the beat dispatcher, the UI mode a
    /// run borrowed — so the bar reports what was pressed and `App` performs it, the
    /// same split [`Self::specimen_source_ui`] uses for a clicked identifier.
    ///
    /// Returns a lab to switch to, because that one is the caller's own business:
    /// `lab_panel_ui` applies it after the panel has finished drawing.
    fn autoplay_controls_ui(
        &mut self,
        ui: &mut egui::Ui,
        lab_text: &Option<String>,
    ) -> Option<LabSource> {
        let request = lab_panel::autoplay_controls_ui(ui, &mut self.lab, self.compiling, lab_text)?;
        match request {
            TransportRequest::Switch(source) => return Some(source),
            TransportRequest::Back => self.lab_back(),
            TransportRequest::Play => self.start_autoplay(),
            TransportRequest::Stopped => self.restore_mode_after_autoplay(),
            TransportRequest::PointAtSelection => {
                // The lab's own label, so the capture names the document the passage
                // can be found in. `None` cannot happen while the panel is drawing a
                // lab, and is reported rather than assumed away.
                match self.lab.selected.as_ref().map(LabSource::label) {
                    Some(lab) => self.arm_lab_passage_capture(ui.ctx(), lab),
                    None => {
                        self.notify("\u{26a0} no lab is open, so there is no passage to point at")
                    }
                }
            }
        }
        None
    }

    /// The **Context Bar** — what Claude will actually have behind the next question.
    ///
    /// **What is left here is policy**, after the assembled state left for
    /// [`crate::context_bar`] on 2026-08-19: the empty-state branch (whose hint is
    /// built from view state this pane otherwise never touches), the one call that
    /// must run *before* the rows that report it, and the four presses the bar
    /// reports rather than performs.
    ///
    /// **`refresh_jump_matches` stays because position, not count, decides.** It is
    /// the only one of the seven `App` methods this function used to call that sits
    /// mid-body: the Following row reads `jump_matches` two lines later. The other
    /// four presses all sat *below* the last `ui` call, so performing them here
    /// costs no frame at all.
    fn context_bar_ui(&mut self, ui: &mut egui::Ui) {
        // The top half of the bar's margin. The bottom half is added by whichever
        // branch below finishes, immediately before that branch's separator.
        ui.add_space(context_bar::BAR_MARGIN);
        let has_point = self.context.pointed_at.is_some();
        let has_thread = self.tracked_identifier.is_some();
        if !has_point && !has_thread {
            // Say the empty state rather than vanishing.
            //
            // Hiding made "nothing is assembled" indistinguishable from "the bar
            // is not rendering", which is absence by *implication* — the thing
            // this design eliminates everywhere else (`mentions: 0`,
            // `kind: "none"`, "not declared in this specimen"). It is also the
            // state a user is in just before asking a question that quietly has
            // nothing behind it.
            //
            // **This branch now runs with no specimen too** (Doug, 2026-08-30), which
            // it never did before: `central_panel_ui` used to return ahead of the bar
            // whenever `selected` was `None`.
            //
            // **The stage is therefore conditional, and that is an accuracy point
            // rather than a tidy one.** `self.stage` always holds *some* variant, so
            // passing it unconditionally would print "· Parse" on a fresh launch —
            // naming a phase that has not run, about a specimen that does not exist.
            // Absence is stated, never filled.
            // **The gesture hint moved into the hover on 2026-08-30.** Doug, seeing it
            // beside the background: *"this message is appended to the content in the
            // context bar: '— open a stage tab to inspect its IR'. Please remove that
            // message."* It is advice, and the bar's line is a report of what Claude
            // has; mixing the two is what made the row hard to read.
            //
            // **Moved rather than deleted, because it was itself a fix.** The first
            // version was generic and named a right-click the source view does not
            // have — advice the reader could not take, which is the bug
            // `the_empty_context_hint_names_only_gestures_that_work` still guards. It
            // is now on hover, where it costs nothing until wanted.
            //
            // **And what it disambiguated is no longer at risk.** Half its job was
            // making "nothing is assembled" distinguishable from "the bar is not
            // rendering" — which the bar being unconditional now settles by itself.
            let hint = self.empty_context_hint();
            let lab = self.lab.selected.as_ref().map(LabSource::label);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Context").strong())
                    .on_hover_text(format!("{EMPTY_CONTEXT_RULE}\n\nRight now: {hint}"));
            });
            context_bar::always_ui(
                ui,
                self.model.as_deref(),
                self.selected.is_some().then_some(self.stage),
                lab.as_deref(),
                context_bar::stage_ir_count(&self.stages),
                self.def_index.len(),
            );
            ui.add_space(context_bar::BAR_MARGIN);
            ui.separator();
            return;
        }

        // The match list has to be current before the row that reports it.
        self.refresh_jump_matches();

        let lab = self.lab.selected.as_ref().map(LabSource::label);
        let press = context_bar::context_bar_ui(
            ui,
            &self.context,
            &self.tracked_identifier,
            self.stage,
            &self.stages,
            &self.identifier_index,
            &self.declaring_classes,
            &self.def_index,
            self.model.as_deref(),
            lab.as_deref(),
        );

        match press {
            Some(ContextBarPress::Jump { forward }) => self.jump_to_next_match(forward),
            Some(ContextBarPress::ClearPoint) => {
                self.context.pointed_at = None;
                // A stale failure would otherwise keep warning about an emission
                // for a point that no longer exists.
                self.context.point_error = None;
                // Clearing is a context change like any other, so it advances the
                // shared counter and re-emits. Emitting matters more here than
                // anywhere: the file still holds the old node until it is rewritten,
                // and a bar showing no point over a file holding one is exactly the
                // disagreement this design exists to prevent.
                self.context.context_seq = self.context.next_seq();
                self.emit_context();
            }
            Some(ContextBarPress::ClearThread) => {
                self.tracked_identifier = None;
                self.context.track_seq = self.context.next_seq();
                self.emit_context();
            }
            Some(ContextBarPress::GoToClass(class)) => self.navigate_to(class),
            None => {}
        }
    }

    /// Toggle the tracked identifier — the single entry point for tracking,
    /// whichever view the click came from.
    ///
    /// Clicking the already-tracked name clears it, so every view untracks the
    /// same way the source view does. Derivative mentions like `der(h)` are
    /// reduced to the base variable, since that is what `IdentifierIndex` and
    /// the source declaration are keyed by (idea #37's "wrinkle").
    fn set_tracked_identifier(&mut self, name: String) {
        let name = crate::identifier_index::strip_der(&name).to_owned();
        // The action most likely to be the last one before a crash: it walks
        // every stage's IR and lexes every code-bearing string in it, which is
        // exactly how the 2026-07-28 em-dash panic was reached.
        diagnostics::record_action(
            "follow",
            if self.tracked_identifier.as_deref() == Some(name.as_str()) {
                format!("stop following {name}")
            } else {
                format!("follow {name} (in {})", self.stage.name())
            },
        );
        if self.tracked_identifier.as_deref() == Some(name.as_str()) {
            self.tracked_identifier = None;
        } else {
            self.tracked_identifier = Some(name);
        }
        // Recency, not identity: the counter advances on *any* change,
        // including clearing, so the emitted context can say which half of it
        // the user touched most recently.
        self.context.track_seq = self.context.next_seq();
        // Following is context, so changing it changes what Claude has. Emit
        // now rather than waiting for the next capture, or the Context Bar
        // would show a thread that had never been sent.
        self.emit_context();
    }

    /// Delegates to [`crate::source_map::source_map_ui`], which names the four fields
    /// this view may touch instead of taking all of `App`.
    fn source_map_ui(&mut self, ui: &mut egui::Ui) {
        crate::source_map::source_map_ui(
            ui,
            &self.cached_equation_sheet,
            &self.identifier_index,
            &self.tracked_identifier,
            &mut self.viewport,
        );
    }
}

impl eframe::App for App {
    /// The main frame function. Called ~60 times/second (or on repaint
    /// request). Every panel, button, label, and tree node is built here.
    ///
    /// The overall layout is:
    /// - **Top**: menu bar (File, Help)
    /// - **Bottom**: status bar (bridge capture feedback)
    /// - **Left panel**: specimen file list
    /// - **Right panel**: field help / specimen info / simulation help
    /// - **Center**: the main content area (stage tabs + IR tree/spy-plot/simulation)
    ///
    /// egui panels claim space in the order they're added: top/bottom first,
    /// then left/right, and `CentralPanel` fills whatever remains. This is
    /// why the panels appear in this specific order in the code.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame_ui(ui);
    }

    /// Release the live-trace breakpoint on the way out.
    ///
    /// **Why this exists** (`docs/tech-debt.md`, "HRW never clears its live-trace
    /// breakpoint on shutdown"). All the in-app removal sites are *reactive* — a
    /// live session ending, a stage changing, a new Debug click — so quitting with
    /// one armed left it registered in VS Code. The orphan is a breakpoint the
    /// user never set, sitting in `live_trace.rs`, and it stops **any** later
    /// debug session that reaches that code. A stop nobody remembers arming is a
    /// confusing signal, which this project treats as teaching something false.
    ///
    /// **This is not a guarantee and is not meant to read as one.** A debugger
    /// stop, a panic or a kill runs no destructor, so only the extension side
    /// could ever promise it — `HRW: Clear Armed Breakpoints` remains the manual
    /// remedy. What this closes is the ordinary case: quitting the app.
    ///
    /// **It also cannot clear an orphan HRW never tracked.** Since `#71`,
    /// `live_breakpoint_armed` is true only when the bridge acked, so a *late* ack
    /// leaves a breakpoint this method will not see. That was the accepted trade —
    /// HRW must not claim state it cannot see — and it stays accepted here rather
    /// than being quietly undone by removing unconditionally.
    fn on_exit(&mut self) {
        self.release_live_breakpoint_at_exit();
    }
}

impl App {
    /// The whole frame, minus the `eframe::Frame` the trait requires and this app
    /// never used.
    ///
    /// **Split out 2026-08-01 so the UI can be driven headlessly.**
    /// `eframe::Frame` cannot be constructed outside eframe, so a test harness
    /// could not call [`eframe::App::ui`] at all — one unused parameter was the
    /// only thing standing between ~12,000 lines of UI and an automated test.
    /// The parameter was already `_frame`; nothing is lost by not passing it.
    ///
    /// Everything below is unchanged and runs in the same order. See
    /// `docs/verification-plan.md` item 2.
    pub(crate) fn frame_ui(&mut self, ui: &mut egui::Ui) {
        // Before anything draws: collect a lab-passage copy that egui produced for us
        // on a previous frame. See `PendingPassage`.
        self.collect_pending_passage();

        // First thing every frame: check for results from the worker thread.
        self.drain_worker();

        // Publish the state a crash file would need. After `drain_worker`, so a
        // crash later in this frame reports the state the frame actually
        // rendered rather than the previous one's. See `diagnostics.rs`.
        diagnostics::set_snapshot(self.diagnostic_snapshot());

        // One-shot: force the debugger to resolve live_trace.rs early, so the
        // first Debug click does not pay for it. No-op after the first few
        // frames. See `Prewarm`.
        self.tick_prewarm(ui.ctx());

        // A self-running lab advances here, after `drain_worker` so `compiling`
        // reflects this frame rather than the last one — the clock must see the
        // compile finish on the frame it finishes.
        self.tick_autoplay(ui.ctx());

        // ---- Top menu bar ----
        self.menu_bar_ui(ui);

        self.floating_windows(ui);

        // ---- Bottom status bar ----
        // Added BEFORE the side panels so it spans the full window width (egui
        // panels claim space in the order they're created — a bottom panel added
        // after left/right panels would only fill the remaining center).
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.add_space(1.0);
            // Success is silent. The Context Bar states what Claude has; this
            // line is for things that happen and then are over.
            match &self.notice {
                // **Not `weak`.** A notice is something that just happened — often a
                // refusal — and rendering it in the same de-emphasised grey as the idle
                // hint made it read as background chrome. Doug clicked a link that was
                // correctly refused, with the reason on screen, and reported that
                // nothing happened (2026-07-30).
                Some(s) => ui.label(s),
                None => ui.weak(
                    "Left-click a tree node to point at it, then ask about it in the chat                      (right-click to follow an identifier, or for more actions).",
                ),
            };
            ui.add_space(1.0);
        });

        // ---- Left panel: content depends on UI mode ----
        // Lab and Specimen modes show a left panel (lab text or specimen list
        // + purpose). Debug mode hides it so the stage tabs fill HRW's window
        // (VS Code occupies the left half of the screen).
        let mut hrw_link_action: Option<HrwLink> = None;

        if self.ui_mode == UiMode::Lab {
            hrw_link_action = self.lab_panel_ui(ui);
        }
        if self.ui_mode == UiMode::Specimen {
            hrw_link_action = self.specimen_panel_ui(ui);
        }

        // **The reset is consumed here, after both panels have had their chance
        // at it.** Clearing it inside either one would leave the other still
        // holding a dragged width on the frame a mode switch happens.
        self.split.end_frame();

        // ---- Dispatch hrw:// link actions ----
        if let Some(action) = hrw_link_action {
            self.dispatch_hrw_link(action);
        }

        // ---- Center panel: stage tabs + main content ----
        //
        // More "deferred action" variables: the CentralPanel closure borrows
        // `self` through the panel, so we can't call methods like
        // `emit_node_focus` inside it. Instead, the closure sets these flags
        // and the actual method calls happen after the closure, below.
        // Everything the panels want done, collected in one place. See
        // `FrameIntent` for why this is a struct rather than seven locals.
        //
        // `hrw_link_action` is deliberately *not* in here: the left panel
        // collects it and it is acted on immediately above, before the central
        // panel renders, so it is not deferred state.
        let mut intent = FrameIntent::default();

        // `CentralPanel` fills all remaining space after top/bottom/left/right
        // panels have claimed theirs. This is where the main content lives.
        // The central panel's body is `central_panel_ui`. It was extracted
        // 2026-07-29 once `FrameIntent` made it possible: it needs only
        // `&mut self` and one `&mut FrameIntent`, where before it would have
        // required seven transposable out-parameters.
        egui::CentralPanel::default().show(ui, |ui| {
            self.central_panel_ui(ui, &mut intent);
        });

        // ---- Deferred actions ----
        //
        // All the flags/options collected during the CentralPanel closure are
        // now acted on. The panel closure has ended, so `self` is no longer
        // borrowed and we can call methods freely. This is the payoff of the
        // "collect intent, act later" pattern used throughout this function.
        let FrameIntent {
            tree: tree_actions,
            canvas_capture,
            want_stage_ask,
            go_back,
            go_home,
        } = intent;

        if go_home {
            self.nav.clear();
        } else if go_back {
            self.nav.pop();
        }
        // The tree is the only producer of these three; earlier revisions
        // declared parallel locals for other views to write into, but nothing
        // ever did, so `nav_to.or(tree_actions.nav_to)` was always the latter.
        let debug_ask = tree_actions.debug;
        let mut node_ask = tree_actions.capture;
        if let Some(name) = tree_actions.nav_to {
            self.navigate_to(name);
        }
        // Reverse tracking from any stage (idea #37). Reveals the source, the
        // same as tracking from the equation sheet — the point is seeing where
        // the identifier came from.
        if let Some(name) = tree_actions.track {
            self.set_tracked_identifier(name);
            if self.tracked_identifier.is_some() {
                self.ui_mode = UiMode::Specimen;
                self.split.request_reset(MODE_SWITCH_RESET);
                self.specimen_detail = SpecimenDetail::Source;
            }
        }
        // **"Show in the Modelica source" — one hop to the declaration.**
        //
        // Dispatched through the existing `ShowSource` verb rather than reimplemented:
        // that link already switches to Specimen mode, opens the Source detail, leaves
        // the log view and scrolls. A second copy of those four steps would be four
        // chances for the menu and the lab link to drift apart.
        if let Some(line) = tree_actions.show_source_line {
            self.dispatch_hrw_link(HrwLink::ShowSource(Some(line)));
        }
        // A spy-plot block click is treated identically to a tree-node click
        // for capture purposes.
        if canvas_capture.is_some() {
            node_ask = canvas_capture;
        }
        // Priority: debugger capture > explain capture > stage capture.
        // Only one bridge write per frame.
        if let Some(key_path) = debug_ask {
            self.emit_node_focus(key_path, bridge::AskRequest::DebugWhereSet);
        } else if let Some(key_path) = node_ask {
            self.emit_node_focus(key_path, bridge::AskRequest::Explain);
        } else if want_stage_ask {
            self.emit_focus(Focus::Stage);
        }

        // The jump lasts exactly one frame. It has now been rendered: the target
        // row called `scroll_to_me`, and forcing its ancestors open *stored*
        // that state in egui, so they stay open on their own from here. Holding
        // it any longer would re-scroll every frame — pinning the view the way
        // the source view's reverse-tracking scroll did before it was gated on
        // change — and would keep those headers forced open, which is the
        // "Reveal identifiers" complaint all over again — which is literally
        // what removed that checkbox on 2026-08-04.
        self.context.jump_target = None;

        // ---- End of frame: publish diagnostics ----
        //
        // Refresh the snapshot and only then write `session.json`, so its `app` block
        // describes the state the frame's actions *produced*. The top-of-frame snapshot
        // is still set (a crash mid-frame reports where the user was), but it is the
        // wrong thing to pair with an action that just fired: on 2026-07-30 that
        // combination made Claude report three phantom bugs from correct values.
        //
        // `flush_session` is a no-op unless an action was recorded, so an idle frame
        // costs one bool check.
        // **End of frame, so the sub-view has settled**, and before the snapshot below
        // so `view_published` describes the file that now exists rather than the
        // previous one.
        //
        // Per frame rather than at a choke point, deliberately: the first attempt put
        // this beside the `pending_sub_view` handling, which lives in
        // `report_sub_view_row_ui` and therefore **only runs on report stages** — so
        // Flatten, the very pane it was built for, published nothing. Caught by
        // `ui_tests::a_rendered_frame_publishes_the_current_view` rather than by
        // reading. The cost of running it every frame is one string compare, because
        // `publish_current_view` returns early when nothing moved.
        self.publish_current_view();
        diagnostics::set_snapshot(self.diagnostic_snapshot());
        diagnostics::flush_session();
    }
}

/// Read a specimen's one-line purpose hint — the first `// purpose:` comment in
/// the file (the phenomenon it's authored to exercise). Scanned without compiling
/// so every file in the list gets a hint, even one that fails to compile. `None`
/// if the convention is absent.
///
/// The convention: each specimen `.mo` file contains a comment like:
/// ```text
/// // purpose: high-index, structurally singular DAE
/// ```
/// This is scanned at **rescan time** (when the specimen directory is read),
/// NOT at compile time. This is deliberate — it means every specimen in the
/// list gets its purpose hint shown in the tooltip even if the specimen fails
/// to compile. The purpose comment describes the *compiler feature* the
/// specimen exercises, which is different from the Modelica `annotation`
/// description string (which describes the *model* itself).
pub(crate) fn read_purpose(path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    text.lines().find_map(|line| {
        line.trim_start()
            .strip_prefix("// purpose:")
            .map(str::trim)
            .filter(|hint| !hint.is_empty())
            .map(str::to_owned)
    })
}

/// A blue-tinted header bar for left-panel sections (Lab, Specimens, Purpose).
/// Uses a navy background with light-blue text in dark mode, matching the RHS
/// stage-tab palette for visual consistency.
pub(crate) struct SectionStyle {
    pub(crate) active_color: egui::Color32,
    pub(crate) inactive_color: egui::Color32,
    pub(crate) frame: egui::Frame,
}

pub(crate) fn section_style(ui: &egui::Ui) -> SectionStyle {
    let dark = ui.visuals().dark_mode;
    let bg = if dark {
        egui::Color32::from_rgb(0x1A, 0x2A, 0x40)
    } else {
        egui::Color32::from_rgb(0xD8, 0xE8, 0xF8)
    };
    let active_color = if dark {
        egui::Color32::from_rgb(0x8A, 0xC4, 0xFF)
    } else {
        egui::Color32::from_rgb(0x0A, 0x5C, 0xC4)
    };
    let inactive_color = if dark {
        egui::Color32::from_rgb(0x50, 0x70, 0x90)
    } else {
        egui::Color32::from_rgb(0x60, 0x90, 0xC0)
    };
    let h_margin = ui.spacing().item_spacing.x;
    let frame = egui::Frame::new()
        .fill(bg)
        .inner_margin(egui::Margin::symmetric(6, 4))
        .outer_margin(egui::Margin {
            left: -h_margin as i8,
            right: -h_margin as i8,
            top: 2,
            bottom: 0,
        });
    SectionStyle {
        active_color,
        inactive_color,
        frame,
    }
}

pub(crate) fn section_header(ui: &mut egui::Ui, title: &str) {
    let style = section_style(ui);
    style.frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(13.0)
                .color(style.active_color),
        );
    });
}

/// A section header bar with clickable toggle options (e.g. "Source | Purpose").
/// The active option is shown in bright text; inactive options are dimmed and clickable.
fn section_header_toggle<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    current: &mut T,
    options: &[(T, &str)],
) {
    let style = section_style(ui);
    style.frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            for (i, (value, label)) in options.iter().enumerate() {
                if i > 0 {
                    ui.label(
                        egui::RichText::new("|")
                            .size(13.0)
                            .color(style.inactive_color),
                    );
                }
                let is_active = *current == *value;
                let color = if is_active {
                    style.active_color
                } else {
                    style.inactive_color
                };
                let text = if is_active {
                    egui::RichText::new(*label).strong().size(13.0).color(color)
                } else {
                    egui::RichText::new(*label).size(13.0).color(color)
                };
                if ui
                    .add(egui::Label::new(text).sense(egui::Sense::click()))
                    .clicked()
                {
                    *current = *value;
                }
            }
        });
    });
}

/// Everything the frame's panels want done, acted on after they close.
///
/// egui panel bodies borrow `self`, so a click cannot mutate app state where it
/// happens — it records what the user asked for and [`App::ui`] acts once the
/// borrows end. Before this struct existed those were seven separate locals,
/// which is what blocked extracting any panel body into a method: each
/// extraction would have needed all eight threaded through as transposable
/// out-parameters. One `&mut FrameIntent` instead.
///
/// Same pattern as `tree::TreeActions`, which bundled the tree's out-parameters
/// for the same reason; that type is nested here rather than flattened, because
/// the tree owns which of its actions exist.
#[derive(Default)]
struct FrameIntent {
    /// What the IR tree wants: capture, debug-capture, navigate, track.
    tree: tree::TreeActions,
    /// A spy-plot or incidence block was clicked — treated identically to a
    /// tree-node click for capture purposes.
    ///
    /// **The line above this one used to be an orphan**, and it is worth a sentence
    /// because it is a *fifth* cause of the doc-comment trap: `71d0dcbf` (2026-08-04)
    /// deleted the `expand_trackable` field and the *second* line of its two-line doc,
    /// so *"Copied out of `self` because the stage-tree block holds an immutable"* — a
    /// sentence that does not even finish — was merged into this field's doc and read as
    /// its opening summary for sixteen days.
    canvas_capture: Option<Vec<Seg>>,
    /// A stage tab was clicked, so the capture should describe the stage.
    want_stage_ask: bool,
    /// Navigation "Back".
    go_back: bool,
    /// Navigation "Specimen" — clears the whole nav stack.
    go_home: bool,
}

/// A sub-view within a stage, addressable by an `hrw://` link.
///
/// **The slugs are exactly the names the capture emits** (`structural_view_name`,
/// `flatten_view_name`, `events_view_name`, `init_view_name`), which is #42's design
/// principle applied: `hrw://` should express any noun `focus.json` can describe, so
/// the two directions share one vocabulary rather than inventing a second.
///
/// Added 2026-07-29 to close a lab hole. Links reached a stage tab and no further,
/// so every animation and custom view — all of them one level below a stage — had to
/// be handed off in prose ("same tab → now click **Incidence**"). The first lab had
/// two working links and four such hand-offs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubView {
    Structural(StructuralView),
    Flatten(FlattenView),
    Events(EventsView),
    Init(InitView),
}

impl SubView {
    /// The slug this sub-view is written as in an `hrw://` link.
    ///
    /// **The canonical inverse of [`Self::from_slug`]**, and it dispatches to the very
    /// functions the *capture* uses (`structural_view_name` and friends) rather than
    /// repeating the strings. That is what makes "the link vocabulary equals the capture
    /// vocabulary" true by construction instead of by assertion.
    ///
    /// Added 2026-07-30. `from_slug` had existed without an inverse — the same gap that
    /// let the stage vocabulary drift into four hand-written copies, one of which
    /// disagreed with the other three.
    fn slug(&self) -> &'static str {
        match self {
            Self::Structural(v) => structural_view_name(*v),
            Self::Flatten(v) => flatten_view_name(*v),
            Self::Events(v) => events_view_name(*v),
            Self::Init(v) => init_view_name(*v),
        }
    }

    /// Resolve a slug against the stage it appears under.
    ///
    /// Stage-scoped rather than global because the same slug means different things
    /// in different stages — `Tree` exists under four of them — and because a link
    /// naming a sub-view the stage does not have should fail to parse rather than
    /// navigate somewhere surprising.
    fn from_slug(stage: StageKind, slug: &str) -> Option<Self> {
        Some(match stage {
            StageKind::Structural | StageKind::IndexReduction => Self::Structural(match slug {
                "Summary" => StructuralView::Summary,
                "SpyPlot" => StructuralView::SpyPlot,
                "Incidence" => StructuralView::Incidence,
                "MatchingAnim" => StructuralView::MatchingAnim,
                "TarjanAnim" => StructuralView::TarjanAnim,
                "TearingAnim" => StructuralView::TearingAnim,
                "AliasAnim" => StructuralView::AliasAnim,
                "Animate" => StructuralView::Animate,
                "Tree" => StructuralView::Tree,
                _ => return None,
            }),
            StageKind::Flatten => Self::Flatten(match slug {
                "EquationSheet" => FlattenView::Equations,
                "SourceMap" => FlattenView::SourceMap,
                "Connections" => FlattenView::Connections,
                "Tree" => FlattenView::Tree,
                _ => return None,
            }),
            StageKind::Events => Self::Events(match slug {
                "PreLowering" => EventsView::PreLowering,
                "Tree" => EventsView::Tree,
                _ => return None,
            }),
            StageKind::Initialization => Self::Init(match slug {
                "IcPlan" => InitView::IcPlan,
                "Tree" => InitView::Tree,
                _ => return None,
            }),
            // Stages with no sub-views: a link naming one is malformed.
            _ => return None,
        })
    }
}

/// Navigation action parsed from an `hrw://` URI in lab or narrative markdown.
#[derive(Debug, PartialEq, Eq)]
enum HrwLink {
    /// `hrw://lab/<name>[/station/<slug>]` — open a **fixture lab**, optionally at a
    /// named stop.
    ///
    /// **The verb that lets one lab cite another.** Added 2026-08-05 for
    /// `docs/ideas.md` #63: Claude's answering repertoire was text, then a freshly
    /// written ad hoc lab, with no way to say *"the answer already exists — walk
    /// `failure-typecheck` from stop 2."* Ten link forms existed and none opened a
    /// lab, so a composed answer could only *describe* a fixture in prose. The
    /// expectations are the thing being lost by that: a fixture's `**Expected:**`
    /// lines are versioned and were checked, while a retelling has whatever Claude
    /// remembers of them.
    ///
    /// **Stops are addressed by SLUG, not by ordinal**, and that is the whole design
    /// decision. `stop/2` is fragile in the way this project has been bitten by twice
    /// already: inserting a stop shifts every later citation **silently**, exactly as
    /// a source line number does (`docs/tech-debt.md`, the `worker.rs:3434` citation
    /// that rotted inside a day). A slug derived from the heading text fails **loudly**
    /// when the heading is renamed, and is immune to insertion — which is the
    /// behaviour the link checker can act on.
    OpenLab { lab: String, stop: Option<String> },
    /// `hrw://breakpoint/<anchor>` — **arm a source breakpoint the reader would
    /// otherwise set by hand.**
    ///
    /// Doug, 2026-08-08, walking `matching-live.md`: *"Having to manually set
    /// breakpoints is friction. I'd like to instead click on links to set
    /// breakpoints."* A live lab cannot avoid breakpoints — they are the
    /// instrument — but it can stop making the reader transcribe a line number
    /// into a gutter while holding the lab in their other hand.
    ///
    /// **The anchor is named, and the line is resolved at click time** from
    /// [`crate::matching_ledger::anchor_by_name`], which locates it by what the
    /// line *says*. This is `OpenLab`'s slug decision applied again: a number
    /// in prose rots silently, a name does not. Here the link cannot even go
    /// stale, because nothing about the line is stored in it.
    ///
    /// **A name that resolves to no anchor does not parse**, so
    /// `fixture_lab_links_all_resolve` fails on a typo or on an anchor whose
    /// locating fragment was edited away — the lab is checked at test time
    /// rather than discovered broken mid-walk.
    ///
    /// **Add-only** (`bridge::arm_source_breakpoint`): `docs/ideas.md` #74 makes
    /// removal a one-way door, so a toggle would break the next click.
    ArmBreakpoint(String),
    /// `hrw://load/<Specimen>` — load and compile a specimen by name.
    LoadSpecimen(String),
    /// `hrw://stage/<Stage>[/<SubView>]` — switch to a stage tab, optionally to a
    /// sub-view within it (specimen already loaded).
    SwitchStage(StageKind, Option<SubView>),
    /// `hrw://load/<Specimen>/<Stage>[/<SubView>]` — load a specimen, then switch.
    LoadAndSwitch(String, StageKind, Option<SubView>),
    /// `hrw://source[/<line>]` — show the Modelica source, optionally scrolled to a
    /// 1-based line.
    ///
    /// Added 2026-07-29 to close a lab hole: two labs had to *quote* a source line
    /// ("reported at line 9, `connect(src.n, gnd.p);`") because nothing could point at
    /// one. Quoting is a prose workaround, which is the quiet-hole species that
    /// accumulates unnoticed — see the lab-holes table in `docs/tech-debt.md`.
    ShowSource(Option<u32>),
    /// `hrw://stage/<Stage>/<SubView>/equation/<n>` — go to a canvas view and **aim
    /// the camera** at equation `n`, so a stop can say "watch this equation" and put it
    /// in front of the reader.
    ///
    /// **0-based, unlike `frame`** — deliberately. Each verb matches how *its* thing is
    /// displayed: equations appear as `f_x[46]` counting from zero, frames appear as
    /// "Frame 3/11" counting from one. Making the two verbs uniform would force one of
    /// them to disagree with the screen. Agreeing with the display is the rule; agreeing
    /// with each other is not.
    ///
    /// **The noun is `equation`, not `node`.** Tarjan's view draws equations as graph
    /// nodes, so "node" was the tempting word — but the matrix views index the same
    /// thing as a row, and the IR calls it `f_x[n]`. A verb shared by four views needs
    /// the vocabulary they share, or the capture and the link drift apart exactly as
    /// the stage slug did.
    ///
    /// Added 2026-07-29. Until then a lab could name a *view* but not a place inside
    /// it, which is where a canvas view's content actually lives: `ideas.md` #42 listed
    /// camera aiming as the biggest missing capability for canvas-backed stops.
    ///
    /// Only meaningful for the canvas-backed views. A text or grid view has no camera,
    /// so the link still navigates and the aim is simply ignored rather than failing
    /// the stop — a lab degrading to "the right view, not aimed" beats one that
    /// silently does nothing.
    AimAtEquation(StageKind, SubView, usize),
    /// `hrw://stage/<Stage>/<SubView>/frame/<n>` — go to an animated view and **stop on
    /// frame `n`**, so a lab can point at the moment a decision is made rather than at
    /// the view containing it.
    ///
    /// The moment is where a replay's content lives: "the algorithm gives up here",
    /// "this is the tear that cascades". Naming only the view leaves the reader to find
    /// it by scrubbing.
    ///
    /// **Recorded playback only.** A live session has no meaningful frame count until
    /// it ends — that was a real bug Doug caught — so a frame index cannot mean anything
    /// stable there. Seeking a live view is refused like any out-of-range frame.
    SeekFrame(StageKind, SubView, usize),
    /// `hrw://stage/<Stage>/<SubView>/node/<path>` — open a stage tree and **point at a
    /// node**, expanding its ancestors and scrolling it into view.
    ///
    /// `<path>` is exactly what a capture writes: `error.unmatched_unknowns[0]`. See
    /// `bridge::parse_path`, the documented inverse of `bridge::describe_path`.
    ///
    /// This was the last and largest parity gap. A node path is the capture's **richest
    /// noun** — it is what a left-click produces, and what most of Doug's questions have
    /// been about — and until 2026-07-29 a lab could open a tree but not point into it.
    ///
    /// **The sub-view is optional, and omitting it is the only form that works for five
    /// stages.** Parse, Resolve, Instantiate, Typecheck and DAE render one generic tree
    /// and have no `SubView` variants at all, so a four-segment
    /// `stage/<Stage>/<SubView>/node/<path>` cannot name a node in any of them.
    /// `SwitchStage` had carried an `Option<SubView>` since it was written; this one did
    /// not, and the asymmetry meant **the richest noun was unavailable on the stages with
    /// the least else to point at.**
    ///
    /// Found 2026-08-03 while rewriting `docs/fixture-labs/dae-construction.md` against
    /// the new DAE tab: every `hrw://stage/Dae/Tree/node/…` in it failed to parse, and
    /// `fixture_lab_links_all_resolve` said so before the lab was ever walked — the
    /// case the link checker exists for.
    PointAtNode(StageKind, Option<SubView>, Vec<Seg>),
    /// `hrw://follow/<name>` — follow an identifier, as a right-click Follow would.
    ///
    /// The other half of the composition primitives: the capture has always carried a
    /// `tracking` section, and no link could set one. A stop can now say "follow
    /// `emf.phi` and watch where it goes", which is the gesture the cross-stage view was
    /// built for.
    Follow(String),
    /// `hrw://notebook/<name.nb>` — open a Wolfram notebook in Wolfram Desktop.
    ///
    /// The cross-platform labs route through a notebook, and a plain markdown link to
    /// one is handed to the *browser* — which does nothing useful with a `.nb`. Doug hit
    /// exactly that on the first cross-platform lab (2026-07-30). A lab should drive
    /// the reader to the stop, not tell him to go and find the file.
    ///
    /// The name is resolved by `bridge::resolve_notebook`, which restricts it to a file
    /// name in one of two known directories.
    OpenNotebook(String),
    /// `hrw://doc/<name.md>` — open a repository document **in VS Code**.
    ///
    /// Doug, 2026-08-31: *"the link for upstream-issues.md causes an attempt to open the
    /// file in Chrome instead of attempting to open the file in VS Code."* Five labs
    /// carried `[upstream-issues.md](../upstream-issues.md)`, and a relative markdown
    /// link is handed to the OS as a URL.
    ///
    /// **`open_with_os` would not have fixed it**, which is why this verb does not reuse
    /// it: the OS association for `.md` is whatever the machine says, and on this one it
    /// is Chrome. A prose document opened in a browser is not wrong so much as useless —
    /// it renders raw markdown and cannot be edited, and editing is what he is there for.
    /// So this spawns `code` explicitly.
    OpenDoc(String),
    /// `hrw://src/<path>[#<symbol>]` — open a workspace source file **in VS Code**, at
    /// the symbol's line when one is named.
    ///
    /// # The verb the code-grounding agreement needs
    ///
    /// Doug, 2026-08-31, pointing at *"`connections/mod.rs` uses union-find"*: *"This
    /// reference and others like it would be much more helpful as links to the code files
    /// in VS Code."* The day's earlier agreement was that lab claims are **grounded in
    /// Rumoca's code** rather than abstractly mathematical — and the moment prose started
    /// naming `connect_primitive_vars` and `generate_equality_equations`, every one of
    /// those names became somewhere he wants to go. A name he cannot reach is a
    /// citation; a name he can click is the source.
    ///
    /// **The link carries a symbol, never a line.** `bridge::resolve_source` computes the
    /// line from the file at click time, so the link cannot rot the way
    /// `docs/tech-debt.md`'s `worker.rs:3434` citation did *inside a day*. Same decision
    /// as `ArmBreakpoint` and `OpenLab`'s slugs, for the same reason.
    ///
    /// **And it is checked**, which is the half that makes grounding pay: a symbol
    /// renamed out of the source fails `doc_citations::lab_source_links_resolve` in the
    /// FAST suite, so the lab breaks in a test rather than under Doug's cursor. That is
    /// the mechanical return promised when the agreement was recorded — a claim naming
    /// `generate_equality_equations` can be wired into the gate, while a claim about
    /// "graphs" never could.
    OpenSource(String),
    /// `hrw://systemmodeler/<Specimen>` — open a specimen in Wolfram System Modeler.
    ///
    /// **The adjudicator verb.** System Modeler is an independent Modelica
    /// implementation, so "SM rejects this model that Rumoca accepts" is the strongest
    /// claim a lab can make — see `docs/upstream-issues.md` #2, which exists because of
    /// exactly that comparison.
    ///
    /// No new mechanism: the System Modeler installer already associates `.mo` with
    /// `ModelCenter.exe` (verified 2026-07-30), so this is the same OS hand-off that
    /// opens a notebook. HRW never learns where System Modeler lives.
    OpenInSystemModeler(String),
}

impl HrwLink {
    /// Whether this link needs a specimen already loaded.
    ///
    /// Doug clicked a lab's *fourth* stop first, without the first three, and nothing
    /// happened. With no specimen the whole stage area returns early, so a stage link
    /// set state that nothing consumed — silently, which is the failure mode every other
    /// verb has been taught to avoid.
    ///
    /// The ones that do **not** need a specimen are the ones that make sense on their
    /// own, and they are the list below rather than a count in this sentence. It said
    /// *"the three"* until 2026-08-23, by which time the list held **six** — the two
    /// load verbs and a notebook, plus System Modeler, opening a lab and arming a
    /// breakpoint, each added with its own reason and none of them updating the number
    /// here. A count in prose beside the list it counts is the cheapest thing in this
    /// repository to leave stale.
    ///
    /// # An exhaustive match, because both list shapes failed the same way twice
    ///
    /// This was a **deny-list** — `!matches!(self, …exemptions…)` — and its hazard is
    /// that a *new* verb needs a specimen unless someone remembers to add it, so a form
    /// needing none is refused by default with a notice about loading a specimen that has
    /// nothing to do with what was clicked. It has now produced that bug twice, and
    /// **Doug reported it both times, in nearly the same words:**
    ///
    /// - **2026-08-05, `OpenLab`** — *"The links in the 'Claude's answer' lab do not
    ///   work."*
    /// - **2026-08-31, `OpenDoc`** — the new `hrw://doc/` verb shipped with a green gate,
    ///   907 tests, and clicking the link Doug had just asked for showed *"no specimen
    ///   loaded — this stop needs one."*
    ///
    /// **An allow-list is not the fix and was correctly refused** the first time: it fails
    /// the other way, letting a verb that *does* need a specimen through to half-apply
    /// itself, which is the worse direction. The error was treating those two as the only
    /// options. **An exhaustive match with no wildcard defaults neither way** — it refuses
    /// to compile until the new variant is ruled on, which is the only shape where
    /// forgetting is not a possible outcome.
    ///
    /// **What was there instead was a test that named a guarantee it did not deliver.**
    /// `link_verbs_declare_whether_they_need_a_specimen` hand-lists four verbs, so it can
    /// only ever check the ones someone already thought about; the sentence claiming it
    /// *"enforces rather than leaving to memory"* was false the day it was written, and it
    /// is why the second occurrence was not caught. **A test over a hand-written list of
    /// siblings cannot see a new sibling** — that is the column-read audit's whole subject,
    /// and here the compiler does it for free.
    ///
    /// **Do not add a `_ =>` arm.** It would restore the exact defect, silently, and
    /// nothing downstream would fail.
    fn requires_specimen(&self) -> bool {
        match self {
            // Loading IS the act; requiring a load first would be circular.
            Self::LoadSpecimen(_) | Self::LoadAndSwitch(..) => false,
            // Opening a document — a lab, a doc, a notebook, a System Modeler model —
            // is navigation between FILES. No model need be loaded to read one, and
            // `OpenDoc` sitting in this group is the fix for 2026-08-31.
            Self::OpenLab { .. }
            | Self::OpenNotebook(_)
            | Self::OpenDoc(_)
            | Self::OpenSource(_)
            | Self::OpenInSystemModeler(_) => false,
            // Arming a breakpoint targets Rumoca's source, not the model — and a reader
            // may well want the breakpoints in place *before* loading a specimen, which
            // is exactly what `matching-live.md` warns them not to do the other way
            // round.
            Self::ArmBreakpoint(_) => false,
            // Everything below acts ON a loaded model: it selects a stage's view, aims at
            // one of its equations, seeks a frame of its captured run, points at a node of
            // its IR, or follows one of its identifiers. With nothing loaded there is no
            // referent, and half-applying would leave a pending state that fires later —
            // sending the reader somewhere no link pointed.
            Self::SwitchStage(..)
            | Self::ShowSource(_)
            | Self::AimAtEquation(..)
            | Self::SeekFrame(..)
            | Self::PointAtNode(..)
            | Self::Follow(_) => true,
        }
    }

    /// Whether following this link puts a **different application** in front of the
    /// reader.
    ///
    /// Autoplay gives such a beat longer on screen: the viewer has to reorient to
    /// another window, which prestarting the app removes the launch cost of but not the
    /// cost of.
    ///
    /// **Extracted from a `matches!` inside `start_autoplay` on 2026-08-31, in the audit
    /// that the `requires_specimen` bug called for** — and it was the second silent
    /// per-variant list in this one type. It was an *allow*-list, so its default is the
    /// gentler failure — a beat too short rather than a dead link — but silent in exactly
    /// the same way, and `OpenDoc` was missing from it for exactly the same reason.
    /// Exhaustive here too, so the question is asked of every future verb whether or not
    /// anyone thinks of autoplay.
    fn leaves_hrw(&self) -> bool {
        match self {
            // `OpenDoc` spawns `code` and `OpenNotebook` hands the file to Wolfram, so
            // both put another window in front of the reader. `OpenLab` does not: a
            // lab opens in HRW's own panel.
            Self::OpenNotebook(_)
            | Self::OpenDoc(_)
            | Self::OpenSource(_)
            | Self::OpenInSystemModeler(_) => true,
            Self::OpenLab { .. }
            | Self::LoadSpecimen(_)
            | Self::LoadAndSwitch(..)
            | Self::ArmBreakpoint(_)
            | Self::SwitchStage(..)
            | Self::ShowSource(_)
            | Self::AimAtEquation(..)
            | Self::SeekFrame(..)
            | Self::PointAtNode(..)
            | Self::Follow(_) => false,
        }
    }

    /// One line naming what this link does, for the action trail.
    ///
    /// Reconstructs the canonical URL rather than `Debug`-printing the enum: the trail
    /// is read by Claude alongside the lab markdown, and matching the lab's own text
    /// is what makes "Doug clicked Station 3" legible at a glance.
    fn describe(&self) -> String {
        match self {
            Self::OpenLab { lab, stop: None } => format!("lab/{lab}"),
            Self::OpenLab { lab, stop: Some(s) } => format!("lab/{lab}/station/{s}"),
            Self::LoadSpecimen(name) => format!("load/{name}"),
            Self::SwitchStage(kind, None) => format!("stage/{}", kind.slug()),
            Self::SwitchStage(kind, Some(sub)) => {
                format!("stage/{}/{}", kind.slug(), sub.slug())
            }
            Self::LoadAndSwitch(name, kind, None) => format!("load/{name}/{}", kind.slug()),
            Self::LoadAndSwitch(name, kind, Some(sub)) => {
                format!("load/{name}/{}/{}", kind.slug(), sub.slug())
            }
            Self::ShowSource(None) => "source".to_owned(),
            Self::ShowSource(Some(line)) => format!("source/{line}"),
            Self::AimAtEquation(kind, sub, n) => {
                format!("stage/{}/{}/equation/{n}", kind.slug(), sub.slug())
            }
            Self::SeekFrame(kind, sub, n) => {
                // +1 back to the 1-based form the link was written in.
                format!("stage/{}/{}/frame/{}", kind.slug(), sub.slug(), n + 1)
            }
            Self::PointAtNode(kind, None, path) => {
                format!("stage/{}/node/{}", kind.slug(), bridge::describe_path(path))
            }
            Self::PointAtNode(kind, Some(sub), path) => format!(
                "stage/{}/{}/node/{}",
                kind.slug(),
                sub.slug(),
                bridge::describe_path(path),
            ),
            Self::Follow(name) => format!("follow/{name}"),
            Self::OpenNotebook(name) => format!("notebook/{name}"),
            Self::OpenDoc(name) => format!("doc/{name}"),
            Self::OpenSource(target) => format!("src/{target}"),
            Self::OpenInSystemModeler(name) => format!("systemmodeler/{name}"),
            Self::ArmBreakpoint(name) => format!("breakpoint/{name}"),
        }
    }
}

/// Parse an `hrw://` URL into a navigation action, or `None` if malformed.
fn parse_hrw_link(url: &str) -> Option<HrwLink> {
    let path = url.strip_prefix("hrw://")?;
    // 5, not 4: the node form (`stage/<Stage>/<View>/node/<n>`) is five segments.
    // With a cap of 4 the trailing `node/<n>` glommed into one segment and the link
    // silently failed to parse — a link that does nothing is the worst outcome in a
    // lab, since nothing on screen says why.
    let parts: Vec<&str> = path.splitn(5, '/').collect();
    match parts.as_slice() {
        ["load", specimen, stage, view] => {
            let kind = StageKind::from_slug(stage)?;
            let sub = SubView::from_slug(kind, view)?;
            Some(HrwLink::LoadAndSwitch(
                (*specimen).to_owned(),
                kind,
                Some(sub),
            ))
        }
        ["load", specimen, stage] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::LoadAndSwitch((*specimen).to_owned(), kind, None))
        }
        ["load", specimen] => Some(HrwLink::LoadSpecimen((*specimen).to_owned())),
        // **Validated here, not at dispatch.** An unknown anchor failing to
        // parse is what puts `fixture_lab_links_all_resolve` in front of it, so
        // a renamed anchor breaks the suite instead of a walk.
        ["breakpoint", name] if crate::matching_ledger::anchor_by_name(name).is_some() => {
            Some(HrwLink::ArmBreakpoint((*name).to_owned()))
        }
        ["lab", name, "station", slug] if !name.is_empty() && !slug.is_empty() => {
            Some(HrwLink::OpenLab {
                lab: (*name).to_owned(),
                stop: Some((*slug).to_owned()),
            })
        }
        ["lab", name] if !name.is_empty() => Some(HrwLink::OpenLab {
            lab: (*name).to_owned(),
            stop: None,
        }),
        ["stage", stage, view] => {
            let kind = StageKind::from_slug(stage)?;
            let sub = SubView::from_slug(kind, view)?;
            Some(HrwLink::SwitchStage(kind, Some(sub)))
        }
        ["stage", stage] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::SwitchStage(kind, None))
        }
        // Longest patterns first: `splitn` caps the segment count, so the node form
        // must be matched before the shorter stage forms can swallow it.
        ["follow", name] => Some(HrwLink::Follow((*name).to_owned())),
        // A non-empty name: `hrw://notebook/` alone names nothing, and accepting it
        // meant a prose mention of the verb parsed as a link to an unnamed file.
        ["systemmodeler", name] if !name.is_empty() => {
            Some(HrwLink::OpenInSystemModeler((*name).to_owned()))
        }
        ["notebook", name] if !name.is_empty() => Some(HrwLink::OpenNotebook((*name).to_owned())),
        // **Rest-joined, because docs nest.** A notebook is a bare file name; a document
        // may be `compiler-phases/the-chain-of-problems.md`, so the tail is rejoined
        // rather than matched as one segment. `bridge::resolve_doc` refuses `..`.
        ["doc", rest @ ..] if !rest.is_empty() => {
            let name = rest.join("/");
            (!name.is_empty()).then_some(HrwLink::OpenDoc(name))
        }
        // Rest-joined for the same reason as `doc`, and more so — a source path is
        // always several segments deep. The `#<symbol>` tail rides along inside the
        // final segment untouched; `bridge::resolve_source` splits it.
        ["src", rest @ ..] if !rest.is_empty() => {
            let target = rest.join("/");
            (!target.is_empty()).then_some(HrwLink::OpenSource(target))
        }
        ["stage", stage, view, "node", path] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::PointAtNode(
                kind,
                Some(SubView::from_slug(kind, view)?),
                bridge::parse_path(path)?,
            ))
        }
        // **Without a sub-view — the only form the five tree-only stages can use.**
        // Deliberately not a fallback for a *misspelled* sub-view: that string would
        // have to parse as a path segment, and `parse_path` rejects what is not one.
        // A typo'd view slug still fails the arm above and then fails here, which is
        // the behaviour a lab author needs — a link that silently degraded to
        // "somewhere in the stage" is the quiet-wrong-place failure the checker
        // cannot see.
        ["stage", stage, "node", path] => Some(HrwLink::PointAtNode(
            StageKind::from_slug(stage)?,
            None,
            bridge::parse_path(path)?,
        )),
        ["stage", stage, view, "frame", n] => {
            let kind = StageKind::from_slug(stage)?;
            // **1-based, matching the frame counter on screen** ("Frame 3/11"). Links
            // were 0-based until 2026-07-29, so a lab saying `frame/40` landed on a
            // view reading "41" — the link vocabulary and the display disagreeing about
            // the same noun, which is the drift the parity audit exists to catch. The
            // fixture lab had the discrepancy *written into it* as a parenthetical,
            // which is documenting a bug rather than fixing it.
            //
            // `checked_sub(1)` rejects `frame/0` for free: under 1-based numbering there
            // is no frame zero, and a link saying so is a mistake worth surfacing.
            Some(HrwLink::SeekFrame(
                kind,
                SubView::from_slug(kind, view)?,
                n.parse::<usize>().ok()?.checked_sub(1)?,
            ))
        }
        ["stage", stage, view, "equation", n] => {
            let kind = StageKind::from_slug(stage)?;
            Some(HrwLink::AimAtEquation(
                kind,
                SubView::from_slug(kind, view)?,
                n.parse().ok()?,
            ))
        }
        ["source", line] => Some(HrwLink::ShowSource(Some(line.parse().ok()?))),
        ["source"] => Some(HrwLink::ShowSource(None)),
        _ => None,
    }
}

/// **The link that seeks frame `index`** of an animated view, where `index` is
/// 0-based as the algorithm's step list numbers it.
///
/// Exists so nobody performs this `+ 1` by hand. Links are **1-based**, matching
/// the counter on screen ("Frame 3/11"); every internal frame list is 0-based.
/// `examples/frame_index` printed the 0-based number and told the author it worked
/// directly in a link — so every link written from its output landed **one frame
/// early**, which is precisely the failure that tool exists to prevent: a
/// wrong-but-valid index resolves fine and simply shows the wrong step, and no link
/// checker can see it.
///
/// Found 2026-08-03 while scouting a matching-animation lab.
/// `a_frame_link_round_trips_through_the_parser` binds this to `parse_hrw_link`, so
/// the two cannot drift again.
pub fn frame_link(stage: &str, view: &str, index: usize) -> String {
    format!("hrw://stage/{stage}/{view}/frame/{}", index + 1)
}

/// Scan markdown text for all unique `hrw://` URLs (for hook registration).
fn extract_hrw_links(text: &str) -> Vec<String> {
    let mut links = Vec::new();
    for cap in text.match_indices("hrw://") {
        let start = cap.0;
        let rest = &text[start..];
        // A backtick ends a URL too: `hrw://notebook/` written in a **code span** is
        // prose *about* the verb, not a link. Without it the extractor captured the
        // backtick as part of the name and registered a hook that could never fire —
        // found 2026-07-30 by the fixture file-reference test, which duly reported a
        // notebook named "`" as missing.
        let end = rest
            .find([')', ' ', '\n', '"', '>', '`'])
            .unwrap_or(rest.len());
        let url = &rest[..end];
        if !links.contains(&url.to_owned()) {
            links.push(url.to_owned());
        }
    }
    links
}

/// Register link hooks so `egui_commonmark` reports clicks on `hrw://` links.
fn register_hrw_hooks(cache: &mut egui_commonmark::CommonMarkCache, links: &[String]) {
    for link in links {
        if cache.get_link_hook(link).is_none() {
            cache.add_link_hook(link);
        }
    }
}

/// Check registered hooks for a click and return the first triggered action.
/// **"Drain" is the whole contract: a fired hook is consumed, not merely read.**
///
/// Doug, 2026-08-16: *"There are two Act 2 links which don't cause any action when
/// clicked."*
///
/// `egui_commonmark` sets a hook to `true` on click and **never clears it**;
/// `get_link_hook` is a read, and `register_hrw_hooks` only initialises hooks it has
/// not seen. So this function used to *look* at the first fired hook and leave it
/// fired — permanently.
///
/// The consequence is worse than the two dead links that exposed it. The loop returns
/// the first `true` hook in **document order**, so after the first click anywhere in a
/// lab, that link is re-dispatched on every frame forever and **every link below it
/// becomes unreachable**: its own hook goes `true`, and the stuck one above it is
/// always found first.
///
/// It read as "nothing happens" rather than as chaos because dispatching a link that
/// navigates where the app already is has no visible effect — the app was busy
/// re-arriving at Station 1's destination while Doug clicked Station 2. And it hid for as long
/// as it did because **restarting HRW clears the cache**, so the next link clicked
/// after any rebuild worked, and this project rebuilds constantly.
///
/// `add_link_hook` inserts `false` unconditionally, which is the documented way to
/// reset one.
fn drain_hrw_hooks(
    cache: &mut egui_commonmark::CommonMarkCache,
    links: &[String],
) -> Option<HrwLink> {
    for link in links {
        if cache.get_link_hook(link) == Some(true) {
            // Consume it before returning, so the next frame starts clean whether or
            // not the link parses.
            cache.add_link_hook(link);
            return parse_hrw_link(link);
        }
    }
    None
}

/// Hand a file to the operating system's association for its type.
///
/// A `.nb` opens in Wolfram Desktop, because that is what Windows associates it with —
/// HRW does not need to know where Wolfram lives, and would be wrong the moment it moved.
///
/// `cmd /C start` rather than a crate: adding a dependency needs asking first, and this
/// is four lines. The empty string after `start` is **required** — `start` treats a lone
/// quoted argument as a window title, so omitting it opens a console instead of the file.
#[cfg(target_os = "windows")]
fn open_with_os(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("cmd")
        .args(["/C", "start", "", &path.display().to_string()])
        .spawn()
        .map(|_| ())
}

/// The VS Code CLI, as a name `CreateProcess` can actually find.
///
/// # `Command::new("code")` cannot find VS Code on Windows
///
/// What VS Code puts on `PATH` is **`code.cmd`** — there is no `code.exe` beside it.
/// Rust resolves a bare program name by trying the literal name and appending `.exe`;
/// it **never consults `PATHEXT`**. So `Command::new("code")` fails with *"program not
/// found"* on a machine where typing `code` in any shell works perfectly, which is what
/// Doug hit on 2026-08-31.
///
/// **And the check that "verified" it was asking a different question.** `code --version`
/// was run in git-bash and reported 1.135.0 — which establishes that VS Code is installed
/// and says nothing about what `CreateProcess` can find, because **a shell resolving a
/// name is not the runtime resolving it**: cmd applies `PATHEXT`, bash has its own lookup
/// including extensionless scripts, and the Win32 API does neither. A verification has to
/// run through the same resolver the code does, or it is evidence about the shell.
///
/// `check_machine` now rules on this, since it is exactly what does not travel with a
/// `git pull` — a different machine may have VS Code somewhere else or not at all.
#[cfg(target_os = "windows")]
pub const VSCODE_CLI: &str = "code.cmd";

/// On other platforms `code` is an ordinary executable script and resolves normally.
#[cfg(not(target_os = "windows"))]
pub const VSCODE_CLI: &str = "code";

/// Open a file in VS Code, rather than in whatever the OS associates with its type.
///
/// Deliberately **not** [`open_with_os`]: `.md` is associated with Chrome on Doug's
/// machine, which is the bug this whole verb exists to fix, so falling back to the
/// association on failure would silently reproduce it. A failure is reported instead.
fn open_in_vscode(path: &Path, line: Option<usize>) -> std::io::Result<()> {
    let mut cmd = std::process::Command::new(VSCODE_CLI);
    // `-g file:line` is VS Code's own "goto" form. Without `-g` the `:line` suffix is
    // read as part of the FILE NAME, and VS Code helpfully offers to create it — so the
    // flag is not decoration, it is the difference between arriving at the symbol and
    // being asked whether to make a new file called `mod.rs:412`.
    match line {
        Some(n) => {
            cmd.arg("-g");
            cmd.arg(format!("{}:{n}", path.display()));
        }
        None => {
            cmd.arg(path);
        }
    }
    // Spawning a `.cmd` runs it through `cmd.exe`, which would otherwise flash a console
    // window over the lab on every doc link. `CREATE_NO_WINDOW`.
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd.spawn().map(|_| ())
}

/// Non-Windows fallback. HRW is Windows-only in practice (charter Decision 5 rules out
/// other targets), but a `cfg` that silently compiles to nothing would be worse than one
/// that says so.
#[cfg(not(target_os = "windows"))]
fn open_with_os(path: &Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map(|_| ())
}

/// Cap markdown heading size to 1.15x body so rendered lab/narrative text stays compact.
pub(crate) fn set_markdown_text_sizes(ui: &mut egui::Ui) {
    let body_size = ui.text_style_height(&egui::TextStyle::Body);
    ui.style_mut().text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::proportional(body_size * 1.15),
    );
}

const GOLDEN_RATIO: f32 = 0.618_034;

/// The colour for simulation series `i` — egui_plot's own auto-colour palette
/// (golden-ratio hue, `Hsva`), replicated so we can pin it explicitly. We must:
/// a variable plotted as several segments (broken at discontinuities) would else
/// take a different auto-colour per segment. Keyed on the variable index, this
/// equals the colour egui_plot picked when each variable was a single line.
///
/// The **golden-ratio hue trick**: multiplying the series index by the golden
/// ratio (0.618...) and using the fractional part as a hue angle produces
/// colors that are maximally spread around the color wheel. Each new series
/// lands as far as possible from all previous ones in hue space, giving
/// visually distinct colors without a hand-picked palette. `Hsva` constructs
/// a color from Hue/Saturation/Value/Alpha; egui wraps hue mod 1.0
/// automatically.
fn series_color(i: usize) -> egui::Color32 {
    egui::ecolor::Hsva::new(i as f32 * GOLDEN_RATIO, 0.85, 0.5, 1.0).into()
}

#[cfg(test)]
mod tests;
