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
pub mod log_view;
pub mod reduction_anim;
pub mod reduction_view;
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

/// Shared animation playback controls (play/pause/reset/step/speed slider).
/// Used by both matching and Tarjan animation views.
/// Whether timed playback (Play/Pause and the speed slider) applies.
///
/// Suppressed *during* a live debug session: the debugger drives the cursor
/// one frame per Continue, so a Play button would fight it for control (and
/// the animations' own `playing && !is_live()` guards make it a no-op anyway).
///
/// Restored once the session finishes. The captured frames are then ordinary
/// recorded frames, and replaying them is exactly what you want after stepping
/// through the algorithm — previously `live_rx` was never cleared, so the view
/// stayed in live mode forever and Play never came back without a recompile.
pub fn playback_applies(is_live: bool, live_finished: bool) -> bool {
    !is_live || live_finished
}

/// Label for the frame counter.
///
/// `n_frames` is zero while a live session is armed but the debugger is still
/// paused at the startup gate — the controls render in that state so Reset
/// stays reachable, and "Frame 1/0" would be nonsense.
pub fn frame_label(cursor: usize, n_frames: usize) -> String {
    if n_frames == 0 {
        "No frames yet".to_owned()
    } else {
        format!("Frame {}/{}", cursor + 1, n_frames)
    }
}

pub fn animation_controls(
    ui: &mut eframe::egui::Ui,
    cursor: &mut usize,
    playing: &mut bool,
    elapsed: &mut f64,
    interval: &mut f64,
    n_frames: usize,
    is_live: bool,
    live_finished: bool,
) {
    use eframe::egui;
    ui.horizontal(|ui| {
        if is_live {
            let status = if live_finished { "Live (done)" } else { "Live" };
            ui.label(egui::RichText::new(status).color(
                if live_finished { colors::ANIM_PATH_FOUND }
                else { colors::ANIM_FAIL }
            ).strong());
            ui.separator();
        }

        if playback_applies(is_live, live_finished) {
            if *playing {
                if ui.button("\u{23f8} Pause").clicked() {
                    *playing = false;
                }
            } else if ui.button("\u{25b6} Play").clicked() {
                if *cursor + 1 >= n_frames {
                    *cursor = 0;
                }
                *playing = true;
                *elapsed = 0.0;
            }
        }

        if ui.button("\u{23ee} Reset").clicked() {
            *cursor = 0;
            *playing = false;
        }

        ui.add_enabled_ui(!*playing, |ui| {
            if ui
                .add_enabled(*cursor > 0, egui::Button::new("\u{25c0} Back"))
                .clicked()
            {
                *cursor = cursor.saturating_sub(1);
            }
            if ui
                .add_enabled(
                    *cursor + 1 < n_frames,
                    egui::Button::new("Step \u{25b6}"),
                )
                .clicked()
            {
                *cursor += 1;
            }
        });

        ui.separator();
        ui.label(frame_label(*cursor, n_frames));

        if playback_applies(is_live, live_finished) {
            ui.separator();
            ui.label("Speed:");
            let mut speed_ms = (*interval * 1000.0) as i32;
            if ui
                .add(egui::Slider::new(&mut speed_ms, 50..=2000).suffix("ms"))
                .changed()
            {
                *interval = speed_ms as f64 / 1000.0;
            }
        }
    });
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
    use super::playback_applies;

    /// The frame counter must stay sensible with zero frames — the controls
    /// now render while a live session waits at the startup gate, so that
    /// state is reachable rather than hypothetical.
    #[test]
    fn frame_label_handles_the_empty_live_case() {
        assert_eq!(super::frame_label(0, 0), "No frames yet");
        assert_eq!(super::frame_label(0, 1), "Frame 1/1");
        assert_eq!(super::frame_label(3, 10), "Frame 4/10");
    }

    /// Play/Pause and the speed slider are hidden only *while* a live debug
    /// session is running — not after it finishes.
    ///
    /// Regression test: `live_rx` is never cleared when a session ends, so
    /// `is_live()` stays true forever. Gating playback on `!is_live` alone left
    /// the Play button gone for good once you had used the Debug button, and
    /// the captured frames could not be replayed without a recompile.
    #[test]
    fn playback_hidden_only_during_a_running_live_session() {
        // Recorded animation: always playable.
        assert!(playback_applies(false, false));
        assert!(playback_applies(false, true));
        // Live session in progress: the debugger owns the cursor.
        assert!(!playback_applies(true, false));
        // Live session finished: frames are now ordinary recorded frames.
        assert!(playback_applies(true, true));
    }
}

#[cfg(test)]
mod tests {
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
