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
