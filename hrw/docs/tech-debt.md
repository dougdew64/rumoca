# Tech Debt — HRW Observatory

Weekly quality improvements identified by code review. Items are grouped by
theme, ordered by severity within each group. Check off items as they are
completed; clear completed items at the end of each cycle.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 in the
2026-07-25 cycle, 25 in the 2026-07-25 sweep, and 20 in the 2026-07-26 sweep.
See git history for details.

## 2026-07-29 scoped sweep

Run *before* #42 (ad hoc tours) rather than as general hygiene, and deliberately
**scoped by what #42 will touch**. Three items were separated out as *not*
sweep material — see "Deferred into #42" below — because unifying them without
#42's requirements in hand would be designing the abstraction before knowing
what it must express, which is the mistake that made `end_to_end_tour.md`
worthless.

**Fixed this sweep:**

- **`ui()` reduced from 982 lines to 325**, closing an item open across two
  sweeps. The fix was identified last sweep and *blocked*: extraction needed
  seven transposable out-parameters threaded through, because egui panel
  closures borrow `self`, so a click records intent and `ui()` acts after the
  borrows end. `FrameIntent` bundles them (the same move `tree::TreeActions`
  already made for the tree), and `central_panel_ui` then moved out wholesale
  taking `&mut self` plus one `&mut FrameIntent`.
- **Three dead locals removed.** `node_ask`, `debug_ask` and `nav_to` were
  declared `None`, never assigned inside the panels, then folded in with
  `.or(tree_actions.x)` — so the fold was always just `tree_actions.x`. Vestiges
  of a design where other views would write into them. The tree is the only
  producer, and a comment now says so.
- **Batch narrative regeneration — closed as obsolete, not done.** The item
  asked for a workflow to review 14 `narrative.md` files after a Rumoca rebase.
  The narratives are being retired (see #42's notebook conversion), so the debt
  evaporated rather than got paid. `trace/` stays and is generated, so it needs
  no review workflow.
- **The test-race entry was wrong about scope.** It blamed `bridge.rs`'s test
  module. Verified 2026-07-29 that `worker::tests::output_capture_handles_large_write_without_deadlock`
  races too — it redirects *process-global* stdout, so any concurrently-running
  test that writes steals bytes — and that both failures reproduce **on a clean
  tree**. `--test-threads=1` is required for the whole suite, not just bridge
  tests. Entry corrected below.

**Re-measured, not re-estimated:**

| | logged 2026-07-28 | measured 2026-07-29 |
|---|---|---|
| `ui()` | 880 | **325** |
| `central_panel_ui()` | — | **664** *(new — extracted from `ui`)* |
| `source_map_ui()` | 245 | 245 |
| `generic_error_summary()` | 201 | 201 |
| `compile()` | 327 | **363** |
| `app.rs` | ~5900 | **6375** |
| `bridge.rs` | 2342 | **2365** |

`compile()` and `app.rs` grew with the four new animations (five view methods,
two new sub-view enums, three cache fields, `record_connection_frames` and its
plumbing). That growth is logged rather than swept: `app.rs`'s next reduction is
`central_panel_ui`, and #42 will touch it.

**Deferred into #42, as its opening moves rather than as debt:**

- **The four dissimilar sub-view enums** (`StructuralView` — shared by two
  stages — plus `EventsView`, `FlattenView`, `InitView`). Unifying them looks
  like cleanup but *is* the first design decision of the `hrw://` link
  vocabulary.
- **`bridge.rs` at 2365 lines.** #42 needs links to reach parity with
  `focus.json`, which is this file's job. Decomposing before knowing what the
  links must express is guessing.
- **`Canvas` cannot aim at a node.** A missing capability, not debt. See #42.

---

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

- [x] **`ui()` is 880 lines.** *(was 1272 before this sweep)* — **fixed
  2026-07-29: now 325.** See the 2026-07-29 sweep above; the `FrameIntent`
  bundling suggested here is exactly what unblocked it.
  Still the largest function in the codebase and still growing with each
  feature. The remaining bulk is the central panel's stage dispatch. Further
  extraction is blocked on one thing: `ui` collects intent into ~8 locals
  (`tree_actions`, `canvas_capture`, `want_stage_ask`, `go_back`, `go_home`,
  `expand_trackable`, …) and acts on them after the panel closures end, so any
  extracted block needs them threaded through. **Suggested fix:** bundle them
  into a `FrameIntent` struct, exactly as `TreeActions` bundled the tree's
  out-parameters; then the stage dispatch can move out wholesale.
  *File:* `app.rs`.

- [ ] **`compile()` is 363 lines with an inlined `macro_rules!`.**
  *(measured 2026-07-29; 327 at Phase 5 close, ~285 before that — it grows every
  time a stage gains an artifact, most recently `record_connection_frames`.)*
  Not swept 2026-07-29: nothing upcoming forces it, and the growth is one line
  per new artifact rather than structural rot.
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

- [x] **No batch narrative regeneration.** ~~After a Rumoca rebase or trace
  regeneration, all 14 narratives may need review with no script or checklist
  driving it.~~ **Closed 2026-07-29 as obsolete** — the narratives are being
  retired (#42's notebook conversion). `trace/` is generated and needs no review
  workflow. Debt that evaporated rather than got paid.

- [ ] **`central_panel_ui()` is 664 lines.** *(new 2026-07-29, extracted from
  `ui()`.)* Now a coherent unit — the stage tab bar, status banners, sub-tab
  bars and per-stage dispatch — rather than a tangle, and it takes only
  `&mut FrameIntent`, so further extraction is no longer blocked. The natural
  next cut is per-stage: the sub-tab bars are already four near-parallel blocks,
  and #42 will rework exactly those. **Do not split before #42** — that is the
  deferral recorded in the 2026-07-29 sweep.
  *File:* `app.rs`.

## Robustness

- [ ] **Test races under parallel execution — two causes, not one.**
  *(Corrected 2026-07-29: the entry previously blamed only `bridge.rs`.)*
  1. `bridge.rs` tests share `.hrw-bridge/focus.json` — requires parameterizing
     the bridge dir to fix.
  2. `worker::tests::output_capture_handles_large_write_without_deadlock`
     redirects **process-global stdout**, so any concurrently-running test that
     writes to stdout steals bytes. Inherently exclusive; would need a serial
     guard or a non-global capture mechanism.

  Both reproduce **on a clean tree** — verified by stashing. Mitigated by
  `--test-threads=1`, now required for the *whole* suite and documented in
  `README.md` and `hrw/CLAUDE.md`. Worth fixing before any CI, since the default
  harness both fails *and* hangs.
  *Files:* `bridge.rs`, `worker.rs` — test modules.

- [ ] **`build_declaring_classes` resolves only the first path segment.**
  `src.V` resolves exactly; `gear.flange_a.tau` yields `gear`'s type, which
  *contains* the declaration rather than being it. The UI wording says "in"
  rather than "declared in", so it is honest — but a deeper resolution would
  need to walk into library class IRs that are loaded on demand.
  *File:* `app.rs`.
