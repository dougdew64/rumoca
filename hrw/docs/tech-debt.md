# Tech Debt — HRW Observatory

Quality improvements identified by code review. Items are grouped by theme,
ordered by severity within each group. Check off items as they are completed;
clear completed items at the end of each cycle.

**Trigger: each phase boundary, scoped to what the next phase touches** — adopted
2026-07-29, replacing a weekly calendar scan. Doug, on the change: *"my original
tech debt schedule is a prime example of old fashioned software development
mentality where teams have to agree upon weekly schedules and other such stuff
which absolutely do not matter for this project. We are entirely agile."*

The rules, in full:

- **Measure, never re-estimate.** The 2026-07-28 sweep found `ui()` had grown 385
  lines in a day, unnoticed; the 2026-07-29 sweep found `compile()` at 363 where
  327 was logged.
- **Three outcomes are all legitimate:** *fixed*; *closed as obsolete* (the batch
  narrative workflow — the narratives it served were retired); or *deferred into the
  phase that will rewrite it*, when sweeping would mean designing an abstraction
  before its requirements exist.
- **Skip what the next phase will rewrite.** Applied three times on 2026-07-29.
- **Start from the tour-holes table above.** It is the only section whose items arrive
  with evidence that they blocked a real answer, which makes it the only section whose
  priority is not a judgement call.

Previous cycles: 48 items fixed across two passes (2026-07-22), plus 22 in the
2026-07-25 cycle, 25 in the 2026-07-25 sweep, and 20 in the 2026-07-26 sweep.
See git history for details.

### A second trigger: code that costs mistakes rather than code the next phase needs

Added 2026-08-01, from Doug:

> We did not add to our tech debt log an item for asking ourselves whether our current code is
> causing too many mistakes or is reducing our feature velocity.

The existing trigger looks **forward** — what does the next phase touch? This one looks
**backward** — what has actually been costing us? They are complementary, and the second was
missing.

#### Ask "who caught it?", not "did it feel hard?"

**"Too many mistakes" is not measurable and would become a vibe.** The observable version is:

> **Did the toolchain catch this defect, or did a human?**

If `cargo`, `clippy` or a test caught it, the environment is doing its job and there is nothing
to sweep. If **Doug** caught it — by noticing a missing line, a frozen number, a claim that did
not match the data — then the code lives somewhere nothing checks, and *that* is the debt.

Evidence from 2026-08-01, which is why this trigger exists. Every silent defect that day was
at or inside a **language boundary**: an array argument collapsed by `powershell -File`, a
formfeed injected into a path by a Python escape, an `eprintln!` swallowed by HRW's own
fd-level `OutputCapture`, a rate limiter that gated its own first fire, an announcement that
stayed silent when models were pending by absence. **None was in `fidelity.rs`**, which is
Rust, tested and clippy-clean.

#### The property is verifiability, not Rust

Rust is the proxy, not the point — a Rust program with no tests is no better than a shell
script. What actually differs:

| | Verifying environment | Not |
|---|---|---|
| type errors | compiler | nothing |
| logic errors | the test suite | nothing |
| silent no-ops | non-vacuity guards | nothing |
| who verifies | **the toolchain, in seconds, unattended** | **a human, watching output** |

So the sweep question is *"can the toolchain check this?"* — and **converting to Rust is only
one of the available answers.** Often cheaper and sufficient: add a test, add a non-vacuity
guard, or make the thing fail loudly instead of silently.

#### When it should fire — all three, not any one

1. The code is **re-run repeatedly** (a runbook step, not a one-off).
2. It can **fail silently** — producing nothing looks like having nothing to report.
3. It has **already produced a defect only a human caught**.

One or two of these is not enough. A five-line shell command that runs once and fails loudly is
fine forever.

#### The counter-argument, which is real

**Scripts are editable without a rebuild.** On 2026-08-01 that mattered: the fidelity binary
was locked by a running sweep, and fixing the watchdog in PowerShell required no rebuild and
did not disturb the run. Converting everything to Rust would have cost that.

So the honest form is *"move it where the toolchain can check it, unless being editable
mid-run is why it exists"* — and say which, rather than converting on principle.

#### Standing candidate

`measure-fidelity.ps1` and `promote-run.ps1` meet all three conditions. **Not urgent**, and
behind fidelity, the oracle test and Test mode — but on the list, with a day of evidence
rather than taste behind it. The memory-sampling part needs a crate, and adding a dependency
needs Doug's approval.

## Priority order — read this before choosing what to fix

Set 2026-07-29, when Doug named the real operating constraint: **his robotics
education has deadlines.**

> Try to imagine me as a Purdue robotics student who is under time pressure to
> complete an assignment, is having a difficulty understanding a concept, needs to get
> an answer from you, but cannot get that answer because we procrastinated
> implementing a bug fix that would require hours to implement. In short, my robotics
> education is not merely going to be for entertainment, on a leisurely schedule of my
> choosing.

**So "high priority" is not enough — fixes here are PRE-EMPTIVE.** High priority means
"first in the next sweep", which still leaves the gap open when the deadline lands, and
the hours a fix needs are hours Doug will not have. **Fix while there is slack.**

Feature *experimentation* stays cheap. **Unavailability does not.** Building the wrong
feature costs some tokens; a missing capability the night before an assignment costs
Doug the assignment.

1. **Anything that forces Claude to guess instead of verify** — a phase not emitting
   its data, a broken bridge, a claim that cannot be checked. **This is the
   catastrophic case, not a missing tour.** On 2026-07-29 the textbook shape of a
   hidden constraint says Pantelides *differentiates* it; the actual report showed
   `differentiated_rows` empty and `emf.phi` demoted via the dummy-derivative path.
   Recall would have been confidently wrong, and under deadline pressure Doug would
   have had no reason to doubt it.
2. **Anything that makes HRW unavailable** — crashes, hangs, failure to build. Note
   the test suite *hangs* under the default harness; that class of failure bites worst
   at the worst time.
3. **Tour holes** — the table below. These usually *degrade* an answer rather than
   block it, since a text answer remains available.
4. **Ordinary debt** — everything further down this file.

---

## Tour holes

**A tour hole is a place where HRW stopped Claude from answering a question.** Doug's
ruling, 2026-07-29:

> When attempting to deliver to me the thing which I value most (answers), I want very
> much for you to have available all of the HRW functionality which you need. Fixing
> those gaps and bugs is high priority.

**These outrank all ordinary debt in this file** (but see the priority order above —
anything that makes Claude *guess*, or makes HRW *unavailable*, comes first), including
items that have been open across several sweeps. Ordinary debt costs *future* effort; a tour hole degrades the
*deliverable*, and it arrives with evidence attached — a real question it got in the
way of. **Every sweep starts here.**

Two kinds, and both count:

- **Loud holes** — Claude cannot get there at all, and has to say so mid-tour. These
  get noticed because they are embarrassing.
- **Quiet holes** — Claude works around it with prose ("same tab → now click X") and
  the tour is a little worse at several points. **These are the dangerous ones**: they
  accumulate unnoticed, and the first tour produced one that went unlogged until Doug
  asked whether holes were being tracked.

| Hole | First hit | Evidence | Tracked as | Status |
|---|---|---|---|---|
| `Matching ▶` hidden when Structural is singular — the one view that shows *why* a rank deficiency exists is unavailable exactly when it matters | 2026-07-29, "what does a rank deficiency of 1 mean?" | Tour Stop 3 was an admission rather than a stop | [ideas #44](ideas.md) | ✅ **fixed 2026-07-29** |
| `hrw://` cannot address a **sub-tab** — only stage tabs. Every animation and every custom view lives one level below what a link can reach | 2026-07-29, same tour | 4 navigation moments degraded to "same tab → click **Incidence** / **Reduction ▶** / **Aliases ▶** / **Matching ▶** yourself" | [ideas #42](ideas.md) gap 2 | ✅ **fixed 2026-07-29** |
| `hrw://` cannot point at a **source line**, so a tour must *quote* one instead | 2026-07-29, both tours | "reported at line 9, `connect(src.n, gnd.p);`" and "lines 7–8" — quoted because nothing could point | [ideas #45](ideas.md) | ✅ **fixed 2026-07-29** |
| `Canvas` cannot centre on a node — a stop cannot make Doug *look at* node 25 | *predicted, not yet hit* | Would bite if a stop needed a specific node; see the `should_refit` fragility | [ideas #42](ideas.md) gap 3 | open (unconfirmed) |

**Under-logged twice, and the pattern is the lesson.** The source-line row above was
added only after Doug asked whether it was time to build highlighting — two tours had
already worked around it in prose and Claude had logged neither. That is the *second*
quiet hole missed this way (sub-tab links was the first). **Loud holes get logged
because they are embarrassing; quiet ones get absorbed.** When writing a tour, treat
every "click this yourself" and every quoted-instead-of-linked reference as a row here,
mechanically, without judging whether it feels worth mentioning.

**Closed 2026-07-29, both pre-emptively** — no question was waiting on either, which is
the point: Doug's deadlines are real, and the cheapest moment to fix a hole is while
nothing is blocked by it.

- **#44 needed no new code.** `MatchingStep::EquationFailed` was already emitted and
  `matching_anim` already painted the failed row red. The feature had been *built and
  then gated out of reach* — one UI condition. A regression test now pins it
  (`a_singular_report_still_animates_and_ends_on_the_failure`: exactly one failure,
  47 of 48, on real `MotorWithBrake` data), because nothing had tested it, which is
  how it stayed hidden.
- **Sub-view links use the capture's own vocabulary.** `SubView::from_slug` resolves
  slugs *per stage*, and the slugs are exactly `structural_view_name` /
  `flatten_view_name` / `events_view_name` / `init_view_name` — #42's parity principle
  as code rather than as an aspiration.
  `link_slugs_and_capture_names_are_the_same_vocabulary` asserts the two lists cannot
  drift.

**Measured effect on the tour that exposed them:** regenerated, it went from **2
working links and 4 prose hand-offs to 9 links and none**, and Stop 3 turned from an
apology into the best stop in the tour — the same algorithm failing on the raw system
and succeeding on the reduced one, with one demotion between.

**Recording discipline.** When a tour hits a hole: add a row here *and* note it in the
[question ledger](question-ledger.md) entry for the question that exposed it. The
ledger says which question suffered; this table is what a sweep reads. A hole worked
around in prose still gets a row — that is the whole point of the quiet/loud split.

---

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
