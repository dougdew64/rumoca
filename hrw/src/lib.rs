//! HRW Observatory library crate — module registration and shared exports.
//!
//! ## Why a library crate?
//!
//! Rust binaries (`main.rs`) cannot be depended on by other targets. By putting
//! all modules in a library crate (`lib.rs`), both the GUI binary (`main.rs`)
//! *and* headless tools (`examples/gen_trace`, which writes a specimen's durable
//! compilation trace log) can share one implementation of the compilation
//! pipeline. The binary is a thin shell that launches eframe; all logic lives here.
//!
//! ## Module map
//!
//! The observatory is organized around a few key roles:
//!
//! - **`app`** — the top-level `eframe::App` implementation: UI layout, tab bar,
//!   specimen list, and the glue that wires everything together each frame.
//! - **`worker`** — background-thread compilation and simulation; sends results
//!   back to the UI over a channel so the GUI never blocks.
//! - **`bridge`** — the "Claude bridge": writes a JSON focus file describing
//!   what the user captured, so Claude Code can reason about it.
//! - **`tree`** — the generic serde-value tree inspector, used for every pipeline
//!   stage's IR (one widget, many stages).
//! - **`expr_format`** — Modelica-like expression pretty-printer (precedence-aware).
//! - **`canvas`** — reusable pan/zoom scaffold for custom-painted views.
//! - **`spyplot`** — BLT (block lower triangular) spy-plot, a custom-painter view.
//! - **`incidence_view`** — incidence matrix (equation x unknown adjacency) view.
//! - **`matching_anim`** — animated matching stepper (augmenting-path replay).
//! - **`tarjan_anim`** — animated Tarjan SCC stepper (BLT discovery replay).
//! - **`reduction_view`** — index reduction process summary (the Pantelides funnel).
//! - **`equation_sheet`** — readable equation sheet from the flat DAE (grouped by origin).
//! - **`identifier_index`** — cross-stage identifier index (source → flat names).
//! - **`log_view`** — timestamped compilation/simulation log panel.
//! - **`reduction_anim`** — index-reduction algorithm animation (step-by-step replay).
//! - **`playback`** — frame cursor/timing/live-session state shared by every
//!   animated algorithm view (`Playback<T>`), plus the `Animated` trait.
//! - **`pre_lowering_anim`** — replay of `pre()` lowering: where a `__pre__.x`
//!   parameter slot is manufactured (idea #40).
//! - **`colors`** — shared color constants used across canvas and view modules.
//! - **`field_help`** — build-time-embedded doc comments for IR fields (fast help).

pub mod app;
pub mod bridge;
pub mod canvas;
pub mod colors;
pub mod equation_sheet;
pub mod expr_format;
pub mod field_help;
pub mod identifier_index;
pub mod incidence_view;
pub mod matching_anim;
pub mod diagnostics;
pub mod log_view;
pub mod modelica_lex;
pub mod alias_anim;
pub mod connection_anim;
pub mod ic_plan_anim;
pub mod doc_citations;
pub mod playback;
pub mod pre_lowering_anim;
pub mod reduction_anim;
pub mod tearing_anim;
#[cfg(test)]
pub mod test_support;
pub mod reduction_view;
pub mod source_view;
pub mod spyplot;
pub mod tarjan_anim;
pub mod tree;
pub mod worker;

/// Minimum zoom level at which matrix axis labels (equation/unknown names)
/// are drawn. Below this threshold the labels would overlap and become
/// unreadable. Used by spyplot, incidence, and matching views.
pub const LABEL_ZOOM_THRESHOLD: f32 = 16.0;

/// Minimum zoom level at which node labels are drawn in graph views
/// (e.g. Tarjan SCC). Graph nodes are larger than matrix cells, so
/// labels remain readable at a lower zoom.
pub const NODE_LABEL_ZOOM_THRESHOLD: f32 = 10.0;

/// Truncate a label to at most `max` bytes, returning a sub-slice.
/// Falls back to the full string if the boundary isn't char-aligned.
pub fn truncate_label(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        s.get(..max).unwrap_or(s)
    }
}

/// Convert a byte offset into a 1-based line number by counting newlines.
pub fn byte_offset_to_line(source: &str, byte_offset: usize) -> u32 {
    let clamped = byte_offset.min(source.len());
    source[..clamped].bytes().filter(|&b| b == b'\n').count() as u32 + 1
}

/// Where an animation view is in the live-debug lifecycle.
///
/// Replaces the `is_live` / `live_finished` boolean pair. Four states, not
/// three: `Arming` is distinct from `Running` because the Debug button's
/// breakpoint handshake takes several frames, during which the view is still
/// showing the *recorded* animation — so neither boolean could express "a live
/// session is starting" and the controls stayed live-looking until the
/// algorithm thread actually spawned.
///
/// ## UI rule
///
/// **Controls are enabled and disabled, never shown and hidden.** A button that
/// vanishes gives no clue that the action exists or why it is unavailable, and
/// the row reflows under the pointer. Everything stays in place; only
/// interactivity changes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LiveState {
    /// No live session. Ordinary recorded playback.
    Idle,
    /// Debug clicked; waiting for the breakpoint handshake and thread spawn.
    Arming,
    /// Live session running — the debugger owns the cursor, one frame per Continue.
    Running,
    /// Live session finished. The captured frames are now ordinary recorded
    /// frames, so playback and a fresh Debug run both become available again.
    Finished,
}

impl LiveState {
    /// True while a live session owns the cursor — from the Debug click until
    /// the algorithm thread finishes.
    ///
    /// Gates both the playback controls (the debugger is driving, so Play would
    /// fight it) and the Debug button itself (no second session on top of a
    /// running one).
    pub fn is_busy(self) -> bool {
        matches!(self, LiveState::Arming | LiveState::Running)
    }

    /// A stable name, for the capture and the crash log.
    ///
    /// Separate from [`LiveState::badge`] on purpose: the badge is display text
    /// that may gain an ellipsis or change wording, while this is an identifier
    /// something else reasons over. One string for looking at, one for matching.
    pub fn name(self) -> &'static str {
        match self {
            LiveState::Idle => "Idle",
            LiveState::Arming => "Arming",
            LiveState::Running => "Running",
            LiveState::Finished => "Finished",
        }
    }

    /// Status badge text and colour, or `None` when there is nothing to say.
    pub fn badge(self) -> Option<(&'static str, eframe::egui::Color32)> {
        match self {
            LiveState::Idle => None,
            LiveState::Arming => Some(("Arming\u{2026}", colors::ANIM_EXPLORE)),
            LiveState::Running => Some(("Live", colors::ANIM_FAIL)),
            LiveState::Finished => Some(("Live (done)", colors::ANIM_PATH_FOUND)),
        }
    }
}

/// Hover text for a gesture that starts or stops **following** an identifier.
///
/// ## Why the hover names the verb, and admits the side effect
///
/// Left-click means different things on different surfaces: **follow** where the
/// clickable thing is a name (the specimen source, the variable grid), **point
/// at** where it is a node (trees, stage tabs, incidence rows, spy blocks). The
/// split is not arbitrary — a source token has no IR address, so "point at" is
/// not even expressible there — but until you have clicked, nothing tells you
/// which you are about to get. The Context Bar answers that afterwards; this
/// answers it beforehand.
///
/// The wording also admits that the click **sends** something. Doug's
/// observation (2026-07-28): "point at" reads like a gesture with no
/// consequence, yet every left-click here writes `focus.json` and changes what
/// Claude has. Renaming to "select" would misdescribe it in the other
/// direction — selection is expected to be free, local and private, and this is
/// none of those. So the verbs stay and the gesture says what it will do. See
/// `docs/context-assembly.md`.
pub fn follow_hover(name: &str, following_now: bool) -> String {
    if following_now {
        format!("Stop following {name} \u{2014} removes it from the context Claude has")
    } else {
        format!("Follow {name} \u{2014} sends it to Claude now, and highlights it in every stage")
    }
}

/// Hover text for a gesture that **points at** a node.
///
/// Companion to [`follow_hover`] — see its docs for why these say "sends".
/// A `const` rather than a function because, unlike following, pointing at
/// names no target: the row under the cursor is the target.
pub const POINT_AT_HOVER: &str =
    "Point at this \u{2014} sends it to Claude now as the subject of your next question";

/// Label for the frame counter.
///
/// **No denominator while a live session is running.** Frames arrive one at a
/// time from the algorithm thread, so `cursor + 1` always equals the count so
/// far — the label read "Frame 1/1, 2/2, … 11/11". That is not merely odd: a
/// denominator is a claim about the total, and `3/3` says *you are at the end*
/// when the algorithm may have fifty steps left. The total is genuinely unknown
/// until the session finishes, and the honest rendering is to omit it rather
/// than print the one number that happens to be available.
///
/// Once the session is `Finished` the frames are an ordinary recorded trace, the
/// total is known, and `n/total` returns.
///
/// `n_frames` is zero while a live session is armed but the debugger is still
/// paused at the startup gate — the controls render in that state so Reset stays
/// reachable, and "Frame 1/0" would be nonsense.
pub fn frame_label(cursor: usize, n_frames: usize, live: LiveState) -> String {
    if n_frames == 0 {
        "No frames yet".to_owned()
    } else if live.is_busy() {
        // The debugger is still producing frames; the total does not exist yet.
        format!("Frame {} \u{00b7} live", cursor + 1)
    } else {
        format!("Frame {}/{}", cursor + 1, n_frames)
    }
}

/// The shared animation control row.
///
/// Layout, left to right:
///
/// ```text
/// [Play/Pause] [Reset] [Back] [Step] | Frame n/m | Speed: [====] | [Debug]  Live (done)
/// ```
///
/// Playback first, then the frame/speed group, then a divider, then the live
/// debug controls. The Debug button lives here rather than in the caller so
/// that every animation control sits on one row; it reports its click through
/// the return value, since arming a session needs app state this function has
/// no access to.
///
/// Every control is always rendered. While a live session is in flight
/// (`LiveState::is_busy`) the playback controls are *disabled*, not removed —
/// the debugger owns the cursor, but the user can still see what exists and,
/// via the disabled-hover text, why it is unavailable.
///
/// Returns `true` on the frame the Debug button is clicked.
#[must_use]
pub fn animation_controls(
    ui: &mut eframe::egui::Ui,
    playback: crate::playback::PlaybackControls<'_>,
    live: LiveState,
    debug_enabled: bool,
) -> bool {
    use eframe::egui;
    // Destructured once, so the body reads exactly as before. The bundle exists
    // to fix the *call site*: this used to take `cursor`, `playing`, `elapsed`
    // and `interval` as four separate `&mut` arguments — two of them adjacent
    // bools — so transposing a pair compiled silently and misbehaved at runtime.
    // Same reasoning as `TreeActions` and `TreeOptions`.
    let crate::playback::PlaybackControls { cursor, playing, elapsed, interval, n_frames } =
        playback;
    let busy = live.is_busy();
    let mut debug_clicked = false;
    // Explains every disabled control in the row.
    const BUSY_HINT: &str =
        "Unavailable during a live debug session \u{2014} the debugger drives the \
         animation. Continue (F5) in VS Code to advance a step.";

    ui.horizontal(|ui| {
        // Play/Pause. Kept in place while busy so the row does not reflow.
        if *playing {
            if ui
                .add_enabled(!busy, egui::Button::new("\u{23f8} Pause"))
                .on_disabled_hover_text(BUSY_HINT)
                .clicked()
            {
                *playing = false;
            }
        } else if ui
            .add_enabled(!busy, egui::Button::new("\u{25b6} Play"))
            .on_disabled_hover_text(BUSY_HINT)
            .clicked()
        {
            if *cursor + 1 >= n_frames {
                *cursor = 0;
            }
            *playing = true;
            *elapsed = 0.0;
        }

        if ui
            .add_enabled(!busy, egui::Button::new("\u{23ee} Reset"))
            .on_disabled_hover_text(BUSY_HINT)
            .clicked()
        {
            *cursor = 0;
            *playing = false;
        }

        // Back/Step are additionally bounded by the cursor position, and are
        // disabled during timed playback so the two cannot fight over the cursor.
        let stepping_enabled = !busy && !*playing;
        if ui
            .add_enabled(stepping_enabled && *cursor > 0, egui::Button::new("\u{25c0} Back"))
            .on_disabled_hover_text(if busy { BUSY_HINT } else { "Already at the first frame" })
            .clicked()
        {
            *cursor = cursor.saturating_sub(1);
        }
        if ui
            .add_enabled(
                stepping_enabled && *cursor + 1 < n_frames,
                egui::Button::new("Step \u{25b6}"),
            )
            .on_disabled_hover_text(if busy { BUSY_HINT } else { "Already at the last frame" })
            .clicked()
        {
            *cursor += 1;
        }

        ui.separator();
        ui.label(frame_label(*cursor, n_frames, live));

        ui.separator();
        ui.add_enabled_ui(!busy, |ui| {
            ui.label("Speed:");
            let mut speed_ms = (*interval * 1000.0) as i32;
            if ui
                .add(egui::Slider::new(&mut speed_ms, 50..=2000).suffix("ms"))
                .changed()
            {
                *interval = speed_ms as f64 / 1000.0;
            }
        });

        // --- Live debug: the Debug button, then the status badge ---
        ui.separator();
        debug_clicked = ui
            .add_enabled(debug_enabled, egui::Button::new("Debug"))
            .on_hover_text(
                "Start live debug session \u{2014} arms a breakpoint on \
                 live_trace_breakpoint, then step with Continue (F5)",
            )
            // When it is disabled and not busy, the only remaining reason is
            // that this algorithm has no data to run against yet.
            .on_disabled_hover_text(if busy {
                "A live debug session is already in progress"
            } else {
                "No data for this algorithm yet \u{2014} compile a specimen first"
            })
            .clicked();

        if let Some((text, color)) = live.badge() {
            ui.label(egui::RichText::new(text).color(color).strong());
        }
    });

    debug_clicked
}

/// Draw column (unknown) labels rotated -45° above the matrix,
/// and row (equation) labels to the left. Shared between the
/// incidence and matching views.
pub fn draw_matrix_axis_labels(
    ui: &eframe::egui::Ui,
    painter: &eframe::egui::Painter,
    view: canvas::View,
    col_labels: &[String],
    row_labels: &[String],
    col_max_chars: usize,
    row_max_chars: usize,
) {
    use eframe::egui;
    let visuals = ui.visuals();
    let font_size = (view.zoom() * 0.35).min(14.0);
    let font = egui::FontId::proportional(font_size);
    let label_color = visuals.text_color().gamma_multiply(0.7);
    let angle = -std::f32::consts::FRAC_PI_4;
    let col_gap_px = font_size * 1.6;
    for (col, name) in col_labels.iter().enumerate() {
        let cell_top = view.to_screen(egui::pos2(col as f32 + 0.5, 0.0));
        let anchor = egui::pos2(cell_top.x, cell_top.y - col_gap_px);
        let galley = painter.layout_no_wrap(
            truncate_label(name, col_max_chars).to_owned(),
            font.clone(),
            label_color,
        );
        let mut shape = egui::epaint::TextShape::new(anchor, galley, label_color);
        shape.angle = angle;
        shape.override_text_color = Some(label_color);
        painter.add(shape);
    }
    let row_gap_px = font_size * 0.5;
    let unclipped = ui.painter();
    for (row, name) in row_labels.iter().enumerate() {
        let cell_left = view.to_screen(egui::pos2(0.0, row as f32 + 0.5));
        let pos = egui::pos2(cell_left.x - row_gap_px, cell_left.y);
        unclipped.text(
            pos,
            egui::Align2::RIGHT_CENTER,
            truncate_label(name, row_max_chars),
            font.clone(),
            label_color,
        );
    }
}

/// Extract a JSON array of strings into a `Vec<String>`.
///
/// Defensive — returns an empty vec if the value is missing or not an array.
/// Used by multiple views to extract equation names, unknown names, etc.
pub fn str_vec(v: Option<&serde_json::Value>) -> Vec<String> {
    v.and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests_playback {
    use super::LiveState;

    /// The frame counter must stay sensible with zero frames — the controls
    /// now render while a live session waits at the startup gate, so that
    /// state is reachable rather than hypothetical.
    #[test]
    fn frame_label_handles_the_empty_live_case() {
        use super::LiveState;
        assert_eq!(super::frame_label(0, 0, LiveState::Idle), "No frames yet");
        assert_eq!(super::frame_label(0, 1, LiveState::Idle), "Frame 1/1");
        assert_eq!(super::frame_label(3, 10, LiveState::Idle), "Frame 4/10");
    }

    /// A running live session must not print a denominator.
    ///
    /// Frames arrive one at a time, so the count so far always equals
    /// `cursor + 1` — the label read "1/1, 2/2, … 11/11", which says *you are at
    /// the end* on every single frame. The total is unknown until the session
    /// finishes; omitting it is the honest rendering. Doug caught this stepping
    /// through all four animations.
    #[test]
    fn a_running_live_session_shows_no_total() {
        use super::LiveState;
        assert_eq!(super::frame_label(0, 1, LiveState::Running), "Frame 1 \u{00b7} live");
        assert_eq!(super::frame_label(10, 11, LiveState::Running), "Frame 11 \u{00b7} live");
        // Arming has no frames yet, so the empty case still wins.
        assert_eq!(super::frame_label(0, 0, LiveState::Arming), "No frames yet");
        // Once finished, the frames are an ordinary recorded trace and the total
        // is real again.
        assert_eq!(super::frame_label(3, 11, LiveState::Finished), "Frame 4/11");
    }

    /// Controls are disabled only *while* a live session owns the cursor — from
    /// the Debug click through to the algorithm thread finishing.
    ///
    /// Two regressions are pinned here. `live_rx` is never cleared when a
    /// session ends, so `is_live()` stays true forever; gating on that alone
    /// left playback dead for good once the Debug button had been used. And
    /// `Arming` must count as busy: the handshake takes several frames during
    /// which the view still shows the *recorded* animation, so a boolean
    /// `is_live` left the controls live right after the click.
    #[test]
    fn controls_disabled_only_while_a_live_session_is_in_flight() {
        assert!(!LiveState::Idle.is_busy(), "recorded playback is always available");
        assert!(LiveState::Arming.is_busy(), "the Debug click must disable immediately");
        assert!(LiveState::Running.is_busy(), "the debugger owns the cursor");
        assert!(
            !LiveState::Finished.is_busy(),
            "after a session the frames are ordinary recorded frames"
        );
    }

    /// Idle is the only state with nothing to report; the rest are labelled.
    #[test]
    fn badge_present_except_when_idle() {
        assert!(LiveState::Idle.badge().is_none());
        for state in [LiveState::Arming, LiveState::Running, LiveState::Finished] {
            assert!(state.badge().is_some(), "{state:?} should carry a badge");
        }
    }
}

#[cfg(test)]
mod tests {
    /// A gesture must say what it will do, and admit that it sends something.
    ///
    /// Doug, 2026-07-28: left-click means "follow" on some surfaces and
    /// "point at" on others, and nothing told him which until after he clicked.
    /// He also noted that "point at" reads like a gesture with no consequence,
    /// while every left-click here writes `focus.json`. Both fixes live in
    /// these strings, so both are pinned here.
    #[test]
    fn gesture_hovers_name_the_verb_and_admit_the_send() {
        let start = follow_hover("emf.phi", false);
        assert!(start.starts_with("Follow emf.phi"), "must lead with the verb: {start}");
        assert!(start.contains("sends it to Claude"), "must admit the side effect: {start}");

        let stop = follow_hover("emf.phi", true);
        assert!(stop.starts_with("Stop following"), "the toggle must be legible: {stop}");
        assert!(stop.contains("removes it"), "must say what it takes away: {stop}");

        assert!(POINT_AT_HOVER.starts_with("Point at"), "{POINT_AT_HOVER}");
        assert!(POINT_AT_HOVER.contains("sends it to Claude"), "{POINT_AT_HOVER}");

        // The two verbs must stay distinguishable — the whole point is that a
        // reader can tell which gesture they are about to make.
        assert_ne!(start, POINT_AT_HOVER);
        assert!(!POINT_AT_HOVER.contains("Follow"));
    }

    use super::*;
    use serde_json::json;

    #[test]
    fn str_vec_extracts_strings() {
        let arr = json!(["a", "b", "c"]);
        assert_eq!(str_vec(Some(&arr)), vec!["a", "b", "c"]);
    }

    #[test]
    fn str_vec_skips_non_strings() {
        let arr = json!(["a", 42, "b", null]);
        assert_eq!(str_vec(Some(&arr)), vec!["a", "b"]);
    }

    #[test]
    fn str_vec_returns_empty_on_none() {
        assert!(str_vec(None).is_empty());
    }

    #[test]
    fn str_vec_returns_empty_on_non_array() {
        assert!(str_vec(Some(&json!("not an array"))).is_empty());
    }

    #[test]
    fn truncate_label_ascii() {
        assert_eq!(truncate_label("abcde", 3), "abc");
        assert_eq!(truncate_label("ab", 3), "ab");
        assert_eq!(truncate_label("abc", 3), "abc");
    }

    #[test]
    fn truncate_label_multibyte_does_not_panic() {
        let s = "élan";
        assert_eq!(truncate_label(s, 1), s);
        assert_eq!(truncate_label(s, 2), "é");
    }
}
