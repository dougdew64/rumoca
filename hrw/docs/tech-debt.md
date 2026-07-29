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

## Measured 2026-07-28, at Phase 5 close

Re-measured rather than re-estimated, per the sweep discipline. Drift across the
whole of Phase 5 was negligible — two functions *shrank* — so **no general sweep
is due**; the animation item above was rescheduled on sequencing grounds, not
because the code rotted.

| | logged | measured |
|---|---|---|
| `ui()` | 880 | 894 |
| `source_map_ui()` | 247 | 245 |
| `generic_error_summary()` | 217 | 201 |
| `compile()` | 380 | 327 |

- [ ] **`bridge.rs` is 2342 lines**, up substantially during Phase 5 (mention
  neighbourhoods, sibling windows, `View`, `phase_source`, `Focus::Nothing`).
  Scheduled after idea #40 and before Phase 6 — both remaining phases touch it.
  *File:* `bridge.rs`.

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

- [x] **The three animation views are near-identical.** *(fixed 2026-07-29)*
  Resolved as `playback::Playback<T>` — a generic struct holding the seven
  fields all three declared (`frames`, `cursor`, `playing`, `interval`,
  `elapsed`, `live_rx`, `live_done`) plus the five byte-identical methods on
  them. **A generic struct rather than a trait**: a trait would have shared the
  behaviour and left the state declared three times, so it could still drift.
  `playback::Animated` is the small trait on top, for the one thing that cannot
  be generic — what the current frame *means*.
  Rolled up with the two items that were the same refactor from other angles:
  `animation_controls` went from **8 positional parameters to 4** (the four
  transposable `&mut`s are now `PlaybackControls`), and `app.rs` lost two more
  copies of "which animation is on screen?" to `on_screen_animation()`.
  Net: `matching_anim` −68, `tarjan_anim` −55; `playback.rs` is 324 lines of
  which roughly half is tests and rationale. Line count is up, *duplication* is
  down from three copies to one — which was the point, since idea #40 adds a
  fourth view.
  **Folded in at the same time:** `Animated::current_frame_context`, so the
  capture's `view.animation` carries *what* the user is looking at and not only
  *where* they are. See `DECISIONS.md` (2026-07-29).

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
