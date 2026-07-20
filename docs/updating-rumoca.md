# Updating the pinned Rumoca version

HRW depends on official Rumoca as a **git dependency pinned to a commit** (`rev` in
`Cargo.toml`). Moving to a newer Rumoca is deliberate and follows this checklist. The Rust
compiler and the test suite do most of the work; a few generated artifacts (the field-help table
and the per-specimen traces) need explicit regeneration.

## 1. Bump the pin
- In `Cargo.toml`, change `rev = "…"` on the `rumoca-*` git dependencies to the new commit.
- Re-lock: `cargo update -p rumoca-compile -p rumoca-ir-ast -p rumoca-phase-parse`
  (or delete the rumoca entries from `Cargo.lock` and `cargo build`). Commit the new `Cargo.lock`.

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

## 4. Regenerate the generic field-help table
- `cargo run --example gen_field_help` — re-extracts `///` field docs from the new
  `rumoca-ir-ast` into `src/field_help.json` (see `src/field_help.rs`).
- **Review the diff.** New/renamed/removed fields are a signal: a renamed field the app reads
  (e.g. in `worker.rs` or `tree.rs`) may need a matching code change; new fields may be worth
  surfacing. Removed fields hint at API breakage step 2 should also have caught.

## 5. Regenerate specimen traces + review narratives
- For each specimen with a notebook entry (`docs/notebook/<Model>/`):
  `cargo run --example gen_trace -- <SpecimenName>` — rewrites the six stage IR files under
  `trace/` and the `trace/manifest.json` (which stamps the new Rumoca rev).
- **Review the trace diff**, then re-read that specimen's `narrative.md` against it. The narrative
  cites specific trace locations, so a `git diff` on `trace/` points straight at any claim that may
  have gone stale (a changed residual, a different tearing, a new/removed block). Update the prose to
  match — the trace is ground truth, the narrative is the story told about it.

## 6. Refresh `docs/understanding/` — only if phases changed, and only by Doug
- These are Doug's authored explanations, matching a specific Rumoca commit. Claude does **not**
  rewrite them automatically. If a phase's behavior changed materially, Doug updates the chapter
  (or asks Claude to draft a diff for ratification). Their being pinned-behind is acceptable;
  silently overwriting them is not.

## 7. Smoke-test the app
- `cargo run`; load a specimen; confirm each stage renders, and the bridge capture / field-help
  panel / "Go to" navigation / debugger arming / "Read: specimen narrative" still work.

## 8. Update the pin references
- **Help → About auto-updates** — `build.rs` reads the pin from `Cargo.lock` and the dialog shows
  it via `env!(...)`. Do **not** hand-edit it; just confirm About shows the new version/git rev after
  rebuilding (a good final sanity check that the pin actually moved).
- Still manual (prose, not derived): update the pinned rev noted in `CLAUDE.md` (Reference
  documentation) and add a `DECISIONS.md` line recording the bump and anything non-trivial it required.

---

**Rule of thumb:** steps 1–3 are mechanical and compiler-driven; steps 4–5 are one command each
(then review the diffs); steps 6 and 8 are human judgement. If `cargo build` + `cargo test` are
green and the field-help + trace diffs look sane, the update is in good shape.
