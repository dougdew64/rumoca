//! Generated facts for `docs/architecture.md` — the numbers that document used to
//! transcribe by hand.
//!
//! ## The problem this module exists to solve
//!
//! `docs/README.md` marks `architecture.md` 👤 — written for Doug and for Rumoca
//! maintainers — and states in the same table the rule this module enforces:
//! **a 👤 document states facts it does not own by *reference*, never by
//! transcription.** The document was breaking that rule in twenty places.
//!
//! Measured 2026-08-09, on a familiarisation read: **every one of its twenty
//! module line counts was understated, several by more than 3×.** `app.rs` was
//! cited at "~3,850" against a real 12,570; `worker.rs` at "~3,950 lines, the
//! largest module" against 9,921 — and it is no longer the largest. Worse than
//! any count, the pipeline it described had **ten stages and was missing `Dae`**,
//! which was added on 2026-08-03 and never written in, so the document showed the
//! chain jumping Flatten → Structural with the phase they both depend on absent.
//!
//! That is the same failure as the deleted `end_to_end_tour.md`, which asserted a
//! 7×7 incidence matrix on a tab showing 48 equations: **prose carrying a number
//! that nothing checks.** `doc_citations` verifies that cited *paths* resolve; no
//! test in this repository could see a cited *count* drift.
//!
//! ## The shape, and why it is not `CATALOGUE.md`'s
//!
//! `CATALOGUE.md` is generated whole — "**Generated — do not edit**" — and that
//! works because every word of it is derived. `architecture.md` is 1,900 lines of
//! hand-written reasoning with derived numbers sprinkled through it, and
//! regenerating the file would destroy the part worth having. So the derived facts
//! live in **marker-delimited regions** that [`splice`] rewrites in place, and the
//! prose around them is never touched.
//!
//! Everything else follows [`crate::tour::catalogue`]'s pattern exactly, for its
//! reason: the generator lives **here, in the library**, so
//! `architecture_regions_are_current` checks the same code that writes the file
//! rather than a second implementation of it. A checker that reimplements what it
//! checks is the drift `docs/fidelity-plan.md` warns about.
//!
//! ## Runtime reads, where `matching_ledger` chose `include_str!`
//!
//! [`crate::matching_ledger`] embeds the source it describes, so its table
//! describes the code the binary was *compiled against* — right there, because the
//! GUI resolves those line numbers at click time and a stale checkout would send a
//! reader to the wrong line. **Nothing here is read at runtime by the app**: both
//! the generator and its test run out of `CARGO_MANIFEST_DIR`, and what the
//! document should describe is the tree the document lives in. So this module
//! reads at runtime, like `doc_citations`' field-count ratchet does on the same
//! `app.rs`, and the GUI binary carries no megabyte of embedded source it will
//! never look at.
//!
//! ## What is generated, and what is deliberately not
//!
//! Three fact families, each exactly derivable:
//!
//! - [`pipeline_stages`] — read from `StageKind::ALL` *itself*, not from a text
//!   parse of it. The stage roster is a compiled constant; there is no reason to
//!   re-derive it from characters.
//! - [`module_sizes`] — every `.rs` file under `src/` **at any depth** and its
//!   line count, by scanning the tree rather than a list, so a **new module
//!   cannot be silently missing**. It scanned only the top level until
//!   2026-08-20, which is the same silence one level down.
//! - [`app_field_groups`] — the `// ---- N. Title ----` headers inside
//!   `pub struct App`. The document's hand-written list still carried a `Bridge`
//!   group that had been extracted and lacked the `Breakpoint pre-warm` group that
//!   replaced it, while the *count* stayed accidentally correct at 15.
//!
//! **The suite's test count is NOT generated, and that is a decision.** The
//! document claimed "270 tests" in one place and "~411 fast / ~59 slow" in
//! another. A generator can count `#[test]` attributes — 624 of them today — but
//! `cargo test` reports 541, because `#[cfg(…)]` gates some out. Publishing a
//! derived 624 next to a suite that prints 541 replaces one stale number with two
//! live ones that disagree, which is worse. The prose now points at the command;
//! **the suite owns that number.**

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::worker::StageKind;

/// The document this module generates into, relative to `hrw/`.
pub const ARCHITECTURE_DOC: &str = "docs/architecture.md";

/// The command that rewrites the regions.
///
/// Quoted in **every** failure message in this module, so a failing test tells the
/// reader what to run rather than only what went wrong — the same courtesy
/// `tour_catalogue_is_current` pays.
pub const GEN_COMMAND: &str = "cargo run -p hrw --example gen_architecture";

/// A region's body generator. `fn` pointers rather than a name-keyed `match`, so
/// the region list and the generators cannot fall out of step and there is no
/// unreachable "no generator for that name" arm to reason about.
type RegionFn = fn() -> Result<String, String>;

/// Every generated region, by marker name.
///
/// Order is irrelevant — each is found by its own markers — but it is the reading
/// order in the document, which makes a diff easier to follow.
const REGIONS: &[(&str, RegionFn)] = &[
    ("pipeline-stages", pipeline_stages_table),
    ("module-sizes", module_sizes_table),
    ("app-field-groups", app_field_groups_table),
];

/// Minimum number of `.rs` files under `src/` a healthy scan finds.
///
/// **A non-vacuity floor, in the sense `docs/fidelity-plan.md` uses.** Without it
/// a scan that silently returned nothing would emit an empty table, and an empty
/// table is a well-formed table — the failure would read as "this crate has no
/// modules" rather than as "the scan broke". 30 against 55 today: loose enough to
/// survive ordinary deletion, tight enough that a broken read cannot pass.
///
/// **It is a floor, so it only ever sees the count fall.** A scan that stopped
/// finding a whole subdirectory would leave the count too *high*, not too low —
/// `the_scan_recurses_and_keys_rows_by_relative_path` is what covers that.
const MIN_MODULES: usize = 30;

/// Absolute path of `architecture.md` in this checkout.
#[must_use]
pub fn architecture_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(ARCHITECTURE_DOC)
}

/// Rewrite every generated region of `doc`, returning the new text.
///
/// **A missing marker is an `Err`, never a silent no-op**, and that is the whole
/// safety property of this function. A splice that quietly skipped a region it
/// could not find would leave the stale numbers in place and report success — the
/// exact shape of `CLAUDE.md`'s stale-negative trap, where *"a tag that resolves
/// nothing is indistinguishable from a claim that is still true"*. Here it would
/// be worse: the currentness test would then pass on a document the generator had
/// never edited. `splice_fails_when_a_marker_is_missing` holds the property.
pub fn splice(doc: &str) -> Result<String, String> {
    let mut out = doc.to_owned();
    for (name, body) in REGIONS {
        out = replace_region(&out, name, &body()?)?;
    }
    Ok(out)
}

/// Remove every generated region's body, leaving the markers.
///
/// For checks that must look at the **hand-written** prose only. Asserting
/// something about the whole document would be satisfied by the generator's own
/// output, which is the vacuity this avoids.
#[must_use]
pub fn without_generated_regions(doc: &str) -> String {
    let mut out = doc.to_owned();
    for (name, _) in REGIONS {
        // A region that is already absent is not an error here: this function
        // answers "what did a human write?", and a document with no regions at
        // all is entirely hand-written.
        if let Ok(stripped) = replace_region(&out, name, "") {
            out = stripped;
        }
    }
    out
}

fn begin_marker(name: &str) -> String {
    format!("<!-- BEGIN GENERATED {name} -->")
}

fn end_marker(name: &str) -> String {
    format!("<!-- END GENERATED {name} -->")
}

/// Replace the text between one region's markers with `body`.
///
/// HTML comments as markers because Markdown renderers drop them: the document
/// reads the same on GitHub and in HRW's own `egui_commonmark` panel as it would
/// without them.
fn replace_region(doc: &str, name: &str, body: &str) -> Result<String, String> {
    let begin = begin_marker(name);
    let end = end_marker(name);

    let begin_at = doc.find(&begin).ok_or_else(|| {
        format!("{ARCHITECTURE_DOC} has no `{begin}` — add it, then run: {GEN_COMMAND}")
    })?;
    let body_at = begin_at + begin.len();
    let end_at = doc[body_at..]
        .find(&end)
        .map(|i| i + body_at)
        .ok_or_else(|| {
            format!("{ARCHITECTURE_DOC} has `{begin}` with no matching `{end}` after it")
        })?;

    let before = &doc[..body_at];
    let after = &doc[end_at..];
    // One blank-free newline on each side, so an empty body collapses to adjacent
    // markers and a re-splice of spliced output is a fixed point.
    if body.is_empty() {
        Ok(format!("{before}\n{after}"))
    } else {
        Ok(format!("{before}\n{body}\n{after}"))
    }
}

// ---------------------------------------------------------------------------
// Fact family 1 — the pipeline stages
// ---------------------------------------------------------------------------

/// One row of the stage roster: the variant and its three names.
///
/// **Three namings exist and the document never listed them together**, which is
/// how the capture came to emit `"Index reduction"` while `from_slug` accepted
/// only `"IndexReduction"` (see `StageKind::slug`). A table that shows all three
/// side by side makes that class of mismatch visible rather than latent.
pub struct StageRow {
    /// The Rust variant, as `StageKind::Dae` spells it.
    pub variant: String,
    /// Tab label — `StageKind::name`.
    pub label: &'static str,
    /// `hrw://stage/<slug>` — `StageKind::slug`.
    pub slug: &'static str,
    /// Name used in the log — `StageKind::log_name`.
    pub log_name: &'static str,
}

/// The pipeline stages, in order, straight from `StageKind::ALL`.
#[must_use]
pub fn pipeline_stages() -> Vec<StageRow> {
    StageKind::ALL
        .iter()
        .map(|st| StageRow {
            // `Debug` is derived on `StageKind`, so this is the variant's real
            // spelling rather than a fourth hand-maintained list of names.
            variant: format!("{st:?}"),
            label: st.name(),
            slug: st.slug(),
            log_name: st.log_name(),
        })
        .collect()
}

fn pipeline_stages_table() -> Result<String, String> {
    let rows = pipeline_stages();
    let n_all = rows.len();
    let n_compilation = StageKind::COMPILATION.len();

    let mut s = String::new();
    let _ = writeln!(
        s,
        "**{n_all} stages, of which the first {n_compilation} are compilation phases.** \
         `Simulation` is in the roster because it is a tab, but it is not a phase — \
         `StageBundle::get()` panics on it, which is why `StageKind::COMPILATION` exists \
         alongside `StageKind::ALL`.\n"
    );
    let _ = writeln!(
        s,
        "| # | variant | tab label | `hrw://stage/` slug | log name |"
    );
    let _ = writeln!(s, "|---|---|---|---|---|");
    for (i, row) in rows.iter().enumerate() {
        let n = i + 1;
        let _ = writeln!(
            s,
            "| {n} | `{}` | {} | `{}` | {} |",
            row.variant, row.label, row.slug, row.log_name
        );
    }
    Ok(s.trim_end().to_owned())
}

// ---------------------------------------------------------------------------
// Fact family 2 — module sizes
// ---------------------------------------------------------------------------

/// One `.rs` file under `src/` and its length.
pub struct ModuleSize {
    /// Path relative to `src/`, `/`-separated — e.g. `app.rs`, `app/tests.rs`.
    ///
    /// **A relative path rather than a bare file name, and that is the whole
    /// design of this field.** `file_name()` would key a submodule as
    /// `` `tests.rs` `` — ambiguous on sight, and it *collides outright* the
    /// moment a second module grows a `tests.rs` of its own, which is the obvious
    /// next thing to happen. A relative path is unique by construction and reads
    /// as a location.
    pub file: String,
    /// Lines in the file.
    pub lines: usize,
}

/// Every `.rs` file under `src/` **at any depth**, with its line count, largest
/// first.
///
/// **Scanned, not listed.** A hard-coded list would let a new module be silently
/// absent from the table, and absence leaves no gap where the missing thing was —
/// the failure mode `CLAUDE.md` records for the Context Bar, which showed three
/// true things and omitted a fourth for weeks.
///
/// **It had the failure mode it was written to prevent** *(fixed 2026-08-20)*.
/// The scan was one `read_dir` with no `is_dir` branch, so a module in a
/// subdirectory did not exist to a table that prints *"Every file under `src/`"* —
/// scanned rather than listed, and silently incomplete anyway. Nothing in `src/`
/// had ever had a subdirectory, so the bug needed one to exist before it could
/// bite. **`MIN_MODULES` cannot catch this**: the row count fails to *rise*, and
/// a floor only ever sees it fall.
pub fn module_sizes() -> Result<Vec<ModuleSize>, String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let rows = scan_modules(&dir)?;

    if rows.len() < MIN_MODULES {
        return Err(format!(
            "found only {} module(s) under {} — expected at least {MIN_MODULES}; \
             the scan is broken, not the crate",
            rows.len(),
            dir.display(),
        ));
    }

    Ok(rows)
}

/// The scan itself, over an arbitrary root, sorted but with no floor applied.
///
/// Separate from [`module_sizes`] so the recursion and the row key can be tested
/// against a temp tree — the real `src/` is whatever shape it happens to be today,
/// and a test that only passes once a subdirectory exists is not a test of the
/// scan.
///
/// **No skip list, unlike `doc_citations::rust_sources`.** That one walks
/// `crates/` and must dodge `target/`; this walks `src/` only, where the
/// extension filter is the entire guard needed (`field_help.json` is the one
/// non-`.rs` file there).
fn scan_modules(root: &Path) -> Result<Vec<ModuleSize>, String> {
    let mut rows: Vec<ModuleSize> = Vec::new();
    walk_modules(root, root, &mut rows)?;

    // Largest first, then alphabetical **by relative path**, so the ordering is
    // total and the file is byte-stable across runs and platforms.
    rows.sort_by(|a, b| b.lines.cmp(&a.lines).then_with(|| a.file.cmp(&b.file)));
    Ok(rows)
}

/// One directory level, recursing into subdirectories. Errors rather than
/// skipping, since an unreadable entry under `src/` means the scan broke.
fn walk_modules(root: &Path, dir: &Path, rows: &mut Vec<ModuleSize>) -> Result<(), String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("cannot read an entry of {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            walk_modules(root, &path, rows)?;
            continue;
        }
        if path.extension().and_then(|x| x.to_str()) != Some("rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        rows.push(ModuleSize {
            lines: text.lines().count(),
            file: relative_key(root, &path)?,
        });
    }
    Ok(())
}

/// `…/src/app/tests.rs` under `…/src` → `app/tests.rs`.
///
/// **`/` on every platform, and that is not cosmetic.** The key is printed into a
/// committed document that a test requires to be byte-identical to a fresh
/// generation; `std::path`'s separator is `\` on Windows, so joining components
/// with it would make the table differ by the machine that ran the generator.
fn relative_key(root: &Path, path: &Path) -> Result<String, String> {
    let rel = path
        .strip_prefix(root)
        .map_err(|_| format!("{} is not under {}", path.display(), root.display()))?;
    let mut parts: Vec<&str> = Vec::new();
    for component in rel.components() {
        parts.push(
            component
                .as_os_str()
                .to_str()
                .ok_or_else(|| format!("{} has a non-UTF-8 path component", path.display()))?,
        );
    }
    Ok(parts.join("/"))
}

fn module_sizes_table() -> Result<String, String> {
    let rows = module_sizes()?;
    let total: usize = rows.iter().map(|r| r.lines).sum();
    let n = rows.len();

    let mut s = String::new();
    let _ = writeln!(
        s,
        "**{n} modules, {} lines**, largest first. Every `.rs` file under `src/` at any \
         depth, including the test-only ones (`ui_tests.rs`, `test_support.rs`); a module \
         in a subdirectory is keyed by its path relative to `src/`.\n",
        thousands(total)
    );
    let _ = writeln!(s, "| module | lines |");
    let _ = writeln!(s, "|---|---:|");
    for row in &rows {
        let _ = writeln!(s, "| `{}` | {} |", row.file, thousands(row.lines));
    }
    let _ = writeln!(s, "| **total** | **{}** |", thousands(total));
    Ok(s.trim_end().to_owned())
}

/// `12570` → `"12,570"`. Grouping only; no locale, no rounding.
fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        // Comma before every digit whose distance from the end is a multiple of 3.
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Fact family 3 — the `App` field groups
// ---------------------------------------------------------------------------

/// One `// ---- N. Title ----` header inside `pub struct App`.
pub struct FieldGroup {
    /// The header's number, which is **not** its position: the numbering has gaps
    /// where a group was extracted into its own state struct.
    pub number: usize,
    /// The header's title.
    pub title: String,
}

/// The `App` struct's field-group headers, in declaration order.
///
/// Parsed out of `app.rs` because there is nothing else to read — the groups are
/// comments, not code. **The struct is delimited exactly as
/// `doc_citations::app_does_not_regrow_its_field_count` delimits it**, on
/// `"\npub struct App {"` and the closing `"\n}\n"`, so the two agree about what
/// the struct is; `the_working_tree_is_checked_out_with_lf_endings` is what keeps
/// that split honest under CRLF.
pub fn app_field_groups() -> Result<Vec<FieldGroup>, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app.rs");
    let src = std::fs::read_to_string(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;

    let body = src
        .split_once("\npub struct App {")
        .ok_or_else(|| "app.rs must declare `pub struct App`".to_owned())?
        .1
        .split_once("\n}\n")
        .ok_or_else(|| {
            "the App struct must be closed by a `}` at column 0 (or the tree is CRLF — \
             see doc_citations::the_working_tree_is_checked_out_with_lf_endings)"
                .to_owned()
        })?
        .0;

    let groups: Vec<FieldGroup> = body
        .lines()
        .filter_map(|line| {
            let header = line
                .trim()
                .strip_prefix("// ---- ")?
                .strip_suffix(" ----")?;
            let (num, title) = header.split_once(". ")?;
            Some(FieldGroup {
                number: num.trim().parse().ok()?,
                title: title.trim().to_owned(),
            })
        })
        .collect();

    if groups.is_empty() {
        return Err(format!(
            "found no `// ---- N. Title ----` group headers in {} — the convention moved, \
             so this generator describes a structure that no longer exists",
            path.display(),
        ));
    }
    Ok(groups)
}

fn app_field_groups_table() -> Result<String, String> {
    let groups = app_field_groups()?;
    let n = groups.len();

    // Gaps are a fact about the struct, so they are reported rather than tidied
    // away: each one is a group that was extracted into its own state struct
    // during the UI pause, and a reader comparing this table with the source
    // would otherwise wonder which of the two had lost a row.
    let highest = groups.iter().map(|g| g.number).max().unwrap_or(0);
    let missing: Vec<String> = (1..=highest)
        .filter(|i| !groups.iter().any(|g| g.number == *i))
        .map(|i| i.to_string())
        .collect();

    let mut s = String::new();
    if missing.is_empty() {
        let _ = writeln!(s, "**{n} groups**, numbered 1–{highest}.\n");
    } else {
        let _ = writeln!(
            s,
            "**{n} groups**, numbered 1–{highest} with {} unused — a number is retired when \
             its group is extracted into its own state struct, and the surviving numbers stay \
             put so they keep matching the comments in `app.rs`.\n",
            if missing.len() == 1 {
                format!("**{}**", missing[0])
            } else {
                format!("**{}**", missing.join(", "))
            }
        );
    }
    let _ = writeln!(s, "| # | group |");
    let _ = writeln!(s, "|---|---|");
    for g in &groups {
        let _ = writeln!(s, "| {} | {} |", g.number, g.title);
    }
    Ok(s.trim_end().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The generated regions of `architecture.md` are current.**
    ///
    /// The same generate-and-compare shape as `app::tests::tour_catalogue_is_current`
    /// and `matching_ledger`'s reference check, and it calls the same [`splice`] the
    /// example calls rather than a second implementation of it.
    ///
    /// **Every field in these regions is derived, so "stale" only ever means "not
    /// regenerated"** — which is exactly what this catches. Adding a pipeline stage,
    /// adding a module, or renaming an `App` field group fails here with the command
    /// to run.
    #[test]
    fn architecture_regions_are_current() {
        let path = architecture_path();
        let on_disk = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let fresh = splice(&on_disk).expect("splice must succeed");
        assert_eq!(
            on_disk, fresh,
            "{ARCHITECTURE_DOC} is out of date \u{2014} run: {GEN_COMMAND}",
        );
    }

    /// **A missing marker fails loudly.** The must-fire rule pointed at silence;
    /// this is it pointed at a splice that finds nothing to do.
    ///
    /// Without this property a renamed or deleted marker would leave the stale
    /// numbers in place, `splice` would report success, and
    /// `architecture_regions_are_current` would then **pass on a document the
    /// generator had never edited** — a green result covering nothing.
    #[test]
    fn splice_fails_when_a_marker_is_missing() {
        let err = splice("# Architecture\n\nNo markers here at all.\n")
            .expect_err("a document with no markers must be an error, not a no-op");
        assert!(
            err.contains(GEN_COMMAND),
            "the failure must name the command to run, got: {err}"
        );

        // A begin with no end is the other half, and it is the more likely typo.
        //
        // Exercised against `replace_region` rather than `splice`, because `splice`
        // checks the regions in order and would fail on the *first* one missing
        // entirely — never reaching the unterminated one. Asserting through `splice`
        // here passed for the wrong reason until the message was read.
        let half = format!("{}\nbody\n", begin_marker("module-sizes"));
        let err = replace_region(&half, "module-sizes", "new")
            .expect_err("an unterminated region must be an error");
        assert!(
            err.contains("no matching"),
            "the failure must say the end marker is missing, got: {err}"
        );
    }

    /// **Splicing twice changes nothing.** A generator whose output is not a fixed
    /// point would make every run a diff, and `architecture_regions_are_current`
    /// could never pass twice in a row.
    #[test]
    fn splice_is_idempotent() {
        let doc = std::fs::read_to_string(architecture_path()).expect("read architecture.md");
        let once = splice(&doc).expect("first splice");
        let twice = splice(&once).expect("second splice");
        assert_eq!(once, twice, "splice must be a fixed point");
    }

    /// **Every pipeline stage is named in the hand-written prose**, not only in the
    /// generated table.
    ///
    /// This is the test that would have caught the defect that prompted the module:
    /// `Dae` joined `StageKind::ALL` on 2026-08-03 and `architecture.md` went on
    /// describing a ten-stage pipeline that jumped straight from Flatten to
    /// Structural.
    ///
    /// **It reads the document with the generated regions stripped**, because
    /// asserting over the whole file would be satisfied by the generator's own
    /// output — the vacuity `docs/fidelity-plan.md` warns about, where a check
    /// passes by looking at the thing it just wrote.
    ///
    /// **The honest bound:** `log_name` is the most prose-like of the three
    /// namings, so this is a strong check for the compound names (`DAE
    /// construction`, `Structural analysis`, `Solve lowering`) and a weak one for
    /// the short generic ones — a document could contain the word "Events" without
    /// documenting the Events stage. It catches an *undocumented new stage*, which
    /// is the failure that actually happened; it does not certify that each stage
    /// is well described.
    #[test]
    fn every_pipeline_stage_is_named_in_the_hand_written_prose() {
        let doc = std::fs::read_to_string(architecture_path()).expect("read architecture.md");
        let prose = without_generated_regions(&doc);
        assert!(
            prose.len() * 2 > doc.len(),
            "stripping the generated regions removed most of the document — \
             the markers or the stripper are wrong, and this check would be vacuous"
        );
        for st in StageKind::ALL {
            assert!(
                prose.contains(st.log_name()),
                "{ARCHITECTURE_DOC} never names the `{st:?}` stage (\"{}\") outside its \
                 generated tables",
                st.log_name(),
            );
        }
    }

    /// The stage roster comes from the constant, so it agrees with it by
    /// construction — this pins the two facts the table states *about* it.
    #[test]
    fn the_stage_roster_matches_stagekind() {
        let rows = pipeline_stages();
        assert_eq!(rows.len(), StageKind::ALL.len());
        assert_eq!(
            StageKind::COMPILATION.len() + 1,
            StageKind::ALL.len(),
            "the table claims COMPILATION is ALL without exactly one stage"
        );
        assert!(
            rows.iter().any(|r| r.variant == "Dae"),
            "the stage that went undocumented must be in the roster"
        );
    }

    /// A module in a **subdirectory** appears, keyed by its path relative to
    /// `src/`.
    ///
    /// **The test that would have caught the scan's own bug.** Until 2026-08-20
    /// [`module_sizes`] was a single `read_dir` with no `is_dir` branch, so a
    /// module one level down did not exist to a table that prints *"Every file
    /// under `src/`"*. `MIN_MODULES` could not see it — a floor watches the count
    /// *fall*, and this failure makes it fail to *rise*, the same shape as
    /// `recorded_animation_reports_no_live_session`.
    ///
    /// **Against a temp tree, not the real `src/`**, so it pins the scan's
    /// behaviour rather than today's layout — a test that only becomes meaningful
    /// once someone happens to create a subdirectory is not a guard.
    ///
    /// **The equal-line pair is doing work.** `bbb.rs` and `zz/aaa.rs` are both
    /// one line, so the ordering falls to the alphabetical tiebreak: by relative
    /// path `bbb.rs` precedes `zz/aaa.rs`, by bare file name `aaa.rs` would
    /// precede it. **One assertion therefore fires on all three regressions** —
    /// dropping the recursion, keying on `file_name()`, and tiebreaking on the
    /// old key.
    #[test]
    fn the_scan_recurses_and_keys_rows_by_relative_path() {
        let root = std::env::temp_dir().join("hrw-module-scan-test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("zz")).unwrap();
        std::fs::write(root.join("app.rs"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(root.join("bbb.rs"), "one\n").unwrap();
        std::fs::write(root.join("zz").join("aaa.rs"), "one\n").unwrap();
        // A non-`.rs` file, as `src/field_help.json` is: not a module.
        std::fs::write(root.join("field_help.json"), "{}\n").unwrap();

        let rows = scan_modules(&root).expect("scan the temp tree");
        let keys: Vec<&str> = rows.iter().map(|r| r.file.as_str()).collect();
        assert_eq!(
            keys,
            ["app.rs", "bbb.rs", "zz/aaa.rs"],
            "the scan must recurse, key rows by `/`-separated relative path, and \
             tiebreak on that key"
        );
        assert_eq!(
            rows.iter().map(|r| r.lines).collect::<Vec<_>>(),
            [3, 1, 1],
            "line counts must come from the file the key names"
        );
    }

    /// The module scan finds real files with real lengths, and the floor holds.
    #[test]
    fn module_sizes_are_scanned_and_ordered() {
        let rows = module_sizes().expect("scan src/");
        assert!(rows.len() >= MIN_MODULES, "the non-vacuity floor must hold");
        assert!(
            rows.iter().all(|r| r.lines > 0),
            "a zero-line module means the read, not the file"
        );
        assert!(
            rows.windows(2).all(|w| w[0].lines >= w[1].lines),
            "rows must be largest-first for the table to be stable"
        );
        let app = rows
            .iter()
            .find(|r| r.file == "app.rs")
            .expect("app.rs must be found");
        assert!(
            app.lines > 5_000,
            "app.rs is a five-figure file; {} suggests a truncated read",
            app.lines
        );
    }

    /// The field-group parse finds the headers, and reports the numbering gap
    /// rather than renumbering around it.
    #[test]
    fn app_field_groups_are_parsed_with_their_numbering() {
        let groups = app_field_groups().expect("parse app.rs");
        assert!(groups.len() >= 10, "found only {} groups", groups.len());
        assert_eq!(
            groups.first().map(|g| g.number),
            Some(1),
            "the first group is numbered 1"
        );
        assert!(
            groups.iter().all(|g| !g.title.is_empty()),
            "a group header with no title means the parse, not the source"
        );
        // Numbers are strictly increasing even where they skip.
        assert!(
            groups.windows(2).all(|w| w[0].number < w[1].number),
            "group numbers must ascend in declaration order"
        );
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(7), "7");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1_000), "1,000");
        assert_eq!(thousands(12_570), "12,570");
        assert_eq!(thousands(50_414), "50,414");
        assert_eq!(thousands(1_234_567), "1,234,567");
    }
}
