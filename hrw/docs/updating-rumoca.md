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
- `git fetch upstream` (CogniPilot/rumoca), then `git rebase <the target chosen in step 0>` on the
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

## 2. Fix compile breakage — the compiler is the guide
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
| sub-view rosters | `SubView`, `STAGES_WITH_SUB_VIEWS` (`answer_check`) |

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
with a reference implementation on our 25 specimens?" is a script, not an afternoon. That comparison
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
  - `answer_check` judges an Answer's pointers against the notebook whenever the live bridge does
    not match the session's model. A stale notebook therefore makes Answer verification quietly
    wrong rather than absent.
  - `matching_ledger` and `pantelides_ladder` read trace data as reference values.
- **Review the trace diff anyway** — it tells you what the rebase changed about each specimen (a
  changed residual, a different tearing, a new or removed block), and after a large release that
  diff *is* the description of what you just adopted.
- **There is no prose to re-read.** Until 2026-07-29 each entry carried a `narrative.md` whose claims
  had to be re-checked against the trace here, and that was the single most expensive step of a pin
  bump. Those narratives are retired (`docs/ideas.md` #42): Claude regenerates the explanation on
  demand, and `purpose.md` makes no claim a rebase can invalidate. The trace is generated, so it
  cannot go stale either.

## 6. Update guided labs
- Guided labs (`docs/compiler-phases/*/guided-lab.md`) contain **line numbers, code snippets,
  and local-variable names** from the Rumoca crates. A rebase that moves code, renames locals, or
  changes trace-step variants will silently stale the lab without any compiler error.
- After fixing compile breakage (step 2), grep the lab files for any function, type, or variable
  name that changed. Pay special attention to:
  - `LiveTrace::push` line number (the breakpoint site)
  - `MatchingStep` / `TarjanStep` enum variants (the frame types the lab runs through)
  - `augment_traced` / `strongconnect` parameter lists and local names
  - `emit_matching_frame` / `TracedTarjanState::record` call sites
- Update the affected labs to match the new code — line numbers, code excerpts, and locals tables.

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

## 9. Confirm the version/commit readout
- **Help → About auto-updates** — `build.rs` reads `rumoca-compile`'s version from `Cargo.lock` and
  the **commit from the workspace git HEAD** (HRW is an in-workspace member, so HEAD *is* the Rumoca
  source it's built against). Do **not** hand-edit it; just confirm About shows the new version/commit
  after rebuilding (a good final sanity check that the rebase landed).

---

**Note on commands:** run everything from the workspace root with `-p hrw`
(`cargo build -p hrw`, `cargo test -p hrw`, `cargo run -p hrw`,
`cargo run -p hrw --example gen_trace -- <Model>`), or `cd hrw/` first. The steps **above** still
read `cargo build` / `cargo test` for brevity — this note sits at the end of the document, so
"below" pointed at nothing.
- Still manual (prose, not derived): update the pinned rev noted in `CLAUDE.md` (Reference
  documentation) and add a `DECISIONS.md` line recording the bump and anything non-trivial it required.

---

**Rule of thumb:** step 0 is research and decides whether the rest happens at all; steps 1–3 are
mechanical and compiler-driven; 3b–3c say what the release actually changed; steps 4–5 are one
command each (then review the diffs); step 6 is a targeted search-and-update; steps 7 and 9 are
human judgement. If `cargo build` + `cargo test` are green and the field-help + trace diffs look
sane, the update is in good shape.

**The rule of thumb does NOT cover a release of 0.10.0's size, and saying so is the point.** It was
written for the deltas this fork had actually seen — a handful of upstream commits, where the
compiler finds nearly everything. **194 commits across 2,377 files with the IR crates restructured
is a different act**: the compiler still finds the type errors, but the judgement calls (which new
phase maps to which pane, which stage owns which crate's tooltips, what the trace diff means) are
the bulk of the work and none of them fail loudly. Treat that size as **an arc with its own plan**,
not as an afternoon following this list. The list is still correct; it is just no longer the
expensive part.
