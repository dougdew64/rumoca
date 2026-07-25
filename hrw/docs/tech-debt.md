# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 items
fixed in the 2026-07-25 cycle. See git history for details.

---

## Code quality / duplication

- [ ] **`compile()` is 285 lines with an inlined `macro_rules!`.** *(deferred — large structural refactor, low urgency)*
  The single method handles file I/O, output capture, all ten pipeline stages,
  progress streaming, def-index building, equation-sheet construction, and
  cleanup. The `run_stage!` macro captures five variables from the enclosing
  scope. Consider extracting stage-extraction and progress-emission into
  smaller functions.
  *File:* `worker.rs` — `compile()` (~lines 921–1205).

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  The bridge tests (`write_stages_creates_and_removes_files`,
  `write_creates_focus_json`, `live_trace_breakpoint_arm_remove_and_ack`) all
  write to the shared compile-time-resolved `BRIDGE_DIR`. Under `cargo test`
  with parallel threads, these tests race on shared filesystem state.
  *File:* `bridge.rs` — test module.
