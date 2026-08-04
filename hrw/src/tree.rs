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
//! Every row in the tree is interactive. Nothing here asks anything — the tree
//! records *what the user acted on* into [`TreeActions`], the app turns that
//! into a bridge focus file (see `bridge.rs`), and the user asks their actual
//! question in the Claude Code chat.
//!
//! - **Left-click** a row to point at it — recording the node's *key-path*, its
//!   address from the stage root, like `components.inertia.type_def_id`.
//! - **Right-click** opens a context menu: Point at, Follow (for names the model
//!   knows), Show-in-debugger, Go-to-definition (for DefId fields *and* for
//!   variable names, via `TreeOptions::declaring_classes`), and Copy-text.
//!
//! Inputs the tree needs — what is tracked, which names are real variables, how
//! to expand — arrive as [`TreeOptions`]. Both are bundles rather than long
//! parameter lists; see their docs for why.
//!
//! **Vocabulary note:** the user-facing verbs are "Point at" and "Follow"
//! (renamed 2026-07-28). The code's nouns — `TreeActions::capture`,
//! `TreeActions::track`, `focus.json` — deliberately stay, because they are
//! also the wire format Claude reads, and renaming a protocol buys nothing.
//! Expect the two vocabularies to differ here; that is intended, not drift.
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
    /// "Follow this identifier" — reverse tracking (idea #37) from any stage.
    pub track: Option<String>,
}

/// What the tree knows, and how it should present itself.
///
/// Bundled for the same reason as [`TreeActions`] — these would otherwise be
/// three more positional arguments on an already-long signature.
#[derive(Default, Clone, Copy)]
pub struct TreeOptions<'a> {
    /// The identifier being tracked, highlighted wherever it is mentioned.
    pub tracked: Option<&'a str>,
    /// Every variable name in the compiled model.
    ///
    /// **The ground truth for what is trackable.** The first attempt decided
    /// syntactically — any string shaped like a dotted identifier — which marked
    /// `causality: "None"`, `op: "Add"`, and `quantity: "Angle"` as trackable
    /// and offered to track them. Roughly half the marks were meaningless, and
    /// when everything is marked nothing is: that over-marking *was* the
    /// discoverability problem. A name is trackable when the model actually has
    /// a variable by that name, and nothing else will do.
    ///
    /// `None` (no successful compile yet) means nothing is offered — a wrong
    /// offer is worse than no offer.
    pub known_variables: Option<&'a HashSet<String>>,
    /// Variable name -> the class that declares it, when that is not the
    /// specimen. Lets "Go to definition" work from a variable name, not only
    /// from a DefId field.
    pub declaring_classes: Option<&'a HashMap<String, String>>,
    /// Scroll this node into view and open everything above it, for one frame.
    ///
    /// The "jump to the followed identifier" control. Set for a single frame:
    /// forcing the ancestors open also *stores* that state in egui, so once the
    /// jump has happened the headers stay open on their own and the user can
    /// collapse them again. Held longer, it would pin the scroll and take the
    /// headers out of the user's hands — the complaint that sank "Reveal
    /// identifiers" as a mode.
    pub jump_to: Option<&'a [Seg]>,
    /// The row a node link pointed at, washed in [`colors::JUMP_FILL`] so it is
    /// findable after the scroll.
    ///
    /// Distinct from `jump_to`, which lasts one frame and only scrolls. Highlighting on
    /// that alone would flash for a single frame; this outlives it.
    pub highlight: Option<&'a [Seg]>,
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
    opts: TreeOptions<'_>,
) {
    let mut path: Vec<Seg> = Vec::new();
    let mut expansion = Expansion::default();
    if let Some(t) = opts.tracked {
        collect_tracked_ancestors(value, t, &mut expansion.default_open);
    }
    // Every ancestor of the jump target, so the row exists to be scrolled to.
    // `force_open`, not `default_open`: the whole point is to open headers the
    // user has already collapsed, and `default_open` is ignored once egui has
    // remembered a header's state.
    if let Some(target) = opts.jump_to {
        let mut node = value;
        expansion.force_open.insert(node as *const Value);
        for seg in target {
            let Some(next) = seg.get_in(node) else { break };
            expansion.force_open.insert(next as *const Value);
            node = next;
        }
    }
    node_ui(ui, 0, root_label, value, prev, &mut path, actions, def_index, field_help, opts, &expansion);
}

/// Which nodes to open, and how firmly.
///
/// The distinction is not cosmetic. `CollapsingHeader::default_open` applies
/// only the **first** time a header is shown; once egui has stored that
/// header's state — frame one — it is ignored. So a set computed later cannot
/// move anything through `default_open`. Forcing with `open(Some(true))` works,
/// but **writes** the open state into egui's memory, which is why forcing is
/// rationed to things that last one frame.
///
/// **"Reveal identifiers" was the counter-example, removed 2026-08-04.** It
/// forced open every path to any variable and stayed on until unticked — and
/// unticking could not put the tree back, because the forcing had already
/// overwritten what the user had collapsed. Doug: *"if I check the box to reveal
/// identifiers, I can't uncheck the box to restore a tree to the condition it
/// had been in before checking the box."* The rule it produced is in
/// `DECISIONS.md`: **a view option must not mutate state the user owns.**
///
/// So `force_open` now serves only `jump_to`, which lasts a single frame:
/// afterwards the headers are open on their own and the user may collapse them.
/// Tracking only *suggests* via `default_open`, because it persists and you must
/// still be able to collapse things while it is on.
#[derive(Default)]
struct Expansion {
    /// Opened if the header has no remembered state — the path to a tracked
    /// identifier.
    default_open: HashSet<*const Value>,
    /// Opened regardless of remembered state. **Only `jump_to` uses this now**,
    /// and only for one frame — see the note above on why forcing is rationed.
    force_open: HashSet<*const Value>,
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
    opts: TreeOptions<'_>,
    expansion: &Expansion,
) {
    let force_open = expansion.force_open.contains(&(value as *const Value));
    let should_expand = force_open || expansion.default_open.contains(&(value as *const Value));
    // Compared by path, not by pointer: the jump target comes from
    // `bridge::mention_paths`, which addresses nodes rather than holding
    // references to them. `path` here is already this node's own path.
    let is_jump_target = opts.jump_to.is_some_and(|target| target == path.as_slice());
    let is_highlighted = opts.highlight.is_some_and(|target| target == path.as_slice());
    // A washed row has to be drawn *behind* the widget, and egui has no row
    // background — so the highlight is a painted rect sized to the row after the fact.
    // Cheaper and more reliable than restyling every widget kind the tree can emit.
    let row_top = ui.cursor().top();
    let painted = ui.push_id(salt, |ui| match value {
        Value::Object(map) => {
            let hint = format!("{{{}}}", map.len());
            let is_tracked = opts.tracked.is_some_and(|t| key == t);
            let resp = egui::CollapsingHeader::new(
                if is_tracked { header_tracked(key, &hint) } else { header(key, &hint) }
            )
                .default_open(should_expand)
                // `open` overrides remembered state; `default_open` cannot.
                .open(force_open.then_some(true))
                .show(ui, |ui| {
                    for (i, (k, v)) in map.iter().enumerate() {
                        path.push(Seg::Key(k.clone()));
                        node_ui(ui, i, k, v, prev.and_then(|p| p.get(k)), path, actions, def_index, field_help, opts, expansion);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                actions.capture = Some(path.to_vec());
            }
            scroll_if_jump_target(is_jump_target, &resp.header_response);
            row_menu(&resp.header_response, path, actions, &format!("{key} {hint}"), None, None);
            if let Some(doc) = field_help.get(key) {
                resp.header_response.clone().on_hover_text(doc);
            }
        }
        Value::Array(arr) => {
            let hint = format!("[{}]", arr.len());
            let resp = egui::CollapsingHeader::new(header(key, &hint))
                .default_open(should_expand)
                .open(force_open.then_some(true))
                .show(ui, |ui| {
                    for (i, v) in arr.iter().enumerate() {
                        path.push(Seg::Index(i));
                        node_ui(ui, i, &i.to_string(), v, prev.and_then(|p| p.get(i)), path, actions, def_index, field_help, opts, expansion);
                        path.pop();
                    }
                });
            if resp.header_response.clicked() {
                actions.capture = Some(path.to_vec());
            }
            scroll_if_jump_target(is_jump_target, &resp.header_response);
            row_menu(&resp.header_response, path, actions, &format!("{key} {hint}"), None, None);
        }
        scalar => {
            let changed = prev.is_some_and(|p| p != scalar);
            let is_tracked = opts.tracked.is_some_and(|t| {
                match scalar {
                    // Prose fields are excluded: a tracked name occurring inside
                    // human-written text is a coincidence, not a mention.
                    Value::String(s) if !crate::identifier_index::is_prose_field(key) => {
                        crate::identifier_index::same_variable(s, t)
                            || crate::source_view::mentions_identifier(s, t)
                    }
                    _ => false,
                }
            });
            let (resp, copy_text) = leaf_ui(ui, key, scalar, def_index, changed, is_tracked, &opts);
            if resp.clicked() {
                actions.capture = Some(path.to_vec());
            }
            scroll_if_jump_target(is_jump_target, &resp);
            let trackable = trackable_name(key, scalar, &opts);
            row_menu(&resp, path, actions, &copy_text, nav_target(key, scalar, def_index, &opts), trackable.clone());
            // Explain the underline. Appended to the field's own help rather
            // than replacing it, so discoverability does not cost the
            // documentation that is already there.
            resp.clone().on_hover_text(row_hover(field_help.get(key), trackable.as_deref()));
        }
    });

    // Wash the row a node link pointed at. Painted *behind* what was just drawn, using
    // the vertical span the row actually occupied — a header with its children open
    // covers many lines, and washing all of them would drown the tree, so only the
    // header's own line is marked.
    if is_highlighted {
        let row = egui::Rect::from_min_max(
            egui::pos2(ui.min_rect().left(), row_top),
            egui::pos2(ui.min_rect().right(), row_top + ui.spacing().interact_size.y),
        );
        ui.painter().rect_filled(row, 2.0, crate::colors::JUMP_FILL);
    }
    let _ = painted;
}

/// Bring a jumped-to row into view.
///
/// **Centred**, not merely made visible. A match scrolled to the very bottom
/// edge is technically on screen and practically still lost — which is exactly
/// how "Reveal identifiers" failed: the node was revealed and the user still
/// could not find it. That checkbox was removed on 2026-08-04; this is the
/// behaviour that replaced it.
fn scroll_if_jump_target(is_target: bool, resp: &egui::Response) {
    if is_target {
        resp.scroll_to_me(Some(egui::Align::Center));
    }
}

/// What a tree row's tooltip should say.
///
/// Three things, in decreasing generality, and the order matters: the field's
/// own Rumoca documentation first (it is what a reader most often wants), then
/// what a left-click will do, then what a right-click additionally offers.
///
/// **Appending rather than replacing** is the point. Discoverability must not
/// cost the documentation already there — field help is the app's fast, no-AI
/// tier, and burying it under interaction hints would trade a real answer for a
/// hint about how to ask for one.
///
/// Every row is left-clickable, so the "point at" line is unconditional. Only
/// rows naming a variable the model actually knows can be followed, so that
/// line depends on `trackable`.
fn row_hover(doc: Option<&String>, trackable: Option<&str>) -> String {
    let mut out = String::new();
    if let Some(doc) = doc {
        out.push_str(doc);
        out.push_str("

");
    }
    out.push_str(crate::POINT_AT_HOVER);
    if let Some(name) = trackable {
        out.push_str(&format!("

Right-click to follow {name} through every stage."));
    }
    out
}

// Check whether a scalar leaf is a DefId that resolves to a *class* definition.
// If so, return the class's fully-qualified Modelica name (e.g.,
// "Modelica.Mechanics.Rotational.Inertia") — this becomes the "Go to
// definition" navigation target. If the DefId resolves to something that isn't
// a class (a variable, a function), navigation doesn't apply and we return None.
fn nav_target(
    key: &str,
    scalar: &Value,
    def_index: &BTreeMap<u64, DefInfo>,
    opts: &TreeOptions<'_>,
) -> Option<String> {
    // Fields whose name ends with `_def_id` or similar carry DefIds.
    if is_def_id_key(key) {
        // Look up the numeric id in the def_index (populated by the worker from
        // Rumoca's resolver output). `as_u64()` returns None for non-number values.
        if let Some(info) = def_index.get(&scalar.as_u64()?) {
            // Only class definitions are navigable — you can "go to" a class,
            // but not to a variable or built-in.
            return (info.kind == crate::worker::DefKind::Class)
                .then(|| info.name.clone());
        }
        return None;
    }
    // A variable name is navigable too, when the model says which class
    // declares it: `src.V` goes to `src`'s type. Same menu item, same
    // navigation stack — the vocabulary stays consistent whether you found the
    // class through a DefId field or through the variable itself.
    let name = trackable_name(key, scalar, opts)?;
    opts.declaring_classes?.get(crate::identifier_index::strip_der(&name)).cloned()
}

// Right-click context menu for any tree row.
//
// This provides the "Point at", "Follow", "Show in debugger", "Go to
// definition", and "Copy text" actions. It is attached to the row's Response via
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
        // "Point at", not "Capture". The user-facing verbs name the two ways a
        // question's subject gets assembled — pointing at one node, following
        // one identifier — rather than naming what the app does to a file. See
        // `docs/context-assembly.md`. The internal names (`actions.capture`,
        // `Focus`, the wire format) are deliberately unchanged: renaming those
        // would churn the emitted contract for a vocabulary change.
        if ui
            .button("\u{1f3af} Point at")
            .on_hover_text(
                "Make this node the subject of your next question, then ask in the chat.",
            )
            .clicked()
        {
            actions.capture = Some(path.to_vec());
            ui.close();
        }
        // Reverse tracking (idea #37) from any stage. The tree is the one view
        // present on every stage tab, so offering it here is what makes
        // "where did this come from?" an ambient gesture rather than a
        // per-view feature.
        if let Some(name) = &track
            && ui
                // Both glyphs in this menu are ones the app already renders.
                // egui ships far less than the whole of Unicode, and an
                // unproven codepoint shows as a tofu box — which U+2715 did in
                // the bar this menu feeds. The magnifier sits with "Follow"
                // because following *is* a search: the identifier is sought in
                // every stage, and where it is absent is as much the point as
                // where it is found.
                .button(format!("\u{1f50e} Follow {name}"))
                .on_hover_text(
                    "Follow this identifier through every stage: highlight it \
                     wherever it appears, count where it does not, and show \
                     where it is declared in the specimen source.",
                )
                .clicked()
        {
            actions.track = Some(name.clone());
            ui.close();
        }
        if ui
            .button("🐞 Show this being set (debugger)")
            .on_hover_text(
                "Point at this field so Claude can arm a breakpoint where Rumoca sets it.",
            )
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
/// Offered as "Follow …" in the row menu. Deliberately conservative: the tree
/// renders every string in the IR, most of which are not variable names, and a
/// Track action on a description or a file path would be noise that tracks
/// nothing.
///
/// Accepts a flat variable name — dot-separated identifier components, possibly
/// wrapped in `der(…)` — and nothing else. Prose fields are excluded for the
/// same reason they are excluded from tracked highlighting (see
/// [`is_prose_field`]).
fn trackable_name(key: &str, value: &Value, opts: &TreeOptions<'_>) -> Option<String> {
    if crate::identifier_index::is_prose_field(key) {
        return None;
    }
    let Value::String(s) = value else { return None };
    let known = opts.known_variables?;
    let bare = crate::identifier_index::strip_der(s);
    (known.contains(bare) || known.contains(s.as_str())).then(|| s.clone())
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
    opts: &TreeOptions<'_>,
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

    // Underline values that name a variable, so the "Track" action in the row
    // menu is discoverable without hunting for it. Underline rather than colour
    // because colour in this row is fully committed already — value type, green
    // for changed, gold fill for tracked — whereas underline is free, and it
    // already means "clickable identifier" in the specimen source view. Same
    // vocabulary, no new one to learn.
    let mut value_fmt = fmt(color);
    if trackable_name(key, scalar, opts).is_some() {
        value_fmt.underline = egui::Stroke::new(1.0, color.gamma_multiply(0.6));
    }
    job.append(&value, 0.0, value_fmt);
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
fn collect_tracked_ancestors(
    value: &Value,
    tracked: &str,
    ancestors: &mut HashSet<*const Value>,
) -> bool {
    // `fold`, not `any`: `any` short-circuits at the first matching child, so
    // later siblings were never visited and their ancestors never recorded —
    // the tree opened the path to the *first* mention only. Every child must be
    // walked for every path to be openable.
    let dominated = match value {
        Value::Object(map) => map.iter().fold(false, |found, (k, v)| {
            let here = if k == tracked {
                true
            } else if crate::identifier_index::is_prose_field(k) && matches!(v, Value::String(_)) {
                // A prose field's string is human text, not code — see
                // `is_prose_field`. Its contents must not drag the whole subtree
                // open as though the identifier were mentioned there.
                false
            } else {
                collect_tracked_ancestors(v, tracked, ancestors)
            };
            found || here
        }),
        Value::Array(arr) => arr.iter().fold(false, |found, v| {
            let here = collect_tracked_ancestors(v, tracked, ancestors);
            found || here
        }),
        Value::String(s) => crate::identifier_index::same_variable(s, tracked)
            || crate::source_view::mentions_identifier(s, tracked),
        _ => false,
    };
    if dominated {
        ancestors.insert(value as *const Value);
    }
    dominated
}


/// A tree header for a node on the followed identifier's path — the key plus a
/// short hint, in the follow colour. Distinct from a *pointed-at* row, which is
/// one row for one link rather than a thread through every stage.
fn header_tracked(key: &str, hint: &str) -> egui::RichText {
    egui::RichText::new(format!("{key}  {hint}"))
        .monospace()
        .color(crate::colors::TRACKED_GOLD)
        .strong()
}

#[cfg(test)]
mod tests {
    /// Interaction hints must never cost the field documentation.
    ///
    /// The field help is HRW's fast, no-AI tier — the answer a reader most
    /// often wants. Replacing it with a hint about how to ask a question would
    /// trade a real answer for directions to one.
    #[test]
    fn row_hover_appends_to_field_help_rather_than_replacing_it() {
        let doc = "The variable's causality: input, output, or none.".to_owned();

        let with_doc = row_hover(Some(&doc), None);
        assert!(with_doc.starts_with(&doc), "documentation comes first: {with_doc}");
        assert!(with_doc.contains("Point at"), "and the gesture is still named: {with_doc}");

        // Every row is left-clickable, so the point-at line is unconditional.
        let bare = row_hover(None, None);
        assert!(bare.contains("Point at"), "{bare}");
        assert!(!bare.contains("Right-click"), "nothing to follow here: {bare}");

        // Only rows naming a known variable offer the second verb.
        let followable = row_hover(Some(&doc), Some("emf.phi"));
        assert!(followable.starts_with(&doc));
        assert!(followable.contains("Point at"));
        assert!(followable.contains("follow emf.phi"), "{followable}");
    }

    use super::*;
    use serde_json::json;

    // --- nav_target ---

    /// A variable name navigates to the class that declares it, using the same
    /// menu item as a DefId field — so "Go to definition" means one thing
    /// whether you found the class through a DefId or through the variable.
    #[test]
    fn nav_target_resolves_a_variable_to_its_declaring_class() {
        let index = BTreeMap::new();
        let known: HashSet<String> = ["src.V"].iter().map(|s| (*s).to_owned()).collect();
        let declaring: HashMap<String, String> = [(
            "src.V".to_owned(),
            "Modelica.Electrical.Analog.Sources.ConstantVoltage".to_owned(),
        )].into_iter().collect();
        let opts = TreeOptions {
            known_variables: Some(&known),
            declaring_classes: Some(&declaring),
            ..Default::default()
        };

        assert_eq!(
            nav_target("name", &json!("src.V"), &index, &opts).as_deref(),
            Some("Modelica.Electrical.Analog.Sources.ConstantVoltage")
        );
        // A derivative resolves through its base variable.
        assert_eq!(
            nav_target("name", &json!("der(src.V)"), &index, &opts).as_deref(),
            Some("Modelica.Electrical.Analog.Sources.ConstantVoltage")
        );
        // Specimen-declared variables have no declaring *class* — the source
        // view is where they are found, not the navigation stack.
        assert!(nav_target("name", &json!("h"), &index, &opts).is_none());
    }

    #[test]
    fn nav_target_returns_none_for_non_def_id_key() {
        let index = BTreeMap::new();
        assert!(nav_target("name", &json!(42), &index, &TreeOptions::default()).is_none());
    }

    #[test]
    fn nav_target_returns_none_for_missing_id() {
        let index = BTreeMap::new();
        assert!(nav_target("type_def_id", &json!(999), &index, &TreeOptions::default()).is_none());
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
        assert!(nav_target("def_id", &json!(42), &index, &TreeOptions::default()).is_none());
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
            nav_target("type_def_id", &json!(100), &index, &TreeOptions::default()).as_deref(),
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
        assert_eq!(nav_target("base_def_id", &json!(7), &index, &TreeOptions::default()).as_deref(), Some("Base.Model"));
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
        assert!(nav_target("def_id", &json!("not a number"), &index, &TreeOptions::default()).is_none());
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
    fn collect_tracked_ancestors_finds_mentions_lexically() {
        let mut set = HashSet::new();
        assert!(
            collect_tracked_ancestors(&json!({"eq": "der(h) - v"}), "h", &mut set),
            "`der(h) - v` mentions h"
        );
        // The point of using the lexer rather than a substring search: `h` is
        // part of `height`, not a mention of it.
        set.clear();
        assert!(
            !collect_tracked_ancestors(&json!({"eq": "height - v"}), "h", &mut set),
            "`height` is one identifier, not a mention of h"
        );
    }

    /// `Real h "height of h"` must not read as a use of `h`.
    ///
    /// The description is prose about the variable, not code referring to it,
    /// so tracking `h` must neither highlight it nor expand the path to it.
    #[test]
    fn collect_tracked_ancestors_ignores_prose_fields() {
        for field in crate::identifier_index::PROSE_FIELDS {
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
    /// Trackability is decided by the model, not by the shape of the string.
    ///
    /// The first implementation accepted anything that *looked* like a dotted
    /// identifier, which marked `causality: "None"`, `op: "Add"`, and
    /// `quantity: "Angle"` as trackable — roughly half the marks in a real IR
    /// were meaningless, and when everything is marked nothing is.
    #[test]
    fn trackable_name_requires_a_real_variable() {
        let known: HashSet<String> = ["h", "gear.flange_a.tau"]
            .iter().map(|s| (*s).to_owned()).collect();
        let opts = TreeOptions { known_variables: Some(&known), ..Default::default() };
        let s = |v: &str| json!(v);

        assert_eq!(trackable_name("name", &s("h"), &opts).as_deref(), Some("h"));
        assert_eq!(
            trackable_name("name", &s("gear.flange_a.tau"), &opts).as_deref(),
            Some("gear.flange_a.tau")
        );
        // A derivative of a known variable is trackable; `strip_der` reduces it.
        assert_eq!(trackable_name("name", &s("der(h)"), &opts).as_deref(), Some("der(h)"));

        // Identifier-shaped, but not variables of this model — these are the
        // ones the syntactic version wrongly offered.
        assert!(trackable_name("causality", &s("None"), &opts).is_none());
        assert!(trackable_name("op", &s("Add"), &opts).is_none());
        assert!(trackable_name("quantity", &s("Angle"), &opts).is_none());
        assert!(trackable_name("kind", &s("scalar"), &opts).is_none());

        // Prose is excluded even when it happens to name a variable.
        assert!(trackable_name("description", &s("h"), &opts).is_none());
        // Non-strings offer nothing to track.
        assert!(trackable_name("count", &json!(42), &opts).is_none());
    }

    /// With no compiled model there is no ground truth, so nothing is offered —
    /// a wrong offer is worse than no offer.
    #[test]
    fn trackable_name_offers_nothing_without_a_model() {
        let opts = TreeOptions::default();
        assert!(trackable_name("name", &json!("h"), &opts).is_none());
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
