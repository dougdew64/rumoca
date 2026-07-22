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

- [ ] **TD-3 (medium): Factor the repetitive stage-emit loop in `compile()`.**
  Six consecutive stages (Flatten through Solve lowering) repeat the same 7-line
  pattern: log start, time, extract, drain traces, log end, clone bundle, emit
  progress. A helper or macro taking a stage name and extraction function would
  halve the line count.

- [ ] **TD-4 (medium): Data-drive the stage tab bar.**
  Each of the 10 stage tabs repeats ~6–10 lines of near-identical `selectable_label`
  code. A loop over `[(StageKind, &str, &Stage, Option<&str>)]` would reduce ~90
  lines to ~15 and make adding a tab a one-line change.

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

- [ ] **TD-8 (medium): Centralize shared color constants.**
  The "success green" `(0x3F, 0xB9, 0x50)` appears in 6 locations across 5 files.
  The dark/light mode branching (`if dark_mode { dark_green } else { light_green }`)
  is repeated in 4 places across 3 files. Other colors (coupled-block orange,
  incidence blue, changed-value green) are also inline. A shared `colors` module
  with named constants and a `fn ok_color(dark_mode: bool) -> Color32` helper would
  make the palette a single point of change.

## Theme 3: Silent error handling

- [ ] **TD-9 (medium): Replace `serde_json::to_value().unwrap_or_default()` with error reporting.**
  14 occurrences in `worker.rs` silently produce `Value::Null` on serialization
  failure, which the UI shows as a blank tree with no explanation. A helper like
  `fn ser_or_err<T: Serialize>(v: &T) -> Stage` that returns `Stage::err(...)` on
  failure would handle this consistently. Compare with `flatten_stage` which already
  does this correctly.

- [ ] **TD-10 (medium): Report editor launch failures to the user.**
  `Command::new("code")` results are discarded with `let _ =` in three places in
  `app.rs`. If VS Code isn't installed or not on PATH, the user clicks a button and
  nothing happens with no feedback. At minimum, set `bridge_status` to an error.

- [ ] **TD-11 (low): Use `.expect()` instead of `.unwrap()` on structural invariants.**
  `json.as_object_mut().unwrap()` in `structural_stage` and `index_reduction_stage`
  assumes JSON is always an Object. If that contract ever breaks, the panic message
  gives no context. `.expect("structural_to_json returns an object")` costs nothing
  and aids debugging.

## Theme 4: Test coverage gaps

- [ ] **TD-12 (medium): Add tests for `app.rs` logic functions.**
  Only `read_purpose` and `field_name_from_path` are tested. Missing:
  `last_successful_stage()`, `previous_stage_value()`, `open()` state reset
  completeness, `drain_worker()` stale-result filtering, `stage_name()`
  exhaustiveness.

- [ ] **TD-13 (medium): Add tests for `canvas.rs` coordinate transforms.**
  Zero tests. `to_screen`, `to_world`, `to_screen_rect`, and fit-to-content are
  pure math — trivially testable with round-trip assertions like
  `to_screen(to_world(p)) ≈ p`.

- [ ] **TD-14 (medium): Add error-path tests for `worker.rs`.**
  Happy paths are well covered but failure paths are untested: invalid library
  paths, `open_def` with no resolved tree, `simulate` when compilation fails,
  `compile` when `read_to_string` fails. These are where regressions hide.

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

- [ ] **TD-18 (medium): Cache `Path::exists()` result for narrative button.**
  `right_panel_specimen()` and `right_panel_read_links()` call
  `Path::new(&abs).exists()` every frame to decide whether to show the narrative
  button. This is a syscall at 60fps. Cache a `narrative_exists: bool` on `App`,
  invalidated on specimen change.

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

- [ ] **TD-24 (medium): `truncate_label` can panic on multi-byte UTF-8.**
  `incidence_view.rs` slices `&s[..max]` by byte index. If `max` falls inside a
  multi-byte character, this panics. Fix: `s.get(..max).unwrap_or(s)` or use
  `char_indices()`. Modelica identifiers are ASCII in practice, but the function
  accepts any `&str`.

- [ ] **TD-25 (medium): Extract `start_simulation()` method.**
  The simulation launch sequence (set flags, clear data, clone path/model, send
  `ToWorker::Simulate`) is duplicated between `simulation_pane()` and the play
  button handler in `ui()`.

- [ ] **TD-26 (medium): Split `ui()` method (~757 lines).**
  The main frame function handles menu bar, floating windows, status bar, specimen
  list, right panel, tab bar, 4 view modes, navigation, and deferred actions.
  Extractable blocks: tab bar (~120 lines), structural view routing (~75 lines),
  specimen list (~90 lines), Settings window (~50 lines).

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
| Medium   | 13    | 0    | Stage boilerplate, colors, error handling, tests, per-frame I/O, ui() size |
| Low      | 17    | 0    | Minor duplication, naming, caching, ergonomics |
| **Total**| **32**| **2**|  |
