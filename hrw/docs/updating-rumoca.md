# Updating Rumoca (rebasing the `hrw` branch on upstream)

HRW now lives **inside a fork of the Rumoca workspace** (`hrw/`), depending on the Rumoca crates
via **path deps** (see [`decisions`](../../DECISIONS.md) — the in-workspace move). So "updating
Rumoca" is no longer a pin bump; it's **rebasing the `hrw` branch on a newer upstream** and fixing
the fallout. The Rust compiler and the test suite do most of the work; a few generated artifacts
(the field-help table and the per-specimen traces) need explicit regeneration.

## 1. Rebase the `hrw` branch on upstream
- `git fetch upstream` (CogniPilot/rumoca), then `git rebase upstream/main` (or a chosen upstream
  rev) on the `hrw` branch. Rumoca's own code advances underneath; your **additive instrumentation
  hooks** should rebase cleanly (that's the point of keeping them observation-only), and `hrw/`
  itself won't conflict — upstream has no files there.
- Path deps track the checked-out tree automatically — no `rev` to edit. `cargo build -p hrw`
  re-locks against the new Rumoca. Commit the updated `Cargo.lock`.
- Resolve any conflicts in the instrumentation hooks against the moved phase code, then continue.

## 2. Fix compile breakage — the compiler is the guide
- `cargo build`. Rust flags every API change HRW's code relies on: `Session` methods,
  `parse_to_ast`, `ClassTree` lookups (`def_map`, `get_class_by_qualified_name`), the `DefInfo`
  extraction in `src/worker.rs`, serde field access, etc. Fix each error.
- Most Rumoca updates surface here as concrete type/name errors — not silent drift.

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

## 4. Regenerate the generic field-help table
- `cargo run --example gen_field_help` — re-extracts `///` field docs from the new
  `rumoca-ir-ast` into `src/field_help.json` (see `src/field_help.rs`).
- **Review the diff.** New/renamed/removed fields are a signal: a renamed field the app reads
  (e.g. in `worker.rs` or `tree.rs`) may need a matching code change; new fields may be worth
  surfacing. Removed fields hint at API breakage step 2 should also have caught.

## 5. Regenerate specimen traces
- For each specimen with a notebook entry (`docs/specimen-notebook/<Model>/`):
  `cargo run --example gen_trace -- <SpecimenName>` — rewrites the stage IR files under
  `trace/` and the `trace/manifest.json` (which stamps the new Rumoca rev).
- **Review the trace diff** — it tells you what the rebase changed about each specimen (a changed
  residual, a different tearing, a new or removed block). That review is for *your* understanding of
  the rebase; nothing else depends on it.
- **There is no prose to re-read.** Until 2026-07-29 each entry carried a `narrative.md` whose claims
  had to be re-checked against the trace here, and that was the single most expensive step of a pin
  bump. Those narratives are retired (`docs/ideas.md` #42): Claude regenerates the explanation on
  demand, and `purpose.md` makes no claim a rebase can invalidate. The trace is generated, so it
  cannot go stale either.

## 6. Update guided tours
- Guided tours (`docs/compiler-phases/*/guided-tour.md`) contain **line numbers, code snippets,
  and local-variable names** from the Rumoca crates. A rebase that moves code, renames locals, or
  changes trace-step variants will silently stale the tour without any compiler error.
- After fixing compile breakage (step 2), grep the tour files for any function, type, or variable
  name that changed. Pay special attention to:
  - `LiveTrace::push` line number (the breakpoint site)
  - `MatchingStep` / `TarjanStep` enum variants (the frame types the tour walks through)
  - `augment_traced` / `strongconnect` parameter lists and local names
  - `emit_matching_frame` / `TracedTarjanState::record` call sites
- Update the affected tours to match the new code — line numbers, code excerpts, and locals tables.

## 7. Refresh `docs/compiler-phases/` — only if phases changed, and only by Doug
- These are Doug's authored explanations, matching a specific Rumoca commit. Claude does **not**
  rewrite them automatically. If a phase's behavior changed materially, Doug updates the chapter
  (or asks Claude to draft a diff for ratification). Their being pinned-behind is acceptable;
  silently overwriting them is not.

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
`cargo run -p hrw --example gen_trace -- <Model>`), or `cd hrw/` first. The steps below still read
`cargo build` / `cargo test` for brevity.
- Still manual (prose, not derived): update the pinned rev noted in `CLAUDE.md` (Reference
  documentation) and add a `DECISIONS.md` line recording the bump and anything non-trivial it required.

---

**Rule of thumb:** steps 1–3 are mechanical and compiler-driven; steps 4–5 are one command each
(then review the diffs); step 6 is a targeted search-and-update; steps 7 and 9 are human judgement.
If `cargo build` + `cargo test` are green and the field-help + trace diffs look sane, the update is
in good shape.
