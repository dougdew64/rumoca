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

- [ ] **TD-5 (low): Extract common fallback arms from stage-extraction functions.**
  Five stage functions (`structural_stage`, `index_reduction_stage`, etc.) copy
  identical `Failed`/`NeedsInner`/`None` match arms. A helper like
  `fn not_reached_stage(result: Option<&PhaseResult>) -> Option<Stage>` would
  reduce 15 duplicated lines to 1 call per function.

- [ ] **TD-6 (low): Move `stage_name()` to `StageKind`.**
  `App::stage_name()` reads only `self.stage` — it should be
  `impl StageKind { fn name(self) -> &'static str }` (or `impl Display`).

- [ ] **TD-7 (low): `DefInfo.kind` is stringly typed.**
  `kind: &'static str` takes values `"class"` or `"definition"`. A `DefKind` enum
  would make the contract explicit and prevent typos.

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

- [ ] **TD-11 (low): Use `.expect()` instead of `.unwrap()` on structural invariants.**
  `json.as_object_mut().unwrap()` in `structural_stage` and `index_reduction_stage`
  assumes JSON is always an Object. If that contract ever breaks, the panic message
  gives no context. `.expect("structural_to_json returns an object")` costs nothing
  and aids debugging.

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

- [ ] **TD-15 (low): Add tests for `log_view::level_style`.**
  Pure function mapping `LogLevel` → color/prefix — trivially testable.

## Theme 5: Bridge correctness

- [x] **TD-16 (high): Update stale stage file list in bridge focus JSON.**
  Fixed 2026-07-22. Extracted `STAGE_FILE_NAMES` constant (single source of truth);
  focus JSON references the constant. Added two tests:
  `focus_json_stage_files_match_constant` and `stage_file_names_covers_all_pipeline_stages`
  to prevent this from going stale again.

- [ ] **TD-17 (low): Cross-stage diff only covers Parse/Resolve.**
  The diff logic hardcodes only `"Parse"` and `"Resolve"`. Later stages get
  `applicable: false` with a misleading reason message. Not a bug (the diff was
  designed for the Parse→Resolve transition), but the fallback message could say
  "cross-stage diff not yet implemented for this stage" instead of "current stage
  has no IR."

## Theme 6: Per-frame performance

- [x] **TD-18 (medium): Cache `Path::exists()` result for narrative button.**
  Fixed 2026-07-22. Added `narrative_exists: bool` field to `App`, set once in
  `drain_worker` when `Compiled` arrives, cleared in `open()`. Both frame-hot
  `Path::exists()` calls replaced with the cached bool.

- [ ] **TD-19 (low): Cache specimen list width calculation.**
  `layout_no_wrap()` on the longest filename runs every frame to auto-size the
  left panel. The result only changes on rescan or zoom change.

- [ ] **TD-20 (low): Cache parsed expressions in `ReductionView`.**
  `abbreviate_expr` re-parses JSON strings every frame. The parsed results could
  be cached at construction time in `from_report()`.

## Theme 7: Code duplication in custom views

- [ ] **TD-21 (low): Extract `cell_rect` into `canvas::View`.**
  Both `spyplot.rs` and `incidence_view.rs` define an identical `cell_rect`
  closure mapping `(col, row)` to a screen rect. A method on `View` would
  eliminate both and serve future custom views.

- [ ] **TD-22 (low): Extract grid-drawing helper into `canvas`.**
  Both views draw grid lines at `zoom >= 6.0` with the same pattern. A
  `View::draw_grid(painter, n_cols, n_rows, color)` method would reduce both
  to one-liners.

- [ ] **TD-23 (low): Share `str_vec` JSON extraction helper.**
  The pattern `.get("field")?.as_array()?.iter().filter_map(as_str).collect()`
  appears inline in `spyplot.rs`, `incidence_view.rs`, and `reduction_view.rs`.
  `spyplot.rs` has a local `str_vec` helper — promote it to a shared utility.

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

- [ ] **TD-27 (low): Duplicate "Read: specimen narrative" button code.**
  The narrative button (check exists, build path, spawn editor) appears in both
  `right_panel_specimen()` and `right_panel_read_links()`. Extract to a shared
  helper.

- [ ] **TD-28 (low): Duplicate `path.to_owned()` calls in `compile()`.**
  The same `path.to_owned()` allocation is repeated 9 times. A single
  `let path_buf = path.to_owned()` at the top with `.clone()` for intermediate
  sends would be clearer.

- [ ] **TD-29 (low): Duplicate log closure between `compile()` and `simulate()`.**
  Both define the same closure pattern for creating `LogEntry`. A private helper
  or small `Logger` struct would eliminate it.

- [ ] **TD-30 (low): Hardcoded editor command `"code"`.**
  No `$EDITOR` fallback. Acceptable for a personal tool, but a rough edge.

- [ ] **TD-31 (low): Zoom range `1.0..400.0` hardcoded in two places.**
  The fit calculation and the scroll-zoom handler in `canvas.rs` both hardcode
  the range. Named constants would keep them in sync.

- [ ] **TD-32 (low): `Seg` lacks `Display` impl.**
  `bridge.rs` has a `describe_path` free function instead. `impl Display for Seg`
  would improve ergonomics.

---

## Summary

| Severity | Count | Done | Key themes |
|----------|-------|------|------------|
| High     | 2     | 2 ✓  | ~~TD-1 (stage bundle on App)~~, ~~TD-16 (stale bridge file list)~~ |
| Medium   | 13    | 13 ✓ | ~~Stage boilerplate~~, ~~colors~~, ~~error handling~~, ~~tests~~, ~~per-frame I/O~~, ~~ui() size~~ |
| Low      | 17    | 0    | Minor duplication, naming, caching, ergonomics |
| **Total**| **32**| **15**|  |
