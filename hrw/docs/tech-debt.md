# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 items
fixed in the 2026-07-25 cycle, plus 25 items completed in the 2026-07-25
sweep (gold colors, dead code, tracking fix, stale comments, zoom/layout/color
constants, section_header dedup, identifier matching dedup, debug dropdown,
Worker::send/DefId robustness, EquationCategory ordering, format_expr alloc,
3 test coverage gaps filled). See git history for details.

---

## Stale comments / docs (from channel-based LiveTrace refactor)

- [x] **Stale module doc in `matching_anim.rs` line 34.**
  Fixed: now says "from an mpsc channel receiver".

- [x] **Stale comment in `matching_anim.rs` line 163.**
  Fixed: now says "from the channel receiver".

- [x] **Stale module doc in `reduction_anim.rs` line 6.**
  Fixed: now says "from an mpsc channel receiver".

- [x] **Stale doc comment in `matching.rs` line 46.**
  Fixed: now says `live_trace_breakpoint`.

- [x] **Stale module doc in `app.rs` lines 23–26.**
  Fixed: removed "Stages present so far: Parse, Resolve".

## Stale docs (architecture.md line counts and structure)

- [x] **Line counts in `architecture.md` are stale.**
  Fixed: updated all file line counts.

- [x] **Test count in `architecture.md` section 10.**
  Fixed: updated to current count.

- [x] **Crate structure tree in `architecture.md` section 2 omits `reduction_anim.rs`.**
  Fixed: added to tree listing.

- [x] **App struct field group numbering skips 13 (12→14).**
  Fixed in both `app.rs` and `architecture.md`: renumbered 14→13, 15→14, 16→15.

## Stale docs (ideas.md, CLAUDE.md)

- [x] **`ideas.md` #27, #28, #29 not marked done.**
  Fixed: added "Implemented" banners.

- [x] **`CLAUDE.md` says "12-specimen notebook".**
  Fixed: updated to 14.

- [x] **`CLAUDE.md` "Current initiative" describes delivered features as current work.**
  Fixed: updated to "Completed initiative".

## Test gaps

- [x] **`tree.rs` has zero unit tests.**
  Fixed: 17 tests covering `nav_target`, `def_annotation`,
  `collect_tracked_ancestors`, `header`, and `header_tracked`.

- [x] **No tests for traced index-reduction paths.**
  Fixed: added `emit_index_reduction_frame_pushes_to_both_vec_and_live_trace`
  and `emit_index_reduction_frame_works_without_live_trace` at the crate level,
  paralleling matching/tarjan `live_trace_receives_same_frames_as_returned`.
  HRW-level traced-reduction tests already existed (`drivetrain_index_reduction_produces_trace_frames`).

- [ ] **Flaky `output_capture_handles_large_write_without_deadlock` test.**
  Captures 0 bytes instead of 128KB. Pre-existing failure unrelated to recent
  changes — the pipe/dup2 stdout redirect is not capturing on this WSL2
  instance. Will become moot after Windows migration.
  *File:* `worker.rs` line 3339.

## Code quality / duplication

- [x] **Duplicated `byte_offset_to_line()` function.**
  Fixed: extracted to `lib.rs`, both call sites now use `crate::byte_offset_to_line`.

- [ ] **`generic_error_summary()` is 217 lines.**
  Dispatches on 6 error kinds with inline UI. Each branch could be a helper.
  *File:* `app.rs` lines ~1573–1789.

- [x] **Tarjan `TracedTarjanState` owns `LiveTrace` (cloned), inconsistent with matching/reduction (borrow-based).**
  Fixed: `TracedTarjanState` now borrows `&'a LiveTrace` like matching and reduction.

- [ ] **`ui()` is ~887 lines.** *(deferred — large structural refactor)*
  *File:* `app.rs`.

- [ ] **`compile()` is ~285 lines with an inlined `macro_rules!`.** *(deferred — low urgency)*
  *File:* `worker.rs`.

- [ ] **Duplicated matching/tarjan/reduction animation rendering in `app.rs`.**
  Three near-identical blocks (~80 lines each).
  *File:* `app.rs`.

- [ ] **Duplicated animation state machine in matching/tarjan/reduction views.**
  `ui()` methods share ~40 lines of identical logic.
  *Files:* `matching_anim.rs`, `tarjan_anim.rs`, `reduction_anim.rs`.

- [ ] **Duplicated matrix canvas boilerplate.**
  Three views repeat the same ~10-line pattern.
  *Files:* `spyplot.rs`, `incidence_view.rs`, `matching_anim.rs`.

- [ ] **Five view functions exceed 100 lines.**
  `draw_matrix` (~190), `incidence_view::ui` (~175), `equation_sheet::build`
  (~127), `spyplot::ui` (~122), `tarjan_anim::draw_graph` (~123).

- [ ] **`source_map_ui()` is ~229 lines.**
  *File:* `app.rs`.

## Build process / specimen notebook

- [x] **No batch trace regeneration.**
  Fixed: `gen_trace` now supports `--all` flag.

- [ ] **No batch narrative regeneration.**
  Narratives are written one at a time by Claude. After a Rumoca rebase or
  trace regeneration, all 14 narratives may need review/refresh — no script
  or checklist drives this. Add a batch workflow (script that iterates
  specimens and invokes Claude for each narrative, or at minimum a checklist
  in `docs/updating-rumoca.md`).

- [x] **GearWithBrake has a trace but no `narrative.md`.**
  Fixed: narrative written covering all 10 pipeline stages.

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  *File:* `bridge.rs` — test module.
