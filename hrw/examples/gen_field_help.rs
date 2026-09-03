//! Regenerate `src/field_help.json` — the generic (build-time) field-help table.
//!
//! Run after bumping the pinned Rumoca version (see `docs/updating-rumoca.md`):
//!
//! ```text
//! cargo run --example gen_field_help
//! ```
//!
//! It locates `rumoca-ir-ast`'s source via `cargo metadata` (robust to the
//! cargo-cache hash/rev — no hard-coded path), extracts every `///` doc comment
//! that sits on a struct field, and writes the `field-name → doc` map the app
//! embeds. Keying is by field name (v1); longest doc wins when a name recurs.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

// The single list of harvested IR crates lives in the library, where a test can
// reach it — see `field_help::IR_CRATES` for why it fell a stage behind.
use hrw::field_help::IR_CRATES;

fn main() {
    // Locate each IR crate's source via `cargo metadata` (robust to the cargo
    // cache hash/rev), then extract every `///` field doc into one table.
    let out = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .expect("run `cargo metadata`");
    assert!(out.status.success(), "cargo metadata failed");
    let meta: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("parse cargo metadata");

    let mut docs: BTreeMap<String, String> = BTreeMap::new();
    for crate_name in IR_CRATES {
        let src_dir = src_dir_of(&meta, crate_name);
        eprintln!(
            "extracting `///` field docs from {} ({})",
            crate_name,
            src_dir.display()
        );
        let mut files = Vec::new();
        collect_rs(&src_dir, &mut files);
        for f in &files {
            extract(&std::fs::read_to_string(f).expect("read source"), &mut docs);
        }
    }

    // Write src/field_help.json (pretty, sorted for stable diffs).
    let out_path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/field_help.json");
    let json = serde_json::to_string_pretty(&docs).expect("serialize");
    std::fs::write(out_path, json + "\n").expect("write field_help.json");
    eprintln!("wrote {} fields → {out_path}", docs.len());
}

/// The `src/` dir of a resolved dependency package, from `cargo metadata`.
fn src_dir_of(meta: &serde_json::Value, crate_name: &str) -> PathBuf {
    let manifest = meta["packages"]
        .as_array()
        .expect("packages array")
        .iter()
        .find(|p| p["name"] == crate_name)
        .unwrap_or_else(|| panic!("{crate_name} in the dependency graph"))["manifest_path"]
        .as_str()
        .expect("manifest_path")
        .to_owned();
    PathBuf::from(&manifest)
        .parent()
        .expect("crate dir")
        .join("src")
}

/// Recursively gather `*.rs` files under `dir`.
fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read_dir").flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_rs(&p, out);
        } else if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// Associate each `///` comment block with the struct field it precedes.
/// `#[...]` attribute lines between the doc and the field are skipped so the
/// doc still attaches. Longest doc wins when a field name appears on many types.
fn extract(src: &str, docs: &mut BTreeMap<String, String>) {
    let mut buf: Vec<String> = Vec::new();
    for line in src.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("///") {
            buf.push(rest.trim().to_owned());
        } else if t.starts_with("#[") {
            // attribute between the doc and the field — keep the buffer
        } else {
            if !buf.is_empty()
                && let Some(name) = field_name(line)
            {
                let doc = buf
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !doc.is_empty() && docs.get(&name).is_none_or(|d| doc.len() > d.len()) {
                    docs.insert(name, doc);
                }
            }
            buf.clear();
        }
    }
}

/// The field name from a line like `pub foo: Bar,` / `foo: Bar,` — a snake_case
/// identifier before the first `:`. Returns `None` for non-field items (fns,
/// consts, enum variants) so their doc comments are ignored.
fn field_name(line: &str) -> Option<String> {
    let mut t = line.trim();
    for pfx in ["pub(crate) ", "pub(crate)", "pub "] {
        if let Some(r) = t.strip_prefix(pfx) {
            t = r.trim_start();
            break;
        }
    }
    let colon = t.find(':')?;
    let name = t[..colon].trim();
    let mut chars = name.chars();
    let first = chars.next()?;
    if !(first.is_ascii_lowercase() || first == '_') {
        return None;
    }
    if chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
        Some(name.to_owned())
    } else {
        None
    }
}
