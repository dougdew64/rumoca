# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 16 items
fixed in the 2026-07-25 cycle. See git history for details.

---

## Bugs / correctness

- [x] **Shared spy-plot/incidence cache serves wrong stage data.**
  `cached_spy_plot` and `cached_incidence` are single `Option` slots shared by
  both the Structural and IndexReduction tabs. The cache is only invalidated on
  `Compiled` (line ~555), not on stage-tab switch. So if the user views
  Structural's spy-plot first, then switches to IndexReduction,
  `get_or_insert_with` returns the Structural data — the user sees the
  pre-reduction (singular) matrix instead of the post-reduction (solvable) one.
  Fix: either key the cache by `StageKind`, or invalidate both caches when
  `self.stage` changes between Structural and IndexReduction.
  *File:* `app.rs` — `get_or_insert_with` at lines ~1851 and ~1861.

- [x] **`simulate()` skips `remove_document` before `update_document`.**
  `compile()` at lines ~1025–1026 calls `session.remove_document(&uri)` before
  `update_document` to bypass the session's content-comparison cache (so armed
  breakpoints fire on recompile). `simulate()` at line ~727 only calls
  `update_document`. If the user simulates the same specimen twice without an
  intervening compile, the session short-circuits and uses cached results —
  armed breakpoints won't fire and source edits won't take effect.
  *File:* `worker.rs` — `simulate()` (~line 727).

- [x] **Tarjan `start_live` returning `None` leaks `live_breakpoint_armed`.**
  `TarjanAnimation::start_live` returns `Option<Self>` (returns `None` if
  `n_eq == 0`). At line ~1998, `live_breakpoint_armed` is set to `true` before
  the `start_live` call. If `start_live` returns `None`, the cache becomes
  `Some(None)`, and the `live_just_finished` cleanup (which requires a live
  animation to check `is_live()` on) never fires. The Debug button disappears
  permanently because `live_breakpoint_armed` stays `true`. Fix: check the
  `start_live` return value and clear `live_breakpoint_armed` if `None`.
  *File:* `app.rs` — Tarjan live-debug spawn at line ~2000.

- [x] **`reduction_view::expr_to_short` renders operators as enum names.**
  Binary operators render as `"x Add y"` instead of `"x + y"`, and unary
  operators render as `"Negatex"` instead of `"-x"` (no separator, no symbol
  mapping). This is user-facing text in the index-reduction elimination table.
  Existing tests codify the broken rendering rather than catching it. Fix: map
  enum variant names to symbols (Add→`+`, Mul→`*`, Negate→`-`, etc.) or use
  the `Display` impl that `expr_format.rs` uses.
  *Files:* `reduction_view.rs` — `expr_to_short` (~lines 415–424).

- [x] **`find_live_trace_line` silently falls back to hardcoded line 109.**
  When the `pub fn live_trace_breakpoint(` signature is not found in the source
  file (e.g., renamed after a Rumoca rebase), the function returns
  `unwrap_or(109)`. Both `arm_` and `remove_live_trace_breakpoint` then write
  a breakpoint request pointing at whatever happens to be on line 109 — an
  unrelated line with no error signal.
  *File:* `bridge.rs` — `find_live_trace_line` (~line 313).

## Code quality / duplication

- [x] **Live-debug state machine duplicated between Matching and Tarjan.**
  The three-phase lifecycle (idle → arming → running, with ack polling, thread
  spawn, `live_just_finished` cleanup, and Debug button rendering) is
  copy-pasted between the MatchingAnim and TarjanAnim branches (~80 lines
  each). They differ only in `PendingLiveDebug` variant, cache field, and
  canvas. Extract to a helper method parameterized on these.
  *File:* `app.rs` — lines ~1878–1958 vs ~1966–2022.

- [x] **`truncate_label` duplicated identically in 3 files.**
  The exact same function (truncate a string to N chars with `…` suffix) is
  copy-pasted in `incidence_view.rs`, `matching_anim.rs`, and
  `tarjan_anim.rs`. Extract to a shared utility in `lib.rs` or a common module.
  *Files:* `incidence_view.rs` (~line 489), `matching_anim.rs` (~line 544),
  `tarjan_anim.rs` (~line 453).

- [x] **Animation control UI duplicated between matching and Tarjan.**
  The play/pause/reset/step/speed-slider UI code is near-identical in both
  animation modules (~65 lines each). A shared `animation_controls` helper or
  a generic `AnimationPlayer<F>` would eliminate this.
  *Files:* `matching_anim.rs` (~lines 191–257), `tarjan_anim.rs` (~lines 216–282).

- [x] **Axis label rendering duplicated between incidence and matching.**
  Font size calculation, -45° column label rotation, `TextShape` construction,
  and row-label loops are copied between the two modules (~35 lines each).
  *Files:* `incidence_view.rs` (~lines 401–440), `matching_anim.rs` (~lines 422–454).

- [x] **`eq_index`/`var_index` HashMaps built twice in `incidence_view`.**
  `from_report` builds name→index lookup maps once for matching overlay parsing
  (lines ~146–155) and again identically for BLT block parsing (lines ~170–179).
  Build once and reuse.
  *File:* `incidence_view.rs` — `from_report`.

- [x] **`expr_format.rs` has identical if/else branches.**
  Lines ~86–95: both branches of the if/else produce identical output (space +
  operator + space). The condition can be simplified to `if !op_str.is_empty()`.
  *File:* `expr_format.rs` — `format_expr_into` (~lines 86–95).

- [ ] **`compile()` is 285 lines with an inlined `macro_rules!`.** *(deferred — large structural refactor, low urgency)*
  The single method handles file I/O, output capture, all ten pipeline stages,
  progress streaming, def-index building, equation-sheet construction, and
  cleanup. The `run_stage!` macro captures five variables from the enclosing
  scope. Consider extracting stage-extraction and progress-emission into
  smaller functions.
  *File:* `worker.rs` — `compile()` (~lines 921–1205).

## Robustness

- [x] **`OutputCapture` pipe can deadlock on verbose Rumoca output.**
  The fd-level stdout/stderr capture redirects into pipes, but the read end is
  only drained *after* each API call returns (`drain_output` is post-hoc).
  Linux pipes buffer 65536 bytes; if a Rumoca phase writes more during a single
  call, the write blocks (pipe full) while drain can't run — a deadlock. The
  worker thread hangs silently.
  *Fix:* reader threads now drain each pipe continuously into `Arc<Mutex<Vec<u8>>>`
  buffers, so the pipe never fills. `drain()` takes accumulated bytes. Readers
  see EOF and exit when `Drop` restores the original fds.
  *File:* `worker.rs` — `OutputCapture`.

- [x] **`Worker::send` silently discards messages to a dead worker.**
  `let _ = self.tx.send(req)` drops the `SendError` if the worker thread has
  panicked. The UI gets no feedback — a compile or simulate request is silently
  lost, and the user sees perpetual "compiling" with no result.
  *File:* `worker.rs` — `Worker::send` (~line 582).

- [x] **Breakpoint JSON built via `format!` string interpolation.**
  `arm_live_trace_breakpoint` and `remove_live_trace_breakpoint` construct JSON
  via `format!()`. If the file path or specimen name contains `"` or `\`
  (possible on Windows paths), the output is malformed JSON. Use
  `serde_json::json!` or a proper serializer instead.
  *Files:* `bridge.rs` — `arm_live_trace_breakpoint` (~lines 329–338),
  `remove_live_trace_breakpoint` (~lines 345–351).

- [x] **`incidence_view::from_report` does not sort column indices.**
  The `cell_at` method uses `binary_search` on column-index vectors, but
  `from_report` does not sort them — it trusts the producer (worker.rs) to
  emit sorted data. Currently safe because `incidence_to_json` sorts
  explicitly, but if the JSON is produced by another source (e.g., a
  future `gen_trace` change), the binary search silently returns wrong results.
  Add a defensive `cols.sort_unstable()` after parsing.
  *File:* `incidence_view.rs` — `from_report` (~lines 131–136).

- [x] **`data.data[i]` index access in simulation plotting.**
  The simulation plot loop iterates `data.names` by index and accesses
  `data.data[i]`. If the worker ever produces mismatched array lengths
  (a bug elsewhere), this panics. Use `zip` instead: `names.iter().zip(&data.data)`.
  *File:* `app.rs` — simulation plotting (~line 837).

- [x] **Thread spawn `.expect()` in both animation modules.**
  `start_live` in both `matching_anim.rs` and `tarjan_anim.rs` calls
  `.expect("failed to spawn thread")`. Thread creation can fail under resource
  limits. Propagate the error instead of panicking the UI thread.
  *Files:* `matching_anim.rs` (~line 108), `tarjan_anim.rs` (~line 136).

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  The bridge tests (`write_stages_creates_and_removes_files`,
  `write_creates_focus_json`, `live_trace_breakpoint_arm_remove_and_ack`) all
  write to the shared compile-time-resolved `BRIDGE_DIR`. Under `cargo test`
  with parallel threads, these tests race on shared filesystem state.
  *File:* `bridge.rs` — test module.

## Test gaps

- [x] **No tests for `drain_worker` message handling.**
  The state-transition logic (stale-result filtering, stage assignment, cache
  invalidation, live-debug cleanup) is the most complex code in `app.rs` and
  has zero test coverage.
  *File:* `app.rs` — `drain_worker`.

- [x] **No tests for `simulate` error paths.**
  `compile` error paths are tested (nonexistent file, invalid syntax, missing
  resolved tree). No corresponding error-path tests exist for `simulate`.
  *File:* `worker.rs`.

- [x] **No tests for `CompileProgress` streaming.**
  The progressive streaming pattern (emitting partial `StageBundle`s after each
  stage) is a key architectural feature with no test coverage. All tests
  discard progress messages.
  *File:* `worker.rs`.

- [x] **No test for `equation_sheet` output correctness.**
  The `equation_sheet` field is populated on `Compiled` but no test checks
  that it is `Some` for a successful compile or inspects its structure beyond
  the integration test `build_on_real_specimen`.
  *File:* `worker.rs`.

- [x] **No test for `slice_source` boundary conditions.**
  Only the happy path is exercised indirectly via `ascent_finds_tightest_location`.
  Missing: `start == end` (zero-length), `start == 0` (file start), `end == len`
  (file end), `start > end` (invalid range).
  *File:* `bridge.rs` — `slice_source`.
