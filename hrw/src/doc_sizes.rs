//! Which documents are on a reading path, and how big they are.
//!
//! # Why this is a module rather than two constants in a test
//!
//! The roster is needed in two places that cannot share a `const`: the ceiling check in
//! `doc_citations`, and `examples/doc_report`, which runs nightly. An example can only
//! reach `pub` items, and a roster copied into both would drift — the failure this
//! repository has recorded under a dozen names.
//!
//! It is also the `gate_policy` pattern applied again: **the rule lives where a test can
//! reach it**, and the runner keeps only what talks to the outside world.
//!
//! # Characters, not lines
//!
//! Measured 2026-08-31, chars per line across the mandatory path: `CLAUDE.md` 68,
//! `docs/README.md` 75, `docs/CHARTER.md` **182**. A line budget therefore treated 164
//! lines of `CHARTER.md` as cheaper than 149 of `docs/README.md`, which is backwards by
//! nearly three to one. Characters approximate tokens, and tokens are the cost paid.

use std::path::Path;

/// Every file a session must read before its first action, relative to `hrw/`.
///
/// **Not total markdown**, which is ~41,000 lines. `ideas.md` and `DECISIONS.md` are
/// *consulted* — they cost a grep, not context — so pruning them would not move this
/// number and they are deliberately uncounted.
pub const MANDATORY: &[&str] = &[
    "CLAUDE.md",
    "docs/working-with-doug.md",
    "docs/CHARTER.md",
    "docs/README.md",
];

/// Read before **one kind** of work, so mandatory when it matters, and checked per file
/// rather than summed.
///
/// `fixture-labs/README.md` is read before lab work; `unattended-runs.md` before any
/// work done while Doug is asleep — the one document whose reader has nobody to ask.
pub const CONDITIONAL: &[&str] = &["docs/fixture-labs/README.md", "docs/unattended-runs.md"];

/// Size of a tracked document in characters.
///
/// Panics rather than returning zero for a missing file: a reading-path entry that does
/// not resolve is a broken roster, and a silent zero would let every ceiling pass.
#[must_use]
pub fn chars_of(hrw: &Path, rel: &str) -> usize {
    std::fs::read_to_string(hrw.join(rel))
        .unwrap_or_else(|e| panic!("{rel} is on a reading path and must be readable: {e}"))
        .len()
}

/// Size of `CLAUDE.md`'s `## Current work` section, in characters.
///
/// Located by its heading and the next `## `, rather than by line numbers, so renaming a
/// later section cannot silently shrink what is measured. Returns 0 if the heading is
/// absent, which callers must treat as a failure to locate rather than as an empty
/// section.
#[must_use]
pub fn current_work_chars(hrw: &Path) -> usize {
    let Ok(text) = std::fs::read_to_string(hrw.join("CLAUDE.md")) else {
        return 0;
    };
    let mut in_section = false;
    let mut n = 0usize;
    for line in text.lines() {
        if line.starts_with("## Current work") {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with("## ") {
            break;
        }
        if in_section {
            n += line.len() + 1;
        }
    }
    n
}
