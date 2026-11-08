//! Regenerate `src/field_help.json` — the generic (build-time) field-help table.
//!
//! Run after bumping the pinned Rumoca version (see `docs/updating-rumoca.md`):
//!
//! ```text
//! cargo run --example gen_field_help
//! ```
//!
//! It locates each harvested crate's source via `cargo metadata` (robust to the
//! cargo-cache hash/rev — no hard-coded path), extracts every `///` doc comment
//! that sits on a struct field, and writes two tables the app embeds:
//!
//! - **`fields`** — `field -> crate -> doc`, longest doc winning when a name recurs
//!   within one crate. What a stage keyed by crate reads.
//! - **`types`** — `crate -> type -> field -> doc`, the doc on **that type's** field.
//!   Added 2026-11-08 because the first table cannot tell two same-named fields of one
//!   crate apart: `rumoca-phase-structural` documents `scalar` on both
//!   `InitialValuePin` ("zero for scalar pins") and `PinTerm` ("zero for scalar terms"),
//!   and `PinTerm`'s is one character longer, so a pin's tooltip described a term.

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

    // **field -> crate -> doc**, so a tooltip can say which crate's type it describes.
    // Flat `field -> doc` was the defect: one global namespace meant `names`, documented
    // in rumoca-ir-ast as an import clause, supplied the tooltip for Solve lowering's
    // `solver_maps.names`.
    let mut docs: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    // **crate -> type -> field -> doc** — the owning type, so a stage whose value is a
    // known type can ask for that type's field rather than for the crate's longest doc.
    let mut types: TypeDocs = BTreeMap::new();
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
            extract(
                &std::fs::read_to_string(f).expect("read source"),
                crate_name,
                &mut docs,
                &mut types,
            );
        }
    }

    // Write src/field_help.json (pretty, sorted for stable diffs).
    let out_path = hrw::manifest_path!("src", "field_help.json");
    let json = serde_json::to_string_pretty(&serde_json::json!({
        "fields": docs,
        "types": types,
    }))
    .expect("serialize");
    std::fs::write(out_path, json + "\n").expect("write field_help.json");
    let pairs: usize = docs.values().map(BTreeMap::len).sum();
    let typed: usize = types
        .values()
        .flat_map(BTreeMap::values)
        .map(BTreeMap::len)
        .sum();
    eprintln!(
        "wrote {} field names, {pairs} (crate, doc) pairs, {typed} (type, field) docs → {out_path}",
        docs.len()
    );
}

/// `crate -> type -> field -> doc`.
type TypeDocs = BTreeMap<String, BTreeMap<String, BTreeMap<String, String>>>;

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
///
/// Each field doc is also filed under the **type it sits in** — the most recent
/// `struct`/`enum` line above it (see [`type_name`]). Structs do not nest, so the most
/// recent one is the enclosing one for every field line a struct body can contain. A
/// field doc that sits in an enum's struct-like variant is filed under the enum.
fn extract(
    src: &str,
    crate_name: &str,
    docs: &mut BTreeMap<String, BTreeMap<String, String>>,
    types: &mut TypeDocs,
) {
    let mut buf: Vec<String> = Vec::new();
    let mut owner: Option<String> = None;
    for line in src.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix("///") {
            buf.push(rest.trim().to_owned());
        } else if t.starts_with("#[") {
            // attribute between the doc and the field — keep the buffer
        } else {
            if let Some(name) = type_name(line) {
                owner = Some(name);
            }
            if !buf.is_empty()
                && let Some(name) = field_name(line)
            {
                let doc = buf
                    .iter()
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !doc.is_empty() {
                    if let Some(ty) = &owner {
                        // Longest wins only within ONE type, which happens only across
                        // an enum's variants.
                        let per_type = types
                            .entry(crate_name.to_owned())
                            .or_default()
                            .entry(ty.clone())
                            .or_default();
                        if per_type.get(&name).is_none_or(|d| doc.len() > d.len()) {
                            per_type.insert(name.clone(), doc.clone());
                        }
                    }
                    // Longest wins WITHIN a crate — a field name can appear on several
                    // types there. Across crates nothing is discarded any more, which is
                    // the whole point: the reader is told which crate they are reading.
                    let per_crate = docs.entry(name).or_default();
                    if per_crate
                        .get(crate_name)
                        .is_none_or(|d| doc.len() > d.len())
                    {
                        per_crate.insert(crate_name.to_owned(), doc);
                    }
                }
            }
            buf.clear();
        }
    }
}

/// The type a line declares, from `pub struct Foo<'a> {` / `pub(crate) enum Bar {` /
/// `struct Baz;` — the identifier after `struct` or `enum`, without generics.
fn type_name(line: &str) -> Option<String> {
    let mut t = line.trim();
    if let Some(r) = t.strip_prefix("pub") {
        t = r.trim_start();
        if let Some(r) = t.strip_prefix('(') {
            t = r.split_once(')')?.1.trim_start();
        }
    }
    let rest = t
        .strip_prefix("struct ")
        .or_else(|| t.strip_prefix("enum "))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    name.chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
        .then_some(name)
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
