//! The generic serde-value tree inspector — the primary IR exploration widget.
//!
//! ## Design decision (Charter §4.4 / Decision 6)
//!
//! Instead of building a bespoke tree widget for each compiler phase (one for
//! the parsed AST, another for resolved IR, etc.), there is ONE generic tree
//! that renders *any* `serde_json::Value`. Every Rumoca IR type implements
//! `serde::Serialize`, so we convert each phase's output to a `Value` and
//! hand it to this tree. The tree knows nothing about Rumoca types — it just
//! renders JSON objects as collapsible nodes, arrays as indexed lists, and
//! scalars as colored leaves.
//!
//! **Why `serde_json::Value`?** It is the "universal IR" — any Rust struct
//! that derives `Serialize` can be converted to it via `serde_json::to_value`.
//! This means the tree inspector automatically supports new IR types and new
//! compiler phases without any code changes.
//!
//! ## Click/capture flow
//!
//! Every row in the tree is interactive:
//!
//! - **Left-click** a row to "capture" it — this records the node's *key-path*
//!   (its address from the stage root, like `components.inertia.type_def_id`)
//!   into the `ask` output parameter. The app then writes a bridge focus file
//!   (see `bridge.rs`) describing what was captured. The user asks their actual
//!   question in the Claude Code chat; this tree never asks anything itself.
//!
//! - **Right-click** opens a context menu with: Capture, Show-in-debugger,
//!   Go-to-definition (for DefId-typed fields), and Copy-text.
//!
//! The key-path is accumulated as the recursive walk descends: each level
//! pushes its segment (`Seg::Key` or `Seg::Index`) onto a `Vec<Seg>` path,
//! and pops it when returning. This keeps the tree entirely type-agnostic
//! while still giving Claude an exact JSON-path address.
//!
//! ## Cross-stage diff highlighting
//!
//! The `prev` parameter carries the *previous* stage's IR at the same path.
//! When a leaf value differs from `prev` (e.g., `def_id` going from `null`
//! to a real id between Parse and Resolve), it is painted green. This makes
//! it visually obvious what each compiler phase changed.

use std::collections::BTreeMap;

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::worker::{is_def_id_key, DefInfo};

// Green highlight for values that changed from the previous stage.
// Using a fixed color rather than a theme color because the "changed" semantic
// is specific to the cross-stage diff and needs to stand out from normal text
// in both light and dark themes.
const CHANGED_COLOR: egui::Color32 = crate::colors::OK_GREEN;

/// Render a `serde_json::Value` as a collapsible tree widget.
///
/// This is the top-level entry point. It creates an empty key-path and begins
/// the recursive descent into `node_ui`.
///
/// # Parameters
///
/// * `ui` — egui drawing context
/// * `root_label` — label shown at the tree's root node (e.g., the stage name)
/// * `value` — the IR to render, as a generic JSON value
/// * `prev` — the previous stage's IR for diff highlighting (`None` if no
///   previous stage exists, e.g., for Parse which is the first stage)
/// * `ask` — output: set to a `Vec<Seg>` key-path when the user captures a node
/// * `nav_to` — output: set to a class name when the user clicks "Go to definition"
/// * `debug` — output: set when the user wants to arm a debugger breakpoint
/// * `def_index` — lookup table mapping numeric DefIds to their resolved names,
///   so `type_def_id: 27579` renders with an inline annotation like
///   `-> model Modelica.Mechanics.Rotational.Inertia`
pub fn tree_ui(
    ui: &mut egui::Ui,
    root_label: &str,
    value: &Value,
    prev: Option<&Value>,
    ask: &mut Option<Vec<Seg>>,
    nav_to: &mut Option<String>,
    debug: &mut Option<Vec<Seg>>,
    def_index: &BTreeMap<u64, DefInfo>,
    open_path: Option<&[Seg]>,
) {
    // Start with an empty path — the root. Each recursive call to `node_ui`
    // pushes/pops one segment as it descends into children.
    let mut path: Vec<Seg> = Vec::new();
    node_ui(ui, 0, root_label, value, prev, &mut path, ask, nav_to, debug, def_index, open_path);
}

// Render one node of the JSON tree recursively.
//
// This is the heart of the tree inspector. It pattern-matches on the three
// `serde_json::Value` variants that have children (Object, Array) vs leaves
// (everything else), and recurses into children.
//
// ## egui id management
//
// egui identifies widgets by id. If two sibling nodes have the same label
// (e.g., two array elements both labeled "0"), their CollapsingHeaders would
// collide. `push_id(salt, ...)` wraps each node in a unique id scope using
// the sibling index as salt, preventing collisions.
//
// ## The path accumulation pattern
//
// `path` is a mutable Vec that acts as a stack. Before recursing into a child:
//   path.push(Seg::Key("field_name"))   // or Seg::Index(i) for arrays
// After returning:
//   path.pop()
// At any point during the walk, `path` holds the complete address from the
// root to the current node. When the user clicks, we snapshot it (`path.to_vec()`).
//
// ## Interaction contract
//
// Each row must be "interacted" (sense clicks) exactly ONCE. Interacting twice
// causes egui to register the widget id twice, which breaks click detection.
// For Objects/Arrays, the CollapsingHeader already senses clicks (it's the
// clickable header). For scalars, `leaf_ui` creates a sensed Label. Neither
// caller adds a second `.interact()` call.

/// True when this node's current `path` is a strict prefix of `open_path`
/// (i.e. the node is an ancestor of the navigation target and should be
/// forced open so the target becomes visible).
fn should_force_open(path: &[Seg], open_path: Option<&[Seg]>) -> bool {
    let Some(target) = open_path else { return false };
    if path.len() >= target.len() { return false; }
    path.iter().zip(target.iter()).all(|(a, b)| match (a, b) {
        (Seg::Key(ka), Seg::Key(kb)) => ka == kb,
        (Seg::Index(ia), Seg::Index(ib)) => ia == ib,
        _ => false,
    })
}

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
    open_path: Option<&[Seg]>,
) {
    let force_open = should_force_open(path, open_path);
    ui.push_id(salt, |ui| match value {
        Value::Object(map) => {
            let hint = format!("{{{}}}", map.len());
            let mut ch = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(false);
            if force_open {
                ch = ch.open(Some(true));
            }
            let resp = ch.show(ui, |ui| {
                    for (i, (k, v)) in map.iter().enumerate() {
                        path.push(Seg::Key(k.clone()));
                        node_ui(ui, i, k, v, prev.and_then(|p| p.get(k)), path, ask, nav_to, debug, def_index, open_path);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, ask, &format!("{key} {hint}"), None, nav_to, debug);
        }
        Value::Array(arr) => {
            let hint = format!("[{}]", arr.len());
            let mut ch = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(false);
            if force_open {
                ch = ch.open(Some(true));
            }
            let resp = ch.show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        path.push(Seg::Index(i));
                        node_ui(ui, i, &i.to_string(), v, prev.and_then(|p| p.get(i)), path, ask, nav_to, debug, def_index, open_path);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, ask, &format!("{key} {hint}"), None, nav_to, debug);
        }
        // --- Scalar (null, bool, number, string): render as a leaf row ---
        scalar => {
            // A value is "changed" if the previous stage had a *different* value
            // at the same path. New-to-this-stage paths (prev is None) don't
            // count — we only highlight deliberate mutations, not new fields.
            let changed = prev.is_some_and(|p| p != scalar);
            // `leaf_ui` already interacts once; do NOT interact again here.
            let (resp, copy_text) = leaf_ui(ui, key, scalar, def_index, changed);
            // Leaves don't expand/collapse, so left-click is a fast path to
            // capture (no ambiguity with expand/collapse as with headers).
            if resp.clicked() {
                *ask = Some(path.to_vec());
            }
            row_menu(&resp, path, ask, &copy_text, nav_target(key, scalar, def_index), nav_to, debug);
        }
    });
}

// Check whether a scalar leaf is a DefId that resolves to a *class* definition.
// If so, return the class's fully-qualified Modelica name (e.g.,
// "Modelica.Mechanics.Rotational.Inertia") — this becomes the "Go to
// definition" navigation target. If the DefId resolves to something that isn't
// a class (a variable, a function), navigation doesn't apply and we return None.
fn nav_target(key: &str, scalar: &Value, def_index: &BTreeMap<u64, DefInfo>) -> Option<String> {
    // Only fields whose name ends with `_def_id` or similar carry DefIds.
    if !is_def_id_key(key) {
        return None;
    }
    // Look up the numeric id in the def_index (populated by the worker from
    // Rumoca's resolver output). `as_u64()` returns None for non-number values.
    let info = def_index.get(&scalar.as_u64()?)?;
    // Only class definitions are navigable — you can "go to" a class, but
    // not to a variable or built-in.
    (info.kind == crate::worker::DefKind::Class).then(|| info.name.clone())
}

// Right-click context menu for any tree row.
//
// This provides the "Capture", "Show in debugger", "Go to definition", and
// "Copy text" actions. It is attached to the row's Response via
// `resp.context_menu(...)`, which egui shows on right-click.
//
// `resp` must already sense clicks (interacted exactly once by the caller).
// Interacting the same Response twice in a frame makes egui lose track of the
// widget id, breaking click detection. This is why the menu function takes a
// reference to an already-interacted Response rather than creating its own.
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
        if let Some(name) = &nav
            && ui.button(format!("↪ Go to {name}")).clicked()
        {
            *nav_to = Some(name.clone());
            ui.close();
        }
        if ui.button("📋 Copy text").clicked() {
            ui.ctx().copy_text(copy_text.to_owned());
            ui.close();
        }
    });
}

// Render a single leaf (scalar) row: "key: value", with per-type coloring.
//
// ## Rendering approach
//
// Uses an egui `LayoutJob` — a rich-text builder that lets different parts of
// one label have different colors. The key is rendered in the normal text color,
// the value is colored by JSON type (strings = hyperlink blue, null = dim, etc.),
// and the resolved DefId annotation (if any) is in weak text.
//
// ## Hover highlight
//
// To make rows feel clickable, we paint a hover-highlight rectangle *behind*
// the text. The trick: `painter.add(Shape::Noop)` reserves a paint-order slot
// before the text is drawn; after layout, if the row is hovered, we fill that
// slot with a colored rectangle via `painter.set(bg, ...)`. This ensures the
// highlight is behind the text, not on top of it.
//
// ## Return value
//
// Returns `(Response, copy_text)` — the Response for click detection by the
// caller, and a plain-text string for the "Copy text" context menu action.
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

// If this leaf is a `def_id`, `type_def_id`, or `base_def_id` field whose
// numeric value is in the def_index, return a human-readable label like
// "model Modelica.Mechanics.Rotational.Inertia". This annotation is displayed
// inline after the numeric value, turning opaque integers into meaningful names.
fn def_annotation(key: &str, scalar: &Value, def_index: &BTreeMap<u64, DefInfo>) -> Option<String> {
    if !is_def_id_key(key) {
        return None;
    }
    // `DefInfo::label` returns "kind name" (e.g., "model Inertia").
    def_index.get(&scalar.as_u64()?).map(DefInfo::label)
}

// Build the collapsing-header title text: monospace "key  {hint}", where hint
// is a size indicator like "{5}" for objects or "[3]" for arrays.
fn header(key: &str, hint: &str) -> egui::RichText {
    egui::RichText::new(format!("{key}  {hint}")).monospace()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::Seg;

    fn k(s: &str) -> Seg {
        Seg::Key(s.to_string())
    }

    fn i(n: usize) -> Seg {
        Seg::Index(n)
    }

    #[test]
    fn force_open_none_target() {
        assert!(!should_force_open(&[k("a")], None));
    }

    #[test]
    fn force_open_empty_path_is_prefix() {
        assert!(should_force_open(&[], Some(&[k("classes"), k("Gear")])));
    }

    #[test]
    fn force_open_strict_prefix() {
        let target = [k("classes"), k("Gear"), k("equations")];
        assert!(should_force_open(&[k("classes")], Some(&target)));
        assert!(should_force_open(&[k("classes"), k("Gear")], Some(&target)));
    }

    #[test]
    fn force_open_exact_match_is_not_prefix() {
        let target = [k("classes"), k("Gear")];
        assert!(!should_force_open(&[k("classes"), k("Gear")], Some(&target)));
    }

    #[test]
    fn force_open_longer_path_is_not_prefix() {
        let target = [k("classes")];
        assert!(!should_force_open(&[k("classes"), k("Gear")], Some(&target)));
    }

    #[test]
    fn force_open_mismatch() {
        let target = [k("classes"), k("Gear")];
        assert!(!should_force_open(&[k("components")], Some(&target)));
    }

    #[test]
    fn force_open_index_segments() {
        let target = [k("equations"), i(2)];
        assert!(should_force_open(&[k("equations")], Some(&target)));
        assert!(!should_force_open(&[k("equations"), i(0)], Some(&target)));
    }

    #[test]
    fn force_open_mixed_seg_types_never_match() {
        let target = [k("0")];
        assert!(!should_force_open(&[i(0)], Some(&target)));
    }
}
