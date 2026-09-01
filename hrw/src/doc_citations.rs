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

    /// A named reading-path budget, from `docs/reading-budgets.txt`.
    ///
    /// # Why the numbers live in `docs/` and not in this file
    ///
    /// Doug, 2026-08-31: *"It seems that we trigger a lot of full test runs because of
    /// that one file."* He was right and it was measurable. `gate_policy` sends any diff
    /// touching `src/` to the FULL gate, so while these budgets were `const`s **here**, a
    /// pure prose commit that pushed a document over its budget became a `src/` change and
    /// paid ~170 s instead of ~6 s. Two of the twenty commits pushed that day were exactly
    /// that — a lab rewrite and a handoff note — costing him about three minutes each,
    /// waiting on a suite that could not observe the change.
    ///
    /// **A budget is data about documents.** It moves when a document moves, so it belongs
    /// on the docs side of the FAST/FULL line by nature, not merely for convenience. This
    /// does not *exempt* anything: a prose commit that raises a budget is still correctly
    /// classified as a prose commit, and editing a **checker** in this file still means
    /// FULL — which it must, since four tests here are slow-gated.
    ///
    /// # A missing name is a failure, never a default
    ///
    /// An absent budget must not read as "no limit". That is the claims-of-absence rule in
    /// its most literal form: a check that quietly stops checking is worse than one that
    /// was never written, because it still looks present.
    fn budget(name: &str) -> usize {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/reading-budgets.txt");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                panic!("{}: not `name = number`: {line}", path.display());
            };
            if key.trim() == name {
                return value.trim().parse().unwrap_or_else(|e| {
                    panic!("{}: `{name}` is not a number ({e}): {line}", path.display())
                });
            }
        }
        panic!(
            "{} defines no budget named `{name}`. An absent budget is a failure, not an \
             unlimited one \u{2014} add it with the reasoning for its value.",
            path.display(),
        );
    }

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

    /// **Every relative markdown link in a governing document resolves.**
    ///
    /// The sibling above checks *backtick citations* — `` `src/app.rs` `` — and nothing
    /// checked `[text](path)`. That gap let the tour → lab rename move
    /// `docs/fixture-tours/` and leave **five broken links in `CLAUDE.md`** while the FULL
    /// gate reported 915 tests green. It was found by grep, days later, and only because
    /// someone happened to look.
    ///
    /// **`fixture_lab_links_all_resolve` covers the labs; the governing documents had no
    /// equivalent** — which is exactly the asymmetry that makes a green run misleading:
    /// the confidence a gate produces does not know what it failed to measure.
    ///
    /// **Skipped deliberately**, each because it is not a claim about a file in this repo:
    /// `hrw://` links (the app resolves those, and `no_lab_links_to_a_bare_file_path`
    /// governs their form), absolute URLs, bare `#anchor` fragments, and any target
    /// containing a `<` placeholder such as `hrw://lab/<name>` — the metavariable trap that
    /// has bitten this repository three times.
    ///
    /// **A link inside a code span is an EXAMPLE, not a link**, and is skipped. `DECISIONS.md`
    /// quotes `` `[the-mathematics.md](the-mathematics.md)` `` while explaining why that form
    /// was wrong; the file has since been renamed, and "fixing" the example would destroy the
    /// account. Same family as the metavariable trap — text that *looks* like a live reference
    /// because it is demonstrating one.
    ///
    /// **Non-vacuity is asserted**, because a link extractor that silently stops matching
    /// would otherwise pass forever.
    #[test]
    fn every_markdown_link_in_a_governing_document_resolves() {
        let mut broken: Vec<String> = Vec::new();
        let mut checked = 0usize;

        for doc in doc_files() {
            let text = std::fs::read_to_string(&doc).unwrap_or_default();
            let dir = doc.parent().unwrap_or(Path::new("."));
            for line in text.lines() {
                // A line whose links are all inside code spans is demonstrating markdown,
                // not using it. Cheap test: strip `...` spans before extracting.
                let mut outside = String::with_capacity(line.len());
                let mut in_span = false;
                for ch in line.chars() {
                    if ch == '`' {
                        in_span = !in_span;
                    } else if !in_span {
                        outside.push(ch);
                    }
                }
                for target in markdown_link_targets(&outside) {
                    // Not a claim about a file in this repository.
                    if target.starts_with("hrw://")
                        || target.starts_with("http://")
                        || target.starts_with("https://")
                        || target.starts_with("mailto:")
                        || target.starts_with('#')
                        || target.contains('<')
                        || target.is_empty()
                    {
                        continue;
                    }
                    // A trailing `#anchor` names a heading, not a file.
                    let path_part = target.split('#').next().unwrap_or(target);
                    if path_part.is_empty() {
                        continue;
                    }
                    checked += 1;
                    if !dir.join(path_part).exists() {
                        broken.push(format!(
                            "{}: [..]({target})",
                            doc.file_name().unwrap_or_default().to_string_lossy()
                        ));
                    }
                }
            }
        }

        assert!(
            checked > 50,
            "expected the governing documents to link freely; found only {checked} \
             relative links, which means the extractor stopped matching rather than the \
             documents stopped linking",
        );
        assert!(
            broken.is_empty(),
            "markdown links point at files that do not exist \u{2014} a rename moved the \
             target and nothing said so:\n  {}",
            broken.join("\n  "),
        );
    }

    /// **No lab uses retired vocabulary.**
    ///
    /// Charter Decision 15 replaced `tour` with `lab` and, in a second pass, `walk` with
    /// `run`/`session`. **There is no alias** — Doug's ruling — so a reintroduction is a
    /// defect, not a variant spelling.
    ///
    /// **Both renames were caught by Doug rather than by a test.** The first missed the
    /// verb entirely; the second left `Walk [matching]` in seven labs and `**Stops:**` in
    /// the catalogue *generator*, because the substitution regexes were case-sensitive.
    /// That is the backward tech-debt trigger — *who caught it?* — answered "a human",
    /// which means the code lived somewhere nothing checks.
    ///
    /// # Why the labs, and only the labs
    ///
    /// **The answer here is crisply zero**, so the check needs no allow-list — and an
    /// allow-list is the exhaustive-list shape that has bitten this repository three
    /// times. The governing documents legitimately hold ~297 retired words: Doug's
    /// quotations, Decision 14's own title, `last_walked`'s real name, the traversal
    /// senses, and `DECISIONS.md`'s 191 historical entries, which are history and do not
    /// bind. Capping those would need per-file numbers that drift.
    ///
    /// **`README.md` is excluded** for the same reason: it is the rules file, and it
    /// documents the collision analysis by naming the retired words.
    #[test]
    fn no_lab_uses_retired_vocabulary() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let mut bad: Vec<String> = Vec::new();
        let mut checked = 0usize;

        let Ok(entries) = std::fs::read_dir(&dir) else {
            panic!("{} is not readable", dir.display());
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            if name == "README.md" {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            checked += 1;
            for (i, line) in text.lines().enumerate() {
                let lower = line.to_lowercase();
                for word in ["tour", "tours", "walk", "walks", "walked", "walking"] {
                    // Word-bounded, so `walk_modules` and `walkable` are untouched.
                    let bounded = lower.split(|c: char| !c.is_alphanumeric() && c != '_');
                    if bounded.into_iter().any(|w| w == word) {
                        bad.push(format!("{name}:{}  says {word:?}", i + 1));
                    }
                }
            }
        }

        assert!(
            checked >= 20,
            "only {checked} labs were read from {} — the scan is broken, which looks \
             like success",
            dir.display(),
        );
        assert!(
            bad.is_empty(),
            "labs use vocabulary charter Decision 15 retired — a lab is a LAB, its unit \
             a STATION, and Doug RUNS one in a SESSION:\n  {}",
            bad.join("\n  "),
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
    /// produce tags nobody had checked, which is the lab-prose mistake again. Low
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
            "only scanned {scanned} documents — the run is broken"
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

    /// Characters from writing systems this repository has no use for.
    ///
    /// **Deliberately narrow.** Em dashes, `§`, `✅`, `⟶`, `λ` and `µ` are all in
    /// legitimate use here, so a blanket non-ASCII rule would be pure noise. These
    /// four ranges are different: nothing in an English repository about a Modelica
    /// compiler produces them, so any occurrence is a **generation slip**.
    fn foreign_script_chars(text: &str) -> Vec<(usize, char)> {
        let mut out = Vec::new();
        for (i, line) in text.lines().enumerate() {
            for c in line.chars() {
                let o = c as u32;
                let foreign = (0x2E80..=0xA4CF).contains(&o)      // CJK
                    || (0xAC00..=0xD7AF).contains(&o)             // Hangul
                    || (0x0400..=0x04FF).contains(&o)             // Cyrillic
                    || (0x0600..=0x06FF).contains(&o); // Arabic
                if foreign {
                    out.push((i + 1, c));
                }
            }
        }
        out
    }

    /// **No document or comment carries a character from a foreign script.**
    ///
    /// # The failure this catches, which happened while writing the file below it
    ///
    /// On 2026-08-22 Claude wrote `docs/unattended-runs.md` with two CJK characters
    /// spliced into the middle of the word *"verifies"* — the sentence still read as
    /// English either side of them. It was caught by re-reading, which is exactly the
    /// check that is **absent overnight**. (The characters are not reproduced here;
    /// this test would flag them, which is the point.)
    ///
    /// **`documents_contain_no_stray_control_characters` does not cover it**: these are
    /// printable, well-formed UTF-8, and survive every existing check. They are also
    /// invisible in a diff unless you are looking, and they corrupt *meaning* rather
    /// than encoding — the word simply becomes a different word.
    ///
    /// # Why it starts green with no exemption list
    ///
    /// Measured over every `.md` and `.rs` in `hrw/` before the check was written:
    /// **zero occurrences.** A checker that needs an exemption list on day one is a
    /// checker someone will switch off; this one does not.
    #[test]
    fn no_document_carries_a_foreign_script_character() {
        let mut bad: Vec<String> = Vec::new();
        let mut scanned = 0usize;

        for path in doc_files() {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            scanned += 1;
            for (line, c) in foreign_script_chars(&text) {
                bad.push(format!("{}:{line}: U+{:04X} {c}", path.display(), c as u32));
            }
        }
        for (path, text) in rust_source_texts() {
            scanned += 1;
            for (line, c) in foreign_script_chars(text) {
                bad.push(format!("{}:{line}: U+{:04X} {c}", path.display(), c as u32));
            }
        }

        assert!(
            scanned > 100,
            "only {scanned} files scanned — the run is broken"
        );
        assert!(
            bad.is_empty(),
            "a character from a foreign script reached the tree \u{2014} this is a \
             generation slip, not something anyone typed, and it changes the word it \
             lands in:\n  {}",
            bad.join("\n  "),
        );
    }

    /// The must-fire half: the detector reports, and does not report what is fine.
    #[test]
    fn a_foreign_script_character_is_reported() {
        let hits = foreign_script_chars("fine line\nno test here\u{9A8C}\u{8BC1}s for meaning\n");
        assert_eq!(hits.len(), 2, "both CJK characters on line 2: {hits:?}");
        assert_eq!(hits[0].0, 2, "and the line number is reported");
        assert!(
            foreign_script_chars(
                "em dash \u{2014}, section \u{A7}, tick \u{2705}, lambda \u{3BB}, micro \u{B5}\n"
            )
            .is_empty(),
            "the punctuation and symbols this repository actually uses must not report",
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
            "only found {} sources — the run is broken",
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
        /// after `ModelListState`; 75 after `Viewport`; 72 after `LabState`; 57
        /// after `SourceViewState` and `ContextBarState`.
        ///
        /// **Raised to 58 on 2026-08-02 for `SplitState`** (`ideas.md` #59). The ratchet
        /// fired and the question it asks was answered honestly: the LHS/RHS split is
        /// **window layout**, used by both the lab and specimen panels and owned by
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
    /// `app::tests::lab_catalogue_is_current` diffs `CATALOGUE.md` against
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
        for rel in ["src/app.rs", "docs/fixture-labs/CATALOGUE.md"] {
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
                .any(|l| forms.iter().any(|f| contains_whole_ident(l, f.as_str())))
        }) || is_enum_variant(symbol)
    }

    /// `line` contains `needle`, and what follows cannot continue a Rust
    /// identifier.
    ///
    /// **Found 2026-08-23 by a false positive that had been reachable all along.**
    /// Adding a module named `pantelides_ladder` made
    /// `<!-- unbuilt: rumoca_phase_structural::pantelides -->` resolve, because
    /// the form `mod pantelides` is a plain substring of `pub mod
    /// pantelides_ladder;`. The claim of absence was retired by a module in a
    /// different crate that merely *starts with* the same letters.
    ///
    /// That is the expensive direction, and [`symbol_is_defined`]'s own doc
    /// comment already said so: a false positive fails the build, and worse, this
    /// particular one **silently un-marks a claim of absence** — the wrong
    /// negative `CLAUDE.md` calls the error nobody catches, because acting on it
    /// means not looking. `identity-and-provenance.md`'s rule is the general form:
    /// no substring search ever decides identity.
    ///
    /// Only the trailing side needs guarding. Every keyword form begins with
    /// `fn `/`mod `/`struct `… so the leading boundary is already a space, and the
    /// struct-field form ends in `:`, which no identifier can contain.
    fn contains_whole_ident(line: &str, needle: &str) -> bool {
        let mut from = 0;
        while let Some(i) = line[from..].find(needle) {
            let end = from + i + needle.len();
            let next = line[end..].chars().next();
            if !next.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                return true;
            }
            from = end;
        }
        false
    }

    /// **A longer identifier must not satisfy a shorter claim.**
    ///
    /// The regression guard for the false positive above, and it fails by name
    /// against the real case rather than a synthetic one: `pantelides_ladder` is
    /// a module in this crate, and `rumoca_phase_structural::pantelides` is a
    /// module that does not exist in any crate.
    ///
    /// The second half matters as much as the first — a boundary check that also
    /// rejected the *genuine* definition would turn every true positive into a
    /// stale claim nobody could retire.
    #[test]
    fn a_longer_identifier_does_not_satisfy_a_shorter_claim() {
        // **Assembled, never spelled.** This file is itself scanned, and a string
        // literal is not a comment — so writing the definition form here would
        // define the very symbol the last assertion proves absent. Fourth
        // instance in this repository of a source check matching its own text,
        // and the first where the trap was inside a test's fixtures.
        let form = format!("{} pantelides", "mod");
        let real = format!("pub {form};");
        let prefixed = format!("pub {form}_ladder;");

        assert!(
            !contains_whole_ident(&prefixed, &form),
            "a module whose name merely starts with the claimed one must not count"
        );
        assert!(
            contains_whole_ident(&real, &form),
            "the genuine definition must still count, or no claim could ever be retired"
        );
        assert!(contains_whole_ident(&format!("{form} {{"), &form));
        assert!(!contains_whole_ident(
            &format!("{} pantelides2()", "fn"),
            &format!("{} pantelides", "fn")
        ));
        // The name occurring twice, the first as a prefix: the scan must keep
        // looking rather than answering from the first hit.
        assert!(contains_whole_ident(&format!("{prefixed} {real}"), &form));

        // End to end, through the resolver the absence tags actually use.
        assert!(
            !symbol_is_defined("rumoca_phase_structural::pantelides"),
            "general Pantelides is not implemented (docs/ideas.md #83); if this now \
             resolves, either it landed or the resolver has gone permissive again"
        );
    }

    /// `A::B` where `B` is a variant of `enum A` — the one definition shape the
    /// `forms` list above cannot express.
    ///
    /// # Why this is worth the parsing
    ///
    /// A variant is declared as bare `Stdout,` or `CompileProgress { .. }` inside an
    /// enum body, matching none of `fn`/`struct`/`enum`/`const`/… — so before this,
    /// **every enum variant read as undefined.** Adding a loose `"{leaf},"` form
    /// instead would have matched the name followed by a comma *anywhere*, including
    /// an argument list, making the resolver over-permissive; and over-permissive is
    /// the dangerous direction for [`claims_of_absence_are_still_true`], whose whole
    /// job is proving a symbol **absent**.
    ///
    /// So it uses the qualifier the citation already carries, which the leaf-only path
    /// throws away: find `enum A`, run to its matching brace, and look for the variant
    /// at the start of a line inside. Precise in both directions.
    ///
    /// Found by extending the citation checker to `architecture.md`, where **five of
    /// six failures were enum variants or external crates** rather than real drift.
    fn is_enum_variant(symbol: &str) -> bool {
        let mut segs: Vec<&str> = symbol.split("::").map(str::trim).collect();
        let (Some(variant), Some(enum_name)) = (segs.pop(), segs.pop()) else {
            return false;
        };
        if variant.is_empty() || enum_name.is_empty() {
            return false;
        }
        let header = format!("enum {enum_name}");
        rust_source_texts().iter().any(|(_, text)| {
            let Some(at) = text.find(&header) else {
                return false;
            };
            let rest = &text[at..];
            let Some(open) = rest.find('{') else {
                return false;
            };
            // Walk to the brace that closes the enum body.
            let mut depth = 0usize;
            let mut end = None;
            for (i, c) in rest[open..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(open + i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(end) = end else {
                return false;
            };
            rest[open + 1..end].lines().any(|l| {
                // A variant starts a line; what follows it says which kind it is —
                // `Stdout,` unit, `Compiled {` struct-like, `Frame(` tuple-like.
                matches!(l.trim().strip_prefix(variant),
                    Some(after) if after.is_empty()
                        || after.starts_with([',', '(', '{', ' ', '=']))
            })
        })
    }

    /// Every Rust source in the workspace, run and read **once**.
    ///
    /// **Memoised because [`symbol_is_defined`] is called once per `unbuilt:` tag**,
    /// and each call used to re-run `crates/` — 56 crates — and re-read every file
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

    /// Does an `hrw://` link form appear in a fixture lab?
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
    /// The fixture labs are the right corpus: a link form nothing exercises is
    /// not really built.
    fn link_form_is_exercised(pattern: &str) -> bool {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
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
                // Subsequence match: run `got`, consuming each named segment of
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
            "frame addressing IS exercised by a fixture lab, so this claim would be stale",
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
    /// **The marker names the specimen**, because one lab describes several panes of
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
    /// So the run now stops at the first line that is neither blank nor part of a
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

    /// Every guarded region of a lab document, as `(marker, rows)` pairs.
    ///
    /// **This is the fingerprint that decides FAST versus FULL**, and it exists because
    /// that decision was previously made by remembering. See
    /// [`editing_a_guarded_lab_table_needs_the_full_gate`].
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

    /// **Every message a pane shows when it has nothing to show is named by a test.**
    ///
    /// The charter's first rule in its cheapest form: *absence is stated, never filled*.
    /// A pane with nothing to show must say the compiler produced nothing — and **a
    /// wrong absence message is invisible to everything else here**, because *"no X in
    /// this report"* is well-formed whether or not it is true. That is how the
    /// 2026-08-19 alias defect survived until Doug hit it: the pane said a model with
    /// several eliminations had none.
    ///
    /// Built 2026-08-24 from a one-night survey, which is the move this repository keeps
    /// making — an audit becomes a mechanism, or it is a number that was true once.
    ///
    /// # What counts as "rendered"
    ///
    /// A string literal opening with *no* / *nothing* / *none*, on or just below a line
    /// that hands it to a widget. Literals in test modules are excluded, so a message
    /// that exists only in a test's expectation is not mistaken for one a pane shows.
    ///
    /// # Why a ratchet, and what the number MEANS
    ///
    /// It is not an arbitrary budget. At the count below, the uncovered set is exactly
    /// the messages this repository has decided not to test, each with a reason on
    /// record in `docs/ui-findings.md`:
    ///
    /// - **Three are unreachable (C17)** — `connection_anim`, `reduction_anim` and
    ///   `pre_lowering_anim` each guard construction on a non-empty frame vector, so
    ///   their internal absence branch never runs and `app.rs` speaks instead. A test on
    ///   those strings would pass forever regardless of what the pane does, which is
    ///   C1's reasoning for accepting rather than testing.
    /// - **Two are per-frame running states**, not empty-pane messages: *"No slots
    ///   created yet"* and *"No iteration needed yet"* describe a cursor position inside
    ///   a populated replay. Worth covering, not yet covered.
    ///
    /// **Lower it as those are covered; never raise it.** A new absence message with no
    /// test raises the count and fails here, which is the whole point.
    #[test]
    fn every_absence_message_a_pane_shows_is_named_by_a_test() {
        /// 6 on 2026-08-24, the day this was built: three unreachable (C17) and two
        /// per-frame states, plus one probe artifact that the punctuation trim below
        /// removed. Anything above this is a pane that can say something false with
        /// nothing to catch it.
        const UNCOVERED_BUDGET: usize = 5;

        // Assembled, so this file's own prose cannot satisfy the scan.
        let openers = [
            format!("\"{}", "No "),
            format!("\"{}", "Nothing "),
            format!("\"({}", "no "),
            format!("\"{}", "no "),
        ];
        let widgets = ["ui.label", "ui.weak", "ui.small", ".weak(", ".label("];

        let mut rendered: std::collections::BTreeMap<String, String> =
            std::collections::BTreeMap::new();
        let mut test_text = String::new();

        for (path, text) in rust_source_texts() {
            let rel = path.to_string_lossy().replace('\\', "/");
            let Some(idx) = rel.find("hrw/src/") else {
                continue;
            };
            let rel = rel[idx + "hrw/src/".len()..].to_owned();

            let lines: Vec<&str> = text.lines().collect();
            let mut in_tests = false;
            for (i, line) in lines.iter().enumerate() {
                if line.trim() == "#[cfg(test)]" {
                    in_tests = true;
                }
                if in_tests {
                    test_text.push_str(line);
                    test_text.push('\n');
                    continue;
                }
                let Some(at) = openers.iter().find_map(|o| line.find(o.as_str())) else {
                    continue;
                };
                // Rendered? This line or the two above must hand it to a widget.
                let window = lines[i.saturating_sub(2)..=i].join("\n");
                if !widgets.iter().any(|w| window.contains(w)) {
                    continue;
                }
                let rest = &line[at + 1..];
                let Some(end) = rest.find('"') else { continue };
                let msg = rest[..end].to_owned();
                if msg.len() >= 6 {
                    rendered.entry(msg).or_insert(rel.clone());
                }
            }
        }

        assert!(
            rendered.len() >= 12,
            "only {} absence messages were found, so the scan has stopped reading the \
             views",
            rendered.len(),
        );

        // **Trailing punctuation is trimmed from the probe**, and that is a correction:
        // the first run reported `lab_panel`'s message uncovered because its test
        // asserts `"No lab right now"` while the source says `"No lab right now."` —
        // the finding was the probe, not the pane.
        let uncovered: Vec<(&String, &String)> = rendered
            .iter()
            .filter(|(msg, _)| {
                let probe: String = msg
                    .chars()
                    .take(30)
                    .collect::<String>()
                    .trim_end_matches(['.', ',', ' ', '\\'])
                    .to_owned();
                !probe.is_empty() && !test_text.contains(probe.as_str())
            })
            .collect();

        assert!(
            uncovered.len() <= UNCOVERED_BUDGET,
            "{} absence messages are named by no test, budget {UNCOVERED_BUDGET}:\n  {}\n\n\
             A pane that says the wrong thing when it has nothing to show is well-formed \
             and invisible to every other check here. Cover it, or record why it cannot \
             be covered in docs/ui-findings.md the way C17 does.",
            uncovered.len(),
            uncovered
                .iter()
                .map(|(m, f)| format!("[{f}] {m}"))
                .collect::<Vec<_>>()
                .join("\n  "),
        );
    }

    /// **"Not reached" is worded in one place, because a checker depends on the words.**
    ///
    /// `fidelity::tests::a_stage_that_says_it_never_ran_shows_no_ir` finds a stage that
    /// never ran by matching the substrings **`"not reached"`** and **`"produced no
    /// result"`**. Its coverage is therefore only as wide as the agreement between every
    /// site that writes those notes — and on 2026-08-24 there were **five**, three of them
    /// rebuilding the same format string by hand.
    ///
    /// They agreed by coincidence. Reword the helper and the copies keep the old text;
    /// reword a copy and the checker silently stops seeing that stage. **A guard whose
    /// premise is maintained by hand in five places is a guard with a slow leak**, and the
    /// leak is invisible: the checker still passes, over less.
    ///
    /// This closes it from the other side. `not_reached_note` and `no_result_note` own the
    /// wording; anywhere else spelling it is a second source, and the next reword splits
    /// them apart again.
    ///
    /// # Scope
    ///
    /// Prose is exempt — a comment *describing* the note is not a second source of it, and
    /// `worker.rs` carries one such comment deliberately. Only code that builds the string
    /// counts, which is why the scan skips `//` lines.
    #[test]
    fn the_not_reached_note_has_exactly_one_author() {
        let worker =
            std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs"))
                .expect("worker.rs must be readable");

        // Assembled, so this file's own prose is not a second source either.
        let needles = [
            format!("\"{} (", "not reached"),
            format!("\"{}", "the reachable-closure pipeline produced no result"),
        ];

        let authors: Vec<String> = worker
            .lines()
            .enumerate()
            .filter(|(_, line)| !line.trim_start().starts_with("//"))
            .filter(|(_, line)| needles.iter().any(|n| line.contains(n.as_str())))
            .map(|(i, line)| {
                format!(
                    "worker.rs:{}  {}",
                    i + 1,
                    line.trim_start().chars().take(72).collect::<String>()
                )
            })
            .collect();

        assert_eq!(
            authors.len(),
            2,
            "the not-run wording is written in {} place(s), not the two helpers that own \
             it:\n  {}\n\n`a_stage_that_says_it_never_ran_shows_no_ir` matches on these \
             words. A second author does not break it — it narrows it, silently, the next \
             time the two are reworded apart. Route the site through `not_reached_note` \
             or `no_result_note`.",
            authors.len(),
            authors.join("\n  "),
        );
    }

    /// **The stranded-sub-view clamp runs, and runs last.**
    ///
    /// From the 2026-08-23 column read of the `has_*` availability family, whose
    /// hypothesis — that a caller might use the alias predicate without the stage test
    /// it depends on — turned out to be **fully guarded**. `clamp_structural_sub_view`
    /// was added on 2026-08-19 after exactly that defect: the Aliases tab shipped
    /// without updating the stage-change default, so a selection stranded on
    /// `AliasAnim` read the Structural report, found no eliminations, and said *"(no
    /// alias eliminations in this report)"* about a model that has several. **Absence
    /// filled rather than stated.**
    ///
    /// # What was missing, and it is the must-fire gap
    ///
    /// Three tests exercise the clamp, and **all three call it directly**. Delete its
    /// one production call site and every one of them still passes — the guard would be
    /// dead, the stranded-view class silently reopened, and nothing would say so. A
    /// checker whose subject is never invoked is indistinguishable from no checker,
    /// which is the shape this repository treats as the error nobody catches.
    ///
    /// # Why the ORDER is asserted and not just the presence
    ///
    /// Its own comment states the requirement: *"Last, so it sees every door."* Three
    /// things set the structural sub-view — the report row, the stage-change default
    /// inside it, and the `hrw://` link guard in `apply_pending_view_and_seek` — and
    /// the clamp checks the result of all three rather than trusting each. Moved above
    /// the link guard it would validate a selection the guard then replaces, which
    /// restores the original defect while still looking present.
    #[test]
    fn the_stranded_sub_view_clamp_runs_last_in_the_paint_path() {
        let app = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"))
            .expect("app.rs must be readable");

        let body = app
            .split_once("fn central_panel_ui(")
            .expect("central_panel_ui must be present")
            .1
            .split_once("\n    }\n")
            .expect("its body is closed by a `}` at four spaces")
            .0;

        // Assembled so a rename cannot be satisfied by this file's own prose.
        let clamp = format!("self.{}()", "clamp_structural_sub_view");
        let link_guard = format!("self.{}()", "apply_pending_view_and_seek");

        let at_clamp = body.find(clamp.as_str()).unwrap_or_else(|| {
            panic!(
                "central_panel_ui never calls the stranded-sub-view clamp. Its three \
                 tests call it directly, so they would all still pass with the guard \
                 dead \u{2014} which is how the 2026-08-19 defect would come back \
                 unannounced."
            )
        });
        let at_guard = body
            .find(link_guard.as_str())
            .unwrap_or_else(|| panic!("central_panel_ui never applies a pending sub-view request"));

        assert!(
            at_clamp > at_guard,
            "the clamp runs BEFORE the link guard, so it validates a selection the \
             guard then replaces. Its own comment is `Last, so it sees every door` \
             \u{2014} there are three doors and this must see the result of all of them.",
        );
    }

    /// **A link that changes the stage must also leave the log.**
    ///
    /// From the 2026-08-23 column read of `dispatch_hrw_link`'s twelve arms. Five of
    /// them navigate to a stage — `SwitchStage`, `LoadAndSwitch`, `AimAtEquation`,
    /// `PointAtNode`, `SeekFrame` — and all five clear `viewing_log`. **The read found
    /// them uniform; this keeps them so.**
    ///
    /// # What goes wrong without it
    ///
    /// `viewing_log` is a full-pane override: while it is set, the centre shows the
    /// compilation log instead of the stage. An arm that changed `self.stage` and left
    /// it set would move the reader's stage **behind** the log, so the click produces
    /// no visible change — *"a link that does nothing is the worst outcome in a lab,
    /// because nothing on screen says why"*, which this router's own `OpenLab` arm
    /// says about a different silence.
    ///
    /// It is the exact failure `apply_pending_view_and_seek` was written for in
    /// another guise: a link that appears to work because the common case hides it.
    ///
    /// # Scope
    ///
    /// Source-level, and it asks only about arms of **this** router. An arm that
    /// navigates by calling a helper which sets the stage internally would not be
    /// seen — `LoadSpecimen` is that shape, reaching the stage through `open`, and it
    /// is correctly not counted here because it does not set `self.stage` itself.
    #[test]
    fn every_link_arm_that_changes_the_stage_leaves_the_log() {
        let app = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs"))
            .expect("app.rs must be readable");

        // Assembled, so this file does not contain the strings it searches for.
        let sets_stage = format!("self.{} = ", "stage");
        let leaves_log = format!("self.{} = false", "viewing_log");

        let body = app
            .split_once("fn dispatch_hrw_link(&mut self, action: HrwLink) {")
            .expect("dispatch_hrw_link must be present")
            .1
            .split_once("\n    }\n")
            .expect("its body is closed by a `}` at four spaces")
            .0;

        // Arms open at twelve spaces inside the `match`.
        let opener = format!("            {}::", "HrwLink");
        let mut arms: Vec<(String, String)> = Vec::new();
        let mut current: Option<(String, String)> = None;
        for line in body.lines() {
            if let Some(rest) = line.strip_prefix(opener.as_str()) {
                if let Some(done) = current.take() {
                    arms.push(done);
                }
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                current = Some((name, String::new()));
            } else if let Some((_, text)) = current.as_mut() {
                text.push_str(line);
                text.push('\n');
            }
        }
        arms.extend(current);

        assert!(
            arms.len() >= 12,
            "only {} arms parsed out of this router; the scan has stopped reading it: {:?}",
            arms.len(),
            arms.iter().map(|(n, _)| n).collect::<Vec<_>>(),
        );

        let navigating: Vec<&(String, String)> = arms
            .iter()
            .filter(|(_, text)| text.contains(sets_stage.as_str()))
            .collect();
        assert!(
            navigating.len() >= 4,
            "only {} arms were found to change the stage, so this check is reading \
             almost nothing",
            navigating.len(),
        );

        let offenders: Vec<&String> = navigating
            .iter()
            .filter(|(_, text)| !text.contains(leaves_log.as_str()))
            .map(|(name, _)| name)
            .collect();
        assert!(
            offenders.is_empty(),
            "these link arms change the stage without leaving the log: {offenders:?}\n\n\
             While `viewing_log` is set the centre pane shows the log instead of the \
             stage, so the reader's stage moves behind it and the click looks like it \
             did nothing.",
        );
    }

    /// **An `ALL` roster must list every variant of its enum.**
    ///
    /// From the 2026-08-23 column read of the tab rosters. No roster was wrong — and
    /// the finding is the sentence above them. `stage_view.rs` tells the next author:
    /// *"**Add new variants here** — that is what makes the omission loud instead of
    /// silent."*
    ///
    /// **It was not loud.** Nothing compared a roster against its enum, so adding a
    /// variant and forgetting the list compiled cleanly and simply went untested:
    /// `every_sub_view_slug_round_trips` iterates `ALL`, so a missing variant is a
    /// variant the round-trip never sees. A doc comment promising a guarantee the
    /// mechanism does not provide is this repository's most-repeated defect — night 1
    /// found the identical shape in a tab roster, and a test named `…shows_every_fixture…`
    /// that checked nine of twenty-two.
    ///
    /// Both sides are read from the source, so a seventh roster is covered the day it
    /// is written.
    ///
    /// # Scope
    ///
    /// It checks rosters whose name **starts with `ALL`**, which is what makes
    /// `StageKind::COMPILATION` correctly exempt: that one is a deliberate subset —
    /// every stage except `Simulation` — and asserting it exhaustive would be wrong.
    /// A future deliberate subset must therefore not be named `ALL…`.
    #[test]
    fn an_all_roster_lists_every_variant_of_its_enum() {
        let sources = rust_source_texts();

        // Every `enum Name { … }` in the crate, as name -> variants.
        let mut variants: std::collections::BTreeMap<String, BTreeSet<String>> =
            std::collections::BTreeMap::new();
        for (path, text) in sources {
            if !path
                .to_string_lossy()
                .replace('\\', "/")
                .contains("hrw/src")
            {
                continue;
            }
            let mut open: Option<(String, usize)> = None;
            for line in text.lines() {
                if let Some((name, indent)) = open.as_ref() {
                    if line.trim_end() == format!("{}}}", " ".repeat(*indent)) {
                        open = None;
                        continue;
                    }
                    let t = line.trim();
                    if t.is_empty() || t.starts_with("//") || t.starts_with('#') {
                        continue;
                    }
                    // A variant is an identifier ending in `,`, `{` or `(`.
                    let ident: String = t
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !ident.is_empty() && ident.starts_with(|c: char| c.is_ascii_uppercase()) {
                        variants
                            .entry(name.clone())
                            .or_default()
                            .insert(ident.clone());
                    }
                    continue;
                }
                if let Some(rest) = line.split_once("enum ").map(|(_, r)| r)
                    && let Some(name) = rest.split_once(' ').map(|(n, _)| n)
                    && rest.trim_end().ends_with('{')
                    && !line.trim_start().starts_with("//")
                {
                    let indent = line.len() - line.trim_start().len();
                    open = Some((name.to_owned(), indent));
                }
            }
        }
        assert!(
            variants.len() > 10,
            "the enum scan found only {} enums, so it has stopped reading the source",
            variants.len(),
        );

        // Every `const ALL…: &[…Type] = &[ … ];` roster.
        let mut checked = 0usize;
        let mut findings = Vec::new();
        for (path, text) in sources {
            for (i, line) in text.lines().enumerate() {
                let t = line.trim();
                let Some(rest) = t.split_once("const ALL").map(|(_, r)| r) else {
                    continue;
                };
                let Some((_, after)) = rest.split_once('[') else {
                    continue;
                };
                let Some((ty, _)) = after.split_once(']') else {
                    continue;
                };
                let ty = ty.trim();
                let Some(expected) = variants.get(ty) else {
                    continue; // Not an enum defined here — nothing to compare against.
                };

                // The roster body runs to the closing `];`, which may be on this
                // same line for a short one. Written as a plain loop rather than an
                // iterator chain: the terminator must be *included*, and every
                // combinator spelling of "take until and including" reads worse than
                // this does.
                let mut body = String::new();
                for l in text.lines().skip(i) {
                    body.push_str(l);
                    body.push('\n');
                    if l.trim_end().ends_with("];") {
                        break;
                    }
                }

                let marker = format!("{ty}::");
                let listed: BTreeSet<String> = body
                    .match_indices(marker.as_str())
                    .map(|(at, _)| {
                        body[at + marker.len()..]
                            .chars()
                            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                            .collect()
                    })
                    .collect();

                checked += 1;
                let missing: Vec<&String> = expected.difference(&listed).collect();
                if !missing.is_empty() {
                    findings.push(format!(
                        "{}: `{ty}`'s ALL roster omits {missing:?}. Its own comment \
                         promises that adding a variant here is what makes the omission \
                         loud \u{2014} anything iterating the roster silently skips these.",
                        path.display(),
                    ));
                }
            }
        }

        assert!(
            checked >= 6,
            "only {checked} ALL rosters were compared; there were six on 2026-08-23, so \
             the scan has stopped finding them",
        );
        assert!(findings.is_empty(), "{}", findings.join("\n\n  "));
    }

    /// **The mandatory reading path must stay small enough to read.**
    ///
    /// # The wall this exists to prevent
    ///
    /// Doug's stated worst case for this project is hitting a wall it cannot recover
    /// from. Measured on 2026-08-22, it was forming in the **documents**, not the code:
    /// `CLAUDE.md` went from **154 lines on 2026-07-26 to 2,320 on 2026-08-21**, and
    /// 45 % of it was closed history restated from files that already held it.
    ///
    /// **It had already been pruned twice — 526→317, then 2,320→1,730 — and regrew
    /// +229 lines in the single day after the second prune.** That is the whole reason
    /// this test exists rather than another cleanup: **a one-time prune is measurably
    /// insufficient**, and nothing else in this repository measures reading cost.
    ///
    /// # Why only these four files
    ///
    /// The **mandatory path** is what a session must read before its first action — not
    /// total markdown, which is ~41,000 lines. `ideas.md` (6,349) and `DECISIONS.md`
    /// (3,934) are *consulted*: they cost a grep, not context. Pruning them would not
    /// move this number, so it is not attempted and they are deliberately uncounted.
    ///
    /// # Raising a budget is allowed; raising it SILENTLY is not
    ///
    /// # Ceilings, not ratchets — retired the ratchet 2026-08-31
    ///
    /// Each budget used to be set at the **achieved** value, so any growth failed and
    /// arrived with a written justification. Doug retired it: *"we never actually
    /// predicted where that wall was or how close we are to it."*
    ///
    /// **Fifteen raises in one day, zero rejections.** A gate that never rejects is
    /// billing, not filtering — and the documents grew anyway. The *"never leave slack"*
    /// rule made it actively worse: three **downward** adjustments that afternoon each
    /// moved the next toll closer. Meanwhile the file enforcing it had reached 353 lines,
    /// longer than three of the four documents it guarded.
    ///
    /// # Characters, because lines mismeasured by 2.7x
    ///
    /// `CHARTER.md` runs 182 chars per line against `docs/README.md`'s 75, so the line
    /// budget treated 164 lines of the first as cheaper than 149 of the second.
    /// [`crate::doc_sizes`] carries the roster and the measurement.
    ///
    /// # What replaces it
    ///
    /// Wide ceilings **derived** in `docs/reading-budgets.txt` — the mandatory path capped
    /// at a quarter of a 200k context — plus nightly growth and duplication reporting by
    /// `examples/doc_report`. **Crossing a ceiling is not a licence to raise it.** It means
    /// the documents need work, and trimming an explanation is Doug's call, so the nightly
    /// report escalates rather than deciding.
    #[test]
    fn the_mandatory_reading_path_stays_small() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut rows: Vec<String> = Vec::new();
        let mut total = 0usize;
        for rel in crate::doc_sizes::MANDATORY {
            let n = crate::doc_sizes::chars_of(&hrw, rel);
            rows.push(format!("{rel}: {n}"));
            total += n;
        }

        let current_work = crate::doc_sizes::current_work_chars(&hrw);
        let ceiling = budget("current_work");
        assert!(
            current_work <= ceiling,
            "`## Current work` is {current_work} chars, ceiling {ceiling} \
             (docs/reading-budgets.txt).\n\n\
             A ceiling is not a target and must NOT be raised to fit \u{2014} that is the \
             ratchet this replaced. The section holds what is IN FLIGHT; a closed arc \
             belongs in DECISIONS.md with a link from here.",
        );

        let ceiling = budget("mandatory");
        assert!(
            total <= ceiling,
            "the mandatory reading path is {total} chars, ceiling {ceiling} \
             (docs/reading-budgets.txt):\n  {}\n\n\
             A ceiling is not a target and must NOT be raised to fit. Past it a session \
             spends more on preamble than any document is worth, and the answer is \
             restructuring \u{2014} which is Doug's call, not a number's.",
            rows.join("\n  "),
        );

        let ceiling = budget("conditional");
        for rel in crate::doc_sizes::CONDITIONAL {
            let n = crate::doc_sizes::chars_of(&hrw, rel);
            assert!(
                n <= ceiling,
                "{rel} is {n} chars, ceiling {ceiling} (docs/reading-budgets.txt).\n\n\
                 Read before one kind of work, so mandatory when it matters. Past this it \
                 is not read before a task, it is consulted \u{2014} which means it wants \
                 splitting, not a bigger number.",
            );
        }
    }

    /// Days since 1970-01-01 for a proleptic-Gregorian date (Howard Hinnant's
    /// `days_from_civil`). Dependency-free and exact across month and year ends —
    /// which a `y*372 + m*31 + d` approximation is not, and a seven-day window
    /// straddling a month boundary is precisely where that would go wrong.
    fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = (if y >= 0 { y } else { y - 399 }) / 400;
        let yoe = y - era * 400;
        let mp = (m + 9) % 12;
        let doy = (153 * mp + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 719468
    }

    /// `YYYY-MM-DD` → days since epoch, or `None` if it is not that shape.
    fn parse_ymd(s: &str) -> Option<i64> {
        let mut parts = s.trim().splitn(3, '-');
        let y: i64 = parts.next()?.trim().parse().ok()?;
        let m: i64 = parts.next()?.trim().parse().ok()?;
        let d: i64 = parts.next()?.trim().parse().ok()?;
        if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
            return None;
        }
        Some(days_from_civil(y, m, d))
    }

    /// **The "who caught it?" ledger must keep up with the work.**
    ///
    /// # What went wrong without this
    ///
    /// The ledger in `docs/tech-debt.md` was built 2026-08-16 to answer Doug's question
    /// *"are we improving?"* with a number rather than an opinion. It got **seventeen
    /// rows over two days and then nothing for a week** — through the transport-bar
    /// arc, a full day of defect-fixing and a night of unattended work, none logged.
    ///
    /// **The instrument did not fail; the habit did.** A ledger nobody appends to is
    /// indistinguishable from a stretch in which nothing was found — the wrong-negative
    /// shape this repository treats as the error nobody catches, because acting on it
    /// means *not looking*.
    ///
    /// # Why it compares against HEAD's commit date, not the wall clock
    ///
    /// A test keyed to `now` starts failing on any old checkout merely because it aged —
    /// the trap [`editing_a_guarded_lab_table_needs_the_full_gate`] documents. Keyed to
    /// HEAD both ends move together, so the result is **deterministic on any checkout**.
    ///
    /// # What it cannot claim
    ///
    /// **It checks the marker, not the rows.** Bumping `<!-- ledger-through: -->` without
    /// appending would pass — the same trust model as every other tag here. What it
    /// catches is *neglect*, which is what actually happened. **A genuinely quiet period
    /// is recorded by saying so**, not by silence.
    ///
    /// **Silent outside a git checkout**, like the other history-aware checks.
    #[test]
    fn the_who_caught_it_ledger_keeps_up_with_the_work() {
        // A week matches the tech-debt sweep cadence and survives a quiet stretch.
        const MAX_LAG_DAYS: i64 = 7;

        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(hrw.join("docs/tech-debt.md"))
            .expect("tech-debt.md holds the who-caught-it ledger");

        let marker = text
            .lines()
            .filter_map(|l| {
                l.trim()
                    .strip_prefix("<!-- ledger-through:")?
                    .strip_suffix("-->")
            })
            .map(str::trim)
            .next()
            .expect(
                "docs/tech-debt.md must carry `<!-- ledger-through: YYYY-MM-DD -->` beneath \
                 the who-caught-it ledger \u{2014} without it nothing can tell a quiet week \
                 from an unmaintained one",
            );
        let through = parse_ymd(marker)
            .unwrap_or_else(|| panic!("`ledger-through: {marker}` is not YYYY-MM-DD"));

        let Some(head) = std::process::Command::new("git")
            .args(["-C", &hrw.to_string_lossy(), "log", "-1", "--format=%cs"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| parse_ymd(String::from_utf8_lossy(&o.stdout).trim()))
        else {
            eprintln!("note: no git HEAD date \u{2014} the ledger-staleness check is inert here");
            return;
        };

        let lag = head - through;
        assert!(
            lag <= MAX_LAG_DAYS,
            "the who-caught-it ledger is {lag} days behind HEAD (through {marker}), budget \
             {MAX_LAG_DAYS}.\n\nIt answers \"are we improving?\" with a ratio, and a gap in it \
             is indistinguishable from a stretch in which nothing was found. Append the \
             defects since {marker} to `docs/tech-debt.md` \u{2014} including a row saying \
             none were found, if that is the truth \u{2014} and move the `ledger-through` \
             marker.",
        );
    }

    /// Backticked `module::item` spans in `text` — the unambiguous code citations.
    pub(super) fn qualified_code_citations(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut rest = text;
        while let Some(open) = rest.find('`') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('`') else { break };
            let span = &after[..close];
            rest = &after[close + 1..];
            // A path, not prose: segments of identifier characters joined by `::`.
            // Anything with whitespace, a slash or parentheses is a phrase or a file
            // path, and a code fence's contents fail the same way.
            if span.contains("::")
                && span.split("::").all(|s| {
                    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                })
            {
                out.push(span.to_owned());
            }
        }
        out
    }

    /// **A pointer at a checker must resolve — that is what makes "shrink to a
    /// pointer" safe.**
    ///
    /// Step 4 of the 2026-08-22 document-wall plan: when a rule becomes a test, the
    /// prose in `CLAUDE.md` shrinks to a sentence and a pointer, and the reasoning
    /// lives on the test's own doc comment. **That trade is only safe if the pointer
    /// cannot rot** — a renamed test would leave the mandatory documents citing
    /// something that does not exist, and a reader following it finds nothing.
    ///
    /// # Why module-qualified citations, and only those
    ///
    /// Measured before building this. The mandatory docs hold **27** backticked
    /// snake_case identifiers, of which **9 are not code items at all**: JSON fields
    /// (`differentiated_rows`, `n_differentiations`, `zero_crossing_condition`), a
    /// crate (`egui_commonmark`), examples (`gen_lab_catalogue`), an event name
    /// (`ide_opened_file`). **A checker over all of them would be a third false
    /// positives, and a checker that cries wolf gets switched off.**
    ///
    /// `module::item` is unambiguous. There were **14, and all 14 resolve** once
    /// modules count as items — `playback::tests_layout` is a `mod`, not a
    /// `fn`, which is the single case that made the naive form look broken (it was
    /// `connection_anim`'s until that check was generalised). **It
    /// starts green with no exemption list**, which is the only shape of this check
    /// worth having.
    ///
    /// # What it does not claim
    ///
    /// Resolution is by **name**, via [`symbol_is_defined`] — the same resolver the
    /// `unbuilt:` tags use. It proves the name exists somewhere in the workspace, not
    /// that the citation points at the right thing. A rename is what this catches.
    #[test]
    fn qualified_citations_resolve() {
        const DOCS: &[&str] = &[
            "CLAUDE.md",
            "docs/working-with-doug.md",
            "docs/CHARTER.md",
            "docs/README.md",
            "docs/fixture-labs/README.md",
            // Added 2026-08-23. It is the project's insurance document — read by
            // nobody, so its prose had no correction loop at all, and one paragraph
            // had been describing a *deleted* re-run for weeks.
            "docs/architecture.md",
        ];

        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // **Citations into other people's crates are not ours to resolve.**
        // `rust_source_texts()` walks `crates/`, `hrw/src` and `hrw/examples`, so a
        // workspace symbol resolves and `egui_plot::Plot::link_axis` never can.
        // Taken from `Cargo.toml` rather than a hand-written list so it cannot rot:
        // a dependency added later is skipped without anyone remembering to.
        let manifest = std::fs::read_to_string(hrw.join("Cargo.toml")).expect("hrw/Cargo.toml");
        let mut external: Vec<String> = vec!["std".into(), "core".into(), "alloc".into()];
        let mut in_deps = false;
        for line in manifest.lines() {
            let t = line.trim();
            if t.starts_with('[') {
                in_deps = t.contains("dependencies");
                continue;
            }
            if in_deps && let Some((name, _)) = t.split_once('=') {
                let name = name.trim().trim_matches('"');
                if !name.is_empty() && !name.starts_with('#') {
                    external.push(name.replace('-', "_"));
                }
            }
        }

        let mut checked = 0usize;
        let mut skipped = 0usize;
        let mut broken: Vec<String> = Vec::new();
        for rel in DOCS {
            let text =
                std::fs::read_to_string(hrw.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
            for cite in qualified_code_citations(&text) {
                let root = cite.split("::").next().unwrap_or("");
                if external.iter().any(|e| e == root) {
                    skipped += 1;
                    continue;
                }
                checked += 1;
                if !symbol_is_defined(&cite) {
                    broken.push(format!("{rel}: `{cite}`"));
                }
            }
        }
        let _ = skipped;

        // Non-vacuity: an extractor that silently matched nothing would pass forever.
        assert!(
            checked >= 10,
            "only {checked} qualified citations found \u{2014} the extractor is broken, \
             not the documents"
        );
        assert!(
            broken.is_empty(),
            "a document cites code that does not exist:\n  {}\n\nThese are pointers a \
             reader follows instead of prose that was deleted, so a dead one is worse \
             than the paragraph it replaced. Fix the citation, or restore what it \
             pointed at.",
            broken.join("\n  "),
        );
    }

    /// **Every lab hyperlink looks the same: no glyph in the text, no bold around it.**
    ///
    /// Doug, 2026-08-30: *"In the labs, hyperlinks are given inconsistent visual
    /// treatment… Implement consistent visual treatment for all hyperlinks. There
    /// should not be triangles preceding the hyperlinks. The text of hyperlinks should
    /// be blue. And hyperlinks should be underlined when hovered."*
    ///
    /// # Why bold is the rule that matters, and how it was isolated
    ///
    /// Blue text and the hover underline are `egui_commonmark`'s defaults, so nothing
    /// had to be styled — **something was suppressing them**. `the-concepts` wrapped its
    /// route links as `**[text](url)**` and rendered them in the body colour; every
    /// other lab's links are plain and rendered blue.
    ///
    /// The two labs also differed in list context, so that was not yet a conclusion.
    /// It became one by counting: **bolded links existed only in `the-concepts`** — the
    /// one lab Doug reported as not blue — while `connect-expansion` has plain links
    /// *inside list items* and reads blue. One difference tracked the symptom and the
    /// other did not.
    ///
    /// **CONFIRMED ON SCREEN 2026-08-30**, and the upgrade matters because the rule
    /// below rests on it. Unwrapping the bold was reasoning from a correlation — colour
    /// cannot be queried headlessly, so the fix shipped explicitly unverified — and Doug
    /// then reported the route links rendering correctly. Bold suppressing the hyperlink
    /// colour is now an observation, not an inference, and a later session need not
    /// re-derive it.
    ///
    /// # What this forbids, and what it does not
    ///
    /// It forbids a **triangle inside link text** and **bold wrapped around a whole
    /// link**. It does not forbid bold near a link: `**3a ·** [matching-live](…)` is
    /// fine and is how that entry keeps its numbering emphasis while the link stays
    /// blue. Nor does it police colour directly — colour is not in the accessibility
    /// tree, so the markup that suppresses it is the checkable proxy.
    #[test]
    fn lab_hyperlinks_are_styled_consistently() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");

        let mut findings: Vec<String> = Vec::new();
        let mut scanned = 0usize;
        for entry in std::fs::read_dir(&dir).expect("fixture-labs must be readable") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_name()
                .expect("named")
                .to_string_lossy()
                .into_owned();
            // Generated from the labs themselves, so it carries whatever they do.
            if name == "CATALOGUE.md" {
                continue;
            }
            let text = std::fs::read_to_string(&path).expect("readable lab");
            scanned += 1;

            for (n, line) in text.lines().enumerate() {
                let n = n + 1;
                if let Some(at) = line.find("](hrw://").or_else(|| line.find("](http")) {
                    let before = &line[..at];
                    if let Some(open) = before.rfind('[') {
                        let link_text = &before[open + 1..];
                        if link_text.contains('\u{25b6}') || link_text.contains('\u{25b2}') {
                            findings.push(format!(
                                "{name}:{n}: link text carries a triangle \u{2014} the \
                                 glyph is not part of the link and the labs dropped it"
                            ));
                        }
                        if before[..open].ends_with("**") && line[at..].contains(")**") {
                            findings.push(format!(
                                "{name}:{n}: link is wrapped in bold, which renders it in \
                                 the BODY colour instead of blue. Put the bold beside the \
                                 link (`**label** [text](url)`), not around it"
                            ));
                        }
                    }
                }
            }
        }

        // Non-vacuity: a scan that reached no lab would pass for ever.
        assert!(
            scanned >= 10,
            "only {scanned} labs scanned \u{2014} the run is broken, not the labs"
        );
        assert!(
            findings.is_empty(),
            "lab hyperlinks are styled inconsistently:\n  {}",
            findings.join("\n  "),
        );
    }

    /// **A `finding <ID>` citation must name a row that exists in the findings log.**
    ///
    /// # The renumbering this catches
    ///
    /// `45e8569e` split [`docs/ui-findings.md`] into two tables and renumbered as it
    /// went: C8-C11 became R1-R4, and the old C7 became C12. One comment was left
    /// citing **C9**, an ID that has not existed since 2026-08-02. Found on 2026-08-30
    /// by a session that followed it, found nothing, and declined to repeat it in a new
    /// comment; Doug supplied the answer, which is that it should have become **R2**.
    ///
    /// # Why only the explicit `finding <ID>` form
    ///
    /// Measured before building this. `src/` cites these IDs **bare** more often than
    /// not — `C6`, `C12`, `C20` — and a bare two-character token cannot be told apart
    /// from an identifier, a hex byte or a Modelica component without reading around
    /// it. The explicit form is unambiguous, it is what the prose citations use, and
    /// **it would have caught C9**, which is the case that motivated this. A checker
    /// that cries wolf gets switched off.
    ///
    /// # What it does not claim
    ///
    /// **It proves the ID resolves, never that it resolves to the RIGHT row** — and
    /// that gap is real here rather than theoretical, because the same commit that
    /// stranded C9 moved old C7's content to C12. A stale citation to a **reused**
    /// number points confidently at the wrong finding and passes this check. Only
    /// reading the entry catches that.
    #[test]
    fn a_cited_finding_id_exists_in_the_findings_log() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        /// `C12` / `R2` and nothing else — the shape of a findings-log row label.
        fn is_finding_id(tok: &str) -> bool {
            let mut cs = tok.chars();
            matches!(cs.next(), Some('C' | 'R')) && tok.len() > 1 && cs.all(|c| c.is_ascii_digit())
        }

        // The log's own table rows define which IDs exist: `| R2 | ... | ... |`.
        let log = hrw.join("docs/ui-findings.md");
        let log_text = std::fs::read_to_string(&log).expect("docs/ui-findings.md must be readable");
        let defined: Vec<&str> = log_text
            .lines()
            .filter_map(|l| l.strip_prefix('|'))
            .filter_map(|rest| rest.split('|').next())
            .map(str::trim)
            .filter(|t| is_finding_id(t))
            .collect();
        assert!(
            defined.len() > 10,
            "only {} finding rows parsed from ui-findings.md \u{2014} the table format \
             changed and this check is measuring nothing, not the log shrinking",
            defined.len(),
        );

        // Every `finding <ID>` in the sources, with where it was written.
        let src = hrw.join("src");
        let mut cited: Vec<(String, String)> = Vec::new();
        let mut run = vec![src.clone()];
        while let Some(dir) = run.pop() {
            for e in std::fs::read_dir(&dir)
                .expect("src must be readable")
                .flatten()
            {
                let p = e.path();
                if p.is_dir() {
                    run.push(p);
                    continue;
                }
                if p.extension().and_then(|x| x.to_str()) != Some("rs") {
                    continue;
                }
                let rel = p
                    .strip_prefix(&src)
                    .expect("under src")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&p).expect("readable");
                for (n, line) in text.lines().enumerate() {
                    for after in line.split("finding ").skip(1) {
                        // Trailing prose punctuation is not part of the ID.
                        let tok = after
                            .split_whitespace()
                            .next()
                            .unwrap_or("")
                            .trim_end_matches([',', '.', ';', ':', ')', '`', '*']);
                        if is_finding_id(tok) {
                            cited.push((format!("{rel}:{}", n + 1), tok.to_owned()));
                        }
                    }
                }
            }
        }

        // Non-vacuity: an extractor that matched nothing would pass forever, which is
        // the wrong-negative this repository treats as the error nobody catches.
        assert!(
            cited.len() >= 4,
            "only {} `finding <ID>` citations found \u{2014} the extractor is broken, \
             not the sources",
            cited.len(),
        );

        let dangling: Vec<String> = cited
            .iter()
            .filter(|(_, id)| !defined.iter().any(|d| d == id))
            .map(|(where_, id)| format!("{where_}: finding {id}"))
            .collect();
        assert!(
            dangling.is_empty(),
            "a comment cites a findings-log entry that does not exist:\n  {}\n\nIDs are \
             renumbered when the log is reorganised \u{2014} C8-C11 became R1-R4 on \
             2026-08-02 \u{2014} so check `docs/ui-findings.md` for the entry's current \
             label rather than deleting the citation.",
            dangling.join("\n  "),
        );
    }

    /// **Editing a guarded lab table must not be committed behind the FAST gate.**
    ///
    /// # The gap this closes, and why it was not a rule problem
    ///
    /// `CLAUDE.md`'s gate procedure greps the staged **paths**: anything under `src/`,
    /// `crates/`, `examples/` or `Cargo.toml` means FULL, and everything else means
    /// FAST. A lab edit is docs-only, so it returns FAST — **correctly, for the prose
    /// that is most of a run's output.** But the five `<!-- pane-* -->` tables in
    /// `connect-expansion.md` are verified by *slow* tests, so editing one and running
    /// FAST means the verification does not happen: a green suite over an unchecked
    /// claim. **That is the silent wrong negative this repository treats as the error
    /// nobody catches**, because acting on it means *not looking*.
    ///
    /// The rule already said *"editing one of those tables means FULL, whatever the grep
    /// says"*. **It was enforced by remembering** — Doug asked on 2026-08-22 whether any
    /// edit to that lab triggers FULL, which is exactly the question a remembered rule
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
    fn editing_a_guarded_lab_table_needs_the_full_gate() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let dir = hrw.join("docs/fixture-labs");
        let repo = hrw.parent().expect("hrw lives inside the workspace");

        let mut changed: Vec<String> = Vec::new();
        let mut compared = 0usize;

        for entry in std::fs::read_dir(&dir).expect("fixture-labs must be readable") {
            let path = entry.expect("readable dir entry").path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let working = std::fs::read_to_string(&path).expect("readable lab");
            let here = guarded_regions(&working);
            if here.is_empty() {
                continue; // no guarded tables: prose edits are genuinely FAST
            }

            let name = path.file_name().expect("named file").to_string_lossy();
            let rel = format!("hrw/docs/fixture-labs/{name}");
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
            "a guarded lab table changed, and this is the FAST suite \u{2014} those tables \
             are verified against a real compile by slow-gated tests, so committing now \
             would land an unchecked claim behind a green suite.\n\n  {}\n\nRun the LAB \
             gate before committing \u{2014} 11.1 s, not the FULL gate's ~101:\n  cargo \
             test -p hrw --lib --features slow-tests -- --test-threads=1 doc_citations \
             lab\n\n(`cargo run -p hrw --example gate` selects this for you. It is the \
             right gate only while the diff is docs-only; a src/ change still needs \
             FULL.)",
            changed.join("\n  "),
        );

        // Non-vacuity: this passing must mean "compared and found equal", never "found
        // nothing to compare". Outside a git checkout `compared` is 0 and the check is
        // honestly inert -- see the doc comment.
        if compared == 0 {
            eprintln!(
                "note: no lab compared against HEAD (not a git checkout?) \u{2014} \
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

    /// **A lab's claims about the equation-sheet PANE match what the pane will show.**
    ///
    /// # The gap this closes
    ///
    /// Until 2026-08-13, every *count* in a lab was read from a generated trace and was
    /// sound, while every *rendering* claim — what the groups are called, how many are in
    /// each — was **unverified**, because Claude cannot see the GUI. Doug run
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
    /// A lab that describes a pane carries a table of its groups:
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
    fn lab_group_tables_match_the_real_equation_sheet() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

        // **The roster is DERIVED from the labs, not listed here** *(2026-08-31)*. It was
        // a `const PANES: &[(&str, &str)]`, so giving a lab a table about a new specimen
        // was a `src/` edit — the last routine lab act that forced the FULL gate, and one
        // of the leaks Doug named when he called a pause on lab content to fix lab
        // friction.
        //
        // **Deriving it also closes a hole the list had.** A roster entry with no marker
        // was reported; a **marker with no roster entry was silently unchecked**, so a
        // lab could add `<!-- pane-groups: Foo -->` and nothing would ever compile `Foo`
        // to compare it. Reading the markers makes that impossible by construction: the
        // thing that declares the claim is the thing that schedules the check.
        let mut panes: Vec<(String, String)> = Vec::new();
        for path in std::fs::read_dir(hrw.join("docs/fixture-labs"))
            .expect("the fixture-lab directory must be readable")
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            for (marker, _) in guarded_regions(&text) {
                // `pane-groups: RcCircuit` — only the group tables drive a compile; the
                // origin and frame tables are checked opportunistically beside them.
                if let Some(specimen) = marker.strip_prefix("pane-groups: ") {
                    let pair = (name.clone(), specimen.trim().to_owned());
                    if !panes.contains(&pair) {
                        panes.push(pair);
                    }
                }
            }
        }
        panes.sort();
        assert!(
            !panes.is_empty(),
            "no `<!-- pane-groups: … -->` marker was found in any lab, so this test would \
             pass having compared nothing"
        );
        let panes: Vec<(&str, &str)> = panes
            .iter()
            .map(|(t, s)| (t.as_str(), s.as_str()))
            .collect();
        let panes = &panes[..];
        let mut bad: Vec<String> = Vec::new();
        let mut checked = 0usize;

        for (lab, specimen) in panes {
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

            // The pane's real groups, from the value the renderer runs.
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

            let text = std::fs::read_to_string(hrw.join("docs/fixture-labs").join(lab))
                .unwrap_or_else(|e| panic!("read {lab}: {e}"));

            // **The table is found by an explicit marker, not by shape.** The first
            // version scanned every `| \`x\` |` row in the file and reported the lab's
            // *specimen* table as claiming groups called `RcCircuit` and `Drivetrain`.
            // A checker that guesses which table it is looking at produces findings the
            // reader has to triage, which is how a checker stops being read.
            let Some(rows) = marked_rows(&text, "pane-groups", specimen) else {
                bad.push(format!(
                    "{lab}: no `<!-- pane-groups: {specimen} -->` marker, so that pane's \
                     group table cannot be checked \u{2014} add one above the table, or \
                     remove the marker"
                ));
                continue;
            };
            let claimed: Vec<(String, String)> = rows
                .iter()
                .filter_map(|r| Some((r.first()?.clone(), r.get(1)?.clone())))
                .collect();

            // **Station 4's per-origin breakdown**, checked the same way. It is a different
            // question from the group table — origins are per *row*, groups are the
            // headings — and it was the last table in this lab holding numbers that
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
                            "{lab} ({specimen}): claims {n} rows with origin `{origin}`; \
                             the pane has {actual}"
                        ));
                    }
                }
            }

            // **The family heading is checked too**, because the nesting is a claim
            // about *why* those equations exist. A lab that lists the children while
            // never naming the parent is back to presenting them as unrelated siblings
            // — the defect the grouping was introduced to fix.
            for family in &families {
                checked += 1;
                if !text.contains(family) {
                    bad.push(format!(
                        "{lab}: the pane groups several kinds under `{family}` and the lab \
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
                        "{lab}: `{label}` is listed as {claimed_n}; the pane has {n}"
                    )),
                    Some(_) => {}
                    None => bad.push(format!(
                        "{lab}: the pane produces a group `{label}` ({n} rows) that the \
                         table never names"
                    )),
                }
            }

            // And the reverse: a row naming a group the pane does not produce.
            for (label, _) in &claimed {
                if !real.iter().any(|(l, _)| l == label) {
                    bad.push(format!(
                        "{lab}: the table claims a group `{label}` that {specimen}'s pane \
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
            "lab prose disagrees with the equation-sheet pane:\n  {}",
            bad.join("\n  "),
        );
    }

    /// **`connect-expansion.md` Station 1's set sizes match the connection replay.**
    ///
    /// # The last claim in that lab nobody could check
    ///
    /// Station 1 predicts *sets of 2, 2 and 3*, and sends the reader to **Flatten →
    /// Connections** — the only pane that shows connection sets. Every other claim in the
    /// lab became checkable when the equation sheet started publishing; this one rested
    /// on Claude having read a trace correctly and never on anything a test could see.
    ///
    /// # What it checks, and the distinction it is careful about
    ///
    /// **Per kind, never in aggregate.** `2, 2, 3` must appear as the sizes of the
    /// **potential** sets *and* independently as the sizes of the **flow** sets, because
    /// pairing by name means no merge ever crosses between members — so the two kinds
    /// come out with the same membership and are still six separate sets, not three.
    ///
    /// **Renamed from `lab_node_sizes_…` on 2026-08-31**, when Doug ruled the node
    /// abstraction out of the lab entirely: *"It is not helpful to me to draw textbook
    /// graphs or nodes for this."* The lab now predicts connection sets directly, which
    /// is the compiler's own noun — and the rename matters because a test named for a
    /// vocabulary its subject no longer uses is how a checker stops being read. **The
    /// assertions did not change**; they were always about sets, and only the words
    /// around them were about nodes.
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
    fn lab_set_sizes_match_the_connection_replay() {
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
            "Station 1 predicts sets of 2, 2 and 3, so the POTENTIAL sets must be three \
             sets of 2, 2 and 3 `.v` variables"
        );
        assert_eq!(
            flow,
            vec![2, 2, 3],
            "...and the FLOW sets must independently be 2, 2 and 3 `.i` variables. \
             Pairing by name is what keeps the two kinds from ever mixing"
        );

        // **The set count the pane declares.** Six, not three — the sizes above are per
        // kind, and `RcCircuit`'s two kinds come out with matching membership. Station 6
        // exists because that matching is not a law.
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
            "six sets produce 4 potential + 3 flow equations"
        );

        // The lab's own words, so a reworded prediction cannot drift from this check.
        let lab = std::fs::read_to_string(hrw.join("docs/fixture-labs/connect-expansion.md"))
            .expect("read connect-expansion.md");

        // **Every frame the lab cites by ORDINAL is the frame it says it is.**
        //
        // Station 2 links `…/Connections/frame/7` and `/frame/13` to point at the moment the
        // n-1 asymmetry happens. `fixture_lab_links_all_resolve` checks only that such a
        // link *parses*. An ordinal citation is the fragility this repository already
        // designed around once — `OpenLab` addresses stops by **slug**, because
        // "inserting a stop shifts every later citation silently, exactly as a source
        // line number does" — and one extra frame emitted by the flatten pass would move
        // both of these with nothing to notice.
        //
        // The answer is `matching_ledger`'s, and Doug's: *"Rotting is bad. If line
        // numbers will help, add line numbers."* Carry the ordinals, and fail loudly
        // when they move. This also pins the **order** the sets are formed in — flow
        // before potential — which the size assertions above cannot see, since they
        // sort.
        let cited = marked_rows(&lab, "pane-frames", "RcCircuit")
            .expect("Station 2 cites frames by number; the table pinning them must exist");
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
                .unwrap_or_else(|| panic!("the lab cites frame {n}, past the end of the replay"));
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
            lab.contains("/frame/") && cited.len() >= 2,
            "Station 2's two frame citations must both be pinned"
        );
        // **Station 1's wording is pinned in `docs/fixture-labs/pinned-claims.txt`**, not
        // here. The numbers above are proved against a real compile and guard nothing if
        // the lab has quietly stopped predicting them — but that half needs no compile,
        // so it moved to `every_pinned_lab_claim_holds` in the FAST suite, where a
        // reword fails in seconds instead of at the next FULL run.
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

    /// **Equation text a lab quotes is text HRW actually renders.**
    ///
    /// Doug, 2026-08-12, running `connect-expansion.md`: *"the Connect sub-lab has this
    /// equation text: `f_x[19]  connection equation: src.p.v = R.p.v` but in the Flatten
    /// → Equations sub-tab that equation is shown with `0 = src.p.v - R.p.v`."*
    ///
    /// **Neither string was invented — and that is what made it hard to see.** Rumoca
    /// stores every continuous equation as an expression that must equal zero, so the
    /// equation sheet prints the **residual** form `0 = src.p.v - R.p.v`, while the
    /// structural report writes a **label** for a human reading a matching:
    /// `f_x[19] (connection equation: src.p.v = R.p.v)`. Both are real. The lab quoted
    /// one and sent the reader to the other, which is a **provenance** error rather than
    /// a fabrication — and no spell-check, link check or count check could see it.
    ///
    /// *(Corrected 2026-08-13: this comment used to say the two forms "live in *different
    /// panes*". They do not. `view.json` shows the equation sheet carries **both** — the
    /// residual as `text`, the label as `origin` — which is the claim the lab got wrong
    /// too. Reading the pane rather than reasoning about it is what settled it.)*
    ///
    /// # What this checks, and what it deliberately does not
    ///
    /// Both forms are recoverable from the committed traces without a compile:
    /// `structural.json` carries every `equation` label and every `equation_text`. So a
    /// quoted string must appear in that union. **It does not verify the string is quoted
    /// from the pane the lab points at** — `lab_group_tables_match_the_real_equation_sheet`
    /// above does that, by compiling. This catches *invented* text and text that has
    /// drifted from the traces.
    ///
    /// Two shapes are recognised, because they are the two that appear:
    /// - `` `f_x[N] (…)` `` inline — the label form, compared verbatim.
    /// - `0 = <expr>` on its own line inside a fenced block — the sheet's form, compared
    ///   after stripping `0 = `. Placeholders containing `<` are skipped, so
    ///   `` `0 = <expression>` `` in prose is not mistaken for a quote.
    #[test]
    fn equation_text_quoted_in_labs_matches_the_traces() {
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
        let labs = hrw.join("docs/fixture-labs");
        let mut lab_files: Vec<PathBuf> = Vec::new();
        collect_markdown(&labs, &mut lab_files);

        for path in lab_files {
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
        // lab; a run that inspects none of them has stopped working.
        assert!(
            checked >= 5,
            "only {checked} quoted equation strings were inspected — the extraction is \
             broken, not the labs",
        );
        assert!(
            bad.is_empty(),
            "{} quoted equation string(s) match nothing HRW renders:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("equation strings checked against the traces: {checked}");
    }

    /// **Every lab the chain overview sends you into must link back to it.**
    ///
    /// # The friction this closes
    ///
    /// Doug, 2026-08-17: *"I encountered yet again an annoying bit of friction which
    /// happens when there's a top-level lab which links to subordinate labs. I really
    /// want to be able to navigate backward from a subordinate lab to the top-level lab
    /// so that I can then navigate downward to another subordinate lab."*
    ///
    /// `the-concepts.md` is a hub: ten rows, each an `hrw://lab/<name>` link into a
    /// phase lab. **The links ran one way only.** Walking the chain therefore meant
    /// opening the picker between every pair of labs — with the hub sitting alphabetically
    /// in the middle of the list, indistinguishable from its own children.
    ///
    /// # Why a checker rather than just the ten edits
    ///
    /// **A missing back-link is invisible from inside the lab that lacks it.** Every
    /// other lab checker asks *"is what this document says true?"*, and a document with
    /// no way back says nothing false — the ten labs were internally perfect and the
    /// chain was still a dead end at every stop. Same shape as the Context Bar's missing
    /// background and the notebook's absent specimens: **a partial report leaves no gap
    /// where the missing part was.**
    ///
    /// So the property has to be stated across *two* files, which is exactly what nothing
    /// checked before. It is also the property most likely to rot: the eleventh lab added
    /// to the overview's table is one line in one file, and remembering the second edit is
    /// the part that fails.
    ///
    /// # What it checks
    ///
    /// For every `hrw://lab/<name>` the overview links to, `<name>.md` must contain
    /// `hrw://lab/the-concepts` — an **`hrw://` link, not a markdown one**. That
    /// distinction is the defect it was written against: `solve-lowering.md` and
    /// `matching-live.md` both referenced the overview as `[the-concepts.md](…)`, which
    /// HRW's commonmark renderer hands to the *operating system* as a relative file URL.
    /// It opens nothing, or opens a text editor. Only the `hrw://` form is a lab link.
    #[test]
    fn every_lab_the_overview_links_to_links_back() {
        let labs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let overview = labs.join(format!("{}.md", crate::lab::OVERVIEW_LAB));
        let text = std::fs::read_to_string(&overview).unwrap_or_else(|e| {
            panic!(
                "{} is the entry point every phase lab hangs off: {e}",
                overview.display()
            )
        });

        // The rows the overview sends the reader into, in order, deduplicated. Derived
        // from the links rather than from a list here, so adding a row to the table is the
        // only edit needed to bring a new lab under this check.
        let mut referenced: Vec<String> = Vec::new();
        for tail in text.split("hrw://lab/").skip(1) {
            let name: String = tail
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                .collect();
            if name.is_empty() || name == crate::lab::OVERVIEW_LAB {
                continue;
            }
            if !referenced.contains(&name) {
                referenced.push(name);
            }
        }

        // **Non-vacuity.** An extraction that finds nothing would report a chain of
        // perfect back-links across zero labs — the failure mode this file has hit four
        // times, most recently with three source-text checks matching their own prose.
        assert!(
            referenced.len() >= 9,
            "found only {} labs referenced by {}: {referenced:?} — the chain has nine \
             phases plus the live variant, so the extraction is broken rather than the \
             overview",
            referenced.len(),
            crate::lab::OVERVIEW_LAB,
        );

        let mut missing: Vec<String> = Vec::new();
        let mut markdown_only: Vec<String> = Vec::new();
        for name in &referenced {
            let path = labs.join(format!("{name}.md"));
            let Ok(body) = std::fs::read_to_string(&path) else {
                // A dangling row is `fixture_lab_links_all_resolve`'s business, not this
                // test's; reporting it twice would give one defect two names.
                continue;
            };
            if body.contains("hrw://lab/the-concepts") {
                continue;
            }
            // Distinguish "no way back at all" from "a way back that goes to the OS",
            // because they read identically in a diff and only one of them looks done.
            if body.contains(&format!("]({}.md)", crate::lab::OVERVIEW_LAB)) {
                markdown_only.push(name.clone());
            } else {
                missing.push(name.clone());
            }
        }

        assert!(
            markdown_only.is_empty(),
            "{} lab(s) reference the overview as a plain markdown file link, which HRW \
             hands to the operating system rather than opening as a lab — it looks like a \
             back-link in the source and does nothing when clicked. Use \
             `[▲ The chain overview](hrw://lab/{})`: {:?}",
            markdown_only.len(),
            crate::lab::OVERVIEW_LAB,
            markdown_only,
        );
        assert!(
            missing.is_empty(),
            "{} lab(s) the overview links into offer no way back to it, so running the \
             chain means reopening the picker at every stop. Add \
             `[▲ The chain overview](hrw://lab/{})` after the H1 and in the closing \
             section: {:?}",
            missing.len(),
            crate::lab::OVERVIEW_LAB,
            missing,
        );
        println!("labs linked back to the overview: {}", referenced.len());
    }

    /// The kinds a lab may declare, and whether that kind predicts.
    ///
    /// **`docs/lab-kinds-plan.md` is the authority**; this table is its executable half.
    /// The `predicts` column is the whole point of declaring a kind at all: without it,
    /// *"a concept lab that lost its predictions"* and *"a feature lab that correctly
    /// has none"* are the same document to a checker.
    const LAB_KINDS: &[(&str, bool)] = &[
        ("concept", true),
        ("feature", false),
        ("failure", false),
        ("calibration", false),
        ("hub", false),
    ];

    /// Every fixture lab, as `(name, kind, text)`.
    ///
    /// Panics rather than skipping an unreadable or untagged lab: a roster that
    /// silently shrinks turns every check below into a check of nothing, which is the
    /// vacuity failure this file has now hit five times.
    fn labs_with_kinds() -> Vec<(String, String, String)> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&dir).expect("docs/fixture-labs must be readable") {
            let path = entry.expect("a readable dir entry").path();
            if path.extension().and_then(|x| x.to_str()) != Some("md") {
                continue;
            }
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            // Neither is a lab: one is documentation ABOUT labs, the other is
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
                         one of {:?}. Without it no checker can tell a concept lab that lost \
                         its predictions from a feature lab that correctly has none.",
                        LAB_KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
                    )
                })
                .trim()
                .to_owned();
            out.push((name, kind, text));
        }
        out.sort();
        assert!(
            out.len() >= 20,
            "found only {} fixture labs — the enumeration is broken, and every check \
             below would pass over an empty corpus",
            out.len(),
        );
        out
    }

    /// The numbered stops of a lab, as `(heading, body-up-to-the-next-heading)`.
    ///
    /// **`Station 0` is returned like any other but is exempt from the prediction rule** by
    /// its caller: a zero stop is setup — it has something to check and nothing to
    /// predict. `matching-live.md` and `frame-seeking.md` both have one.
    fn numbered_stations(text: &str) -> Vec<(String, String)> {
        let mut stops: Vec<(String, String)> = Vec::new();
        for line in text.lines() {
            if line.starts_with("## ") {
                // The emoji-prefixed form the adjudication labs use — `## 📐 Station 1 — …`
                // — is a stop too. Matching on "Station " rather than on a line prefix is
                // what makes those four labs visible to this run at all.
                if line.contains("Station ") {
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
    /// Doug's model, 2026-08-17: *"while all kinds of labs have stops, each kind of lab
    /// might have different activities at its stops."* True — and the corpus says exactly
    /// one thing does **not** vary. `Predict` appears zero times in all 12 non-concept
    /// labs and once per stop in all 10 concept labs; `Expected` appears at every stop
    /// of every kind.
    ///
    /// **So `Expected` is what makes a lab a *test* rather than an explanation**, which
    /// is this directory's whole justification. `Predict` is merely how a *concept* lab
    /// earns its Expected — a feature lab earns the same claim by having Doug **do** the
    /// action, a failure lab by having him **read** the diagnosis.
    ///
    /// A stop with no falsifiable line is a paragraph with a heading, and a lab of those
    /// is the stored prose this project retired 1,632 lines of.
    #[test]
    fn every_stop_of_every_lab_owes_an_expected() {
        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        for (name, _kind, text) in labs_with_kinds() {
            for (heading, body) in numbered_stations(&text) {
                checked += 1;
                if !body.contains("**Expected:") && !body.contains("**Expected**") {
                    bad.push(format!("{name}.md  {}", heading.trim()));
                }
            }
        }

        assert!(
            checked >= 90,
            "only {checked} stops were inspected across the corpus — the heading run is \
             broken, not the labs",
        );
        assert!(
            bad.is_empty(),
            "{} stop(s) state nothing that could fail. Every stop of every kind owes an \
             **Expected:** — it is what makes a lab a test rather than an explanation:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!("stops carrying a falsifiable Expected: {checked}");
    }

    /// **A lab predicts if and only if its kind says it does.**
    ///
    /// # Both directions matter, and they fail for opposite reasons
    ///
    /// **A concept lab without predictions** has lost its engine. The prediction is what
    /// makes Doug an instrument rather than an audience — it is the reason the unit is a
    /// question rather than a topic — and a concept lab that merely explains and then
    /// shows is the "book" form he reported bouncing off.
    ///
    /// **A feature lab *with* predictions** is the other error, and it is the one Claude
    /// was about to commit. On 2026-08-17 Claude wrote that the 12 non-concept labs were
    /// *"unconverted, not differently designed"* — which would have meant converting them,
    /// and the count says otherwise: zero predictions across all twelve is a design, not a
    /// backlog. **There is no gradient anywhere in the corpus**, and this check is what
    /// keeps it that way.
    ///
    /// `Station 0` is exempt: it is setup, with an expectation to check and nothing to
    /// predict.
    #[test]
    fn a_lab_predicts_if_and_only_if_its_kind_says_so() {
        let mut bad: Vec<String> = Vec::new();
        let mut concept_stations = 0usize;
        let mut other_stations = 0usize;

        for (name, kind, text) in labs_with_kinds() {
            let predicts = LAB_KINDS
                .iter()
                .find(|(k, _)| *k == kind)
                .unwrap_or_else(|| {
                    panic!(
                        "{name}.md declares kind {kind:?}, which is not one of {:?}",
                        LAB_KINDS.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
                    )
                })
                .1;

            for (heading, body) in numbered_stations(&text) {
                let is_setup = heading.contains("Station 0");
                let has = body.contains("**Predict.**");
                if predicts {
                    concept_stations += 1;
                } else {
                    other_stations += 1;
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
            concept_stations >= 40 && other_stations >= 40,
            "inspected {concept_stations} concept stops and {other_stations} other stops — one \
             arm of this check is running on an empty set",
        );
        assert!(
            bad.is_empty(),
            "{} stop(s) disagree with their lab's declared kind:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        println!(
            "stops checked against their kind: {concept_stations} concept, {other_stations} other"
        );
    }

    /// **Every pinned lab claim still holds.**
    ///
    /// # What a pin is, and why it is not a test of the prose
    ///
    /// A pin does not check that a sentence is *true*. It checks that a sentence **some
    /// other guard depends on** is still there. `lab_set_sizes_match_the_connection_replay`
    /// proves against a real compile that `RcCircuit` forms sets of 2, 2 and 3 — and that
    /// proof guards nothing if Station 1 has quietly stopped predicting 2, 2 and 3. The pin is
    /// what makes such a reword loud instead of silent.
    ///
    /// # Why the strings are in `docs/` and this test is FAST — 2026-08-31
    ///
    /// Doug: *"Why did we have to run the full gate for a lab change?"* Because the lab
    /// change forced a **checker** change: these strings were hard-coded here, so rewording
    /// a pinned sentence was a `src/` edit and cost the ~170 s FULL gate — during a phase
    /// whose whole activity is rewording labs. Same finding as `reading-budgets.txt`, one
    /// file later: **data about documents was living in code.**
    ///
    /// **And it made the guard stronger, not merely cheaper.** `text.contains(…)` reads a
    /// file and needs no compile; these assertions were slow-gated only because they shared
    /// a function with ones that DO compile a specimen. Split out, a reword that forgets its
    /// pin now fails in the fast suite rather than at the next FULL run.
    ///
    /// **What still needs FULL, correctly:** a lab's *numbers*. A `<!-- pane-groups -->`
    /// table or a frame ordinal is checked against a real compile, and no data file changes
    /// that.
    #[test]
    fn every_pinned_lab_claim_holds() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let path = dir.join("pinned-claims.txt");
        let spec = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));

        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();
        for (i, line) in spec.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // Two separators, so the claim itself may contain `|` without escaping.
            let mut parts = line.splitn(3, '|');
            let (Some(lab), Some(kind), Some(claim)) = (parts.next(), parts.next(), parts.next())
            else {
                panic!(
                    "{}:{} is not `<lab> | require|forbid | <text>`: {line}",
                    path.display(),
                    i + 1,
                );
            };
            let (lab, kind, claim) = (lab.trim(), kind.trim(), claim.trim());

            // **A pin naming a missing lab fails rather than skipping.** A pin pointing
            // at nothing is indistinguishable from one that passes, which is the
            // claims-of-absence trap in a new place.
            let text = std::fs::read_to_string(dir.join(lab)).unwrap_or_else(|e| {
                panic!(
                    "{}:{} pins {lab}, which is not readable: {e}",
                    path.display(),
                    i + 1
                )
            });

            let present = text.contains(claim);
            match kind {
                "require" if !present => bad.push(format!("{lab} no longer says {claim:?}")),
                "forbid" if present => {
                    bad.push(format!("{lab} says {claim:?}, which is forbidden"))
                }
                "require" | "forbid" => {}
                // **`blurb` pins the CATALOGUE SUMMARY, which is derived from position.**
                // `LabSource::blurb_of` takes a lab's first bolded line, so inserting any
                // bolded paragraph above the opening one silently replaces the summary —
                // once with a mid-sentence fragment. It bit three times on 2026-08-31 and
                // nothing reported it, so the reflex became regenerating the catalogue
                // after every prose edit. A prefix is enough: an insertion changes what
                // comes first, which is exactly what this compares.
                "blurb" => {
                    let actual = crate::lab::LabSource::blurb_of(&text);
                    if !actual.starts_with(claim) {
                        bad.push(format!(
                            "{lab}'s catalogue blurb starts {actual:?}, not {claim:?} — \
                             a bolded line was inserted above the opening one, which \
                             silently rewrites the summary",
                        ));
                    }
                }
                other => panic!(
                    "{}:{} has kind {other:?}; only `require`, `forbid` and `blurb` exist",
                    path.display(),
                    i + 1,
                ),
            }
            checked += 1;
        }

        // Non-vacuity: an empty or unparsed file must not read as "all pins hold".
        assert!(
            checked >= 6,
            "only {checked} pins were read from {} — the file or the parse is broken, \
             which is worse than a failing pin because it looks like success",
            path.display(),
        );
        assert!(
            bad.is_empty(),
            "{} pinned claim(s) no longer hold. A pin marks a sentence another guard \
             depends on; if the reword is intended, update `pinned-claims.txt` in the \
             same commit — that file says what each pin is protecting:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
    }

    /// **No lab calls its units "acts" again.**
    ///
    /// A ratchet, in the shape `app_does_not_regrow_its_field_count` established. The
    /// rename cost 110 edits across ten labs and it would come back one heading at a
    /// time, written from the memory of a lab read an hour earlier — which is exactly how
    /// the word arrived in the first place: `matching.md` shipped with Acts on the same
    /// day `dae-construction.md` shipped with Stops, and nobody noticed for thirteen days.
    #[test]
    fn no_lab_heading_calls_a_stop_an_act() {
        let mut bad: Vec<String> = Vec::new();
        for (name, _kind, text) in labs_with_kinds() {
            for (i, line) in text.lines().enumerate() {
                if line.starts_with("## ") && (line.contains("Act ") || line.contains("Scene ")) {
                    bad.push(format!("{name}.md:{}  {}", i + 1, line.trim()));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} heading(s) call a stop an act or a scene. The top-level noun is `lab`, so \
             the unit is a `stop`; theatre vocabulary casts the reader as an audience, and \
             the labs exist to make him an instrument:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
    }

    /// **A lab never links to a bare file path.**
    ///
    /// # The same defect twice, eleven months apart in file type only
    ///
    /// A lab is rendered *inside HRW*, and a link egui does not recognise is handed to
    /// **`open_url`** — the OS browser. So a relative path that reads perfectly in an
    /// editor becomes, on click, Chrome being asked to fetch `../upstream-issues.md`.
    ///
    /// - **2026-07-30** — eighteen links to `.nb` notebooks opened the browser, which does
    ///   nothing useful with a Wolfram notebook. Fixed by adding the `hrw://notebook/`
    ///   verb and rewriting them.
    /// - **2026-08-31** — five labs linked `[upstream-issues.md](../upstream-issues.md)`.
    ///   Doug: *"causes an attempt to open the file in Chrome instead of attempting to
    ///   open the file in VS Code."* Fixed by adding `hrw://doc/`.
    ///
    /// **The July fix did not generalise because it was a VERB, not a RULE.** Adding
    /// `hrw://notebook/` fixed the eighteen links that existed and did nothing whatever
    /// about the nineteenth, or about the first `.md` one — nothing in the repository said
    /// *"a lab link goes through HRW"*, so the next author wrote the natural markdown and
    /// no gate disagreed. **This test is the half that generalises**, and it is why the
    /// `.md` recurrence should be the last of its family: a link to a `.pdf`, a `.csv` or
    /// a source file now fails here on the day it is written, before it is ever clicked.
    ///
    /// # What it does NOT forbid
    ///
    /// **`http://` and `https://` are fine, and are not an oversight.** The browser is the
    /// *correct* destination for a web page — a link to the Modelica specification or to a
    /// Wolfram documentation page should open exactly where it opens. The defect is never
    /// "the browser was used"; it is "the browser was used for a **local file**", which it
    /// cannot resolve at all. Both halves of that are stated so a later session does not
    /// read this as a ban on leaving HRW.
    ///
    /// **It also says nothing about `README.md` or `CATALOGUE.md`**, which
    /// [`labs_with_kinds`] already excludes: those are read in an editor, where a
    /// relative path is the right form and `hrw://` would be the broken one. **Same text,
    /// opposite correct answer, decided by where it is rendered** — which is why the rule
    /// is scoped to labs rather than to markdown.
    #[test]
    fn no_lab_links_to_a_bare_file_path() {
        let mut bad: Vec<String> = Vec::new();
        for (name, _kind, text) in labs_with_kinds() {
            for (i, line) in text.lines().enumerate() {
                for target in markdown_link_targets(line) {
                    let ok = target.starts_with("hrw://")
                        || target.starts_with("https://")
                        || target.starts_with("http://");
                    if !ok {
                        bad.push(format!("{name}.md:{}  ({target})", i + 1));
                    }
                }
            }
        }
        assert!(
            bad.is_empty(),
            "{} lab link(s) name a path rather than a verb. A lab is rendered inside \
             HRW, so a link egui does not recognise is handed to the OS BROWSER — which \
             cannot open a local file and will not try. Use `hrw://doc/<name>.md` for a \
             document under `hrw/docs/`, `hrw://notebook/<name>.nb` for a Wolfram \
             notebook, or `hrw://lab/<name>` for another lab. `http(s)://` is fine: the \
             browser is the right place for a web page.\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
    }

    /// Every `[text](target)` on one line, as the targets alone.
    ///
    /// Deliberately not a markdown parse: a lab's links are authored one per pair of
    /// brackets and this is the whole grammar in play. It does mean an inline-code span
    /// containing the sequence would be read as a link — which has not happened, and would
    /// fail loudly rather than silently if it did.
    fn markdown_link_targets(line: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = line;
        while let Some(open) = rest.find("](") {
            let after = &rest[open + 2..];
            match after.find(')') {
                Some(close) => {
                    out.push(&after[..close]);
                    rest = &after[close + 1..];
                }
                None => break,
            }
        }
        out
    }

    /// **`matching-live.md` keeps its three words apart.**
    ///
    /// This is the one document where **stop** (a place in the lab), **break** (where the
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
    fn the_live_lab_keeps_stop_break_and_anchor_apart() {
        let path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs/matching-live.md");
        let text = std::fs::read_to_string(&path).expect("matching-live.md must be readable");

        // **The note and the three wrong phrasings are pinned in
        // `docs/fixture-labs/pinned-claims.txt`**, moved there 2026-08-31 so that
        // rewording them is a docs change rather than a `src/` one. What stays here is
        // the part a data file cannot express: the *reason* this lab is special, which
        // is that `stop`, `break` and `anchor` are all live in it at once and the
        // Act→Stop rename CREATED that collision rather than exposing one.
        assert!(
            !text.is_empty(),
            "matching-live.md is empty; its vocabulary pins would then pass vacuously",
        );
    }

    /// **A `<lab>.md Station N` reference in the source resolves to a real heading.**
    ///
    /// # The gap this closes
    ///
    /// Nineteen comments in `src/` named a lab unit — *"evidence for
    /// `connect-expansion.md` Act 1"*, *"`matching.md` ends Act 3 with one"* — and every
    /// one of them dangled the moment the labs were renamed. Nothing noticed, because a
    /// comment is not compiled and a stale one is indistinguishable from a live one.
    ///
    /// **These are navigational, not decorative.** A comment that says *"see Station 4"* is
    /// how the next session finds the prose a piece of code exists to serve; pointing at a
    /// stop that is not there sends it looking for something that never arrives.
    ///
    /// **Quotations are exempt, and that exemption is the interesting part.** Six comments
    /// quote Doug saying "Act", and they stay verbatim forever — editing a quotation so it
    /// matches a rename that happened afterwards falsifies the record this repository's
    /// whole discipline rests on. So the check skips a line quoting him, which is why it
    /// looks only for the *current* vocabulary.
    #[test]
    fn a_stop_a_source_comment_cites_exists_in_that_lab() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let labs = hrw.join("docs/fixture-labs");

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
                // A lab named in backticks, then `Stop <n>` anywhere later on the same
                // line. **Adjacency was the first implementation and it was too narrow** —
                // it matched 3 of the 19 real references, because most of them read
                // "`matching.md` ends Station 3 with one" rather than "`matching.md` Station 3".
                // A check that inspects a sixth of its subject is most of the way to
                // vacuous while looking green.
                let Some(stop_at) = line.find("Station ") else {
                    continue;
                };
                let n: String = line[stop_at + "Station ".len()..]
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if n.is_empty() {
                    continue;
                }
                // The nearest lab named *before* the citation owns it.
                let head = &line[..stop_at];
                let Some(md) = head.rfind(".md`") else {
                    continue;
                };
                let Some(open) = head[..md].rfind('`') else {
                    continue;
                };
                let lab = &head[open + 1..md];
                if lab.is_empty() || lab.contains(' ') || lab.contains('/') {
                    continue;
                }

                checked += 1;
                let Ok(body) = std::fs::read_to_string(labs.join(format!("{lab}.md"))) else {
                    bad.push(format!("{src_name}:{}  no lab named {lab:?}", i + 1));
                    continue;
                };
                if !body.contains(&format!("Station {n} ")) {
                    bad.push(format!("{src_name}:{}  {lab}.md has no Station {n}", i + 1));
                }
            }
        }

        // **What this does NOT reach, said out loud rather than left as a green result.**
        // A citation whose lab is named on an *earlier* line of the same doc comment —
        // "/// **`connect-expansion.md` Station 1's set sizes…**" followed later by "Station 1
        // predicts *sets of 2, 2 and 3*" — is invisible here, because the pairing is
        // per-line. Roughly
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
        println!("source comments citing a lab stop: {checked}");
    }

    /// **An equation id a lab cites must name the equation the prose claims.**
    ///
    /// # The gap this closes, found by falling into it
    ///
    /// `equation_text_quoted_in_labs_matches_the_traces` above verifies quoted
    /// *text*. Nothing verified a quoted **id**. On 2026-08-16 a new act of
    /// `connect-expansion.md` was written claiming `C.v` reads `der in f_x[19]` —
    /// a real, existing equation, and the wrong one: `f_x[19]` is the connection
    /// equation `src.p.v = R.p.v`, while the capacitor's rate law is `f_x[14]`.
    /// The number was written from memory of one seen an hour earlier.
    ///
    /// **Every existing checker would have passed it.** The id is well-formed, the
    /// equation exists, the link resolves, and no quoted text was wrong. Doug would
    /// have run to that stop, seen `der in f_x[14]` on screen, and had to work out
    /// which of the two of us was mistaken — the precise failure this repository
    /// exists to prevent, since he cannot tell which parts are false.
    ///
    /// # What it checks
    ///
    /// The Why column renders `der in f_x[N]`, so a lab quoting that string is
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
    /// which the lab template guarantees: every expectation follows a **▶ Look**
    /// link. A citation with no preceding link is counted and skipped rather than
    /// guessed at.
    ///
    /// Fast by construction — `structural.json` carries every `id` and
    /// `equation_text`, so nothing here compiles anything.
    ///
    /// # What the current corpus does NOT exercise, stated rather than left silent
    ///
    /// **Check 3 is vacuous today.** The only lab citing a Why cell is
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
    /// f_x[0], not f_x[1]"*. **The first lab to quote a Why cell on a multi-state
    /// specimen makes this real**, and until then check 3 is a guard nothing proves
    /// still works.
    #[test]
    fn an_equation_id_a_lab_cites_names_the_equation_the_prose_claims() {
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

        let mut lab_files: Vec<PathBuf> = Vec::new();
        collect_markdown(&hrw.join("docs/fixture-labs"), &mut lab_files);

        for path in lab_files {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let lab = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
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
                        &format!("{lab}:{}", i + 1),
                        "no preceding hrw://load link names the specimen",
                    );
                    continue;
                };
                ledger.inspected();
                let Some(equations) = by_specimen.get(spec) else {
                    bad.push(format!(
                        "{lab}:{}: cites an equation of `{spec}`, which has no committed \
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
                            "{lab}:{}: cites `{id}`, which names no equation in {spec}",
                            i + 1
                        ));
                        continue;
                    };
                    if !eq_text.contains("der(") {
                        bad.push(format!(
                            "{lab}:{}: claims `{id}` differentiates something, but that \
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
                                "{lab}:{}: says `{var}` reads `{cite}`, but {spec} \
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
            "no `der in f_x[N]` citations were found in any lab; the extraction is \
             broken, or the Why column stopped being quoted (in which case delete \
             this test rather than let it pass on nothing)",
        );
        assert!(
            bad.is_empty(),
            "{} lab citation(s) name the wrong equation:\n  {}",
            bad.len(),
            bad.join("\n  "),
        );
        // **Budget zero.** Every citation in a committed lab follows a ▶ Look link,
        // because the template requires one. A skip here means a lab stopped saying
        // which specimen it is talking about — a lab defect, not a reason for this
        // check to quietly cover less.
        ledger.assert_coverage("lab equation-id citations", 1, 0);
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
    /// exists — because `fixture_lab_links_all_resolve` and the citation checks
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

        // **`docs/running-things.md`, not `CLAUDE.md`** — the gate commands moved there on
        // 2026-09-01 under charter Decision 11, which puts procedure in its own file. This
        // test caught the move by failing, which is what it is for.
        let procedures = std::fs::read_to_string(hrw.join("docs/running-things.md"))
            .expect("docs/running-things.md");
        // The gate line, as the file spells it. Located rather than assumed, so a
        // reworded section fails here instead of silently matching nothing.
        // **Not merely "contains slow-tests"** — the ITERATE line does too, and the
        // first draft of this test found *that* one and reported a failure about the
        // wrong command. The gate is the unfiltered invocation, so the placeholder is
        // what distinguishes them.
        let gate = procedures
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
    /// - check 3 of `an_equation_id_a_lab_cites_names_the_equation_the_prose_claims`,
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
    /// Every checker in this file runs subjects — specimens, labs, stage files —
    /// and every one of them has `continue` arms for subjects it cannot read. Those
    /// arms are invisible: the check reports what it *found*, never what it *passed
    /// over*, so a deleted trace or a renamed lab quietly shrinks coverage while the
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
                 {min_inspected} \u{2014} the run is broken, not the corpus",
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
    /// It was found because Doug asked whether the labs needed updating — that is,
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

    /// **The tearing lab gains its dynamic-loop act at the moment it is converted.**
    ///
    /// **DELIVERED 2026-08-17.** `tearing.md` was converted with `LoopWithInertia` as its
    /// Station 5, so this test now runs its *enforcing* branch — 5 `**Predict.**` markers and
    /// an `hrw://load/LoopWithInertia` link — rather than the not-yet-converted early
    /// return. The `## OWED` note it used to guard is gone because the act replaced it,
    /// which is the outcome the note asked for. What remains guarded: the act cannot be
    /// removed while the lab stays converted.
    ///
    /// # A commitment, made mechanical
    ///
    /// Doug, 2026-08-16: *"Eventually, I will want very much to add LoopWithInertia to
    /// the tearing lab, as you've recommended. Please ensure that we do that."* He is
    /// running the labs in compiler-phase order and is on Connections → DAE, so
    /// tearing is weeks away. A promise made now, in a conversation, is exactly the
    /// thing `CLAUDE.md` says must live in the repository instead: *code whose
    /// rationale exists only in chat violates the rule the moment the session ends.*
    ///
    /// # Why it triggers on conversion rather than on a date
    ///
    /// The act cannot simply be written today — `tearing.md` is still in its
    /// 2026-08-08 prose form, and the agreement is that a lab is converted **as Doug
    /// runs it**, because the conversion is itself the teaching. So the commitment
    /// has to survive until that moment and fire precisely then.
    ///
    /// Conversion is detectable: a converted lab runs each act to a **Predict**. Only
    /// `connect-expansion.md` has been converted, and it carries six. So two or more
    /// `**Predict.**` markers means the work has started, and from that instant the
    /// lab must also mention `LoopWithInertia`.
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
    fn the_tearing_lab_gains_its_dynamic_loop_when_it_is_converted() {
        let lab = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs/tearing.md");
        let text = std::fs::read_to_string(&lab).expect("tearing.md exists");

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
            // Not yet run. Nothing to enforce, and saying so keeps the pass honest
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
             owed act is: the same 3-cycle as Station 1, now re-solved between every pair \
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
mod tests_lab_link_form {
    use std::path::PathBuf;

    /// **A lab referenced from a lab must be an `hrw://` link, not a file link.**
    ///
    /// Doug, 2026-08-19, testing the new Back control: *"I found a broken link in the
    /// blt-ordering lab. The link for matching.md causes an external browser window to
    /// open instead of opening that lab in HRW."*
    ///
    /// **There were eighteen of them.** HRW's commonmark renderer hands a relative file
    /// link to the operating system, so `[`matching.md`](matching.md)` opens a browser —
    /// or nothing — while looking in the source exactly like a working navigation. That
    /// is the same defect the hub back-links had on 2026-08-17, fixed there and never
    /// generalised, which is why it was still waiting in ten other documents.
    ///
    /// **Nothing could have caught it.** `fixture_lab_links_all_resolve` checks `hrw://`
    /// links; a markdown file link is not one, so it was invisible to every checker here
    /// — and invisible in the rendered pane too, since it looks like an ordinary link
    /// until clicked.
    ///
    /// The hub's `[file](x.md)` cells are exempt: they sit beside an `hrw://lab/…` link
    /// in the same cell and are labelled "file", which is a different offer rather than a
    /// broken one.
    #[test]
    fn a_lab_is_never_referenced_by_a_plain_file_link() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
        let mut checked = 0usize;
        let mut bad: Vec<String> = Vec::new();

        for entry in std::fs::read_dir(&dir).expect("the lab directory must be readable") {
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
            "only {total_links} links seen in a lab known to be full of them \u{2014} the \
             scan is broken, not the corpus",
        );
        assert!(
            bad.is_empty(),
            "{} lab reference(s) use a plain markdown file link. HRW hands those to the \
             operating system, so they open a browser rather than the lab \u{2014} and \
             they look identical to a working link in the source. Use \
             `[\u{25b6} name](hrw://lab/name)`:\n  {}",
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
            //
            // 5 -> 6 on 2026-08-23, same shape and triaged the same way. The one new
            // hit is inside `an_all_roster_lists_every_variant_of_its_enum`'s doc
            // block, which opens with its own summary and later leads a paragraph with
            // **It was not loud.** Every item added in that commit — the roster check,
            // `authored_regions`, `unterminated_authored`,
            // `doug_authored_prose_is_never_edited_silently` and the new pure tests —
            // carries its own summary, so there is no undocumented item an orphan could
            // belong to.
            ("doc_citations.rs", 6),
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
            ("lab_panel.rs", 1),
            ("ui_tests.rs", 5),
            // 19 → 20 on 2026-08-21 (`docs/ideas.md` #48, lever C). Triaged, not
            // waved through: the new hit is `worker.rs`'s
            // `compile_specimen_headless_matches_worker` doc, where a wrapped line
            // ends with a backtick and the next line is blank — the continuation-line
            // shape that accounts for 65 of the tree's 87 hits. The block has one
            // owner, that owner is documented, and the file has no undocumented item
            // for an orphan to belong to, which is the triage shortcut this test's
            // own doc prescribes.
            //
            // 20 → 15 + 5 on 2026-08-25, when the two `#[cfg(test)]` modules moved to
            // `worker/tests.rs` and `worker/test_msl.rs`. **The two halves sum to the
            // old total exactly**, which is the useful part: the move relocated the
            // orphan population without creating or hiding one. Both numbers are set
            // to the achieved value rather than rounded up, per this table's rule that
            // slack gets used.
            ("worker.rs", 15),
            ("worker/tests.rs", 5),
        ];

        let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files: Vec<(String, Vec<usize>)> = Vec::new();
        let mut run = vec![src.clone()];
        while let Some(dir) = run.pop() {
            for e in std::fs::read_dir(&dir)
                .expect("src must be readable")
                .flatten()
            {
                let p = e.path();
                if p.is_dir() {
                    run.push(p);
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
            "only {} source files scanned \u{2014} the run is broken, not the tree",
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

    /// The name an item line declares, ignoring visibility and modifiers.
    ///
    /// Returns `None` for anything that is not an item declaration, which is most
    /// lines — the first meaningful token has to be a kind keyword, so `impl`, `let`,
    /// `use`, `//` and a closing brace all fall out immediately.
    ///
    /// `mod` is deliberately absent from the kinds: `pub mod x;` is never documented
    /// in this tree, so including it would add sixty-odd permanently-undocumented
    /// names to every comparison without ever changing an answer.
    fn item_name(line: &str) -> Option<String> {
        const KINDS: &[&str] = &[
            "fn", "struct", "enum", "trait", "const", "static", "type", "union",
        ];
        const MODIFIERS: &[&str] = &["async", "unsafe", "extern", "default"];

        let mut it = line.split_whitespace().peekable();
        let kind = loop {
            let tok = it.next()?;
            // `pub`, `pub(crate)`, `pub(super)`, `pub(in path)` — all start the same.
            if tok.starts_with("pub") || MODIFIERS.contains(&tok) {
                continue;
            }
            if KINDS.contains(&tok) {
                // `const fn` is a fn; the name is one token further along.
                if tok == "const" && it.peek() == Some(&"fn") {
                    it.next();
                }
                break tok;
            }
            return None;
        };
        let _ = kind;

        let name: String = it
            .next()?
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        (!name.is_empty()).then_some(name)
    }

    /// Does the item on line `i` carry a doc comment, looking past its attributes?
    fn is_documented(lines: &[&str], i: usize) -> bool {
        let mut j = i;
        while j > 0 {
            let prev = lines[j - 1].trim();
            if prev.starts_with("#[") || prev.starts_with("#!") {
                j -= 1;
                continue;
            }
            return prev.starts_with("///");
        }
        false
    }

    /// Per item name: how many of its definitions are documented, and how many exist.
    ///
    /// Counted rather than flagged because a name can legitimately appear more than
    /// once in a file — `fn build` in two modules, a test helper repeated per module.
    /// Comparing counts keeps those cases from reading as a loss.
    fn doc_counts(text: &str) -> std::collections::HashMap<String, (usize, usize)> {
        let lines: Vec<&str> = text.lines().collect();
        let mut counts: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();
        for (i, line) in lines.iter().enumerate() {
            if let Some(name) = item_name(line) {
                let entry = counts.entry(name).or_insert((0, 0));
                entry.1 += 1;
                if is_documented(&lines, i) {
                    entry.0 += 1;
                }
            }
        }
        counts
    }

    /// Items documented in `before` that are still present in `after` but no longer
    /// documented.
    ///
    /// **The `after_total >= before_documented` clause is what keeps this quiet.**
    /// Without it, deleting a documented item or renaming one would fire — and both
    /// are ordinary work. The check is about a doc comment coming *off* an item that
    /// is still there, which is the only shape the insertion defect produces.
    fn items_that_lost_their_doc(before: &str, after: &str) -> Vec<String> {
        let (b, a) = (doc_counts(before), doc_counts(after));
        let mut lost: Vec<String> = b
            .iter()
            .filter(|(name, (before_doc, _))| {
                let (after_doc, after_total) = a.get(*name).copied().unwrap_or((0, 0));
                after_doc < *before_doc && after_total >= *before_doc
            })
            .map(|(name, _)| name.clone())
            .collect();
        lost.sort();
        lost
    }

    /// **No item may lose the doc comment it had at `HEAD`** — the insertion defect,
    /// caught by its other half.
    ///
    /// # Why this exists beside [`no_doc_block_gains_a_second_summary`]
    ///
    /// That check watches the **merged block**; this one watches the **stranded
    /// item**. One event, two symptoms, and the reason to have both is that the first
    /// is a *budget*: its shape is only ~29 % precise, so prose-heavy files carry an
    /// allowance, and a new orphan in a file under its ceiling is invisible. That is
    /// not hypothetical — on 2026-08-25 the defect happened a **fourth** time and was
    /// caught only because `worker.rs` sat at exactly 20 of 20. One under and
    /// `not_reached_stage` would have silently kept the helper's documentation.
    ///
    /// **A ratchet counts; it does not detect.**
    ///
    /// # Why a diff against `HEAD` rather than a population ratchet
    ///
    /// The obvious alternative — assert every item is documented — was measured
    /// first: **264 undocumented column-0 items across 56 files.** A budget over that
    /// has precisely the blind spot being fixed. But an item losing its doc is a
    /// **transition**, and a transition is exact: `git` holds the before-state, so
    /// there is no heuristic, no allowance, and nothing to triage.
    ///
    /// # What this does NOT cover
    ///
    /// **Only what changed since `HEAD`.** A defect that was committed and never
    /// touched again is invisible here — that stock is what the budgeted check covers.
    /// Together: one watches the flow exactly, the other watches the stock loosely.
    /// **On a clean tree this test is inert**, exactly like
    /// [`super::tests::editing_a_guarded_lab_table_needs_the_full_gate`], which is
    /// why its must-fire half is [`the_stranded_item_is_detected`] over literals
    /// rather than anything requiring a checkout.
    ///
    /// **Removing a doc comment on purpose fails here, and that is intended.** It is
    /// rare enough to be worth answering for, and there is no budget to hide it in.
    #[test]
    fn no_item_loses_its_doc_comment() {
        let hrw = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let git = |args: &[&str]| {
            std::process::Command::new("git")
                .arg("-C")
                .arg(&hrw)
                .args(args)
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        };

        let Some(changed) = git(&["diff", "--name-only", "HEAD", "--", "src"]) else {
            eprintln!("note: no git HEAD \u{2014} the lost-doc check is inert here");
            return;
        };

        let mut lost: Vec<String> = Vec::new();
        for rel in changed
            .lines()
            .map(str::trim)
            .filter(|l| l.ends_with(".rs"))
        {
            // `git diff` reports repo-relative paths; the blob spelling needs the same.
            let Some(before) = git(&[
                "show",
                &format!("HEAD:./{}", rel.trim_start_matches("hrw/")),
            ]) else {
                continue; // Added since HEAD: it had no doc comments to lose.
            };
            let path = hrw.join(rel.trim_start_matches("hrw/"));
            let Ok(after) = std::fs::read_to_string(&path) else {
                continue; // Deleted in the working tree.
            };
            for name in items_that_lost_their_doc(&before, &after) {
                lost.push(format!("{rel}: `{name}`"));
            }
        }

        assert!(
            lost.is_empty(),
            "an item that was documented at HEAD no longer is. The usual cause is an \
             item inserted ABOVE another item's doc comment, which makes the new item \
             adopt it and strands the old one \u{2014} anchor the edit on a CLOSING \
             BRACE, never on a `fn` line or a doc line. If you removed the doc \
             deliberately, say so in the commit:\n  {}",
            lost.join("\n  "),
        );
    }

    /// **The must-fire half**, over literals, because the test above is inert on a
    /// clean tree.
    ///
    /// The first case is the real 2026-08-25 defect in miniature: a helper inserted
    /// above `not_reached_stage` adopts its doc comment and strands it.
    #[test]
    fn the_stranded_item_is_detected() {
        let before = "/// Placeholder for a stage that did not run.\n\
                      fn not_reached_stage() {}\n";
        let after = "/// Placeholder for a stage that did not run.\n\
                     /// The one place the not-run sentence is worded.\n\
                     fn not_reached_note() {}\n\
                     fn not_reached_stage() {}\n";
        assert_eq!(
            items_that_lost_their_doc(before, after),
            vec!["not_reached_stage".to_string()],
            "the stranded item must be named"
        );

        // Adding a documented helper without disturbing anything is ordinary work.
        let clean = "/// Placeholder for a stage that did not run.\n\
                     fn not_reached_stage() {}\n\
                     /// The one place the not-run sentence is worded.\n\
                     fn not_reached_note() {}\n";
        assert!(
            items_that_lost_their_doc(before, clean).is_empty(),
            "a helper added below the closing brace is not a loss"
        );

        // Deleting or renaming a documented item must stay quiet.
        assert!(
            items_that_lost_their_doc(before, "fn something_else() {}\n").is_empty(),
            "a rename is not a lost doc comment"
        );
        assert!(
            items_that_lost_their_doc(before, "").is_empty(),
            "a deletion is not a lost doc comment"
        );

        // An attribute between the doc and its item is still documented.
        let attributed = "/// A test.\n#[test]\nfn a_test() {}\n";
        assert!(
            items_that_lost_their_doc(attributed, attributed).is_empty(),
            "attributes must not hide a doc comment"
        );

        // Non-vacuity: the parser finds items at all, and reads visibility forms.
        let counts = doc_counts(
            "/// A.\npub fn a() {}\n/// B.\npub(crate) const B: u8 = 1;\nfn c() {}\nimpl D {}\n",
        );
        assert_eq!(counts.get("a"), Some(&(1, 1)));
        assert_eq!(counts.get("B"), Some(&(1, 1)));
        assert_eq!(counts.get("c"), Some(&(0, 1)), "undocumented, but counted");
        assert_eq!(counts.get("D"), None, "`impl` is not an item declaration");
    }
}

/// **The guarded-region parser, tested without git or a compile.**
///
/// [`tests::editing_a_guarded_lab_table_needs_the_full_gate`] needs a git checkout to
/// say anything, and is honestly inert without one. These do not: they pin the two
/// properties the whole FAST/FULL decision rests on — **a prose edit is not a guarded
/// change, and a table edit is** — as pure functions of text.
///
/// They are also the must-fire half. A fingerprint that ignored the rows would call
/// every edit safe, and nothing else here would notice.
#[cfg(test)]
mod tests_guarded_regions {
    use super::tests::{guarded_regions, marked_rows};

    /// A lab with one guarded table and prose on both sides of it.
    fn lab(rows: &str, prose: &str) -> String {
        format!(
            "# A lab\n\n{prose}\n\n<!-- pane-groups: RcCircuit -->\n\n\
             | group | rows |\n|---|---|\n{rows}\n\nMore prose about {prose}.\n"
        )
    }

    #[test]
    fn editing_prose_is_not_a_guarded_change() {
        let before = lab("| `Component equations` | 16 |", "the voltages are equal");
        let after = lab("| `Component equations` | 16 |", "the potentials are equal");
        assert_ne!(before, after, "the fixture must actually differ");
        assert_eq!(
            guarded_regions(&before),
            guarded_regions(&after),
            "a prose edit was reported as a guarded change, which would send every run \
             edit to the 220s gate",
        );
    }

    #[test]
    fn editing_a_guarded_row_is_a_guarded_change() {
        let before = lab("| `Component equations` | 16 |", "same prose");
        let after = lab("| `Component equations` | 17 |", "same prose");
        assert_ne!(
            guarded_regions(&before),
            guarded_regions(&after),
            "a changed table row was reported as safe: this is the silent wrong negative \
             the gate check exists to prevent",
        );
    }

    #[test]
    fn removing_a_marker_is_a_guarded_change() {
        let before = lab("| `Component equations` | 16 |", "same prose");
        let after = before.replace("<!-- pane-groups: RcCircuit -->", "");
        assert!(
            !guarded_regions(&before).is_empty() && guarded_regions(&after).is_empty(),
            "removing a marker must change the fingerprint -- it removes a table from \
             verification entirely",
        );
    }

    /// **A marker whose table was deleted must not adopt the next table in the file.**
    ///
    /// The bounded run added 2026-08-22. Unbounded, `pane-groups` here would skip past
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

/// **No lab table is wide enough to break the wrapping of the prose beneath it.**
///
/// # The defect this bounds
///
/// `lab_panel` renders into `ScrollArea::both()`, and inside a scroll area with the
/// horizontal axis enabled **a child that allocates beyond the `Ui`'s `max_rect`
/// expands it for every later sibling**. So a table wider than the panel silently
/// becomes the wrap width for every paragraph after it. Doug run into it on
/// 2026-08-28: *"prose does get wrapped, but only according to the width of the table
/// which precedes it."*
///
/// **The code defect is open and deliberately unfixed** — `ui-findings.md` C21, with
/// two measured-ineffective attempts. Doug ruled that the horizontal scrollbar makes
/// everything reachable, so the cost is cosmetic and the fix is not worth the risk to
/// the rendering path. **This bounds the content instead.**
///
/// # Where 90 comes from
///
/// Measured, not guessed, with a harness that renders a table of a given width and
/// reads back the width of the paragraph after it. At the 40 % default panel on a
/// 1600pt window — 590pt of prose:
///
/// ```text
/// 84, 88, 90 chars   paragraph after = 591pt   correct
/// 92 chars           paragraph after = 623pt   inheriting
/// 178 chars          paragraph after = 1180pt  the-concepts before conversion
/// ```
///
/// **The bound is panel-dependent and this is the reference panel.** A reader who drags
/// the divider narrower breaks sooner; one on a wide monitor never sees it at all. 90 is
/// the widest that is safe at the default, which is where a lab is first read.
///
/// # The exceptions, and why they are listed rather than fixed
///
/// **Lab prose is Doug's**, and converting a table changes how a lab teaches — so
/// the two below are recorded, not rewritten. `the-concepts` was converted on
/// 2026-08-28 *because he asked for it*, and its route table became a numbered list
/// and its three-graph matrix three labelled blocks.
#[test]
fn no_lab_table_is_wider_than_the_panel() {
    /// Widest table row that still lets the next paragraph wrap correctly.
    const MAX_TABLE_ROW: usize = 90;
    /// Labs whose tables are over and have not been converted. **Converting one is
    /// a change to how it teaches, so it needs Doug.** Removing a name from this list
    /// is the goal; adding one needs his say-so.
    const NOT_YET_CONVERTED: &[&str] = &[
        // 116.
        "structural-vs-numerical-rank.md",
        // 266, and not a lab: the directory's own README, never rendered in the
        // panel. Listed so the scan does not have to special-case a filename.
        "README.md",
    ];

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs/fixture-labs");
    let mut over: Vec<String> = Vec::new();
    let mut scanned = 0usize;
    for entry in std::fs::read_dir(&dir)
        .expect("fixture labs are readable")
        .flatten()
    {
        let path = entry.path();
        if path.extension().and_then(|x| x.to_str()) != Some("md") {
            continue;
        }
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if NOT_YET_CONVERTED.contains(&name.as_str()) {
            continue;
        }
        scanned += 1;
        let text = std::fs::read_to_string(&path).expect("readable");
        if let Some(widest) = text
            .lines()
            .filter(|l| l.trim_start().starts_with('|'))
            .map(str::chars)
            .map(Iterator::count)
            .max()
            .filter(|w| *w > MAX_TABLE_ROW)
        {
            over.push(format!("{name}: widest table row {widest} chars"));
        }
    }

    // Non-vacuity: a scan that found no labs would pass while checking nothing.
    assert!(
        scanned >= 10,
        "only {scanned} labs scanned; the run is broken"
    );

    assert!(
        over.is_empty(),
        "a lab table is wider than {MAX_TABLE_ROW} characters, which makes every \
             paragraph beneath it wrap to the TABLE's width instead of the panel's \
             (`ui-findings.md` C21). Narrow the table, or convert it to a list as \
             `the-concepts` was:\n  {}",
        over.join("\n  "),
    );
}
