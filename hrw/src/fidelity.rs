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

/// **Every check name `--only-checks` accepts** — the roster, in one place.
///
/// # Why this exists, and it cost a whole sweep
///
/// On 2026-11-05 a 2.5-hour corpus run was launched with
/// `--only-checks 'F1,F2,F3,F5,F6,F7,F8,F9'`, written to exclude the blind F4. That
/// string also **excluded F10**; named **F1** and **F8**, which are not checks at
/// all; and named **F9**, which [`check_model`] cannot gate. Five checks ran, the
/// report was read as though eight had, and nothing said otherwise — the flag
/// accepted every name and silently selected nothing for the ones it did not know.
///
/// A check name is data about the code, so it belongs in one list that
/// `unknown_checks` validates against, the same discipline as `StageKind::ALL` and
/// `FloatingWindow::ALL` and for the same reason: **the roster is the thing that
/// turns a stale name into an error instead of a quieter sweep.**
///
/// # What is deliberately NOT here
///
/// **F1 is not implemented.** Every `F1` left in this crate is prose; the module doc
/// above says it "lives in `worker.rs`", where the one remaining mention is a remark
/// about a bug it once found. **F8 is not a check** — it is the size accounting in
/// `examples/fidelity_msl.rs`, which raises no violation and is covered by
/// `every_stage_serializes_without_panicking`. Both are rejected on purpose, so that
/// asking for them fails rather than quietly measuring nothing.
pub const CHECKS: &[&str] = &["F2", "F3", "F4", "F5", "F6", "F7", "F9", "F10"];

/// The roster entries enforced by the runner instead of by [`check_model`].
///
/// **Empty, and worth keeping empty.** F9 was the only entry: it lived in
/// `examples/fidelity_msl.rs` because it needs the whole [`StageBundle`] rather than a
/// subject — which also put it outside `--only-checks` entirely, and out of reach of
/// `cargo test`. [`check_f10`] has the same signature and never left this file, so the
/// separation bought nothing; F9 moved here on 2026-11-06 and `check_model` gates it.
///
/// The constant stays because the mechanism is the point: if a future check must be
/// enforced by the runner, listing it here is what keeps
/// `the_check_roster_matches_the_code` honest instead of forcing an exemption into the
/// test itself. A gate that lives outside this file must still be *declared* in it.
pub const CHECKS_OWNED_BY_RUNNER: &[&str] = &[];

/// The requested names that are **not** on the [`CHECKS`] roster.
///
/// Returned rather than reported, so the caller can name all of them in one message
/// and a must-fire test can drive the predicate directly. Sorted and de-duplicated.
///
/// An empty result means every name was recognised. **It does not mean the selection
/// is useful**: a caller can still ask for a valid subset and omit a check that
/// matters, which is why the runner prints the checks it is *skipping* as well as the
/// ones it will run.
pub fn unknown_checks<'a>(requested: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut bad: Vec<String> = requested
        .into_iter()
        .filter(|c| !CHECKS.contains(c))
        .map(str::to_owned)
        .collect();
    bad.sort_unstable();
    bad.dedup();
    bad
}

/// Run every subject-based and view-based check on one compiled model.
///
/// **The `reduced` parameter is gone for the v0.10.0 rebase, and the reason it
/// existed is worth keeping.** It was `Option` because index reduction is
/// capped by system size, exactly as the survey caps it: passing `None` omitted
/// the `IndexReduction` subjects so a 10,175-equation model still contributed
/// F2, F3, F5, F6 and F7 on its `Structural` subject rather than dropping out of
/// the corpus. **That cap must come back with the re-model** — it is the
/// difference between a corpus sweep that covers the large models and one that
/// silently excludes them.
///
/// There is nothing to pass today: the reduced DAE came from re-running
/// reduction over a clone, and 0.10.0's `Dae` is immutable and not `Clone`.
///
/// F8 (sizes) and F9 (failure faithfulness) are the runner's business, not this
/// function's: they need the whole bundle rather than a subject.
pub fn check_model(
    stages: &StageBundle,
    dae: &rumoca_ir_dae::Dae,
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

    // `subjects` yields only the `Structural` subject now, so the
    // raw-vs-reduced skip that used to live here has nothing to skip — see
    // its own note for why passing the raw DAE as the reduced one would be
    // worse than omitting the subject.
    for s in subjects(stages, dae) {
        coverage.subjects += 1;
        if s.report["blocks"].as_array().is_some_and(|b| !b.is_empty()) {
            coverage.with_blocks += 1;
        }
        if s.report["matching"]
            .as_array()
            .is_some_and(|m| !m.is_empty())
        {
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
    if want("F9") {
        let t = std::time::Instant::now();
        out.extend(tag("F9", check_f9(stages)));
        timing.add("F9", t);
    }
    if want("F10") {
        let t = std::time::Instant::now();
        // Counted whether or not F10 finds anything: the question this answers is
        // "did the clause have anything to act on", which a violation count cannot.
        coverage.empty_stages += crate::worker::StageKind::COMPILATION
            .iter()
            .filter(|k| stages.get(**k).value.is_none())
            .count();
        out.extend(tag("F10", check_f10(stages)));
        timing.add("F10", t);
    }
    out
}

/// **F10 — a pane's claim about the compiler, checked at corpus scale.**
///
/// The first check in this file about a **verb** rather than a noun. F1-F9 all ask
/// *is this structure what Rumoca produced?* — and on 2026-08-04 that programme
/// reported 2,614 models green while HRW's log and UI carried fictions, because a
/// fabricated structure is well-formed and a good replay is indistinguishable by
/// construction ([`fidelity-plan.md`](../docs/fidelity-plan.md)'s scope table).
///
/// Charter Decision 7 makes three claims about every pane. This checks all three on
/// every stage of every model:
///
/// 1. **Nothing HRW invented is on screen.** `Provenance::Hrw` is reachable only
///    through `Stage::computed`, which demands a written reason; no production pane
///    calls it. A violation here is not necessarily a defect — it means a pane began
///    deriving its own content, and the same commit has to say how the UI declares
///    that.
/// 2. **Provenance and content agree.** `Empty` exactly when there is no value, so
///    "this pane is showing nothing" stops being a judgement made by reading it.
/// 3. **Absence is stated, never silent.** A pane with no content must say why. This
///    is the one that generalises the fix applied tab-by-tab that day: a blank pane
///    and a pane whose phase never ran look identical, and the reader has no way to
///    tell which they are looking at.
///
/// **Not a duplicate of F9**, which is the nearest neighbour. F9 inspects only
/// stages whose `outcome` is *abnormal* and requires those to carry a real note.
/// F10's third clause covers the stages F9 skips: an outcome of `Ok` with no value
/// and no note — the `Stage::default()` state, which says nothing while claiming
/// nothing went wrong. **The pane that misleads most is the one that looks fine.**
///
/// **Why the corpus and not a unit test.** `no_stage_shows_content_hrw_invented`
/// already checks 1 and 2 on four specimens. Four specimens reach four shapes of
/// failure; the corpus reaches 2,626, including every way a compile can stop early —
/// which is precisely when a pane has nothing to show and must say so.
pub fn check_f10(stages: &StageBundle) -> Vec<String> {
    let mut out = Vec::new();
    for kind in crate::worker::StageKind::COMPILATION {
        let stage = stages.get(*kind);
        let name = kind.name();
        if stage.provenance == crate::worker::Provenance::Hrw {
            out.push(format!(
                "{name}: shows content HRW produced rather than the compiler's. That may \
                 be legitimate, but it must be declared where the user can see it"
            ));
        }
        let empty = stage.provenance == crate::worker::Provenance::Empty;
        if empty != stage.value.is_none() {
            out.push(format!(
                "{name}: provenance says {:?} while value.is_none() is {} \u{2014} these \
                 must agree",
                stage.provenance,
                stage.value.is_none(),
            ));
        }
        if stage.value.is_none() && stage.note.is_none() {
            out.push(format!(
                "{name}: shows nothing and says nothing. A pane with no content must \
                 state why \u{2014} silence is indistinguishable from a phase that ran \
                 and produced an empty result"
            ));
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
    /// **Stages with no value — the only thing F10's third clause can act on.**
    ///
    /// Added 2026-08-05, after the first corpus run reported 0 F10 violations and it
    /// turned out **no MSL model produces an empty stage at all**. Two of F10's three
    /// clauses are near-tautological in production; the third is the one carrying the
    /// check, and it has nothing to do unless a stage is empty. Without this counter
    /// the run could not distinguish *"absence is always stated"* from *"absence never
    /// occurred"*.
    ///
    /// **F10 shipped without it, which is the failure this whole type exists to
    /// prevent** — the doc above says a corpus that produced no blocks and no
    /// matchings would pass in silence, and that is exactly what happened one level
    /// over.
    pub empty_stages: usize,
}

/// Violations grouped by check, most numerous first — the triage view.
///
/// Returns `(check, count, up to `examples` details)`.
pub fn group_by_check(
    violations: &[Violation],
    examples: usize,
) -> Vec<(&'static str, usize, Vec<String>)> {
    let mut by: BTreeMap<&'static str, Vec<&str>> = BTreeMap::new();
    for v in violations {
        by.entry(v.check).or_default().push(&v.detail);
    }
    let mut out: Vec<(&'static str, usize, Vec<String>)> = by
        .into_iter()
        .map(|(check, ds)| {
            let n = ds.len();
            (
                check,
                n,
                ds.into_iter().take(examples).map(str::to_owned).collect(),
            )
        })
        .collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    out
}

/// Every incidence-bearing report in the bundle, with the DAE it describes.
///
/// **One DAE, not two, for the v0.10.0 rebase.** This took the raw system *and*
/// the index-reduced one, because the Structural stage describes the raw system
/// and Index Reduction the reduced one — a distinction that made F1's first
/// tearing check fail for the wrong reason, and which still matters.
///
/// It is one now because the reduced DAE cannot be had: it came from
/// `worker::index_reduce_in_place` re-running reduction over a *clone*, and
/// 0.10.0's `Dae` is immutable and not `Clone`. The Index Reduction stage is
/// disabled accordingly (`docs/tech-debt.md`), so it publishes no incidence and
/// would yield no subject even if a DAE were passed for it.
///
/// **The parameter is removed rather than passed the raw DAE twice**, which
/// would have compiled and been a lie: the `IndexReduction` subject would then
/// claim the raw system's incidence was the reduced one's, and every F2
/// comparison under that label would pass by describing the wrong system. When
/// the re-model lands it will supply a real reduced DAE and this signature
/// grows back.
pub fn subjects<'a>(stages: &'a StageBundle, dae: &'a rumoca_ir_dae::Dae) -> Vec<Subject<'a>> {
    let mut out = Vec::new();
    let mut push = |label: &str, report: Option<&'a Value>, d: &'a rumoca_ir_dae::Dae| {
        if let Some(r) = report
            && r.get("incidence").is_some()
        {
            out.push(Subject {
                label: label.to_owned(),
                report: r,
                dae: Some(d),
            });
        }
    };
    push("Structural", stages.structural.value.as_ref(), dae);
    // `IndexReduction` and `IndexReduction.before` are not pushed. Both
    // described systems only the disabled stage produced; see the note above
    // for why passing `dae` for them would be worse than omitting them.
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
        .map(|ns| {
            ns.iter()
                .map(|n| n.as_str().unwrap_or_default().to_owned())
                .collect()
        })
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
                        .map(|u| {
                            u.iter()
                                .filter_map(Value::as_u64)
                                .map(|v| v as usize)
                                .collect()
                        })
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
///
/// # Two v0.10.0 changes, and one of them is a finding rather than a port
///
/// `build_incidence` takes a `DaeView` and returns a `Result`, so the whole
/// check runs inside `Dae::inspect`. **A derivation failure is reported, not
/// skipped**: `subjects()` only yields a subject when a report actually
/// published an incidence, so a DAE whose incidence cannot be built while HRW
/// published one for it is a genuine discrepancy. Returning no findings there
/// would be a fidelity check passing vacuously, which is the exact failure this
/// suite exists to catch.
///
/// `Incidence::unknown_names` is gone; the unknowns are `UnknownId`s now and
/// are rendered through `unknown_label`, the same function the published view
/// uses. That is the stronger comparison — the old one compared against
/// whatever `unknown_names` happened to hold, while this compares against the
/// renderer the publisher itself calls.
pub fn check_f2(s: &Subject) -> Vec<String> {
    let Some(dae) = s.dae else { return Vec::new() };
    dae.inspect(|view| {
        let mut v = Vec::new();
        let inc = match rumoca_phase_structural::build_incidence(view) {
            Ok(inc) => inc,
            Err(e) => {
                v.push(format!(
                    "{}: build_incidence failed ({e}), but an incidence was published for \
                     this DAE — the published matrix cannot have come from this analysis",
                    s.label,
                ));
                return v;
            }
        };

        let (n_eq, n_var) = (
            s.report["incidence"]["n_eq"].as_u64().unwrap_or(u64::MAX) as usize,
            s.report["incidence"]["n_var"].as_u64().unwrap_or(u64::MAX) as usize,
        );
        if n_eq != inc.n_eq {
            v.push(format!(
                "{}: n_eq {n_eq} published, {} from build_incidence",
                s.label, inc.n_eq
            ));
        }
        if n_var != inc.n_var {
            v.push(format!(
                "{}: n_var {n_var} published, {} from build_incidence",
                s.label, inc.n_var
            ));
        }

        let published_unknowns = unknown_names(s.report);
        let real_unknowns: Vec<String> = inc
            .unknowns
            .iter()
            .map(|unknown| rumoca_phase_structural::unknown_label(view, *unknown))
            .collect();
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
            .map(|i| rumoca_phase_structural::equation_label(view, &inc.equation_refs[i]))
            .collect();
        if published_labels != real_labels {
            v.push(format!(
                "{}: row equation labels differ from equation_label",
                s.label
            ));
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
                None => v.push(format!(
                    "{}: row {i} missing; only {} published",
                    s.label,
                    published_rows.len()
                )),
            }
        }
        v
    })
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
            v.push(format!(
                "{}: {key} = {got}, incidence says {expect}",
                s.label
            ));
        }
    }

    let n_matched = s.report["matching"].as_array().map(Vec::len);
    if let (Some(m), Some(published)) = (n_matched, s.report["n_matched"].as_u64())
        && m != published as usize
    {
        v.push(format!(
            "{}: n_matched = {published}, but matching has {m} pairs",
            s.label
        ));
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
            v.push(format!(
                "{}: rank_deficiency {rd} != max({ne},{nu}) - {nm}",
                s.label
            ));
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
    let Some(blocks) = s.report["blocks"].as_array() else {
        return Vec::new();
    };
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
            v.push(format!(
                "{}: equation row {row} appears in {count} blocks",
                s.label
            ));
        }
    }
    if seen.len() != labels.len() {
        let missing: Vec<usize> = (0..labels.len())
            .filter(|i| !seen.contains_key(i))
            .collect();
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
    let Some(pairs) = s.report["matching"].as_array() else {
        return Vec::new();
    };
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
            v.push(format!(
                "{}: matching pair {i} is not a (equation, unknown) pair",
                s.label
            ));
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
    // `continuous_equation_count()` is the 0.10.0 spelling of the old
    // `dae.continuous.equations.len()`; both count scalar residual rows.
    let n_eq = dae.inspect(|view| view.continuous_equation_count());

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
            v.push(format!(
                "equation sheet lists equation {i}, beyond the DAE's {n_eq}"
            ));
        }
    }

    if let Some(index) = index {
        // Keyed by the `kind` the index itself recorded, so this check does
        // not merely re-list the DAE's roles and drift. The first draft did
        // exactly that — it omitted the two discrete partitions and reported
        // `BouncingBall`'s `c` as a phantom variable.
        //
        // The label comes from `identifier_index::role_label`, the single home
        // for the role → label mapping, so this check and the index cannot
        // disagree about a spelling. That makes the unknown-kind arm below
        // *narrower* than it was: a role added upstream now fails to compile in
        // `role_label` rather than arriving here, so the arm fires only for an
        // index entry carrying a kind string `role_label` never produces.
        //
        // Every DAE variable is counted, not just the specimen's — the index is
        // deliberately a subset (it filters on declaration source), and the
        // checks below ask only whether each indexed name is present under the
        // role it was filed as.
        let mut partitions: BTreeMap<&'static str, BTreeSet<String>> = BTreeMap::new();
        dae.inspect(|view| {
            for (_id, var) in view.variables() {
                partitions
                    .entry(crate::identifier_index::role_label(var.role()))
                    .or_default()
                    .insert(var.name().to_string());
            }
        });

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
        let Some(node) = crate::bridge::navigate(root, &path) else {
            continue;
        };
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

/// **F9 — a stage that went abnormal says something usable about why.**
///
/// Rumoca is a recovering compiler: almost nothing hard-fails, problems are *flagged*
/// and the pipeline continues. That makes the note on an abnormal stage the only thing
/// the reader has, so this check holds it to three conditions — it exists, it is a
/// sentence rather than a label, and a value published beside it carries fields.
///
/// # Why it moved here on 2026-11-06
///
/// It lived in `examples/fidelity_msl.rs` as `check_failures`, which had two costs.
/// It sat **outside `check_model`**, so `--only-checks` could not gate it: asking for
/// F9 did not select it and omitting F9 did not skip it. And a check in an example is
/// not reachable from `cargo test`, which is why F9 was one of the checks nothing had
/// ever driven with a deliberate violation.
///
/// It only ever needed `stages`, exactly like [`check_f10`], so there was nothing to
/// justify the separation.
pub fn check_f9(stages: &StageBundle) -> Vec<String> {
    let mut v = Vec::new();
    for kind in crate::worker::StageKind::COMPILATION {
        let stage = stages.get(*kind);
        if stage.outcome == crate::worker::Outcome::Ok {
            continue;
        }
        let Some(note) = stage.note.as_deref() else {
            v.push(format!("{kind:?}: abnormal with no note"));
            continue;
        };
        match stage.value.as_ref() {
            None => {
                if note.trim().len() < 12 || !note.contains(' ') {
                    v.push(format!("{kind:?}: no value and note {note:?} is a label"));
                }
            }
            Some(value) => {
                if value.as_object().is_none_or(serde_json::Map::is_empty) {
                    v.push(format!(
                        "{kind:?}: note {note:?} with a value carrying no fields"
                    ));
                }
            }
        }
    }
    v
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

    use crate::worker::FromWorker;
    use crate::worker::test_msl::compile_specimen_shared;

    /// The specimens the invariants run over.
    ///
    /// Deliberately the same list F1 uses, so a model added for one check is
    /// covered by all of them. This is the stand-in for the stratified MSL
    /// sample `docs/fidelity-plan.md` describes — the harness is written to take
    /// any list of names, so growing the corpus is a one-line change.
    const MODELS: &[&str] = &[
        "ProportionalLoop",
        "MixedLoop",
        "TwoLoops",
        "NonlinearLoop",
        "Drivetrain",
        "RcCircuit",
        "SingleInertia",
        "CapacitorLoop",
        "BouncingBall",
        "MotorWithBrake",
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
        // **Named for its scope, deliberately.** This is the 10-specimen pre-commit
        // gate. The MSL corpus artifact is `docs/reports/msl-fidelity-report.csv`, written
        // by `scripts/promote-run.ps1` — two reports with the same name but different
        // scopes would be confused exactly once, expensively.
        std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/reports/specimen-fidelity-report.csv"
        ))
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

            let outcome = if self.violations.is_empty() {
                "ok"
            } else {
                "violations"
            };
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
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
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
                stages,
                dae,
                equation_sheet,
                identifier_index,
                ..
            } = compile_specimen_shared(name)
            else {
                panic!("{name}: expected Compiled");
            };
            let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));

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

            for s in subjects(&stages, &dae) {
                subjects_checked += 1;
                if s.report["blocks"].as_array().is_some_and(|b| !b.is_empty()) {
                    with_blocks += 1;
                }
                if s.report["matching"]
                    .as_array()
                    .is_some_and(|m| !m.is_empty())
                {
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
             {with_singular_error} singular), {stage_values} stage IRs run, \
             {} violations",
            MODELS.len(),
            violations.len(),
        );

        // **The emitted report loads through the shared loader** — asserted
        // *before* the violations, so a genuine finding cannot mask a broken
        // report format. `docs/reports.md`: one loader for all three reports,
        // which is a convention only if something checks it.
        let loaded = crate::report::parse(&report_text);
        assert!(
            loaded.has_shared_columns(),
            "the fidelity report lost a shared column"
        );
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
        // **One subject per model, exactly — and that is stronger than the floor it
        // replaces.**
        //
        // This read `>= 20` for ten models, because `subjects` yielded **two**
        // incidence-bearing reports each: the raw system and the index-reduced one.
        // v0.10.0's reduced DAE is not a surface HRW holds, so `subjects` lost that
        // parameter and there is one report per model.
        //
        // Pinned as equality rather than lowered to `>= 10`. A floor of ten would
        // pass if one model stopped producing a report while another started
        // producing two; equality against `MODELS.len()` cannot, and *"a corpus that
        // quietly stopped producing subjects reads as a clean bill"* is the hazard
        // this guard exists for. **When the reduced DAE is re-modelled this becomes
        // 2x and must be rewritten deliberately** rather than nudged.
        assert_eq!(
            subjects_checked,
            MODELS.len(),
            "F2/F3 must see exactly one incidence-bearing report per model",
        );
        assert!(stage_values >= 50, "only {stage_values} stage IRs run (F7)");
        assert!(
            with_blocks >= 5,
            "only {with_blocks} reports had BLT blocks (F4)"
        );
        assert!(
            with_matching >= 5,
            "only {with_matching} reports had a matching (F5)"
        );
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
        "UndefinedRef",
        "IncompatibleConnect",
        "DimensionMismatch",
        "CapacitorLoop",
        "OverInitRc",
        "UnbalancedShaft",
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
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
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

        assert!(
            oversize.is_empty(),
            "stage IR beyond the sanity ceiling:\n  {}",
            oversize.join("\n  ")
        );
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
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn a_rumoca_failure_is_represented_faithfully() {
        let mut violations = Vec::new();
        let mut abnormal_stages = 0usize;
        let mut with_payload = 0usize;
        let mut locations_checked = 0usize;
        // Which abnormal stages carried a located entry and which did not. The
        // count alone cannot distinguish "a span was dropped" from "this failure
        // has no line to point at", and the assertion below needs that distinction
        // to be readable rather than guessed.
        let mut located: Vec<String> = Vec::new();
        let mut unlocated: Vec<String> = Vec::new();
        // Source text per file a location names, so the vendored MSL is read once
        // across the whole corpus rather than once per location.
        let mut file_cache: std::collections::HashMap<String, Option<String>> =
            std::collections::HashMap::new();

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

                // Every location anywhere in the value must be a real line of **the
                // file it names** — not only inside `error`, since a determinacy
                // verdict or a diagnostic label carries them too.
                //
                // # This check used to assume the specimen, and that is why it
                // # certified the bug instead of catching it
                //
                // It read `source.lines()` and compared `line_text` against the
                // *specimen's* line, for every location from every stage. Under the
                // old resolver both numbers came out of the specimen, so they agreed
                // — and this check reported "{n} source locations verified" over
                // locations naming the wrong file entirely. `CapacitorLoop`'s
                // `gnd.p.i` was certified as line 9 of `CapacitorLoop.mo` when it is
                // `flow SI.Current i` in the MSL's `Interfaces/Pin.mo`; see
                // `worker::span_location`.
                //
                // **A location with no `file` is now a violation in itself**, which
                // is the shape the old resolver produced: there is no way to verify
                // a line number that does not say which file it counts lines in, and
                // "cannot be verified" must not read as "verified".
                let before_locations = locations_checked;
                let mut stack = vec![value];
                while let Some(node) = stack.pop() {
                    match node {
                        Value::Array(items) => stack.extend(items),
                        Value::Object(map) => {
                            let is_location =
                                map.contains_key("line") && map.contains_key("line_text");
                            if is_location {
                                locations_checked += 1;
                                let line = map["line"].as_u64().unwrap_or(0) as usize;
                                let text = map["line_text"].as_str().unwrap_or_default();
                                // The file the location claims. Read from disk and
                                // memoized, because a span may name any file the
                                // compile registered — the vendored MSL included.
                                let named = map.get("file").and_then(Value::as_str);
                                let contents = match named {
                                    None => {
                                        violations.push(format!(
                                            "{at}: a location quoting line {line} with no \
                                             `file` \u{2014} nothing says which file that \
                                             line is in, so it cannot be verified",
                                        ));
                                        None
                                    }
                                    Some(path) if path.ends_with(&format!("{name}.mo")) => {
                                        Some(source.clone())
                                    }
                                    Some(path) => match file_cache.entry(path.to_owned()) {
                                        std::collections::hash_map::Entry::Occupied(e) => {
                                            e.get().clone()
                                        }
                                        std::collections::hash_map::Entry::Vacant(e) => {
                                            // Published names are workspace-relative
                                            // (`worker::portable_source_name`), so they
                                            // are resolved against the workspace root
                                            // rather than the process's cwd.
                                            let root =
                                                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                                    .parent()
                                                    .expect("the crate dir has a parent")
                                                    .to_path_buf();
                                            e.insert(std::fs::read_to_string(root.join(path)).ok())
                                                .clone()
                                        }
                                    },
                                };
                                let Some(contents) = contents else {
                                    if let Some(path) = named {
                                        violations.push(format!(
                                            "{at}: a location names {path}, which cannot be \
                                             read \u{2014} the quote cannot be checked",
                                        ));
                                    }
                                    stack.extend(map.values());
                                    continue;
                                };
                                let lines: Vec<&str> = contents.lines().collect();
                                let where_ = named.unwrap_or("<unnamed>");
                                match lines.get(line.wrapping_sub(1)) {
                                    None => violations.push(format!(
                                        "{at}: blames line {line}, but {where_} has {} lines",
                                        lines.len(),
                                    )),
                                    Some(actual) if actual.trim_end() != text => {
                                        violations.push(format!(
                                            "{at}: line {line} quoted as {text:?}, but \
                                             {where_} has {:?}",
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
                if locations_checked > before_locations {
                    located.push(at.clone());
                } else {
                    unlocated.push(at.clone());
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
        // 10 → 6 on 2026-08-25, and the drop is the population being corrected rather
        // than coverage being lost. The C20 fix stopped eight stages claiming
        // `Outcome::Failed` — which its own definition reads as *"the pipeline stopped
        // here"* — for a pipeline that had stopped once somewhere else. Those stages
        // were counted here as abnormal, so the measured 14 included samples that were
        // abnormal by mistake, and it is now 8.
        //
        // **What did NOT change is the assertion below**: `with_payload` is still over
        // its floor, because every stage removed carried a bare formatted string and
        // no structured error. They were the samples F9 learns least from — the shape
        // its own doc calls "a stage that failed with nothing but a formatted string".
        assert!(
            abnormal_stages >= 6,
            "only {abnormal_stages} abnormal stages; F9 barely ran"
        );
        assert!(
            with_payload >= 5,
            "only {with_payload} abnormal stages carried structure; the payload checks \
             had almost nothing to inspect",
        );
        // **4 since 2026-11-03, and the drop from 5 was traced before it was
        // accepted.** This assertion's own message names the risk — *"spans have
        // been dropped again, which is the ideas #45 regression"* — so lowering it
        // without an account would be exactly the move it exists to prevent.
        //
        // What the breakdown showed: the four located stages are unchanged
        // (`UndefinedRef / Resolve`, `DimensionMismatch / Typecheck`, and
        // `IncompatibleConnect` and `CapacitorLoop` at `Structural`). The fifth was
        // the **index-reduction copy** of CapacitorLoop's singular payload, and that
        // stage is on `worker::AWAITING_REMODEL` — the same duplicate removed from
        // `a_structural_failure_names_the_source_line`. No span was dropped; a
        // second report of one stopped being published.
        //
        // The two unlocated stages are unlocated *correctly*. `UnbalancedShaft / Dae`
        // says "2 equations, 3 unknowns" and `IncompatibleConnect / SolveLowering`
        // says "structurally singular system: 1 matched out of 2" — both are
        // properties of a whole system, with no one line to blame. The second is new
        // on v0.10.0, which refuses to lower a singular system rather than lowering
        // it anyway.
        //
        // **The breakdown stays in the message**, because it is what made a count
        // diagnosable: "4, down from 5" says nothing, while naming the stages
        // distinguishes a lost span from a failure that never had one.
        assert!(
            locations_checked >= 4,
            "located: {located:?}; unlocated: {unlocated:?}\n\
             only {locations_checked} source locations in the failure payloads — either \
             the specimens stopped failing the way they used to, or spans have been \
             dropped again, which is the ideas #45 regression",
        );
    }

    /// **A stage claiming it never ran must not also show IR.**
    ///
    /// The *declined* verb — the third the fidelity programme does not reach. F1–F9 ask
    /// whether a structure matches what Rumoca produced; this asks whether a stage's
    /// claim about **not** having produced anything is consistent with what it shows.
    /// *"Not reached (Flatten failed earlier)"* beside a populated tree is a
    /// contradiction on screen, and the value is the half a reader believes.
    ///
    /// # The check I set out to write was wrong, and the data said so
    ///
    /// The intended claim was *"every stage after the first failure says it never
    /// ran"*. It fails immediately, and instructively: **the pipeline continues past a
    /// stage-level error.** `IncompatibleConnect` reports structurally singular at
    /// Structural, and Index Reduction, Initialization, Events and Solve Lowering all
    /// genuinely **run** and produce IR afterwards. *"First stage with an error note"*
    /// is what `compile_outcome` reports, not where the pipeline stopped —
    /// `not_reached_stage` fires on a failed `PhaseResult`, which is a different and
    /// rarer condition.
    ///
    /// So the premise was a wrong model of the compiler, corrected by running it. What
    /// survives is the half that does not depend on the premise: **wherever a stage
    /// does say it never ran, it must show nothing** — recorded here rather than
    /// quietly weakened, because a test fitted to its data until it passes has stopped
    /// being evidence.
    ///
    /// Its sibling [`a_rumoca_failure_is_represented_faithfully`] checks that an
    /// abnormal stage carries a *diagnosis* rather than a label; it says nothing about
    /// whether the diagnosis and the payload agree.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn a_stage_that_says_it_never_ran_shows_no_ir() {
        let mut violations = Vec::new();
        let mut not_reached = 0usize;

        for name in FAILING_MODELS {
            let crate::worker::FromWorker::Compiled { stages, .. } =
                crate::worker::test_msl::compile_specimen_shared(name)
            else {
                panic!("expected Compiled for {name}");
            };

            for kind in crate::worker::StageKind::COMPILATION {
                let stage = stages.get(*kind);
                let Some(note) = stage.note.as_deref() else {
                    continue;
                };
                // The two spellings `not_reached_stage` produces.
                if !note.contains("not reached") && !note.contains("produced no result") {
                    continue;
                }
                not_reached += 1;

                if let Some(value) = stage.value.as_ref() {
                    violations.push(format!(
                        "{name} / {kind:?}: the note says it never ran \u{2014} {note:?} \
                         \u{2014} and yet it carries IR: {}. Whatever is in that value \
                         came from somewhere other than this phase on this run, which is \
                         the one thing the pane must never show.",
                        value.to_string().chars().take(80).collect::<String>(),
                    ));
                }
            }
        }

        assert!(
            not_reached >= 3,
            "only {not_reached} stage(s) across {} failing specimens said they never ran \
             \u{2014} either every pipeline now completes, or this is reading nothing",
            FAILING_MODELS.len(),
        );
        assert!(violations.is_empty(), "{}", violations.join("\n  "));
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
        let clean = Subject {
            label: "T".into(),
            report: &base,
            dae: None,
        };
        assert!(check_f3(&clean).is_empty(), "{:?}", check_f3(&clean));
        assert!(check_f4(&clean).is_empty(), "{:?}", check_f4(&clean));
        assert!(check_f5(&clean).is_empty(), "{:?}", check_f5(&clean));

        // F3: a count that disagrees with the incidence beside it.
        let mut bad = base.clone();
        bad["n_equations"] = serde_json::json!(7);
        assert!(
            !check_f3(&Subject {
                label: "T".into(),
                report: &bad,
                dae: None
            })
            .is_empty(),
            "F3 must notice n_equations disagreeing with the incidence",
        );

        // F4: an equation left out of every block.
        let mut bad = base.clone();
        bad["blocks"] = serde_json::json!([{ "kind": "scalar", "equation": "f_x[0]" }]);
        assert!(
            !check_f4(&Subject {
                label: "T".into(),
                report: &bad,
                dae: None
            })
            .is_empty(),
            "F4 must notice an equation covered by no block",
        );

        // F5: a pair whose equation does not reference its unknown — `f_x[1]`
        // touches only column 1, so matching it to `x` is not a matching.
        let mut bad = base.clone();
        bad["matching"] = serde_json::json!([{ "equation": "f_x[1]", "unknown": "x" }]);
        assert!(
            !check_f5(&Subject {
                label: "T".into(),
                report: &bad,
                dae: None
            })
            .is_empty(),
            "F5 must notice a pair that is not an incidence non-zero",
        );

        // F5: the same unknown claimed twice.
        let mut bad = base.clone();
        bad["matching"] = serde_json::json!([
            { "equation": "f_x[0]", "unknown": "y" },
            { "equation": "f_x[1]", "unknown": "y" },
        ]);
        assert!(
            !check_f5(&Subject {
                label: "T".into(),
                report: &bad,
                dae: None
            })
            .is_empty(),
            "F5 must notice an unknown matched twice",
        );

        // F2 without a DAE is a skip, not a pass by accident.
        assert!(
            check_f2(&clean).is_empty(),
            "F2 skips when there is no DAE to compare against"
        );
    }

    /// **The roster names exactly the checks the code can gate, and nothing else.**
    ///
    /// Scans this file's production half for `want(…)` gates and requires their union
    /// with [`super::CHECKS_OWNED_BY_RUNNER`] to equal [`super::CHECKS`]. Adding a check
    /// without rostering it fails here; rostering a name that no gate reads fails here
    /// too — and that second half is the one that would have caught **F1 and F8 being
    /// advertised as checks**, which `--only-checks` accepted for months while
    /// selecting nothing.
    ///
    /// # Two exclusions, each of which would otherwise let the test agree with itself
    ///
    /// **Comment lines are stripped.** `CHECKS_OWNED_BY_RUNNER`'s own documentation
    /// spells `want("F9")` to say what the runner must contain, and a prose mention is
    /// not a gate. Without the filter, documenting a check would satisfy the roster for
    /// it.
    ///
    /// **Only the text before `#[cfg(test)]` is read**, so this module's assertions —
    /// which necessarily contain the same spellings — are not their own evidence.
    #[test]
    fn the_check_roster_matches_the_code() {
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let this =
            std::fs::read_to_string(manifest.join("src/fidelity.rs")).expect("read fidelity.rs");
        let runner = std::fs::read_to_string(manifest.join("examples/fidelity_msl.rs"))
            .expect("read examples/fidelity_msl.rs");

        let needle = "want(\"";
        let code_only = |text: &str| -> String {
            text.split("#[cfg(test)]")
                .next()
                .unwrap_or(text)
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let production = code_only(&this);
        let mut gated: Vec<String> = Vec::new();
        for (i, _) in production.match_indices(needle) {
            let rest = &production[i + needle.len()..];
            if let Some(end) = rest.find('"') {
                gated.push(rest[..end].to_owned());
            }
        }

        let runner_code = code_only(&runner);
        for name in super::CHECKS_OWNED_BY_RUNNER {
            assert!(
                runner_code.contains(&format!("{needle}{name}\")")),
                "{name} is rostered as the runner's own, so examples/fidelity_msl.rs must gate \
                 it with the same spelling this file uses -- no such gate found. A rostered \
                 check that nothing gates runs unconditionally or not at all, and either way \
                 --only-checks is lying about it.",
            );
            gated.push((*name).to_owned());
        }
        gated.sort_unstable();
        gated.dedup();

        let mut roster: Vec<String> = super::CHECKS.iter().map(|c| (*c).to_owned()).collect();
        roster.sort_unstable();
        assert_eq!(
            gated, roster,
            "the gates and the roster disagree. CHECKS is the vocabulary --only-checks \
             accepts: a gate with no roster entry cannot be asked for, and a roster entry \
             with no gate silently measures nothing -- which is exactly what F1 and F8 did \
             to the 2026-11-05 sweep.",
        );
    }

    /// **An off-roster check name is rejected** — the must-fire half of the roster.
    ///
    /// Driven with the exact string the 2026-11-05 sweep ran with, because a guard that
    /// has not been shown to reject the real input is not protection, and this one
    /// exists for precisely that input.
    ///
    /// Note what it does **not** claim: `F10`'s absence from that string is not an
    /// error and cannot be, since a caller may legitimately want a subset. Catching a
    /// *forgotten* check is the runner's job, and it does it by printing the checks it
    /// is skipping — not this predicate's.
    #[test]
    fn an_off_roster_check_name_is_rejected() {
        assert_eq!(
            super::unknown_checks("F1,F2,F3,F5,F6,F7,F8,F9".split(',')),
            vec!["F1".to_owned(), "F8".to_owned()],
            "the overnight sweep's own string names two things that are not checks",
        );
        assert!(
            super::unknown_checks(super::CHECKS.iter().copied()).is_empty(),
            "every rostered name must be accepted by the guard that reads the roster",
        );
        assert_eq!(
            super::unknown_checks(["F2", "f2", "F11", "F2"]),
            vec!["F11".to_owned(), "f2".to_owned()],
            "matching is exact and de-duplicated: a case difference and an out-of-range \
             number are both unknown names, and a repeated good name is not a complaint",
        );
    }

    /// **F2 and F6 fire when a view is paired with the wrong model's DAE.**
    ///
    /// The must-fire half for the two checks whose signatures need a real
    /// `rumoca_ir_dae::Dae`, which cannot be hand-built — the reason both went without
    /// coverage while F3, F4 and F5 got theirs from hand-written JSON.
    ///
    /// # Cross-pairing, and why it is the right lever rather than a trick
    ///
    /// Both checks ask the same question: *does this derived view describe the DAE beside
    /// it?* Compiling two specimens and handing one's view to the other's DAE is a
    /// violation **constructed entirely out of real IR** — no synthetic block kinds, no
    /// invented field shapes, nothing the compiler would never emit.
    ///
    /// That matters because of F4. F4 has had a must-fire test since it was written, and
    /// went blind on `StructuredScalar` blocks anyway, because its test builds blocks as
    /// `{"kind": "scalar"}` — the shape F4 already handled. **A must-fire test over
    /// author-chosen JSON proves only that the handled shape is handled.** Cross-pairing
    /// cannot drift that way: its inputs are whatever the compiler currently produces.
    ///
    /// # The vacuity guard comes first
    ///
    /// If the two specimens happened to have the same equation count, the F6 pairing
    /// would be *valid* and the test would pass having proved nothing. The assertion on
    /// the counts is therefore load-bearing, not a sanity check — it is what stops this
    /// test from joining the checks it exists to falsify.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn f2_and_f6_catch_a_view_paired_with_the_wrong_model() {
        let open = |name: &'static str| {
            let FromWorker::Compiled {
                stages,
                dae,
                equation_sheet,
                identifier_index,
                ..
            } = compile_specimen_shared(name)
            else {
                panic!("{name}: expected Compiled");
            };
            let dae = dae.unwrap_or_else(|| panic!("{name}: no DAE"));
            (stages, dae, equation_sheet, identifier_index)
        };

        let (stages_a, dae_a, sheet_a, index_a) = open("SingleInertia");
        let (_stages_b, dae_b, _sheet_b, _index_b) = open("TwoLoops");

        let n_a = dae_a.inspect(|v| v.continuous_equation_count());
        let n_b = dae_b.inspect(|v| v.continuous_equation_count());
        assert_ne!(
            n_a, n_b,
            "the two specimens must differ in equation count or the cross-pairing below \
             is a VALID pairing and proves nothing. Pick two that differ.",
        );

        // The honest baseline: each view against its own DAE is clean.
        assert!(
            check_f6(sheet_a.as_ref(), index_a.as_ref(), &dae_a).is_empty(),
            "SingleInertia's own views must agree with its own DAE: {:?}",
            check_f6(sheet_a.as_ref(), index_a.as_ref(), &dae_a),
        );

        // F6 — the equation sheet now describes a different system.
        let crossed = check_f6(sheet_a.as_ref(), index_a.as_ref(), &dae_b);
        assert!(
            !crossed.is_empty(),
            "F6 compares a sheet's equation count against the DAE's ({n_a} vs {n_b}) and \
             must notice the mismatch",
        );

        // F2 — the published incidence belongs to a different DAE, so re-deriving the
        // incidence from this DAE cannot reproduce it.
        let mut fired = 0usize;
        let mut subjects_seen = 0usize;
        for s in subjects(&stages_a, &dae_b) {
            subjects_seen += 1;
            if !check_f2(&s).is_empty() {
                fired += 1;
            }
        }
        assert!(
            subjects_seen > 0,
            "cross-pairing produced no subjects at all, so F2 was never asked anything",
        );
        assert!(
            fired > 0,
            "F2 re-derives the incidence with build_incidence and compares it against the \
             published one; against the wrong DAE that comparison must fail for at least \
             one of the {subjects_seen} subjects",
        );
    }

    /// **F7 fires on a path whose description does not navigate back to it.**
    ///
    /// F7 is a round-trip property over `describe_path` / `parse_path` / `navigate`, so
    /// it cannot be made to fail by an input the grammar handles correctly — which is
    /// why it had no must-fire test. The input below is one the grammar does **not**
    /// handle: a key spelled exactly like the string `describe_path` returns for the
    /// empty path.
    ///
    /// `key_needs_quoting` is `k.is_empty() || k.contains(['.', '[', ']', '"', '\\'])`,
    /// and `(tree root)` contains none of those, so the key is written bare and its
    /// description is indistinguishable from the root's. **That is a real collision in
    /// the grammar, not a contrivance** — and F7 catching it is exactly the drift F7
    /// exists to catch, which is what makes it the honest must-fire input.
    ///
    /// # This test's input is an UNFIXED defect, and that is a liability to retire
    ///
    /// Adding `(tree root)` to `key_needs_quoting` is a one-line fix, and it would make
    /// this test fail — **the bug is currently load-bearing**, which is not a state to
    /// leave unremarked. It is tolerable only because no Modelica IR key is spelled
    /// `(tree root)`: the corpus run over 11 stage IRs per model has never produced one,
    /// so the collision is latent rather than active.
    ///
    /// The principled repair is to make F7 **injectable** — parameterise the round trip
    /// over its describer so a test can supply a deliberately broken one — after which
    /// the grammar can be fixed and this input replaced. Recorded in
    /// `docs/tech-debt.md` as a pair, because doing either half alone leaves F7 with no
    /// must-fire coverage again.
    #[test]
    fn f7_catches_a_description_that_does_not_navigate_back() {
        // The baseline: ordinary keys, including ones that DO need quoting, round-trip.
        let fine = serde_json::json!({
            "components": { "inertia.J": { "def_id": 7 } },
            "equations": [ { "Connect": 1 } ],
        });
        assert!(
            check_f7("T", &fine).is_empty(),
            "a key carrying the grammar's separators is bracketed and must round-trip: \
             {:?}",
            check_f7("T", &fine),
        );

        // The collision: a key whose bare description is the root's own spelling.
        let collides = serde_json::json!({ "(tree root)": { "x": 1 } });
        let found = check_f7("T", &collides);
        assert!(
            !found.is_empty(),
            "a key that describes to the root sentinel cannot navigate back to itself, \
             and F7 is the check that is supposed to notice",
        );
    }

    /// **F9 fires on each of the three ways an abnormal stage can say nothing useful.**
    ///
    /// The must-fire half for a check that had none until 2026-11-06, because it lived in
    /// an example where `cargo test` could not reach it. Rumoca flags far more than it
    /// fails — almost nothing hard-stops — so the note on an abnormal stage is usually
    /// the only account of what went wrong, and a check that has never been shown to fail
    /// is no evidence that the account is there.
    ///
    /// The last assertion is the half that keeps the check honest in the other
    /// direction: a flagged stage that explains itself and publishes IR is **correct**,
    /// and a check that flagged it would put 1,209 singular MSL models into the report as
    /// violations.
    #[test]
    fn f9_catches_each_way_an_abnormal_stage_can_say_nothing() {
        use crate::worker::{Outcome, Stage, StageBundle, StageKind};

        // The clean baseline: `Outcome::Ok` is also `Default`, so a fresh bundle gives F9
        // nothing to look at and must stay quiet.
        let clean = StageBundle::default();
        assert!(
            check_f9(&clean).is_empty(),
            "a bundle with no abnormal stage must pass: {:?}",
            check_f9(&clean),
        );

        let with = |stage: Stage| {
            let mut b = StageBundle::default();
            *b.get_mut(StageKind::Flatten) = stage;
            b
        };

        // 1. Abnormal with no note at all — nothing on screen and nothing to read.
        let silent = with(Stage {
            outcome: Outcome::Failed,
            note: None,
            ..Stage::default()
        });
        assert!(
            check_f9(&silent)
                .iter()
                .any(|v| v.contains("abnormal with no note")),
            "{:?}",
            check_f9(&silent),
        );

        // 2. No value, and a note that is a label rather than an account of anything.
        let label = with(Stage {
            outcome: Outcome::Failed,
            note: Some("singular".to_owned()),
            ..Stage::default()
        });
        assert!(
            check_f9(&label).iter().any(|v| v.contains("is a label")),
            "a one-word note is a classification, not an explanation: {:?}",
            check_f9(&label),
        );

        // 3. A real note with a value beside it that carries no fields — the pane offers
        //    a tab with nothing in it.
        let hollow = with(Stage {
            outcome: Outcome::Flagged,
            note: Some("the system is structurally singular".to_owned()),
            value: Some(serde_json::json!({})),
            ..Stage::default()
        });
        assert!(
            check_f9(&hollow)
                .iter()
                .any(|v| v.contains("carrying no fields")),
            "{:?}",
            check_f9(&hollow),
        );

        // The shape F9 must NOT complain about.
        let good = with(Stage {
            outcome: Outcome::Flagged,
            note: Some("the system is structurally singular".to_owned()),
            value: Some(serde_json::json!({ "residue": 2 })),
            ..Stage::default()
        });
        assert!(
            check_f9(&good).is_empty(),
            "a flagged stage that explains itself and publishes IR is correct, not a \
             violation: {:?}",
            check_f9(&good),
        );
    }

    /// **F10 is clean on real specimens, including ones that fail.**
    ///
    /// The other half of the must-fire pair: the test above proves F10 *can* fail,
    /// this one proves it does not fail on correct behaviour. Without it, wiring F10
    /// into the corpus run would risk 2,626 rows of noise from a check that is simply
    /// wrong about what a healthy compile looks like.
    ///
    /// **The failing specimens carry this test.** `UnbalancedShaft` stops at DAE
    /// construction and `CapacitorLoop` is structurally singular, so between them
    /// most stages take the branch that has nothing to show — which is exactly where
    /// "absence is stated, never silent" is a real constraint rather than a
    /// tautology.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn f10_is_quiet_on_specimens_that_behave() {
        use crate::worker::FromWorker;

        let mut checked_empty = 0usize;
        for name in [
            "SingleInertia",
            "UnbalancedShaft",
            "CapacitorLoop",
            "Drivetrain",
        ] {
            let FromWorker::Compiled { stages, .. } = compile_specimen_shared(name) else {
                panic!("{name}: expected a compiled result");
            };
            let violations = check_f10(&stages);
            assert!(
                violations.is_empty(),
                "{name}: F10 fired on a correct compile \u{2014} {violations:?}",
            );
            checked_empty += crate::worker::StageKind::COMPILATION
                .iter()
                .filter(|k| stages.get(**k).value.is_none())
                .count();
        }
        // Non-vacuity: if every stage on every specimen were populated, the
        // "absence is stated" clause would never have been exercised.
        assert!(
            checked_empty > 0,
            "no stage across four specimens \u{2014} two of which fail \u{2014} was \
             empty, so F10's absence clause was never tested",
        );
    }

    /// **F10's absence clause is exercised by a partial class, and stays quiet.**
    ///
    /// Written 2026-08-05, reading the corpus run that reported **0 violations across
    /// 2,614 models**. A zero is worth nothing without knowing what it covered, and
    /// F10's first two clauses are near-tautological in production — nothing calls
    /// `Stage::computed`, and the constructors set provenance and value together. **The
    /// third clause is the one carrying the check**, and it only bites when a stage has
    /// no value at all.
    ///
    /// The corpus does reach that: **350 of its 2,626 entries are `Interfaces`,
    /// `Icons`, `BaseClasses` or `Types`** — partial and abstract classes that cannot
    /// produce a DAE, so their later stages are necessarily empty. That was an
    /// inference from the report's `kind` column; this test measures it.
    ///
    /// A partial class is also the shape most likely to expose a *silent* blank pane,
    /// since nothing about it is an error — the compile does not fail, it simply has
    /// nothing to say past a point, which is exactly the state clause 3 exists for.
    #[test]
    #[cfg_attr(
        not(feature = "slow-tests"),
        ignore = "compile-heavy; run with --features slow-tests"
    )]
    fn f10s_absence_clause_is_exercised_by_a_partial_class() {
        use crate::worker::FromWorker;

        // One of each partial kind the corpus contains, plus a normal component for
        // contrast. **`BooleanMIMOs` was tried first and produced a value for every
        // stage** — which is why this scans several rather than asserting on one, and
        // why the failure message below is worth reading rather than fixing by
        // swapping the name.
        const CANDIDATES: &[&str] = &[
            "Modelica.Blocks.Interfaces.BooleanMIMOs",
            "Modelica.Blocks.Icons.Block",
            "Modelica.Blocks.Types.ExternalCombiTable1D",
            "Modelica.Electrical.Batteries.BaseClasses.BaseCellStack",
            "Modelica.Blocks.Tables.Internal.CombiTable2DBase",
        ];

        let mut with_empty = 0usize;
        let mut empty_detail: Vec<String> = Vec::new();
        for name in CANDIDATES {
            let FromWorker::Compiled { stages, .. } =
                crate::worker::test_msl::compile_library_shared(name)
            else {
                panic!("{name}: expected a compiled result");
            };

            // F10 must be quiet on all of them: a partial class is ordinary, not
            // broken, and a check that fires on 350 corpus entries would be noise.
            assert!(
                check_f10(&stages).is_empty(),
                "{name}: F10 fired on a partial class: {:?}",
                check_f10(&stages),
            );

            // **The rostered stages are excluded, or this count means the opposite
            // of what it says** (2026-11-03). A stage on
            // `worker::AWAITING_REMODEL` is one Rumoca runs and HRW publishes no IR
            // for, so it is empty for **every** model. When that roster held index
            // reduction, initialization and events, counting them
            // put `with_empty` at 5 of 5 and the assertion below announced that
            // *"F10's absence clause has corpus coverage for the first time"* —
            // which would have been recorded in `fidelity-plan.md` as a fact about
            // the MSL when it is a fact about HRW's disabled panes.
            //
            // The clause is about a **compile stopping early**. A pane HRW cannot
            // draw is a different thing with the same shape, which is why the
            // exclusion is by roster rather than by the stages happening to be
            // empty.
            let empties: Vec<&str> = crate::worker::StageKind::COMPILATION
                .iter()
                .filter(|k| !crate::worker::AWAITING_REMODEL.contains(k))
                .filter(|k| stages.get(**k).value.is_none())
                .map(|k| k.name())
                .collect();
            if !empties.is_empty() {
                with_empty += 1;
                empty_detail.push(format!("{name}: {empties:?}"));
                // The clause restated as the property, so a change that drops a note
                // fails here as well as inside F10.
                for kind in crate::worker::StageKind::COMPILATION {
                    let s = stages.get(*kind);
                    assert!(
                        s.value.is_some() || s.note.is_some(),
                        "{name}/{}: empty and silent",
                        kind.name(),
                    );
                }
            }
        }

        // **Measured 2026-08-05: `with_empty` was 0, and that was the finding.**
        //
        // None of the five produced an empty stage. Every MSL model compiled all the
        // way through and populated all eleven, **including the partial and abstract
        // classes** — so the corpus could not exercise F10's absence clause, and the
        // sweep's zero for F10 covered only the two near-tautological clauses. It was
        // deliberately not asserted as `> 0`: that would demand the corpus have a
        // property it did not have.
        //
        // # IT IS 2 SINCE v0.10.0, AND THE DAY THIS TEST WAS WRITTEN FOR HAS ARRIVED
        //
        // *2026-11-03.* `Modelica.Electrical.Batteries.BaseClasses.BaseCellStack` and
        // `Modelica.Blocks.Tables.Internal.CombiTable2DBase` now stop before Flatten —
        // Flatten, Structural and Solve lowering are all empty. **Both are `partial`
        // base classes**, which is what this test is named after, and declining to
        // flatten an incomplete class is the correct answer: there is no complete
        // model to flatten. 0.9.20 flattened them anyway.
        //
        // **So clause 3 has corpus coverage for the first time**, and from the source
        // the old note guessed it would come from. `docs/fidelity-plan.md` records it.
        //
        // The three rostered stages are excluded above, so this 2 is about compiles
        // stopping early rather than about HRW's disabled panes — the distinction
        // that assertion is for.
        //
        // **Pinned by name as well as by count.** A count of 2 would also be
        // satisfied by two *complete* models silently failing to flatten, which is
        // the finding that would matter far more than this one.
        let partials: Vec<&String> = empty_detail
            .iter()
            .filter(|d| d.contains("BaseClasses") || d.contains("Internal"))
            .collect();
        assert_eq!(
            partials.len(),
            with_empty,
            "every MSL model that stops early must be a partial base class; a \
             COMPLETE model failing to flatten is a different and much larger \
             finding:\n  {}",
            empty_detail.join("\n  "),
        );
        assert_eq!(
            with_empty,
            2,
            "the number of MSL models stopping early has moved. Two partial base \
             classes stop before Flatten on v0.10.0, which is what gives F10's \
             absence clause its corpus coverage; a third \u{2014} or a second that \
             is not partial \u{2014} is a new finding and belongs in \
             docs/fidelity-plan.md.\n  {}",
            empty_detail.join("\n  "),
        );
    }

    /// **F10 fires on each of the three things it claims to check.**
    ///
    /// The must-fire rule applied to the first *verb* check in this file. F1-F9 all
    /// ask whether a structure matches the compiler's; F10 asks whether the pane's
    /// claim about the compiler is honest — and a check about honesty that cannot
    /// fail is the exact shape of the problem it was written for.
    #[test]
    fn f10_catches_each_way_a_pane_can_mislead() {
        use crate::worker::{Provenance, Stage, StageBundle};

        // A bundle where every stage states its absence: the clean baseline.
        let mut clean = StageBundle::default();
        for kind in crate::worker::StageKind::COMPILATION {
            *clean.get_mut(*kind) = Stage::info("not reached");
        }
        assert!(
            check_f10(&clean).is_empty(),
            "a bundle whose panes all state their absence must pass: {:?}",
            check_f10(&clean),
        );

        // 1. Content HRW produced.
        let mut invented = clean.clone();
        *invented.get_mut(crate::worker::StageKind::Events) =
            Stage::computed(serde_json::json!({}), "probe");
        assert!(
            check_f10(&invented)
                .iter()
                .any(|v| v.contains("HRW produced")),
            "{:?}",
            check_f10(&invented),
        );

        // 2. Provenance disagreeing with content.
        let mut inconsistent = clean.clone();
        let s = inconsistent.get_mut(crate::worker::StageKind::Flatten);
        *s = Stage::ok(serde_json::json!({ "a": 1 }));
        s.provenance = Provenance::Empty;
        assert!(
            check_f10(&inconsistent)
                .iter()
                .any(|v| v.contains("must agree")),
            "{:?}",
            check_f10(&inconsistent),
        );

        // 3. **A silent blank pane** — the one the corpus is for. `Stage::default()`
        // is the mid-compile state: no value, no note, nothing on screen and nothing
        // said. Indistinguishable from a phase that ran and produced nothing.
        let mut silent = clean.clone();
        *silent.get_mut(crate::worker::StageKind::Dae) = Stage::default();
        assert!(
            check_f10(&silent)
                .iter()
                .any(|v| v.contains("says nothing")),
            "{:?}",
            check_f10(&silent),
        );
    }
}
