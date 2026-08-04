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
    /// The predicate, factored out so a must-fire test can drive it with text
    /// rather than with the corpus. Returns `(line number, character name)`.
    fn control_chars_in(text: &str) -> Vec<(usize, &'static str)> {
        let mut out = Vec::new();
        for (n, line) in text.lines().enumerate() {
            for (ch, name) in [('\u{7}', "BEL"), ('\u{c}', "FORMFEED"), ('\u{b}', "VTAB")] {
                if line.contains(ch) {
                    out.push((n + 1, name));
                }
            }
        }
        out
    }

    #[test]
    fn documents_contain_no_stray_control_characters() {
        let mut offences = Vec::new();
        let mut scanned = 0usize;
        for path in doc_files() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            scanned += 1;
            for (line, name) in control_chars_in(&text) {
                offences.push(format!("{}:{line} contains {name}", path.display()));
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

    /// The control-character check catches one, and leaves ordinary text alone.
    ///
    /// **Added 2026-08-01 during the must-fire audit** (`verification-plan.md`
    /// item 0). The corpus check above was verified when written by injecting a
    /// BEL into `provenance.md` by hand and watching it fail — which proved it
    /// worked *that day* and proves nothing on any later day. **A manual proof is
    /// not a must-fire test**, and this was the only checker in the codebase
    /// without an automated one.
    ///
    /// **TAB is deliberately absent from the checked set** and this pins that
    /// too: tabs are ordinary inside code blocks, so flagging them would be noise.
    /// The real bug that motivated the checker ate a tab *and* a BEL *and* a
    /// FORMFEED out of `C:\tmp\all-models.txt`; the two unambiguous ones are
    /// enough to catch it.
    #[test]
    fn a_stray_control_character_is_reported() {
        let bel = format!("fine line\nbroken {} line\n", '\u{7}');
        let hits = control_chars_in(&bel);
        assert_eq!(hits, vec![(2, "BEL")], "the BEL on line 2, and nothing else");

        let ff = format!("C:{}tmp{}id-full.csv\n", '\t', '\u{c}');
        assert_eq!(
            control_chars_in(&ff),
            vec![(1, "FORMFEED")],
            "a formfeed is caught; the tab beside it is deliberately not",
        );

        assert!(
            control_chars_in("ordinary prose\n\twith a tab\nand `code`\n").is_empty(),
            "clean text, including tabs, must report nothing",
        );
    }

    /// No function carries two `#[test]` attributes.
    ///
    /// **The signature of an insertion that landed between another test's
    /// attributes and its `fn`** — which silently un-tests that function while
    /// double-registering the new one. Nothing fails: the suite goes green, the
    /// harness just lists one name twice and one function stops being a test.
    ///
    /// **This has now happened three times.** On 2026-07-31 a misplaced
    /// `#[cfg(test)]` let two helpers compile into `--bin hrw` and broke Doug's
    /// debugger launch. On 2026-08-01 it silently disabled
    /// `a_broken_specimen_does_not_poison_the_next_compile` — the regression
    /// guard for upstream issue 1 — twice in one session, by the same author
    /// making the same edit.
    ///
    /// **The rule it encodes: insert a test AFTER a function's closing brace,
    /// never before its `fn` line.** A doc comment and its attributes sit above
    /// the item, so anything placed between them is adopted by the wrong one.
    #[test]
    fn no_function_has_two_test_attributes() {
        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rust(&src, &mut files);
        assert!(files.len() > 10, "only found {} sources — the walk is broken", files.len());

        let mut offences = Vec::new();
        for path in &files {
            let Ok(text) = std::fs::read_to_string(path) else { continue };
            let lines: Vec<&str> = text.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.trim() != "#[test]" {
                    continue;
                }
                // Walk forward over attributes and doc comments. A second
                // `#[test]` before reaching an item means both attach to it.
                for l in lines.iter().skip(i + 1) {
                    let s = l.trim();
                    if s == "#[test]" {
                        offences.push(format!("{}:{}", path.display(), i + 1));
                        break;
                    }
                    if !(s.starts_with("#[") || s.starts_with("///") || s.is_empty()) {
                        break;
                    }
                }
            }
        }
        assert!(
            offences.is_empty(),
            "a function carries two `#[test]` attributes, which means a test was inserted              between another test's attributes and its `fn` — that other function is no              longer a test and nothing else will tell you:
  {}",
            offences.join("
  "),
        );
    }

    /// **No Rust escape may appear literally in comment text.**
    ///
    /// A comment is prose, so an escape sequence in one is not the character it
    /// names — it is six literal characters a reader has to decode:
    ///
    /// ```text
    /// // the offset that would centre it is negative \u{2014} egui clamps to 0     ESCAPE-OK
    /// ```
    ///
    /// It also means the comment was *generated* rather than written: Rust source
    /// produced from another language's string literals, where the escape survived
    /// one layer too many.
    ///
    /// **This exists because a manual sweep did not hold.** Ten such lines were
    /// cleaned out of `app.rs` and `worker.rs` on 2026-08-01, and three more
    /// appeared in `ui_tests.rs` **the same evening**, written by the scripts that
    /// added that day's tests. Doug, 2026-08-02: *"That problem was disruptive and
    /// annoying yesterday."* A rule with nothing checking it rots like any other
    /// claim, which is the must-fire rule pointed at Claude's own working habits.
    ///
    /// String *literals* are untouched: an escape is doing real work there.
    #[test]
    fn no_rust_escape_leaks_into_comment_text() {
        let mut files = Vec::new();
        collect_rust(Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(), &mut files);
        assert!(!files.is_empty(), "found no sources to scan — the check would pass vacuously");

        let mut offences = Vec::new();
        let mut scanned = 0usize;
        for f in &files {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            for (i, line) in text.lines().enumerate() {
                let s = line.trim_start();
                if !s.starts_with("//") {
                    continue;
                }
                scanned += 1;
                // **The opt-out.** A comment explaining this very bug has to be
                // able to show one. Marking it is cheap and keeps the check
                // absolute rather than heuristic.
                if line.contains("ESCAPE-OK") {
                    continue;
                }
                if line.contains("\\u{") {
                    offences.push(format!(
                        "{}:{}: {}",
                        f.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
                        i + 1,
                        s.trim(),
                    ));
                }
            }
        }

        assert!(
            scanned > 500,
            "only {scanned} comment lines scanned — too few to have exercised anything",
        );
        assert!(
            offences.is_empty(),
            "Rust escapes are sitting in comment text, where they are not escapes at all \
             but literal characters a reader must decode. Write the character itself, or \
             mark the line ESCAPE-OK if it is demonstrating the bug:\n  {}",
            offences.join("\n  "),
        );
    }

    /// **The model list defines its row menu once**, so every list gets the same one.
    ///
    /// Doug, 2026-08-04: *"unlike the correctly-working items in the HRW specimens
    /// list, the items in the MSL Corpus list do not provide right-click context
    /// menus."* The corpus rows had **no menu at all** — the one list with 2,626
    /// entries was the one that could not be recompiled or pointed at.
    ///
    /// **Checked at the source level, and honestly about why.** `egui_kittest` clicks
    /// only the primary button, so a context menu cannot be opened in a headless test
    /// and the *rendering* is out of reach (`docs/tech-debt.md`). What is checkable is
    /// the property Doug actually asked for — **one definition** — which makes
    /// consistency structural rather than remembered. Copy the menu inline onto a
    /// second list and this fires, which is the drift that produced the bug.
    ///
    /// Same shape as the field-count ratchet below: read the source, count, and
    /// require the reasoning in the commit that changes the number.
    #[test]
    fn the_model_list_has_exactly_one_row_context_menu() {
        let src = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/model_list.rs"),
        )
        .expect("model_list.rs must be readable");

        let calls = src.matches(".context_menu(").count();
        assert_eq!(
            calls, 1,
            "model_list.rs should build its row menu in one place (`row_context_menu`) \
             and share it; found {calls} call sites, which is how the corpus list came \
             to have no menu while the specimen list had one",
        );

        // Non-vacuity: the helper must exist and every list must reach it, or "one
        // call site" is satisfied by a file that lost the feature altogether.
        assert!(src.contains("fn row_context_menu("), "the shared menu must exist");
        assert_eq!(
            src.matches("row_context_menu(").count(),
            3,
            "one definition plus one call per list (specimens, corpus)",
        );
    }

    /// **`App` has at most `MAX_APP_FIELDS` fields — a ratchet, not a limit.**
    ///
    /// The UI pause's success criterion (`docs/ui-pause-plan.md`). *"Extract
    /// state, not just functions"* is otherwise an unfalsifiable claim: a
    /// refactor that splits a 771-line function into ten that each take
    /// `&mut self` has moved lines and changed nothing, because every one can
    /// still reach every field.
    ///
    /// **Lower the number as extractions land; never raise it.** Failing here
    /// means a field was added to `App` without anyone asking whether it belongs
    /// there — which is the question that went unasked 105 times. The answer may
    /// legitimately be "yes, it is shared": `stage`, `stages`, `tracked_identifier`
    /// and `selected` are the four the measurement found genuinely global. If so,
    /// raise the number **in the same commit as the reasoning**.
    ///
    /// Counted from the source rather than by a macro, so it needs no
    /// cooperation from `App` itself and cannot be silenced by changing a
    /// derive.
    #[test]
    fn app_does_not_regrow_its_field_count() {
        /// 105 before the pause (2026-08-02); 94 after `StageViewCaches`; 85
        /// after `ModelListState`; 75 after `Viewport`; 72 after `TourState`; 57
        /// after `SourceViewState` and `ContextBarState`.
        ///
        /// **Raised to 58 on 2026-08-02 for `SplitState`** (`ideas.md` #59). The ratchet
        /// fired and the question it asks was answered honestly: the LHS/RHS split is
        /// **window layout**, used by both the tour and specimen panels and owned by
        /// neither, so there is no pane to push it into. This is the intended outcome of
        /// a ratchet, not a defeat of one — it forced the question and the answer was
        /// recorded rather than assumed.
        ///
        /// **Lowered to 57 on 2026-08-04**, when `expand_trackable` went with the
        /// "Reveal identifiers" checkbox (`DECISIONS.md`). **A ratchet only ratchets if
        /// removals tighten it**; leaving the bound at 58 would have banked a free slot
        /// for the next field to occupy without argument, which is the whole thing this
        /// test exists to prevent.
        const MAX_APP_FIELDS: usize = 57;

        let src = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"),
        )
        .expect("app.rs must be readable");

        let body = src
            .split_once("\npub struct App {")
            .expect("app.rs must declare `pub struct App`")
            .1
            .split_once("\n}\n")
            .expect("the App struct must be closed by a `}` at column 0")
            .0;

        // A field is `    name: Type,` at exactly one indent level. Doc comments,
        // attributes and blank lines are skipped by the shape of the pattern.
        let fields: Vec<&str> = body
            .lines()
            .filter_map(|l| {
                let rest = l.strip_prefix("    ")?;
                if rest.starts_with(' ') || rest.starts_with("//") || rest.starts_with('#') {
                    return None;
                }
                let name = rest.split_once(':')?.0;
                (!name.is_empty()
                    && name.chars().all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
                .then_some(name)
            })
            .collect();

        // **Non-vacuity.** A parse that silently found nothing would satisfy any
        // ceiling, which is the exact shape this whole suite exists to refuse.
        assert!(
            fields.len() > 50,
            "only {} fields parsed out of App — the parser broke, and a broken parser \
             passes every ceiling",
            fields.len(),
        );
        assert!(
            fields.len() <= MAX_APP_FIELDS,
            "App has {} fields, over the {MAX_APP_FIELDS} ratchet. Does the new field \
             belong on App, or on the pane that owns it? If it is genuinely shared, raise \
             MAX_APP_FIELDS in the same commit as the reasoning.",
            fields.len(),
        );
    }

    fn collect_rust(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                collect_rust(&p, out);
            } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                out.push(p);
            }
        }
    }

    // ----------------------------------------------------------------------
    // Stale-negative checking — `verification-plan.md` item 0b.
    //
    // The mirror of the citation check above. That one asserts every cited path
    // EXISTS; this one asserts that everything a document claims does NOT exist
    // still does not.
    //
    // **A wrong negative is the one error nobody catches.** A wrong *positive*
    // claim gets caught the moment someone acts on it — you go to use the thing
    // and it is not there. Acting on a wrong *negative* means NOT LOOKING, and
    // the natural response to "that is not possible yet" is to build it. On
    // 2026-08-01 `ideas.md` #42 was two days from having its link vocabulary
    // re-implemented on top of itself.
    // ----------------------------------------------------------------------

    /// One `<!-- unbuilt: TARGET -->` tag, with where it was written.
    struct Unbuilt {
        file: String,
        line: usize,
        target: String,
    }

    /// **Fenced code blocks are skipped**, and finding that out was the point of
    /// running this. `verification-plan.md` documents the tag with two worked
    /// examples, and both were deliberately chosen from real stale claims — so the
    /// first run reported the *documentation of the mechanism* as a defect. An
    /// example is not an assertion.
    fn unbuilt_tags(text: &str, file: &str) -> Vec<Unbuilt> {
        let mut out = Vec::new();
        let mut in_fence = false;
        for (i, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("```") {
                in_fence = !in_fence;
                continue;
            }
            if in_fence {
                continue;
            }
            let mut rest = line;
            while let Some(at) = rest.find("<!-- unbuilt:") {
                let after = &rest[at + "<!-- unbuilt:".len()..];
                let Some(close) = after.find("-->") else { break };
                let target = after[..close].trim();
                if !target.is_empty() {
                    out.push(Unbuilt {
                        file: file.to_owned(),
                        line: i + 1,
                        target: target.to_owned(),
                    });
                }
                rest = &after[close..];
            }
        }
        out
    }

    /// Every `.rs` file in the workspace, for symbol resolution.
    fn rust_sources() -> Vec<PathBuf> {
        fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(dir) else { return };
            for e in entries.flatten() {
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_dir() {
                    if !matches!(name, "target" | "node_modules" | "vendor" | ".git") {
                        walk(&p, out);
                    }
                } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    out.push(p);
                }
            }
        }
        let mut out = Vec::new();
        walk(&workspace_root().join("crates"), &mut out);
        walk(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"), &mut out);
        walk(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"), &mut out);
        out
    }

    /// Does a Rust symbol exist, as a **definition** rather than a mention?
    ///
    /// **Deliberately conservative, and the direction matters.** This test *fails
    /// the build*, so a false positive is expensive and a false negative merely
    /// leaves a stale claim for the companion lint to surface. Matching a bare
    /// identifier anywhere would count a mention in a comment — including the very
    /// sentence claiming the thing does not exist — so only definition-shaped
    /// occurrences count. When in doubt this reports "still absent", which lets the
    /// claim stand.
    fn symbol_is_defined(symbol: &str) -> bool {
        // `App::scratch_specimens` -> `scratch_specimens`; a bare name is used as is.
        let leaf = symbol.rsplit("::").next().unwrap_or(symbol).trim();
        if leaf.is_empty() {
            return false;
        }
        let forms = [
            format!("fn {leaf}"),
            format!("struct {leaf}"),
            format!("enum {leaf}"),
            format!("trait {leaf}"),
            format!("const {leaf}"),
            format!("static {leaf}"),
            format!("type {leaf}"),
            format!("mod {leaf}"),
            format!("{leaf}:"), // a struct field
        ];
        rust_sources().iter().any(|p| {
            let Ok(text) = std::fs::read_to_string(p) else { return false };
            text.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .any(|l| forms.iter().any(|f| l.contains(f.as_str())))
        })
    }

    /// Does an `hrw://` link form appear in a fixture tour?
    ///
    /// The named segments must appear **in order**, with `*` skipping any number
    /// of segments between them — so `hrw://stage/*/frame` matches
    /// `hrw://stage/Structural/MatchingAnim/frame/41`.
    ///
    /// **`*` meaning exactly one segment was the first implementation and it was
    /// wrong**: that pattern failed against the real link, which has *two*
    /// segments where the star is, and the check reported a shipped capability as
    /// still absent. Someone writing `stage/*/frame` means "a stage link with a
    /// frame verb", not "with exactly one thing between".
    ///
    /// The fixture tours are the right corpus: a link form nothing exercises is
    /// not really built.
    fn link_form_is_exercised(pattern: &str) -> bool {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours");
        let Ok(entries) = std::fs::read_dir(&dir) else { return false };
        let wanted: Vec<&str> = pattern.trim_start_matches("hrw://").split('/').collect();
        entries.flatten().any(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                return false;
            }
            let Ok(text) = std::fs::read_to_string(&p) else { return false };
            text.split("hrw://").skip(1).any(|tail| {
                let link: String =
                    tail.chars().take_while(|c| !c.is_whitespace() && *c != ')' && *c != '`').collect();
                let got: Vec<&str> = link.split('/').collect();
                // Subsequence match: walk `got`, consuming each named segment of
                // `wanted` in order. `*` consumes nothing and simply allows a gap.
                let mut g = 0usize;
                wanted.iter().all(|w| {
                    if *w == "*" {
                        return true;
                    }
                    while g < got.len() {
                        g += 1;
                        if got[g - 1] == *w {
                            return true;
                        }
                    }
                    false
                })
            })
        })
    }

    fn still_absent(target: &str) -> bool {
        if target.starts_with("hrw://") {
            !link_form_is_exercised(target)
        } else if target.contains("::") || !target.contains('/') {
            !symbol_is_defined(target)
        } else {
            !resolves(target) && !workspace_root().join(target).exists()
        }
    }

    /// Everything a document claims does not exist still does not.
    ///
    /// Tag a claim of absence so it can be checked:
    ///
    /// ```markdown
    /// Frame addressing is not built yet. <!-- unbuilt: hrw://stage/*/frame -->
    /// ```
    ///
    /// **Coverage is expected to be low, and that is fine** — the tag is added when
    /// a claim of absence is written, the way a provenance tag is added when a
    /// claim of fact is. A *wrong* tag fails, because a tag is a claim.
    #[test]
    fn claims_of_absence_are_still_true() {
        let mut tags = Vec::new();
        for path in doc_files() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            let name = path
                .strip_prefix(workspace_root())
                .unwrap_or(&path)
                .display()
                .to_string();
            tags.extend(unbuilt_tags(&text, &name));
        }

        let stale: Vec<String> = tags
            .iter()
            .filter(|t| !still_absent(&t.target))
            .map(|t| format!("{}:{} claims `{}` is unbuilt — it exists", t.file, t.line, t.target))
            .collect();

        println!("stale-negative check: {} `unbuilt:` tag(s) verified", tags.len());
        assert!(
            stale.is_empty(),
            "documents claim something is unbuilt, but it exists. **Acting on a wrong \
             claim of absence means not looking**, so this is the error whose cost is \
             duplicated work rather than a failed build:\n  {}",
            stale.join("\n  "),
        );
    }

    /// The tag mechanism itself works — both directions.
    ///
    /// **A checker reporting zero problems is exactly when to prove it can report
    /// one.** Without this, an `unbuilt:` parser that silently matched nothing
    /// would look identical to a clean corpus.
    #[test]
    fn the_unbuilt_tag_is_parsed_and_both_verdicts_fire() {
        let md = "Frame addressing is not built. <!-- unbuilt: hrw://stage/*/frame -->\n\
                  Scratch specimens do not exist. <!-- unbuilt: App::scratch_specimens -->\n\
                  Nothing tagged on this line.\n\
                  A made-up thing. <!-- unbuilt: NoSuchSymbolAnywhere -->\n";
        let tags = unbuilt_tags(md, "test.md");
        assert_eq!(tags.len(), 3, "three tags, and the untagged line is not one: {:?}",
                   tags.iter().map(|t| &t.target).collect::<Vec<_>>());
        assert_eq!(tags[0].line, 1);
        assert_eq!(tags[1].target, "App::scratch_specimens");

        // Both verdicts must be reachable, or the check is decorative.
        assert!(
            !still_absent("hrw://stage/*/frame"),
            "frame addressing IS exercised by a fixture tour, so this claim would be stale",
        );
        assert!(
            !still_absent("App::scratch_specimens"),
            "scratch_specimens IS a field in app.rs, so this claim would be stale",
        );
        assert!(
            still_absent("NoSuchSymbolAnywhere"),
            "a genuinely absent symbol must read as still absent",
        );
        assert!(
            still_absent("hrw://stage/*/no-such-verb"),
            "an unexercised link form must read as still absent",
        );
    }

    /// **A lint, not a test** — prints untagged claims of absence and never fails.
    ///
    /// The tagged check above catches what someone chose to tag; it cannot read
    /// English. This surfaces the candidates so the retrofit can stay lazy, in the
    /// way provenance tags do. **Failing on these would be wrong**: most are
    /// accurate, and many are prose about the past rather than a live claim.
    #[test]
    fn untagged_claims_of_absence_are_listed() {
        const PHRASES: [&str; 6] = [
            "not yet built",
            "currently impossible",
            "does not exist yet",
            "below the reach",
            "cannot currently",
            "is not built",
        ];
        let mut found = 0usize;
        let mut examples: Vec<String> = Vec::new();
        for path in doc_files() {
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in text.lines().enumerate() {
                if line.contains("unbuilt:") {
                    continue; // already tagged
                }
                let lower = line.to_lowercase();
                if PHRASES.iter().any(|p| lower.contains(p)) {
                    found += 1;
                    if examples.len() < 12 {
                        let f = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                        examples.push(format!("{f}:{}  {}", i + 1, line.trim()));
                    }
                }
            }
        }
        println!(
            "untagged claims of absence: {found} (lint only, never fails)\n  {}",
            examples.join("\n  "),
        );
    }
}
