//! The Claude bridge — question-driven help.
//!
//! A *thin emitter*. When you invoke "Ask Claude about this," the app writes a
//! single JSON *focus file* describing what you are looking at: which specimen,
//! which pipeline stage, which IR node, and — via **span-ascent** — where in
//! the Modelica source that node came from. It carries no answers and embeds no
//! model. The reasoning happens in a Claude Code session that reads the file,
//! with the specimen source, the staged IR, the Rumoca phase code, and Doug's
//! `docs/compiler-phases` all already in that session's context.
//!
//! Design rationale (thin emitter, thick reasoner) is in DECISIONS.md.
//!
//! Span-ascent: Rumoca IR nodes carry source provenance pervasively, but a leaf
//! you click (e.g. a bare `"name": "flange_a"`) usually has none of its own —
//! the nearest `location`/`span` lives on an *ancestor*. So from the clicked
//! node we walk up the tree to the tightest enclosing `location` (preferred:
//! `rumoca_core::Location` carries byte offsets *and* a `file_name`) or `span`
//! (`rumoca_core::Span`: byte offsets, source is an opaque `SourceId`), then
//! slice that byte range out of the source. This walk is fully generic — it
//! knows no Rumoca types — keeping the one-generic-tree rule intact.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::worker::DefInfo;

/// Gitignored directory holding the focus file. Repo-relative so it is stable
/// across Claude Code sessions and the app needs no knowledge of the session.
pub const BRIDGE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge");

/// Sub-directory holding one JSON file per pipeline stage's *full* IR, rewritten
/// once per compile. The focus references it so Claude can diff any two stages
/// (e.g. instantiate vs typecheck) without the focus carrying all five IRs.
pub const STAGES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/.hrw-bridge/stages");

/// Largest node subtree inlined into the focus file; larger nodes are described
/// by shape only (Claude can re-derive the rest from the specimen + staged IR).
const MAX_NODE_BYTES: usize = 16 * 1024;

/// Self-describing note embedded in every focus file, so it reads sensibly when
/// opened directly during dogfooding.
const INSTRUCTIONS: &str = "\
HRW bridge focus file, written by the app when you capture a node/stage/model \
(the 🔎 Capture actions) — capturing does NOT ask anything by itself. Ask your \
question in the Claude Code chat; Claude reads this file to see what you \
captured, then reasons over the specimen source, the staged IR, the Rumoca \
phase code, and docs/compiler-phases.";

/// One step of a path into the serde tree: an object key or an array index.
#[derive(Clone, Debug)]
pub enum Seg {
    Key(String),
    Index(usize),
}

impl Seg {
    fn as_json(&self) -> Value {
        match self {
            Seg::Key(k) => Value::String(k.clone()),
            Seg::Index(i) => json!(i),
        }
    }

    fn get<'a>(&self, v: &'a Value) -> Option<&'a Value> {
        match self {
            Seg::Key(k) => v.get(k),
            Seg::Index(i) => v.get(i),
        }
    }
}

/// A human-readable path to the clicked node, e.g. `components.inertia.type_def_id`
/// or `equations[0].Connect.lhs` — so the app can name what was captured instead
/// of an opaque sequence number. Empty path ⇒ the tree root.
pub fn describe_path(path: &[Seg]) -> String {
    if path.is_empty() {
        return "(tree root)".to_owned();
    }
    let mut s = String::new();
    for seg in path {
        match seg {
            Seg::Key(k) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(k);
            }
            Seg::Index(i) => s.push_str(&format!("[{i}]")),
        }
    }
    s
}

/// What the user is asking about.
pub enum Focus<'a> {
    /// A specific IR node in the current stage, at `key_path` from the stage root.
    Node { key_path: Vec<Seg>, stage_value: &'a Value },
    /// The current stage's IR as a whole.
    Stage,
    /// The model / specimen as a whole.
    Model,
}

/// Everything needed to write one focus file.
pub struct Ask<'a> {
    pub seq: u64,
    /// What the user wants: "explain" (default) or "debug-where-set" (they want
    /// to watch this field being assigned in the Rumoca phase, in the debugger).
    pub request: &'a str,
    pub specimen: Option<&'a Path>,
    pub model: Option<&'a str>,
    pub stage: &'a str,
    pub libraries: Vec<String>,
    /// Resolved identity of the DefIds referenced in the model's IR, so an
    /// opaque `type_def_id: 27579` in the focus reads as the class it names.
    pub def_index: &'a BTreeMap<u64, DefInfo>,
    /// Both stages' IR, so a node focus can carry the *same* node before and
    /// after resolution (cross-stage diff). `None` if a stage produced no tree.
    pub parse_value: Option<&'a Value>,
    pub resolve_value: Option<&'a Value>,
    pub focus: Focus<'a>,
}

/// Write the focus file, returning its path on success.
pub fn write(ask: &Ask) -> std::io::Result<PathBuf> {
    fs::create_dir_all(BRIDGE_DIR)?;
    let path = Path::new(BRIDGE_DIR).join("focus.json");
    let doc = build(ask);
    fs::write(&path, serde_json::to_string_pretty(&doc).unwrap_or_default())?;
    Ok(path)
}

/// Write each stage's full IR to `.hrw-bridge/stages/<name>.json` (once per
/// compile). A stage with no IR has its file removed, so the directory always
/// reflects the current specimen. Diffing two stages = reading two of these.
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

fn build(ask: &Ask) -> Value {
    let kind = match ask.focus {
        Focus::Node { .. } => "node",
        Focus::Stage => "stage",
        Focus::Model => "model",
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
            "files": ["parse.json", "resolve.json", "instantiate.json", "typecheck.json", "flatten.json"],
        },
    });
    if let Focus::Node { key_path, stage_value } = &ask.focus {
        doc["node"] = build_node(key_path, stage_value, ask.specimen);
        doc["cross_stage"] = build_cross_stage(ask, key_path);
    }
    doc
}

/// Largest change list emitted in a cross-stage diff (backstop; real diffs are
/// small — a handful of `null → id` fields).
const MAX_CHANGES: usize = 400;

/// The clicked node at *both* stages plus the scalar deltas between them, so
/// "what did Resolve do here?" is answered from data. Correspondence is by
/// class-relative path: each stage's class subtree is auto-detected (descend
/// `classes.<model>` if the root wraps it, else the root already *is* the class),
/// so the same node lines up whether captured from the Parse or Resolve tab.
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
        return json!({ "applicable": false, "reason": "current stage has no IR" });
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

/// Find a stage's user-class subtree: descend `classes.<model>` when the root
/// wraps it (the parsed `StoredDefinition`), else the root already is the class
/// (the resolve extract). Returns the subtree and the prefix depth (2 or 0).
fn class_subtree<'a>(stage_value: &'a Value, model: &str) -> (&'a Value, usize) {
    if let Some(class) = stage_value.get("classes").and_then(|c| c.get(model)) {
        return (class, 2);
    }
    (stage_value, 0)
}

/// Node subtree wrapped for the focus: inline when small, shape-only when large.
fn capped(node: &Value) -> Value {
    let bytes = serde_json::to_string(node).map(|s| s.len()).unwrap_or(0);
    if bytes <= MAX_NODE_BYTES {
        json!({ "value": node })
    } else {
        json!({ "truncated": true, "bytes": bytes, "shape": shape(node) })
    }
}

/// Recursively collect scalar differences between two IR subtrees as
/// `{path, parse, resolve}` records. Objects diff by key, arrays by index.
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

/// `DefId → DefInfo` as a JSON object (string keys), so any `def_id`/
/// `type_def_id`/`base_def_id` in the focus can be looked up by number.
fn def_resolutions(index: &BTreeMap<u64, DefInfo>) -> Value {
    let mut map = serde_json::Map::new();
    for (id, info) in index {
        map.insert(id.to_string(), info.to_json());
    }
    Value::Object(map)
}

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

/// Follow a path from the tree root to the addressed node.
fn navigate<'a>(root: &'a Value, path: &[Seg]) -> Option<&'a Value> {
    let mut cur = root;
    for seg in path {
        cur = seg.get(cur)?;
    }
    Some(cur)
}

/// Walk from the clicked node up to the root, returning the tightest enclosing
/// `location`/`span` provenance (see module docs).
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

/// `rumoca_core::Location`: byte offsets plus a `file_name`.
fn is_location(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("file_name").and_then(Value::as_str).is_some()
}

/// `rumoca_core::Span`: byte offsets plus an opaque `source` id.
fn is_span(v: &Value) -> bool {
    v.get("start").and_then(Value::as_u64).is_some()
        && v.get("end").and_then(Value::as_u64).is_some()
        && v.get("source").is_some()
}

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

/// Resolve which file the byte offsets index into, then slice the exact range
/// and expand it to whole containing lines. Prefers a readable `file_name`
/// (from a `Location`); otherwise falls back to the specimen (spans carry only
/// an opaque source id). Returns `(file, excerpt, line_context)`.
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

/// A compact shape summary for an over-large node: object keys, or array length.
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
}
