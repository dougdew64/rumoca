# Updating Rumoca (rebasing the `hrw` branch on upstream)

**Purpose:** the rebase procedure, and which generated artifacts need explicit regeneration
afterwards.
**Status:** procedure.
**Read when:** pulling in newer upstream Rumoca, and **before deciding whether to fix anything in
the fork** — step 0 exists to catch a defect upstream has already repaired. This is a rebase, not a
pin bump; the large fidelity suite is a step here, not something to remember.

HRW now lives **inside a fork of the Rumoca workspace** (`hrw/`), depending on the Rumoca crates
via **path deps** (see [`DECISIONS.md`](../DECISIONS.md) — the in-workspace move). So "updating
Rumoca" is no longer a pin bump; it's **rebasing the `hrw` branch on a newer upstream** and fixing
the fallout. The Rust compiler and the test suite do most of the work; a few generated artifacts
(the field-help table and the per-specimen traces) need explicit regeneration.

## 0. Choose the target first — `upstream/main` is not automatically it

**Added 2026-09-04, measured rather than assumed.** This procedure said "rebase on
`upstream/main`" as though the only question were *when*. It is also *what*, and on that date
the two answers were far apart:

| | state on 2026-09-04 |
|---|---|
| `upstream/main` | last commit **2026-07-29**, five weeks stale |
| latest tag / release | **v0.9.20**, 2026-07-13 — what this fork is on |
| where the work is | PR **#340** `msl-trace-parity-50`, **194 commits, 2,377 files**, updated 2026-08-27 |

**So a quiet `main` does not mean a quiet project**, and rebasing onto `main` can pick up almost
nothing while a release sits unmerged beside it. Check three things before choosing:
`git log upstream/main -1`, the open PR list, and the tag list.

**Prefer a tag.** A tag is a point the maintainer chose to publish; a release branch is one they
are still moving, and `mergeable_state: unstable` on #340 means its own CI was not green. Rebasing
onto an unmerged branch means this fork's history depends on a ref that may be force-pushed,
renamed, or abandoned.

**And check whether the release already fixes what we were about to fix.** On 2026-09-04 the
0.10.0 branch turned out to contain `lower/initial_pins.rs` and `lower/implicit_derivative.rs`,
and to have deleted the `BuiltinFunction::Der => emit_const_at(0.0)` arm — i.e. the initialization
defect in [`upstream-issues.md`](upstream-issues.md) looks repaired there, by the formulation this
project had independently derived. **That check cost four `curl`s and saved implementing a large
semantic change to code upstream had already rewritten.** Do it before any fork-side fix, not
after.

## 1. Rebase the `hrw` branch on upstream

**THE `upstream` REMOTE DOES NOT EXIST — add it first** *(found 2026-09-04; this step opened with
`git fetch upstream` and `git remote -v` lists only `origin`, which is Doug's fork)*. **The whole
procedure was blocked at its first command** and nothing said so, because a rebase has not been
run since HRW moved in-workspace:

```text
git remote add upstream https://github.com/CogniPilot/rumoca.git
git fetch upstream --tags
```

**Know where you are starting from, because the version string will not tell you.** Measured
2026-09-04: the workspace declares `0.9.20` and `Cargo.lock` agrees, but the newest
upstream-authored commit in this history is **`8cdc7419`, 2026-07-18** — five commits behind
`upstream/main`, and *later* than the `v0.9.20` tag of 2026-07-13. **The version number is a label
carried along in `Cargo.toml`, not a statement about which upstream code is present**, and
`crates/` additionally carries this fork's instrumentation. To find the real base:

```text
git log --format="%h %ad %an | %s" --date=short | grep -v dougdew64 | head -3
```

- Then `git rebase <the target chosen in step 0>` on the
  `hrw` branch. Rumoca's own code advances underneath; your **additive instrumentation
  hooks** should rebase cleanly (that's the point of keeping them observation-only), and `hrw/`
  itself won't conflict — upstream has no files there.
- **Tell Doug the first-launch cost before he hits it.** `compiler_source_fingerprint()` hashes the
  whole `crates/` tree plus the workspace `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`, so a
  rebase invalidates **every** cached parsed MSL file at once. His next launch pays one full MSL
  re-parse, and it lands on the *first compile* rather than at startup, which reads as "HRW is
  broken today" — it has already done so once (`../CLAUDE.md`, the invalidation rule).
- Path deps track the checked-out tree automatically — no `rev` to edit. `cargo build -p hrw`
  re-locks against the new Rumoca. Commit the updated `Cargo.lock`.
- Resolve any conflicts in the instrumentation hooks against the moved phase code, then continue.
- **If the rebase moved `rust-toolchain.toml`, re-test the rust-analyzer workaround** — added
  2026-09-21. `.vscode/settings.json` runs the analyzer under `stable` because the pinned
  nightly's cargo rejects the `--lockfile-path` flag the analyzer sends on nightly toolchains,
  and the fallback query has no dependency graph (the file carries the full account). A newer
  pin may accept the flag, making the line unnecessary; a newer pin may also introduce
  nightly-only features that stable cannot analyse, making it wrong. Test it the way it was
  found: `cargo +<new nightly> metadata --help | grep lockfile` says whether the flag exists,
  and a Go to Definition across crates after a restart says whether the graph is there. Keep,
  drop, or re-justify the line on that evidence; do not carry it forward unread. The
  `PYO3_PYTHON` override in the same map is independent of the pin — it corrects upstream's
  Linux-only path in `.cargo/config.toml` and stays until upstream makes that portable.

## 2. Fix compile breakage — the compiler is the guide

**KNOWN IN ADVANCE FOR 0.10.0: the MSL source-root shape changed, and this one does NOT
surface as a compile error** *(measured 2026-09-19 against `853791b`)*. HRW's `msl_roots()`
hands over three roots:

```text
vendor/msl/Modelica 4.1.0    vendor/msl/ModelicaServices 4.1.0    vendor/msl/Complex.mo
```

On 0.10.0 that combination fails at *resolve* with `unresolved component reference` on
`Integer_inf`. **Passing the parent `vendor/msl` as a single root works.** So it is a runtime
resolution failure on a real model, not a type error — `cargo build` is green and every
MSL-dependent test fails instead. `msl_roots()` is duplicated across `worker.rs` and several
examples, so grep for it rather than fixing the first one.
- `cargo build`. Rust flags every API change HRW's code relies on: `Session` methods,
  `parse_to_ast`, `ClassTree` lookups (`def_map`, `get_class_by_qualified_name`), the `DefInfo`
  extraction in `src/worker.rs`, serde field access, etc. Fix each error.
- Most Rumoca updates surface here as concrete type/name errors — not silent drift.

**The exception, and a large release is exactly when it bites: A CHANGED PIPELINE SHAPE.** If
upstream adds, removes, renames or reorders a phase, HRW's stage machinery does not fail to
compile — it silently describes a compiler that no longer exists. This has happened: `Dae` was
added to the pipeline and `gen_trace`'s own hard-coded roster was not updated, so **7 of 21
manifests had a `dae` entry and 14 did not, for seventeen days**, with nothing able to notice.

**The per-stage systems that must all agree** (`../CLAUDE.md`'s new-stage rule is the authority;
this is the rebase-time instance of it):

| what | where |
|---|---|
| the roster, and the compile-only subset | `StageKind::ALL`, `StageKind::COMPILATION` |
| link + capture slug, and its inverse | `StageKind::slug`, `from_slug` |
| bridge and notebook file name | `StageKind::stage_file_name`, `notebook_key` |
| the canonical file list | `bridge::STAGE_FILE_NAMES` |
| what a compile publishes | `StageBundle::as_stage_pairs` |
| which crate supplies tooltips | `StageKind::ir_crate` |
| sub-view rosters | `SubView`, `STAGES_WITH_SUB_VIEWS` (`node_link_check`) |

Several of these are pinned against each other by tests — `stage_kind_all_is_exhaustive`,
`every_stage_file_name_is_in_the_canonical_list`, `stage_pairs_names_match_stage_file_names`,
`manifest_stage_rosters_match_the_pipeline`, `every_stage_round_trips_between_capture_and_link` —
so run the full suite and read *which* of them fails: that names the system that fell behind.
**What no test can supply is the mapping from a new upstream phase to a pane**; that is judgement,
and it is the real work of a large rebase.

## 3. Re-verify behavior
- `cargo test`. The suite pins the behavior HRW assumes:
  - `tests/msl_resolve.rs` — MSL loads and a specimen resolves against it (+ negative control).
  - `worker::tests::resolves_def_ids_against_msl` — component types resolve to their MSL classes.
  - `bridge::tests::*` — span-ascent, cross-stage diff.
- Test failures flag **semantic** changes (behavior moved even though it still compiles).
- **Watch `worker::tests::drivetrain_index_reduces_from_singular_to_solvable`.** HRW's index-reduction
  funnel (`worker::index_reduce_for_structural_analysis`) mirrors the *order* of rumoca-sim's internal
  `prepare_dae_for_structural_analysis` (`solve_lowering/structural_lowering.rs`). The compiler catches
  renamed/removed `dae_prepare` fns; a **reordering** it won't — but this test will (before = singular,
  after = solvable). If it fails, re-diff the funnel against that rumoca-sim source and update the order.

## 3b. Run the large-scale fidelity suite

- This is **trigger 1** of the fidelity policy (`docs/fidelity-plan.md`), and it lives here
  rather than in anyone's memory on purpose.
- The pre-commit suite already carries the small-scale checks over the 16 curated specimens.
  The large suite adds the stratified MSL sample — IR shapes those 16 do not reach.
- **What a failure means here is different from usual.** These checks compare HRW's output
  against Rumoca's *on the same input*, so a failure after a rebase says Rumoca's own answer
  moved. That is information about the upstream change, not necessarily a bug in HRW — read
  the diff before "fixing" anything.

## 3c. Re-test every open entry in `upstream-issues.md` — added 2026-09-04

**This step did not exist, and it is the one a release is most likely to settle.** Each entry is a
reproduced defect with a reproducer; after a rebase some of them are simply gone, and an entry that
stays in the file describing fixed behaviour is worse than no entry — it is a false claim about
someone else's code, in a public repository, with Doug's name on it.

**Work down the file and re-run each reproducer.** For every entry, one of three outcomes:

- **Fixed** — strike it through, keep the account, and say which upstream version fixed it. The
  file already uses strikethrough headings for this; follow that shape.
- **Still present** — note the version it was re-confirmed on. That is what turns an entry into
  something a maintainer can act on, since "reproduced on 0.9.20" ages badly.
- **Changed shape** — the most valuable outcome and the easiest to mis-file. Re-diagnose rather
  than editing the old text, because a half-updated entry reads as authoritative.

**Un-ignore the acceptance tests.** Open entries carry `#[ignore]`d tests naming the defect as the
reason — `worker::tests::rc_circuit_charges_its_capacitor` is one, and
[`ui-findings.md`](ui-findings.md) C21 carries another. Running the ignored set is the cheapest
possible survey of what a release repaired:

```text
cargo test -p hrw --lib --features slow-tests -- --test-threads=1 --ignored
```

**Then re-run the oracle**, which costs almost nothing now: System Modeler is reachable from the
Wolfram MCP server (recipe in [`ideas.md`](ideas.md) #43), so "did this release make Rumoca agree
with a reference implementation on our specimens?" is a script, not an afternoon. That comparison
is the strongest single statement this project can make about a release.

## 4. Regenerate the per-stage field-help table
- `cargo run --example gen_field_help` — re-extracts `///` field docs into `src/field_help.json`
  (see `src/field_help.rs`).
- **It harvests FIVE crates, not one, and the table is keyed BY STAGE** *(corrected 2026-09-04;
  this step said "the new `rumoca-ir-ast`", which was true when the table was one flat map)*. The
  roster is `field_help::IR_CRATES` — `rumoca-ir-ast`, `rumoca-ir-flat`, `rumoca-ir-dae`,
  `rumoca-ir-solve`, `rumoca-phase-structural` — and each stage draws only from the crate
  `StageKind::ir_crate()` names for it. **66 field names are documented in two or more of those
  crates**, which is why the split exists: a flat map served `rumoca-ir-ast`'s doc for an *import
  clause* as the tooltip for Solve lowering's `names`.
- **So an upstream crate RENAME or SPLIT breaks two things, and only one is a compile error.**
  `IR_CRATES` failing to resolve is loud. `StageKind::ir_crate()` pointing a stage at a crate that
  no longer holds its type is silent — the stage simply loses its tooltips, or borrows another
  stage's. Re-check that mapping against the new crate layout by hand.
- **Review the diff.** New/renamed/removed fields are a signal: a renamed field the app reads
  (e.g. in `worker.rs` or `tree.rs`) may need a matching code change; new fields may be worth
  surfacing. Removed fields hint at API breakage step 2 should also have caught.

## 5. Regenerate specimen traces
- `cargo run -p hrw --example gen_trace -- --all` (3m45s) rewrites every specimen's stage IR under
  `trace/` and its `trace/manifest.json`, which stamps the new Rumoca rev. A single specimen is
  `-- <SpecimenName>`.
- **The verifier is a gate, not your eyes** — run it *before* regenerating, so it tells you what
  moved:

  ```text
  cargo test -p hrw --lib --features notebook-check -- --test-threads=1 the_committed_notebook
  ```

  It costs ~109 s and needs its own feature because each specimen requires a **fresh**
  `WorkerState`; against the shared worker it is order-dependent and passes alone while failing in
  company. A committed trace is one sample of a function whose hidden argument is the session, and
  `gen_trace` runs one process per specimen — so what is committed is the **virgin-session** value.
- **"Nothing else depends on it" was FALSE, and it made this step look optional** *(corrected
  2026-09-04)*. Eight source files read `docs/specimen-notebook/`. Three matter here:
  - `the_committed_notebook_matches_what_the_pipeline_produces_now` — a gate, and the only thing
    that catches trace *contents* drifting. The committed traces were stale for **25 days** before
    it existed.
  - `node_link_check` judges every fixture lab's node links against the notebook
    (`every_lab_node_link_lands_on_a_real_node`). A stale notebook therefore makes that check
    quietly wrong rather than absent. *(It also judged HRW Answers until 2026-11-07, when the
    Answer was removed.)*
  - `matching_ledger` and `pantelides_ladder` read trace data as reference values.
- **Review the trace diff anyway** — it tells you what the rebase changed about each specimen (a
  changed residual, a different tearing, a new or removed block), and after a large release that
  diff *is* the description of what you just adopted.
- **There is no prose to re-read.** Until 2026-07-29 each entry carried a `narrative.md` whose claims
  had to be re-checked against the trace here, and that was the single most expensive step of a pin
  bump. Those narratives are retired (`docs/ideas.md` #42): Claude regenerates the explanation on
  demand, and `purpose.md` makes no claim a rebase can invalidate. The trace is generated, so it
  cannot go stale either.

## 5b. Regenerate the OTHER generated documents — three were missing from this list

**There are five generators and only two were named here** (`gen_field_help`, step 4;
`gen_trace`, step 5). Found 2026-11-03, during the 0.10.0 rebase, by noticing that a change to
the matching ledger necessarily invalidated a committed document no step mentioned:

```text
cargo run -p hrw --example gen_matching_reference    # docs/.../matching-live-reference.md
cargo run -p hrw --example gen_architecture          # docs/architecture.md's pipeline table
cargo run -p hrw --example gen_lab_catalogue         # the lab catalogue
```

**`gen_matching_reference` is the rebase-sensitive one.** It reads `matching.rs` for the emit-site
line of every `MatchingStep` variant *and* runs the real matching over a specimen, so a rebase
that moves a line, adds a step variant, or changes the matching API stales it on both axes. The
0.10.0 rebase did all three.

**Each is caught by a test, so this step is about ordering rather than safety.**
`matching_ledger::tests::the_generated_reference_is_current` and
`arch_doc::the_stage_roster_matches_stagekind` fail when their document is stale. Running the
generators here means the gate confirms the work; skipping it means the gate *reports* the work,
after you thought you were finished.

**Regenerating `matching-live-reference.md` moves line numbers that `matching-live.md` cites**, so
step 6 below follows directly from this one.

## 6. Update guided labs
- Guided labs (`docs/compiler-phases/*/guided-lab.md`) contain **line numbers, code snippets,
  and local-variable names** from the Rumoca crates. A rebase that moves code, renames locals, or
  changes trace-step variants will silently stale the lab without any compiler error.
- **KNOWN STALE FOR 0.10.0: every `equation from X` label in lab prose** *(confirmed
  2026-11-03 by reading the renderer, not by inference)*. `equation_label` now returns
  `"{equation} ({source_text})"` — the equation's own Modelica source line — where 0.9.20
  returned `"f_x[4] (equation from R)"`, naming the component it came from. So a lab table of
  `equation from R` / `equation from src` rows with counts beside them describes labels HRW no
  longer renders.

  Affected: `docs/fixture-labs/connect-expansion.md` (a counted table),
  `docs/fixture-labs/matching.md`, and
  `docs/compiler-phases/phase6_dae_construction/dae_construction.md`. **Nothing catches this** —
  `fixture_lab_links_all_resolve` checks links, not prose, and the numbers beside the labels are
  still arithmetically consistent. It needs a real compile to correct against, so it is gated on
  the build being green.

  **`src/doc_citations.rs`'s `"f_x[4] (equation from R)"` must NOT be changed.** It is inside a
  test comment recording what Doug's capture actually read on 2026-08-15 — a true statement about
  that day, in the same category as `DECISIONS.md`'s pre-rename vocabulary. Correcting it would
  falsify the history the comment exists to carry.
- After fixing compile breakage (step 2), grep the lab files for any function, type, or variable
  name that changed. Pay special attention to:
  - `LiveTrace::push` line number (the breakpoint site)
  - `MatchingStep` / `TarjanStep` enum variants (the frame types the lab runs through)
  - `augment_traced` / `strongconnect` parameter lists and local names
  - `emit_matching_frame` / `TracedTarjanState::record` call sites
- Update the affected labs to match the new code — line numbers, code excerpts, and locals tables.

## 6b. Re-measure the lectures — the ONLY time they are checked

**`docs/lectures/` is exempt from the gate by design and re-measured here instead.** Doug,
2026-09-24: *"Add lectures to the rebase checklist so that the lectures are checked only during
rebase. Otherwise, there does not seem to be a need to test lectures after edits."* He reads them in
**VS Code**, not in HRW, and no test can judge whether a lecture teaches — so the one checkable
property is whether its **numbers are still true**, and that can only change when Rumoca does.

**The split to hold in mind while working down a lecture:**

- **Hand-worked arithmetic is permanent.** Lecture 1's four forward Euler steps give
  $\varphi(1) = 0.375$ against a true $0.5$ for reasons no compiler can affect. Do not re-measure
  it; do not "verify" it against Rumoca, which is not running that method.
- **Anything Rumoca reported must be re-measured.** Step records, orders, step counts, errors
  against a closed form, tolerance tables, stage readouts, and every *"Rumoca stops here"* claim.
  **These are the rot.**

**The reliable way to find them: each lecture's header names the version it was measured against**
(*"Measured against Rumoca 0.9.20 on 2026-09-24"*). Grep `docs/lectures/` for the old version
string; every hit is a claim to re-take, and updating the header is how the next rebase knows what
it is looking at.

**Known to change at 0.10.0**, from `specimen-ladder.md`'s own rot list — so expect lecture edits
wherever a lecture leans on one of these:

- the initialization defect is fixed, so `RcCircuit`, `OverInitRc`, `Drivetrain` and
  `GearWithBrake` come back, and **rung 4's `LoopWithInertia` may stop being flat**
- `LoopWithInertia` and `BenchActuator`'s `L.i` were both recorded healthy while flat-lined
  (2026-09-24) — **a lecture citing either as working is suspect even before the rebase**
- `StabilizedPendulum`'s open question resolves either way: does IC planning match `lambda`?

**And a lecture may become WRONG IN ITS ARGUMENT, not merely in a digit.** Lecture 1's §4 explains a
constant `phi` offset by *"exactly one step ran at order 1"*. If the rebase changes startup order
selection, the numbers and the explanation both move, and only the explanation will look fine.
**Re-read the reasoning around any number that changed**, rather than substituting the new value.

## 7. Refresh `docs/compiler-phases/` — if phases changed
- **Claude maintains these and commits them.** *(Corrected 2026-08-01. This step said they were
  "Doug's authored explanations" that "Claude does not rewrite automatically" — a framing
  corrected on 2026-07-29, when it was established that **Claude wrote 100% of them**, on
  Doug's request. `CLAUDE.md` was updated then and this step was not, so the two documents
  contradicted each other and this one told Claude to refuse work that is its job.)*
- The audience is **Claude, not Doug** — he reads them only indirectly, through answers. Their
  job is to make Claude a better teacher over months.
- If a phase's behaviour changed materially, update the affected pages. **Re-tag provenance
  rather than leaving stale `verified` claims**: a tag is a claim about trustworthiness, and
  one naming a file that moved is worse than no tag. See [`provenance.md`](provenance.md).

## 8. Smoke-test the app
- `cargo run`; load a specimen; confirm each stage renders, and the bridge capture / field-help
  tooltips / "Go to" navigation / debugger arming still work.

## 9. Confirm the version/commit readout (continues below — step 10 is new)
- **Help → About auto-updates** — `build.rs` reads `rumoca-compile`'s version from `Cargo.lock` and
  the **commit from the workspace git HEAD** (HRW is an in-workspace member, so HEAD *is* the Rumoca
  source it's built against). Do **not** hand-edit it; just confirm About shows the new version/commit
  after rebuilding (a good final sanity check that the rebase landed).

## 10. Run the long-running suites — **after** the rebase closes, not during it

**Doug's ruling, 2026-11-05**, made while the 0.10.4 rebase was still open: *"after we complete the
rebase we need to perform the long-running tests such as the fidelity test and the oracle test."*
So this is a step of its own at the end, not a line inside 3b.

**Why it is not simply 3b again.** 3b runs *during* the rebase to say what the release changed, and
on the 0.10.4 hop it ran on night 8 and found **F4 blind since v0.10.0** — so that run measured
less than it appeared to, and the committed artifacts in [`reports/`](reports/) were stale as well
as wrong. A suite that was broken when it ran has to run again after it is fixed; **a green sweep
covers the territory it measured and no more.** The same applies to anything else repaired between
3b and here.

*(That sentence named `docs/reports.md` until 2026-11-06. Wrong file: `reports.md` is the design
authority for how the three reports compose and carries no F4 claim and no corpus counts, so a
session sent to correct it finds nothing to correct and may conclude the warning was spent. What
was stale was `docs/reports/` — the committed CSVs, which still held the 2026-08-03 run against
0.9.x. **A pointer that resolves to a real file it does not mean is worse than a broken one**,
because nothing fails and the reader's own reading gets blamed.)*

**On the 0.10.4 hop this step closed on 2026-11-06**: the corpus sweep ran green over all eight
checks with F4 repaired, and `promote_run` replaced the August artifacts. The result and what it
does *not* establish are in [`../DECISIONS.md`](../DECISIONS.md).

```text
# the stratified MSL sweep (hours; own runbook and watchdog)
cargo run -p hrw --example fidelity_msl -- --out <file>
# every slow test, including the ignored set a release may have repaired
cargo test -p hrw --lib --features slow-tests -- --test-threads=1
cargo test -p hrw --lib --features slow-tests -- --test-threads=1 --ignored
cargo test -p hrw --test msl_resolve --features slow-tests -- --test-threads=1
# the committed notebook's values, which the DOCS gate lane skips
cargo test -p hrw --lib --features notebook-check -- --test-threads=1 the_committed_notebook
```

**Then the oracle**, which is the strongest single statement this project can make about a release:
System Modeler over every specimen. **The count is whatever `hrw/specimens/*.mo` holds** — the
`ls` below is the definition, so nothing here spells a number.

```bash
ls hrw/specimens/*.mo > /tmp/specimens.txt
"/c/Program Files/Wolfram Research/WolframScript/wolframscript" \
  -script hrw/scripts/oracle.wls \
  -models /tmp/specimens.txt \
  -out "$TEMP/oracle-specimens.csv" \
  -kinds hrw/docs/reports/specimen-fidelity-report.csv \
  -timeout 90
```

About **3–7 s per specimen**, so a couple of minutes for the set — this is the one part of step 10
that is not long.

**Point `-out` at the committed report to top it up instead.** The resume reads the output back and
skips any `name` already present, so pointing it at
`hrw/docs/reports/specimen-oracle-report.csv` adjudicates **only the specimens added since the last
sweep** and leaves the rest untouched. That is how `ArrayFamily` was adjudicated on 2026-11-06 for
a few seconds of System Modeler time rather than a full re-run — and it is the step that keeps a
**new specimen** from sitting in the corpus un-adjudicated, which `specimen-oracle-report.csv`
cannot itself detect, since a missing row and a specimen that was never authored look identical.

**Use the script, NOT the Wolfram MCP server.** This step read *"through the Wolfram MCP server …
(recipe in `ideas.md` #43)"* until 2026-11-06, and that recipe is right for **one** adjudication
and wrong for a sweep: the MCP tool aborts a call after 30 minutes of silence, a sweep prints
nothing until it returns, and an aborted call **discards everything the kernel computed**. Three
calls and 90 minutes went that way. [`long-runs.md`](long-runs.md) has the full account, including
the `-script` flag trap and the two silent traps the script handles for you.

**Oracle-first still applies** — where Rumoca and the oracle disagree, that is not yet a Rumoca bug
until the specimen itself has been cleared.

**And `Flagged` is a rejection.** On 2026-11-06 three specimens were nearly filed as mismatches
because Rumoca *flagged* them while System Modeler refused them. A flag means the problem was
reported and the pipeline continued; it is not acceptance. Compare **"did the tool report a
problem"**, not "did the tool stop" — one of the three was a specimen authored to provoke exactly
the diagnosis Rumoca gave it.

**A mismatch is not yet a finding.** [`reports.md`](reports.md) requires fidelity green for the
model first, or the mismatch may be the instrument lying rather than the compiler erring — which
means a specimen the oracle disagrees about needs a row in `specimen-fidelity-report.csv`, i.e. a
place in `fidelity::MODELS`.

**Announce the cost before starting.** These are hours, not minutes, and
[`long-runs.md`](long-runs.md) owns the safety rules.

## 11. Propose a comprehensive tech-debt sweep — the last step

**Doug's ruling, 2026-11-07**, after the 0.10.0 and 0.10.4 rebases and a day of feature changes:
*"Some tech debt sweeps are not forward-looking. In fact, our document which contains the
procedure for rebases should have a final step which suggests a tech debt sweep."*

**Why a rebase earns one.** The other sweep triggers in [`tech-debt.md`](tech-debt.md) look
**forward** (what the next phase will touch) or at **what has been costing mistakes**. A rebase
leaves debt that neither one aims at, because it sits in what was *changed* rather than in what
comes next:

- **Port residue.** The v0.10.0 port (`b2f75914`) turned 32 `\` line continuations into runs of
  spaces inside string literals, three of them fields emitted to Claude. The compiler accepted
  all of it, and it was found **four days later**, by eye, in a screenshot.
- **Renamed and removed things still described in prose.** Comments, docs, emitted strings and
  lab text name the old surface, and no compiler error points at a sentence.
- **Workarounds whose reason is gone.** A shim for the old API, a disabled surface waiting on a
  re-model, an `upstream-issues.md` entry the release fixed (step 3c catches only the last).

**What to do.** Propose the sweep to Doug with a scope and a cost; do not start it unasked,
because it competes with whatever he wants next. When it runs, sweep the **rebase's diff** and
the feature work since, not the whole tree. Record what it finds in the who-caught-it ledger, so
the next rebase can see which residue the toolchain now catches.

---

**Note on commands:** run everything from the workspace root with `-p hrw`
(`cargo build -p hrw`, `cargo test -p hrw`, `cargo run -p hrw`,
`cargo run -p hrw --example gen_trace -- <Model>`), or `cd hrw/` first. The steps **above** still
read `cargo build` / `cargo test` for brevity — this note sits at the end of the document, so
"below" pointed at nothing.
- Still manual (prose, not derived): **check the pinned base SHA and version in `CLAUDE.md`'s
  rebase box** — the line reading `upstream/main@<sha>` (version `<x.y.z>`) — and add a
  `DECISIONS.md` entry recording the bump and anything non-trivial it required.
  - *(Corrected 2026-11-06: this said "the pinned rev noted in `CLAUDE.md` (Reference
    documentation)". There is no such section, and a step that sends the next rebase looking for
    one wastes its time. The box is what it meant.)*
  - **Do not spell a commit count there.** The line said "95 commits on top of" and was at 147
    before anyone looked; it now names no number. Same rule as the corpus size — a figure that
    drifts with ordinary work does not belong in prose.

---

**Rule of thumb:** step 0 is research and decides whether the rest happens at all; steps 1–3 are
mechanical and compiler-driven; 3b–3c say what the release actually changed; steps 4–5 are one
command each (then review the diffs); step 6 is a targeted search-and-update; steps 7 and 9 are
human judgement; **step 10 is hours of machine time and is the one that cannot be hurried**;
step 11 proposes the sweep that clears what the port left behind. If
`cargo build` + `cargo test` are green and the field-help + trace diffs look sane, the update is in
good shape — **but not finished until step 10 has run**, because the pre-commit gate deliberately
excludes every suite that would make it slow.

**The rule of thumb does NOT cover a release of 0.10.0's size, and saying so is the point.** It was
written for the deltas this fork had actually seen — a handful of upstream commits, where the
compiler finds nearly everything. **194 commits across 2,377 files with the IR crates restructured
is a different act**: the compiler still finds the type errors, but the judgement calls (which new
phase maps to which pane, which stage owns which crate's tooltips, what the trace diff means) are
the bulk of the work and none of them fail loudly. Treat that size as **an arc with its own plan**,
not as an afternoon following this list. The list is still correct; it is just no longer the
expensive part.
