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

use std::collections::{BTreeMap, HashMap, HashSet};

use eframe::egui;
use serde_json::Value;

use crate::bridge::Seg;
use crate::worker::{is_def_id_key, DefInfo};

// Green highlight for values that changed from the previous stage.
// Using a fixed color rather than a theme color because the "changed" semantic
// is specific to the cross-stage diff and needs to stand out from normal text
// in both light and dark themes.
const CHANGED_COLOR: egui::Color32 = crate::colors::OK_GREEN;

/// What the user asked the tree to do this frame.
///
/// Bundled rather than passed as four separate `&mut Option<_>` out-parameters:
/// `tree_ui` was already at ten arguments and `node_ui` at thirteen, both under
/// `#[allow(clippy::too_many_arguments)]`, and adding `track` would have made a
/// transposable signature worse. Each field is `None` unless the user acted.
#[derive(Default)]
pub struct TreeActions {
    /// Left-click capture — the key-path of a node to explain.
    pub capture: Option<Vec<Seg>>,
    /// "Go to definition" — a class name to navigate to.
    pub nav_to: Option<String>,
    /// "Show this being set" — the key-path to arm a breakpoint for.
    pub debug: Option<Vec<Seg>>,
    /// "Track this identifier" — reverse tracking (idea #37) from any stage.
    pub track: Option<String>,
}

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
/// * `actions` — output: what the user asked for this frame (see [`TreeActions`])
/// * `def_index` — lookup table mapping numeric DefIds to their resolved names,
///   so `type_def_id: 27579` renders with an inline annotation like
///   `-> model Modelica.Mechanics.Rotational.Inertia`
pub fn tree_ui(
    ui: &mut egui::Ui,
    root_label: &str,
    value: &Value,
    prev: Option<&Value>,
    actions: &mut TreeActions,
    def_index: &BTreeMap<u64, DefInfo>,
    field_help: &HashMap<String, String>,
    tracked: Option<&str>,
) {
    let mut path: Vec<Seg> = Vec::new();
    let expand = tracked.map(|t| {
        let mut set = HashSet::new();
        collect_tracked_ancestors(value, t, &mut set);
        set
    });
    node_ui(ui, 0, root_label, value, prev, &mut path, actions, def_index, field_help, tracked, expand.as_ref());
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
#[allow(clippy::too_many_arguments)]
fn node_ui(
    ui: &mut egui::Ui,
    salt: usize,
    key: &str,
    value: &Value,
    prev: Option<&Value>,
    path: &mut Vec<Seg>,
    actions: &mut TreeActions,
    def_index: &BTreeMap<u64, DefInfo>,
    field_help: &HashMap<String, String>,
    tracked: Option<&str>,
    expand: Option<&HashSet<*const Value>>,
) {
    let should_expand = expand.is_some_and(|set| set.contains(&(value as *const Value)));
    ui.push_id(salt, |ui| match value {
        Value::Object(map) => {
            let hint = format!("{{{}}}", map.len());
            let is_tracked = tracked.is_some_and(|t| key == t);
            let resp = egui::CollapsingHeader::new(
                if is_tracked { header_tracked(key, &hint) } else { header(key, &hint) }
            )
                .default_open(should_expand)
                .show(ui, |ui| {
                    for (i, (k, v)) in map.iter().enumerate() {
                        path.push(Seg::Key(k.clone()));
                        node_ui(ui, i, k, v, prev.and_then(|p| p.get(k)), path, actions, def_index, field_help, tracked, expand);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                actions.capture = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, actions, &format!("{key} {hint}"), None, None);
            if let Some(doc) = field_help.get(key) {
                resp.header_response.clone().on_hover_text(doc);
            }
        }
        Value::Array(arr) => {
            let hint = format!("[{}]", arr.len());
            let resp = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(should_expand)
                .show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        path.push(Seg::Index(i));
                        node_ui(ui, i, &i.to_string(), v, prev.and_then(|p| p.get(i)), path, actions, def_index, field_help, tracked, expand);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                actions.capture = Some(path.to_vec());
            }
            row_menu(&resp.header_response, path, actions, &format!("{key} {hint}"), None, None);
        }
        scalar => {
            let changed = prev.is_some_and(|p| p != scalar);
            let is_tracked = tracked.is_some_and(|t| {
                match scalar {
                    // Prose fields are excluded: a tracked name occurring inside
                    // human-written text is a coincidence, not a mention.
                    Value::String(s) if !is_prose_field(key) => {
                        s == t || crate::identifier_index::matches_tracked(s, t)
                    }
                    _ => false,
                }
            });
            let (resp, copy_text) = leaf_ui(ui, key, scalar, def_index, changed, is_tracked);
            if resp.clicked() {
                actions.capture = Some(path.to_vec());
            }
            row_menu(&resp, path, actions, &copy_text, nav_target(key, scalar, def_index), trackable_name(key, scalar));
            if let Some(doc) = field_help.get(key) {
                resp.clone().on_hover_text(doc);
            }
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
    actions: &mut TreeActions,
    copy_text: &str,
    nav: Option<String>,
    track: Option<String>,
) {
    resp.context_menu(|ui| {
        // Don't wrap menu labels — widen the menu to fit long "Go to <name>"
        // items (fully-qualified Modelica type names get long).
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        if ui.button("🔎 Capture").clicked() {
            actions.capture = Some(path.to_vec());
            ui.close();
        }
        // Reverse tracking (idea #37) from any stage. The tree is the one view
        // present on every stage tab, so offering it here is what makes
        // "where did this come from?" an ambient gesture rather than a
        // per-view feature.
        if let Some(name) = &track
            && ui
                .button(format!("\u{25ce} Track {name}"))
                .on_hover_text(
                    "Highlight this identifier across every stage, and show \
                     where it is declared in the specimen source.",
                )
                .clicked()
        {
            actions.track = Some(name.clone());
            ui.close();
        }
        if ui
            .button("🐞 Show this being set (debugger)")
            .on_hover_text("Capture this field so Claude can arm a breakpoint at where Rumoca sets it.")
            .clicked()
        {
            actions.debug = Some(path.to_vec());
            ui.close();
        }
        if let Some(name) = &nav
            && ui.button(format!("↪ Go to {name}")).clicked()
        {
            actions.nav_to = Some(name.clone());
            ui.close();
        }
        if ui.button("📋 Copy text").clicked() {
            ui.ctx().copy_text(copy_text.to_owned());
            ui.close();
        }
    });
}

/// The identifier a leaf names, if it names one.
///
/// Offered as "Track …" in the row menu. Deliberately conservative: the tree
/// renders every string in the IR, most of which are not variable names, and a
/// Track action on a description or a file path would be noise that tracks
/// nothing.
///
/// Accepts a flat variable name — dot-separated identifier components, possibly
/// wrapped in `der(…)` — and nothing else. Prose fields are excluded for the
/// same reason they are excluded from tracked highlighting (see
/// [`is_prose_field`]).
fn trackable_name(key: &str, value: &Value) -> Option<String> {
    if is_prose_field(key) {
        return None;
    }
    let Value::String(s) = value else { return None };
    let bare = s.strip_prefix("der(").and_then(|r| r.strip_suffix(')')).unwrap_or(s);
    if bare.is_empty() {
        return None;
    }
    let is_name = bare.split('.').all(|part| {
        !part.is_empty()
            && part.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    });
    is_name.then(|| s.clone())
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
    is_tracked: bool,
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

    if is_tracked {
        let fill = crate::colors::TRACKED_FILL;
        let rect = resp.rect.expand2(egui::vec2(3.0, 1.0));
        ui.painter().set(bg, egui::Shape::rect_filled(rect, 2.0, fill));
    } else if resp.hovered() {
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

// Single O(N) pre-pass: collect pointers to every Value node that is an
// ancestor of a leaf matching `tracked`. During rendering, nodes in this
// set get `default_open(true)` so the path to the tracked identifier is
// fully expanded without re-walking the subtree at every level.
fn collect_tracked_ancestors<'a>(
    value: &'a Value,
    tracked: &str,
    ancestors: &mut HashSet<*const Value>,
) -> bool {
    let dominated = match value {
        Value::Object(map) => map.iter().any(|(k, v)| {
            if k == tracked {
                return true;
            }
            // A prose field's string is human text, not code — see
            // `is_prose_field`. Its contents must not drag the whole subtree
            // open as though the identifier were mentioned there.
            if is_prose_field(k) && matches!(v, Value::String(_)) {
                return false;
            }
            collect_tracked_ancestors(v, tracked, ancestors)
        }),
        Value::Array(arr) => arr
            .iter()
            .any(|v| collect_tracked_ancestors(v, tracked, ancestors)),
        Value::String(s) => s == tracked || crate::identifier_index::matches_tracked(s, tracked),
        _ => false,
    };
    if dominated {
        ancestors.insert(value as *const Value);
    }
    dominated
}

/// IR fields whose string values are prose written for a human, not code.
///
/// Tracked-identifier matching is a whole-word text search, which cannot tell a
/// mention from a coincidence. In code-bearing strings — equation text, variable
/// names — an occurrence *is* a mention. In prose it is not:
///
/// ```modelica
/// Real h "height of h";
/// ```
///
/// Tracking `h` used to highlight that description and expand the path to it,
/// claiming the variable is used somewhere it is only talked about. This is the
/// same false positive the lexer removed from the source view, but the fix here
/// is different: these strings are not Modelica, so tokenizing them would be a
/// category error. What matters is which *field* the string came from.
///
/// Deliberately short. Listing a field wrongly hides real matches, which is the
/// worse failure — so a field is added only when its contents are certainly
/// prose. `unit` and `quantity` are omitted on purpose: they hold code-like
/// values (`"N.m"`), and `matches_tracked` already treats `.` as a word
/// character, so they do not produce false positives.
const PROSE_FIELDS: &[&str] = &["description", "comment", "file_name"];

fn is_prose_field(key: &str) -> bool {
    PROSE_FIELDS.contains(&key)
}

fn header_tracked(key: &str, hint: &str) -> egui::RichText {
    egui::RichText::new(format!("{key}  {hint}"))
        .monospace()
        .color(crate::colors::TRACKED_GOLD)
        .strong()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- nav_target ---

    #[test]
    fn nav_target_returns_none_for_non_def_id_key() {
        let index = BTreeMap::new();
        assert!(nav_target("name", &json!(42), &index).is_none());
    }

    #[test]
    fn nav_target_returns_none_for_missing_id() {
        let index = BTreeMap::new();
        assert!(nav_target("type_def_id", &json!(999), &index).is_none());
    }

    #[test]
    fn nav_target_returns_none_for_non_class() {
        let mut index = BTreeMap::new();
        index.insert(42, DefInfo {
            name: "someVar".into(),
            kind: crate::worker::DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        });
        assert!(nav_target("def_id", &json!(42), &index).is_none());
    }

    #[test]
    fn nav_target_returns_class_name() {
        let mut index = BTreeMap::new();
        index.insert(100, DefInfo {
            name: "Modelica.Mechanics.Rotational.Inertia".into(),
            kind: crate::worker::DefKind::Class,
            class_type: Some("model".into()),
            file_name: None,
            line: None,
        });
        assert_eq!(
            nav_target("type_def_id", &json!(100), &index).as_deref(),
            Some("Modelica.Mechanics.Rotational.Inertia"),
        );
    }

    #[test]
    fn nav_target_works_for_base_def_id() {
        let mut index = BTreeMap::new();
        index.insert(7, DefInfo {
            name: "Base.Model".into(),
            kind: crate::worker::DefKind::Class,
            class_type: Some("model".into()),
            file_name: None,
            line: None,
        });
        assert_eq!(nav_target("base_def_id", &json!(7), &index).as_deref(), Some("Base.Model"));
    }

    #[test]
    fn nav_target_returns_none_for_non_numeric_value() {
        let mut index = BTreeMap::new();
        index.insert(1, DefInfo {
            name: "X".into(),
            kind: crate::worker::DefKind::Class,
            class_type: Some("model".into()),
            file_name: None,
            line: None,
        });
        assert!(nav_target("def_id", &json!("not a number"), &index).is_none());
    }

    // --- def_annotation ---

    #[test]
    fn def_annotation_returns_none_for_non_def_key() {
        let index = BTreeMap::new();
        assert!(def_annotation("name", &json!(42), &index).is_none());
    }

    #[test]
    fn def_annotation_returns_label_for_class() {
        let mut index = BTreeMap::new();
        index.insert(55, DefInfo {
            name: "Modelica.SIunits.Angle".into(),
            kind: crate::worker::DefKind::Class,
            class_type: Some("type".into()),
            file_name: None,
            line: None,
        });
        assert_eq!(
            def_annotation("type_def_id", &json!(55), &index).as_deref(),
            Some("type Modelica.SIunits.Angle"),
        );
    }

    #[test]
    fn def_annotation_returns_label_for_definition() {
        let mut index = BTreeMap::new();
        index.insert(10, DefInfo {
            name: "someComponent".into(),
            kind: crate::worker::DefKind::Definition,
            class_type: None,
            file_name: None,
            line: None,
        });
        assert_eq!(
            def_annotation("def_id", &json!(10), &index).as_deref(),
            Some("someComponent"),
        );
    }

    // --- collect_tracked_ancestors ---

    #[test]
    fn collect_tracked_ancestors_finds_string_leaf() {
        let tree = json!({"a": {"b": "target"}});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "target", &mut set);
        assert!(found);
        assert!(set.contains(&(&tree as *const Value)));
    }

    #[test]
    fn collect_tracked_ancestors_finds_nested_match() {
        let inner = json!({"name": "h"});
        let tree = json!({"components": [inner]});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "h", &mut set);
        assert!(found);
        assert!(set.len() >= 2, "root and at least one intermediate should be in the set");
    }

    #[test]
    fn collect_tracked_ancestors_returns_false_when_absent() {
        let tree = json!({"a": 1, "b": "other"});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "missing", &mut set);
        assert!(!found);
        assert!(set.is_empty());
    }

    #[test]
    fn collect_tracked_ancestors_matches_object_key() {
        let tree = json!({"h": 42});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "h", &mut set);
        assert!(found);
    }

    #[test]
    fn collect_tracked_ancestors_uses_matches_tracked_for_strings() {
        let tree = json!({"eq": "der(h) - v"});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "h", &mut set);
        assert!(found, "should match 'h' inside 'der(h) - v' via matches_tracked");
    }

    /// `Real h "height of h"` must not read as a use of `h`.
    ///
    /// The description is prose about the variable, not code referring to it,
    /// so tracking `h` must neither highlight it nor expand the path to it.
    #[test]
    fn collect_tracked_ancestors_ignores_prose_fields() {
        for field in PROSE_FIELDS {
            let tree = json!({ *field: "height of h" });
            let mut set = HashSet::new();
            let found = collect_tracked_ancestors(&tree, "h", &mut set);
            assert!(!found, "{field} is prose; a name inside it is not a mention");
            assert!(set.is_empty(), "{field} must not drag its ancestors open");
        }
    }

    /// The Track action is offered on names, not on every string the tree
    /// renders — a Track item on a description or a file path would track
    /// nothing and be pure noise.
    #[test]
    fn trackable_name_accepts_only_variable_names() {
        let s = |v: &str| json!(v);
        assert_eq!(trackable_name("name", &s("h")).as_deref(), Some("h"));
        assert_eq!(
            trackable_name("name", &s("gear.flange_a.tau")).as_deref(),
            Some("gear.flange_a.tau")
        );
        // Derivatives are names too — `strip_der` reduces them when tracking.
        assert_eq!(trackable_name("name", &s("der(h)")).as_deref(), Some("der(h)"));

        // Prose is excluded for the same reason it is excluded from matching.
        assert!(trackable_name("description", &s("height")).is_none());
        // Not names.
        assert!(trackable_name("text", &s("der(h) - v")).is_none());
        assert!(trackable_name("text", &s("height of h")).is_none());
        assert!(trackable_name("text", &s("")).is_none());
        assert!(trackable_name("text", &s("9lives")).is_none());
        assert!(trackable_name("text", &s("a..b")).is_none());
        // Non-strings offer nothing to track.
        assert!(trackable_name("count", &json!(42)).is_none());
    }

    /// The exclusion is by field, not by content — code-bearing strings must
    /// still match, or tracking stops working where it matters most.
    #[test]
    fn collect_tracked_ancestors_still_matches_code_fields() {
        let tree = json!({ "equation": "der(h) - v", "description": "height of h" });
        let mut set = HashSet::new();
        assert!(
            collect_tracked_ancestors(&tree, "h", &mut set),
            "the equation still mentions h even though the description is ignored"
        );
    }

    #[test]
    fn collect_tracked_ancestors_rejects_substring_in_key() {
        let tree = json!({"height": 1.0});
        let mut set = HashSet::new();
        let found = collect_tracked_ancestors(&tree, "h", &mut set);
        assert!(!found, "key 'height' should not match tracked 'h'");
    }

    // --- header / header_tracked ---

    #[test]
    fn header_formats_key_and_hint() {
        let rt = header("components", "{5}");
        assert_eq!(rt.text(), "components  {5}");
    }

    #[test]
    fn header_tracked_formats_key_and_hint() {
        let rt = header_tracked("h", "[3]");
        assert_eq!(rt.text(), "h  [3]");
    }
}
