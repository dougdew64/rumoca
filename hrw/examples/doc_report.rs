//! **Nightly document maintenance: sizes, growth, duplication — and when to wake Doug.**
//!
//! ```text
//! cargo run -p hrw --example doc_report              # report only
//! cargo run -p hrw --example doc_report -- --since HEAD~20   # growth against a commit
//! ```
//!
//! # Why this replaced a per-commit budget
//!
//! Doug, 2026-08-31: *"rather than fighting budget battles several times during a workday,
//! perhaps we can do document clean-up every night during that night's unattended run. And,
//! if during a document cleanup you determine that we're about to hit a document wall, then
//! we can pause and work together to trim documents."*
//!
//! The ratchet it replaces charged fifteen tolls in one day and rejected nothing. This
//! moves the same watching to a time when nobody is waiting, and — more importantly —
//! **watches the variable that actually failed.** The growth that prompted all of this was
//! not a document getting long; it was **the same prose in two files.** A line counter
//! catches that as a symptom and bills for it. A duplication check catches the thing.
//!
//! # What it may do, and what it must bring back
//!
//! `CLAUDE.md` already authorises Claude to reorganise and condense documents without
//! asking. The line this must not cross is the same one that governs lab prose: **trimming
//! an explanation is Doug's learning material and therefore Doug's call.**
//!
//! | unattended | bring to Doug |
//! |---|---|
//! | retire closed history to `DECISIONS.md` | trim or reword an explanation |
//! | delete a duplicated passage, keeping one | restructure a README |
//! | fix a stale claim | anything crossing a ceiling |
//!
//! # The exit code is the escalation
//!
//! **0** — reported, nothing needed. **1** — a ceiling is crossed or duplication is above
//! the reporting floor, so the night's log should say so and the morning should start here.
//! It never edits: a sweep that both decides and acts, unwatched, on documents whose value
//! is judgement, is precisely what Doug reserved to himself.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn hrw_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// A ceiling from `docs/reading-budgets.txt`, or a loud failure.
///
/// Duplicated from `doc_citations`'s private reader rather than shared, deliberately: that
/// one is `#[cfg(test)]` and reachable by no example. **The file is the single source**, so
/// the two readers cannot disagree about a value — only about parsing, which the format
/// makes hard to get differently wrong.
fn ceiling(name: &str) -> usize {
    let path = hrw_dir().join("docs/reading-budgets.txt");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some((key, value)) = line.split_once('=')
            && key.trim() == name
        {
            return value
                .trim()
                .parse()
                .unwrap_or_else(|e| panic!("{name} is not a number ({e})"));
        }
    }
    panic!("{} defines no ceiling named `{name}`", path.display());
}

/// Paragraphs of at least this many characters are compared across documents.
///
/// Short paragraphs repeat innocently — a heading, a one-line rule, a shared quote — and
/// reporting those would bury the finding that matters. 400 characters is roughly a
/// four-line paragraph: long enough that appearing twice is an authoring decision rather
/// than a coincidence.
const DUPLICATE_FLOOR: usize = 400;

/// Every markdown file worth comparing, relative to `hrw/`.
///
/// **Wider than the reading paths on purpose.** Duplication between `DECISIONS.md` and a
/// README is exactly the case that prompted this, and neither `DECISIONS.md` nor
/// `tech-debt.md` is on any reading path.
fn corpus(hrw: &Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut push = |rel: &str| {
        if let Ok(text) = std::fs::read_to_string(hrw.join(rel)) {
            out.push((rel.to_owned(), text));
        }
    };
    for rel in hrw::doc_sizes::MANDATORY {
        push(rel);
    }
    for rel in hrw::doc_sizes::CONDITIONAL {
        push(rel);
    }
    for rel in ["DECISIONS.md", "docs/tech-debt.md", "docs/vision.md"] {
        push(rel);
    }
    out
}

/// Blank-line-separated paragraphs, normalised so that a re-wrap is not a difference.
fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| p.len() >= DUPLICATE_FLOOR)
        .collect()
}

fn main() {
    let hrw = hrw_dir();
    let mut escalate = false;

    println!("\nHRW document report  --  {}\n", hrw.display());

    // ---------------------------------------------------------------- sizes --
    println!("  Reading paths (characters; ceilings are NOT targets)");
    let mandatory_ceiling = ceiling("mandatory");
    let mut total = 0usize;
    for rel in hrw::doc_sizes::MANDATORY {
        let n = hrw::doc_sizes::chars_of(&hrw, rel);
        total += n;
        println!("    {rel:<34} {n:>7}");
    }
    let pct = total * 100 / mandatory_ceiling;
    println!(
        "    {:<34} {total:>7}  {pct}% of ceiling",
        "mandatory TOTAL"
    );
    if total > mandatory_ceiling {
        escalate = true;
        println!("    ^^ OVER. Restructuring is Doug's call, not a raise.");
    }

    let cw = hrw::doc_sizes::current_work_chars(&hrw);
    let cw_ceiling = ceiling("current_work");
    println!(
        "    {:<34} {cw:>7}  {}% of ceiling",
        "CLAUDE.md ## Current work",
        cw * 100 / cw_ceiling
    );
    if cw > cw_ceiling {
        escalate = true;
        println!("    ^^ OVER \u{2014} a closed arc is being restated. DECISIONS.md holds those.");
    }

    let cond_ceiling = ceiling("conditional");
    for rel in hrw::doc_sizes::CONDITIONAL {
        let n = hrw::doc_sizes::chars_of(&hrw, rel);
        println!(
            "    {rel:<34} {n:>7}  {}% of ceiling",
            n * 100 / cond_ceiling
        );
        if n > cond_ceiling {
            escalate = true;
            println!(
                "    ^^ OVER \u{2014} past this it is consulted, not read. It wants splitting."
            );
        }
    }

    // ---------------------------------------------------- duplication --
    //
    // **The check that would have caught the real failure.** On 2026-08-31 four rulings
    // were written into both `DECISIONS.md` and `fixture-labs/README.md`; the budget saw
    // only "a document got longer" and charged for it seven times.
    println!("\n  Duplicated passages (>= {DUPLICATE_FLOOR} chars, across documents)");
    let docs = corpus(&hrw);
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    for (name, text) in &docs {
        for p in paragraphs(text) {
            let owners = seen.entry(p).or_default();
            if !owners.contains(name) {
                owners.push(name.clone());
            }
        }
    }
    let mut dupes: Vec<(&String, &Vec<String>)> =
        seen.iter().filter(|(_, o)| o.len() > 1).collect();
    dupes.sort_by_key(|(p, _)| std::cmp::Reverse(p.len()));
    if dupes.is_empty() {
        println!("    none");
    } else {
        escalate = true;
        for (p, owners) in dupes.iter().take(10) {
            let head: String = p.chars().take(72).collect();
            println!(
                "    {} chars in {}\n      \"{head}...\"",
                p.len(),
                owners.join(" + ")
            );
        }
        println!(
            "\n    Keep ONE copy. The account belongs in DECISIONS.md; a reading-path \n\
             \x20   document states the rule and points at it."
        );
    }

    // ------------------------------------------------------------- verdict --
    println!();
    if escalate {
        println!(
            "Escalate. Nothing was edited \u{2014} deduplicating and retiring closed history\n\
             is unattended work, but trimming an explanation is Doug's call."
        );
        std::process::exit(1);
    }
    println!("Documents are within their ceilings and carry no cross-document duplication.");
}
