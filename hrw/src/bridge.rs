//! The Claude bridge — the app's communication channel to Claude Code.
//!
//! ## Architecture: thin emitter, thick reasoner
//!
//! When the user captures a node (clicks in the tree inspector or a custom view)
//! and then asks a question in the Claude Code chat, this module writes a single
//! JSON **focus file** (`focus.json`) describing what the user is looking at:
//!
//! - Which specimen (`.mo` file)
//! - Which pipeline stage (Parse, Resolve, Typecheck, etc.)
//! - Which IR node (by key-path from the stage root)
//! - The node's source provenance (which Modelica source line it came from)
//! - A cross-stage diff (how the node differs between Parse and Resolve)
//! - A DefId resolution table (mapping numeric ids to human-readable names)
//!
//! The focus file carries **no answers** and embeds **no language model**. It is
//! a pure description of context. The reasoning happens entirely in the Claude
//! Code session, which reads the focus file along with the specimen source, the
//! staged IR files, the Rumoca phase code, and Doug's `docs/compiler-phases`.
//! This "thin emitter, thick reasoner" split is documented in DECISIONS.md.
//!
//! ## The JSON file protocol
//!
//! The bridge uses the filesystem as the communication channel:
//!
//! 1. **Focus file** (`.hrw-bridge/focus.json`): written on each capture. Contains
//!    the `instructions` (self-describing), `seq` (monotonic counter), `request`
//!    (what the user wants: "explain" or "debug-where-set"), `kind` (node/stage/
//!    specimen), and the node/provenance/cross-stage data.
//!
//! 2. **Stage files** (`.hrw-bridge/stages/<name>.json`): one file per pipeline
//!    stage's full IR, rewritten once per compile. Claude can diff any two stages
//!    by reading two files (e.g., `instantiate.json` vs `typecheck.json`).
//!
//! The `.hrw-bridge/` directory is gitignored. The paths are repo-relative
//! (via `CARGO_MANIFEST_DIR`) so they are stable across Claude Code sessions.
//!
//! ## Span-ascent (source provenance)
//!
//! Rumoca IR nodes carry source provenance (`location` or `span` fields with
//! byte offsets into the Modelica source). However, leaf nodes you typically
//! click (e.g., a bare `"name": "flange_a"`) usually have **no provenance of
//! their own** — the nearest `location`/`span` lives on an ancestor node.
//!
//! So the bridge walks **up** the tree from the clicked node to the root,
//! looking for the tightest enclosing provenance:
//! - `location` (preferred): `rumoca_core::Location` with byte offsets + `file_name`
//! - `span` (fallback): `rumoca_core::Span` with byte offsets + opaque `source` id
//!
//! Once found, the byte range is sliced out of the Modelica source file, and
//! the enclosing line(s) are included for context. This walk is fully generic
//! (it pattern-matches on JSON structure, not Rumoca types), maintaining the
//! one-generic-tree rule.

use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::worker::DefInfo;

/// Path to the bridge directory, resolved at compile time via `CARGO_MANIFEST_DIR`.
///
/// Using `CARGO_MANIFEST_DIR` (the directory containing `Cargo.toml`) makes
/// this path repo-relative and stable regardless of the working directory.
/// The directory is gitignored so focus files don't pollute version control.
pub const BRIDGE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge");

/// Path to the stage-files directory (one JSON file per pipeline stage).
///
/// These files are rewritten once per compile. Claude can diff any two stages
/// by reading two files — e.g., comparing `instantiate.json` to `typecheck.json`
/// shows what the instanced typecheck phase added (type_ids resolved, dimensions
/// evaluated). This avoids bloating the focus file with all stages' IR.
pub const STAGES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/stages");

/// The canonical list of pipeline stage file names written to `.hrw-bridge/stages/`.
///
/// This constant is the single source of truth for which stage files exist.
/// Both the focus JSON (so Claude knows what to read) and `write_stages` callers
/// should reference this list. If a new stage is added to the pipeline, add its
/// filename here — a test (`focus_json_stage_files_match_constant`) will fail if
/// the focus JSON and this list diverge.
pub const STAGE_FILE_NAMES: &[&str] = &[
    "parse.json",
    "resolve.json",
    "instantiate.json",
    "typecheck.json",
    "flatten.json",
    "structural.json",
    "index_reduction.json",
    "initialization.json",
    "events.json",
    "solve_lowering.json",
];

// Maximum size (in bytes) of a node subtree to inline in the focus file.
// Nodes larger than this are described by "shape" only (list of keys, or array
// length). Claude can always re-derive the full subtree from the staged IR file.
// 16KB is generous for typical IR nodes (a single component, equation, etc.)
// but prevents the focus file from exploding on large class definitions.
const MAX_NODE_BYTES: usize = 16 * 1024;

// Self-describing instructions embedded in every focus file. When Doug opens
// `focus.json` directly while dogfooding, this text explains what it is and
// how to use it, without needing to consult separate documentation.
const INSTRUCTIONS: &str = "\
HRW bridge focus file, written by the app when you capture a node/stage/model \
(the 🔎 Capture actions) — capturing does NOT ask anything by itself. Ask your \
question in the Claude Code chat; Claude reads this file to see what you \
captured, then reasons over the specimen source, the staged IR, the Rumoca \
phase code, and docs/compiler-phases.";

/// One segment of a key-path into a JSON tree.
///
/// A key-path is a sequence of `Seg` values that addresses a specific node
/// from the tree root. For example, the path to `components.inertia.def_id`
/// would be `[Key("components"), Key("inertia"), Key("def_id")]`. Array
/// elements use `Index(n)` — e.g., `equations[0]` is `[Key("equations"), Index(0)]`.
///
/// This is the tree-agnostic addressing scheme that lets the bridge, tree
/// inspector, and custom views all refer to the same node without knowing
/// Rumoca types.
#[derive(Clone, Debug)]
pub enum Seg {
    /// An object field name (e.g., "components", "def_id").
    Key(String),
    /// An array index (e.g., 0 for the first element).
    Index(usize),
}

impl fmt::Display for Seg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Seg::Key(k) => write!(f, "{k}"),
            Seg::Index(i) => write!(f, "[{i}]"),
        }
    }
}

impl Seg {
    // Serialize this segment for inclusion in the focus JSON.
    // Keys become JSON strings, indices become JSON numbers.
    fn as_json(&self) -> Value {
        match self {
            Seg::Key(k) => Value::String(k.clone()),
            Seg::Index(i) => json!(i),
        }
    }

    // Navigate one step into a JSON value: `Key("x")` does `v["x"]`,
    // `Index(3)` does `v[3]`. Returns None if the key/index doesn't exist.
    // This is the fundamental building block of `navigate()`.
    fn get<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        match self {
            Seg::Key(k) => v.get(k),
            Seg::Index(i) => v.get(i),
        }
    }
}

/// Format a key-path as a human-readable dotted string.
///
/// Examples:
/// - `[Key("components"), Key("inertia"), Key("def_id")]` -> `"components.inertia.def_id"`
/// - `[Key("equations"), Index(0), Key("Connect")]` -> `"equations[0].Connect"`
/// - `[]` -> `"(tree root)"`
///
/// Used in the UI status bar to show what was captured, and in the bridge
/// focus file as a human-readable path alongside the machine-readable key array.
pub fn describe_path(path: &[Seg]) -> String {
    if path.is_empty() {
        return "(tree root)".to_owned();
    }
    let mut s = String::new();
    for seg in path {
        match seg {
            Seg::Key(_) => {
                if !s.is_empty() {
                    s.push('.');
                }
                write!(s, "{seg}").unwrap();
            }
            Seg::Index(_) => write!(s, "{seg}").unwrap(),
        }
    }
    s
}

/// What the user captured — the "focus" of their question.
///
/// This enum distinguishes three granularity levels:
/// - `Node`: a specific IR node (the most common — user clicked a field in the tree)
/// - `Stage`: the entire stage's IR (user captured from the stage tab header)
/// - `Specimen`: the whole `.mo` file (user captured from the specimen list)
///
/// The lifetime `'a` borrows the IR data from the app's state, avoiding clones
/// of potentially large JSON trees during the focus-file build.
pub enum Focus<'a> {
    /// A specific IR node in the current stage, at `key_path` from the stage root.
    /// `stage_value` is a reference to the stage's full IR (used to navigate to
    /// the node and build provenance).
    Node { key_path: Vec<Seg>, stage_value: &'a Value },
    /// The current stage's IR as a whole.
    Stage,
    /// The whole specimen (the `.mo` file) — captured from the specimen list.
    /// Distinct from the `model` field of `Ask`, which names the compiled class
    /// (a `.mo` file can contain multiple classes).
    Specimen,
}

/// All the context needed to write one focus file.
///
/// This struct aggregates everything the bridge needs from the app's state:
/// the capture target, the current specimen/stage, and the IR data for
/// cross-stage diffing. It borrows everything (lifetime `'a`) to avoid
/// cloning large IR trees.
pub struct Ask<'a> {
    /// Monotonically increasing sequence number — lets Claude detect when a
    /// new capture has occurred since the last question.
    pub seq: u64,
    /// What the user wants: "explain" (default) or "debug-where-set" (they want
    /// to watch this field being assigned in the Rumoca phase, in the debugger).
    pub request: &'a str,
    /// Path to the Modelica source file (`.mo`). `None` if no specimen is loaded.
    pub specimen: Option<&'a Path>,
    /// The class name being compiled (e.g., "RotationalInertia"). A specimen file
    /// can contain multiple classes; this names the one currently viewed.
    pub model: Option<&'a str>,
    /// The pipeline stage name (e.g., "Parse", "Resolve", "Typecheck").
    pub stage: &'a str,
    /// Paths to any Modelica library files used during compilation.
    pub libraries: Vec<String>,
    /// Resolved identity of DefIds in the model's IR. The IR contains numeric
    /// DefIds (e.g., `type_def_id: 27579`); this table maps each to its
    /// human-readable name and kind, so the focus file is self-explanatory.
    pub def_index: &'a BTreeMap<u64, DefInfo>,
    /// The Parse stage's IR (if available). Used for cross-stage diffing.
    pub parse_value: Option<&'a Value>,
    /// The Resolve stage's IR (if available). Used for cross-stage diffing.
    pub resolve_value: Option<&'a Value>,
    /// What the user captured (node, stage, or specimen).
    pub focus: Focus<'a>,
}

/// Write the focus file to `.hrw-bridge/focus.json`.
///
/// Called by the app whenever the user captures a node/stage/specimen.
/// Creates the bridge directory if it doesn't exist. Returns the path
/// on success (used by the app to show a status message).
pub fn write(ask: &Ask) -> std::io::Result<PathBuf> {
    fs::create_dir_all(BRIDGE_DIR)?;
    let path = Path::new(BRIDGE_DIR).join("focus.json");
    let doc = build(ask);
    // Pretty-print for readability — Doug reads these during dogfooding.
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default())?;
    Ok(path)
}

/// Write each stage's full IR to `.hrw-bridge/stages/<name>.json`.
///
/// Called once per compile (not per capture). Each stage's entire IR is
/// written as a separate file so Claude can diff any two stages by reading
/// both files. A stage with no IR (e.g., it failed or doesn't apply) has
/// its file removed, keeping the directory in sync with the current specimen.
pub fn write_stages(stages: &[(&str, Option<&Value>)]) -> std::io::Result<()> {
    fs::create_dir_all(STAGES_DIR)?;
    for (name, value) in stages {
        let path = Path::new(STAGES_DIR).join(format!("{name}.json"));
        match value {
            Some(v) => fs::write(&path, serde_json::to_string_pretty(v).unwrap_or_default())?,
            None => {
                let _ = fs::remove_file(&path);
            }
        }
    }
    Ok(())
}

// Build the complete focus JSON document from an `Ask`.
//
// The document structure:
// {
//   "instructions": <self-describing text>,
//   "seq": <monotonic counter>,
//   "request": "explain" | "debug-where-set",
//   "kind": "node" | "stage" | "specimen",
//   "specimen": <path to .mo file>,
//   "model": <class name>,
//   "stage": <pipeline stage name>,
//   "libraries": [<library paths>],
//   "def_resolutions": { "<id>": { "name": ..., "kind": ... }, ... },
//   "stages": { "dir": ..., "files": [...] },
//   "node": { ... }           // only for kind=node
//   "cross_stage": { ... }    // only for kind=node
// }
fn build(ask: &Ask) -> Value {
    let kind = match ask.focus {
        Focus::Node { .. } => "node",
        Focus::Stage => "stage",
        Focus::Specimen => "specimen",
    };
    let mut doc = json!({
        "instructions": INSTRUCTIONS,
        "seq": ask.seq,
        "request": ask.request,
        "kind": kind,
        "specimen": ask.specimen.map(|p| p.to_string_lossy().into_owned()),
        "model": ask.model,
        "stage": ask.stage,
        "libraries": ask.libraries,
        "def_resolutions": def_resolutions(ask.def_index),
        "stages": {
            "dir": STAGES_DIR,
            "note": "each <name>.json is that stage's FULL IR for the current specimen \
                     (absent if the stage produced none). To diff two stages, read the two \
                     files and compare — e.g. instantiate.json vs typecheck.json shows what the \
                     instanced typecheck added (type_ids resolved, dimensions evaluated).",
            "files": STAGE_FILE_NAMES,
        },
    });
    if let Focus::Node { key_path, stage_value } = &ask.focus {
        doc["node"] = build_node(key_path, stage_value, ask.specimen);
        doc["cross_stage"] = build_cross_stage(ask, key_path);
    }
    doc
}

// Safety limit on the number of scalar changes reported in a cross-stage diff.
// Real diffs are small (a handful of `null -> id` fields); this backstop
// prevents pathological cases from producing an enormous focus file.
const MAX_CHANGES: usize = 400;

// Build the cross-stage diff for a captured node.
//
// This answers "what did Resolve do to this node?" by showing the SAME node
// as it appears in Parse and in Resolve, plus a list of scalar field changes.
//
// ## Class-relative path alignment
//
// Parse and Resolve have different root structures:
// - Parse wraps the class in `{ "classes": { "M": { ... } }, "within": ... }`
// - Resolve extracts the class directly: `{ "def_id": ..., "components": ... }`
//
// To find the same node in both stages, we strip the class prefix: if the
// clicked path starts with `classes.M.`, we drop those two segments to get a
// class-relative path, then navigate from each stage's class subtree.
// `class_subtree()` handles the detection: it returns the class subtree and
// the prefix depth (2 for Parse's wrapped form, 0 for Resolve's flat form).
fn build_cross_stage(ask: &Ask, key_path: &[Seg]) -> Value {
    let Some(model) = ask.model else {
        return json!({ "applicable": false, "reason": "no model name" });
    };
    let current = match ask.stage {
        "Parse" => ask.parse_value,
        "Resolve" => ask.resolve_value,
        _ => None,
    };
    let Some(current) = current else {
        return json!({ "applicable": false, "reason": "cross-stage diff not yet implemented for this stage" });
    };

    // Strip the current stage's class prefix to get the class-relative path.
    let (_, cur_depth) = class_subtree(current, model);
    let rel: &[Seg] = if cur_depth == 0 {
        key_path
    } else if key_path.len() >= cur_depth
        && matches!(&key_path[0], Seg::Key(k) if k == "classes")
        && matches!(&key_path[1], Seg::Key(k) if k == model)
    {
        &key_path[cur_depth..]
    } else {
        return json!({ "applicable": false, "reason": "node is outside the model class" });
    };

    let stage_node = |value: Option<&Value>| -> Value {
        match value {
            Some(v) => {
                let (class, _) = class_subtree(v, model);
                match navigate(class, rel) {
                    Some(n) => {
                        let mut out = capped(n);
                        out["found"] = json!(true);
                        out
                    }
                    None => json!({ "found": false }),
                }
            }
            None => json!({ "found": false, "reason": "stage not available" }),
        }
    };

    let parse_node = stage_node(ask.parse_value);
    let resolve_node = stage_node(ask.resolve_value);

    // Scalar deltas, only when both nodes are present in full.
    let mut changes = Vec::new();
    if let (Some(p), Some(r)) = (parse_node.get("value"), resolve_node.get("value")) {
        let mut path = Vec::new();
        diff(p, r, &mut path, &mut changes);
    }

    json!({
        "applicable": true,
        "note": "the same node before (parse) and after (resolve) name resolution; `changes` lists scalar field deltas",
        "class_relative_path": rel.iter().map(Seg::as_json).collect::<Vec<_>>(),
        "parse": parse_node,
        "resolve": resolve_node,
        "changes": changes,
    })
}

// Find the class subtree within a stage's IR.
//
// Parse wraps the class in a `StoredDefinition`: `{ "classes": { "M": { ... } } }`.
// Resolve extracts the class directly. This function detects which form is
// present and returns (subtree, prefix_depth):
// - Parse: returns (`classes.M` subtree, 2)
// - Resolve: returns (root, 0)
//
// The prefix_depth tells the caller how many segments to strip from the
// clicked path to get a class-relative path.
fn class_subtree<'a>(stage_value: &'a Value, model: &str) -> (&'a Value, usize) {
    if let Some(class) = stage_value.get("classes").and_then(|c| c.get(model)) {
        return (class, 2);
    }
    (stage_value, 0)
}

// Wrap a node for inclusion in the focus file.
// Small nodes (< MAX_NODE_BYTES) are inlined as `{ "value": <node> }`.
// Large nodes are truncated to a shape summary: `{ "truncated": true, "bytes": N, "shape": [...] }`.
// Claude can always get the full node from the staged IR file if needed.
fn capped(node: &Value) -> Value {
    let bytes = serde_json::to_string(node).map(|s| s.len()).unwrap_or(0);
    if bytes <= MAX_NODE_BYTES {
        json!({ "value": node })
    } else {
        json!({ "truncated": true, "bytes": bytes, "shape": shape(node) })
    }
}

// Recursively diff two JSON subtrees, collecting scalar-level changes.
//
// Each change is a `{ "path": "...", "parse": <old>, "resolve": <new> }` record.
// The diff is structural:
// - Objects: diff by key (report added/removed/changed keys)
// - Arrays: diff by index (element-wise comparison)
// - Scalars: report if they differ
//
// `path` is a mutable stack of path segments (same push/pop pattern as tree.rs).
// `out` collects the change records, capped at MAX_CHANGES.
fn diff(a: &Value, b: &Value, path: &mut Vec<String>, out: &mut Vec<Value>) {
    if out.len() >= MAX_CHANGES {
        return;
    }
    match (a, b) {
        (Value::Object(ma), Value::Object(mb)) => {
            for (k, va) in ma {
                path.push(k.clone());
                match mb.get(k) {
                    Some(vb) => diff(va, vb, path, out),
                    None => out.push(json!({ "path": path.join("."), "parse": va, "resolve": null })),
                }
                path.pop();
            }
            for (k, vb) in mb {
                if !ma.contains_key(k) {
                    path.push(k.clone());
                    out.push(json!({ "path": path.join("."), "parse": null, "resolve": vb }));
                    path.pop();
                }
            }
        }
        (Value::Array(aa), Value::Array(ab)) => {
            for i in 0..aa.len().max(ab.len()) {
                path.push(format!("[{i}]"));
                match (aa.get(i), ab.get(i)) {
                    (Some(x), Some(y)) => diff(x, y, path, out),
                    (x, y) => out.push(json!({
                        "path": path.join("."),
                        "parse": x.cloned().unwrap_or(Value::Null),
                        "resolve": y.cloned().unwrap_or(Value::Null),
                    })),
                }
                path.pop();
            }
        }
        _ if a != b => out.push(json!({ "path": path.join("."), "parse": a, "resolve": b })),
        _ => {}
    }
}

// Convert the DefId -> DefInfo lookup table to a JSON object.
// Keys are stringified numeric ids (JSON object keys must be strings).
// This table is included in every focus file so that when Claude sees
// `type_def_id: 27579`, it can immediately look up that 27579 = "model
// Modelica.Mechanics.Rotational.Inertia" without needing to re-run the resolver.
fn def_resolutions(index: &BTreeMap<u64, DefInfo>) -> Value {
    let mut map = serde_json::Map::new();
    for (id, info) in index {
        map.insert(id.to_string(), info.to_json());
    }
    Value::Object(map)
}

// Build the `node` section of the focus file.
//
// Contains:
// - `key_path`: the machine-readable path segments (for Claude to navigate)
// - `subtree`: the node's value (inlined if small, shape if large)
// - `provenance`: the source-code excerpt this node came from (via span-ascent)
fn build_node(key_path: &[Seg], root: &Value, specimen: Option<&Path>) -> Value {
    let key_path_json: Vec<Value> = key_path.iter().map(Seg::as_json).collect();

    let subtree = match navigate(root, key_path) {
        Some(node) => capped(node),
        None => Value::Null,
    };

    json!({
        "key_path": key_path_json,
        "subtree": subtree,
        "provenance": ascend_provenance(root, key_path, specimen),
    })
}

// Navigate a key-path from the root to a specific node.
//
// Returns `None` if any segment in the path doesn't exist (e.g., the key
// is missing or the index is out of bounds). This is the inverse of the
// path-accumulation done during tree traversal.
fn navigate<'a>(root: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path {
        cur = seg.get(cur)?;
    }
    Some(cur)
}

// The span-ascent algorithm: walk from the clicked node up to the root,
// returning the tightest (deepest) enclosing `location` or `span`.
//
// Why "ascent"? The clicked leaf usually has no provenance of its own.
// For example, clicking `"name": "flange_a"` gives a bare string — no
// byte offsets. But its parent (a component object) carries a `location`
// with byte offsets into the Modelica source. By walking up the path from
// leaf to root, we find the nearest ancestor that has provenance.
//
// The loop iterates `depth` from `path.len()` (the leaf) down to 0 (the root).
// At each depth, it navigates to `path[..depth]` and checks for a `location`
// or `span` field. `location` is preferred (it includes a `file_name`);
// `span` is a fallback (it has an opaque `source` id instead of a filename).
fn ascend_provenance(root: &Value, path: &[Seg], specimen: Option<&Path>) -> Value {
    for depth in (0..=path.len()).rev() {
        let Some(Value::Object(map)) = navigate(root, &path[..depth]) else { continue };
        if let Some(loc) = map.get("location").filter(|v| is_location(v)) {
            return provenance(loc, "location", depth, specimen);
        }
        if let Some(span) = map.get("span").filter(|v| is_span(v)) {
            return provenance(span, "span", depth, specimen);
        }
    }
    json!({ "found": false, "note": "no location/span on this node or its ancestors" })
}

// Check if a JSON value is a `rumoca_core::Location` (byte offsets + file_name).
// A Location is the preferred provenance type because it includes the source
// filename, enabling direct file reads for the excerpt.
fn is_location(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("file_name").and_then(Value::as_str).is_some()
}

// Check if a JSON value is a `rumoca_core::Span` (byte offsets + opaque source id).
// A Span is the fallback provenance type — it has byte offsets but the source
// is identified by an opaque `SourceId` rather than a filename, so we fall
// back to using the specimen path to read the source.
fn is_span(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("source").is_some()
}

// Build the provenance result JSON from a found location/span.
//
// Extracts byte offsets, resolves the source file, slices the excerpt,
// and expands to enclosing lines. The result includes:
// - `found: true` / `false`
// - `kind`: "location" or "span"
// - `at_depth`: how many levels up from the leaf the provenance was found
// - `raw`: the original location/span value
// - `byte_range`: [start, end]
// - `file`: resolved source file path
// - `excerpt`: the exact bytes from [start, end)
// - `line_context`: the enclosing full source line(s)
fn provenance(raw: &Value, kind: &str, depth: usize, specimen: Option<&Path>) -> Value {
    let start = raw.get("start").and_then(Value::as_u64).unwrap_or(0) as usize;
    let end = raw.get("end").and_then(Value::as_u64).unwrap_or(0) as usize;
    let file_name = raw.get("file_name").and_then(Value::as_str).unwrap_or("");
    let sliced = slice_source(file_name, specimen, start, end);

    let mut out = json!({
        "found": true,
        "kind": kind,
        "at_depth": depth,
        "raw": raw,
        "byte_range": [start, end],
    });
    match sliced {
        Some((file, excerpt, line_context)) => {
            out["file"] = json!(file);
            out["excerpt"] = json!(excerpt);
            out["line_context"] = json!(line_context);
        }
        None => {
            out["excerpt"] = Value::Null;
            out["note"] = json!("could not resolve/read the source file for these offsets");
        }
    }
    out
}

// Resolve the source file and slice the byte range into an excerpt.
//
// Tries two strategies to find the source file:
// 1. Use `file_name` from a Location (if non-empty and the file exists)
// 2. Fall back to the specimen path (for Spans with opaque source ids)
//
// Then slices `bytes[start..end]` for the exact excerpt, and expands to
// the enclosing full lines for `line_context` (so Claude sees the complete
// Modelica statement, not just a fragment).
//
// Returns `(file_path, excerpt, line_context)` or None if the file can't
// be read or the byte range is invalid.
fn slice_source(
    file_name: &str,
    specimen: Option<&Path>,
    start: usize,
    end: usize,
) -> Option<(String, String, String)> {
    let path = if !file_name.is_empty() && Path::new(file_name).is_file() {
        PathBuf::from(file_name)
    } else {
        specimen?.to_path_buf()
    };
    let src = fs::read_to_string(&path).ok()?;
    let bytes = src.as_bytes();
    if start > end || end > bytes.len() {
        return None;
    }
    let excerpt = String::from_utf8_lossy(&bytes[start..end]).into_owned();
    // Expand to whole containing lines (byte-wise, so we never split a char).
    let line_start = bytes[..start].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1);
    let line_end = bytes[end..].iter().position(|&b| b == b'\n').map_or(bytes.len(), |i| end + i);
    let line_context = String::from_utf8_lossy(&bytes[line_start..line_end]).into_owned();
    Some((path.to_string_lossy().into_owned(), excerpt, line_context))
}

// A compact shape summary for an over-large node that was truncated from
// the focus file. Shows object keys (so Claude knows the node's structure)
// or array length (so Claude knows how many elements). This gives Claude
// enough information to navigate to the full node in the staged IR file.
fn shape(v: &Value) -> Value {
    match v {
        Value::Object(m) => json!(m.keys().cloned().collect::<Vec<_>>()),
        Value::Array(a) => json!(format!("[{} items]", a.len())),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Span-ascent picks the *tightest* enclosing location, and the slice is
    /// expanded to whole source lines. The clicked node is a bare string leaf
    /// with no location of its own — provenance must come from its ancestor.
    #[test]
    fn ascent_finds_tightest_location_and_slices_lines() {
        // Source whose bytes 8..17 are `flange_a` on line 2.
        let src = "model M\n  flange_a x;\nend M;\n";
        let start = src.find("flange_a").unwrap();
        let end = start + "flange_a".len();

        let dir = std::env::var("CARGO_TARGET_TMPDIR")
            .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().into_owned());
        let file = Path::new(&dir).join("hrw_bridge_ascent.mo");
        fs::write(&file, src).unwrap();

        // A tree where the leaf `name` has no location; its parent component does.
        let root = json!({
            "components": [
                {
                    "location": { "start": start, "end": end, "file_name": "M.mo" },
                    "name": "flange_a"
                }
            ]
        });
        let path = vec![Seg::Key("components".into()), Seg::Index(0), Seg::Key("name".into())];

        let prov = ascend_provenance(&root, &path, Some(&file));
        assert_eq!(prov["found"], json!(true));
        assert_eq!(prov["kind"], json!("location"));
        // Ascended one level from the leaf to the component object.
        assert_eq!(prov["at_depth"], json!(2));
        assert_eq!(prov["excerpt"], json!("flange_a"));
        // Line context is the whole line, not just the token.
        assert_eq!(prov["line_context"], json!("  flange_a x;"));
    }

    /// End-to-end over real Rumoca parse IR: every `location`-bearing node's
    /// byte range must slice cleanly out of the specimen source.
    #[test]
    fn provenance_holds_over_real_parsed_specimen() {
        let specimen = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/specimens/RotationalInertia.mo"));
        let source = fs::read_to_string(specimen).expect("read specimen");
        let ast = rumoca_phase_parse::parse_to_ast(&source, "RotationalInertia.mo").expect("parse");
        let root = serde_json::to_value(&ast).expect("to_value");

        // Find a path to some object carrying a real (non-dummy) location.
        let mut path = Vec::new();
        let found = first_location_path(&root, &mut path);
        assert!(found, "expected at least one located node in the parsed AST");

        let prov = ascend_provenance(&root, &path, Some(specimen));
        assert_eq!(prov["found"], json!(true), "provenance: {prov}");
        let excerpt = prov["excerpt"].as_str().expect("excerpt string");
        assert!(source.contains(excerpt), "excerpt {excerpt:?} not found in source");
    }

    /// A node captured in the Resolve tab carries the same node from Parse and
    /// the scalar deltas, even though Parse is rooted at the `StoredDefinition`
    /// (class under `classes.<model>`) and Resolve at the class itself.
    #[test]
    fn cross_stage_diffs_the_same_node_across_roots() {
        // Parse: wrapped in a StoredDefinition, def_ids still null.
        let parse = json!({
            "classes": { "M": { "def_id": null, "components": {
                "c": { "def_id": null, "type_def_id": null }
            }}},
            "within": null
        });
        // Resolve: extracted class, def_ids populated.
        let resolve = json!({
            "def_id": 5,
            "components": { "c": { "def_id": 9, "type_def_id": 100 } }
        });
        let empty = BTreeMap::new();
        let key_path = vec![Seg::Key("components".into()), Seg::Key("c".into())];
        let ask = Ask {
            seq: 1,
            request: "explain",
            specimen: None,
            model: Some("M"),
            stage: "Resolve",
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: Some(&resolve),
            focus: Focus::Node { key_path: key_path.clone(), stage_value: &resolve },
        };

        let cs = build(&ask)["cross_stage"].clone();
        assert_eq!(cs["applicable"], json!(true), "{cs}");
        // Parse side found the node under classes.M.
        assert_eq!(cs["parse"]["value"]["def_id"], json!(null));
        assert_eq!(cs["resolve"]["value"]["def_id"], json!(9));
        // The two field changes are reported.
        let changes = cs["changes"].as_array().unwrap();
        let has = |p: &str, r: i64| {
            changes.iter().any(|c| c["path"] == json!(p) && c["parse"] == json!(null) && c["resolve"] == json!(r))
        };
        assert!(has("def_id", 9), "changes: {changes:?}");
        assert!(has("type_def_id", 100), "changes: {changes:?}");
    }

    /// A node outside the model class (e.g. the parse `within`) is not diffable.
    #[test]
    fn cross_stage_not_applicable_outside_class() {
        let parse = json!({ "classes": { "M": {} }, "within": "Foo" });
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: "explain",
            specimen: None,
            model: Some("M"),
            stage: "Parse",
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&parse),
            resolve_value: None,
            focus: Focus::Node { key_path: vec![Seg::Key("within".into())], stage_value: &parse },
        };
        assert_eq!(build(&ask)["cross_stage"]["applicable"], json!(false));
    }

    /// The focus JSON's `stages.files` array must list exactly `STAGE_FILE_NAMES`.
    /// This test catches the staleness bug where new stages were added to
    /// `write_stages` but not to the focus JSON (TD-16).
    #[test]
    fn focus_json_stage_files_match_constant() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: "explain",
            specimen: None,
            model: Some("M"),
            stage: "Parse",
            libraries: vec![],
            def_index: &empty,
            parse_value: None,
            resolve_value: None,
            focus: Focus::Specimen,
        };
        let doc = build(&ask);
        let files = doc["stages"]["files"]
            .as_array()
            .expect("stages.files should be an array");
        let file_strs: Vec<&str> = files
            .iter()
            .map(|v| v.as_str().expect("each file should be a string"))
            .collect();
        assert_eq!(
            file_strs.as_slice(),
            STAGE_FILE_NAMES,
            "focus JSON's stages.files is out of sync with STAGE_FILE_NAMES"
        );
    }

    /// `STAGE_FILE_NAMES` must cover every stage the app writes via `write_stages`.
    /// This test checks the count matches the pipeline's 10 stages so a new stage
    /// addition without updating the constant is caught.
    #[test]
    fn stage_file_names_covers_all_pipeline_stages() {
        // The pipeline has exactly 10 stages: Parse through Solve lowering.
        assert_eq!(
            STAGE_FILE_NAMES.len(),
            10,
            "STAGE_FILE_NAMES has {} entries but the pipeline has 10 stages",
            STAGE_FILE_NAMES.len(),
        );
        // Every name must end with .json and be unique.
        for name in STAGE_FILE_NAMES {
            assert!(name.ends_with(".json"), "stage file name should end with .json: {name}");
        }
        let unique: std::collections::HashSet<&&str> = STAGE_FILE_NAMES.iter().collect();
        assert_eq!(unique.len(), STAGE_FILE_NAMES.len(), "duplicate stage file names");
    }

    /// Depth-first search for the first object with a usable `location`,
    /// recording its key-path into `path`.
    fn first_location_path(v: &Value, path: &mut Vec<Seg>) -> bool {
        match v {
            Value::Object(map) => {
                if map.get("location").is_some_and(is_location) {
                    return true;
                }
                for (k, child) in map {
                    path.push(Seg::Key(k.clone()));
                    if first_location_path(child, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            Value::Array(arr) => {
                for (i, child) in arr.iter().enumerate() {
                    path.push(Seg::Index(i));
                    if first_location_path(child, path) {
                        return true;
                    }
                    path.pop();
                }
                false
            }
            _ => false,
        }
    }

    #[test]
    fn seg_display_key() {
        assert_eq!(Seg::Key("components".into()).to_string(), "components");
    }

    #[test]
    fn seg_display_index() {
        assert_eq!(Seg::Index(3).to_string(), "[3]");
    }

    #[test]
    fn describe_path_uses_display() {
        let path = vec![
            Seg::Key("equations".into()),
            Seg::Index(0),
            Seg::Key("Connect".into()),
        ];
        assert_eq!(describe_path(&path), "equations[0].Connect");
    }

    #[test]
    fn cross_stage_fallback_message_for_unsupported_stage() {
        let empty = BTreeMap::new();
        let ask = Ask {
            seq: 1,
            request: "explain",
            specimen: None,
            model: Some("M"),
            stage: "Typecheck",
            libraries: vec![],
            def_index: &empty,
            parse_value: Some(&json!({})),
            resolve_value: Some(&json!({})),
            focus: Focus::Node {
                key_path: vec![Seg::Key("x".into())],
                stage_value: &json!({"x": 1}),
            },
        };
        let doc = build(&ask);
        let cs = &doc["cross_stage"];
        assert_eq!(cs["applicable"], json!(false));
        let reason = cs["reason"].as_str().unwrap();
        assert!(
            reason.contains("not yet implemented"),
            "expected 'not yet implemented' in reason, got: {reason}"
        );
    }
}
