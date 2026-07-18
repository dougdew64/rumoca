//! The generic serde-value tree inspector.
//!
//! Charter §4.4 / Decision 6: one generic serde-value tree inspector, pointed
//! at every pipeline stage's IR — not per-stage bespoke tree widgets. It walks
//! a `serde_json::Value` (into which any Serialize IR has been converted), so
//! it knows nothing about Rumoca types and works unchanged for every arc.

use eframe::egui;
use serde_json::Value;

/// Render a serde value as a collapsible tree under the given root label.
pub fn tree_ui(ui: &mut egui::Ui, root_label: &str, value: &Value) {
    node_ui(ui, 0, root_label, value);
}

/// Render one node. `salt` disambiguates sibling widget ids so repeated field
/// names or array indices never collide in egui's id space.
fn node_ui(ui: &mut egui::Ui, salt: usize, key: &str, value: &Value) {
    ui.push_id(salt, |ui| match value {
        Value::Object(map) => {
            egui::CollapsingHeader::new(header(key, &format!("{{{}}}", map.len())))
                .default_open(false)
                .show(ui, |ui| {
                    for (i, (k, v)) in map.iter().enumerate() {
                        node_ui(ui, i, k, v);
                    }
                });
        }
        Value::Array(arr) => {
            egui::CollapsingHeader::new(header(key, &format!("[{}]", arr.len())))
                .default_open(false)
                .show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        node_ui(ui, i, &i.to_string(), v);
                    }
                });
        }
        scalar => leaf_ui(ui, key, scalar),
    });
}

/// A leaf (scalar) row: `key: value`, the value colored by type.
fn leaf_ui(ui: &mut egui::Ui, key: &str, scalar: &Value) {
    let visuals = ui.visuals();
    let (text, color) = match scalar {
        Value::Null => ("null".to_owned(), visuals.weak_text_color()),
        Value::Bool(b) => (b.to_string(), visuals.text_color()),
        Value::Number(n) => (n.to_string(), visuals.text_color()),
        Value::String(s) => (format!("{s:?}"), visuals.hyperlink_color),
        // Objects/arrays never reach here.
        other => (other.to_string(), visuals.text_color()),
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{key}:")).monospace());
        ui.label(egui::RichText::new(text).monospace().color(color));
    });
}

/// A collapsing-header title: bold key plus a dim size/shape hint.
fn header(key: &str, hint: &str) -> egui::RichText {
    egui::RichText::new(format!("{key}  {hint}")).monospace()
}
