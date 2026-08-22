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
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .to_path_buf()
    }

    /// Every documentation file worth scanning.
    fn doc_files() -> Vec<PathBuf> {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut out = vec![
            hrw.join("CLAUDE.md"),
            hrw.join("README.md"),
            hrw.join("DECISIONS.md"),
        ];
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
            .map(|d| {
                d.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect()
            })
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
        assert!(
            found.contains("crates/rumoca-phase-structural/src/tearing.rs"),
            "{found:?}"
        );
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
        assert_eq!(
            tags.len(),
            3,
            "three tags, and the plain italics is not one: {tags:?}"
        );
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
        assert!(!resolves(
            "crates/rumoca-phase-structural/src/no_such_file.rs"
        ));
        assert!(!resolves("src/definitely_not_here.rs"));
        assert!(resolves("crates/rumoca-phase-structural/src/tearing.rs"));
        assert!(
            resolves("src/tearing.rs"),
            "bare paths resolve against any crate"
        );
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
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            scanned += 1;
            for (line, name) in control_chars_in(&text) {
                offences.push(format!("{}:{line} contains {name}", path.display()));
            }
        }
        // Non-vacuity: a clean scan of nothing is not a clean scan.
        assert!(
            scanned > 20,
            "only scanned {scanned} documents — the walk is broken"
        );
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
        assert_eq!(
            hits,
            vec![(2, "BEL")],
            "the BEL on line 2, and nothing else"
        );

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
        assert!(
            files.len() > 10,
            "only found {} sources — the walk is broken",
            files.len()
        );

        let mut offences = Vec::new();
        for path in &files {
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
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
        collect_rust(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("src").as_path(),
            &mut files,
        );
        assert!(
            !files.is_empty(),
            "found no sources to scan — the check would pass vacuously"
        );

        let mut offences = Vec::new();
        let mut scanned = 0usize;
        for f in &files {
            let Ok(text) = std::fs::read_to_string(f) else {
                continue;
            };
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

    /// **No recorded animation re-runs a phase algorithm.**
    ///
    /// The acceptance criterion for a whole arc of work (2026-08-04). HRW used to
    /// populate its views by **re-executing** the compiler: connection expansion,
    /// `pre()` lowering, matching, Tarjan, tearing, and instantiate+typecheck were
    /// all run a second time so their intermediate steps could be seen. Doug: *"I
    /// very much want to measure the compilation as it actually happened rather than
    /// make use of replays."*
    ///
    /// Every one now reads frames captured **during the compile that produced the IR
    /// on screen**, through the capture scopes added to `rumoca-phase-{flatten,dae,
    /// structural}` and `rumoca-compile`.
    ///
    /// # Why a source-level test
    ///
    /// "We eliminated the replays" is a claim about **absence**, and this project's
    /// standing rule is that a claim of absence rots unnoticed unless something fails
    /// when it stops being true. Twice in one day I recorded work as outstanding that
    /// was already done, and once as done what was not — reasoning about which
    /// animation re-derives is exactly the thing to stop doing by hand.
    ///
    /// # What is deliberately allowed
    ///
    /// **`start_live` may re-run anything.** A live debug session *is* the user
    /// asking to execute an algorithm again under a debugger; that is the feature,
    /// not a replay of a compile. Tests may too.
    ///
    /// What must not happen is a **default path** that re-derives: that is what makes
    /// the picture describe a run nobody saw.
    ///
    /// # This test is no longer the primary guarantee
    ///
    /// *(2026-08-04.)* It checks a **string** — that `app.rs` does not contain
    /// `from_incidence`. That works and is fragile in the way this project forbids
    /// everywhere else: a re-export, an alias, a wrapper, or moving the call into
    /// another module defeats it in silence, and nothing here decides identity by
    /// substring (`docs/identity-and-provenance.md`).
    ///
    /// All three re-deriving constructors are now `#[cfg(test)]`, so **the UI cannot
    /// call them because in a non-test build they do not exist.** What this test
    /// still adds is the *other* half, which a `cfg` cannot express: that the
    /// captured constructors **are** reached from the UI. A world where nothing
    /// re-derives because nothing is animated at all would satisfy the compiler and
    /// fail here.
    ///
    /// An earlier version of this comment described `record`/`from_incidence` as a
    /// permitted **fallback** for an absent capture. That was true until the same
    /// day, when the fallbacks were removed for drawing blocks the compiler never
    /// built; the captured constructors return `None` and the panes state the
    /// absence.
    #[test]
    fn no_animation_re_runs_a_phase_by_default() {
        let app = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"))
            .expect("app.rs must be readable");

        // **The re-deriving constructor is unreachable from the UI.**
        //
        // `from_incidence` runs matching from scratch (and, for Tarjan, matching then
        // Tarjan). It survives inside the animation modules as the fallback a
        // captured constructor delegates to when no capture exists — but the UI must
        // never name it, because reaching it directly is exactly what made an
        // animation replay a search that produced nothing.
        //
        // Checked against `app.rs` rather than the animation sources because the
        // question is which path the UI *takes*, not which paths exist. A first
        // version of this test read the animation modules and duly flagged the
        // fallbacks it was written to permit.
        // **Neither re-deriving constructor may be named.** `record` went the same
        // way as `from_incidence` on 2026-08-04: both build their own matching and
        // BLT, so on a singular model they draw a decomposition of blocks the
        // compiler never created. Measured on `CapacitorLoop` — the Tarjan tab
        // rendered a *non-empty* SCC animation for a system that produced none.
        //
        // The captured constructors now return `None` instead of re-deriving, and
        // the panes say why (`App::structural_unavailable`). Nothing may reintroduce
        // the fabrication by calling these from the UI.
        for ctor in ["from_incidence", "TearingAnimation::record"] {
            assert!(
                !app.contains(ctor),
                "app.rs constructs an animation with `{ctor}`, which re-derives. On a \
                 model whose compile stopped early that draws an algorithm run that \
                 never happened \u{2014} say the capture is absent instead",
            );
        }

        // And every animation that can be fed from a capture is.
        for ctor in [
            "from_captured_frames",
            "from_captured",
            "from_frames",
            "from_report",
        ] {
            assert!(
                app.contains(ctor),
                "no animation is constructed with `{ctor}` \u{2014} the capture path is \
                 not reached from the UI at all",
            );
        }

        // Non-vacuity: this really is reading the file that builds the animations.
        let sites = app.matches("Animation::").count();
        assert!(
            sites >= 10,
            "expected the animation construction sites, found {sites} \u{2014} this test \
             is looking at the wrong file",
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
        assert!(
            src.contains("fn row_context_menu("),
            "the shared menu must exist"
        );
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
        ///
        /// **Raised to 58 on 2026-08-04 for `matching_frames`**, and the question was
        /// answered rather than waved through. It is a *compile output* — frames
        /// captured from `build_structural_report`'s own matching run — and it belongs
        /// exactly where its three siblings already are: `index_reduction_frames`,
        /// `connection_frames` and `pre_lowering_frames`. All four arrive on
        /// `FromWorker::Compiled`, outlive any one pane, and are read by animations
        /// that can be opened from more than one stage. Pushing this one into
        /// `matching_anim` would split a set that is uniform today and make the odd
        /// member the one nobody expects.
        const MAX_APP_FIELDS: usize = 58;

        let src = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"))
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
                    && name
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_' || c.is_ascii_digit()))
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

    /// **The working tree is checked out with LF line endings.**
    ///
    /// Several tests read repository files as *exact text*:
    /// `app::tests::tour_catalogue_is_current` diffs `CATALOGUE.md` against
    /// freshly generated Markdown, and `app_does_not_regrow_its_field_count`
    /// directly above splits `app.rs` on `"\n}\n"`. Under CRLF both fail — and
    /// the field-count one fails claiming *"the App struct must be closed by a
    /// `}` at column 0"*, **which is false**. The struct is closed correctly;
    /// the line is `"}\r"`. A session that believes that message goes hunting
    /// for an `app.rs` defect that does not exist.
    ///
    /// **So this test exists to fail truthfully, naming the real cause.** It
    /// cannot guarantee running first, but it turns one confusing failure into
    /// two of which one is actionable.
    ///
    /// `hrw/.gitattributes` is the guard. Git for Windows ships
    /// `core.autocrlf=true` in its *system* config, so a fresh clone on a stock
    /// Windows box arrives in CRLF without anyone choosing it — found
    /// 2026-08-07 on the second Windows machine.
    #[test]
    fn the_working_tree_is_checked_out_with_lf_endings() {
        let hrw = Path::new(env!("CARGO_MANIFEST_DIR"));

        // Check the guard itself, so deleting it is a failure rather than a
        // silent return to the hazard.
        let attrs = std::fs::read_to_string(hrw.join(".gitattributes"))
            .expect("hrw/.gitattributes must exist — it is what pins LF on a Windows clone");
        assert!(
            attrs.contains("* text=auto eol=lf"),
            "hrw/.gitattributes must pin `* text=auto eol=lf`",
        );

        // Then the condition it is meant to produce, on the two files whose
        // exact text other tests depend on.
        for rel in ["src/app.rs", "docs/fixture-tours/CATALOGUE.md"] {
            let text = std::fs::read_to_string(hrw.join(rel))
                .unwrap_or_else(|e| panic!("{rel} must be readable: {e}"));

            // **Non-vacuity.** An empty read contains no CRLF and would pass.
            let len = text.len();
            assert!(
                len > 1_000,
                "{rel} read as only {len} bytes — this check would pass vacuously",
            );

            assert!(
                !text.contains("\r\n"),
                "{rel} is checked out with CRLF line endings, which breaks the tests that \
                 read it as exact text. This is a CLONE problem, not a defect in that file: \
                 Git for Windows sets core.autocrlf=true in its system config. Fix with \
                 `git config core.autocrlf false`, then `git rm --cached -r -q .` and \
                 `git reset --hard`. See hrw/.gitattributes.",
            );
        }
    }

    fn collect_rust(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
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
                let Some(close) = after.find("-->") else {
                    break;
                };
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
            let Ok(entries) = std::fs::read_dir(dir) else {
                return;
            };
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
        walk(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
            &mut out,
        );
        walk(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples"),
            &mut out,
        );
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
        rust_source_texts().iter().any(|(_, text)| {
            // **Cheap prefilter, and it is exact rather than approximate.** Every
            // form above contains `leaf`, so a file that does not contain `leaf`
            // anywhere cannot match any form — skipping it changes no verdict.
            //
            // It matters because of what this check is *for*: proving a symbol is
            // **absent**. An absent symbol matches nothing, so the scan cannot
            // short-circuit and pays the full corpus every time — 1,218 files and
            // 24.7 MB, times nine substring forms per line. One `contains` per file
            // rejects almost all of them before any line is split.
            if !text.contains(leaf) {
                return false;
            }
            text.lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .any(|l| forms.iter().any(|f| l.contains(f.as_str())))
        })
    }

    /// Every Rust source in the workspace, walked and read **once**.
    ///
    /// **Memoised because [`symbol_is_defined`] is called once per `unbuilt:` tag**,
    /// and each call used to re-walk `crates/` — 56 crates — and re-read every file
    /// it found. Nine tags therefore paid for nine full-workspace reads.
    ///
    /// **Measured 2026-08-13**, after Doug reported answer latency as friction:
    /// `claims_of_absence_are_still_true` took **10,427 ms** while every other test
    /// in this module took under 900 ms, so one test was **~85 %** of the module's
    /// cost and most of what a prose edit had to wait for. Reading once removes it.
    ///
    /// A `OnceLock` rather than a parameter thread-through: the callers are tests
    /// that run in one process against a tree that does not change mid-run, and the
    /// alternative is passing a corpus through predicates that read better without
    /// one.
    fn rust_source_texts() -> &'static [(PathBuf, String)] {
        static CACHE: std::sync::OnceLock<Vec<(PathBuf, String)>> = std::sync::OnceLock::new();
        CACHE.get_or_init(|| {
            rust_sources()
                .into_iter()
                .filter_map(|p| std::fs::read_to_string(&p).ok().map(|text| (p, text)))
                .collect()
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
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return false;
        };
        let wanted: Vec<&str> = pattern.trim_start_matches("hrw://").split('/').collect();
        entries.flatten().any(|e| {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) != Some("md") {
                return false;
            }
            let Ok(text) = std::fs::read_to_string(&p) else {
                return false;
            };
            text.split("hrw://").skip(1).any(|tail| {
                let link: String = tail
                    .chars()
                    .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '`')
                    .collect();
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
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
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
            .map(|t| {
                format!(
                    "{}:{} claims `{}` is unbuilt — it exists",
                    t.file, t.line, t.target
                )
            })
            .collect();

        println!(
            "stale-negative check: {} `unbuilt:` tag(s) verified",
            tags.len()
        );
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
        assert_eq!(
            tags.len(),
            3,
            "three tags, and the untagged line is not one: {:?}",
            tags.iter().map(|t| &t.target).collect::<Vec<_>>()
        );
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
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
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

    /// Cells of the table under `<!-- <marker>: <Specimen> -->`, backticks stripped.
    ///
    /// **The marker names the specimen**, because one tour describes several panes of
    /// several models — `connect-expansion.md` covers `RcCircuit` and `TwoLoops` — and a
    /// checker that took "the first table after the marker" would compare one model's
    /// table against another model's compile and report confident nonsense.
    ///
    /// Returns `None` when the marker is absent, which callers must treat as a
    /// **finding** rather than as "nothing to check": an unmarked table is exactly a
    /// table nobody verifies.
    ///
    /// # The search is BOUNDED to the table immediately following the marker
    ///
    /// **It used to `skip_while(|l| !l.starts_with("| `"))` with no bound, and that is
    /// worse than it looks** *(found 2026-08-22, by reading — not by a failure)*. Delete
    /// a guarded table but leave its marker, and the scan would run on to the *next*
    /// backticked table anywhere later in the file and compare against **that** —
    /// `pane-groups: RcCircuit` silently binding to the `pane-origins` table. Deleting a
    /// *marker* fails loudly; deleting the *table* under one did something quieter and
    /// wrong. Inserting any backticked table between a marker and its own had the same
    /// effect.
    ///
    /// So the walk now stops at the first line that is neither blank nor part of a
    /// table: a marker's region is the table that follows it, and nothing else. An empty
    /// result therefore means *"marker present, table missing"*, which callers report
    /// rather than skip.
    pub(super) fn marked_rows(
        text: &str,
        marker: &str,
        specimen: &str,
    ) -> Option<Vec<Vec<String>>> {
        let needle = format!("<!-- {marker}: {specimen} -->");
        let start = text.find(&needle)?;
        let mut rows = Vec::new();
        let mut in_table = false;
        for line in text[start + needle.len()..].lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                // Blank lines are allowed *before* the table (markdown wants one after
                // an HTML comment) but end it once it has started.
                if in_table {
                    break;
                }
                continue;
            }
            if !trimmed.starts_with('|') {
                break;
            }
            in_table = true;
            // Header and `|---|` separator rows do not start with a backticked cell.
            if trimmed.starts_with("| `") {
                rows.push(
                    trimmed
                        .trim_matches('|')
                        .split('|')
                        .map(|c| c.trim().trim_matches('`').to_owned())
                        .collect(),
                );
            }
        }
        Some(rows)
    }

    /// Every guarded region of a tour document, as `(marker, rows)` pairs.
    ///
    /// **This is the fingerprint that decides FAST versus FULL**, and it exists because
    /// that decision was previously made by remembering. See
    /// [`editing_a_guarded_tour_table_needs_the_full_gate`].
    ///
    /// Deliberately built from the same [`marked_rows`] the slow checkers use, so the
    /// two can never disagree about what "guarded" means — a second parser would be a
    /// second definition, and the one that drifted would be the one nobody ran.
    pub(super) fn guarded_regions(text: &str) -> Vec<(String, Vec<Vec<String>>)> {
        let mut out = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            let Some(rest) = trimmed.strip_prefix("<!-- pane-") else {
                continue;
            };
            let Some(inner) = rest.strip_suffix("-->") else {
                continue;
            };
            // `groups: RcCircuit ` -> ("groups", "RcCircuit")
            let Some((marker, specimen)) = inner.trim().split_once(':') else {
                continue;
            };
            let (marker, specimen) = (marker.trim(), specimen.trim());
            let rows = marked_rows(text, &format!("pane-{marker}"), specimen).unwrap_or_default();
            out.push((format!("pane-{marker}: {specimen}"), rows));
        }
        out
    }

    /// Every **walked** region of a document, as `(slug, date, body)`.
    ///
    /// # What a walked region is, and why it needs marking at all
    ///
    /// **Corrected tour prose is the artifact a phase-2 walk produces** (Doug,
    /// 2026-08-22), and it is load-bearing twice: it is the only measurement of what he
    /// has learned, and it is what a phase-3 walk treats as trustworthy. **Nothing in
    /// the repository distinguished it from Claude's own first-draft prose**, so a
    /// rewrite saw uniform text and treated it as uniformly Claude's to replace. That is
    /// exactly what happened to `connect-expansion.md`'s lead paragraph: repaired
    /// 2026-08-12 in `21f7cbb0`, silently reverted by the 2026-08-13 rewrite, and asked
    /// about again ten days later.
    ///
    /// ```markdown
    /// <!-- walked: connect-expansion-opening 2026-08-22 -->
    /// …the prose that came out of the walk…
    /// <!-- /walked -->
    /// ```
    ///
    /// **The slug is the identity and the date is the acknowledgement.** They are
    /// separate on purpose — see
    /// [`walked_prose_never_changes_silently`] for what each one buys.
    ///
    /// # The region is BOUNDED by an explicit close
    ///
    /// Same lesson as [`marked_rows`], which used to scan forward unbounded and would
    /// adopt the next table in the file. Here an unterminated marker would swallow the
    /// rest of the document and make every later edit look like a walked change. So a
    /// region runs to its `<!-- /walked -->` and no further, and an unterminated marker
    /// is a **finding** reported by [`unterminated_walked`] rather than a region.
    pub(super) fn walked_regions(text: &str) -> Vec<(String, String, String)> {
        let mut out = Vec::new();
        let mut open: Option<(String, String, Vec<&str>)> = None;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "<!-- /walked -->" {
                if let Some((slug, date, body)) = open.take() {
                    out.push((slug, date, body.join("\n")));
                }
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("<!-- walked:")
                && let Some(inner) = rest.strip_suffix("-->")
            {
                // A second open before a close discards the first; `unterminated_walked`
                // is what reports that, so this stays a parser and not a validator.
                // `split_whitespace` already skips leading and trailing runs, so no
                // `trim()` here: clippy's `trim_split_whitespace` denies the pair.
                let mut parts = inner.split_whitespace();
                let slug = parts.next().unwrap_or_default().to_owned();
                let date = parts.next().unwrap_or_default().to_owned();
                open = Some((slug, date, Vec::new()));
                continue;
            }
            if let Some((_, _, body)) = open.as_mut() {
                body.push(line);
            }
        }
        out
    }

    /// Slugs of `<!-- walked: -->` markers that never reach a `<!-- /walked -->`.
    ///
    /// Reported as a finding rather than tolerated, because an unterminated marker is
    /// indistinguishable from a region whose body happens to be the rest of the file —
    /// and a marker that silently protects nothing is the **wrong negative** this
    /// repository treats as the error nobody catches.
    pub(super) fn unterminated_walked(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut open: Option<String> = None;
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "<!-- /walked -->" {
                open = None;
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("<!-- walked:")
                && let Some(inner) = rest.strip_suffix("-->")
            {
                if let Some(previous) = open.take() {
                    out.push(previous);
                }
                // No `trim()` before `split_whitespace` — see `walked_regions`.
                open = Some(
                    inner
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                );
            }
        }
        out.extend(open);
        out
    }

    /// **Prose Doug corrected during a walk must not change without being acknowledged.**
    ///
    /// # What it forbids, and — as importantly — what it does not
    ///
    /// It does **not** freeze walked prose. Doug rewrites his own explanations
    /// constantly and that must stay cheap; a checker that made improving a tour
    /// expensive would cost more learning than the regression it prevents. **It forbids
    /// changing walked prose *silently*** — the
    /// `app_does_not_regrow_its_field_count` shape, where the change is fine and the
    /// unremarked change is not.
    ///
    /// # Why the slug and the date are separate fields
    ///
    /// The slug is stable identity; the date is the acknowledgement, and **bumping it is
    /// how an edit is acknowledged.** That gives three distinguishable outcomes from one
    /// comparison against `HEAD`:
    ///
    /// | in `HEAD` | in the working tree | verdict |
    /// |---|---|---|
    /// | slug present | body changed, **same date** | **FAIL** — a silent rewrite |
    /// | slug present | body changed, **date bumped** | pass — acknowledged in the same commit |
    /// | slug present | **slug gone** | **FAIL** — a walked passage was deleted |
    /// | absent | slug present | pass — newly marked |
    ///
    /// Keying on the slug alone would make a re-dated edit look like a delete-plus-add;
    /// keying on the whole marker would make every acknowledgement look like a deletion.
    /// **Deletion is the case that matters most** and it is the one the pair recovers.
    ///
    /// # Why it runs in BOTH gates
    ///
    /// Unlike [`editing_a_guarded_tour_table_needs_the_full_gate`], which is off under
    /// `slow-tests` because the real checkers are running then, **this is the only check
    /// of its claim.** There is no slower test that verifies walked prose, so there is no
    /// gate it can defer to.
    ///
    /// # What it cannot claim
    ///
    /// **Silent outside a git checkout**, exactly like the guarded-table check: no
    /// repository, no `HEAD`, or no `git` on `PATH` and it passes for want of a baseline.
    /// The pure half is covered by [`tests_walked_regions`], which needs no git.
    ///
    /// **And it cannot tell whether a marked region deserves its marker.** Marking a
    /// Claude draft as walked would quietly defeat the measurement purpose, so which
    /// passages carry markers is Doug's ruling and no test can stand in for it.
    #[test]
    fn walked_prose_never_changes_silently() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = hrw.join("docs/fixture-tours");
        let repo = hrw.parent().expect("hrw lives inside the workspace");

        let mut findings: Vec<String> = Vec::new();
        let mut compared = 0usize;

        for entry in std::fs::read_dir(&dir).expect("fixture-tours must be readable") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_name()
                .expect("named file")
                .to_string_lossy()
                .into_owned();
            let working = std::fs::read_to_string(&path).expect("readable tour");

            for slug in unterminated_walked(&working) {
                findings.push(format!(
                    "{name}: `<!-- walked: {slug} -->` never closes. Add `<!-- /walked -->` \
                     at the end of the passage \u{2014} an unterminated marker protects \
                     nothing and hides that it protects nothing."
                ));
            }

            let here = walked_regions(&working);
            let Some(head) = file_at_head(repo, &format!("hrw/docs/fixture-tours/{name}")) else {
                continue; // New file, or no git: no baseline to compare against.
            };
            let before = walked_regions(&head);
            if before.is_empty() && here.is_empty() {
                continue;
            }
            compared += 1;

            for (slug, date, body) in &before {
                match here.iter().find(|(s, _, _)| s == slug) {
                    None => findings.push(format!(
                        "{name}: walked region `{slug}` was DELETED. That prose is a record \
                         of what Doug learned on {date}; removing it is the regression this \
                         check exists for. Restore it, or agree with Doug that it is \
                         superseded and say so in docs/question-ledger.md."
                    )),
                    Some((_, now, now_body)) if now_body != body && now == date => {
                        findings.push(format!(
                            "{name}: walked region `{slug}` changed, but its date is still \
                             {date}. If the rewrite is intended, bump the marker's date in \
                             THIS commit \u{2014} that is the acknowledgement. If it was not \
                             intended, this is a phase-2 correction about to be lost."
                        ));
                    }
                    Some(_) => {}
                }
            }
        }

        assert!(findings.is_empty(), "{}", findings.join("\n\n  "));

        // Non-vacuity: passing must mean "compared and found equal", never "found
        // nothing to compare". Zero is honest before any passage is marked, and once
        // markers exist it means git could not answer -- see the doc comment.
        if compared == 0 {
            eprintln!(
                "note: no walked region compared against HEAD \u{2014} either none is \
                 marked yet, or this is not a git checkout"
            );
        }
    }

    /// **Editing a guarded tour table must not be committed behind the FAST gate.**
    ///
    /// # The gap this closes, and why it was not a rule problem
    ///
    /// `CLAUDE.md`'s gate procedure greps the staged **paths**: anything under `src/`,
    /// `crates/`, `examples/` or `Cargo.toml` means FULL, and everything else means
    /// FAST. A tour edit is docs-only, so it returns FAST — **correctly, for the prose
    /// that is most of a walk's output.** But the five `<!-- pane-* -->` tables in
    /// `connect-expansion.md` are verified by *slow* tests, so editing one and running
    /// FAST means the verification does not happen: a green suite over an unchecked
    /// claim. **That is the silent wrong negative this repository treats as the error
    /// nobody catches**, because acting on it means *not looking*.
    ///
    /// The rule already said *"editing one of those tables means FULL, whatever the grep
    /// says"*. **It was enforced by remembering** — Doug asked on 2026-08-22 whether any
    /// edit to that tour triggers FULL, which is exactly the question a remembered rule
    /// produces. This makes it fail by name instead.
    ///
    /// # Why it lives in the FAST suite, which is the whole point
    ///
    /// It is gated **off** under `slow-tests`: in a FULL run the real checkers are
    /// executing, so there is nothing to warn about. It fires only in the cheap suite —
    /// **the cheap gate reports that the cheap gate is insufficient.**
    ///
    /// # Why it compares CONTENT rather than diff line numbers
    ///
    /// Mapping hunk offsets onto marked regions would re-derive, badly, something git
    /// already knows. Instead this extracts every guarded region from the working tree
    /// and from `HEAD` and compares them, which states the property directly — *did a
    /// guarded table change?* — and catches a marker being added or deleted for free.
    ///
    /// # What it cannot claim
    ///
    /// **It is silent outside a git checkout.** No repository, no `HEAD`, or no `git` on
    /// `PATH` and it passes, because there is no baseline to compare against. That is a
    /// wrong negative and it is stated rather than hidden; the alternative is a test that
    /// fails on a source tarball. The pure half — [`guarded_regions`] — is covered by
    /// [`tests_guarded_regions`], which needs no git at all.
    #[test]
    #[cfg_attr(
        feature = "slow-tests",
        ignore = "the FULL gate is running, so the guarded tables are being checked for real"
    )]
    fn editing_a_guarded_tour_table_needs_the_full_gate() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = hrw.join("docs/fixture-tours");
        let repo = hrw.parent().expect("hrw lives inside the workspace");

        let mut changed: Vec<String> = Vec::new();
        let mut compared = 0usize;

        for entry in std::fs::read_dir(&dir).expect("fixture-tours must be readable") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let working = std::fs::read_to_string(&path).expect("readable tour");
            let here = guarded_regions(&working);
            if here.is_empty() {
                continue; // no guarded tables: prose edits are genuinely FAST
            }

            let name = path.file_name().expect("named file").to_string_lossy();
            let rel = format!("hrw/docs/fixture-tours/{name}");
            let Some(head) = file_at_head(repo, &rel) else {
                // New file, or no git. Either way there is no baseline; say nothing.
                continue;
            };
            compared += 1;
            let before = guarded_regions(&head);
            if before == here {
                continue;
            }
            for (marker, rows) in &here {
                let was = before.iter().find(|(m, _)| m == marker).map(|(_, r)| r);
                if was != Some(rows) {
                    changed.push(format!("{name}: `<!-- {marker} -->`"));
                }
            }
            for (marker, _) in &before {
                if !here.iter().any(|(m, _)| m == marker) {
                    changed.push(format!("{name}: `<!-- {marker} -->` was REMOVED"));
                }
            }
        }

        assert!(
            changed.is_empty(),
            "a guarded tour table changed, and this is the FAST suite \u{2014} those tables \
             are verified against a real compile by slow-gated tests, so committing now \
             would land an unchecked claim behind a green suite.\n\n  {}\n\nRun the FULL \
             gate before committing:\n  cargo test -p hrw --lib --test msl_resolve \
             --features slow-tests -- --test-threads=1\n\n(If a table's numbers are \
             genuinely new, the FULL gate is what confirms them against the pane.)",
            changed.join("\n  "),
        );

        // Non-vacuity: this passing must mean "compared and found equal", never "found
        // nothing to compare". Outside a git checkout `compared` is 0 and the check is
        // honestly inert -- see the doc comment.
        if compared == 0 {
            eprintln!(
                "note: no tour compared against HEAD (not a git checkout?) \u{2014} \
                 the guarded-table gate check is inert in this environment"
            );
        }
    }

    /// One file's contents at `HEAD`, or `None` if git cannot answer.
    fn file_at_head(repo: &std::path::Path, rel: &str) -> Option<String> {
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .arg("show")
            .arg(format!("HEAD:{rel}"))
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// **A tour's claims about the equation-sheet PANE match what the pane will show.**
    ///
    /// # The gap this closes
    ///
    /// Until 2026-08-13, every *count* in a tour was read from a generated trace and was
    /// sound, while every *rendering* claim — what the groups are called, how many are in
    /// each — was **unverified**, because Claude cannot see the GUI. Doug walked
    /// `connect-expansion.md` against the real pane and found six disagreements in one
    /// sitting. Four of them were structure claims of exactly the kind this now checks.
    ///
    /// # Why this is possible at all
    ///
    /// `EquationSheet::to_bridge_json` publishes **the renderer's input**, and
    /// `compile_specimen` runs headless. So the pane's content is a pure function of a
    /// compile, callable from a test — no GUI, no running app, no transcription. That
    /// property came from the *data-not-description* rule, not from the bridge being
    /// file-based; the file choice bought travel and headless availability instead.
    ///
    /// # The convention it enforces
    ///
    /// A tour that describes a pane carries a table of its groups:
    ///
    /// ```markdown
    /// | group | rows |
    /// |---|---|
    /// | `Connection equations` | 4 |
    /// ```
    ///
    /// Each row is checked against a real compile: the label must be a label the pane
    /// actually produces, and the count must be its real count. **A table row is used
    /// rather than prose because prose cannot be checked without guessing at it** — the
    /// same reason `unbuilt:` claims carry a tag instead of being inferred from wording.
    ///
    /// # What it still cannot see
    ///
    /// Whether a `category` is *drawn* as a heading, whether rows are legible, whether
    /// anything is scrolled out of view. It verifies **content, never pixels** — Doug's
    /// report remains the only instrument for the rest, and `docs/vision.md` says so.
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    #[test]
    fn tour_group_tables_match_the_real_equation_sheet() {
        // (tour file, specimen) pairs. Grows as tours gain group tables.
        // Every (tour, specimen) pane a tour makes a table claim about. `TwoLoops` was
        // added 2026-08-15: Stop 5's claims had rested on a hand-read trace, which is the
        // footing every other act had already been lifted off.
        const PANES: &[(&str, &str)] = &[
            ("connect-expansion.md", "RcCircuit"),
            ("connect-expansion.md", "TwoLoops"),
        ];

        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut bad: Vec<String> = Vec::new();
        let mut checked = 0usize;

        for (tour, specimen) in PANES {
            // **The memoised helper, not a fresh compile.** Written the other way first
            // and measured at 8.6s of the slow suite's 194s, for specimens other tests
            // had already compiled in the same process. `docs/ideas.md` #48 exists for
            // exactly this and every other module was already using it.
            let compiled = crate::worker::test_msl::compile_specimen_shared(specimen);
            let crate::worker::FromWorker::Compiled { equation_sheet, .. } = compiled else {
                panic!("{specimen}: expected Compiled");
            };
            let sheet = equation_sheet
                .unwrap_or_else(|| panic!("{specimen}: healthy specimen must have a sheet"));

            // The pane's real groups, from the value the renderer walks.
            let real: Vec<(String, usize)> = sheet
                .groups
                .iter()
                .map(|(c, eqs)| (c.label().to_owned(), eqs.len()))
                .collect();

            let families: std::collections::BTreeSet<&str> = sheet
                .groups
                .iter()
                .filter_map(|(c, _)| c.family())
                .collect();

            let text = std::fs::read_to_string(hrw.join("docs/fixture-tours").join(tour))
                .unwrap_or_else(|e| panic!("read {tour}: {e}"));

            // **The table is found by an explicit marker, not by shape.** The first
            // version scanned every `| \`x\` |` row in the file and reported the tour's
            // *specimen* table as claiming groups called `RcCircuit` and `Drivetrain`.
            // A checker that guesses which table it is looking at produces findings the
            // reader has to triage, which is how a checker stops being read.
            let Some(rows) = marked_rows(&text, "pane-groups", specimen) else {
                bad.push(format!(
                    "{tour}: no `<!-- pane-groups: {specimen} -->` marker, so that pane's \
                     group table cannot be checked \u{2014} add one above the table, or \
                     remove the pair from PANES"
                ));
                continue;
            };
            let claimed: Vec<(String, String)> = rows
                .iter()
                .filter_map(|r| Some((r.first()?.clone(), r.get(1)?.clone())))
                .collect();

            // **Stop 4's per-origin breakdown**, checked the same way. It is a different
            // question from the group table — origins are per *row*, groups are the
            // headings — and it was the last table in this tour holding numbers that
            // only a hand-count had ever confirmed.
            if let Some(rows) = marked_rows(&text, "pane-origins", specimen) {
                let mut real_origins: std::collections::BTreeMap<&str, usize> =
                    std::collections::BTreeMap::new();
                for (_, eqs) in &sheet.groups {
                    for e in eqs {
                        *real_origins.entry(e.origin.as_str()).or_default() += 1;
                    }
                }
                for row in &rows {
                    let (Some(origin), Some(n)) = (row.first(), row.get(1)) else {
                        continue;
                    };
                    checked += 1;
                    let actual = real_origins.get(origin.as_str()).copied().unwrap_or(0);
                    if actual.to_string() != *n {
                        bad.push(format!(
                            "{tour} ({specimen}): claims {n} rows with origin `{origin}`; \
                             the pane has {actual}"
                        ));
                    }
                }
            }

            // **The family heading is checked too**, because the nesting is a claim
            // about *why* those equations exist. A tour that lists the children while
            // never naming the parent is back to presenting them as unrelated siblings
            // — the defect the grouping was introduced to fix.
            for family in &families {
                checked += 1;
                if !text.contains(family) {
                    bad.push(format!(
                        "{tour}: the pane groups several kinds under `{family}` and the tour \
                         never names it"
                    ));
                }
            }

            for (label, n) in &real {
                checked += 1;
                match claimed.iter().find(|(l, _)| l == label) {
                    // Distinguish "named wrongly" from "counted wrongly": the fixes
                    // differ, and one message for both hides which it is.
                    Some((_, claimed_n)) if claimed_n != &n.to_string() => bad.push(format!(
                        "{tour}: `{label}` is listed as {claimed_n}; the pane has {n}"
                    )),
                    Some(_) => {}
                    None => bad.push(format!(
                        "{tour}: the pane produces a group `{label}` ({n} rows) that the \
                         table never names"
                    )),
                }
            }

            // And the reverse: a row naming a group the pane does not produce.
            for (label, _) in &claimed {
                if !real.iter().any(|(l, _)| l == label) {
                    bad.push(format!(
                        "{tour}: the table claims a group `{label}` that {specimen}'s pane \
                         does not produce"
                    ));
                }
            }
        }

        assert!(
            checked >= 3,
            "expected several groups to check, got {checked}"
        );
        assert!(
            bad.is_empty(),
            "tour prose disagrees with the equation-sheet pane:\n  {}",
            bad.join("\n  "),
        );
    }

    /// **`connect-expansion.md` Stop 1's node sizes match the connection replay.**
    ///
    /// # The last claim in that tour nobody could check
    ///
    /// Stop 1 predicts *three nodes, of sizes 2, 2 and 3*, and sends the reader to
    /// **Flatten → Connections** — the only pane that shows connection sets. Every other
    /// claim in the tour became checkable when the equation sheet started publishing;
    /// this one rested on Claude having read a trace correctly and never on anything a
    /// test could see.
    ///
    /// # What it checks, and the distinction it is careful about
    ///
    /// Stop 1 counts **nodes**, which are sets of *connectors*. The compiler never groups
    /// connectors — it groups **variables**, in two independent graphs, one per kind. So
    /// a node of size 3 shows up as a **potential set of three `.v`** *and* a **flow set
    /// of three `.i`**, and the tour's `2, 2, 3` must appear as the set sizes of **each
    /// kind separately**, not as some total across both.
    ///
    /// Getting that wrong is the mistake the tour itself made twice before Doug pinned
    /// the vocabulary down, so the check asserts it per kind rather than in aggregate.
    ///
    /// # What it does not check
    ///
    /// That the pane *renders* those sets legibly, or that stepping the replay reads as
    /// an expansion. Content, never pixels.
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    #[test]
    fn tour_node_sizes_match_the_connection_replay() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        // Memoised, for the reason above: `RcCircuit` is compiled by several tests in
        // this process and a fresh compile here bought nothing but 4.3 seconds.
        let crate::worker::FromWorker::Compiled {
            connection_frames, ..
        } = crate::worker::test_msl::compile_specimen_shared("RcCircuit")
        else {
            panic!("expected Compiled");
        };
        assert!(
            !connection_frames.is_empty(),
            "RcCircuit has four connect statements; no frames means the capture scope \
             stopped recording, not that the model stopped connecting"
        );

        let published = crate::connection_anim::ConnectionAnimation::from_frames(connection_frames)
            .to_bridge_json();
        let frames = published["frames"]
            .as_array()
            .expect("frames must be an array");

        // Set sizes per kind, in the order the pass formed them.
        let sizes_of = |kind: &str| -> Vec<u64> {
            frames
                .iter()
                .filter(|f| f["step"] == "SetFormed" && f["kind"] == kind)
                .filter_map(|f| f["size"].as_u64())
                .collect()
        };

        let mut potential = sizes_of("potential");
        let mut flow = sizes_of("flow");
        potential.sort_unstable();
        flow.sort_unstable();

        assert_eq!(
            potential,
            vec![2, 2, 3],
            "Stop 1 predicts nodes of 2, 2 and 3 connectors, so the POTENTIAL sets must be \
             three sets of 2, 2 and 3 `.v` variables"
        );
        assert_eq!(
            flow,
            vec![2, 2, 3],
            "...and the FLOW sets must independently be 2, 2 and 3 `.i` variables. A node \
             is a set of connectors; the compiler groups variables, one graph per kind"
        );

        // **The set count the pane declares**, which is not the node count and was the
        // first thing reading the live pane turned up: the replay's last frame says
        // 6 sets, while Stop 1 predicts 3 nodes. Both are right — one node yields one
        // set per kind — and the tour now states both numbers, so both are pinned.
        let complete = frames
            .iter()
            .find(|f| f["step"] == "Complete")
            .expect("the pass must report a Complete frame");
        assert_eq!(
            complete["sets"].as_u64(),
            Some((potential.len() + flow.len()) as u64),
            "the declared set count must equal the sets actually formed"
        );
        assert_eq!(
            complete["equations_added"].as_u64(),
            Some(7),
            "three nodes over two kinds produce 4 potential + 3 flow equations"
        );

        // The tour's own words, so a reworded prediction cannot drift from this check.
        let tour = std::fs::read_to_string(hrw.join("docs/fixture-tours/connect-expansion.md"))
            .expect("read connect-expansion.md");

        // **Every frame the tour cites by ORDINAL is the frame it says it is.**
        //
        // Stop 2 links `…/Connections/frame/7` and `/frame/13` to point at the moment the
        // n-1 asymmetry happens. `fixture_tour_links_all_resolve` checks only that such a
        // link *parses*. An ordinal citation is the fragility this repository already
        // designed around once — `OpenTour` addresses stops by **slug**, because
        // "inserting a stop shifts every later citation silently, exactly as a source
        // line number does" — and one extra frame emitted by the flatten pass would move
        // both of these with nothing to notice.
        //
        // The answer is `matching_ledger`'s, and Doug's: *"Rotting is bad. If line
        // numbers will help, add line numbers."* Carry the ordinals, and fail loudly
        // when they move. This also pins the **order** the sets are formed in — flow
        // before potential — which the size assertions above cannot see, since they
        // sort.
        let cited = marked_rows(&tour, "pane-frames", "RcCircuit")
            .expect("Stop 2 cites frames by number; the table pinning them must exist");
        assert!(
            !cited.is_empty(),
            "the pane-frames table is empty, so the frame links it exists to pin are \
             unchecked"
        );
        for row in &cited {
            let [n, step, kind, set_size, equations] = &row[..] else {
                panic!("a pane-frames row needs 5 cells, got {row:?}");
            };
            let idx: usize = n.parse().expect("frame ordinal must be a number");
            let frame = frames
                .get(idx - 1)
                .unwrap_or_else(|| panic!("the tour cites frame {n}, past the end of the replay"));
            assert_eq!(
                frame["frame"].as_u64(),
                Some(idx as u64),
                "1-based mismatch"
            );
            assert_eq!(
                frame["step"],
                step.as_str(),
                "frame {n} is a different step"
            );
            assert_eq!(
                frame["kind"],
                kind.as_str(),
                "frame {n} is a different kind"
            );
            assert_eq!(
                frame["set_size"].as_u64().map(|v| v.to_string()).as_deref(),
                Some(set_size.as_str()),
                "frame {n} has a different set size",
            );
            assert_eq!(
                frame["equations_added"]
                    .as_u64()
                    .map(|v| v.to_string())
                    .as_deref(),
                Some(equations.as_str()),
                "frame {n} added a different number of equations",
            );
        }
        assert!(
            tour.contains("/frame/") && cited.len() >= 2,
            "Stop 2's two frame citations must both be pinned"
        );
        for claim in [
            "**three** nodes, of sizes **2, 2 and 3**",
            "**6 connection sets** producing **7 equations**",
        ] {
            assert!(
                tour.contains(claim),
                "Stop 1 no longer states {claim:?}; this check pins that wording and must be \
                 updated with it, or it silently stops matching the tour it guards"
            );
        }
    }

    /// **`f_x[N]` names the same equation in the equation sheet and in the incidence
    /// matrix.**
    ///
    /// # What rests on this
    ///
    /// Doug's deixis requirement — *"I will want to… ask you questions such as 'why is
    /// **this** partial derivative value so high', with the hope that you would be able
    /// to leverage our point-at context scheme and relieve me of the friction of having
    /// to invent a name."* Every published pane carries an `id` per row so that *"this
    /// equation"* resolves to one object across panes. **If the two panes number their
    /// equations differently, that resolution is silently wrong** — a question about
    /// `f_x[19]` would be answered from a different equation than the one under his
    /// cursor, with nothing on either side to reveal it.
    ///
    /// # Why a test rather than an argument
    ///
    /// Both panes derive their ids from the same DAE ordering *today*, which is exactly
    /// the kind of shared assumption that holds until one of them starts sorting for
    /// display. The failure would be invisible: both panes stay well-formed, both look
    /// right, and only a cross-reference disagrees.
    ///
    /// Checked on a real compile, since the ids exist only after one.
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    #[test]
    fn an_equation_id_names_the_same_equation_in_every_pane() {
        let crate::worker::FromWorker::Compiled {
            equation_sheet,
            stages,
            ..
        } = crate::worker::test_msl::compile_specimen_shared("RcCircuit")
        else {
            panic!("expected Compiled");
        };
        let sheet = equation_sheet.expect("RcCircuit has an equation sheet");
        let report = stages
            .get(crate::worker::StageKind::Structural)
            .value
            .clone()
            .expect("structural report");
        let matrix = crate::incidence_view::IncidenceMatrix::from_report(&report)
            .expect("RcCircuit has an incidence matrix");

        // The sheet's ids, as `to_bridge_json` spells them.
        let sheet_ids: Vec<String> = sheet
            .groups
            .iter()
            .flat_map(|(_, eqs)| eqs.iter().map(|e| format!("f_x[{}]", e.index)))
            .collect();
        let matrix_ids: Vec<String> = matrix.to_bridge_json()["equations"]
            .as_array()
            .expect("equations array")
            .iter()
            .filter_map(|e| e["id"].as_str().map(str::to_owned))
            .collect();

        assert!(
            !sheet_ids.is_empty() && !matrix_ids.is_empty(),
            "both panes must produce ids, or this compares nothing",
        );

        // Every id the sheet publishes must name a row the matrix also knows. The
        // reverse is not required: the matrix is built from the *continuous* system
        // and the sheet groups every equation the model gained.
        let known: std::collections::BTreeSet<&str> =
            matrix_ids.iter().map(String::as_str).collect();
        let orphans: Vec<&String> = sheet_ids
            .iter()
            .filter(|id| !known.contains(id.as_str()))
            .collect();
        assert!(
            orphans.is_empty(),
            "these ids exist in the equation sheet and name nothing in the incidence \
             matrix, so \"this equation\" would resolve to different things in the two \
             panes: {orphans:?}",
        );

        // **And the STAGE JSON carries it too, which is the surface deixis uses.**
        //
        // The first version of this test checked only `to_bridge_json`, so it passed
        // while the bug it was written for was still live. `focus.json` — what a
        // point-at capture writes, and what Claude actually reads to resolve "this
        // equation" — carries the raw stage node, not the published view. Doug pointed
        // at an incidence cell on 2026-08-15 *after rebuilding* and the capture still
        // read `"f_x[4] (equation from R)"`.
        //
        // **A test that covers one of two writers is a test that certifies half a
        // fix.** The two rows below are the two writers.
        let rows = report["incidence"]["rows"]
            .as_array()
            .expect("the stage JSON has incidence rows");
        // The AUTHORITATIVE ids: these come from `Incidence::equation_refs`, the bare
        // reference Rumoca kept. Everything below is checked against them.
        let mut authoritative: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for (i, row) in rows.iter().enumerate() {
            let id = row["id"].as_str().unwrap_or_else(|| {
                panic!("stage row {i} has no `id`; a capture of it resolves to nothing")
            });
            assert!(
                known.contains(id),
                "stage row {i} publishes id {id:?}, which names nothing the sheet knows",
            );
            assert!(
                !id.contains(" ("),
                "the stage `id` must be the bare reference, not the decorated label \
                 ({id:?}) \u{2014} the label is `equation`, beside it",
            );
            authoritative.insert(id);
        }

        // **Every array that names an equation must carry an id, and the derived ones
        // must agree with the authoritative set.**
        //
        // `matching` and `blocks` cannot carry the true reference — `StructuralReport`
        // keeps labels only — so their ids come from `equation_id_from_label`. That is
        // a *derivation* where the incidence path has a *value*, and this is what makes
        // it safe rather than assumed: the derivation is required to land inside the
        // set the authoritative path produced, on real data.
        //
        // Written after fixing `incidence.rows` alone and being asked whether the other
        // panes were fixed too. They were not: each equation is named in **three**
        // places and one had been done. A test that covers one writer certifies a
        // fraction of a fix.
        let mut derived = 0usize;
        let mut check = |id: &str, what: &str| {
            assert!(
                !id.contains(" ("),
                "{what} publishes a decorated label as its id ({id:?})",
            );
            assert!(
                authoritative.contains(id),
                "{what} publishes id {id:?}, which is not one of the incidence rows' \
                 ids \u{2014} so the derivation from the label disagrees with the \
                 reference Rumoca kept",
            );
            derived += 1;
        };

        for (i, m) in report["matching"]
            .as_array()
            .expect("matching array")
            .iter()
            .enumerate()
        {
            check(
                m["id"].as_str().unwrap_or_else(|| {
                    panic!("matching[{i}] has no `id`; pointing at it resolves to nothing")
                }),
                &format!("matching[{i}]"),
            );
        }

        for (i, b) in report["blocks"]
            .as_array()
            .expect("blocks array")
            .iter()
            .enumerate()
        {
            match b["kind"].as_str() {
                Some("scalar") => check(
                    b["id"].as_str().unwrap_or_else(|| {
                        panic!("blocks[{i}] (scalar) has no `id` \u{2014} the spy-plot's capture")
                    }),
                    &format!("blocks[{i}]"),
                ),
                Some("coupled") => {
                    let ids = b["ids"]
                        .as_array()
                        .unwrap_or_else(|| panic!("blocks[{i}] (coupled) has no `ids`"));
                    let n_eq = b["equations"].as_array().map_or(0, Vec::len);
                    assert_eq!(
                        ids.len(),
                        n_eq,
                        "blocks[{i}] must give one id per equation in the block",
                    );
                    for id in ids {
                        check(
                            id.as_str().expect("id is a string"),
                            &format!("blocks[{i}].ids"),
                        );
                    }
                }
                other => panic!("blocks[{i}] has an unknown kind {other:?}"),
            }
        }

        assert!(
            derived >= 20,
            "only {derived} derived ids were checked; RcCircuit has 23 equations \
             matched and blocked, so this is not exercising what it claims",
        );
    }

    /// **A live compile of a COUPLED model publishes no equation it cannot identify.**
    ///
    /// # Why a second specimen, and why this one
    ///
    /// The test above runs on `RcCircuit`, whose `coupled_block_count` is **0**. Every
    /// one of its 23 blocks is scalar, so the assertions covering `blocks[].ids` and
    /// the tearing report were code that **never executed** — asserted, and vacuous.
    /// Doug, 2026-08-15: *"Do we have an evidence-based reason to conclude that we have
    /// fixed all id bugs?"* For the coupled path the answer was no, and the reason was
    /// not a missing check but a specimen that could not reach it.
    ///
    /// `TwoLoops` has two coupled blocks, each with a tearing report — the only shape
    /// that reaches `residual_equations` and `causal_sequence`, which were **unidentified
    /// until this test was written**.
    ///
    /// # Why it runs the general walker rather than naming fields
    ///
    /// [`crate::unidentified_equation_labels`] finds every decorated label in the tree
    /// and demands its identity be recoverable from the same object, so a stage that
    /// grows a new equation-naming field fails here on arrival. Enumerating the sites by
    /// hand is what produced five separate half-fixes; the point of this test is that
    /// nobody has to enumerate them again.
    ///
    /// The committed-trace counterpart is
    /// `crate::tests_equation_identity::every_committed_trace_identifies_every_equation_it_names`,
    /// which is fast and covers every specimen. This one guards against the traces going
    /// stale relative to the writers — which they had, for seven specimens.
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    #[test]
    fn a_coupled_model_identifies_every_equation_it_publishes() {
        let crate::worker::FromWorker::Compiled { stages, .. } =
            crate::worker::test_msl::compile_specimen_shared("TwoLoops")
        else {
            panic!("expected Compiled");
        };

        let structural = stages
            .get(crate::worker::StageKind::Structural)
            .value
            .clone()
            .expect("structural report");

        // **Non-vacuity, stated as a precondition rather than assumed.** If TwoLoops
        // ever stops producing coupled blocks, this test must fail loudly rather than
        // quietly go back to covering only the scalar path.
        let coupled = structural["blocks"]
            .as_array()
            .expect("blocks array")
            .iter()
            .filter(|b| b["kind"].as_str() == Some("coupled"))
            .count();
        assert!(
            coupled >= 2,
            "TwoLoops must produce coupled blocks or this test covers the same \
             scalar-only path RcCircuit already covers; found {coupled}",
        );
        let torn = structural["blocks"]
            .as_array()
            .expect("blocks array")
            .iter()
            .filter(|b| b["tearing"].is_object())
            .count();
        assert!(
            torn >= 1,
            "no block carried a tearing report, so the two sites that were broken \
             when this test was written are not being reached",
        );

        for stage in [
            crate::worker::StageKind::Structural,
            crate::worker::StageKind::IndexReduction,
        ] {
            let Some(value) = stages.get(stage).value.clone() else {
                continue;
            };
            let orphans = crate::unidentified_equation_labels(&value);
            assert!(
                orphans.is_empty(),
                "{} equation labels in the live {stage:?} stage carry no identity, so \
                 pointing at them resolves to nothing:\n  {}",
                orphans.len(),
                orphans
                    .iter()
                    .map(|u| format!("{} = {:?}", u.path, u.label))
                    .collect::<Vec<_>>()
                    .join("\n  "),
            );
        }
    }

    /// **Equation text a tour quotes is text HRW actually renders.**
    ///
    /// Doug, 2026-08-12, walking `connect-expansion.md`: *"the Connect sub-tour has this
    /// equation text: `f_x[19]  connection equation: src.p.v = R.p.v` but in the Flatten
    /// → Equations sub-tab that equation is shown with `0 = src.p.v - R.p.v`."*
    ///
    /// **Neither string was invented — and that is what made it hard to see.** Rumoca
    /// stores every continuous equation as an expression that must equal zero, so the
    /// equation sheet prints the **residual** form `0 = src.p.v - R.p.v`, while the
    /// structural report writes a **label** for a human reading a matching:
    /// `f_x[19] (connection equation: src.p.v = R.p.v)`. Both are real. The tour quoted
    /// one and sent the reader to the other, which is a **provenance** error rather than
    /// a fabrication — and no spell-check, link check or count check could see it.
    ///
    /// *(Corrected 2026-08-13: this comment used to say the two forms "live in *different
    /// panes*". They do not. `view.json` shows the equation sheet carries **both** — the
    /// residual as `text`, the label as `origin` — which is the claim the tour got wrong
    /// too. Reading the pane rather than reasoning about it is what settled it.)*
    ///
    /// # What this checks, and what it deliberately does not
    ///
    /// Both forms are recoverable from the committed traces without a compile:
    /// `structural.json` carries every `equation` label and every `equation_text`. So a
    /// quoted string must appear in that union. **It does not verify the string is quoted
    /// from the pane the tour points at** — `tour_group_tables_match_the_real_equation_sheet`
    /// above does that, by compiling. This catches *invented* text and text that has
    /// drifted from the traces.
    ///
    /// Two shapes are recognised, because they are the two that appear:
    /// - `` `f_x[N] (…)` `` inline — the label form, compared verbatim.
    /// - `0 = <expr>` on its own line inside a fenced block — the sheet's form, compared
    ///   after stripping `0 = `. Placeholders containing `<` are skipped, so
    ///   `` `0 = <expression>` `` in prose is not mistaken for a quote.
    #[test]
    fn equation_text_quoted_in_tours_matches_the_traces() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // Every equation label and every rendered residual, from every trace.
        let mut labels: BTreeSet<String> = BTreeSet::new();
        let mut residuals: BTreeSet<String> = BTreeSet::new();
        let notebook = hrw.join("docs/specimen-notebook");
        let Ok(entries) = std::fs::read_dir(&notebook) else {
            panic!("no specimen notebook at {}", notebook.display());
        };
        for entry in entries.flatten() {
            let structural = entry.path().join("trace/structural.json");
            let Ok(raw) = std::fs::read_to_string(&structural) else {
                continue;
            };
            let Ok(json) = serde_json::from_str::<serde_json::Value>(&raw) else {
                continue;
            };
            collect_equation_strings(&json, &mut labels, &mut residuals);
        }
        assert!(
            labels.len() > 50 && residuals.len() > 50,
            "collected only {} labels and {} residuals from the traces — the scan is \
             broken, so every assertion below would pass vacuously",
            labels.len(),
            residuals.len(),
        );

        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();
        let tours = hrw.join("docs/fixture-tours");
        let mut tour_files: Vec<PathBuf> = Vec::new();
        collect_markdown(&tours, &mut tour_files);

        for path in tour_files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let mut fenced = false;
            for (i, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("```") {
                    fenced = !fenced;
                    continue;
                }

                // The label form, anywhere.
                for quoted in backticked(line) {
                    if quoted.starts_with("f_x[") && quoted.contains(" (") {
                        checked += 1;
                        if !labels.contains(&quoted) {
                            bad.push(format!(
                                "{name}:{}  label not in any trace: `{quoted}`",
                                i + 1
                            ));
                        }
                    }
                }

                // The sheet's residual form, only inside a fence.
                let t = line.trim();
                if fenced && let Some(expr) = t.strip_prefix("0 = ") {
                    if expr.contains('<') {
                        continue; // a placeholder, not a quote
                    }
                    checked += 1;
                    if !residuals.contains(expr) {
                        bad.push(format!(
                            "{name}:{}  residual not in any trace: {:?}",
                            i + 1,
                            expr
                        ));
                    }
                }
            }
        }

        // **Non-vacuity.** The defect that prompted this was seven such lines in one
        // tour; a run that inspects none of them has stopped working.
        assert!(
            checked >= 5,
            "only {checked} quoted equation strings were inspected — the extraction is \
             broken, not the tours",
        );
        assert!(
            bad.is_empty(),
            "{} quoted equation string(s) match nothing HRW renders:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("equation strings checked against the traces: {checked}");
    }

    /// **Every tour the chain overview sends you into must link back to it.**
    ///
    /// # The friction this closes
    ///
    /// Doug, 2026-08-17: *"I encountered yet again an annoying bit of friction which
    /// happens when there's a top-level tour which links to subordinate tours. I really
    /// want to be able to navigate backward from a subordinate tour to the top-level tour
    /// so that I can then navigate downward to another subordinate tour."*
    ///
    /// `the-concepts.md` is a hub: ten rows, each an `hrw://tour/<name>` link into a
    /// phase tour. **The links ran one way only.** Walking the chain therefore meant
    /// opening the picker between every pair of tours — with the hub sitting alphabetically
    /// in the middle of the list, indistinguishable from its own children.
    ///
    /// # Why a checker rather than just the ten edits
    ///
    /// **A missing back-link is invisible from inside the tour that lacks it.** Every
    /// other tour checker asks *"is what this document says true?"*, and a document with
    /// no way back says nothing false — the ten tours were internally perfect and the
    /// chain was still a dead end at every stop. Same shape as the Context Bar's missing
    /// background and the notebook's absent specimens: **a partial report leaves no gap
    /// where the missing part was.**
    ///
    /// So the property has to be stated across *two* files, which is exactly what nothing
    /// checked before. It is also the property most likely to rot: the eleventh tour added
    /// to the overview's table is one line in one file, and remembering the second edit is
    /// the part that fails.
    ///
    /// # What it checks
    ///
    /// For every `hrw://tour/<name>` the overview links to, `<name>.md` must contain
    /// `hrw://tour/the-concepts` — an **`hrw://` link, not a markdown one**. That
    /// distinction is the defect it was written against: `solve-lowering.md` and
    /// `matching-live.md` both referenced the overview as `[the-concepts.md](…)`, which
    /// HRW's commonmark renderer hands to the *operating system* as a relative file URL.
    /// It opens nothing, or opens a text editor. Only the `hrw://` form is a tour link.
    #[test]
    fn every_tour_the_overview_links_to_links_back() {
        let tours = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours");
        let overview = tours.join(format!("{}.md", crate::tour::OVERVIEW_TOUR));
        let text = std::fs::read_to_string(&overview).unwrap_or_else(|e| {
            panic!(
                "{} is the entry point every phase tour hangs off: {e}",
                overview.display()
            )
        });

        // The rows the overview sends the reader into, in order, deduplicated. Derived
        // from the links rather than from a list here, so adding a row to the table is the
        // only edit needed to bring a new tour under this check.
        let mut referenced: Vec<String> = Vec::new();
        for tail in text.split("hrw://tour/").skip(1) {
            let name: String = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if name.is_empty() || name == crate::tour::OVERVIEW_TOUR {
                continue;
            }
            if !referenced.contains(&name) {
                referenced.push(name);
            }
        }

        // **Non-vacuity.** An extraction that finds nothing would report a chain of
        // perfect back-links across zero tours — the failure mode this file has hit four
        // times, most recently with three source-text checks matching their own prose.
        assert!(
            referenced.len() >= 9,
            "found only {} tours referenced by {}: {referenced:?} — the chain has nine \
             phases plus the live variant, so the extraction is broken rather than the \
             overview",
            referenced.len(),
            crate::tour::OVERVIEW_TOUR,
        );

        let mut missing: Vec<String> = Vec::new();
        let mut markdown_only: Vec<String> = Vec::new();
        for name in &referenced {
            let path = tours.join(format!("{name}.md"));
            let Ok(body) = std::fs::read_to_string(&path) else {
                // A dangling row is `fixture_tour_links_all_resolve`'s business, not this
                // test's; reporting it twice would give one defect two names.
                continue;
            };
            if body.contains("hrw://tour/the-concepts") {
                continue;
            }
            // Distinguish "no way back at all" from "a way back that goes to the OS",
            // because they read identically in a diff and only one of them looks done.
            if body.contains(&format!("]({}.md)", crate::tour::OVERVIEW_TOUR)) {
                markdown_only.push(name.clone());
            } else {
                missing.push(name.clone());
            }
        }

        assert!(
            markdown_only.is_empty(),
            "{} tour(s) reference the overview as a plain markdown file link, which HRW \
             hands to the operating system rather than opening as a tour — it looks like a \
             back-link in the source and does nothing when clicked. Use \
             `[▲ The chain overview](hrw://tour/{})`: {:?}",
            markdown_only.len(),
            crate::tour::OVERVIEW_TOUR,
            markdown_only,
        );
        assert!(
            missing.is_empty(),
            "{} tour(s) the overview links into offer no way back to it, so walking the \
             chain means reopening the picker at every stop. Add \
             `[▲ The chain overview](hrw://tour/{})` after the H1 and in the closing \
             section: {:?}",
            missing.len(),
            crate::tour::OVERVIEW_TOUR,
            missing,
        );
        println!("tours linked back to the overview: {}", referenced.len());
    }

    /// The kinds a tour may declare, and whether that kind predicts.
    ///
    /// **`docs/tour-kinds-plan.md` is the authority**; this table is its executable half.
    /// The `predicts` column is the whole point of declaring a kind at all: without it,
    /// *"a concept tour that lost its predictions"* and *"a feature tour that correctly
    /// has none"* are the same document to a checker.
    const TOUR_KINDS: &[(&str, bool)] = &[
        ("concept", true),
        ("feature", false),
        ("failure", false),
        ("adjudication", false),
        ("hub", false),
    ];

    /// Every fixture tour, as `(name, kind, text)`.
    ///
    /// Panics rather than skipping an unreadable or untagged tour: a roster that
    /// silently shrinks turns every check below into a check of nothing, which is the
    /// vacuity failure this file has now hit five times.
    fn tours_with_kinds() -> Vec<(String, String, String)> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("docs/fixture-tours must be readable") {
            let path = entry.expect("a readable dir entry").path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            // Neither is a tour: one is documentation ABOUT tours, the other is
            // generated FROM them.
            if name == "README" || name == "CATALOGUE" {
                continue;
            }
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
            let kind = text
                .split("<!-- kind: ")
                .nth(1)
                .and_then(|tail| tail.split(" -->").next())
                .unwrap_or_else(|| {
                    panic!(
                        "{name}.md declares no kind. Add `<!-- kind: … -->` under the H1 — \
                         one of {:?}. Without it no checker can tell a concept tour that lost \
                         its predictions from a feature tour that correctly has none.",
                        TOUR_KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
                    )
                })
                .trim()
                .to_owned();
            out.push((name, kind, text));
        }
        out.sort();
        assert!(
            out.len() >= 20,
            "found only {} fixture tours — the enumeration is broken, and every check \
             below would pass over an empty corpus",
            out.len(),
        );
        out
    }

    /// The numbered stops of a tour, as `(heading, body-up-to-the-next-heading)`.
    ///
    /// **`Stop 0` is returned like any other but is exempt from the prediction rule** by
    /// its caller: a zero stop is setup — it has something to check and nothing to
    /// predict. `matching-live.md` and `frame-seeking.md` both have one.
    fn numbered_stops(text: &str) -> Vec<(String, String)> {
        let mut stops: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            if line.starts_with("## ") {
                // The emoji-prefixed form the adjudication tours use — `## 📐 Stop 1 — …`
                // — is a stop too. Matching on "Stop " rather than on a line prefix is
                // what makes those four tours visible to this walk at all.
                if line.contains("Stop ") {
                    stops.push((line.to_owned(), String::new()));
                } else if let Some((_, body)) = stops.last_mut().map(|s| (&s.0, &mut s.1)) {
                    // A later non-stop heading ends the current stop.
                    let _ = body;
                    stops.push((String::new(), String::new()));
                }
                continue;
            }
            if let Some(last) = stops.last_mut()
                && !last.0.is_empty()
            {
                last.1.push_str(line);
                last.1.push('\n');
            }
        }
        stops.retain(|(h, _)| !h.is_empty());
        stops
    }

    /// **Every stop of every kind owes an `Expected` — that is the invariant.**
    ///
    /// # Why this one and not `Predict`
    ///
    /// Doug's model, 2026-08-17: *"while all kinds of tours have stops, each kind of tour
    /// might have different activities at its stops."* True — and the corpus says exactly
    /// one thing does **not** vary. `Predict` appears zero times in all 12 non-concept
    /// tours and once per stop in all 10 concept tours; `Expected` appears at every stop
    /// of every kind.
    ///
    /// **So `Expected` is what makes a tour a *test* rather than an explanation**, which
    /// is this directory's whole justification. `Predict` is merely how a *concept* tour
    /// earns its Expected — a feature tour earns the same claim by having Doug **do** the
    /// action, a failure tour by having him **read** the diagnosis.
    ///
    /// A stop with no falsifiable line is a paragraph with a heading, and a tour of those
    /// is the stored prose this project retired 1,632 lines of.
    #[test]
    fn every_stop_of_every_tour_owes_an_expected() {
        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        for (name, _kind, text) in tours_with_kinds() {
            for (heading, body) in numbered_stops(&text) {
                checked += 1;
                if !body.contains("**Expected:") && !body.contains("**Expected**") {
                    bad.push(format!("{name}.md  {}", heading.trim()));
                }
            }
        }

        assert!(
            checked >= 90,
            "only {checked} stops were inspected across the corpus — the heading walk is \
             broken, not the tours",
        );
        assert!(
            bad.is_empty(),
            "{} stop(s) state nothing that could fail. Every stop of every kind owes an \
             **Expected:** — it is what makes a tour a test rather than an explanation:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("stops carrying a falsifiable Expected: {checked}");
    }

    /// **A tour predicts if and only if its kind says it does.**
    ///
    /// # Both directions matter, and they fail for opposite reasons
    ///
    /// **A concept tour without predictions** has lost its engine. The prediction is what
    /// makes Doug an instrument rather than an audience — it is the reason the unit is a
    /// question rather than a topic — and a concept tour that merely explains and then
    /// shows is the "book" form he reported bouncing off.
    ///
    /// **A feature tour *with* predictions** is the other error, and it is the one Claude
    /// was about to commit. On 2026-08-17 Claude wrote that the 12 non-concept tours were
    /// *"unconverted, not differently designed"* — which would have meant converting them,
    /// and the count says otherwise: zero predictions across all twelve is a design, not a
    /// backlog. **There is no gradient anywhere in the corpus**, and this check is what
    /// keeps it that way.
    ///
    /// `Stop 0` is exempt: it is setup, with an expectation to check and nothing to
    /// predict.
    #[test]
    fn a_tour_predicts_if_and_only_if_its_kind_says_so() {
        let mut bad: Vec<String> = Vec::new();
        let mut concept_stops = 0usize;
        let mut other_stops = 0usize;

        for (name, kind, text) in tours_with_kinds() {
            let predicts = TOUR_KINDS
                .iter()
                .find(|(k, _)| *k == kind)
                .unwrap_or_else(|| {
                    panic!(
                        "{name}.md declares kind {kind:?}, which is not one of {:?}",
                        TOUR_KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
                    )
                })
                .1;

            for (heading, body) in numbered_stops(&text) {
                let is_setup = heading.contains("Stop 0");
                let has = body.contains("**Predict.**");
                if predicts {
                    concept_stops += 1;
                } else {
                    other_stops += 1;
                }

                if predicts && !has && !is_setup {
                    bad.push(format!(
                        "{name}.md  {}  — a concept stop with nothing to predict",
                        heading.trim()
                    ));
                } else if !predicts && has {
                    bad.push(format!(
                        "{name}.md  {}  — a {kind} stop should not predict",
                        heading.trim()
                    ));
                }
            }
        }

        // **Non-vacuity on BOTH arms.** A run that inspected only concept stops would
        // report the negative rule as holding across nothing at all.
        assert!(
            concept_stops >= 40 && other_stops >= 40,
            "inspected {concept_stops} concept stops and {other_stops} other stops — one \
             arm of this check is running on an empty set",
        );
        assert!(
            bad.is_empty(),
            "{} stop(s) disagree with their tour's declared kind:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("stops checked against their kind: {concept_stops} concept, {other_stops} other");
    }

    /// **No tour calls its units "acts" again.**
    ///
    /// A ratchet, in the shape `app_does_not_regrow_its_field_count` established. The
    /// rename cost 110 edits across ten tours and it would come back one heading at a
    /// time, written from the memory of a tour read an hour earlier — which is exactly how
    /// the word arrived in the first place: `matching.md` shipped with Acts on the same
    /// day `dae-construction.md` shipped with Stops, and nobody noticed for thirteen days.
    #[test]
    fn no_tour_heading_calls_a_stop_an_act() {
        let mut bad: Vec<String> = Vec::new();
        for (name, _kind, text) in tours_with_kinds() {
            for (i, line) in text.lines().enumerate() {
                if line.starts_with("## ") && (line.contains("Act ") || line.contains("Scene ")) {
                    bad.push(format!("{name}.md:{}  {}", i + 1, line.trim()));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} heading(s) call a stop an act or a scene. The top-level noun is `tour`, so \
             the unit is a `stop`; theatre vocabulary casts the reader as an audience, and \
             the tours exist to make him an instrument:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
    }

    /// **`matching-live.md` keeps its three words apart.**
    ///
    /// This is the one document where **stop** (a place in the tour), **break** (where the
    /// debugger halts) and **anchor** (the named location a break is armed at) are all in
    /// play at once. Before the rename its units were "acts", so "a stop" unambiguously
    /// meant the debugger; afterwards the same phrase reads most naturally as the *wrong*
    /// sense — the rename **created** this collision rather than exposing one.
    ///
    /// **The mitigation is the vocabulary note, so the note is what gets checked.** A
    /// general prose linter would have to infer which sense a noun carries, and inferring
    /// identity from words is the thing `identity-and-provenance.md` forbids outright.
    /// This check is exact instead: the note must be present, and the two phrasings that
    /// were actually wrong must not come back.
    #[test]
    fn the_live_tour_keeps_stop_break_and_anchor_apart() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours/matching-live.md");
        let text = std::fs::read_to_string(&path).expect("matching-live.md must be readable");

        assert!(
            text.contains("A **break** is where the *debugger* halts execution."),
            "matching-live.md must open with the note distinguishing stop, break and \
             anchor. It is the only tour where all three are live, and the note is the \
             whole mitigation for a collision the Act→Stop rename created.",
        );
        for phrase in ["debugger stop", "at every stop", "about a stop"] {
            assert!(
                !text.contains(phrase),
                "matching-live.md says {phrase:?}, which reads as a tour stop in a document \
                 whose units are stops. Say `break` — the tour already writes `⬤ Break at \
                 the free-versus-displace decision` and the scheme is `hrw://breakpoint/`.",
            );
        }
    }

    /// **A `<tour>.md Stop N` reference in the source resolves to a real heading.**
    ///
    /// # The gap this closes
    ///
    /// Nineteen comments in `src/` named a tour unit — *"evidence for
    /// `connect-expansion.md` Act 1"*, *"`matching.md` ends Act 3 with one"* — and every
    /// one of them dangled the moment the tours were renamed. Nothing noticed, because a
    /// comment is not compiled and a stale one is indistinguishable from a live one.
    ///
    /// **These are navigational, not decorative.** A comment that says *"see Stop 4"* is
    /// how the next session finds the prose a piece of code exists to serve; pointing at a
    /// stop that is not there sends it looking for something that never arrives.
    ///
    /// **Quotations are exempt, and that exemption is the interesting part.** Six comments
    /// quote Doug saying "Act", and they stay verbatim forever — editing a quotation so it
    /// matches a rename that happened afterwards falsifies the record this repository's
    /// whole discipline rests on. So the check skips a line quoting him, which is why it
    /// looks only for the *current* vocabulary.
    #[test]
    fn a_stop_a_source_comment_cites_exists_in_that_tour() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let tours = hrw.join("docs/fixture-tours");

        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        let mut sources: Vec<PathBuf> = Vec::new();
        collect_rust(&hrw.join("src"), &mut sources);
        assert!(
            sources.len() >= 10,
            "found only {} source files — the scan is broken",
            sources.len(),
        );

        for src in sources {
            let Ok(text) = std::fs::read_to_string(&src) else {
                continue;
            };
            let src_name = src.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            for (i, line) in text.lines().enumerate() {
                // A tour named in backticks, then `Stop <n>` anywhere later on the same
                // line. **Adjacency was the first implementation and it was too narrow** —
                // it matched 3 of the 19 real references, because most of them read
                // "`matching.md` ends Stop 3 with one" rather than "`matching.md` Stop 3".
                // A check that inspects a sixth of its subject is most of the way to
                // vacuous while looking green.
                let Some(stop_at) = line.find("Stop ") else {
                    continue;
                };
                let n: String = line[stop_at + "Stop ".len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if n.is_empty() {
                    continue;
                }
                // The nearest tour named *before* the citation owns it.
                let head = &line[..stop_at];
                let Some(md) = head.rfind(".md`") else {
                    continue;
                };
                let Some(open) = head[..md].rfind('`') else {
                    continue;
                };
                let tour = &head[open + 1..md];
                if tour.is_empty() || tour.contains(' ') || tour.contains('/') {
                    continue;
                }

                checked += 1;
                let Ok(body) = std::fs::read_to_string(tours.join(format!("{tour}.md"))) else {
                    bad.push(format!("{src_name}:{}  no tour named {tour:?}", i + 1));
                    continue;
                };
                if !body.contains(&format!("Stop {n} ")) {
                    bad.push(format!("{src_name}:{}  {tour}.md has no Stop {n}", i + 1));
                }
            }
        }

        // **What this does NOT reach, said out loud rather than left as a green result.**
        // A citation whose tour is named on an *earlier* line of the same doc comment —
        // "/// **`connect-expansion.md` Stop 1's node sizes…**" followed later by "Stop 1
        // counts **nodes**" — is invisible here, because the pairing is per-line. Roughly
        // half the references in `doc_citations.rs` are that shape. Widening to whole doc
        // comments is possible and was not done; this checker covers the single-line form
        // and says so.
        assert!(
            checked >= 6,
            "only {checked} stop citations were inspected — the extraction is broken, so \
             this check would pass over a source tree full of dangling references",
        );
        assert!(
            bad.is_empty(),
            "{} source comment(s) cite a stop that does not exist:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("source comments citing a tour stop: {checked}");
    }

    /// **An equation id a tour cites must name the equation the prose claims.**
    ///
    /// # The gap this closes, found by falling into it
    ///
    /// `equation_text_quoted_in_tours_matches_the_traces` above verifies quoted
    /// *text*. Nothing verified a quoted **id**. On 2026-08-16 a new act of
    /// `connect-expansion.md` was written claiming `C.v` reads `der in f_x[19]` —
    /// a real, existing equation, and the wrong one: `f_x[19]` is the connection
    /// equation `src.p.v = R.p.v`, while the capacitor's rate law is `f_x[14]`.
    /// The number was written from memory of one seen an hour earlier.
    ///
    /// **Every existing checker would have passed it.** The id is well-formed, the
    /// equation exists, the link resolves, and no quoted text was wrong. Doug would
    /// have walked to that stop, seen `der in f_x[14]` on screen, and had to work out
    /// which of the two of us was mistaken — the precise failure this repository
    /// exists to prevent, since he cannot tell which parts are false.
    ///
    /// # What it checks
    ///
    /// The Why column renders `der in f_x[N]`, so a tour quoting that string is
    /// quoting a **pane cell**, and the cell is checkable against the committed
    /// trace without a compile:
    ///
    /// 1. **Equation N exists** in the specimen the surrounding section targets.
    /// 2. **Equation N contains a derivative.** `f_x[19]` fails here.
    /// 3. **If the same line names a variable in backticks** and some equation of
    ///    that specimen differentiates it, equation N must be *that* equation. This
    ///    is the check that catches citing the right kind of equation about the
    ///    wrong variable — a two-state model where the ids are swapped.
    ///
    /// The specimen comes from the nearest preceding `hrw://load/<Specimen>/` link,
    /// which the tour template guarantees: every expectation follows a **▶ Look**
    /// link. A citation with no preceding link is counted and skipped rather than
    /// guessed at.
    ///
    /// Fast by construction — `structural.json` carries every `id` and
    /// `equation_text`, so nothing here compiles anything.
    ///
    /// # What the current corpus does NOT exercise, stated rather than left silent
    ///
    /// **Check 3 is vacuous today.** The only tour citing a Why cell is
    /// `connect-expansion.md`, on `RcCircuit`, which has exactly **one** derivative
    /// equation — so a wrong id there always fails check 2 first, and the
    /// wrong-variable branch never runs. That is the same shape as the coupled-block
    /// hole found on 2026-08-15: an assertion that reads as coverage while never
    /// executing.
    ///
    /// It was verified by hand instead, and the probe is recorded so it can be
    /// repeated: point the section at `BouncingBall` (which differentiates `h` in
    /// `f_x[0]` and `v` in `f_x[1]`) and cite `h` with `f_x[1]`. It fails with
    /// *"says `h` reads `der in f_x[1]`, but BouncingBall differentiates it in
    /// f_x[0], not f_x[1]"*. **The first tour to quote a Why cell on a multi-state
    /// specimen makes this real**, and until then check 3 is a guard nothing proves
    /// still works.
    #[test]
    fn an_equation_id_a_tour_cites_names_the_equation_the_prose_claims() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // specimen -> (id -> equation_text), straight from the committed traces.
        let mut by_specimen: std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, String>,
        > = std::collections::BTreeMap::new();
        let notebook = hrw.join("docs/specimen-notebook");
        for entry in std::fs::read_dir(&notebook).expect("notebook").flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let Ok(text) = std::fs::read_to_string(entry.path().join("trace/structural.json"))
            else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            let mut map = std::collections::BTreeMap::new();
            if let Some(rows) = v["incidence"]["rows"].as_array() {
                for row in rows {
                    if let (Some(id), Some(t)) = (row["id"].as_str(), row["equation_text"].as_str())
                    {
                        map.insert(id.to_owned(), t.to_owned());
                    }
                }
            }
            if !map.is_empty() {
                by_specimen.insert(name, map);
            }
        }
        assert!(
            by_specimen.len() >= 10,
            "only {} specimens yielded equations; the trace shape changed and this \
             check is inspecting nothing",
            by_specimen.len(),
        );

        let mut checked = 0usize;
        let mut ledger = SkipLedger::default();
        let mut bad: Vec<String> = Vec::new();

        let mut tour_files: Vec<PathBuf> = Vec::new();
        collect_markdown(&hrw.join("docs/fixture-tours"), &mut tour_files);

        for path in tour_files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let tour = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            let mut specimen: Option<String> = None;

            for (i, line) in text.lines().enumerate() {
                // The template puts a ▶ Look link before every expectation, so the
                // nearest preceding one names the specimen on screen.
                if let Some(at) = line.find("hrw://load/") {
                    let rest = &line[at + "hrw://load/".len()..];
                    let spec: String = rest
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !spec.is_empty() {
                        specimen = Some(spec);
                    }
                }

                let cites: Vec<String> = backticked(line)
                    .into_iter()
                    .filter(|q| q.starts_with("der in f_x["))
                    .collect();
                if cites.is_empty() {
                    continue;
                }
                let Some(spec) = specimen.as_deref() else {
                    ledger.skip(
                        &format!("{tour}:{}", i + 1),
                        "no preceding hrw://load link names the specimen",
                    );
                    continue;
                };
                ledger.inspected();
                let Some(equations) = by_specimen.get(spec) else {
                    bad.push(format!(
                        "{tour}:{}: cites an equation of `{spec}`, which has no committed \
                         trace",
                        i + 1
                    ));
                    continue;
                };

                // Variables named on the same line, and the equation (if any) that
                // differentiates each — used for check 3.
                let named: Vec<String> = backticked(line)
                    .into_iter()
                    .filter(|q| !q.starts_with("der in") && !q.starts_with("f_x["))
                    .collect();

                for cite in cites {
                    checked += 1;
                    let id = cite.trim_start_matches("der in ").trim();
                    let Some(eq_text) = equations.get(id) else {
                        bad.push(format!(
                            "{tour}:{}: cites `{id}`, which names no equation in {spec}",
                            i + 1
                        ));
                        continue;
                    };
                    if !eq_text.contains("der(") {
                        bad.push(format!(
                            "{tour}:{}: claims `{id}` differentiates something, but that \
                             equation has no derivative in it: {eq_text}",
                            i + 1
                        ));
                        continue;
                    }
                    for var in &named {
                        let needle = format!("der({var})");
                        let real = equations
                            .iter()
                            .find(|(_, t)| t.contains(&needle))
                            .map(|(k, _)| k.clone());
                        if let Some(real_id) = real
                            && real_id != id
                        {
                            bad.push(format!(
                                "{tour}:{}: says `{var}` reads `{cite}`, but {spec} \
                                 differentiates it in {real_id}, not {id}",
                                i + 1
                            ));
                        }
                    }
                }
            }
        }

        assert!(
            checked >= 1,
            "no `der in f_x[N]` citations were found in any tour; the extraction is \
             broken, or the Why column stopped being quoted (in which case delete \
             this test rather than let it pass on nothing)",
        );
        assert!(
            bad.is_empty(),
            "{} tour citation(s) name the wrong equation:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        // **Budget zero.** Every citation in a committed tour follows a ▶ Look link,
        // because the template requires one. A skip here means a tour stopped saying
        // which specimen it is talking about — a tour defect, not a reason for this
        // check to quietly cover less.
        ledger.assert_coverage("tour equation-id citations", 1, 0);
    }

    /// **Every test target runs in the gate the documentation names.**
    ///
    /// # The gap this closes
    ///
    /// Doug, 2026-08-16: *"Do all of the checkers which you have implemented run when
    /// you execute the full gate testing prior to commits?"* The answer was **no**, and
    /// one of the two reasons was an accident: the gate command said `--lib`, which is
    /// a **filter**, and `tests/msl_resolve.rs` had therefore not run in any pre-commit
    /// gate since at least 2026-08-05. Its two tests prove the MSL dependency-loading
    /// path resolves `Modelica.*` references end to end. They passed when finally run,
    /// and cost 6.3 s.
    ///
    /// **Nothing was broken, and nothing would have said so either** — which is the
    /// whole problem. A filter is silent about what it removes, so an unrun test is
    /// indistinguishable from a passing one in every report anyone looks at.
    ///
    /// # Why this is a documentation check rather than a build check
    ///
    /// Cargo cannot be asked "is this target in someone's habitual command line". The
    /// gate lives in `CLAUDE.md` as text, so the text is what has to be checked: every
    /// file in `tests/` must be named by the gate command, or the command must select
    /// all targets. Adding `tests/foo.rs` and not updating the gate now fails here
    /// rather than in six weeks.
    ///
    /// It deliberately does **not** check the reverse — that everything named still
    /// exists — because `fixture_tour_links_all_resolve` and the citation checks
    /// already fail loudly on a path that has moved.
    #[test]
    fn every_test_target_runs_in_the_documented_gate() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        let mut targets: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(hrw.join("tests")) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("rs")
                    && let Some(stem) = p.file_stem().and_then(|s| s.to_str())
                {
                    targets.push(stem.to_owned());
                }
            }
        }

        let claude_md = std::fs::read_to_string(hrw.join("CLAUDE.md")).expect("CLAUDE.md");
        // The gate line, as the file spells it. Located rather than assumed, so a
        // reworded section fails here instead of silently matching nothing.
        // **Not merely "contains slow-tests"** — the ITERATE line does too, and the
        // first draft of this test found *that* one and reported a failure about the
        // wrong command. The gate is the unfiltered invocation, so the placeholder is
        // what distinguishes them.
        let gate = claude_md
            .lines()
            .find(|l| {
                l.starts_with("cargo test -p hrw")
                    && l.contains("--features slow-tests")
                    && !l.contains("<name-filter>")
            })
            .unwrap_or_else(|| {
                panic!(
                    "CLAUDE.md no longer contains an unfiltered `cargo test -p hrw … \
                     --features slow-tests` line, so the gate this test checks against \
                     cannot be found"
                )
            });

        assert!(
            !targets.is_empty(),
            "no integration test targets were found; if `tests/` was deleted this test \
             should go with it, and if it was moved this check is now inspecting nothing",
        );

        let selects_everything = !gate.contains("--lib");
        let missing: Vec<&String> = targets
            .iter()
            .filter(|t| !selects_everything && !gate.contains(&format!("--test {t}")))
            .collect();

        assert!(
            missing.is_empty(),
            "these integration test targets exist but the documented gate does not run \
             them, so they pass or fail unobserved: {missing:?}\n  gate: {gate}",
        );
    }

    /// **Every structural feature a checker depends on is exhibited by some specimen,
    /// and this names which.**
    ///
    /// # The failure this exists to stop, which happened three times
    ///
    /// A check can only exercise what its specimen contains. When the corpus lacks a
    /// feature, an assertion about that feature **runs zero times and reports
    /// success** — indistinguishable from coverage in every report anyone reads.
    ///
    /// Measured 2026-08-16, and the shape was stark: the ten specimens with
    /// derivative equations had **zero** coupled blocks, and the four with coupled
    /// blocks had **zero** derivative equations. No model had both. So a check needing
    /// both found one absent whichever specimen it chose. It cost:
    ///
    /// - the coupled-block branch of `an_equation_id_names_the_same_equation_in_every_pane`,
    ///   asserted on `RcCircuit` (0 coupled blocks) and never executed;
    /// - check 3 of `an_equation_id_a_tour_cites_names_the_equation_the_prose_claims`,
    ///   vacuous because `RcCircuit` has one derivative equation;
    /// - the 2026-08-02 corpus sweep, whose F-checks "found nothing because there was
    ///   nothing there".
    ///
    /// `LoopWithInertia` was authored to close it — a servo loop closed around a real
    /// inertia, so one model carries a torn coupled block *and* a state.
    ///
    /// # What this checks, and what it deliberately does not
    ///
    /// For each feature: at least one specimen exhibits it, **and the specimen is
    /// named in the failure message**, so the next person writing a check knows where
    /// to point rather than discovering the gap by writing a test that passes.
    ///
    /// It does **not** assert exact counts. Counts are the notebook's job and change
    /// whenever a specimen is edited; this asserts only that the *capability* exists,
    /// which is the property a check author needs.
    ///
    /// Fast — reads committed traces, compiles nothing.
    #[test]
    fn the_corpus_covers_every_feature_the_checkers_need() {
        let notebook = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/specimen-notebook");

        struct Seen {
            der_equations: Vec<String>,
            multi_der: Vec<String>,
            coupled: Vec<String>,
            tearing: Vec<String>,
            both: Vec<String>,
            failures: Vec<String>,
        }
        let mut seen = Seen {
            der_equations: Vec::new(),
            multi_der: Vec::new(),
            coupled: Vec::new(),
            tearing: Vec::new(),
            both: Vec::new(),
            failures: Vec::new(),
        };
        let mut specimens = 0usize;

        for entry in std::fs::read_dir(&notebook)
            .expect("the notebook exists")
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();

            // A specimen that fails to compile is a feature too — `#46` is built on
            // them, and F10's absence clause has nothing to act on without one.
            if let Ok(m) = std::fs::read_to_string(dir.join("trace/manifest.json"))
                && let Ok(v) = serde_json::from_str::<serde_json::Value>(&m)
                && v["stages"]
                    .as_object()
                    .is_some_and(|s| s.values().any(|st| st["has_ir"] == false))
            {
                seen.failures.push(name.clone());
            }

            let Ok(text) = std::fs::read_to_string(dir.join("trace/structural.json")) else {
                continue;
            };
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
                continue;
            };
            specimens += 1;

            let ders = v["incidence"]["rows"].as_array().map_or(0, |rows| {
                rows.iter()
                    .filter(|r| {
                        r["equation_text"]
                            .as_str()
                            .is_some_and(|t| t.contains("der("))
                    })
                    .count()
            });
            let blocks = v["blocks"].as_array();
            let coupled = blocks.map_or(0, |b| b.iter().filter(|x| x["kind"] == "coupled").count());
            let torn = blocks.map_or(0, |b| b.iter().filter(|x| x["tearing"].is_object()).count());

            if ders > 0 {
                seen.der_equations.push(name.clone());
            }
            if ders > 1 {
                seen.multi_der.push(name.clone());
            }
            if coupled > 0 {
                seen.coupled.push(name.clone());
            }
            if torn > 0 {
                seen.tearing.push(name.clone());
            }
            if ders > 0 && coupled > 0 {
                seen.both.push(name.clone());
            }
        }

        assert!(
            specimens >= 15,
            "only {specimens} specimens had a structural trace; the notebook shrank or \
             the trace shape changed, and this check is inspecting almost nothing",
        );

        // (feature, specimens exhibiting it, why a checker needs it)
        let required: [(&str, &Vec<String>, &str); 5] = [
            (
                "a derivative equation",
                &seen.der_equations,
                "anything about states, integration, or the Why column",
            ),
            (
                "TWO OR MORE derivative equations",
                &seen.multi_der,
                "distinguishing which variable an equation differentiates \u{2014} with \
                 one der equation a wrong citation is caught by luck, not by the check",
            ),
            (
                "a coupled BLT block",
                &seen.coupled,
                "simultaneous-solve rendering, and every `blocks[].ids` assertion",
            ),
            (
                "a tearing report",
                &seen.tearing,
                "`residual_equations` and `causal_sequence`, which had no identity at \
                 all until 2026-08-15",
            ),
            (
                "a coupled block AND a derivative in ONE model",
                &seen.both,
                "any check that needs to see tearing and integration interact; its \
                 absence silently disabled three separate assertions",
            ),
        ];

        let mut missing: Vec<String> = Vec::new();
        for (feature, specimens, why) in required {
            if specimens.is_empty() {
                missing.push(format!("{feature} \u{2014} needed for: {why}"));
            }
        }
        assert!(
            missing.is_empty(),
            "the corpus no longer exhibits {} structural feature(s), so any check \
             relying on them now passes without running:\n  {}",
            missing.len(),
            missing.join("\n  "),
        );

        // Printed, not asserted: the map is for whoever writes the next check.
        println!("corpus feature map ({specimens} specimens with a structural trace):");
        for (feature, specimens, _) in required {
            println!("  {feature}: {}", specimens.join(", "));
        }
        println!("  a failing stage: {}", seen.failures.join(", "));
    }

    /// What a check declined to inspect, and why.
    ///
    /// # Absence is where everything hides
    ///
    /// Every checker in this file walks subjects — specimens, tours, stage files —
    /// and every one of them has `continue` arms for subjects it cannot read. Those
    /// arms are invisible: the check reports what it *found*, never what it *passed
    /// over*, so a deleted trace or a renamed tour quietly shrinks coverage while the
    /// suite stays green.
    ///
    /// That is the same failure as `--lib` silently skipping `tests/`, and as
    /// `git diff` being blind to untracked files: **a filter is silent about what it
    /// removed.** Both cost real coverage in the same week.
    ///
    /// So a skip is *recorded* rather than merely taken, and the count is asserted
    /// against what the corpus should produce. A skip that is expected (a failure
    /// specimen has no structural report) stays legal; a skip that is *new* trips the
    /// bound and names itself.
    #[derive(Default)]
    struct SkipLedger {
        inspected: usize,
        skipped: Vec<String>,
    }

    impl SkipLedger {
        fn inspected(&mut self) {
            self.inspected += 1;
        }

        fn skip(&mut self, subject: &str, why: &str) {
            self.skipped.push(format!("{subject} ({why})"));
        }

        /// Assert the check saw enough, and skipped no more than expected.
        ///
        /// `max_skipped` is a **budget, not a target**: it exists so that an increase
        /// fails loudly. Raising it is a deliberate act that belongs in the same
        /// commit as the reason.
        fn assert_coverage(&self, what: &str, min_inspected: usize, max_skipped: usize) {
            assert!(
                self.inspected >= min_inspected,
                "{what}: only {} subject(s) inspected, expected at least \
                 {min_inspected} \u{2014} the walk is broken, not the corpus",
                self.inspected,
            );
            assert!(
                self.skipped.len() <= max_skipped,
                "{what}: skipped {} subject(s), budget is {max_skipped}. A skip is lost \
                 coverage that reports as success, so either restore them or raise the \
                 budget in the same commit as the reason:\n  {}",
                self.skipped.len(),
                self.skipped.join("\n  "),
            );
            println!(
                "{what}: {} inspected, {} skipped{}",
                self.inspected,
                self.skipped.len(),
                if self.skipped.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", self.skipped.join("; "))
                },
            );
        }
    }

    /// **Every field of a published struct reaches the JSON that claims to serialize it.**
    ///
    /// # The defect this would have caught, hours before Doug did
    ///
    /// `EquationSheet::to_bridge_json` promises in its own doc comment that a field
    /// the renderer draws cannot be missing from `view.json`, *"because the renderer
    /// has no other source"*. Adding `ClassifiedVariable::derivative_evidence` on
    /// 2026-08-16 made that false within the hour: the Why column was drawn from it,
    /// and the bridge published `kind`, `start` and `unit` only. `view.json` described
    /// a pane that no longer existed.
    ///
    /// It was found because Doug asked whether the tours needed updating — that is,
    /// **by a question, not by the toolchain.** `tech-debt.md`'s backward sweep trigger
    /// fires on exactly that.
    ///
    /// # Why a source-text check, and why that is uncomfortable
    ///
    /// Rust has no reflection, so "did every field get serialized" cannot be asked of
    /// the type. The alternative — a hand-maintained list — is the second roster that
    /// `gen_trace`'s `const STAGES` already proved rots.
    ///
    /// **Source-text checks are the ones that keep matching their own prose**: three
    /// separate near-misses in two days, including one that searched for a symbol its
    /// own comment mentioned. So this one is deliberately narrow: it reads the field
    /// names out of a `struct` block and asks whether each appears as a **JSON key**
    /// (`"name":`) inside the function body, which is a shape prose does not have.
    ///
    /// # What it cannot see
    ///
    /// A field published under a *different* key, and a key whose value is wrong. It
    /// answers "was this field considered", not "was it serialized correctly" —
    /// `the_published_variables_carry_the_evidence_the_pane_draws` owns the latter for
    /// the field that matters. Stated because a check that looks broader than it is
    /// will be trusted for the wider claim.
    #[test]
    fn every_published_struct_field_appears_in_its_bridge_json() {
        let src = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/equation_sheet.rs"),
        )
        .expect("equation_sheet.rs");

        // Fields of `pub struct ClassifiedVariable { … }`, taken from the source.
        let start = src
            .find("pub struct ClassifiedVariable {")
            .expect("ClassifiedVariable is still declared here");
        let body = &src[start..];
        let end = body.find("\n}").expect("struct closes");
        let fields: Vec<String> = body[..end]
            .lines()
            .filter_map(|l| {
                let t = l.trim();
                // `pub name: String,` — declarations only, never doc comments.
                let rest = t.strip_prefix("pub ")?;
                let (name, _) = rest.split_once(':')?;
                let name = name.trim();
                name.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_')
                    .then(|| name.to_owned())
            })
            .collect();

        assert!(
            fields.len() >= 5,
            "only {} fields were extracted from ClassifiedVariable; the struct was \
             reshaped and this check is now inspecting almost nothing: {fields:?}",
            fields.len(),
        );

        // The body of `to_bridge_json`, where the variable rows are built.
        let f_start = src
            .find("pub fn to_bridge_json")
            .expect("to_bridge_json is still here");
        let json_body = &src[f_start..];
        let f_end = json_body.find("\n    }\n").expect("to_bridge_json closes");
        let json_body = &json_body[..f_end];

        // `name` is published as `id`, deliberately: every bridge row spells its
        // identity `id` so that "this variable" resolves across panes the same way
        // `f_x[N]` does for equations. Named here rather than special-cased silently.
        const PUBLISHED_AS: [(&str, &str); 1] = [("name", "id")];
        // Fields with no place in a published view. Each needs a reason, and the
        // reason is what a future reader will check rather than re-derive.
        const NOT_PUBLISHED: [(&str, &str); 1] = [(
            "description",
            "shown in the pane's tooltip only; never a row field",
        )];

        let mut missing: Vec<String> = Vec::new();
        for field in &fields {
            if let Some((_, why)) = NOT_PUBLISHED.iter().find(|(f, _)| f == field) {
                let _ = why;
                continue;
            }
            let key = PUBLISHED_AS
                .iter()
                .find(|(f, _)| f == field)
                .map_or(field.as_str(), |(_, k)| k);
            if !json_body.contains(&format!("\"{key}\":")) {
                missing.push(format!("`{field}` (expected JSON key `{key}`)"));
            }
        }

        assert!(
            missing.is_empty(),
            "{} field(s) of ClassifiedVariable never reach `to_bridge_json`, so \
             `view.json` describes a pane that is not the pane on screen: {}\n\
             If a field genuinely should not be published, add it to NOT_PUBLISHED \
             with the reason.",
            missing.len(),
            missing.join(", "),
        );
    }

    /// **The tearing tour gains its dynamic-loop act at the moment it is converted.**
    ///
    /// **DELIVERED 2026-08-17.** `tearing.md` was converted with `LoopWithInertia` as its
    /// Stop 5, so this test now runs its *enforcing* branch — 5 `**Predict.**` markers and
    /// an `hrw://load/LoopWithInertia` link — rather than the not-yet-converted early
    /// return. The `## OWED` note it used to guard is gone because the act replaced it,
    /// which is the outcome the note asked for. What remains guarded: the act cannot be
    /// removed while the tour stays converted.
    ///
    /// # A commitment, made mechanical
    ///
    /// Doug, 2026-08-16: *"Eventually, I will want very much to add LoopWithInertia to
    /// the tearing tour, as you've recommended. Please ensure that we do that."* He is
    /// walking the tours in compiler-phase order and is on Connections → DAE, so
    /// tearing is weeks away. A promise made now, in a conversation, is exactly the
    /// thing `CLAUDE.md` says must live in the repository instead: *code whose
    /// rationale exists only in chat violates the rule the moment the session ends.*
    ///
    /// # Why it triggers on conversion rather than on a date
    ///
    /// The act cannot simply be written today — `tearing.md` is still in its
    /// 2026-08-08 prose form, and the agreement is that a tour is converted **as Doug
    /// walks it**, because the conversion is itself the teaching. So the commitment
    /// has to survive until that moment and fire precisely then.
    ///
    /// Conversion is detectable: a converted tour runs each act to a **Predict**. Only
    /// `connect-expansion.md` has been converted, and it carries six. So two or more
    /// `**Predict.**` markers means the work has started, and from that instant the
    /// tour must also mention `LoopWithInertia`.
    ///
    /// **It cannot fire early** — `tearing.md` has zero markers today, so this passes
    /// until someone begins the conversion, which is the only time the reminder is
    /// worth anything.
    ///
    /// # What it does not claim
    ///
    /// That the act is *good*, or in the right place. It checks that the specimen is
    /// named, not that the teaching landed — the half no test reaches, and the half
    /// Doug reports.
    #[test]
    fn the_tearing_tour_gains_its_dynamic_loop_when_it_is_converted() {
        let tour = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours/tearing.md");
        let text = std::fs::read_to_string(&tour).expect("tearing.md exists");

        // The owed-work note must survive until the work is done: deleting it is how
        // a commitment quietly stops existing.
        assert!(
            text.contains("LoopWithInertia"),
            "tearing.md no longer mentions LoopWithInertia at all. The owed final act \
             was recorded on 2026-08-16 at Doug's explicit request; if it has been \
             delivered the note should say so, and if it has been abandoned that is a \
             decision to record rather than a line to delete.",
        );

        let predicts = text.matches("**Predict.**").count();
        let converted = predicts >= 2;
        if !converted {
            // Not yet walked. Nothing to enforce, and saying so keeps the pass honest
            // rather than silent.
            println!(
                "tearing.md is not yet converted ({predicts} Predict marker(s)); the \
                 LoopWithInertia act is still owed"
            );
            return;
        }

        // Converted. The specimen must now be a subject, not only a promise: a ▶ Look
        // link is what makes an act walkable.
        assert!(
            text.contains("hrw://load/LoopWithInertia"),
            "tearing.md has been converted to the Predict/Look template ({predicts} \
             Predict markers) but still has no ▶ Look link for LoopWithInertia. The \
             owed act is: the same 3-cycle as Stop 1, now re-solved between every pair \
             of integrator steps \u{2014} what a coupled block costs when time is \
             advancing.",
        );
    }

    /// Recursively harvest `equation` labels and `equation_text` residuals.
    fn collect_equation_strings(
        v: &serde_json::Value,
        labels: &mut BTreeSet<String>,
        residuals: &mut BTreeSet<String>,
    ) {
        match v {
            serde_json::Value::Object(map) => {
                for (k, val) in map {
                    if let Some(s) = val.as_str() {
                        match k.as_str() {
                            "equation" => {
                                labels.insert(s.to_owned());
                            }
                            "equation_text" => {
                                residuals.insert(s.to_owned());
                            }
                            _ => {}
                        }
                    }
                    collect_equation_strings(val, labels, residuals);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_equation_strings(item, labels, residuals);
                }
            }
            _ => {}
        }
    }

    /// The contents of every `` `backticked` `` span on a line.
    fn backticked(line: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let after = &rest[start + 1..];
            match after.find('`') {
                Some(end) => {
                    out.push(after[..end].to_owned());
                    rest = &after[end + 1..];
                }
                None => break,
            }
        }
        out
    }
}

#[cfg(test)]
mod tests_tour_link_form {
    use std::path::PathBuf;

    /// **A tour referenced from a tour must be an `hrw://` link, not a file link.**
    ///
    /// Doug, 2026-08-19, testing the new Back control: *"I found a broken link in the
    /// blt-ordering tour. The link for matching.md causes an external browser window to
    /// open instead of opening that tour in HRW."*
    ///
    /// **There were eighteen of them.** HRW's commonmark renderer hands a relative file
    /// link to the operating system, so `[`matching.md`](matching.md)` opens a browser —
    /// or nothing — while looking in the source exactly like a working navigation. That
    /// is the same defect the hub back-links had on 2026-08-17, fixed there and never
    /// generalised, which is why it was still waiting in ten other documents.
    ///
    /// **Nothing could have caught it.** `fixture_tour_links_all_resolve` checks `hrw://`
    /// links; a markdown file link is not one, so it was invisible to every checker here
    /// — and invisible in the rendered pane too, since it looks like an ordinary link
    /// until clicked.
    ///
    /// The hub's `[file](x.md)` cells are exempt: they sit beside an `hrw://tour/…` link
    /// in the same cell and are labelled "file", which is a different offer rather than a
    /// broken one.
    #[test]
    fn a_tour_is_never_referenced_by_a_plain_file_link() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-tours");
        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("the tour directory must be readable") {
            let path = entry.expect("a readable entry").path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            if name == "CATALOGUE" || name == "README" || name == "the-concepts" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable");

            for (i, line) in text.lines().enumerate() {
                let mut rest = line;
                while let Some(at) = rest.find("](") {
                    let tail = &rest[at + 2..];
                    let Some(close) = tail.find(')') else { break };
                    let href = &tail[..close];
                    rest = &tail[close..];
                    if !href.ends_with(".md") || href.contains('/') {
                        continue;
                    }
                    checked += 1;
                    bad.push(format!("{name}.md:{}  [..]({href})", i + 1));
                }
            }
        }

        // **Non-vacuity.** The extraction must be looking at links at all; a scan finding
        // none would report a clean corpus over a corpus it never read.
        let total_links = std::fs::read_to_string(dir.join("index-reduction.md"))
            .expect("readable")
            .matches("](")
            .count();
        assert!(
            total_links > 5,
            "only {total_links} links seen in a tour known to be full of them \u{2014} the \
             scan is broken, not the corpus",
        );
        assert!(
            bad.is_empty(),
            "{} tour reference(s) use a plain markdown file link. HRW hands those to the \
             operating system, so they open a browser rather than the tour \u{2014} and \
             they look identical to a working link in the source. Use \
             `[\u{25b6} name](hrw://tour/name)`:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        let _ = checked;
    }
}

#[cfg(test)]
mod tests_orphaned_docs {
    use std::path::PathBuf;

    /// A `///` line that opens a **new summary in the middle of a doc block** — the
    /// shape a merged doc block has, and the only automatable half of the
    /// wrong-owner defect.
    ///
    /// Rust concatenates contiguous `///` lines, so an item inserted above another
    /// item's doc comment silently adopts it and the original loses its own. See
    /// `CLAUDE.md`, *"a doc comment can be adopted by the wrong function"*.
    ///
    /// The three conditions, all required, and the third is what keeps the rate
    /// tolerable — without it every wrapped line reports:
    ///
    /// 1. a non-blank `///` line,
    /// 2. whose previous line is a non-blank `///` that **ends a sentence**,
    /// 3. and whose next line is a bare `///`, so this line is a one-line paragraph.
    ///
    /// **Condition 3 is also this detector's known blind spot**, measured
    /// 2026-08-21: `lib.rs`'s `STEPPED_FRAME_DELAY` summary wraps onto a second
    /// line, so the real orphan above it was invisible here and had to be found by
    /// hand. Relaxing 3 to allow a two-line summary doubles the hit count
    /// (87 → 169 on the tree as it stood), which is why it is not relaxed.
    fn opens_a_second_summary(prev: &str, cur: &str, next: &str) -> bool {
        fn is_doc(s: &str) -> bool {
            s.trim_start()
                .strip_prefix("///")
                .is_some_and(|r| !r.trim().is_empty())
        }
        fn is_blank_doc(s: &str) -> bool {
            s.trim_start()
                .strip_prefix("///")
                .is_some_and(|r| r.trim().is_empty())
        }
        fn ends_sentence(s: &str) -> bool {
            let t = s.trim_end().trim_end_matches('"');
            t.ends_with(['.', '!', '?', '*', '`', ')', ']', '"'])
        }
        is_doc(cur) && is_doc(prev) && ends_sentence(prev) && is_blank_doc(next)
    }

    fn hits(text: &str) -> Vec<usize> {
        let lines: Vec<&str> = text.lines().collect();
        (1..lines.len().saturating_sub(1))
            .filter(|&i| opens_a_second_summary(lines[i - 1], lines[i], lines[i + 1]))
            .map(|i| i + 1)
            .collect()
    }

    /// **A doc block must not gain a second opening summary** — the merged-block
    /// defect, ratcheted per file.
    ///
    /// # Why a ratchet rather than a zero
    ///
    /// The shape is **necessary but not sufficient**: ordinary prose produces it
    /// whenever a wrapped continuation line happens to end a sentence. The
    /// 2026-08-21 sweep triaged all 87 hits in the tree and found **25 real
    /// orphans in 24 blocks** — a precision of about 29 %, far above the *"it may
    /// be largely noise"* the plan had guessed from a single sample, and far below
    /// anything that could be asserted to zero. The 65 that remain are all
    /// continuation lines, each read individually.
    ///
    /// # Why per file rather than one total
    ///
    /// **Forty files are at zero**, so a merged block in any of them fails here by
    /// name rather than nudging a global counter nobody can attribute. The
    /// eighteen with a budget are the prose-heavy modules.
    ///
    /// # What this cannot claim
    ///
    /// **29 % is precision on the STOCK, not on the flow.** It says what fraction
    /// of the hits standing on 2026-08-21 were defects; it says nothing about what
    /// fraction of the *next* hit will be. So a failure here is *"go and look"*,
    /// exactly like [`super::tests::app_does_not_regrow_its_field_count`] — and if
    /// the new hit is ordinary prose, raise that file's number **in the same
    /// commit as the reasoning**.
    ///
    /// The triage shortcut, measured on every one of the 25: list the file's
    /// **undocumented** items and match the orphaned summary to one by name. Every
    /// orphan but three paired that way, and reading the merged block top-down is
    /// far slower. The three exceptions are the variants with no owner to match —
    /// a doc left by a deleted field, one superseded by a rewrite, and one whose
    /// owner moved to another module.
    ///
    /// **This test's own prose is part of the corpus it scans**, which is why
    /// `doc_citations.rs` carries a budget at all; editing this comment can move
    /// that number, and that is the same self-reference
    /// `no_test_arms_a_breakpoint_on_the_watched_path` records one file over.
    #[test]
    fn no_doc_block_gains_a_second_summary() {
        // Measured 2026-08-21, after the sweep reattached 25 orphans. Every file
        // absent from this table must have zero.
        const BUDGET: &[(&str, usize)] = &[
            ("app.rs", 9),
            ("app/tests.rs", 4),
            ("bridge.rs", 9),
            ("colors.rs", 1),
            // 3 -> 5 on 2026-08-22, with the reasoning the ratchet requires. Both new
            // hits are inside ONE doc block — `walked_prose_never_changes_silently` —
            // which opens with its own summary and then uses `**bold**` to lead two
            // later paragraphs ("Deletion is the case that matters most", "And it
            // cannot tell whether a marked region deserves its marker"). That is this
            // file's prevailing style and the reason it carried a budget already.
            // Triaged by the documented shortcut: every item added in that commit —
            // `walked_regions`, `unterminated_walked`, the test, its module and each of
            // its tests — has its own summary, so there is no undocumented item for an
            // orphan to belong to. False positives, at the ~29 % precision this check
            // is documented to have.
            ("doc_citations.rs", 5),
            ("equation_sheet.rs", 1),
            ("lib.rs", 2),
            ("matching_anim.rs", 2),
            ("matching_ledger.rs", 1),
            ("model_list.rs", 1),
            ("source_view.rs", 1),
            ("spyplot.rs", 2),
            ("stage_view.rs", 1),
            ("sub_view_rows.rs", 1),
            ("tarjan_anim.rs", 2),
            ("tour_panel.rs", 1),
            ("ui_tests.rs", 5),
            // 19 → 20 on 2026-08-21 (`docs/ideas.md` #48, lever C). Triaged, not
            // waved through: the new hit is `worker.rs`'s
            // `compile_specimen_headless_matches_worker` doc, where a wrapped line
            // ends with a backtick and the next line is blank — the continuation-line
            // shape that accounts for 65 of the tree's 87 hits. The block has one
            // owner, that owner is documented, and the file has no undocumented item
            // for an orphan to belong to, which is the triage shortcut this test's
            // own doc prescribes.
            ("worker.rs", 20),
        ];

        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<(String, Vec<usize>)> = Vec::new();
        let mut walk = vec![src.clone()];
        while let Some(dir) = walk.pop() {
            for e in std::fs::read_dir(&dir)
                .expect("src must be readable")
                .flatten()
            {
                let p = e.path();
                if p.is_dir() {
                    walk.push(p);
                } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
                    let rel = p
                        .strip_prefix(&src)
                        .expect("under src")
                        .to_string_lossy()
                        .replace('\\', "/");
                    let text = std::fs::read_to_string(&p).expect("readable");
                    files.push((rel, hits(&text)));
                }
            }
        }

        // Non-vacuity: the scan reached the tree, not an empty directory.
        assert!(
            files.len() > 40,
            "only {} source files scanned \u{2014} the walk is broken, not the tree",
            files.len(),
        );

        let mut over: Vec<String> = Vec::new();
        for (rel, found) in &files {
            let allowed = BUDGET
                .iter()
                .find(|(f, _)| *f == rel)
                .map_or(0, |(_, n)| *n);
            if found.len() > allowed {
                let lines: Vec<String> = found.iter().map(usize::to_string).collect();
                over.push(format!(
                    "{rel}: {} hits, budget {allowed} (lines {})",
                    found.len(),
                    lines.join(", "),
                ));
            }
        }

        assert!(
            over.is_empty(),
            "a doc block gained a second opening summary \u{2014} check whether an item was \
             inserted above another item's doc comment, or a rewrite was written above the \
             old doc instead of replacing it. Triage by listing the file's UNDOCUMENTED \
             items and matching the orphaned summary to one by name. If it is ordinary \
             prose, raise the budget in the same commit as the reasoning:\n  {}",
            over.join("\n  "),
        );
    }

    /// **The detector still fires** — the must-fire half, since every
    /// assertion above is about an absence.
    ///
    /// Builds the exact shape the real defect has: one item's doc, ending a
    /// sentence, immediately followed by a second item's one-line summary.
    #[test]
    fn a_merged_doc_block_is_detected() {
        let merged = "/// The first item's summary.\n\
                      ///\n\
                      /// A body paragraph that ends a sentence.\n\
                      /// The second item's summary.\n\
                      ///\n\
                      /// Its own body.\n\
                      fn second() {}\n";
        assert_eq!(hits(merged), vec![4], "the merged summary must be found");

        let clean = "/// The only summary.\n\
                     ///\n\
                     /// A body paragraph that ends a sentence.\n\
                     ///\n\
                     /// Another body paragraph.\n\
                     fn only() {}\n";
        assert!(
            hits(clean).is_empty(),
            "a well-formed block must not report: {:?}",
            hits(clean),
        );
    }
}

/// **The guarded-region parser, tested without git or a compile.**
///
/// [`tests::editing_a_guarded_tour_table_needs_the_full_gate`] needs a git checkout to
/// say anything, and is honestly inert without one. These do not: they pin the two
/// properties the whole FAST/FULL decision rests on — **a prose edit is not a guarded
/// change, and a table edit is** — as pure functions of text.
///
/// They are also the must-fire half. A fingerprint that ignored the rows would call
/// every edit safe, and nothing else here would notice.
#[cfg(test)]
mod tests_guarded_regions {
    use super::tests::{guarded_regions, marked_rows};

    /// A tour with one guarded table and prose on both sides of it.
    fn tour(rows: &str, prose: &str) -> String {
        format!(
            "# A tour\n\n{prose}\n\n<!-- pane-groups: RcCircuit -->\n\n\
             | group | rows |\n|---|---|\n{rows}\n\nMore prose about {prose}.\n"
        )
    }

    #[test]
    fn editing_prose_is_not_a_guarded_change() {
        let before = tour("| `Component equations` | 16 |", "the voltages are equal");
        let after = tour("| `Component equations` | 16 |", "the potentials are equal");
        assert_ne!(before, after, "the fixture must actually differ");
        assert_eq!(
            guarded_regions(&before),
            guarded_regions(&after),
            "a prose edit was reported as a guarded change, which would send every walk \
             edit to the 220s gate",
        );
    }

    #[test]
    fn editing_a_guarded_row_is_a_guarded_change() {
        let before = tour("| `Component equations` | 16 |", "same prose");
        let after = tour("| `Component equations` | 17 |", "same prose");
        assert_ne!(
            guarded_regions(&before),
            guarded_regions(&after),
            "a changed table row was reported as safe: this is the silent wrong negative \
             the gate check exists to prevent",
        );
    }

    #[test]
    fn removing_a_marker_is_a_guarded_change() {
        let before = tour("| `Component equations` | 16 |", "same prose");
        let after = before.replace("<!-- pane-groups: RcCircuit -->", "");
        assert!(
            !guarded_regions(&before).is_empty() && guarded_regions(&after).is_empty(),
            "removing a marker must change the fingerprint -- it removes a table from \
             verification entirely",
        );
    }

    /// **A marker whose table was deleted must not adopt the next table in the file.**
    ///
    /// The bounded walk added 2026-08-22. Unbounded, `pane-groups` here would skip past
    /// the prose and return the `pane-origins` rows, comparing one claim against another
    /// model's numbers and reporting confident nonsense.
    #[test]
    fn a_marker_whose_table_is_gone_does_not_adopt_a_later_one() {
        let text = "<!-- pane-groups: RcCircuit -->\n\nThe table that belonged here is gone.\n\n\
                    <!-- pane-origins: RcCircuit -->\n\n| origin | rows |\n|---|---|\n\
                    | `connect(src.p, R.p)` | 2 |\n";
        let groups = marked_rows(text, "pane-groups", "RcCircuit")
            .expect("the marker is present, so this is Some");
        assert!(
            groups.is_empty(),
            "a marker with no table adopted a later table's rows: {groups:?}",
        );
        let origins = marked_rows(text, "pane-origins", "RcCircuit").expect("marker present");
        assert_eq!(
            origins.len(),
            1,
            "the later table must still parse on its own"
        );
    }

    /// Blank lines between the marker and its table are normal markdown and must not
    /// end the region before it starts.
    #[test]
    fn a_blank_line_after_the_marker_does_not_end_the_region() {
        let text = "<!-- pane-groups: RcCircuit -->\n\n| group | rows |\n|---|---|\n\
                    | `Component equations` | 16 |\n| `Flow conservation` | 3 |\n";
        let rows = marked_rows(text, "pane-groups", "RcCircuit").expect("marker present");
        assert_eq!(rows.len(), 2, "both data rows must be read: {rows:?}");
        assert_eq!(rows[0], vec!["Component equations", "16"]);
    }
}

/// **The walked-region parser, tested without git.**
///
/// [`tests::walked_prose_never_changes_silently`] needs a git checkout to say anything
/// and is honestly inert without one. These do not. They are the **must-fire** half:
/// a parser that ignored the body, or that let an unterminated marker run to the end of
/// the file, would call every edit safe — and a check that cannot fail is
/// indistinguishable from no check, which is the failure mode this whole mechanism
/// exists to answer.
#[cfg(test)]
mod tests_walked_regions {
    use super::tests::{unterminated_walked, walked_regions};

    /// A tour with one walked region, and prose outside it on both sides.
    fn tour(date: &str, walked: &str, outside: &str) -> String {
        format!(
            "# A tour\n\n{outside}\n\n<!-- walked: the-opening {date} -->\n\
             {walked}\n<!-- /walked -->\n\nLater prose: {outside}.\n"
        )
    }

    /// The body is what is protected, so it must be what the scan reports.
    #[test]
    fn a_walked_region_reports_its_slug_date_and_body() {
        let text = tour("2026-08-22", "One edge per member.", "unrelated");
        let regions = walked_regions(&text);
        assert_eq!(regions.len(), 1, "expected exactly one region: {regions:?}");
        assert_eq!(regions[0].0, "the-opening");
        assert_eq!(regions[0].1, "2026-08-22");
        assert_eq!(regions[0].2, "One edge per member.");
    }

    /// Editing prose **outside** a region must not read as a walked change, or every
    /// ordinary tour edit would fail the check and the mechanism would be abandoned.
    #[test]
    fn editing_prose_outside_the_region_is_not_a_walked_change() {
        let before = tour(
            "2026-08-22",
            "One edge per member.",
            "the voltages are equal",
        );
        let after = tour(
            "2026-08-22",
            "One edge per member.",
            "the potentials are equal",
        );
        assert_ne!(before, after, "the fixture must actually differ");
        assert_eq!(
            walked_regions(&before),
            walked_regions(&after),
            "an edit outside the marked passage was reported as a walked change",
        );
    }

    /// The must-fire case: the regression that started all of this.
    #[test]
    fn rewriting_walked_prose_at_the_same_date_is_detectable() {
        let before = tour("2026-08-22", "One edge per member.", "same");
        let after = tour("2026-08-22", "One edge in a graph.", "same");
        let (b, a) = (walked_regions(&before), walked_regions(&after));
        assert_eq!(b[0].1, a[0].1, "the fixture keeps the date fixed");
        assert_ne!(
            b[0].2, a[0].2,
            "a rewritten walked passage was reported as unchanged: this is exactly the \
             2026-08-13 loss, and the check would be vacuous",
        );
    }

    /// Bumping the date must be visible **independently** of the body, since that is
    /// what separates an acknowledged rewrite from a silent one.
    #[test]
    fn bumping_the_date_is_visible_apart_from_the_body() {
        let before = tour("2026-08-12", "Original wording.", "same");
        let after = tour("2026-08-22", "Original wording.", "same");
        let (b, a) = (walked_regions(&before), walked_regions(&after));
        assert_eq!(b[0].0, a[0].0, "the slug is the stable identity");
        assert_ne!(b[0].1, a[0].1, "the date must be readable on its own");
        assert_eq!(b[0].2, a[0].2, "the body is untouched here");
    }

    /// **The same-day case, and the first live use found it within the hour.**
    ///
    /// Doug marked a region on 2026-08-22 and improved it on 2026-08-22. A bare date
    /// cannot acknowledge that edit — the token is unchanged, so the checker fires and
    /// there is no honest way to satisfy it. **A phase-2 walk produces several
    /// acknowledged edits in one day, so this is the common case, not the corner one**
    /// (Doug: *"today's same-date scenario would be common during phase 2 walks. You
    /// might need to add a time to the marker."*).
    ///
    /// **No parser change was needed** — the field is the second whitespace-delimited
    /// token, so `2026-08-22T17:45` already works. This test is what stops a later
    /// "tidy-up" from parsing the field as a bare date and silently removing the only
    /// way to acknowledge a second edit on one day.
    #[test]
    fn a_timestamp_distinguishes_two_acknowledgements_on_one_day() {
        let before = tour("2026-08-22", "First wording.", "same");
        let after = tour("2026-08-22T17:45", "Second wording.", "same");
        let (b, a) = (walked_regions(&before), walked_regions(&after));
        assert_eq!(
            b[0].0, a[0].0,
            "the slug is unchanged, so it is the same region"
        );
        assert_ne!(
            b[0].1, a[0].1,
            "a timestamped token must differ from the bare date of the same day, or a \
             second edit during one walk cannot be acknowledged at all",
        );
        assert_ne!(b[0].2, a[0].2, "the fixture must actually rewrite the body");
    }

    /// Deleting the passage must remove the slug, which is what lets the checker say
    /// **DELETED** rather than shrug.
    #[test]
    fn deleting_a_walked_region_removes_its_slug() {
        let before = tour("2026-08-22", "One edge per member.", "same");
        let after = "# A tour\n\nsame\n\nLater prose: same.\n";
        assert!(
            !walked_regions(&before).is_empty() && walked_regions(after).is_empty(),
            "a deleted walked region must disappear from the scan",
        );
    }

    /// An unterminated marker is a finding, not a region — the [`super::tests::marked_rows`]
    /// lesson applied to prose.
    #[test]
    fn an_unterminated_marker_is_reported_and_yields_no_region() {
        let text = "<!-- walked: the-opening 2026-08-22 -->\n\nProse with no closing marker.\n";
        assert_eq!(
            unterminated_walked(text),
            vec!["the-opening".to_owned()],
            "an unterminated marker must be reported",
        );
        assert!(
            walked_regions(text).is_empty(),
            "an unterminated marker must not produce a region that silently covers the \
             rest of the document",
        );
    }

    /// Two regions in one document stay apart, and a second marker opening before the
    /// first closes is reported rather than silently absorbed.
    #[test]
    fn a_second_marker_before_the_close_is_reported() {
        let text = "<!-- walked: first 2026-08-22 -->\nA.\n\
                    <!-- walked: second 2026-08-22 -->\nB.\n<!-- /walked -->\n";
        assert_eq!(
            unterminated_walked(text),
            vec!["first".to_owned()],
            "the unclosed first marker must be reported",
        );
        let regions = walked_regions(text);
        assert_eq!(
            regions.len(),
            1,
            "only the closed region parses: {regions:?}"
        );
        assert_eq!(regions[0].0, "second");
        assert_eq!(regions[0].2, "B.");
    }

    /// A well-formed document reports nothing — the other end of must-fire, so a green
    /// result means "checked and clean" rather than "the parser reports everything".
    #[test]
    fn a_well_formed_document_reports_no_finding() {
        let text = tour("2026-08-22", "One edge per member.", "same");
        assert!(
            unterminated_walked(&text).is_empty(),
            "a closed marker must not be reported as unterminated",
        );
    }
}
