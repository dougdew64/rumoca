//! Generated reference for stepping the matching search under a debugger.
//!
//! ## Why this module exists
//!
//! `docs/ideas.md` #73's live-trace tours quote three things from
//! `crates/rumoca-phase-structural/src/matching.rs`: **which line emits which
//! `MatchingStep`**, **the frame-by-frame ledger for a specimen**, and **the
//! recursion depth at each frame**. `CLAUDE.md` already warns that tours quoting
//! line numbers "go stale silently, and a learner following one with wrong line
//! numbers is simply confused" — nothing compiles a Markdown table.
//!
//! Two of those three were established by Doug walking a debugger for an hour
//! per specimen. **All three are derivable without one**, and this module
//! derives them:
//!
//! - [`emit_sites`] reads the emit line for each step variant **out of the
//!   source**, so the table cannot disagree with the code it describes.
//! - [`derive_depths`] recovers the call depth from the step sequence alone —
//!   `TryDisplace` descends, `DisplaceOk`/`DisplaceFail` return.
//! - [`ledger`] runs the real traced algorithm over a specimen's recorded
//!   incidence and renders the whole run.
//!
//! ## What it does NOT replace
//!
//! Everything the walks discovered about the **instrument** rather than the
//! algorithm: that an anchor stop exposes only `frame_index`, that `Option`
//! payloads are invisible one level down, that `var`/`iter` reading unavailable
//! means the loop *ended*, and the frame-delay paint race. None of that is in
//! the source, and none of it is derivable here. Neither is the check that
//! matters most — **whether a tour's promised rhythm survives contact with a
//! human**, which `#73` requires before Act 5 ships.
//!
//! **The error this would have caught**: `EquationFailed` was stated as
//! `matching.rs:137` for a day. 137 is where the variant is *named*; 133 is the
//! `emit_matching_frame(` call the stack reports. Read by hand from the wrong
//! line of the same call, in the one row never observed.

use std::collections::{BTreeMap, HashSet};

use rumoca_phase_structural::matching::{MatchingStep, maximum_matching_with_trace};

/// The traced algorithm's source, read at build time from the sibling crate.
///
/// `include_str!` rather than a runtime read: the table then describes the
/// source this binary was **compiled against**, so a stale checkout cannot
/// produce a table that looks current.
const MATCHING_SOURCE: &str = include_str!("../../crates/rumoca-phase-structural/src/matching.rs");

/// Where the generated reference lives, relative to the crate manifest.
pub const REFERENCE_PATH: &str =
    "docs/compiler-phases/phase7_structural_analysis/matching-live-reference.md";

/// Specimens rendered into the reference, and why each earns its place.
///
/// One that succeeds and one that fails, because the pair is the teaching
/// content: both reach a displacement at depth 2, and only one of them ends at
/// a free variable (`docs/ideas.md` #73).
const LEDGER_SPECIMENS: &[(&str, &str)] = &[
    ("ProportionalLoop", "succeeds — the displacement finds a home"),
    ("TwiceDefined", "fails — the displacement has nowhere to go"),
];

/// Strip a trailing `//` line comment so commented-out parens cannot unbalance
/// the scan. `matching.rs` has no string literal containing `//`.
fn strip_comment(line: &str) -> &str {
    match line.find("//") {
        Some(i) => &line[..i],
        None => line,
    }
}

/// Every `MatchingStep::Variant` named in `seg`, in order.
fn variants_in(seg: &str) -> Vec<String> {
    const MARKER: &str = "MatchingStep::";
    let mut out = Vec::new();
    let mut rest = seg;
    while let Some(i) = rest.find(MARKER) {
        let tail = &rest[i + MARKER.len()..];
        let end = tail
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(tail.len());
        if end > 0 {
            out.push(tail[..end].to_owned());
        }
        rest = &tail[end..];
    }
    out
}

/// Map each `MatchingStep` variant to the 1-based line of the
/// `emit_matching_frame(` call that emits it.
///
/// **The call line, not the line naming the variant.** A stack frame reports
/// where the call was made, so that is the number a debugger shows and the only
/// one worth quoting in a tour.
///
/// A variant can map to more than one line, and one does:
/// `DisplaceOk`/`DisplaceFail` share a single emit whose `step:` is an `if`
/// expression. That is why the value is a `Vec` — collapsing it to one line
/// would hide that **the emit site does not determine the outcome**.
///
/// Scanning stops at `#[cfg(test)]`, so test fixtures cannot contribute rows.
#[must_use]
pub fn emit_sites(source: &str) -> BTreeMap<String, Vec<usize>> {
    let mut sites: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    let mut call_line: Option<usize> = None;
    let mut balance: i32 = 0;

    for (idx, raw) in source.lines().enumerate() {
        if raw.trim_start().starts_with("#[cfg(test)]") {
            break;
        }
        let line = strip_comment(raw);

        // Start of a call: begin counting from its opening paren.
        let mut from = 0;
        if call_line.is_none()
            && let Some(pos) = line.find("emit_matching_frame(")
        {
            call_line = Some(idx + 1);
            balance = 0;
            from = pos + "emit_matching_frame".len();
        }

        let Some(line_of_call) = call_line else {
            continue;
        };
        let seg = &line[from.min(line.len())..];

        for v in variants_in(seg) {
            let lines = sites.entry(v).or_default();
            if !lines.contains(&line_of_call) {
                lines.push(line_of_call);
            }
        }

        balance += i32::try_from(seg.matches('(').count()).unwrap_or(0);
        balance -= i32::try_from(seg.matches(')').count()).unwrap_or(0);
        if balance <= 0 {
            call_line = None;
        }
    }
    sites
}

/// Recursion depth at each frame — the number of `augment_traced` frames on the
/// stack when it was emitted.
///
/// **Derivable from the step sequence alone**, which is what makes the ledger a
/// generated artifact rather than a debugging session:
///
/// - `TryEquation` is emitted by the driver, *outside* `augment_traced` — depth
///   0 — and the call it precedes runs at depth 1.
/// - `TryDisplace` is emitted *before* the recursive call, so it sits at the
///   current depth and the frames after it are one deeper.
/// - `DisplaceOk`/`DisplaceFail` are emitted *after* that call returned, so the
///   depth has already come back up.
/// - `EquationFailed` is the driver again — depth 0.
///
/// Pinned against both debugger walks by
/// `the_derived_depths_match_what_the_debugger_showed`.
#[must_use]
pub fn derive_depths(steps: &[MatchingStep]) -> Vec<usize> {
    let mut out = Vec::with_capacity(steps.len());
    let mut depth = 0usize;
    for step in steps {
        match step {
            MatchingStep::TryEquation(_) => {
                out.push(0);
                depth = 1;
            }
            MatchingStep::EquationFailed(_) => {
                depth = 0;
                out.push(0);
            }
            MatchingStep::TryDisplace { .. } => {
                out.push(depth);
                depth += 1;
            }
            MatchingStep::DisplaceOk { .. } | MatchingStep::DisplaceFail { .. } => {
                depth = depth.saturating_sub(1);
                out.push(depth);
            }
            _ => out.push(depth),
        }
    }
    out
}

/// A specimen's incidence, read from its generated notebook trace.
///
/// `CLAUDE.md`: the notebook trace is "generated and therefore correct by
/// construction — any number about a specimen is read from here". Reading it
/// avoids compiling against the MSL, so this stays in the fast suite.
fn incidence_of(model: &str) -> Option<(usize, usize, Vec<HashSet<usize>>)> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/specimen-notebook")
        .join(model)
        .join("trace/structural.json");
    let text = std::fs::read_to_string(path).ok()?;
    let doc: serde_json::Value = serde_json::from_str(&text).ok()?;
    let inc = doc.get("incidence")?;
    let n_var = usize::try_from(inc.get("n_var")?.as_u64()?).ok()?;
    let rows = inc.get("rows")?.as_array()?;
    let eq_vars: Vec<HashSet<usize>> = rows
        .iter()
        .map(|r| {
            r.get("unknowns")
                .and_then(|u| u.as_array())
                .map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as usize).collect())
                .unwrap_or_default()
        })
        .collect();
    Some((eq_vars.len(), n_var, eq_vars))
}

/// One row per frame: index, step, emit line, depth.
fn ledger_rows(model: &str) -> Option<Vec<(usize, String, String, usize)>> {
    let (n_eq, n_var, eq_vars) = incidence_of(model)?;
    let result = maximum_matching_with_trace(n_eq, n_var, &eq_vars, None);
    let steps: Vec<MatchingStep> = result.frames.iter().map(|f| f.step.clone()).collect();
    let depths = derive_depths(&steps);
    let sites = emit_sites(MATCHING_SOURCE);

    Some(
        steps
            .iter()
            .zip(depths)
            .enumerate()
            .map(|(i, (step, depth))| {
                let name = variant_name(step);
                let line = sites
                    .get(name)
                    .map(|ls| {
                        ls.iter()
                            .map(ToString::to_string)
                            .collect::<Vec<_>>()
                            .join(" / ")
                    })
                    // Absence is stated: a variant with no emit site found is a
                    // scanner failure, and printing a plausible number would be
                    // the exact fiction this file exists to prevent.
                    .unwrap_or_else(|| "NOT FOUND".to_owned());
                (i, describe(step), line, depth)
            })
            .collect(),
    )
}

/// The variant's bare name, for looking up its emit site.
fn variant_name(step: &MatchingStep) -> &'static str {
    match step {
        MatchingStep::TryEquation(_) => "TryEquation",
        MatchingStep::Explore { .. } => "Explore",
        MatchingStep::FoundFree { .. } => "FoundFree",
        MatchingStep::TryDisplace { .. } => "TryDisplace",
        MatchingStep::DisplaceOk { .. } => "DisplaceOk",
        MatchingStep::DisplaceFail { .. } => "DisplaceFail",
        MatchingStep::Assign { .. } => "Assign",
        MatchingStep::EquationFailed(_) => "EquationFailed",
    }
}

/// Human-readable step with its operands — what the debugger's `frames` local
/// shows, spelled out.
fn describe(step: &MatchingStep) -> String {
    match step {
        MatchingStep::TryEquation(eq) => format!("`TryEquation({eq})`"),
        MatchingStep::Explore { eq, var } => format!("`Explore {{ eq: {eq}, var: {var} }}`"),
        MatchingStep::FoundFree { eq, var } => format!("`FoundFree {{ eq: {eq}, var: {var} }}`"),
        MatchingStep::TryDisplace { eq, var, holder } => {
            format!("`TryDisplace {{ eq: {eq}, var: {var}, holder: {holder} }}`")
        }
        MatchingStep::DisplaceOk { eq, var } => format!("`DisplaceOk {{ eq: {eq}, var: {var} }}`"),
        MatchingStep::DisplaceFail { eq, var } => {
            format!("`DisplaceFail {{ eq: {eq}, var: {var} }}`")
        }
        MatchingStep::Assign { eq, var } => format!("`Assign {{ eq: {eq}, var: {var} }}`"),
        MatchingStep::EquationFailed(eq) => format!("`EquationFailed({eq})`"),
    }
}

/// The whole generated reference document.
///
/// Written by `examples/gen_matching_reference.rs` and compared against disk by
/// `matching_ledger::tests::the_generated_reference_is_current`, the same
/// generate/compare shape as `tour::catalogue`.
#[must_use]
pub fn reference() -> String {
    let mut out = String::new();
    out.push_str("# Matching — live-trace reference (GENERATED)\n\n");
    out.push_str(
        "*Generated by `cargo run -p hrw --example gen_matching_reference`. \
         **Do not edit by hand** — `matching_ledger.rs` derives every number here \
         from `crates/rumoca-phase-structural/src/matching.rs` and from the \
         specimens' notebook traces, so the tables cannot drift from the code \
         they describe.*\n\n",
    );
    out.push_str(
        "The prose that interprets these numbers lives in \
         [`maximum_bipartite_matching.md`](maximum_bipartite_matching.md) and \
         `docs/ideas.md` #73. **This file is the numbers only**, so that fixing a \
         stale line number never means editing an argument.\n\n",
    );

    out.push_str("## Emit sites\n\n");
    out.push_str(
        "Where each `MatchingStep` is pushed. **This is the line a debugger \
         reports for the calling frame**, which is what makes it the stop's \
         identity: at the `live_trace_breakpoint` anchor every stop looks \
         identical, and the caller's line is the only discriminator.\n\n",
    );
    out.push_str("| step | emitted at `matching.rs:` |\n|---|---|\n");
    for (variant, lines) in emit_sites(MATCHING_SOURCE) {
        let joined = lines
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" / ");
        out.push_str(&format!("| `{variant}` | {joined} |\n"));
    }
    out.push_str(
        "\n**`DisplaceOk` and `DisplaceFail` share one line**, because a single \
         emit chooses between them with an `if`. The site identifies *where*, \
         never *which* — only the frame itself distinguishes the outcome.\n\n",
    );

    for (model, why) in LEDGER_SPECIMENS {
        out.push_str(&format!("## Ledger — `{model}` ({why})\n\n"));
        match ledger_rows(model) {
            Some(rows) => {
                out.push_str("| idx | step | emit line | depth |\n|---|---|---|---|\n");
                for (i, step, line, depth) in rows {
                    out.push_str(&format!("| {i} | {step} | {line} | {depth} |\n"));
                }
                out.push('\n');
            }
            None => {
                // Stated, not filled — the rule this repository is built on.
                out.push_str(
                    "*No ledger: this specimen has no recorded structural trace. \
                     Regenerate it with `cargo run -p hrw --example gen_trace`.*\n\n",
                );
            }
        }
    }

    out.push_str(
        "**Depth is derived from the step sequence, not from a stack** — \
         `TryDisplace` descends and `DisplaceOk`/`DisplaceFail` return. It is \
         pinned against two real debugger walks by `matching_ledger`'s tests.\n\n",
    );
    out.push_str(
        "**What is missing here, deliberately:** the two `augment_traced:243` \
         give-ups that emit no frame at all. They are real algorithm steps that \
         the frame stream cannot contain, so no generator can list them — see \
         `docs/ideas.md` #73. **A ledger is not a transcript of the search.**\n",
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Every step variant has an emit site, and `EquationFailed`'s is 133.**
    ///
    /// 133 is the `emit_matching_frame(` call; **137** is where the variant is
    /// named, inside the struct-literal argument. It was published as 137 for a
    /// day — read by hand from the wrong line of the same call, in the one row
    /// no walk had ever observed. This test is the mechanism that makes reading
    /// it by hand unnecessary.
    #[test]
    fn every_step_has_an_emit_site() {
        let sites = emit_sites(MATCHING_SOURCE);
        for variant in [
            "TryEquation",
            "Explore",
            "FoundFree",
            "TryDisplace",
            "DisplaceOk",
            "DisplaceFail",
            "Assign",
            "EquationFailed",
        ] {
            assert!(
                sites.contains_key(variant),
                "no emit site found for {variant} \u{2014} the scanner missed it, \
                 and a missing row is how a tour ends up quoting a guess: {sites:?}"
            );
        }

        // The call line, never the line naming the variant.
        let failed = &sites["EquationFailed"];
        let src_line = MATCHING_SOURCE
            .lines()
            .nth(failed[0] - 1)
            .expect("the recorded line must exist in the source");
        assert!(
            src_line.contains("emit_matching_frame("),
            "EquationFailed's site must be the emit CALL line, got line {} = {src_line:?}",
            failed[0]
        );
    }

    /// **`DisplaceOk` and `DisplaceFail` share a line, and that must survive.**
    ///
    /// Collapsing the site list to one entry would hide that the emit site does
    /// not determine the outcome — the thing a tour would otherwise get wrong
    /// by assuming a line number names a step.
    #[test]
    fn the_two_displacement_outcomes_share_one_emit_site() {
        let sites = emit_sites(MATCHING_SOURCE);
        assert_eq!(
            sites["DisplaceOk"], sites["DisplaceFail"],
            "these are emitted by one call with an `if` for the step"
        );
    }

    /// **The derived depths match what the debugger actually showed.**
    ///
    /// Both sequences were read off live stacks on 2026-08-08 by counting
    /// `augment_traced` frames (`docs/ideas.md` #73). They are the oracle: a
    /// derivation checked only against itself would be the vacuous-test trap
    /// this project hit the same day.
    #[test]
    fn the_derived_depths_match_what_the_debugger_showed() {
        use MatchingStep::{
            Assign, DisplaceFail, DisplaceOk, Explore, FoundFree, TryDisplace, TryEquation,
        };

        // ProportionalLoop, observed depths 0,1,1,1,0,1,1,2,2,2,1,1.
        let proportional = [
            TryEquation(0),
            Explore { eq: 0, var: 0 },
            FoundFree { eq: 0, var: 0 },
            Assign { eq: 0, var: 0 },
            TryEquation(1),
            Explore { eq: 1, var: 0 },
            TryDisplace {
                eq: 1,
                var: 0,
                holder: 0,
            },
            Explore { eq: 0, var: 2 },
            FoundFree { eq: 0, var: 2 },
            Assign { eq: 0, var: 2 },
            DisplaceOk { eq: 1, var: 0 },
            Assign { eq: 1, var: 0 },
        ];
        assert_eq!(
            derive_depths(&proportional),
            vec![0, 1, 1, 1, 0, 1, 1, 2, 2, 2, 1, 1],
            "ProportionalLoop's depths must match the walked stack"
        );

        // TwiceDefined, observed depths 0,1,1,1,0,1,1,1,0.
        let twice = [
            TryEquation(0),
            Explore { eq: 0, var: 0 },
            FoundFree { eq: 0, var: 0 },
            Assign { eq: 0, var: 0 },
            TryEquation(1),
            Explore { eq: 1, var: 0 },
            TryDisplace {
                eq: 1,
                var: 0,
                holder: 0,
            },
            DisplaceFail { eq: 1, var: 0 },
            MatchingStep::EquationFailed(1),
        ];
        assert_eq!(
            derive_depths(&twice),
            vec![0, 1, 1, 1, 0, 1, 1, 1, 0],
            "TwiceDefined's depths must match the walked stack"
        );
    }

    /// **The generated ledger reproduces the walked run, step for step.**
    ///
    /// Non-vacuity for the whole generator: it re-runs the real algorithm over
    /// `TwiceDefined`'s recorded incidence and must produce exactly the nine
    /// frames Doug stepped through, in order. If the specimen, the algorithm or
    /// the trace changes, this fails rather than silently describing a different
    /// run.
    #[test]
    fn the_generated_ledger_reproduces_the_twicedefined_walk() {
        let rows = ledger_rows("TwiceDefined").expect("TwiceDefined must have a structural trace");
        let steps: Vec<&str> = rows.iter().map(|(_, s, _, _)| s.as_str()).collect();
        assert_eq!(rows.len(), 9, "the walk produced nine frames, got {steps:?}");

        assert!(steps[0].contains("TryEquation(0)"));
        assert!(steps[6].contains("TryDisplace"), "frame 6 is the displacement");
        assert!(steps[7].contains("DisplaceFail"), "frame 7 is the refusal");
        assert!(
            steps[8].contains("EquationFailed(1)"),
            "frame 8 is the give-up"
        );

        let depths: Vec<usize> = rows.iter().map(|(_, _, _, d)| *d).collect();
        assert_eq!(depths, vec![0, 1, 1, 1, 0, 1, 1, 1, 0]);
    }

    /// **The reference on disk matches a fresh generation.**
    ///
    /// The same generate-and-compare shape as `tour_catalogue_is_current`, and
    /// the reason this whole module exists: when `matching.rs` moves, this fails
    /// and names the command, instead of the tours quietly citing lines that
    /// have shifted.
    #[test]
    fn the_generated_reference_is_current() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(REFERENCE_PATH);
        let Ok(on_disk) = std::fs::read_to_string(&path) else {
            panic!(
                "no {REFERENCE_PATH} \u{2014} run: \
                 cargo run -p hrw --example gen_matching_reference"
            );
        };
        let fresh = reference();
        if on_disk == fresh {
            return;
        }

        // **Report the first differing line, not both documents.** A whole-file
        // `assert_eq!` on a 3 KB table prints 6 KB to say one number moved, and
        // a failure nobody can read is a failure nobody acts on. Line numbers
        // shift in blocks, so the first difference is the diagnosis.
        let (mut old, mut new) = (on_disk.lines(), fresh.lines());
        let mut n = 0usize;
        loop {
            n += 1;
            match (old.next(), new.next()) {
                (Some(a), Some(b)) if a == b => {}
                (a, b) => {
                    panic!(
                        "{REFERENCE_PATH} is out of date at line {n} \u{2014} run: \
                         cargo run -p hrw --example gen_matching_reference\n  \
                         on disk: {a:?}\n  derived: {b:?}\n\
                         (the derived side is the truth: it is read from \
                         matching.rs as compiled)"
                    );
                }
            }
        }
    }
}
