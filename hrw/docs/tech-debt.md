# Tech Debt — HRW Observatory

**Purpose:** the two sweep triggers, the priority order, the tour holes, and the outstanding
debt with its dispositions.
**Status:** authority for when a sweep fires and what it prioritises; record for the debt list
itself.
**Read when:** at a phase boundary, or after noticing that a defect was caught by Doug rather
than by the toolchain. Every sweep starts from the tour holes, then re-measures.

Quality improvements identified by code review, grouped by theme and ordered by severity within
each group.

**A closed item is cut down to one line in *Closed debt*, not left checked in place**
*(2026-08-01, matching `ideas.md`)*. A debt file is for what has not been paid; a list of
ticked boxes reads like outstanding work and hides the items that are.

**Trigger: each phase boundary, scoped to what the next phase touches** — adopted
2026-07-29, replacing a weekly calendar scan. Doug, on the change: *"my original
tech debt schedule is a prime example of old fashioned software development
mentality where teams have to agree upon weekly schedules and other such stuff
which absolutely do not matter for this project. We are entirely agile."*

The rules, in full:

- **Measure, never re-estimate.** This rule keeps paying. The 2026-07-28 sweep found
  `ui()` had grown 385 lines in a day, unnoticed. The 2026-07-29 sweep found `compile()`
  at 363 where 327 was logged. **The 2026-08-01 measurement found `app.rs` up 42% in three
  days and `compile()` refactored out of existence** — a logged item resolved by work
  nobody connected to it. Estimates would have missed all three.
- **Three outcomes are all legitimate:** *fixed*; *closed as obsolete* (the batch
  narrative workflow — the narratives it served were retired); or *deferred into the
  phase that will rewrite it*, when sweeping would mean designing an abstraction
  before its requirements exist.
- **Skip what the next phase will rewrite.** Applied three times on 2026-07-29.
- **Start from the tour holes.** They are the only items that arrive with evidence that
  they blocked a real answer, which makes theirs the only priority that is not a judgement
  call. **All four logged holes are currently closed** — so a sweep starting there today
  starts empty, which is the correct outcome and not a reason to skip the step. The
  section's *recording discipline* is the part that stays live.

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

~~`measure-fidelity.ps1` and `promote-run.ps1` meet all three conditions.~~ **Resolved
2026-08-01 by splitting them** (`verification-plan.md` item 3). `promote-run` became
`examples/promote_run.rs` with its guards tested in `src/promote.rs` — it writes a **published
claim** and needed no crate. **`measure-fidelity.ps1` stays in PowerShell**, with the reason
recorded: it needs process memory sampling, and being editable while a sweep holds the binary
is why it exists in that form.

**And one condition weakened, which is worth noticing about the trigger itself.** "Re-run
repeatedly" was true while sweeps were daily; the corpus is green now and the suite runs a few
times a year. A trigger whose conditions can lapse is working as intended — but it means the
standing candidates list needs re-reading, not just appending to.

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
3. **Tour holes** — below. These usually *degrade* an answer rather than block it, since a
   text answer remains available. **All four logged holes are closed as of 2026-07-30.**
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

**All four logged holes are closed.** They are recorded as one line each rather than as a
live table, because a table of closed rows reads like open work *(condensed 2026-08-01)*.

| Hole | Closed | How |
|---|---|---|
| `Matching ▶` hidden when Structural is singular — the view that shows *why* a rank deficiency exists, unavailable exactly when it matters | 2026-07-29 | **No new code.** `MatchingStep::EquationFailed` was already emitted and `matching_anim` already painted the failed row red. The feature was **built and then gated out of reach** — one UI condition — and nothing tested it, which is how it stayed hidden. `a_singular_report_still_animates_and_ends_on_the_failure` pins it. |
| `hrw://` cannot address a **sub-tab**, so every animation and custom view sat one level below what a link could reach | 2026-07-29 | `SubView::from_slug` resolves slugs *per stage*, and the slugs **are** the capture's own names. `link_slugs_and_capture_names_are_the_same_vocabulary` asserts the two lists cannot drift. |
| `hrw://` cannot point at a **source line**, so a tour had to *quote* one | 2026-07-29 | `hrw://source[/<line>]`, plus a tinted blamed line — ideas #45 step 2b. |
| `Canvas` cannot centre on a node | 2026-07-30 | `App::aim_at_equation` → `anim.aim_at_equation(canvas, target)`, with `camera-aiming.md` as its fixture tour. *(This row read "open (unconfirmed)" until 2026-08-01, two days after it shipped — the stale-negative class that `verification-plan.md` item 0b exists to catch.)* |

**Measured effect on the tour that exposed the first three:** regenerated, it went from
**2 working links and 4 prose hand-offs to 9 links and none**, and Stop 3 turned from an
apology into the best stop in the tour.

**Under-logged twice, and the pattern is the lesson.** The source-line row was added only
after Doug asked whether it was time to build highlighting — two tours had already worked
around it in prose and Claude had logged neither. That was the *second* quiet hole missed this
way. **Loud holes get logged because they are embarrassing; quiet ones get absorbed.** When
writing a tour, treat every "click this yourself" and every quoted-instead-of-linked reference
as a row here, mechanically, without judging whether it feels worth mentioning.

**Recording discipline.** When a tour hits a hole: add a row here *and* note it in the
[question ledger](question-ledger.md) entry for the question that exposed it. The
ledger says which question suffered; this table is what a sweep reads. A hole worked
around in prose still gets a row — that is the whole point of the quiet/loud split.

---

## Sweep history

**Condensed 2026-08-01.** Three sections recorded past sweeps blow by blow. What survives is
what a future sweep can use: what was fixed, and the lesson. Detail is in git history.

| Sweep | Scope | Fixed |
|---|---|---|
| **2026-07-28** comprehensive | before source-tooling Phases 5-7 | `collect_tracked_ancestors` short-circuited on `any`, so the tree opened a path to the **first** mention only — now folds. `ui()` 1272 → 880. The specimen source view's private copy of the tracking toggle now calls `set_tracked_identifier` like every other entry point. `tree.rs` module docs refreshed. |
| **2026-07-29** scoped to #42 | before ad hoc tours | `ui()` 982 → 325 via `FrameIntent`, closing an item open across two sweeps and *blocked* the previous time — extraction needed seven out-parameters threaded through, because egui panel closures borrow `self`. Three dead locals removed (`node_ask`, `debug_ask`, `nav_to` — folded in with `.or(…)` where the fold was always the other operand). Batch narrative regeneration **closed as obsolete**. Test-race entry **corrected**: it blamed `bridge.rs` alone, but `worker`'s stdout redirect races too, and both reproduce on a clean tree. |

**Three items were deferred into #42 rather than swept**, and the reasoning generalises:
unifying the four sub-view enums, decomposing `bridge.rs`, and aiming the canvas were all
**design decisions of the link vocabulary wearing cleanup's clothes**. Sweeping them would
have meant designing the abstraction before knowing what it must express — the mistake that
made `end_to_end_tour.md` worthless. All three have since landed as #42 work.

---

## Measured 2026-08-01

**Measure, never re-estimate** — this file's own first rule, applied to itself. Three days of
drift, and it is not small:

| | logged 2026-07-29 | **measured 2026-08-01** | drift |
|---|---|---|---|
| `app.rs` | 6,375 | **9,039** | **+2,664** |
| `bridge.rs` | 2,365 | **2,855** | +490 |
| `worker.rs` | — | **5,668** | new baseline |
| `ui()` | 325 | **410** | +85 |
| `central_panel_ui()` | 664 | **756** | +92 |
| `compile()` | 363 | **3** | **refactored away** |
| `compile_target()` | — | **454** | the body `compile()` used to hold |
| `source_map_ui()` | 245 | 245 | — |
| `generic_error_summary()` | 201 | 201 | — |

**`app.rs` grew 42% in three days** and is now over 9,000 lines. That is the fidelity, survey
and report work landing, and it is logged rather than swept because the next sweep is scoped by
what the next phase touches — Test mode (#52) and the verification pause both touch `app.rs`
heavily, and `egui_kittest` may change how its UI is structured for testability.

**`compile()` closed itself.** It is now a 3-line wrapper over `compile_target`, which took the
454-line body — so the logged item below is resolved by a refactor nobody logged. The bulk did
not go away; it moved and gained a second caller.

---

## Closed debt

**Cut down to one line each 2026-08-01**, the same treatment `ideas.md` got: a debt file is for
what has not been paid. Full reasoning is in git history and in the sweep table above.

| Item | Closed | Resolution |
|---|---|---|
| Parse stage empty for every MSL model | 2026-08-01 | Fixed — it parsed the empty string Rumoca keeps in place of source-root text; now parses the declaring file. Display-only, so the compile path is unchanged. All 2,553 MSL files verified to parse standalone. |
| **`ui()` was 1,272 lines** | 2026-07-29 | → 325 via `FrameIntent`, which bundles the ~8 intent locals that egui's borrow rules force `ui()` to act on after its panel closures end. Now **410** and growing again — logged below, not re-opened. |
| **The three animation views were near-identical** | 2026-07-29 | `playback::Playback<T>` — a **generic struct, not a trait**, deliberately: a trait would share the behaviour and leave the state declared three times, so it could still drift. `playback::Animated` is the small trait on top for the one thing that cannot be generic — what the current frame *means*. `animation_controls` went 8 positional parameters → 4. |
| **No batch narrative regeneration** | 2026-07-29 | **Closed as obsolete.** The 14 `narrative.md` files it would have driven were retired. Debt that evaporated rather than got paid — one of the three legitimate outcomes. |
| **`compile()` was 363 lines with an inlined `macro_rules!`** | *(discovered closed 2026-08-01)* | Now a 3-line wrapper over `compile_target` (454 lines), which gained a second caller — `compile_model_by_name`. **Resolved by a refactor nobody logged**, which is why the sweep rule is *measure*. |
| **67 clippy warnings in HRW, drifting upward unwatched** | 2026-08-01 | Cleared 75 (it had reached 75 — this session added 8) and set `[lints.clippy] all = "deny"`, so a new one is now a build failure. Three were not style: four constant assertions became compile-time `const` blocks, `--fix` relocated `impl Canvas` above `mod tests` (the lint for the debugger-launch bug shape), and a doc block turned out to have been documenting the wrong function. |
| **Crash files were never pruned, and `cargo test` failures wrote them** | 2026-08-01 | The accumulation was the symptom; **the panic hook is process-global**, so every failing assertion left a `crash-*.json` looking like an app crash — all five found that day were test failures. Now: no full file under `cargo test`, newest 3 kept, and a `crashes.log` digest appended forever so pruning loses no recurrence history. |

---

## Code quality / duplication

- [ ] **`app.rs` is 9,039 lines** *(measured 2026-08-01, up from 6,375 on 2026-07-29 — **+42% in
  three days**).* The growth is the fidelity, survey and report work. **Do not sweep yet:**
  Test mode (#52) and the verification pause both touch it heavily, and `egui_kittest` may
  change how its UI is structured for testability. Sweeping now would be designing against
  requirements that arrive next week.
  *File:* `app.rs`.

- [ ] **`ui()` is 410 lines** *(was 325 after the 2026-07-29 extraction).* Grew back by 85 in
  three days. Not alarming on its own; recorded so the next sweep sees a trend rather than a
  number.
  *File:* `app.rs`.

- [ ] **`central_panel_ui()` is 756 lines** *(664 when extracted 2026-07-29).* A coherent unit —
  stage tab bar, status banners, sub-tab bars, per-stage dispatch — and it takes only
  `&mut FrameIntent`, so further extraction is unblocked. The natural next cut is per-stage.
  **Do not split before #52** reworks the sub-tab bars; that deferral is deliberate and Doug
  acknowledged it.
  *File:* `app.rs`.

- [ ] **`bridge.rs` is 2,855 lines** *(2,365 on 2026-07-29).* It owns `focus.json`, so it owns
  half of noun parity, and #42 kept touching it. Decomposition was deferred into #42 rather
  than swept; #42's remaining work is small, so this is now a genuine candidate.
  *File:* `bridge.rs`.

- [ ] **`Expansion::force_open` exists only because "Reveal identifiers" is a mode.**
  Source-tooling Phase 6 is expected to make revealing an *action*, at which point forcing
  headers open every frame becomes unnecessary and the struct collapses back to one set. Remove
  it when that lands.
  *File:* `tree.rs`.

- [ ] **`lldb.verboseLogging` is off but retained** in `.vscode/settings.json` with a comment
  explaining when to switch it on. **Keep** — it documents a diagnostic that was hard to find.

## UI defects — found by walking

- [ ] **A selected fixture tour does not always open scrolled to the top.**
  *(Doug, 2026-08-01, walking the fixture tours as a smoke test.)*

  **Cause identified, not guessed.** The tour pane is
  `egui::ScrollArea::vertical().id_salt("tour")` (`app.rs`) — **one fixed id for every
  tour** — and egui persists scroll offset per `ScrollArea` id. Switching tours therefore
  reuses the previous tour's offset. `select_tour` already resets the right-hand side on a
  real change (specimen, stage, log, compiling) and deliberately *not* on re-selecting the
  same tour; **the scroll offset was simply never included in that reset.**

  **Fix:** extend the existing "only on an actual change" branch in `select_tour` to raise a
  flag, and have the pane apply `.vertical_scroll_offset(0.0)` for one frame. Salting the id
  per tour is the other option and is worse here — it would make each tour *resume* where it
  was left, which contradicts the reset that branch exists to perform.

  **Why it matters more than it looks.** Opening mid-document means Stop 1 is off-screen, so
  a tour whose whole purpose is a sequence starts by hiding its start — the same species as
  the RHS-not-re-initialising bug, which made Stop 1 look already done.

  **This is the class `egui_kittest` is for** (`verification-plan.md` item 2): its table
  already lists *"the RHS doesn't re-initialise on a second tour"*, and *"scroll offset is 0
  after selecting a different tour"* is the same assertion shape. **Caught by Doug, which the
  backward trigger says is the signal** — nothing checks the rendered surface today.
  *File:* `app.rs`.

- [ ] **`compile_cost` makes the survey artifact non-reproducible.** *(Measured 2026-08-01
  while verifying the runbook.)* The column is **wall-clock derived**, so it varies with how
  many shards are competing: re-running the survey at 4 shards against the committed 6-shard
  artifact moved **16 of 2,626 models, every one `slow` → `fast`** — one direction, which is
  load rather than randomness.

  **It matters because of who we hand this to.** `upstream-strategy.md` requires anything
  published to be **reproducible**, and a maintainer's natural check is *regenerate and diff*.
  Sixteen spurious rows is exactly the noise that teaches people to ignore diffs — the same
  argument `upstream-issues.md` issue 3 makes about the `message` column.

  **Options, none chosen:** drop the column from the committed artifact and keep it in the
  health log; bucket it far more coarsely so threshold-crossing is rare; or record the shard
  count in the sidecar and state that the column is comparable only within a shard count. The
  runbook now documents the instability and gives a diff recipe that excludes it, which is the
  honest interim.
  *Files:* `examples/survey_msl.rs`, `docs/long-runs.md`.

## Robustness

- [ ] **Test races under parallel execution — two causes, not one.**
  *(Corrected 2026-07-29: the entry previously blamed only `bridge.rs`.)*
  1. `bridge.rs` tests share `.hrw-bridge/focus.json` — requires parameterizing the bridge dir
     to fix.
  2. `worker::tests::output_capture_handles_large_write_without_deadlock` redirects
     **process-global stdout**, so any concurrently-running test that writes to stdout steals
     bytes. Inherently exclusive; would need a serial guard or a non-global capture mechanism.

  Both reproduce **on a clean tree** — verified by stashing. Mitigated by `--test-threads=1`,
  required for the *whole* suite. **Worth fixing before any CI, since the default harness both
  fails *and* hangs** — and a hang reads as a broken build rather than a test-isolation problem.
  *Files:* `bridge.rs`, `worker.rs` — test modules.

- [ ] **A stage's emitted JSON depends on what else the session holds.**
  *(Found 2026-08-01 while building the memoisation guard for #48.)* Compiling `Drivetrain`
  early in the test run and again late produces **different Resolve JSON**, against the same
  shared `Session`. The compiles are identical; what changed is that the session has
  accumulated every other specimen's document in between.

  **Not yet characterised** — the difference could be diagnostics that name other documents,
  or something that would matter more. It was found by elimination (two back-to-back compiles
  agree; two separated ones do not), not by inspecting the diff, so **treat the cause as
  unverified.**

  **Why it is worth chasing.** HRW shows the Resolve tab for *one* model, and if that view
  varies with what was loaded earlier in the session then two users — or the same user in two
  orders — see different things for the same file. It is adjacent to
  [`upstream-issues.md`](upstream-issues.md) issue 1, where a *failed* resolve leaked into the
  next model's result; this is the same surface with a milder symptom.

  **First step is a probe, not a theory:** compile a specimen, dump Resolve's JSON, compile ten
  others, compile it again, and diff. Cheap, and it converts a suspicion into a fact.
  *Files:* `worker.rs`, and possibly `crates/rumoca-compile` session caching.

- [ ] **`build_declaring_classes` resolves only the first path segment.** `src.V` resolves
  exactly; `gear.flange_a.tau` yields `gear`'s type, which *contains* the declaration rather
  than being it. The UI wording says "in" rather than "declared in", so it is honest — but a
  deeper resolution would need to walk into library class IRs that are loaded on demand.
  *File:* `app.rs`.

## The Context Bar reported less than it emitted, and only Doug noticed

**Who caught it: Doug** — which is the backward sweep trigger. Nothing in the
toolchain could have: the bar had **no test of its own**, so every claim it made
was true and the claims it *failed* to make cost nothing.

Two distinct omissions in one widget, both invisible for the same reason:

- the **stage** was never drawn, from the bar's first commit (`b2732393`);
- the **specimen** was drawn only in the populated branch, so the empty-state
  branch — the common case — rendered `Context — nothing assembled` while
  `focus.json` was carrying both.

**The general shape: a reporter that under-reports.** The must-fire rule says
silence must be a failure, and the bar was never silent — it spoke, just
incompletely. **Everything it said was correct**, which is why review, tests and
daily use all passed over it. A partial report is harder to catch than no report,
and this one sat in the widget whose single job is to make emission legible.

**What now checks it:** `ui_tests::the_background_names_both_the_specimen_and_the_stage`
and `..._names_the_stage_before_a_model_exists`. The two branches share one
`background_ui`, so they can no longer drift apart.

**What remains unswept:** the rest of `context_bar_ui`'s rows have no headless
test either. The point-at and follow rows are *labelled*, so an omission there is
visible in a way the background's was not — which lowers the priority without
removing it.

## UI testing debt — the harness exists and almost nothing uses it

**Logged 2026-08-01, at Doug's request**, immediately after the Context Bar defect
above. That defect is this item's evidence, not a separate story: `egui_kittest`
landed in the verification pause on the same day, and the bar it could have
guarded shipped unguarded for weeks.

**The debt is not "few tests". It is that the untested surface is invisible.**
A missing unit test leaves a function that still looks untested. A missing UI
test leaves a *screen that looks fine* — the reader cannot tell a pane that
renders everything from a pane that renders most things, because **what is
omitted leaves no gap where it was**. The Context Bar said three true things and
skipped a fourth; nobody sees a fourth thing missing.

### Measured 2026-08-01

- **11 tests** in `src/ui_tests.rs`, against **17 `*_ui` functions** in `app.rs`.
- **5 of the 11 are tour-link tests.** They exist because tours were the first
  thing with a machine-checkable contract (`fixture_tour_links_all_resolve`), not
  because tours are the highest-risk surface.
- **Panes with no headless test at all:** `menu_bar_ui`, `equation_sheet_ui`,
  `source_map_ui`, `specimen_source_ui`, the stage tab row, the chat panel, the
  help panel, the log view, the Purpose tab, and the status bar's notices.

### What the harness genuinely cannot see — and it is narrower than assumed

`egui_kittest` queries the **accessibility tree**, so anything drawn straight to
a `Painter` is invisible to it. Checked rather than presumed: HRW has ten painter
call sites, and **most are decoration layered under real widgets** — the jump
highlight in `tree.rs`, the selection fills, the stage-diff rule. Those panes are
widget-based and **are** testable.

Only two surfaces paint their content as text and are therefore genuinely out of
reach: **`incidence_view.rs:457`** (the matrix cell glyphs) and
**`spyplot.rs:289`**. The **animations are testable** — their controls, step
labels and state text are ordinary widgets; only the matrix underneath is not.

**This matters for the division of labour.** The standing plan is that fixture
tours cover what cannot be automated, and that set is much smaller than it looked
— which means tours should *not* be spent re-walking widget panes a headless
test can hold. Two things stay Doug's alone regardless of harness reach:
**colour** (fills carry meaning throughout HRW and the tree records no colour)
and **layout** (a widget laid out off-screen is still in the tree — the reason
the harness runs at 1600×1200).

### Priority order for paying it down

Not "write tests for all 17". The ordering follows the failure this item came
from:

1. **Panes that report** — the status bar's notices, the log view, the equation
   sheet. Same shape as the Context Bar: they exist to say something, so a
   partial report is both plausible and unnoticeable. **The must-fire rule
   already covers these in principle and has never been applied to a pane.**
2. **Panes whose emptiness is legitimate**, where "nothing here" and "broken"
   look identical: `specimen_source_ui` (which was silently empty for library
   models until 2026-08-01), the Purpose tab, the source map.
3. **Everything reachable by a click that changes state elsewhere** — the stage
   tab row, the mode switch. Cross-pane effects are where a test beats a walk,
   since a human checks the pane they clicked in.

**The animations are testable, and none of them are tested** *(added 2026-08-02, from
`ui-findings.md` C6/H7)*. The earlier reading of this entry assumed the animation panes were
out of reach because they sit near painters. Checked: only `incidence_view.rs`'s cell glyphs
and `spyplot.rs` paint their content as text — every animation's **controls, step labels and
state text are ordinary widgets**. Six panes, no tests, and no line in the pause's four steps.
**Not scheduled**, deliberately: they are not on the refactor's path, so they are debt rather
than blocking work.

**A third surface the harness cannot reach: scroll-area configuration** *(added
2026-08-04)*. `ScrollArea::both()` versus `ScrollArea::vertical()` decides whether a
horizontal scrollbar is **offered**, and that is configuration rather than behaviour —
nothing observable differs. Established by measurement, not assumed:

- The rendered row's a11y `rect()` is **logical** and reports full width whether or not
  the pane can scroll to it.
- `content_size.x` already exceeds the viewport under *both* settings, so it cannot
  distinguish them either.
- Wrapping cannot be guarded from outside either: under `both()` the inner `Ui` gets
  infinite width, so forcing `TextWrapMode::Wrap` changes nothing measurable.

**Three tests were written for this and all three passed on the unfixed code.** Each was
caught by reverting the fix and watching the test stay green, and then deleted —
*"a test that can pass while checking nothing is worse than none."* The honest record is
this entry, not a green assertion.

**What would make it testable** is a behavioural probe rather than a geometric one:
drive a horizontal scroll and observe the offset move. `egui_kittest` has no ergonomic
way to do that today, which is why this is debt and not a task.

**Do not chase a coverage number.** The metric that would matter is *panes whose
reports are guarded*, and counting tests instead would reward the tour-link tests
that already dominate the file.

### The rule going forward

**A new pane that reports something ships with a headless test, the way a new
reporter ships with a must-fire test.** Retrofitting the existing 17 is the debt;
not growing it is free. The Context Bar is the worked example — two guards, both
of which fail against the code as it stood that morning.

## Remove the split reporting once the LHS width has proven itself

**Logged 2026-08-03 at Doug's request**, immediately after the width was confirmed working.

`SplitState::observe` writes up to six lines per session to the diagnostics file recording the
available width, the panel width and the resulting fraction. It exists because **five attempts
at the opening width failed and the sixth succeeded the moment somebody looked at the numbers**
(`ui-findings.md` C15).

**It is committed, which under `CLAUDE.md`'s probe rule makes it a decision rather than a
drift.** The case for keeping it: it is capped, it writes only to the diagnostics file, it costs
nothing per frame, and if the width ever misbehaves again it turns another five-attempt loop
into one restart. The case for removing it: it was added to diagnose a bug that is now fixed,
and instrumentation that outlives its question is how a codebase accretes noise nobody dares
delete.

**The trigger is confidence, not a date.** Remove it when the width has survived a stretch of
ordinary use — different window sizes, a maximise, a restart or two — without a surprise. If it
is still there and nothing has gone wrong for weeks, that is the answer.

**If it is removed, remove the whole path**: `reports_left`, `log_split`, and the
`record_action("split", ..)` call. Leaving a disabled reporter behind is worse than either
choice, because the next reader cannot tell whether it is off on purpose.

## `cargo fmt --all -- --check` fails, and it is `hrw/`'s fault alone

**Logged 2026-08-03 at Doug's request**, after `cargo fmt` was ruled out mid-change as too
disruptive to run in passing.

### This is a live CI failure, not a tidiness preference

Three facts, each measured rather than assumed:

- **Upstream CI runs it.** `.github/workflows/ci.yml:86` — `cargo fmt --all -- --check`.
- **`hrw` is a workspace member.** `Cargo.toml:62`. So `--all` includes it.
- **The `crates/` are already clean and `hrw/` is not.** Running the exact CI command under the
  pinned toolchain (`rust-toolchain.toml`, `nightly-2026-02-27`) reports **zero** hunks under
  `crates/` and roughly **900** under `hrw/`.

So the fmt job is red on this branch, and **`hrw/` is the entire reason.** That was not known
when the entry was requested — the assumption was that formatting was merely unenforced.

Worst offenders: `worker.rs` (276 hunks), `app.rs` (231), `bridge.rs` (56), `fidelity.rs` (42),
`tree.rs` (32), `ui_tests.rs` (30). Long tail across most other modules.

### What this does and does not block

**It does not block upstreaming.** An upstream PR is a cherry-pick of `crates/rumoca-*` changes
only (`CLAUDE.md`, the separable-commits rule), and those are clean. The instrumentation
commits would pass CI on arrival.

**It does block having a trustworthy green build**, which `docs/upstream-strategy.md` stakes
Doug's credibility on: work that is *reproducible and honestly bounded*. A maintainer glancing
at the fork sees a failing check and cannot tell that it is confined to a directory their PR
will never contain.

### Why it is not fixed in passing

**`cargo fmt` would touch ~900 sites across most of `hrw/`**, which buries whatever change it
rides along with. This was hit for real on 2026-08-03: wrapping the playback bar in a frame
left one closure body under-indented, and the correct-looking fix — run `fmt` — would have made
a four-line change unreviewable. It was re-indented by hand over a fixed line range instead.

### How to do it

**One commit, containing nothing else.** The whole value is that it is mechanically verifiable:
a reviewer confirms `cargo fmt --all` was run and the tests still pass, and reads none of it.

1. Run the full suite first and record the count, so "the same tests pass" is checkable.
2. `cargo fmt --all`, then `cargo clippy -p hrw --all-targets` (**check the exit code**, per
   `CLAUDE.md` — a compile error survives a warning-count grep).
3. Full suite again, `--features slow-tests`. Same count, no failures.
4. Commit with a subject that says it is formatting only, and push nothing else with it.

**Two things to watch.** `doc_citations` asserts on source structure — the field-count ratchet
parses `pub struct App` by line shape, and the two-`#[test]` check scans attributes; both should
survive reformatting, but they are the tests most likely to be sensitive to it, so run them
first. And the `#[rustfmt::skip]` escape hatch exists for any table-shaped literal whose
alignment carries meaning: **prefer skipping a block to accepting a formatting that makes data
harder to read**, since the source is a learning artifact here.

### Then keep it clean

Once green, formatting becomes a pre-commit step rather than a project. Until then, **do not run
`cargo fmt` as part of unrelated work** — hand-indent the block and note it, as
`f3c6fb90` did.

---

## A3 — the duplicate resolve: measured, solved, and reverted at the last step

**Status 2026-08-04: not fixed. The fix is known and proven to work; one error path
defeats it.** Recorded here rather than left in a branch, because the measurement was
the expensive part and it should not need repeating.

### The problem, measured

Every HRW compile resolves the whole library **twice** — confirmed, not inferred: a
traced compile emits two `resolve timing summary` lines, `def_count=38855` and
`class_count=6521` both times.

`Session::tree()` builds under `ResolveBuildMode::Standard`. A strict compile builds
under `StrictCompileRecovery`. **Both are cached, but separately**, so inspecting the
tree and then compiling resolves twice. Nothing is wrong with either — HRW simply had
no way to ask for the tree the compile was about to build.

### The fix, and that it works

Add `Session::strict_compile_resolved()` — additive, public, returns the `Arc` — and
have HRW call it instead of `tree()`. The compile then finds its tree cached.
Measured with the change in place:

| | Resolve | Whole compile |
|---|---|---|
| before | 883 ms | ~2000 ms |
| after the clone fix (`tree()`, landed) | 740 ms | 1815 ms |
| **after this** | **596 ms** | **1102 ms** |

`resolve timing summary` count went **2 → 1**. The compile roughly halves.

### Why it was reverted

**Recovery mode succeeds past errors that `Standard` treats as fatal** — that is what
recovery is for. So `Ok` no longer implies the model resolved cleanly, and HRW's
Resolve tab stopped reporting resolve failures. Two tests caught it:
`a_broken_specimen_does_not_poison_the_next_compile` and
`a_resolve_failure_names_the_reference_and_its_line` (*"a resolve failure must carry a
structured payload"*).

That is the tests working. The old code used `tree()`'s `Err` as its failure signal,
and the replacement has no `Err` to use.

### What finishing it needs

The diagnostics are available — `build_resolved_for_strict_compile_with_diagnostics`
returns them alongside the tree, and the accessor was already amended to pass them
through. The remaining work is **deciding which diagnostics mean "this model failed to
resolve"**: the set includes library-wide diagnostics that must not fail a good model,
and the existing failure arm gets its structured payload from a *model-scoped* call
(`compile_model_diagnostics`) rather than from the error value.

So: on `Ok`, filter to the model's own errors and route to the existing failure arm
when any exist. **Not attempted** — the semantics of model-scoped versus library-wide
diagnostics were not verified, and guessing them would produce a Resolve tab that is
silent on real failures, which is worse than the duplicate resolve.

---

## The animations still re-deriving, and what each needs

**Matching and Tarjan now replay the compile's own run** (2026-08-04). Three remain,
recorded with the specific blocker rather than as a vague "finish B".

**`tearing_anim` — blocked on block names.** The Rumoca-side capture is **already
landed** (`tearing::{start_capture, take_capture}`, segmented one entry per coupled
block). What is missing is HRW-side: `TearingAnimation::record` gets its per-block
*names* from `walk_blocks`, the same walk that does the tearing — so consuming
captured frames still needs names from somewhere. The structural report carries a
`blocks` array; the work is mapping that to the animation's block-names type. Not
started, and deliberately not begun at the end of a long session.

**`reduction_anim` — probably already correct, unverified.** It is fed by
`index_reduction_frames`, which *are* captured from `index_reduction_stage`'s real
run. It also names `tarjan`, so **check whether it re-derives anything** before
assuming either way. One grep answers it.

**`ic_plan_anim` — needs a capture that does not exist yet.** `ic_plan.rs` traces
through `tracing` (`ic_plan_trace_enabled`), not frames, so unlike the others there is
no `*_with_trace` variant to switch to. That is a larger change: give the pass a frame
type first, then the capture.

### The pattern, now applied five times

`rumoca-phase-flatten` (connections), `rumoca-phase-dae` (pre-lowering),
`rumoca-phase-structural` (matching, Tarjan, tearing). Two shapes, and which one
applies is decided by whether the untraced entry point already routes through the
emit site:

- **Branch at the call site** when there are two implementations (matching, Tarjan):
  the caller picks the traced path only while a scope is open.
- **Hook the emit site** when the untraced entry *is* the traced one with `None`
  (connections, pre-lowering, tearing): no call-site change at all.

Both cost one thread-local read when closed, and neither moves a signature.
