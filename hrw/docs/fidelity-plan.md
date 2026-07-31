# Plan — does HRW faithfully represent Rumoca?

Written 2026-07-30, from Doug's concern:

> We have not sufficiently tested that the HRW code correctly represents Rumoca's
> understanding or decisions about the specimens which it compiles.

He is right, and nothing in the 444 tests addresses it. They check HRW's **own** logic
(link parsing, playback, layout) and that specimens **compile**. Neither asks whether what
HRW *shows* is what Rumoca *decided*.

## The goal, stated exactly

Doug, 2026-07-30:

> I want us to gain confidence that HRW tells the truth about Rumoca, **even if Rumoca is
> wrong.**

That last clause is the whole scope. **A Rumoca bug faithfully rendered is a PASS here.**
Finding compiler bugs is a different effort with a different instrument (#43's oracle), and
mixing them would make every failure ambiguous again — which is the cost #51 could not pay.

Two consequences that changed this plan after it was first written:

- **Stratify by IR shape, not by physics domain.** The first draft said "cover every MSL
  top-level package". But if the question is fidelity, the domain is a *proxy* — a `Fluid`
  model and an `Electrical` model with the same IR shape test HRW identically. What
  stresses the representation is **arrays, function calls, records, deep hierarchies, huge
  incidence matrices, many small blocks vs one large one**.
- **Failures are in scope.** F1–F7 as first written all assume a *successful* compile. But
  "even if Rumoca is wrong" means a Rumoca **failure** must be faithfully represented too:
  the error payload, the unmatched equation and unknown lists, the blamed source spans.
  Several of those were fixed by hand on 2026-07-29–30 and nothing checks them at scale.
  See **F9**.

## Why the bar is higher than "correct enough for Doug"

Doug, 2026-07-30, on what this is ultimately for:

> I need HRW to provide a tour of the Rumoca malfunction which I can capture as a video or
> screenshot to link in the PR description [...] to demonstrate to Rumoca PR reviewers that
> HRW is useful and therefore worthy of being merged. [...] I need HRW to faithfully
> represent Rumoca so that the Rumoca PR reviewers don't dismiss HRW as junk.

**The audience changes the bar.** Until now HRW's reader has been Doug, who trusts it. A
Rumoca maintainer knows the compiler better than Claude does, will spot a wrong number in a
screenshot instantly, and has no investment in HRW succeeding while being asked to take on
maintenance burden. **One visible misrepresentation does not merely fail to persuade — it
discredits.**

That re-ranks the checks by *what a maintainer would catch in a demo*, which is not the
same as what would most mislead Doug:

| Noticed in a demo? | Checks |
|---|---|
| **Immediately** | F1 (they know what tearing should decide), F2 (a transposed matrix is obvious), F3 (a wrong count is checkable at a glance) |
| **It is the demo path** | **F9** — a bug PR demo shows Rumoca *failing* |
| Invisible | F6, F7 — internal quality only |

**The convergence worth noticing:** a bug-PR demo runs entirely through the
failure-representation path, which is exactly where every hand-found bug of 2026-07-29–30
clustered. **F9 protects the thing HRW would be judged on**, and has the demonstrated bug
density. Start there.

### A note on how to use the demo

Let the tour **be** the bug report rather than accompany it. A report that doubles as a
pitch can read as using a maintainer's review time for promotion; "here is the malfunction,
walked stop by stop" is simply the clearest explanation they will receive, and HRW's
usefulness is then demonstrated **implicitly**. `docs/fixture-tours/the-oracle.md` is
already this shape — it explains a real Rumoca defect and never argues that HRW is good.

## The one-sentence property

**HRW must invent nothing and omit nothing.** Every value it displays should be traceable
to Rumoca's output, and every decision Rumoca made should be recoverable from what HRW
shows. The checks below are that property, made checkable one piece at a time.

## Why this is the dangerous gap

HRW does two different things with Rumoca's output, and only one of them is safe:

| | Risk |
|---|---|
| **Reads** a phase result and renders it | distortion — a dropped field, a transposed index, a mislabelled column |
| **Re-derives** a result by re-running the algorithm | **divergence** — HRW computes a *different answer* and presents it as the compiler's |

The second is the one that can teach Doug something false. `tearing_anim::walk_blocks`
re-runs tearing on every coupled block to produce its animation; `matching_anim` and
`tarjan_anim` re-run matching and Tarjan from the incidence JSON. **None of these has ever
been compared against what Rumoca's own report says.**

They agree on the 18 hand-picked specimens by assumption, not by test.

## The design idea: invariants, not expected answers

The obstacle raised by `docs/ideas.md` #51 is **triage cost** — with 1,656 unfamiliar
models, every failure is an investigation, and Claude has already twice reported a finding
that was not there.

**Invariant checks dodge that entirely.** They do not require knowing the right answer for
a given model; they assert a property that must hold for *every* model. A violation is
definitionally a bug, with nothing to adjudicate. That is what makes this affordable at
scale where a compile-census is not.

## The checks

Ordered by how much they would hurt if they failed.

### F1. Re-derivation matches the report  *(the centrepiece)*

For every model with a coupled block: run `tearing_anim::walk_blocks` and compare its
result against the `tearing` field of the structural report — same tear variables, same
residual equations, same causal sequence, in the same order.

Both sides exist today. The report says
`{tear_vars, residual_equations, causal_sequence}`; the animation produces `TearingFrame`s
plus `BlockNames` to resolve local indices back to names.

Same shape for `matching_anim` (final matching vs the report's `matching`) and
`tarjan_anim` (its SCCs vs the report's `blocks`).

**If this fails, an animation is teaching a decision the compiler did not make.**

**Status 2026-07-31: all three halves built and passing** over ten specimens
(`worker::tests::F1_MODELS`). Compared per *tab*, against the DAE that tab animates —
Structural shows the raw system and Index Reduction the reduced one, so a single
comparison against "the report" tests nothing and fails on models that are singular
before reduction.

**F1 found one real bug on its first run**, of exactly the class it was built for.
`partial_matching_to_json` labelled equations with the bare `EquationRef` while
`incidence_to_json` used the labelled form Rumoca's report uses, so on any model whose
equations carry origins the two never correlated and the singular incidence view showed
**nothing** as matched — `Drivetrain` rendering 0 of 97 when Rumoca had matched 93. The
cause was a reimplementation of `equation_label`; the fix made that function `pub`
upstream and deleted both copies. `before_report_json` had the same defect.

Two other F1 failures on that run were **the test's fault, not HRW's**, and both are worth
recording because they are the shape of a false finding:

- Tarjan's `sccs_so_far` lags by one — `tarjan.rs` records the frame *before* pushing the
  component — so reading the last frame yields an empty partition on a graph that is a
  single SCC. `final_sccs` collects `SccFound` steps instead.
- The tearing check compared the raw DAE's re-derivation against whichever report existed,
  and singular stages hide the tearing view anyway (`structural_view_available`). A
  re-derivation the UI never shows is not a misrepresentation.

### F2. Incidence is faithful

HRW's `incidence` JSON against a fresh `build_incidence(&dae)`: same `n_eq`, same `n_var`,
same `unknown_names` in the same order, and every row's index set identical. Catches a
transposition or an off-by-one that would make every downstream view subtly wrong while
looking plausible.

### F3. Structural counts agree with themselves

`n_equations`, `n_unknowns`, `matching.len()`, `rank_deficiency` — mutually consistent, and
consistent with `incidence`. The `rank_deficiency: 7` bug (2026-07-29, true value 1) is
exactly this class and was found by eye.

### F4. BLT blocks partition the equations

Every equation appears in exactly one block; block sizes sum to `n_equations`. A
partition is easy to check and impossible to satisfy accidentally.

### F5. Matching is a matching

Injective: no equation matched to two unknowns, no unknown claimed twice, every index in
range.

**Status 2026-07-31: F2-F5 built and passing** as one harness in `src/fidelity.rs` —
30 incidence-bearing reports across 10 models (16 with blocks, 29 with a matching, 4
singular), 0 violations.

Three things the build settled that the plan had not:

- **A "subject" is not "the structural report".** It is *every* report HRW publishes that
  carries an incidence — today Structural, Index Reduction, and the `before` report nested
  inside the latter, so three per model. Walking them generically matters: `before`
  carried the same labelling bug F1 found in the singular matching, and a check written
  against "structural" alone would have missed it.
- **F5's teeth are the incidence-nonzero check**, not injectivity. An equation matched to
  a variable it does not reference is not a wrong choice among valid ones — it is not a
  matching at all. That check would have caught F1's bug independently.
- **Non-vacuity guards are load-bearing here**, more than in F1. Every check skips
  subjects it does not apply to (no blocks, no matching, no error), so without the guards a
  corpus that stopped producing them would pass in silence. The harness prints its
  coverage for the same reason: "0 violations" means nothing without "over how much".

Cost: **32s for 10 models**, against 148s for F1's three checks over the same 10 — the
harness shape (compile once, apply every invariant, drop) rather than any optimisation.
See `docs/ideas.md` #48 for why memoization is the wrong lever at sample scale.

### F6. Derived views cover their source

The equation sheet names every continuous equation; the identifier index's variables are a
subset of the DAE's; every `def_id` referenced in a stage resolves in `def_index`.

### F7. Every capture noun works on real IR

For a sample of node paths from each stage: `describe_path` → `parse_path` → `navigate`
returns the same subtree. The capture vocabulary has only ever been exercised on 18
models, and it is the thing every question depends on.

### F8. No stage panics, and sizes are recorded

The stress test, as a **byproduct** rather than the goal. `Media.Examples.WaterIF97`
produced a 3.2 MB flatten stage without panicking; that was previously unknown. Record
size and time per stage per model so a regression in either is visible.

### F9. Failures are represented faithfully too

For every model where a phase **fails**, the error payload must carry what Rumoca actually
reported — not a paraphrase, and not less:

- the counts in a structural singularity match `StructuralError::Singular`'s own fields
- `unmatched_equations` / `unmatched_unknowns` match the error's lists, in order
- a blamed source span resolves to a real line in the specimen, and to *that* variable
- a diagnostic's `labels` survive into the JSON with their spans intact
- the message is emitted **verbatim** somewhere, so nothing is lost to summarising

This is where 2026-07-29–30's hand-found bugs clustered: `rank_deficiency` computed as 7
when the truth was 1, spans that Rumoca supplied and HRW dropped, labels dropped by every
emitter, a `ToDae` failure reduced to a bare informational note. **Every one was HRW
failing to tell the truth about a Rumoca failure** — exactly the case Doug's clause is
about, and the one the first draft omitted.

## Status — F1-F9 all built, 2026-07-31

`src/fidelity.rs` (F2-F9) and `worker.rs` (F1). Coverage on the current corpus:

| | |
|---|---|
| F2-F7 | 10 models, 30 incidence-bearing reports, 100 stage IRs walked, **0 violations** |
| F8 | 16 models serialized; largest total 1.88 MB (`Drivetrain`), largest single stage 536 KB |
| F9 | 6 failing specimens, 14 abnormal stages, 8 with structure, 6 source locations verified |

**F7 found the second real bug**, and a large one: JSON object keys containing `.`
were written bare into node paths, so `parse_path` split them and landed on nothing.
`enum_literal_ordinals` is keyed by qualified names like `StateSelect.never`, so this hit
**every model in the corpus — 6,169 broken paths**. Every `hrw://` link and every
`focus.json` naming such a node pointed Claude at a subtree that does not exist. Fixed by
quoting keys that carry the grammar's own separators (`a["b.c"]`), additively: a key that
never needed escaping still renders byte-identically, so existing links keep working.

### What the false alarms taught, which is most of the value

Of 12 violations across F6-F9's first runs, **9 were the check's fault**. Recording the
pattern because it is the same failure mode as HRW inventing a decision, one level up:

- **F6** re-listed the DAE's variable partitions by hand, omitted the two discrete ones,
  and reported `BouncingBall`'s `c` as a phantom. Now keyed by the `kind` the index itself
  recorded, so an unrecognised partition says so instead of masquerading as a violation.
- **F9** demanded structure from stages where Rumoca supplied none (`UndefinedRef`'s six
  downstream "no result" stages). Carrying nothing is *faithful* there — the property is
  that HRW must not **lose** structure, which is only checkable where there was some.
- **F9** looked for that structure under `"error"` alone, and `OverInitRc` publishes a
  full IC plan with a `determinacy` verdict and no `error` key.
- **F1's** Tarjan accessor read `sccs_so_far`, which lags by one component.

The recurring shape: **a check that knows one form of the truth reports every other form
as a defect.** Cheap to write, expensive to trust. Hence the discipline of reading the
data before believing an assertion message.

### The "no result" bucket, triaged

`not_reached_stage` has three branches, and they are **not** equivalent. `Failed { phase }`
and `NeedsInner` produce `Stage::info` — neutral, correct, since a stage that never ran is
not a failed stage. `None` produces `Stage::err`, which renders six tabs red for one
resolve failure. That is not a *fidelity* violation (Rumoca really did supply nothing, and
HRW says so) but it is a **legibility** one, and it is Doug's call because it changes what
he sees. Left as-is, recorded here.

## The sample

**Not all 1,656.** A stratified sample of **40–60**, chosen to cover:

- **IR shapes** that stress the representation: array variables, function calls, records
  and nested types, deep component hierarchies, very large incidence matrices, many small
  BLT blocks versus one enormous coupled block
- the phenomena HRW has **views** for: high index, algebraic loops, events, alias
  elimination, connection expansion, `pre()` lowering
- **models that fail**, in each phase that can fail — F9 has no data otherwise
- the extremes: the largest models available, and the smallest

Package coverage is worth *sampling* for variety, but it is not the criterion. Two models
from different packages with identical IR shape test HRW once, not twice.

Stratification matters more than count: 50 models spread across packages will find more
than 500 from one.

## Sequencing

1. ~~**Fix outcome classification first.**~~ **Done 2026-07-31** —
   `Outcome::{Ok, Flagged, Failed}`, with `note_is_error()` derived from it so no
   colour or branch changed. `Stage::note_is_error` was set by three
   constructors meaning "nothing produced", "failed with a diagnosis", and **"singular but
   perfectly usable"**. Anything counting outcomes on that boolean will miscount, and it
   already produced one false finding (see #51). A three-way `Stage::outcome()` comes
   before any harness reads it.
2. ~~**F1 on the existing 18 specimens.**~~ **Done 2026-07-31**, on ten of them. If the re-derivations already disagree with the
   report on models we know, that is the bug of the day and the MSL work can wait.
3. **The sample list**, checked in, with why each model was chosen.
4. ~~**F2-F5 as invariants.**~~ **Done 2026-07-31** on the F1 corpus; the harness takes any list of model names, so pointing it at the sample is a one-line change. **F6-F7** remain.
5. ~~**F8 recorded.**~~ **Done 2026-07-31** — printed per model, with a loose ceiling that catches a runaway rather than a tight bound that would fail on a legitimately large model.

Step 2 is deliberately first and deliberately small: it is the cheapest possible test of
the plan's central hypothesis, and if it passes on 18 models the same harness runs on 50
with no new code.

## Cost, and where it runs

Each model is a full uncached compile against MSL — seconds. 50 models is minutes, not the
hour a full census would take, which is why the sample is stratified rather than
exhaustive. **Behind `slow-tests`**, not in the 7-second loop.

## What this does not establish

That Rumoca is *correct*. Every check here asks whether HRW agrees with Rumoca, so a
compiler bug faithfully rendered passes every one of them. Correctness needs the
independent implementation — `docs/ideas.md` #43 and the differential test in #4, on the
same corpus.

**Two different questions, and this plan answers only the first:**

- *Does HRW tell the truth about Rumoca?* — this plan.
- *Does Rumoca tell the truth about Modelica?* — System Modeler.
