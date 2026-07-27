# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 items
fixed in the 2026-07-25 cycle, plus 25 items completed in the 2026-07-25
sweep, plus 20 items completed in the 2026-07-26 sweep (stale LiveTrace docs,
architecture.md updates, ideas/CLAUDE.md updates, byte_offset_to_line dedup,
Tarjan borrow consistency, field group numbering, tree.rs tests, traced
index-reduction tests, gen_trace --all, GearWithBrake narrative). See git
history for details.

---

## Test gaps

- [ ] **Flaky `output_capture_handles_large_write_without_deadlock` test.**
  Captures 0 bytes instead of 128KB. Pre-existing failure unrelated to recent
  changes — the pipe/dup2 stdout redirect is not capturing on this WSL2
  instance. Will become moot after Windows migration.
  *File:* `worker.rs` line 3339.

## Code quality / duplication

- [ ] **`generic_error_summary()` is 217 lines.**
  Dispatches on 6 error kinds with inline UI. Each branch could be a helper.
  *File:* `app.rs` lines ~1573–1789.

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

- [ ] **No batch narrative regeneration.**
  Narratives are written one at a time by Claude. After a Rumoca rebase or
  trace regeneration, all 14 narratives may need review/refresh — no script
  or checklist drives this. Add a batch workflow (script that iterates
  specimens and invokes Claude for each narrative, or at minimum a checklist
  in `docs/updating-rumoca.md`).

## Debugging

- [ ] **LLDB step-over deadlocks during live-trace debugging.**
  Stepping over (`F10`) in VS Code's CodeLLDB debugger deadlocks when a
  breakpoint is inside a Rumoca algorithm that pushes to a LiveTrace channel
  (e.g. `live_trace_breakpoint` in matching). Continue (`F5`) between
  breakpoints works; only step-over hangs. Tested with
  `thread step-over -m all-threads` — same deadlock. Hypothesised to be a
  WSL2 ptrace issue, but not yet confirmed on native Windows. Migrating to
  native Windows (where CodeLLDB uses Windows debug APIs instead of ptrace)
  is the planned next diagnostic step.

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  *File:* `bridge.rs` — test module.
