# Tech Debt — HRW Observatory

Prioritized quality improvements identified by code review (2026-07-22).
Items are grouped by theme, ordered by severity within each group.
Check off items as they are completed.

---

## Theme 1: Stage management boilerplate

The single biggest source of tech debt. Adding a new pipeline stage currently
requires touching 10+ locations across `app.rs` and `worker.rs`. These items
are related and should be addressed together.

- [x] **TD-1 (high): Store stages in a `StageBundle` on `App`, not as 10 individual fields.**
  Fixed 2026-07-22. `App` now stores a single `stages: StageBundle` field. `open()`
  resets with one `StageBundle::default()`. `drain_worker()` assigns with one
  `self.stages = stages`. `StageKind` moved to `worker.rs` (public), with
  `StageKind::name()` and `StageBundle::get(kind)` / `as_stage_pairs()` methods.

- [x] **TD-2 (medium): Eliminate `FromWorker::Compiled` / `StageBundle` duplication.**
  Fixed 2026-07-22. `FromWorker::Compiled` now embeds `stages: StageBundle` instead
  of 10 individual stage fields.

- [x] **TD-3 (medium): Factor the repetitive stage-emit loop in `compile()`.**
  Fixed 2026-07-22. A `run_stage!` macro captures `log`, `drain_traces`, `bundle`,
  `emit`, and `path` from the enclosing scope. Six invocations replace ~42 lines of
  repeated stage-emit boilerplate.

- [x] **TD-4 (medium): Data-drive the stage tab bar.**
  Fixed 2026-07-22. A `tabs` array of `(StageKind, &str, &Stage, Option<&str>)` and
  a loop replace ~90 lines with ~15. Adding a new tab is now a one-line array entry.

- [x] **TD-5 (low): Extract common fallback arms from stage-extraction functions.**
  Fixed 2026-07-22. New `not_reached_stage()` helper returns the placeholder `Stage`
  for `Failed`/`NeedsInner`/`None` variants. Five stage functions (`structural_stage`,
  `index_reduction_stage`, `initialization_stage`, `events_stage`, `solve_lowering_stage`)
  now call it instead of repeating the same three match arms.

- [x] **TD-6 (low): Move `stage_name()` to `StageKind`.**
  Fixed 2026-07-22. Removed `App::stage_name()` wrapper; all 5 call sites inlined to
  `self.stage.name()`.

- [x] **TD-7 (low): `DefInfo.kind` is stringly typed.**
  Fixed 2026-07-22. New `DefKind` enum (`Class`, `Definition`) replaces
  `kind: &'static str`. Two construction sites in `build_def_index` and one
  comparison site in `tree.rs` updated. `DefKind::as_str()` preserves JSON output.

## Theme 2: Color constants

- [x] **TD-8 (medium): Centralize shared color constants.**
  Fixed 2026-07-22. New `colors.rs` module with `OK_GREEN` constant and
  `ok_color(dark_mode)` helper. Six call sites across 5 files updated to use
  the centralized definitions.

## Theme 3: Silent error handling

- [x] **TD-9 (medium): Replace `serde_json::to_value().unwrap_or_default()` with error reporting.**
  Fixed 2026-07-22. Two helpers: `Stage::from_ser()` (returns `Stage::err` on failure)
  for Stage-producing call sites, `ser_value()` (returns a descriptive error string)
  for nested `json!()` calls. All 14 occurrences replaced.

- [x] **TD-10 (medium): Report editor launch failures to the user.**
  Fixed 2026-07-22. New `open_in_editor()` method replaces the three `let _ =`
  call sites. On failure, sets `bridge_status` to an error message shown in the
  status bar.

- [x] **TD-11 (low): Use `.expect()` instead of `.unwrap()` on structural invariants.**
  Fixed 2026-07-22. Changed to `.expect("structural_to_json returns an object")`
  in `structural_stage` and `index_reduction_stage`.

## Theme 4: Test coverage gaps

- [x] **TD-12 (medium): Add tests for `app.rs` logic functions.**
  Fixed 2026-07-22. Added `test_default()` constructor and tests for:
  `last_successful_stage` (furthest-ok, fallback, skips-errored),
  `previous_stage_value` (Parse=None, Instantiate=Resolve),
  `stage_name` (exhaustive over all StageKind variants).

- [x] **TD-13 (medium): Add tests for `canvas.rs` coordinate transforms.**
  Fixed 2026-07-22. Eight tests: `to_screen`/`to_world` round-trip, origin mapping,
  rect size preservation, rect round-trip, zoom accessor, `to_world` at rect min,
  `Canvas::default` fit flag, and `request_fit`.

- [x] **TD-14 (medium): Add error-path tests for `worker.rs`.**
  Fixed 2026-07-22. Added tests for: compile with nonexistent file,
  compile with invalid syntax, `open_def` without resolved tree,
  `extract_class` with missing name.

- [x] **TD-15 (low): Add tests for `log_view::level_style`.**
  Fixed 2026-07-22. Extracted `level_prefix()` pure function from `level_style()`.
  Four tests: consistent column width, all variants covered, stage-start has play
  symbol (▶), stage-end has checkmark (✓).

## Theme 5: Bridge correctness

- [x] **TD-16 (high): Update stale stage file list in bridge focus JSON.**
  Fixed 2026-07-22. Extracted `STAGE_FILE_NAMES` constant (single source of truth);
  focus JSON references the constant. Added two tests:
  `focus_json_stage_files_match_constant` and `stage_file_names_covers_all_pipeline_stages`
  to prevent this from going stale again.

- [x] **TD-17 (low): Cross-stage diff only covers Parse/Resolve.**
  Fixed 2026-07-22. Changed fallback message from "current stage has no IR" to
  "cross-stage diff not yet implemented for this stage". Added test
  `cross_stage_fallback_message_for_unsupported_stage`.

## Theme 6: Per-frame performance

- [x] **TD-18 (medium): Cache `Path::exists()` result for narrative button.**
  Fixed 2026-07-22. Added `narrative_exists: bool` field to `App`, set once in
  `drain_worker` when `Compiled` arrives, cleared in `open()`. Both frame-hot
  `Path::exists()` calls replaced with the cached bool.

- [x] **TD-19 (low): Cache specimen list width calculation.**
  Fixed 2026-07-22. Added `cached_specimen_width: Option<f32>` field to `App`,
  populated lazily via `get_or_insert_with`, invalidated on `rescan()`.

- [x] **TD-20 (low): Cache parsed expressions in `ReductionView`.**
  Fixed 2026-07-22. `Elimination` now stores a pre-rendered `display: String`
  (computed once in `from_report()` via `abbreviate_expr`) instead of the raw
  JSON `replacement` that was re-parsed every frame.

## Theme 7: Code duplication in custom views

- [x] **TD-21 (low): Extract `cell_rect` into `canvas::View`.**
  Fixed 2026-07-22. New `View::cell_rect(col, row) -> Rect` method; both
  `spyplot.rs` and `incidence_view.rs` closures replaced with `view.cell_rect(...)`.
  Added `cell_rect_is_one_by_one_world_unit` test.

- [x] **TD-22 (low): Extract grid-drawing helper into `canvas`.**
  Fixed 2026-07-22. New `View::draw_grid(painter, n_cols, n_rows, color)` method
  with built-in `zoom >= 6.0` guard. Both views reduced to one-liner calls.
  Added `draw_grid_skips_low_zoom` test.

- [x] **TD-23 (low): Share `str_vec` JSON extraction helper.**
  Fixed 2026-07-22. Promoted `str_vec` to a `pub fn` in `lib.rs` with four tests.
  `spyplot.rs`, `incidence_view.rs`, and `reduction_view.rs` all import
  `crate::str_vec` instead of inlining the pattern.

## Theme 8: Miscellaneous

- [x] **TD-24 (medium): `truncate_label` can panic on multi-byte UTF-8.**
  Fixed 2026-07-22. Changed `&s[..max]` to `s.get(..max).unwrap_or(s)` — safe
  fallback to full string on boundary miss. Added two tests
  (`truncate_label_ascii`, `truncate_label_multibyte_does_not_panic`).

- [x] **TD-25 (medium): Extract `start_simulation()` method.**
  Fixed 2026-07-22. New `start_simulation()` method deduplicates the launch
  sequence from `simulation_pane()` and the play button handler.

- [x] **TD-26 (medium): Split `ui()` method (~757 lines).**
  Fixed 2026-07-22. Extracted `floating_windows()` (~130 lines: Help, About, and
  Settings windows with their deferred-action patterns) into a separate method.
  Combined with TD-4's data-driven tab bar, `ui()` dropped from ~757 to ~620 lines.

- [x] **TD-27 (low): Duplicate "Read: specimen narrative" button code.**
  Fixed 2026-07-22. New `narrative_button()` method replaces duplicate code in
  `right_panel_specimen()` and `right_panel_read_links()`.

- [x] **TD-28 (low): Duplicate `path.to_owned()` calls in `compile()`.**
  Fixed 2026-07-22. Single `let path_owned = path.to_owned()` at the top of
  `compile()`, with `.clone()` for intermediate sends and a final move.

- [x] **TD-29 (low): Duplicate log closure between `compile()` and `simulate()`.**
  Fixed 2026-07-22. New `make_log()` function returns a closure wrapping `emit`
  with elapsed-time tracking. Both methods call `make_log(&t0, emit)`.

- [x] **TD-30 (low): Hardcoded editor command `"code"`.**
  Fixed 2026-07-22. `open_in_editor()` now reads `$EDITOR` and falls back to
  `"code"` when unset. Error message includes the editor name.

- [x] **TD-31 (low): Zoom range `1.0..400.0` hardcoded in two places.**
  Fixed 2026-07-22. Named constants `MIN_ZOOM` and `MAX_ZOOM` in `canvas.rs`;
  both clamp sites reference them.

- [x] **TD-32 (low): `Seg` lacks `Display` impl.**
  Fixed 2026-07-22. Added `impl Display for Seg`; `describe_path` now uses it
  internally. Three tests: `seg_display_key`, `seg_display_index`,
  `describe_path_uses_display`.

---

## Second-pass items (2026-07-22)

A follow-up scan after the first 32 items were resolved. These are refinements
that build on the first-pass infrastructure.

### Theme A: Stringly-typed Ask fields

- [x] **TD-33 (medium): `AskRequest` enum replaces `request: &'static str`.**
  Fixed 2026-07-22. New `AskRequest` enum (`Explain`, `DebugWhereSet`) with
  `as_str()`. `Ask.request` changed from `&'a str` to `AskRequest`. Tests updated.

- [x] **TD-34 (medium): `Ask.stage` uses `Option<StageKind>` instead of `&str`.**
  Fixed 2026-07-22. `None` = navigated library definition (was a magic string).
  `build()` calls `.map_or("(navigated definition)", StageKind::name)`.
  `build_cross_stage()` matches on `Some(StageKind::Parse)` / `Some(StageKind::Resolve)`.

- [x] **TD-35 (low): `base_ask()` helper deduplicates Ask construction.**
  Fixed 2026-07-22. New `App::base_ask()` method populates all shared fields;
  callers only supply `seq`, `request`, and `focus`.

### Theme B: Color centralization (second pass)

- [x] **TD-36 (low): Additional color constants for log_view and custom views.**
  Fixed 2026-07-22. Added `stage_start_color(dark_mode)`, `WARN_AMBER`,
  `INCIDENCE_CELL`, `INCIDENCE_HOVER`, `COUPLED_STROKE`, `coupled_fill()`,
  `GRID_ALPHA` to `colors.rs`. Updated `log_view.rs`, `incidence_view.rs`,
  `spyplot.rs`. Fixed `ok_color()` dark branch (was duplicating RGB instead
  of returning `OK_GREEN`).

### Theme C: Per-frame re-parsing

- [x] **TD-37 (medium): Cache `from_report` views in App.**
  Fixed 2026-07-22. Three `Option<Option<T>>` fields (`cached_spy_plot`,
  `cached_incidence`, `cached_reduction`) cache parsed views. Outer Option =
  cache state (None = stale), inner = parse result (None = no data in report).
  Invalidated on `Compiled`. Rendering uses `get_or_insert_with`.

### Theme D: Worker boilerplate (second pass)

- [x] **TD-38 (low): `unwrap_success()` helper replaces 5 duplicated match arms.**
  Fixed 2026-07-22. New `unwrap_success(result)` function extracts
  `&CompilationResult` from `PhaseResult::Success`, replacing inline matches
  in five stage functions.

- [x] **TD-39 (low): Unified `run_step!` macro with bail-out.**
  Fixed 2026-07-22. Merged `run_step!` and `run_step_unit!` into a single macro
  that handles both `Ok(v)` and error bail-out (sets `stopped_at`, returns early).

- [x] **TD-40 (low): `StageKind::ALL` const array.**
  Fixed 2026-07-22. Lists all 11 variants in order. Used by
  `stage_file_names_covers_all_pipeline_stages` test (replaced hardcoded `10`).

### Theme E: Miscellaneous (second pass)

- [x] **TD-41 (low): Misplaced doc comment on `narrative_button`.**
  Fixed 2026-07-22. Moved the simulation-pane doc comment from `narrative_button`
  to `simulation_pane`.

- [x] **TD-42 (low): Silent `write_stages` error.**
  Fixed 2026-07-22. Changed `let _ = bridge::write_stages(...)` to report errors
  to `bridge_status`.

- [x] **TD-43 (low): `APP_NAME` constant in `main.rs`.**
  Fixed 2026-07-22. Replaced two inline `"HRW Observatory"` strings with
  `const APP_NAME`.

- [x] **TD-44 (low): `GOLDEN_RATIO` constant in `app.rs`.**
  Fixed 2026-07-22. Named constant replaces inline `0.618_033_99` magic number.

- [x] **TD-45 (low): Canvas constants `FIT_MARGIN`, `SCROLL_ZOOM_SENSITIVITY`.**
  Fixed 2026-07-22. Named constants replace inline `0.92` and `0.002` in
  `Canvas::show()`.

- [x] **TD-46 (low): `RANGE_FRACTION`, `MEDIAN_MULTIPLIER` in worker.rs.**
  Fixed 2026-07-22. Named constants for discontinuity-detection thresholds.

- [x] **TD-47 (low): `DIFF_ROW_MARKER` in worker.rs.**
  Fixed 2026-07-22. Named constant for `"index_reduction:d_dt_for_"` prefix
  used by differentiated-row detection.

- [x] **TD-48 (low): `hovered_cell()` in `canvas::View`.**
  Fixed 2026-07-22. Shared hover → cell-index logic replaces duplicate code
  in `spyplot.rs` and `incidence_view.rs`.

---

## Summary

### First pass (32 items)

| Severity | Count | Done | Key themes |
|----------|-------|------|------------|
| High     | 2     | 2 ✓  | ~~TD-1 (stage bundle on App)~~, ~~TD-16 (stale bridge file list)~~ |
| Medium   | 13    | 13 ✓ | ~~Stage boilerplate~~, ~~colors~~, ~~error handling~~, ~~tests~~, ~~per-frame I/O~~, ~~ui() size~~ |
| Low      | 17    | 17 ✓ | ~~Duplication~~, ~~naming~~, ~~caching~~, ~~ergonomics~~ |
| **Total**| **32**| **32 ✓**|  |

### Second pass (16 items)

| Severity | Count | Done | Key themes |
|----------|-------|------|------------|
| Medium   | 3     | 3 ✓  | ~~AskRequest enum~~, ~~Ask.stage type~~, ~~from_report caching~~ |
| Low      | 13    | 13 ✓ | ~~Color centralization~~, ~~worker helpers~~, ~~named constants~~, ~~hovered_cell~~ |
| **Total**| **16**| **16 ✓**|  |
