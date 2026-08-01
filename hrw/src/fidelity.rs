//! **Does HRW faithfully represent Rumoca?** — the F2–F5 invariant harness.
//!
//! `docs/fidelity-plan.md` in one sentence: *HRW must invent nothing and omit
//! nothing.* F1 lives in `worker.rs` because it compares a re-derivation against
//! a report; the checks here are different in kind and cheaper to trust.
//!
//! # Invariants, not expected answers
//!
//! The obstacle to testing at scale is **triage cost**: with unfamiliar models
//! every failure is an investigation, and a wrong finding costs more than a
//! missing one. An invariant dodges that. It does not ask what the right answer
//! is for a given model — it asserts a property that must hold for *every*
//! model, so a violation is definitionally a bug with nothing to adjudicate.
//!
//! That is what makes this affordable over 40–60 MSL models where a
//! compile-census is not (`docs/ideas.md` #51).
//!
//! # Why a harness rather than one test per check
//!
//! Each model is a full uncached compile against the MSL — seconds. Running
//! F2–F5 as four tests over the same corpus would compile every model four
//! times, which is what made F1 take 148s over ten specimens.
//!
//! So: **compile once, apply every invariant, drop the model.** Memoizing
//! instead would hold every compiled model in memory at once, and the payload is
//! the entire compiled state — see `docs/ideas.md` #48 for why that trade runs
//! backwards on this machine.
//!
//! Violations are **collected and reported together**, not asserted one at a
//! time: a checker that stops at the first failure turns fixing a batch into a
//! sequence of rebuilds.
//!
//! # What a "subject" is
//!
//! Not "the structural report" — *every* report HRW publishes that carries an
//! incidence matrix. Today that is the Structural stage, the Index Reduction
//! stage, and the `before` report nested inside the latter. Walking them
//! generically matters: the `before` report carried the same equation-labelling
//! bug F1 found in the singular matching, and a check written against
//! "structural" alone would have missed it.

//! # The checks mirror the algorithm chain
//!
//! The fastest way to hold all of them. Structural analysis is a pipeline where
//! each step consumes the last:
//!
//! ```text
//! DAE -> incidence -> matching -> SCCs (Tarjan) -> BLT blocks -> tearing
//! ```
//!
//! **Each check guards one link.** F2 guards the incidence (everything indexes
//! into it); F5 guards the matching (a pair whose equation does not *contain*
//! its unknown is not a matching at all); F4 guards the BLT partition; F3
//! guards the counts across all of it; and **F1 guards the re-derivation** —
//! the animations re-run these algorithms, so F1 is what stops an animation
//! teaching a decision Rumoca never made.
//!
//! F6, F7, F8 and F9 sit outside the chain, guarding the derived views, the
//! capture vocabulary, scale, and failure reporting.
//!
//! **A break in an early link shows up as violations in every later one.** If
//! F2 fails, F5 and F4 almost certainly fail too — not because they are
//! separately wrong but because they consumed a matrix that was already wrong.
//! **Triage the earliest failing link first.** Full version in
//! `docs/fidelity-plan.md`.
//!
//! # Why the checks are not test-only code
//!
//! They were, until 2026-07-31. Running them over hundreds of MSL models needs a
//! second caller — `examples/fidelity_msl.rs` — and a check that exists twice is
//! a check that drifts, which is the exact defect F1 and F7 both found. So the
//! checks are ordinary module functions with **two** callers: the fast test over
//! the curated specimens, and the scale runner.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::worker::StageBundle;

/// A report HRW publishes that carries an incidence matrix, together with the
/// DAE it claims to describe.
///
/// `dae` is `None` when the subject's system is not one we hold — nothing in
/// the bundle is like that today, but F2 skips rather than guesses if it
/// appears, because comparing against the wrong DAE would manufacture failures.
pub struct Subject<'a> {
    /// Where this report lives, for a violation message: `Structural`,
    /// `IndexReduction`, `IndexReduction.before`.
    pub label: String,
    pub report: &'a Value,
    pub dae: Option<&'a rumoca_ir_dae::Dae>,
}

/// One violation, tagged with the check that raised it.
///
/// **Tagging is what makes a large run triageable.** F7's first run produced
/// 6,169 violations and was diagnosable only because the first few lines
/// happened to be representative — luck that does not survive a corpus of
/// hundreds. Grouped by check, a flood becomes "F7: 6,169, all of one shape".
///
/// Grouping is by *check* rather than by message template, deliberately: a
/// template needs every push site to name its kind, and per-check proved
/// sufficient for every real finding so far. Revisit if a stage-B run produces
/// two genuinely different failures inside one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// `"F2"` … `"F7"`.
    pub check: &'static str,
    pub detail: String,
}

/// Run every subject-based and view-based check on one compiled model.
///
/// `reduced` is `Option` and that is the point: **index reduction is capped by
/// system size**, exactly as the survey caps it, because the cost is the same
/// cost. Passing `None` omits the `IndexReduction` subjects and the checks skip
/// them — which they already do for singular models — so a 10,175-equation model
/// still contributes F2, F3, F5, F6 and F7 on its `Structural` subject rather
/// than being excluded from the corpus entirely.
///
/// F8 (sizes) and F9 (failure faithfulness) are the runner's business, not this
/// function's: they need the whole bundle rather than a subject.
/// Milliseconds spent in each check, accumulated across models.
///
/// **Built because the cause of a 16x slowdown was a guess.** `IMC_Transformer`
/// (5,061 equations) compiles in 18.4s in the survey and exceeds 300s under the
/// fidelity checks, with index reduction skipped in both — so the cost is in the
/// checks, but *which* check was inference, not measurement
/// (`docs/architecture.md` §11).
///
/// It matters beyond curiosity: 44 models in the corpus are >=1,200 equations,
/// and at 300s each that is 3.7 hours of a full run producing **no data** on
/// exactly the models that stress the representation hardest. Knowing whether
/// one check is superlinear decides whether that is fixable or inherent.
#[derive(Debug, Default, Clone)]
pub struct CheckTiming {
    /// `(check, milliseconds)` — F2..F7, in the order they run.
    pub ms: BTreeMap<&'static str, f64>,
}

impl CheckTiming {
    fn add(&mut self, check: &'static str, t: std::time::Instant) {
        *self.ms.entry(check).or_default() += t.elapsed().as_secs_f64() * 1000.0;
    }

    /// Checks by cost, most expensive first.
    pub fn ranked(&self) -> Vec<(&'static str, f64)> {
        let mut v: Vec<(&'static str, f64)> = self.ms.iter().map(|(k, t)| (*k, *t)).collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1));
        v
    }

    pub fn total_ms(&self) -> f64 {
        self.ms.values().sum()
    }
}

pub fn check_model(
    stages: &StageBundle,
    dae: &rumoca_ir_dae::Dae,
    reduced: Option<&rumoca_ir_dae::Dae>,
    sheet: Option<&crate::equation_sheet::EquationSheet>,
    index: Option<&crate::identifier_index::IdentifierIndex>,
    coverage: &mut Coverage,
    timing: &mut CheckTiming,
    // `only`: which checks to run; `None` runs all of them.
    //
    // **Exists to make the checks measurable, not configurable.** Running one
    // model once per check turns the per-process watchdog — which already
    // records peak RSS and elapsed time — into a per-check profiler, with no
    // new instrumentation and no dependency. That matters because stage C
    // produced TWO scaling failures: seven timeouts and three memory blowouts
    // at up to 7.7 GB, and timing alone would have explained only the first.
    only: Option<&std::collections::BTreeSet<String>>,
) -> Vec<Violation> {
    let mut out = Vec::new();
    let want = |c: &str| only.is_none_or(|set| set.contains(c));
    let tag = |check: &'static str, v: Vec<String>| {
        v.into_iter().map(move |detail| Violation { check, detail })
    };

    for s in subjects(stages, dae, reduced.unwrap_or(dae)) {
        // Without a reduced DAE the IndexReduction subjects would describe the
        // raw system while claiming to be the reduced one — the raw-vs-reduced
        // confusion that broke F1's first draft. Skip them instead.
        if reduced.is_none() && s.label.starts_with("IndexReduction") {
            continue;
        }
        coverage.subjects += 1;
        if s.report["blocks"].as_array().is_some_and(|b| !b.is_empty()) {
            coverage.with_blocks += 1;
        }
        if s.report["matching"].as_array().is_some_and(|m| !m.is_empty()) {
            coverage.with_matching += 1;
        }
        if want("F2") {
            let t = std::time::Instant::now();
            out.extend(tag("F2", check_f2(&s)));
            timing.add("F2", t);
        }
        if want("F3") {
            let t = std::time::Instant::now();
            out.extend(tag("F3", check_f3(&s)));
            timing.add("F3", t);
        }
        if want("F4") {
            let t = std::time::Instant::now();
            out.extend(tag("F4", check_f4(&s)));
            timing.add("F4", t);
        }
        if want("F5") {
            let t = std::time::Instant::now();
            out.extend(tag("F5", check_f5(&s)));
            timing.add("F5", t);
        }
    }
    if sheet.is_some() {
        coverage.with_sheet += 1;
    }
    if index.is_some() {
        coverage.with_index += 1;
    }
    if want("F6") {
        let t = std::time::Instant::now();
        out.extend(tag("F6", check_f6(sheet, index, dae)));
        timing.add("F6", t);
    }
    for kind in crate::worker::StageKind::COMPILATION {
        if let Some(root) = stages.get(*kind).value.as_ref() {
            coverage.stage_irs += 1;
            if want("F7") {
                let t = std::time::Instant::now();
                out.extend(tag("F7", check_f7(&format!("{kind:?}"), root)));
                timing.add("F7", t);
            }
        }
    }
    out
}

/// **What the checks actually looked at.**
///
/// "0 violations" means nothing without "over how much". Every check skips
/// subjects it does not apply to, so a corpus that produced no blocks and no
/// matchings would pass in silence — and that is not hypothetical: the survey
/// shipped a column that was zero everywhere because nothing asked whether it
/// ever fired.
#[derive(Debug, Default, Clone)]
pub struct Coverage {
    pub subjects: usize,
    pub with_blocks: usize,
    pub with_matching: usize,
    pub with_sheet: usize,
    pub with_index: usize,
    pub stage_irs: usize,
}

/// Violations grouped by check, most numerous first — the triage view.
///
/// Returns `(check, count, up to `examples` details)`.
pub fn group_by_check(violations: &[Violation], examples: usize) -> Vec<(&'static str, usize, Vec<String>)> {
    let mut by: BTreeMap<&'static str, Vec<&str>> = BTreeMap::new();
    for v in violations {
        by.entry(v.check).or_default().push(&v.detail);
    }
    let mut out: Vec<(&'static str, usize, Vec<String>)> = by
        .into_iter()
        .map(|(check, ds)| {
            let n = ds.len();
            (check, n, ds.into_iter().take(examples).map(str::to_owned).collect())
        })
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    out
}

/// Every incidence-bearing report in the bundle, with the DAE it describes.
///
/// The Structural stage describes the **raw** system and Index Reduction the
/// **reduced** one — the distinction that made F1's first tearing check fail
/// for the wrong reason. `before` is the raw system again, published inside
/// the reduced stage.
pub fn subjects<'a>(
    stages: &'a StageBundle,
    dae: &'a rumoca_ir_dae::Dae,
    reduced: &'a rumoca_ir_dae::Dae,
) -> Vec<Subject<'a>> {
    let mut out = Vec::new();
    let mut push = |label: &str, report: Option<&'a Value>, d: &'a rumoca_ir_dae::Dae| {
        if let Some(r) = report
            && r.get("incidence").is_some()
        {
            out.push(Subject { label: label.to_owned(), report: r, dae: Some(d) });
        }
    };
    push("Structural", stages.structural.value.as_ref(), dae);
    push("IndexReduction", stages.index_reduction.value.as_ref(), reduced);
    push(
        "IndexReduction.before",
        stages.index_reduction.value.as_ref().and_then(|v| v.get("before")),
        dae,
    );
    out
}

/// `rows[i]["equation"]` — the published row labels, in row order.
fn row_labels(report: &Value) -> Vec<String> {
    report["incidence"]["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| r["equation"].as_str().unwrap_or_default().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn unknown_names(report: &Value) -> Vec<String> {
    report["incidence"]["unknown_names"]
        .as_array()
        .map(|ns| ns.iter().map(|n| n.as_str().unwrap_or_default().to_owned()).collect())
        .unwrap_or_default()
}

/// `rows[i]["unknowns"]` as sets, in row order.
fn row_sets(report: &Value) -> Vec<BTreeSet<usize>> {
    report["incidence"]["rows"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    r["unknowns"]
                        .as_array()
                        .map(|u| u.iter().filter_map(Value::as_u64).map(|v| v as usize).collect())
                        .unwrap_or_default()
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A name → index map, reporting duplicates rather than silently keeping the
/// last. Duplicate row labels would make every name-keyed lookup in the UI
/// ambiguous, which is worth knowing about on its own.
fn index_of(names: &[String]) -> (BTreeMap<&str, usize>, Vec<String>) {
    let mut map = BTreeMap::new();
    let mut dups = Vec::new();
    for (i, n) in names.iter().enumerate() {
        if let Some(prev) = map.insert(n.as_str(), i) {
            dups.push(format!("duplicate name {n:?} at rows {prev} and {i}"));
        }
    }
    (map, dups)
}

// ---------------------------------------------------------------- F2

/// **F2 — the published incidence is what Rumoca builds.**
///
/// Compared against a fresh `build_incidence` on the DAE the subject
/// describes: same dimensions, same unknown order, same row labels, and the
/// same column set per row.
///
/// Order matters and is checked as order. Every downstream view indexes into
/// these arrays, so a permutation would leave the matrix looking entirely
/// plausible while every label sat against the wrong row.
pub fn check_f2(s: &Subject) -> Vec<String> {
    let Some(dae) = s.dae else { return Vec::new() };
    let mut v = Vec::new();
    let inc = rumoca_phase_structural::build_incidence(dae);

    let (n_eq, n_var) = (
        s.report["incidence"]["n_eq"].as_u64().unwrap_or(u64::MAX) as usize,
        s.report["incidence"]["n_var"].as_u64().unwrap_or(u64::MAX) as usize,
    );
    if n_eq != inc.n_eq {
        v.push(format!("{}: n_eq {n_eq} published, {} from build_incidence", s.label, inc.n_eq));
    }
    if n_var != inc.n_var {
        v.push(format!("{}: n_var {n_var} published, {} from build_incidence", s.label, inc.n_var));
    }

    let published_unknowns = unknown_names(s.report);
    let real_unknowns: Vec<String> = inc.unknown_names.iter().map(ToString::to_string).collect();
    if published_unknowns != real_unknowns {
        v.push(format!(
            "{}: unknown_names differ from build_incidence (first difference at {:?})",
            s.label,
            published_unknowns
                .iter()
                .zip(&real_unknowns)
                .position(|(a, b)| a != b)
                .map_or_else(|| "length".to_owned(), |i| i.to_string()),
        ));
    }

    let published_labels = row_labels(s.report);
    let real_labels: Vec<String> = (0..inc.n_eq)
        .map(|i| rumoca_phase_structural::equation_label(dae, &inc.equation_refs[i]))
        .collect();
    if published_labels != real_labels {
        v.push(format!("{}: row equation labels differ from equation_label", s.label));
    }

    let published_rows = row_sets(s.report);
    for (i, real) in inc.eq_unknowns.iter().enumerate() {
        let real: BTreeSet<usize> = real.iter().copied().collect();
        match published_rows.get(i) {
            Some(pub_row) if *pub_row == real => {}
            Some(pub_row) => v.push(format!(
                "{}: row {i} publishes {pub_row:?}, build_incidence says {real:?}",
                s.label,
            )),
            None => v.push(format!("{}: row {i} missing; only {} published", s.label, published_rows.len())),
        }
    }
    v
}

// ---------------------------------------------------------------- F3

/// **F3 — the counts agree with each other and with the incidence.**
///
/// The `rank_deficiency: 7` bug of 2026-07-29 (true value 1) is exactly this
/// class and was found by eye: the error described the *reduced* system while
/// the incidence beside it described the raw one. A wrong number reads as
/// authoritative, so it is worse than a missing one.
pub fn check_f3(s: &Subject) -> Vec<String> {
    let mut v = Vec::new();
    let n_eq = s.report["incidence"]["n_eq"].as_u64().unwrap_or_default() as usize;
    let n_var = s.report["incidence"]["n_var"].as_u64().unwrap_or_default() as usize;

    for key in ["n_equations", "n_unknowns"] {
        let expect = if key == "n_equations" { n_eq } else { n_var };
        if let Some(got) = s.report[key].as_u64()
            && got as usize != expect
        {
            v.push(format!("{}: {key} = {got}, incidence says {expect}", s.label));
        }
    }

    let n_matched = s.report["matching"].as_array().map(Vec::len);
    if let (Some(m), Some(published)) = (n_matched, s.report["n_matched"].as_u64())
        && m != published as usize
    {
        v.push(format!("{}: n_matched = {published}, but matching has {m} pairs", s.label));
    }
    if let Some(m) = n_matched
        && m > n_eq.min(n_var)
    {
        v.push(format!(
            "{}: {m} matched pairs exceeds min(n_eq {n_eq}, n_var {n_var})",
            s.label,
        ));
    }

    // A singular error's own counts must describe the same system as the
    // incidence published beside it — the 2026-07-29 defect exactly.
    let err = &s.report["error"];
    if err["kind"] == "singular" {
        for (key, expect) in [("n_equations", n_eq), ("n_unknowns", n_var)] {
            if let Some(got) = err[key].as_u64()
                && got as usize != expect
            {
                v.push(format!(
                    "{}: error.{key} = {got} but the incidence beside it says {expect} \
                     — the error and the matrix describe different systems",
                    s.label,
                ));
            }
        }
        if let (Some(rd), Some(nm), Some(ne), Some(nu)) = (
            err["rank_deficiency"].as_u64(),
            err["n_matched"].as_u64(),
            err["n_equations"].as_u64(),
            err["n_unknowns"].as_u64(),
        ) && rd != ne.max(nu) - nm
        {
            v.push(format!("{}: rank_deficiency {rd} != max({ne},{nu}) - {nm}", s.label));
        }
    }
    v
}

// ---------------------------------------------------------------- F4

/// **F4 — the BLT blocks partition the equations.**
///
/// Every equation in exactly one block, and every block equation resolvable
/// to a real row. A partition is impossible to satisfy by accident, so a
/// violation is a genuine defect with no adjudication needed.
///
/// Reports with no `blocks` are skipped rather than failed: a singular system
/// legitimately has none, because the analysis stopped before BLT ran.
pub fn check_f4(s: &Subject) -> Vec<String> {
    let Some(blocks) = s.report["blocks"].as_array() else { return Vec::new() };
    let mut v = Vec::new();
    let labels = row_labels(s.report);
    let (index, dups) = index_of(&labels);
    v.extend(dups.into_iter().map(|d| format!("{}: {d}", s.label)));

    let mut seen: BTreeMap<usize, usize> = BTreeMap::new();
    for (bi, b) in blocks.iter().enumerate() {
        let names: Vec<&str> = match (&b["equation"], &b["equations"]) {
            (Value::String(one), _) => vec![one.as_str()],
            (_, Value::Array(many)) => many.iter().filter_map(Value::as_str).collect(),
            _ => {
                v.push(format!("{}: block {bi} names no equations", s.label));
                continue;
            }
        };
        for n in names {
            match index.get(n) {
                Some(&row) => *seen.entry(row).or_default() += 1,
                None => v.push(format!(
                    "{}: block {bi} names equation {n:?}, which is not an incidence row",
                    s.label,
                )),
            }
        }
    }

    for (row, count) in &seen {
        if *count > 1 {
            v.push(format!("{}: equation row {row} appears in {count} blocks", s.label));
        }
    }
    if seen.len() != labels.len() {
        let missing: Vec<usize> = (0..labels.len()).filter(|i| !seen.contains_key(i)).collect();
        v.push(format!(
            "{}: blocks cover {} of {} equations; missing rows {missing:?}",
            s.label,
            seen.len(),
            labels.len(),
        ));
    }
    v
}

// ---------------------------------------------------------------- F5

/// **F5 — the matching is a matching.**
///
/// Injective both ways, in range, and — the check with real teeth — every
/// matched pair must be a **non-zero of the incidence**. An equation paired
/// with a variable it does not contain is not a wrong choice among valid
/// ones; it is not a matching at all, and every solve order built on it is
/// meaningless.
pub fn check_f5(s: &Subject) -> Vec<String> {
    let Some(pairs) = s.report["matching"].as_array() else { return Vec::new() };
    let mut v = Vec::new();
    let labels = row_labels(s.report);
    let unknowns = unknown_names(s.report);
    let (eq_index, _) = index_of(&labels);
    let (var_index, _) = index_of(&unknowns);
    let rows = row_sets(s.report);

    let mut used_eq: BTreeMap<usize, usize> = BTreeMap::new();
    let mut used_var: BTreeMap<usize, usize> = BTreeMap::new();

    for (i, p) in pairs.iter().enumerate() {
        let (Some(e), Some(u)) = (p["equation"].as_str(), p["unknown"].as_str()) else {
            v.push(format!("{}: matching pair {i} is not a (equation, unknown) pair", s.label));
            continue;
        };
        let (Some(&er), Some(&uc)) = (eq_index.get(e), var_index.get(u)) else {
            v.push(format!(
                "{}: matching pair {i} names {e:?} / {u:?}, which do not both resolve \
                 against the incidence — the overlay would silently show nothing",
                s.label,
            ));
            continue;
        };
        *used_eq.entry(er).or_default() += 1;
        *used_var.entry(uc).or_default() += 1;

        if !rows.get(er).is_some_and(|r| r.contains(&uc)) {
            v.push(format!(
                "{}: equation {e:?} is matched to {u:?}, which it does not reference \
                 — that is not a matching",
                s.label,
            ));
        }
    }

    for (what, used) in [("equation", &used_eq), ("unknown", &used_var)] {
        for (idx, count) in used.iter().filter(|(_, c)| **c > 1) {
            v.push(format!("{}: {what} {idx} matched {count} times", s.label));
        }
    }
    v
}

// ---------------------------------------------------------------- F6

/// **F6 — the derived views cover their source.**
///
/// The equation sheet and the identifier index are *rebuilt* from the DAE
/// rather than read from a stage, so they can silently cover less than the
/// thing they claim to describe. An equation sheet missing an equation does
/// not look broken — it looks like a shorter model.
pub fn check_f6(
    sheet: Option<&crate::equation_sheet::EquationSheet>,
    index: Option<&crate::identifier_index::IdentifierIndex>,
    dae: &rumoca_ir_dae::Dae,
) -> Vec<String> {
    let mut v = Vec::new();
    let n_eq = dae.continuous.equations.len();

    if let Some(sheet) = sheet {
        if sheet.n_equations != n_eq {
            v.push(format!(
                "equation sheet reports {} equations, the DAE has {n_eq}",
                sheet.n_equations,
            ));
        }
        // Every equation exactly once, by index — a sheet that grouped one
        // equation twice and dropped another would keep the count right.
        let mut seen: BTreeMap<usize, usize> = BTreeMap::new();
        for (_, eqs) in &sheet.groups {
            for e in eqs {
                *seen.entry(e.index).or_default() += 1;
            }
        }
        let missing: Vec<usize> = (0..n_eq).filter(|i| !seen.contains_key(i)).collect();
        if !missing.is_empty() {
            v.push(format!("equation sheet omits equation indices {missing:?}"));
        }
        for (i, c) in seen.iter().filter(|(_, c)| **c > 1) {
            v.push(format!("equation sheet lists equation {i} {c} times"));
        }
        for i in seen.keys().filter(|i| **i >= n_eq) {
            v.push(format!("equation sheet lists equation {i}, beyond the DAE's {n_eq}"));
        }
    }

    if let Some(index) = index {
        // Keyed by the `kind` the index itself recorded, so this check does
        // not merely re-list the DAE's partitions and drift. The first draft
        // did exactly that — it omitted the two discrete partitions and
        // reported `BouncingBall`'s `c` as a phantom variable. A partition
        // added later lands in the `_` arm and says so, instead of
        // masquerading as a fidelity violation.
        let vars = &dae.variables;
        let set = |ks: Vec<String>| -> BTreeSet<String> { ks.into_iter().collect() };
        let partitions: BTreeMap<&str, BTreeSet<String>> = [
            ("state", set(vars.states.keys().map(ToString::to_string).collect())),
            ("algebraic", set(vars.algebraics.keys().map(ToString::to_string).collect())),
            ("input", set(vars.inputs.keys().map(ToString::to_string).collect())),
            ("output", set(vars.outputs.keys().map(ToString::to_string).collect())),
            ("parameter", set(vars.parameters.keys().map(ToString::to_string).collect())),
            ("constant", set(vars.constants.keys().map(ToString::to_string).collect())),
            ("discrete real", set(vars.discrete_reals.keys().map(ToString::to_string).collect())),
            ("discrete valued", set(vars.discrete_valued.keys().map(ToString::to_string).collect())),
        ]
        .into_iter()
        .collect();

        for (name, iv) in &index.variables {
            let Some(partition) = partitions.get(iv.kind) else {
                v.push(format!(
                    "identifier index uses partition {:?}, which this check does not \
                     know — extend check_f6 rather than trusting it",
                    iv.kind,
                ));
                continue;
            };
            if !partition.contains(name) {
                v.push(format!(
                    "identifier index names {name:?} as a {} , which the DAE's {} \
                     partition does not contain — a click on it resolves to nothing",
                    iv.kind, iv.kind,
                ));
            }
        }
        // The line map must agree with the variables it indexes.
        for (line, names) in &index.line_to_variables {
            for n in names {
                if !index.variables.contains_key(n) {
                    v.push(format!("line {line} maps to {n:?}, absent from the index"));
                }
            }
        }
    }
    v
}

// ---------------------------------------------------------------- F7

/// Up to `cap` node paths from a JSON tree, breadth-first.
///
/// Breadth-first on purpose: depth-first on real IR spends the whole budget
/// inside the first equation's expression tree and never reaches a sibling
/// stage field, so the sample would not represent what a user can click.
pub fn sample_paths(root: &Value, cap: usize) -> Vec<Vec<crate::bridge::Seg>> {
    use crate::bridge::Seg;
    let mut out: Vec<Vec<Seg>> = Vec::new();
    let mut queue: std::collections::VecDeque<Vec<Seg>> = std::collections::VecDeque::new();
    queue.push_back(Vec::new());
    while let Some(path) = queue.pop_front() {
        if out.len() >= cap {
            break;
        }
        let Some(node) = crate::bridge::navigate(root, &path) else { continue };
        match node {
            Value::Object(map) => {
                for k in map.keys() {
                    let mut p = path.clone();
                    p.push(Seg::Key(k.clone()));
                    out.push(p.clone());
                    queue.push_back(p);
                }
            }
            Value::Array(items) => {
                for i in 0..items.len().min(4) {
                    let mut p = path.clone();
                    p.push(Seg::Index(i));
                    out.push(p.clone());
                    queue.push_back(p);
                }
            }
            _ => {}
        }
    }
    out.truncate(cap);
    out
}

/// **F7 — every capture noun survives the round trip on real IR.**
///
/// `describe_path` renders a node path into an `hrw://` link; `parse_path`
/// reads it back. The two have only ever been exercised on hand-built values
/// and short specimens, and they are what *every* question Doug asks depends
/// on — a noun that does not round-trip points Claude at the wrong subtree
/// while looking perfectly well-formed.
///
/// Compares the **navigated subtree**, not the rendered string: a path may
/// legitimately render differently as long as it still lands on the same
/// node, and comparing strings would flag that as a failure.
pub fn check_f7(label: &str, root: &Value) -> Vec<String> {
    let mut v = Vec::new();
    for path in sample_paths(root, 400) {
        let described = crate::bridge::describe_path(&path);
        let Some(parsed) = crate::bridge::parse_path(&described) else {
            v.push(format!("{label}: {described:?} does not parse back"));
            continue;
        };
        let want = crate::bridge::navigate(root, &path);
        let got = crate::bridge::navigate(root, &parsed);
        if want != got {
            v.push(format!(
                "{label}: {described:?} round-trips to a different node \
                 (path {path:?} vs parsed {parsed:?})",
            ));
        }
    }
    v
}


#[cfg(test)]
mod tests {
    use super::*;

    use crate::worker::test_msl::compile_specimen_shared;
    use crate::worker::{FromWorker, index_reduce_in_place};

    /// The specimens the invariants run over.
    ///
    /// Deliberately the same list F1 uses, so a model added for one check is
    /// covered by all of them. This is the stand-in for the stratified MSL
    /// sample `docs/fidelity-plan.md` describes — the harness is written to take
    /// any list of names, so growing the corpus is a one-line change.
    const MODELS: &[&str] = &[
        "ProportionalLoop", "MixedLoop", "TwoLoops", "NonlinearLoop", "Drivetrain",
        "RcCircuit", "SingleInertia", "CapacitorLoop", "BouncingBall", "MotorWithBrake",
    ];

    // ---------------------------------------------------------------- report

    /// Where the fidelity report is written.
    ///
    /// Checked in, like the survey: it is a **deliverable**, not scratch output
    /// (`docs/upstream-strategy.md` planning rule 1). Its steady state is green,
    /// and a green report over the corpus is the evidence artifact the
    /// methodology rests on — a certificate rather than a bug list
    /// (`docs/reports.md`).
    fn report_path() -> std::path::PathBuf {
        std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/fidelity-report.csv"))
    }

    /// One model's fidelity result.
    ///
    /// **Every model gets a row, green ones included.** The report is a
    /// certificate, so "2,600 models, no violations" has to be *readable from
    /// the file*; Test mode filters to exceptions for display rather than the
    /// writer filtering them out of existence.
    struct FidelityRow {
        name: String,
        violations: Vec<String>,
    }

    impl FidelityRow {
        /// Shares its first four columns with every other report so one loader
        /// serves all three — see `crate::report`.
        const HEADER: &'static str = "name,kind,outcome,message,checks_failed,n_violations";

        fn to_csv(&self) -> String {
            // Which checks fired, deduplicated and ordered — `F2,F5` clusters a
            // failure far better than its first message does.
            let mut checks: Vec<&str> = self
                .violations
                .iter()
                .filter_map(|v| v.split(':').nth(1).map(str::trim))
                .filter(|t| t.len() == 2 && t.starts_with('F'))
                .collect();
            checks.sort_unstable();
            checks.dedup();

            let outcome = if self.violations.is_empty() { "ok" } else { "violations" };
            let f = crate::survey::csv_field;
            format!(
                "{},{},{},{},{},{}",
                f(&self.name),
                f(&crate::survey::classify(&self.name)),
                outcome,
                // The first violation, verbatim. The rest are reachable by
                // re-running the check on that one model, which is what Test
                // mode's click is for — so the report stays a list, not a log.
                f(self.violations.first().map_or("", String::as_str)),
                f(&checks.join(" ")),
                self.violations.len(),
            )
        }
    }

    /// Write the report, and return what was written for assertion.
    fn write_report(rows: &[FidelityRow]) -> String {
        let mut out = vec![FidelityRow::HEADER.to_owned()];
        out.extend(rows.iter().map(FidelityRow::to_csv));
        let text = out.join("\n") + "\n";
        // **No timestamp and no timings in the row data**, deliberately: the
        // file is checked in, so it must not churn on a run that found nothing
        // new. Provenance belongs in the sidecar, where it changes on purpose.
        if let Err(e) = std::fs::write(report_path(), &text) {
            eprintln!("could not write the fidelity report: {e}");
        }
        text
    }

    // ---------------------------------------------------------------- harness

    /// **F2–F5 over the corpus, one compile per model.**
    ///
    /// Every violation from every check on every subject is collected and
    /// reported together. The non-vacuity guards at the end are load-bearing:
    /// each check skips subjects it does not apply to, so without them a corpus
    /// that produced no blocks and no matching would pass in silence.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn hrw_reports_satisfy_the_structural_invariants() {
        let mut violations: Vec<String> = Vec::new();
        let mut subjects_checked = 0usize;
        let mut with_blocks = 0usize;
        let mut with_matching = 0usize;
        let mut with_singular_error = 0usize;

        let mut stage_values = 0usize;
        let mut report: Vec<FidelityRow> = Vec::new();

        for name in MODELS {
            let before = violations.len();
            let FromWorker::Compiled {
                stages, dae, equation_sheet, identifier_index, ..
            } = compile_specimen_shared(name)
            else {
                panic!("{name}: expected Compiled");
            };
            let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));
            let mut reduced = dae.clone();
            index_reduce_in_place(&mut reduced);

            // F6 — the views rebuilt from the DAE, against the DAE.
            violations.extend(
                check_f6(equation_sheet.as_ref(), identifier_index.as_ref(), &dae)
                    .into_iter()
                    .map(|msg| format!("{name} / F6: {msg}")),
            );

            // F7 — the capture vocabulary, on every stage's real IR.
            for kind in crate::worker::StageKind::COMPILATION {
                if let Some(root) = stages.get(*kind).value.as_ref() {
                    stage_values += 1;
                    violations.extend(
                        check_f7(&format!("{kind:?}"), root)
                            .into_iter()
                            .map(|msg| format!("{name} / F7: {msg}")),
                    );
                }
            }

            for s in subjects(&stages, &dae, &reduced) {
                subjects_checked += 1;
                if s.report["blocks"].as_array().is_some_and(|b| !b.is_empty()) {
                    with_blocks += 1;
                }
                if s.report["matching"].as_array().is_some_and(|m| !m.is_empty()) {
                    with_matching += 1;
                }
                if s.report["error"]["kind"] == "singular" {
                    with_singular_error += 1;
                }
                for check in [check_f2, check_f3, check_f4, check_f5] {
                    violations.extend(check(&s).into_iter().map(|msg| format!("{name} / {msg}")));
                }
            }

            // One row per model, green ones included: the report is a
            // certificate, so "no violations" must be readable from the file.
            report.push(FidelityRow {
                name: (*name).to_owned(),
                violations: violations[before..].to_vec(),
            });
        }

        let report_text = write_report(&report);

        // Printed rather than merely asserted: "0 violations" means nothing
        // without knowing how much was looked at, and a corpus that quietly
        // stopped producing subjects would otherwise read as a clean bill.
        println!(
            "fidelity F2-F7: {} models, {subjects_checked} incidence-bearing reports \
             ({with_blocks} with blocks, {with_matching} with a matching, \
             {with_singular_error} singular), {stage_values} stage IRs walked, \
             {} violations",
            MODELS.len(),
            violations.len(),
        );

        // **The emitted report loads through the shared loader** — asserted
        // *before* the violations, so a genuine finding cannot mask a broken
        // report format. `docs/reports.md`: one loader for all three reports,
        // which is a convention only if something checks it.
        let loaded = crate::report::parse(&report_text);
        assert!(loaded.has_shared_columns(), "the fidelity report lost a shared column");
        assert_eq!(
            loaded.rows.len(),
            MODELS.len(),
            "one row per model, green ones included — the report is a certificate",
        );
        assert_eq!(
            loaded.exceptions().len(),
            report.iter().filter(|r| !r.violations.is_empty()).count(),
            "the loader's exception filter disagrees with the rows that had violations",
        );

        assert!(
            violations.is_empty(),
            "{} structural-fidelity violations:\n  {}",
            violations.len(),
            violations.join("\n  "),
        );

        // Each check silently skips what it does not apply to, so prove each one
        // had something to look at.
        assert!(subjects_checked >= 20, "only {subjects_checked} incidence-bearing reports (F2, F3)");
        assert!(stage_values >= 50, "only {stage_values} stage IRs walked (F7)");
        assert!(with_blocks >= 5, "only {with_blocks} reports had BLT blocks (F4)");
        assert!(with_matching >= 5, "only {with_matching} reports had a matching (F5)");
        assert!(
            with_singular_error >= 1,
            "no singular error in the corpus; F3's rank-deficiency arithmetic never ran",
        );
    }

    /// Specimens authored to **fail**, and the corpus F8/F9 add to `MODELS`.
    ///
    /// F9 has no data without them, which is why the plan calls out failure
    /// coverage as its own gap (`docs/ideas.md` #46 tracks the phases still
    /// missing a failing specimen).
    const FAILING_MODELS: &[&str] = &[
        "UndefinedRef", "IncompatibleConnect", "DimensionMismatch",
        "CapacitorLoop", "OverInitRc", "UnbalancedShaft",
    ];

    /// **F8 — no stage panics, and the sizes are on the record.**
    ///
    /// The stress test as a *byproduct* rather than the goal: every model in both
    /// corpora is compiled and every stage serialized, so a panic anywhere in the
    /// pipeline or in HRW's own JSON construction fails the test by existing.
    ///
    /// Sizes are printed rather than bounded tightly. A tight bound on unfamiliar
    /// models would fail on a legitimately large one, which is the triage cost the
    /// invariant design exists to avoid — but a runaway (a cyclic structure, a
    /// stage serializing the whole MSL) is a different animal, and the loose
    /// ceiling catches it.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn every_stage_serializes_without_panicking() {
        /// Beyond this, something is structurally wrong rather than merely big.
        /// `Media.Examples.WaterIF97`'s flatten stage measured 3.2 MB.
        const CEILING: usize = 64 * 1024 * 1024;

        let mut rows: Vec<(String, usize, usize)> = Vec::new();
        let mut oversize = Vec::new();

        for name in MODELS.iter().chain(FAILING_MODELS) {
            let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            let mut total = 0usize;
            let mut largest = 0usize;
            for kind in crate::worker::StageKind::COMPILATION {
                let stage = stages.get(*kind);
                // Serializing here is the check: a stage whose JSON cannot be
                // rendered would panic or produce nothing.
                let bytes = stage.value.as_ref().map_or(0, |v| v.to_string().len());
                total += bytes;
                largest = largest.max(bytes);
                if bytes > CEILING {
                    oversize.push(format!("{name} / {kind:?}: {bytes} bytes"));
                }
            }
            rows.push(((*name).to_owned(), total, largest));
        }

        rows.sort_by_key(|(_, total, _)| std::cmp::Reverse(*total));
        println!("fidelity F8 — stage IR size per model (total, largest stage):");
        for (name, total, largest) in &rows {
            println!("  {name:<20} {total:>9} {largest:>9}");
        }

        assert!(oversize.is_empty(), "stage IR beyond the sanity ceiling:\n  {}", oversize.join("\n  "));
        assert_eq!(
            rows.len(),
            MODELS.len() + FAILING_MODELS.len(),
            "a model produced no row, so the loop exited early",
        );
    }

    /// **F9 — a Rumoca failure is represented faithfully too.**
    ///
    /// The plan's scope is "HRW tells the truth about Rumoca **even if Rumoca is
    /// wrong**", so a failure is as much in scope as a success — and it is where
    /// every hand-found bug of 2026-07-29–30 clustered: `rank_deficiency`
    /// computed as 7 when the truth was 1, spans Rumoca supplied and HRW dropped,
    /// labels dropped by every emitter, a `ToDae` failure reduced to a bare note.
    ///
    /// It is also the path a bug-PR demo runs through end to end, which is why
    /// `docs/fidelity-plan.md` ranks it first among the checks a Rumoca
    /// maintainer would notice.
    ///
    /// What is asserted, for each stage that did not reach `Outcome::Ok`:
    ///
    /// - **a note exists** — a failure with nothing to say is unusable
    /// - **a stage with no value has a note that is a real diagnosis**, since the
    ///   note is then all there is. This is the `"ToDae"` regression guard.
    /// - **a stage with a value carries real structure**, not an empty husk
    /// - **an `error` payload carries its message**, not a paraphrase
    /// - **every source location resolves** to a real line of the specimen, and
    ///   `line_text` is really that line
    ///
    /// # Two things the first draft got wrong, both worth keeping written down
    ///
    /// It demanded a structured payload from *every* abnormal stage. But
    /// `UndefinedRef`'s six downstream stages say "the pipeline produced no
    /// result", and Rumoca supplied **nothing** for them — so carrying nothing is
    /// faithful, not lossy. F9's property is that HRW must not *lose* structure
    /// Rumoca gave, which is only checkable where there was some.
    ///
    /// It also looked for that structure under `"error"` alone. `OverInitRc`'s
    /// over-determined initialization publishes a full IC plan with a
    /// `determinacy` verdict and no `error` key at all. A check that knows only
    /// one shape reports the other as missing.
    ///
    /// Both were the check inventing violations — which is the same failure mode
    /// as HRW inventing a decision, on the instrument rather than the subject.
    #[test]
    #[cfg_attr(not(feature = "slow-tests"), ignore = "compile-heavy; run with --features slow-tests")]
    fn a_rumoca_failure_is_represented_faithfully() {
        let mut violations = Vec::new();
        let mut abnormal_stages = 0usize;
        let mut with_payload = 0usize;
        let mut locations_checked = 0usize;

        for name in FAILING_MODELS {
            let source = std::fs::read_to_string(format!(
                "{}/specimens/{name}.mo",
                env!("CARGO_MANIFEST_DIR"),
            ))
            .unwrap_or_else(|e| panic!("{name}: {e}"));
            let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };

            for kind in crate::worker::StageKind::COMPILATION {
                let stage = stages.get(*kind);
                if stage.outcome == crate::worker::Outcome::Ok {
                    continue;
                }
                abnormal_stages += 1;
                let at = format!("{name} / {kind:?}");

                let Some(note) = stage.note.as_deref() else {
                    violations.push(format!("{at}: abnormal outcome with no note at all"));
                    continue;
                };

                let Some(value) = stage.value.as_ref() else {
                    // Nothing but the note, so the note has to carry the whole
                    // diagnosis. `"ToDae"` — a bare variant name — did not, and
                    // that is the regression this guards.
                    if note.trim().len() < 12 || !note.contains(' ') {
                        violations.push(format!(
                            "{at}: no value, and the note {note:?} is a label rather than \
                             a diagnosis — there is nothing else for the user to read",
                        ));
                    }
                    continue;
                };

                // A value must be real structure, not an empty husk that makes the
                // stage *look* diagnosed.
                match value.as_object() {
                    Some(o) if !o.is_empty() => with_payload += 1,
                    _ => violations.push(format!(
                        "{at}: note {note:?} with a value carrying no fields — the spans, \
                         labels and counts Rumoca reported would have nowhere to live",
                    )),
                }

                // Where there IS an error payload, it must carry its message
                // rather than a summary of it.
                if let Some(err) = stage.error_json()
                    && err["message"].as_str().unwrap_or_default().is_empty()
                {
                    violations.push(format!("{at}: an error payload with no message field"));
                }

                // Every location anywhere in the value must be a real line of
                // *this* specimen — not only inside `error`, since a determinacy
                // verdict or a diagnostic label carries them too.
                let lines: Vec<&str> = source.lines().collect();
                let mut stack = vec![value];
                while let Some(node) = stack.pop() {
                    match node {
                        Value::Array(items) => stack.extend(items),
                        Value::Object(map) => {
                            let is_location = map.contains_key("line") && map.contains_key("line_text");
                            if is_location {
                                locations_checked += 1;
                                let line = map["line"].as_u64().unwrap_or(0) as usize;
                                let text = map["line_text"].as_str().unwrap_or_default();
                                match lines.get(line.wrapping_sub(1)) {
                                    None => violations.push(format!(
                                        "{at}: blames line {line}, but the specimen has {} lines",
                                        lines.len(),
                                    )),
                                    Some(actual) if actual.trim_end() != text => {
                                        violations.push(format!(
                                            "{at}: line {line} quoted as {text:?}, the specimen \
                                             has {:?}",
                                            actual.trim_end(),
                                        ));
                                    }
                                    Some(_) => {}
                                }
                            }
                            stack.extend(map.values());
                        }
                        _ => {}
                    }
                }
            }
        }

        println!(
            "fidelity F9: {} failing specimens, {abnormal_stages} abnormal stages, \
             {with_payload} with a structured payload, {locations_checked} source \
             locations verified, {} violations",
            FAILING_MODELS.len(),
            violations.len(),
        );

        assert!(
            violations.is_empty(),
            "{} failure-representation violations:\n  {}",
            violations.len(),
            violations.join("\n  "),
        );
        // Floors set just under the measured values (14 / 8 / 6 at 2026-07-31), so a
        // real drop is loud while ordinary variation is not. Every assertion above
        // skips what it does not apply to, and a corpus that quietly stopped failing
        // would otherwise read as a clean bill of health.
        assert!(abnormal_stages >= 10, "only {abnormal_stages} abnormal stages; F9 barely ran");
        assert!(
            with_payload >= 5,
            "only {with_payload} abnormal stages carried structure; the payload checks \
             had almost nothing to inspect",
        );
        assert!(
            locations_checked >= 5,
            "only {locations_checked} source locations in the failure payloads — either \
             the specimens stopped failing the way they used to, or spans have been \
             dropped again, which is the ideas #45 regression",
        );
    }

    /// The checks can still fail.
    ///
    /// A harness reporting zero violations is exactly when to prove it is not
    /// simply blind — each of F2–F5 is handed a report violating it alone.
    #[test]
    fn each_invariant_catches_its_own_violation() {
        let base = serde_json::json!({
            "n_equations": 2, "n_unknowns": 2,
            "incidence": {
                "n_eq": 2, "n_var": 2,
                "unknown_names": ["x", "y"],
                "rows": [
                    { "equation": "f_x[0]", "unknowns": [0, 1] },
                    { "equation": "f_x[1]", "unknowns": [1] },
                ],
            },
            "matching": [
                { "equation": "f_x[0]", "unknown": "x" },
                { "equation": "f_x[1]", "unknown": "y" },
            ],
            "blocks": [
                { "kind": "scalar", "equation": "f_x[0]", "unknown": "x" },
                { "kind": "scalar", "equation": "f_x[1]", "unknown": "y" },
            ],
        });
        let clean = Subject { label: "T".into(), report: &base, dae: None };
        assert!(check_f3(&clean).is_empty(), "{:?}", check_f3(&clean));
        assert!(check_f4(&clean).is_empty(), "{:?}", check_f4(&clean));
        assert!(check_f5(&clean).is_empty(), "{:?}", check_f5(&clean));

        // F3: a count that disagrees with the incidence beside it.
        let mut bad = base.clone();
        bad["n_equations"] = serde_json::json!(7);
        assert!(
            !check_f3(&Subject { label: "T".into(), report: &bad, dae: None }).is_empty(),
            "F3 must notice n_equations disagreeing with the incidence",
        );

        // F4: an equation left out of every block.
        let mut bad = base.clone();
        bad["blocks"] = serde_json::json!([{ "kind": "scalar", "equation": "f_x[0]" }]);
        assert!(
            !check_f4(&Subject { label: "T".into(), report: &bad, dae: None }).is_empty(),
            "F4 must notice an equation covered by no block",
        );

        // F5: a pair whose equation does not reference its unknown — `f_x[1]`
        // touches only column 1, so matching it to `x` is not a matching.
        let mut bad = base.clone();
        bad["matching"] = serde_json::json!([{ "equation": "f_x[1]", "unknown": "x" }]);
        assert!(
            !check_f5(&Subject { label: "T".into(), report: &bad, dae: None }).is_empty(),
            "F5 must notice a pair that is not an incidence non-zero",
        );

        // F5: the same unknown claimed twice.
        let mut bad = base.clone();
        bad["matching"] = serde_json::json!([
            { "equation": "f_x[0]", "unknown": "y" },
            { "equation": "f_x[1]", "unknown": "y" },
        ]);
        assert!(
            !check_f5(&Subject { label: "T".into(), report: &bad, dae: None }).is_empty(),
            "F5 must notice an unknown matched twice",
        );

        // F2 without a DAE is a skip, not a pass by accident.
        assert!(check_f2(&clean).is_empty(), "F2 skips when there is no DAE to compare against");
    }
}
