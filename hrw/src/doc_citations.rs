//! Do the documentation's source citations still point at real files?
//!
//! `docs/compiler-phases/` is **Claude's teaching database** (`docs/ideas.md` #41), and
//! its value depends on being trustworthy on re-read. Prose rots silently; a *citation*
//! does not have to. A path is checkable, so it gets checked — mechanically, on every
//! test run.
//!
//! This is #41 stage B, built as a **test rather than the example binary the idea
//! sketched**: an example has to be remembered and a test does not, and "remembered" is
//! exactly the property that failed for every stale record this project has found.
//!
//! What it cannot check is whether the prose *around* a citation is still true. That is
//! stage C's job (provenance tags), and the reason untagged text is a lead, not a fact.

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    /// Workspace root — one level above this crate.
    fn workspace_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
    }

    /// Every documentation file worth scanning.
    fn doc_files() -> Vec<PathBuf> {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut out = vec![hrw.join("CLAUDE.md"), hrw.join("README.md"), hrw.join("DECISIONS.md")];
        collect_markdown(&hrw.join("docs"), &mut out);
        out.retain(|p| p.is_file());
        out
    }

    fn collect_markdown(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_markdown(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("md") {
                out.push(path);
            }
        }
    }

    /// Characters that may precede a citation. Anything else means the `src/` sits
    /// inside a longer path.
    fn is_boundary(c: char) -> bool {
        matches!(c, ' ' | '\n' | '\t' | '`' | '(' | '[' | '*' | '"')
    }

    /// Pull Rust source citations out of a document.
    ///
    /// **Boundary-sensitive**, which is the whole difficulty. Without it `src/renderer.rs`
    /// matches inside a quoted third-party panic (`egui-wgpu-0.35.0/src/renderer.rs:981`)
    /// and the checker reports a dependency's file as a broken citation of ours. The
    /// first version did precisely that, and with over-broad matching claimed **98 of
    /// 116** citations broken when the true number was 5.
    fn citations(text: &str) -> BTreeSet<String> {
        let mut found = BTreeSet::new();
        for (idx, _) in text.match_indices("src/") {
            // Walk back over an optional `crates/<name>/` or `hrw/` prefix.
            let start = text[..idx]
                .rfind(|c: char| is_boundary(c))
                .map_or(0, |b| b + c_len(text, b));
            let prefix = &text[start..idx];
            if !(prefix.is_empty()
                || prefix == "hrw/"
                || (prefix.starts_with("crates/") && prefix.ends_with('/')))
            {
                continue; // `src/` is embedded in something longer
            }
            let rest = &text[start..];
            let end = rest
                .find(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '-' | '/' | '.')))
                .unwrap_or(rest.len());
            let cite = &rest[..end];
            if cite.ends_with(".rs") {
                found.insert(cite.to_owned());
            }
        }
        found
    }

    /// Byte length of the char at `i`, so the slice after a boundary stays on a char
    /// boundary — the docs are full of em dashes.
    fn c_len(text: &str, i: usize) -> usize {
        text[i..].chars().next().map_or(1, char::len_utf8)
    }

    /// A citation resolves if the path exists — from the workspace root when qualified,
    /// and from **any crate** for a bare `src/...`.
    ///
    /// Bare paths are *crate-relative*: a phase document citing `src/contents.rs` means
    /// that phase's own crate. Resolving them against `hrw/` alone was the first
    /// version's other mistake, and the larger half of that false 98.
    fn resolves(cite: &str) -> bool {
        let root = workspace_root();
        if cite.starts_with("crates/") || cite.starts_with("hrw/") {
            return root.join(cite).exists();
        }
        let mut bases: Vec<PathBuf> = std::fs::read_dir(root.join("crates"))
            .map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
            .unwrap_or_default();
        bases.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")));
        bases.iter().any(|b| b.join(cite).exists())
    }

    /// Every source file the documentation cites still exists.
    ///
    /// Reports **all** failures at once: a checker that stops at the first turns fixing a
    /// batch into a sequence of rebuilds.
    #[test]
    fn every_documented_source_path_exists() {
        let mut broken: Vec<String> = Vec::new();
        let mut checked = 0usize;

        for doc in doc_files() {
            let text = std::fs::read_to_string(&doc).unwrap_or_default();
            for cite in citations(&text) {
                checked += 1;
                if !resolves(&cite) {
                    broken.push(format!("{}: {cite}", doc.display()));
                }
            }
        }

        assert!(
            checked > 20,
            "expected the docs to cite source freely; found only {checked}, which means \
             the extractor stopped matching rather than the docs stopped citing",
        );
        assert!(
            broken.is_empty(),
            "documentation cites files that do not exist:\n  {}",
            broken.join("\n  "),
        );
    }

    /// A path that is part of a longer path is not a citation.
    ///
    /// Guards the false positive that made the first version unusable: a quoted panic
    /// from a dependency is not a citation of this workspace.
    #[test]
    fn a_path_inside_another_path_is_not_a_citation() {
        let quoted = "panicked at egui-wgpu-0.35.0/src/renderer.rs:981";
        assert!(
            citations(quoted).is_empty(),
            "a third-party path in a quoted panic is not our citation: {:?}",
            citations(quoted),
        );

        // ...while every real form is found.
        let real = "see `crates/rumoca-phase-structural/src/tearing.rs` and src/lib.rs \
                    and [x](hrw/src/app.rs)";
        let found = citations(real);
        assert!(found.contains("crates/rumoca-phase-structural/src/tearing.rs"), "{found:?}");
        assert!(found.contains("src/lib.rs"), "{found:?}");
        assert!(found.contains("hrw/src/app.rs"), "{found:?}");
    }

    /// Provenance tags found in a document: `(kind, cited path or None)`.
    ///
    /// The convention is in `docs/provenance.md`. Recognised on a line of its own,
    /// immediately under the heading it governs:
    ///
    /// ```markdown
    /// *Verified 2026-07-30 against `crates/rumoca-phase-structural/src/tearing.rs`.*
    /// *Inference — not checked against the source.*
    /// *Cellier & Kofman, CSM §9.3.*
    /// ```
    fn provenance_tags(text: &str) -> Vec<(String, Option<String>)> {
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            // A tag *starts* an italic run; it need not be the whole line. The useful
            // tags say what was checked and how, in roman after the italic marker —
            // requiring the entire line to be italic rejected every real one written,
            // which is how this mismatch was found.
            // A tag opens with a SINGLE asterisk. `**Bold**` prose is not a tag, and
            // stripping all leading asterisks read `**Verified 2026-07-28 under
            // cppvsdbg**` — an ordinary emphasised sentence — as one.
            if line.starts_with("**") {
                continue;
            }
            let Some(inner) = line.strip_prefix('*') else {
                continue;
            };
            let kind = if inner.starts_with("Verified") {
                "verified"
            } else if inner.starts_with("Inference") {
                "inference"
            } else if inner.starts_with("Cellier")
                || inner.starts_with("Hairer")
                || inner.starts_with("Brenan")
                || inner.starts_with("MLS ")
            {
                "citation"
            } else {
                continue;
            };
            // A `Verified` tag names the file it was checked against.
            let path = inner
                .split('`')
                .nth(1)
                .filter(|p| p.ends_with(".rs"))
                .map(str::to_owned);
            out.push((kind.to_owned(), path));
        }
        out
    }

    /// Provenance tags are well-formed, and a `Verified` tag's file still exists.
    ///
    /// **Deliberately does not fail on untagged prose.** Upgrading is lazy by design
    /// (`docs/ideas.md` #41, `docs/provenance.md`): tagging 9,000 lines up front would
    /// produce tags nobody had checked, which is the tour-prose mistake again. Low
    /// coverage is expected; a *wrong* tag is not, because a tag is a claim about
    /// trustworthiness and a false one is worse than silence.
    ///
    /// A `Verified` path is also checked by `every_documented_source_path_exists`, so a
    /// tag cannot outlive the thing it points at.
    #[test]
    fn provenance_tags_are_well_formed() {
        let mut verified = 0usize;
        let mut inference = 0usize;
        let mut citation = 0usize;
        let mut tagged_docs = 0usize;
        let mut total_docs = 0usize;
        let mut problems: Vec<String> = Vec::new();

        for doc in doc_files() {
            total_docs += 1;
            let text = std::fs::read_to_string(&doc).unwrap_or_default();
            let tags = provenance_tags(&text);
            if !tags.is_empty() {
                tagged_docs += 1;
            }
            for (kind, path) in tags {
                match kind.as_str() {
                    "verified" => {
                        verified += 1;
                        match path {
                            Some(p) if !resolves(&p) => problems.push(format!(
                                "{}: Verified against {p}, which does not exist",
                                doc.display(),
                            )),
                            None => problems.push(format!(
                                "{}: a Verified tag must name the file it was checked \
                                 against, in backticks",
                                doc.display(),
                            )),
                            Some(_) => {}
                        }
                    }
                    "inference" => inference += 1,
                    _ => citation += 1,
                }
            }
        }

        println!(
            "provenance: {tagged_docs}/{total_docs} docs tagged \
             ({verified} verified, {citation} citations, {inference} inference)",
        );
        assert!(
            problems.is_empty(),
            "malformed or stale provenance tags:\n  {}",
            problems.join("\n  "),
        );
    }

    /// The tag parser recognises each form, and nothing else.
    #[test]
    fn provenance_tags_are_recognised_by_form() {
        let md = "\
## A heading

*Verified 2026-07-30 against `crates/rumoca-phase-structural/src/tearing.rs`.*

Some prose.

## Another

*Inference — not checked against the source.*

## A third

*Cellier & Kofman, CSM §9.3.*

*This italic line is ordinary emphasis, not a tag.*
";
        let tags = provenance_tags(md);
        assert_eq!(tags.len(), 3, "three tags, and the plain italics is not one: {tags:?}");
        assert_eq!(tags[0].0, "verified");
        assert_eq!(
            tags[0].1.as_deref(),
            Some("crates/rumoca-phase-structural/src/tearing.rs"),
            "a Verified tag yields the path it names, so the citation checker can validate it",
        );
        assert_eq!(tags[1].0, "inference");
        assert_eq!(tags[1].1, None);
        assert_eq!(tags[2].0, "citation");
    }

    /// A citation that does not exist is caught.
    ///
    /// A checker reporting zero problems is exactly when to prove it can still report
    /// one.
    #[test]
    fn a_missing_file_is_reported() {
        assert!(!resolves("crates/rumoca-phase-structural/src/no_such_file.rs"));
        assert!(!resolves("src/definitely_not_here.rs"));
        assert!(resolves("crates/rumoca-phase-structural/src/tearing.rs"));
        assert!(resolves("src/tearing.rs"), "bare paths resolve against any crate");
    }

    /// Control characters injected by an interpreted backslash escape.
    ///
    /// **A runbook is copy-pasted, so a byte nobody can see is a broken command.**
    /// `docs/long-runs.md` carried `C:\tmp\all-models.txt` written through something
    /// that interpreted the escapes, eating three characters at once — `\t` → TAB,
    /// `\a` → BEL, `\f` → FORMFEED — leaving `C:<TAB>mp<BEL>ll-models.txt`. It rendered
    /// close enough to right in a terminal to read past, and **survived six commits**
    /// (introduced 2026-08-01 in `1c2e3472`, found 2026-08-01 while grouping the
    /// scripts) because nothing checked.
    ///
    /// BEL and FORMFEED have no legitimate use in our markdown. **TAB is deliberately
    /// not checked** — tabs are ordinary in code blocks, so flagging them would produce
    /// noise, and the two that are unambiguous are enough to catch this class.
    #[test]
    fn documents_contain_no_stray_control_characters() {
        let mut offences = Vec::new();
        let mut scanned = 0usize;
        for path in doc_files() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            scanned += 1;
            for (n, line) in text.lines().enumerate() {
                for (ch, name) in [('\u{7}', "BEL"), ('\u{c}', "FORMFEED"), ('\u{b}', "VTAB")] {
                    if line.contains(ch) {
                        offences.push(format!("{}:{} contains {name}", path.display(), n + 1));
                    }
                }
            }
        }
        // Non-vacuity: a clean scan of nothing is not a clean scan.
        assert!(scanned > 20, "only scanned {scanned} documents — the walk is broken");
        assert!(
            offences.is_empty(),
            "control characters in documentation (a backslash escape was interpreted \
             somewhere — check how the text was written, not just the text):\n  {}",
            offences.join("\n  "),
        );
    }
}
