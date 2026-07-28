# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 in the
2026-07-25 cycle, 25 in the 2026-07-25 sweep, and 20 in the 2026-07-26 sweep.
See git history for details.

## 2026-07-28 comprehensive sweep

Run before Phases 5–7 of `source-tooling-plan.md`, on the principle that this
code is a learning resource and that future work is easier on code that is
understandable and correct.

**Fixed this sweep:**

- **`collect_tracked_ancestors` short-circuited.** It used `any`, so the walk
  stopped at the first child whose subtree contained the tracked identifier —
  later siblings were never visited and the tree opened the path to the *first*
  mention only. Now folds. *(Was logged 2026-07-27; fixed.)*
- **`ui()` reduced from 1272 lines to 880** by extracting `menu_bar_ui`,
  `specimen_source_ui`, `tracking_bar_ui`, `matching_anim_ui`,
  `tarjan_anim_ui`, and `reduction_anim_ui`. Note the previous entry recorded
  it at ~887 — it had grown by 385 lines in a day, unnoticed, which is the
  argument for measuring rather than estimating.
- **The specimen source view had a private copy of the tracking toggle.** It now
  calls `set_tracked_identifier` like every other entry point.
- **`tree.rs` module documentation was stale** — it described an `ask` output
  parameter that no longer exists and a context menu missing two of its items.
  Refreshed, with a note that Phase 5 renames the user-facing verbs while the
  code's nouns stay (they are also the wire format).
- **Retired:** the two stale items from the 2026-07-27 reconciliation
  (WSL2 LLDB deadlock; unverified `output_capture` test) stay retired.

---

## Code quality / duplication

- [ ] **`ui()` is 880 lines.** *(was 1272 before this sweep)*
  Still the largest function in the codebase and still growing with each
  feature. The remaining bulk is the central panel's stage dispatch. Further
  extraction is blocked on one thing: `ui` collects intent into ~8 locals
  (`tree_actions`, `canvas_capture`, `want_stage_ask`, `go_back`, `go_home`,
  `expand_trackable`, …) and acts on them after the panel closures end, so any
  extracted block needs them threaded through. **Suggested fix:** bundle them
  into a `FrameIntent` struct, exactly as `TreeActions` bundled the tree's
  out-parameters; then the stage dispatch can move out wholesale.
  *File:* `app.rs`.

- [ ] **`compile()` is 380 lines with an inlined `macro_rules!`.**
  *(was ~285 when last logged — it has grown too.)*
  *File:* `worker.rs`.

- [ ] **The three animation views are near-identical.**
  `matching_anim_ui` / `tarjan_anim_ui` / `reduction_anim_ui` (extracted this
  sweep, ~57/57/46 lines) repeat the same six-step live-debug sequence,
  differing only in the `PendingLiveDebug` variant and the cached-animation
  field. Their `ui()` methods in `matching_anim.rs` / `tarjan_anim.rs` /
  `reduction_anim.rs` likewise share ~40 lines, and `live_state(&self, arming)`
  is byte-identical in all three. **Deliberately not fixed this sweep:** the
  real fix is a trait over the three animation types, and Phase 7 reworks these
  views anyway — doing it now risks the churn Phase 7 was postponed to avoid.
  Extracting them into adjacent named functions was the safe half, and it makes
  the duplication visible.
  *Files:* `app.rs`, `matching_anim.rs`, `tarjan_anim.rs`, `reduction_anim.rs`.

- [ ] **`animation_controls` takes 8 positional parameters**, two of them
  adjacent bools, so transposing arguments compiles silently. Grouping
  cursor/playing/elapsed/interval into an `AnimationPlayback` struct would make
  misuse a type error. Same pattern already applied successfully to
  `TreeActions` and `TreeOptions`.
  *File:* `lib.rs`.

- [ ] **`source_map_ui()` is 247 lines.** *File:* `app.rs`.

- [ ] **`generic_error_summary()` is 217 lines.** Dispatches on 6 error kinds
  with inline UI; each branch could be a helper. *File:* `app.rs`.

- [ ] **Long view functions.** `matching_anim::draw_matrix` (191),
  `incidence_view::ui` (190), `expr_format::format_expr_into` (177),
  `equation_sheet::match_connection_to_source` (162), `app::equation_sheet_ui`
  (157), `spyplot::ui` (136), `worker::to_json` (135).

- [ ] **Duplicated matrix canvas boilerplate.** Three views repeat the same
  ~10-line pattern. *Files:* `spyplot.rs`, `incidence_view.rs`,
  `matching_anim.rs`. **Overlaps Phase 7** — fix them together.

## Known-temporary code

- [ ] **`Expansion::force_open` exists only because "Reveal identifiers" is a
  mode.** Phase 6 is expected to make revealing an *action*, at which point
  forcing headers open every frame becomes unnecessary and the struct collapses
  back to one set. Remove it when that lands.
  *File:* `tree.rs`.

- [ ] **`lldb.verboseLogging` is off but retained** in `.vscode/settings.json`
  with a comment explaining when to switch it on. Keep — it documents a
  diagnostic that was hard to find.

## Build process / specimen notebook

- [ ] **No batch narrative regeneration.** After a Rumoca rebase or trace
  regeneration, all 14 narratives may need review with no script or checklist
  driving it. Add a batch workflow, or at minimum a checklist in
  `docs/updating-rumoca.md`.

## Robustness

- [ ] **Bridge test filesystem races under parallel execution.**
  *(deferred — requires parameterizing the bridge dir; mitigated by
  `--test-threads=1`, which `README.md` documents as required.)*
  *File:* `bridge.rs` — test module.

- [ ] **`build_declaring_classes` resolves only the first path segment.**
  `src.V` resolves exactly; `gear.flange_a.tau` yields `gear`'s type, which
  *contains* the declaration rather than being it. The UI wording says "in"
  rather than "declared in", so it is honest — but a deeper resolution would
  need to walk into library class IRs that are loaded on demand.
  *File:* `app.rs`.
