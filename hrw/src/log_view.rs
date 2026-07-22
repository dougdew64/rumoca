//! The compilation/simulation log view.
//!
//! This module renders a scrollable, timestamped event log that the worker
//! thread populates during compilation and simulation. Each `LogEntry` carries
//! a `LogLevel` (info, stage-start, stage-end, warn, error, stdout, stderr,
//! trace) plus an elapsed-time timestamp, so you can see both *what* each
//! compiler phase did and *how long* it took.
//!
//! ## Architecture role
//!
//! The log view is the primary "proof the in-workspace migration was worthwhile":
//! when HRW depended on Rumoca as an external crate, the public API exposed
//! only final results, not per-phase timing or internal trace events. Now that
//! HRW lives inside the Rumoca workspace, the worker thread can instrument each
//! phase boundary and capture Rumoca's internal `tracing` output.
//!
//! ## Data flow
//!
//! 1. The worker thread (`worker.rs`) pushes `LogEntry` values into a `Vec`
//!    as each compilation phase starts/ends and as simulation progresses.
//! 2. The main thread polls for new entries each frame (channel-based).
//! 3. This module renders the accumulated entries in a scrollable panel,
//!    with `stick_to_bottom(true)` so new entries auto-scroll into view.
//!
//! ## Tracing toggle
//!
//! A "Rumoca tracing" checkbox lets the user enable debug-level trace output
//! from Rumoca's internals (structural analysis, flatten, DAE construction,
//! solvers). This must be toggled *before* compiling — it controls whether
//! the worker installs a tracing subscriber that captures those events.

use eframe::egui;

use crate::worker::{LogEntry, LogLevel};

/// Render the log view and return `true` if the tracing checkbox was toggled.
///
/// This is a *stateless* function — it takes references to the log entries and
/// the tracing-enabled flag, and returns whether the user toggled it. The
/// caller (app.rs) owns the state and acts on the toggle (e.g., reconfiguring
/// the worker's tracing subscriber).
///
/// # Arguments
///
/// * `ui` — the egui drawing context for this panel
/// * `entries` — the accumulated log entries from the worker thread
/// * `tracing_enabled` — mutable ref to the tracing toggle state; the checkbox
///   reads and writes this
pub fn ui(ui: &mut egui::Ui, entries: &[LogEntry], tracing_enabled: &mut bool) -> bool {
    let mut toggled = false;

    // Tracing checkbox — placed above the log, so it is always visible even
    // when the log is long. The checkbox mutates `tracing_enabled`; the caller
    // uses the `toggled` return value to know when to reconfigure the worker.
    ui.horizontal(|ui| {
        if ui.checkbox(tracing_enabled, "Rumoca tracing")
            .on_hover_text(
                "Enable Rumoca\u{2019}s internal tracing (debug-level events from \
                 structural analysis, flatten, DAE construction, solvers). \
                 Toggle before compiling a specimen to see the trace.",
            )
            .changed()
        {
            toggled = true;
        }
    });
    ui.separator();

    // Empty state — no specimen compiled yet.
    if entries.is_empty() {
        ui.weak("Select a specimen to see its compilation log.");
        return toggled;
    }

    // Scrollable log body. `stick_to_bottom(true)` keeps the newest entries
    // visible as they arrive — important during a live simulation where entries
    // stream in over multiple frames.
    egui::ScrollArea::vertical()
        .id_salt("log_view")
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for entry in entries {
                let (prefix, color) = level_style(entry, ui);
                // Each row: [elapsed ms] [level prefix] [message]
                // All monospace so columns align vertically across entries.
                ui.horizontal(|ui| {
                    // Right-aligned elapsed time in milliseconds, with one
                    // decimal place (e.g. "   12.3ms").
                    ui.monospace(format!("{:>8.1}ms", entry.elapsed_secs * 1000.0));
                    // Level prefix — color-coded to visually distinguish
                    // phase transitions, warnings, errors, and trace output.
                    ui.colored_label(color, egui::RichText::new(prefix).monospace());
                    ui.colored_label(color, egui::RichText::new(&entry.message).monospace());
                });
            }
        });
    toggled
}

// Map a LogLevel to its display prefix and color.
//
// The prefix strings are padded to align in the monospace log. Unicode symbols
// are used for stage-start (play triangle) and stage-end (checkmark) to make
// phase boundaries visually distinct at a glance.
//
// Colors adapt to light/dark theme via `ui.visuals().dark_mode` — important
// because egui supports both themes and we want readable contrast in each.
fn level_style<'a>(entry: &LogEntry, ui: &egui::Ui) -> (&'a str, egui::Color32) {
    match entry.level {
        LogLevel::Info => ("  info", ui.visuals().text_color()),
        // Stage-start: blue play-triangle prefix
        LogLevel::StageStart => ("    \u{25b6}", if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x58, 0xa6, 0xff)
        } else {
            egui::Color32::from_rgb(0x0a, 0x5c, 0xc4)
        }),
        // Stage-end: green checkmark prefix
        LogLevel::StageEnd => ("    \u{2713}", if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x3f, 0xb9, 0x50)
        } else {
            egui::Color32::from_rgb(0x1a, 0x7f, 0x37)
        }),
        LogLevel::Warn => ("  warn", egui::Color32::from_rgb(0xd2, 0x9e, 0x22)),
        LogLevel::Error => (" error", ui.visuals().error_fg_color),
        // Stdout/stderr from the simulation process, shown dimmer since they
        // are pass-through output rather than structured log events.
        LogLevel::Stdout => ("stdout", ui.visuals().weak_text_color()),
        LogLevel::Stderr => ("stderr", ui.visuals().warn_fg_color),
        // Trace-level: Rumoca internal tracing output (only present when the
        // tracing checkbox is enabled).
        LogLevel::Trace => (" trace", ui.visuals().weak_text_color()),
    }
}
