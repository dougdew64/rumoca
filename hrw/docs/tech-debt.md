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

**2026-07-27 reconciliation** (not a sweep — the 07-26 sweep was the day
before). Two items retired as resolved: the WSL2 LLDB deadlock (development
moved to native Windows; the deadlock is gone, and the breakpoint failure it
was entangled with turned out to be linker COMDAT folding of the trace anchor —
see `architecture.md` § Live trace debugging on Windows) and the unverified
`output_capture_handles_large_write_without_deadlock` test (now passes in every
full run on native Windows). Three items added from that day's live-trace and
UI work, below.

---

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
  `ui()` methods share ~40 lines of identical logic. *Grew on 2026-07-27:*
  `live_state(&self, arming)` is now byte-identical in all three (~14 lines
  each), as is the "drop timed playback while busy / repaint while busy /
  return `debug_clicked`" sequence. A shared `LiveAnimation` trait — or a small
  embedded struct holding `frames`/`cursor`/`playing`/`live_rx`/`live_done` with
  the common methods on it — would collapse all of it.
  *Files:* `matching_anim.rs`, `tarjan_anim.rs`, `reduction_anim.rs`.

- [ ] **Repeated live-debug wiring at the three `app.rs` call sites.**
  Each animation view repeats the same six-step sequence: `is_arming` →
  `live_state` (with an `Idle`/`Arming` fallback when no animation exists) →
  `has_live_debug_data` → `live_debug_poll` → `anim.ui(.., arming,
  debug_enabled)` → `if debug_clicked { start_live_debug(..) }`. ~12 lines x 3,
  differing only in the `PendingLiveDebug` variant and the cached-animation
  field. Compounds the "duplicated animation rendering" item above; the two
  should be fixed together.
  *File:* `app.rs` (~2762, ~2820, ~2885).

- [ ] **`animation_controls` takes 8 positional parameters.**
  Two are adjacent `bool`s (`live`, `debug_enabled` — plus `&mut bool playing`
  earlier in the list), so transposing arguments compiles silently. Grouping the
  cursor/playing/elapsed/interval quartet into an `AnimationPlayback` struct,
  and passing `live`/`debug_enabled` as one small state struct, would make
  misuse a type error.
  *File:* `lib.rs`.

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

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  *File:* `bridge.rs` — test module.
