# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 items
fixed in the 2026-07-25 cycle. See git history for details.

---

## Bugs / correctness

- [ ] **Reduction view uses exact equality for tracked-identifier matching.**
  The reduction view checks `tracked == Some(name.as_str())` while every other
  view also falls back to `identifier_index::matches_tracked()`, which handles
  derivative wrappers (`der(h)` matching `h`) and equation text. Tracking a
  variable like `h` in the reduction view will not highlight derivative or
  equation-text mentions.
  *File:* `reduction_view.rs` — lines ~289, 332, 370.

- [ ] **Debug mode specimen dropdown only appears after a specimen is loaded.**
  The dropdown is gated on `self.ui_mode == UiMode::Debug` but the combo box
  shows the current specimen name (`self.selected`). If HRW starts in Debug
  mode (or the user switches to Debug before loading a specimen), the dropdown
  shows "(none)" but the file list is available — the real issue is that the
  dropdown should work as the primary specimen selector in Debug mode even when
  nothing is loaded yet. Verify the dropdown is functional for initial specimen
  selection, not just switching.
  *File:* `app.rs` — specimen switcher in the tab bar header.

- [ ] **Stale user-facing help text says "five stages" when there are ten.**
  The help window (line ~1081) says "Every capture publishes all five stages'
  full IR" — written early in development before the full pipeline was wired.
  *File:* `app.rs` — help window text.

## Hardcoded values / missing constants

- [ ] **Gold tracked-identifier color `(0xFF, 0xD5, 0x4F)` hardcoded in 17+ places.**
  The same RGB triple appears as a raw literal across 8 files (`app.rs`,
  `tree.rs`, `spyplot.rs`, `incidence_view.rs`, `matching_anim.rs`,
  `tarjan_anim.rs`, `reduction_view.rs`, `equation_sheet.rs`) with varying
  alpha (`0x30`, `0x40`, opaque). Should be a `TRACKED_GOLD` constant in
  `colors.rs` with alpha helpers.
  *Files:* 8 files, ~17 call sites.

- [ ] **Zoom threshold `16.0` for axis labels hardcoded in 3 matrix views.**
  `spyplot.rs`, `incidence_view.rs`, and `matching_anim.rs` all use `16.0`;
  `tarjan_anim.rs` uses `10.0` without explanation. Should be a shared constant.
  *Files:* `spyplot.rs`, `incidence_view.rs`, `matching_anim.rs`, `tarjan_anim.rs`.

- [ ] **Inline color literals for animation controls, SCC palette, equation categories, solver diagnostics, source-map highlight.**
  Several color values defined inline that should move to `colors.rs`:
  `animation_controls` in `lib.rs` duplicates `ANIM_PATH_FOUND`/`ANIM_FAIL`;
  SCC palette in `tarjan_anim.rs`; equation category colors in
  `equation_sheet.rs`; solver plot colors and source-map highlight in `app.rs`.
  *Files:* `lib.rs`, `tarjan_anim.rs`, `equation_sheet.rs`, `app.rs`.

- [ ] **Hardcoded layout ratios scattered through `app.rs`.**
  Panel widths (`0.4`), specimen list height (`panel_height / 3.0`), source-map
  split (`0.45`), trajectory plot height (`0.65`) — no named constants.
  *File:* `app.rs` — lines ~1621, 1637, 1642, 1336, 972.

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

- [ ] **Duplicated `section_header` / `section_header_toggle` styling.**
  Both functions compute the same four theme-dependent colors, the same
  `h_margin`, and the same `outer_margin`. The shared setup could be extracted.
  *File:* `app.rs` — lines ~2463–2534.

- [ ] **`find_whole_identifier` and `matches_tracked` have near-identical loops.**
  Both iterate through a haystack with byte-boundary checks. Only differences:
  dot-as-word-char and return type (position vs bool). Could unify into a
  single parameterized function.
  *File:* `identifier_index.rs` — lines ~130–168.

- [ ] **Five view functions exceed 100 lines.**
  `matching_anim::draw_matrix` (~190 lines), `incidence_view::ui` (~175),
  `equation_sheet::build` (~127), `spyplot::ui` (~122),
  `tarjan_anim::draw_graph` (~123). The tracked-highlight and axis-label blocks
  in each could be extracted into helpers.

- [ ] **`source_map_ui()` is ~229 lines.**
  Builds both the left pane (source code) and right pane (equations) with click
  handling and cross-highlighting. Could split into two methods.
  *File:* `app.rs` — lines ~1298–1527.

## Dead code / stale comments

- [ ] **Dead field: `cached_specimen_width: Option<f32>` never read.**
  Declared, initialized to `None`, and invalidated in `rescan()`, but never
  read anywhere.
  *File:* `app.rs` — line ~269.

- [ ] **Dead method: `pub fn set_mode()` never called.**
  The View menu sets `self.ui_mode` directly. This method is unreachable.
  *File:* `app.rs` — line ~501.

- [ ] **Stale comment: `open()` ordering description is backwards.**
  Comment says "LoadAndSwitch sets pending_stage BEFORE calling open()" but
  the code does `self.open(path)` first, then sets `pending_stage`.
  *File:* `app.rs` — line ~606.

- [ ] **Orphan doc comment before `#[cfg(test)]` block.**
  A doc comment starting "The field name to look up generic help for..." ends
  mid-sentence and attaches to the test-only `impl App` block.
  *File:* `app.rs` — lines ~2657–2664.

- [ ] **Stale module doc lists nonexistent `format_equation_residual`.**
  The module doc lists this as an entry point, but no such function exists.
  *File:* `expr_format.rs` — line ~11.

- [ ] **Orphan `truncate_label` comment in incidence_view.**
  A documentation comment for `truncate_label` (defined in `lib.rs`) sits
  above `#[cfg(test)] mod tests` in the wrong file.
  *File:* `incidence_view.rs` — lines ~462–464.

- [ ] **Misplaced import in tarjan_anim.**
  `use crate::truncate_label;` appears after the impl block instead of with
  the other imports at the top.
  *File:* `tarjan_anim.rs` — line ~415.

- [ ] **Redundant `#[cfg(test)]` on `test_default()`.**
  Already inside a `#[cfg(test)]` impl block.
  *File:* `app.rs` — line ~2667.

## Robustness

- [ ] **`Worker::send` swallows send errors with `eprintln`.**
  When the worker thread has exited (e.g., panic), `send` logs to stderr and
  drops the message. Callers have no way to detect this failure — subsequent
  UI actions silently do nothing. Should set a flag the UI can display.
  *File:* `worker.rs` — lines ~601–605.

- [ ] **`build_def_index` silently truncates u64 DefId to u32.**
  `name_by_id.get(&(id as u32))` — safe today because Rumoca's `DefId` is
  internally `u32`, but the cast is implicit and undocumented. A future Rumoca
  change could widen `DefId` without a compile error here.
  *File:* `worker.rs` — line ~1831.

- [ ] **Bridge test filesystem races under parallel execution.** *(deferred — requires parameterizing bridge dir; mitigated by `--test-threads=1`)*
  The bridge tests all write to the shared compile-time-resolved `BRIDGE_DIR`.
  Under `cargo test` with parallel threads, these tests race on shared
  filesystem state.
  *File:* `bridge.rs` — test module.

## Test coverage gaps

- [ ] **No test for `open()` field-reset logic.**
  `open()` resets ~15 fields. No test verifies the reset is complete — a new
  field added to `App` but forgotten in `open()`'s cleanup would go undetected.
  *File:* `app.rs` — line ~575.

- [ ] **No test for `StageBundle::as_stage_pairs` ordering.**
  Returns a fixed-order array of 10 pairs. No test asserts the names stay in
  sync with `STAGE_FILE_NAMES` in `bridge.rs` — a reorder or rename in one
  but not the other would produce mismatched bridge files.
  *File:* `worker.rs` — lines ~359–372.

- [ ] **No direct unit test for `IdentifierIndex::build`.**
  The constructor is only exercised via the integration test
  `compile_produces_identifier_index_for_healthy_specimen` in `worker.rs`.
  A focused unit test against a mock `Dae` would catch partition-iteration
  regressions (e.g., a new partition added to `Dae::variables` but not wired
  into `build`).
  *File:* `identifier_index.rs` — lines ~41–56.

## Minor style / cleanup

- [ ] **Redundant `font_size` computation in `draw_matrix_axis_labels`.**
  Computes the same `(view.zoom() * 0.35).min(14.0)` expression twice — once
  for the `FontId` and again as a local `font_size`. Should extract once.
  *File:* `lib.rs` — lines ~154, 157.

- [ ] **`format_expr_into` allocates via `format!("{op}")` on a hot path.**
  Could use `write!(out, " {op} ")` to avoid intermediate `String` allocation.
  Called on every expression node during equation-sheet construction.
  *File:* `expr_format.rs` — lines ~86, 98.

- [ ] **`is_some()` + `.clone().unwrap()` instead of `if let`.**
  One place uses `if self.tracked_identifier.is_some() { ... .clone().unwrap() }`
  instead of the idiomatic `if let Some(name) = ... { }`.
  *File:* `app.rs` — lines ~2178–2179.

- [ ] **`EquationCategory` has duplicated ordering — `Ord` impl and `display_order` array.**
  The `cmp_key()` method and the `display_order` array encode the same
  ordering independently. One source of truth would be cleaner.
  *File:* `equation_sheet.rs` — lines ~220, 296, 392–414.
