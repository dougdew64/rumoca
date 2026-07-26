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

## Hardcoded values / missing constants

- [ ] **Inline color literals for solver diagnostics, source-map highlight in `app.rs`.**
  Solver plot colors and source-map highlight remain inline in `app.rs`.
  (Animation controls, SCC palette, and equation category colors moved to
  `colors.rs` in the 2026-07-25 sweep.)
  *File:* `app.rs`.

## Code quality / duplication

- [ ] **`ui()` is ~887 lines.** *(deferred — large structural refactor)*
  The `eframe::App::ui()` implementation handles menus, status bar, tour panel,
  specimen panel, hrw link dispatch, center panel (11 stage tabs, log, simulation,
  structural sub-views, equation sheet, source map, tree, navigation), and
  deferred actions. The stage-rendering section (~lines 2136–2370) could be
  extracted.
  *File:* `app.rs` — `ui()` (~lines 1544–2431).

- [ ] **`compile()` is 285 lines with an inlined `macro_rules!`.** *(deferred — large structural refactor, low urgency)*
  The single method handles file I/O, output capture, all ten pipeline stages,
  progress streaming, def-index building, equation-sheet construction, and
  cleanup. The `run_stage!` macro captures five variables from the enclosing
  scope. Consider extracting stage-extraction and progress-emission into
  smaller functions.
  *File:* `worker.rs` — `compile()` (~lines 921–1205).

- [ ] **Duplicated matching/tarjan animation rendering in `app.rs`.**
  Lines ~2260–2298 (matching) and ~2300–2338 (tarjan) are structurally
  near-identical (~80 lines): lazy-init `cached_incidence`, compute
  `is_live`/`finished`, call `live_debug_lifecycle`, handle `SpawnLive`,
  lazy-init the cached animation, call `.ui()`. A bug fix in one block could
  miss the other.
  *File:* `app.rs`.

- [ ] **Duplicated animation state machine in matching/tarjan views.**
  The `ui()` methods of `MatchingAnimation` and `TarjanAnimation` share ~40
  lines of identical logic: `sync_live`, empty-state handling, elapsed-time
  auto-advance, repaint requests, delegation to `animation_controls()`. Could
  extract into a shared `AnimationState` struct.
  *Files:* `matching_anim.rs` (~lines 159–204), `tarjan_anim.rs` (~lines 185–228).

- [ ] **Duplicated matrix canvas boilerplate.**
  Three matrix views repeat the same ~10-line pattern: `label_headroom`,
  `matrix_rect`, `bounds` subtraction, `canvas.show()`. Could be a
  `canvas.show_matrix(ui, cols, rows)` helper.
  *Files:* `spyplot.rs`, `incidence_view.rs`, `matching_anim.rs`.

- [ ] **Five view functions exceed 100 lines.**
  `matching_anim::draw_matrix` (~190 lines), `incidence_view::ui` (~175),
  `equation_sheet::build` (~127), `spyplot::ui` (~122),
  `tarjan_anim::draw_graph` (~123). The tracked-highlight and axis-label blocks
  in each could be extracted into helpers.

- [ ] **`source_map_ui()` is ~229 lines.**
  Builds both the left pane (source code) and right pane (equations) with click
  handling and cross-highlighting. Could split into two methods.
  *File:* `app.rs` — lines ~1298–1527.

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  The bridge tests all write to the shared compile-time-resolved `BRIDGE_DIR`.
  Under `cargo test` with parallel threads, these tests race on shared
  filesystem state.
  *File:* `bridge.rs` — test module.
