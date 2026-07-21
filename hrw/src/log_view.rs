//! The compilation log view — a scrollable panel showing timestamped events
//! streamed from the worker thread during a compile.

use eframe::egui;

use crate::worker::{LogEntry, LogLevel};

/// Returns `true` if the tracing checkbox was toggled this frame.
pub fn ui(ui: &mut egui::Ui, entries: &[LogEntry], tracing_enabled: &mut bool) -> bool {
    let mut toggled = false;
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

    if entries.is_empty() {
        ui.weak("Select a specimen to see its compilation log.");
        return toggled;
    }

    egui::ScrollArea::vertical()
        .id_salt("log_view")
        .auto_shrink(false)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for entry in entries {
                let (prefix, color) = level_style(entry, ui);
                ui.horizontal(|ui| {
                    ui.monospace(format!("{:>8.1}ms", entry.elapsed_secs * 1000.0));
                    ui.colored_label(color, egui::RichText::new(prefix).monospace());
                    ui.colored_label(color, egui::RichText::new(&entry.message).monospace());
                });
            }
        });
    toggled
}

fn level_style<'a>(entry: &LogEntry, ui: &egui::Ui) -> (&'a str, egui::Color32) {
    match entry.level {
        LogLevel::Info => ("  info", ui.visuals().text_color()),
        LogLevel::StageStart => ("    \u{25b6}", if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x58, 0xa6, 0xff)
        } else {
            egui::Color32::from_rgb(0x0a, 0x5c, 0xc4)
        }),
        LogLevel::StageEnd => ("    \u{2713}", if ui.visuals().dark_mode {
            egui::Color32::from_rgb(0x3f, 0xb9, 0x50)
        } else {
            egui::Color32::from_rgb(0x1a, 0x7f, 0x37)
        }),
        LogLevel::Warn => ("  warn", egui::Color32::from_rgb(0xd2, 0x9e, 0x22)),
        LogLevel::Error => (" error", ui.visuals().error_fg_color),
        LogLevel::Stdout => ("stdout", ui.visuals().weak_text_color()),
        LogLevel::Stderr => ("stderr", ui.visuals().warn_fg_color),
        LogLevel::Trace => (" trace", ui.visuals().weak_text_color()),
    }
}
