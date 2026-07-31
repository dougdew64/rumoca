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

#[cfg(test)]
use serde_json::Value;

/// A report HRW publishes that carries an incidence matrix, together with the
/// DAE it claims to describe.
///
/// `dae` is `None` when the subject's system is not one we hold — nothing in
/// the bundle is like that today, but F2 skips rather than guesses if it
/// appears, because comparing against the wrong DAE would manufacture failures.
#[cfg(test)]
struct Subject<'a> {
    /// Where this report lives, for a violation message: `Structural`,
    /// `IndexReduction`, `IndexReduction.before`.
    label: String,
    report: &'a Value,
    dae: Option<&'a rumoca_ir_dae::Dae>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};

    use crate::worker::{FromWorker, StageBundle, compile_specimen_shared, index_reduce_in_place};

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

    /// Every incidence-bearing report in the bundle, with the DAE it describes.
    ///
    /// The Structural stage describes the **raw** system and Index Reduction the
    /// **reduced** one — the distinction that made F1's first tearing check fail
    /// for the wrong reason. `before` is the raw system again, published inside
    /// the reduced stage.
    fn subjects<'a>(
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
    fn check_f2(s: &Subject) -> Vec<String> {
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
    fn check_f3(s: &Subject) -> Vec<String> {
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
    fn check_f4(s: &Subject) -> Vec<String> {
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
    fn check_f5(s: &Subject) -> Vec<String> {
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

        for name in MODELS {
            let FromWorker::Compiled { stages, dae, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected Compiled");
            };
            let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));
            let mut reduced = dae.clone();
            index_reduce_in_place(&mut reduced);

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
        }

        // Printed rather than merely asserted: "0 violations" means nothing
        // without knowing how much was looked at, and a corpus that quietly
        // stopped producing subjects would otherwise read as a clean bill.
        println!(
            "fidelity F2-F5: {} models, {subjects_checked} incidence-bearing reports \
             ({with_blocks} with blocks, {with_matching} with a matching, \
             {with_singular_error} singular), {} violations",
            MODELS.len(),
            violations.len(),
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
        assert!(with_blocks >= 5, "only {with_blocks} reports had BLT blocks (F4)");
        assert!(with_matching >= 5, "only {with_matching} reports had a matching (F5)");
        assert!(
            with_singular_error >= 1,
            "no singular error in the corpus; F3's rank-deficiency arithmetic never ran",
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
