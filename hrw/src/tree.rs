//! The generic serde-value tree inspector.
//!
//! Charter §4.4 / Decision 6: one generic serde-value tree inspector, pointed
//! at every pipeline stage's IR — not per-stage bespoke tree widgets. It walks
//! a `serde_json::Value` (into which any Serialize IR has been converted), so
//! it knows nothing about Rumoca types and works unchanged for every arc.
//!
//! Each row carries a "Capture this for a question" affordance (left-click, or
//! the right-click menu). It never asks anything here — it records the clicked
//! node's *key-path* (its address from the stage root) into `ask`, which the app
//! turns into a bridge focus file (see `bridge`); the user then asks in the chat.
//! The path is accumulated as the walk descends, so the tree stays type-agnostic
//! while still handing Claude an exact address.

use std::collections::BTreeMap;

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::worker::{is_def_id_key, DefInfo};

/// Value color for a leaf whose value differs from the *previous* stage's value
/// at the same path — the "this stage changed it" highlight (e.g. def_id going
/// null→id at Resolve, type_id sentinel→id at Typecheck). Reads on light & dark.
const CHANGED_COLOR: egui::Color32 = egui::Color32::from_rgb(0x3F, 0xB9, 0x50);

/// Render a serde value as a collapsible tree under the given root label. A
/// right-click "Ask Claude about this" on any row sets `ask` to that node's
/// key-path from the root. `def_index` resolves `def_id`/`type_def_id` leaves to
/// the definition they name, shown inline; "Go to definition" on a class DefId
/// sets `nav_to` to that class's qualified name. `prev` is the previous stage's
/// IR aligned to the same root; leaf values that differ from it (at the same
/// path, and only where the path exists in both) are painted green.
pub fn tree_ui(
    ui: &mut egui::Ui,
    root_label: &str,
    value: &Value,
    prev: Option<&Value>,
    ask: &mut Option<Vec<Seg>>,
    nav_to: &mut Option<String>,
    debug: &mut Option<Vec<Seg>>,
    def_index: &BTreeMap<u64, DefInfo>,
) {
    let mut path: Vec<Seg> = Vec::new();
    node_ui(ui, 0, root_label, value, prev, &mut path, ask, nav_to, debug, def_index);
}

/// Render one node. `salt` disambiguates sibling widget ids so repeated field
/// names or array indices never collide in egui's id space. `path` holds this
/// node's address from the root (its own segment already pushed by the caller).
#[allow(clippy::too_many_arguments)]
fn node_ui(
    ui: &mut egui::Ui,
    salt: usize,
    key: &str,
    value: &Value,
    prev: Option<&Value>,
    path: &mut Vec<Seg>,
    ask: &mut Option<Vec<Seg>>,
    nav_to: &mut Option<String>,
    debug: &mut Option<Vec<Seg>>,
    def_index: &BTreeMap<u64, DefInfo>,
) {
    ui.push_id(salt, |ui| match value {
        Value::Object(map) => {
            let hint = format!("{{{}}}", map.len());
            let resp = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(false)
                .show(ui, |ui| {
                    for (i, (k, v)) in map.iter().enumerate() {
                        path.push(Seg::Key(k.clone()));
                        node_ui(ui, i, k, v, prev.and_then(|p| p.get(k)), path, ask, nav_to, debug, def_index);
                        path.pop();
                    }
                });
            // The header response already senses clicks (it IS the clickable
            // header) — do NOT re-interact it (that double-registers its id and
            // muddies its own click handling). Left-click captures the container
            // too, in addition to the header's built-in expand/collapse.
            if resp.header_response.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, ask, &format!("{key} {hint}"), None, nav_to, debug);
        }
        Value::Array(arr) => {
            let hint = format!("[{}]", arr.len());
            let resp = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(false)
                .show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        path.push(Seg::Index(i));
                        node_ui(ui, i, &i.to_string(), v, prev.and_then(|p| p.get(i)), path, ask, nav_to, debug, def_index);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, ask, &format!("{key} {hint}"), None, nav_to, debug);
        }
        scalar => {
            // Changed by this stage = the same path existed in the previous
            // stage with a different value (new-to-this-stage paths don't count).
            let changed = prev.is_some_and(|p| p != scalar);
            // `leaf_ui` already interacts once; do NOT interact again here.
            let (resp, copy_text) = leaf_ui(ui, key, scalar, def_index, changed);
            // Leaves don't expand, so left-click is a fast path to capture.
            if resp.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp, path, ask, &copy_text, nav_target(key, scalar, def_index), nav_to, debug);
        }
    });
}

/// If a leaf is a `DefId` that resolves to a *class*, that class's qualified
/// name (the navigation target); otherwise `None`.
fn nav_target(key: &str, scalar: &Value, def_index: &BTreeMap<u64, DefInfo>) -> Option<String> {
    if !is_def_id_key(key) {
        return None;
    }
    let info = def_index.get(&scalar.as_u64()?)?;
    (info.kind == "class").then(|| info.name.clone())
}

/// Right-click menu for any row. `resp` must already sense clicks (interacted
/// exactly once by the caller — interacting twice makes egui drop the id).
/// When `nav` is `Some(qualified_name)`, offers "Go to definition".
#[allow(clippy::too_many_arguments)]
fn row_menu(
    resp: &egui::Response,
    path: &[Seg],
    ask: &mut Option<Vec<Seg>>,
    copy_text: &str,
    nav: Option<String>,
    nav_to: &mut Option<String>,
    debug: &mut Option<Vec<Seg>>,
) {
    resp.context_menu(|ui| {
        // Don't wrap menu labels — widen the menu to fit long "Go to <name>"
        // items (fully-qualified Modelica type names get long).
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        if ui.button("🔎 Capture").clicked() {
            *ask = Some(path.to_vec());
            ui.close();
        }
        if ui
            .button("🐞 Show this being set (debugger)")
            .on_hover_text("Capture this field so Claude can arm a breakpoint at where Rumoca sets it.")
            .clicked()
        {
            *debug = Some(path.to_vec());
            ui.close();
        }
        if let Some(name) = &nav {
            if ui.button(format!("↪ Go to {name}")).clicked() {
                *nav_to = Some(name.clone());
                ui.close();
            }
        }
        if ui.button("📋 Copy text").clicked() {
            ui.ctx().copy_text(copy_text.to_owned());
            ui.close();
        }
    });
}

/// A leaf (scalar) row: `key: value`, the value colored by type. When the leaf
/// is a resolved `DefId`, its resolved definition is shown inline. The row
/// senses clicks (interacted exactly once) and highlights on hover, so it reads
/// as interactive — every row carries "Ask Claude about this". Returns the row
/// response and the plain text for the Copy action.
fn leaf_ui(
    ui: &mut egui::Ui,
    key: &str,
    scalar: &Value,
    def_index: &BTreeMap<u64, DefInfo>,
    changed: bool,
) -> (egui::Response, String) {
    // Reserve a paint slot up front so the hover highlight draws *behind* the
    // text rather than over it (shapes added later paint on top).
    let bg = ui.painter().add(egui::Shape::Noop);

    let visuals = ui.visuals();
    let (value, base_color) = match scalar {
        Value::Null => ("null".to_owned(), visuals.weak_text_color()),
        Value::Bool(b) => (b.to_string(), visuals.text_color()),
        Value::Number(n) => (n.to_string(), visuals.text_color()),
        Value::String(s) => (format!("{s:?}"), visuals.hyperlink_color),
        // Objects/arrays never reach here.
        other => (other.to_string(), visuals.text_color()),
    };
    // A value changed from the previous stage is highlighted green.
    let color = if changed { CHANGED_COLOR } else { base_color };
    let key_color = ui.visuals().text_color();
    let weak_color = ui.visuals().weak_text_color();

    let resolved = def_annotation(key, scalar, def_index);
    let copy_text = match &resolved {
        Some(label) => format!("{key}: {value}  → {label}"),
        None => format!("{key}: {value}"),
    };

    // Render the whole row as ONE click-sensing Label (a real widget, so it is
    // reliably hit-tested — a bare `ui.horizontal(...).response.interact(...)`
    // was not registering pointer events). A LayoutJob keeps the per-part
    // colors: key, value (typed), and the resolved-DefId annotation.
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let fmt = |c| egui::text::TextFormat { font_id: mono.clone(), color: c, ..Default::default() };
    let mut job = egui::text::LayoutJob::default();
    job.append(&format!("{key}: "), 0.0, fmt(key_color));
    job.append(&value, 0.0, fmt(color));
    if let Some(label) = &resolved {
        job.append(&format!("  → {label}"), 0.0, fmt(weak_color));
    }

    let resp = ui.add(egui::Label::new(job).sense(egui::Sense::click()).extend());

    if resp.hovered() {
        let fill = ui.visuals().widgets.hovered.weak_bg_fill;
        let rect = resp.rect.expand2(egui::vec2(3.0, 1.0));
        ui.painter().set(bg, egui::Shape::rect_filled(rect, 2.0, fill));
    }
    (resp, copy_text)
}

/// If this leaf is a resolved `DefId` field, the label of the definition it
/// names (e.g. `model Modelica.…Inertia`); otherwise `None`.
fn def_annotation(key: &str, scalar: &Value, def_index: &BTreeMap<u64, DefInfo>) -> Option<String> {
    if !is_def_id_key(key) {
        return None;
    }
    def_index.get(&scalar.as_u64()?).map(DefInfo::label)
}

/// A collapsing-header title: bold key plus a dim size/shape hint.
fn header(key: &str, hint: &str) -> egui::RichText {
    egui::RichText::new(format!("{key}  {hint}")).monospace()
}
