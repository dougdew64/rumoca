# Plan — making the environment verify more, so Doug verifies less

**Purpose:** the items that make the toolchain catch what Doug currently catches by hand.
**Status:** **live plan.** Update it as items land; delete it when all six are complete and
their conventions have moved into `CLAUDE.md` and `tech-debt.md`.
**Read when:** picking up the pause, or when tempted to add a plan item — the ordering rule is
*what makes the rest cheaper*, not what is biggest.

**A deliberate pause in feature work**, agreed 2026-08-01. Oracle testing and Test mode resume
when this is done. *(Items 0b and 0c added 2026-08-01 during the document and tech-debt
cleanups, each on evidence those cleanups turned up — a stale-negative claim that nearly caused
duplicated work, and a clippy lint that was firing on the exact shape of a shipped bug.)*

## Why this pause exists

Doug: *"Anything which slows down your ability to help bring my ideas to life is absolutely
worth fixing now."*

The evidence is a day of measurement rather than a feeling. On 2026-08-01, building the
fidelity harness produced roughly a dozen defects. **Not one was caused by tech debt** — they
divided cleanly by *whether the toolchain could see them*:

| Where | Defects | Who caught them |
|---|---|---|
| `fidelity.rs` — Rust, tested, clippy-clean | the checks themselves worked | the toolchain |
| PowerShell / bash / Python scaffolding | **all seven silent bugs** | **Doug** |
| Causal claims written before measuring | four | **Doug** |

And the standing asymmetry, which predates that day:

> **11,939 lines of UI code. One test that exercises rendering.** Everything else is verified
> by Doug running labs.

`docs/tech-debt.md`'s second trigger names the property: **verifiability, not Rust.** This plan
is that trigger's first application.

## The six items, in order

Ordered by *what makes the rest cheaper*, not by size.

**Numbers are identifiers; execution order is separate.** Items 0b and 0c were inserted on
2026-08-01 ahead of item 1, whose own text says *"First, because every later phase pays this
cost repeatedly"* — so the numbering contradicted the rationale. **Item 1 was done first**, and
it was the right call: the full suite went 375s -> 113s, and every remaining item runs it
repeatedly.

---

### 0. The must-fire convention — adopt now, not a phase — DONE 2026-08-01

**Every piece of code whose job is to report something gets a test proving it reports.**
Silence must be a failure, never a pass.

The fidelity checks already have this
(`fidelity::tests::each_invariant_catches_its_own_violation`). The tooling around them did
not, which is precisely where all seven silent bugs lived: a dead column, a collapsed array
argument, a swallowed `eprintln!`, a rate limiter gating its own first fire, an announcement
silent when work was pending by absence.

**Done when:** the rule is in `CLAUDE.md`, and every existing observer either has a must-fire
test or a recorded reason it cannot. **No dependency, no approval, no schedule** — it applies
from now on, and its absence makes a change incomplete.

#### The audit, 2026-08-01

**Scope.** An *observer* is code whose job is to report a problem, where producing nothing is
a possible outcome. Found by enumerating detector-shaped functions and then by the shape most
at risk of a vacuous pass: an assertion that a collection of findings **is empty**. Seventeen
such assertions exist; most are ordinary positive tests (*"no span means no line to blame"*)
rather than "the checker found nothing", and only the checkers below carry the risk.

| Observer | Proof that it fires |
|---|---|
| `fidelity::check_model` — F1-F9 | `each_invariant_catches_its_own_violation` |
| `survey::all_zero_columns` | `a_column_that_is_always_zero_is_reported` |
| `report::exceptions` | `exceptions_are_recognised_across_reports` — asserts *only* the failing row |
| `doc_citations::every_documented_source_path_exists` | `a_missing_file_is_reported` |
| `doc_citations::claims_of_absence_are_still_true` | `the_unbuilt_tag_is_parsed_and_both_verdicts_fire` |
| `doc_citations::provenance_tags_are_well_formed` | `provenance_tags_are_recognised_by_form` |
| `doc_citations::documents_contain_no_stray_control_characters` | **`a_stray_control_character_is_reported` — added by this audit** |
| `app::fixture_lab_links_all_resolve` | `parse_hrw_link_invalid_stage` proves the predicate rejects; the test carries its own non-vacuity guard |
| F8's stage-IR ceiling | non-vacuity guard on row count — *"a model produced no row, so the loop exited early"* |
| `specimen_purpose::purpose_placeholder` | `the_purpose_placeholder_fits_the_actual_state` |
| `bridge::check_breakpoint_ack` | `live_trace_breakpoint_arm_remove_and_ack` |
| diagnostics digest + pruning | `a_digest_entry_is_one_line…`, `pruning_keeps_the_newest_crash_files` |

**One gap found and closed.** The control-character checker had been verified *by hand* —
injecting a BEL into `provenance.md` and watching it fail. **That proved it worked that day
and nothing about any later day.** A manual proof is not a must-fire test, and it was the only
checker in the codebase without an automated one.

#### The recorded reason, for the observers that cannot have one

**PowerShell has no test harness**, so `scripts/measure-fidelity.ps1`'s watchdog narration and
its retry-verdict clearing are verified only by being run. *(Updated 2026-08-01: the promotion
guards were in this sentence until item 3 moved them to `src/promote.rs`, where they are now
tested. This is the stale-negative class in miniature — a recorded limitation that stopped
being true.)* On 2026-08-01 that verification was done deliberately and by hand — a seeded
profile proving the retry clears the right rows, plus a control showing the array
normalisation is load-bearing — but **it is not repeatable without a person.**

**This is not an exemption; it is item 3.** Those scripts are precisely the standing candidate
in `tech-debt.md`'s second trigger, and the honest position is that they are *unverified
between runs* until item 3 either moves them to Rust or records why mid-run editability is
worth more. Recording it here so the gap is a known one rather than an oversight.

---

### 0b. Make a claim of *absence* checkable — the stale-negative test — DONE 2026-08-01

**The mirror of `doc_citations.rs`.** That test asserts every cited path **exists**. This one
asserts that everything a document claims **does not exist** still does not.

**The evidence, found 2026-08-01 while cleaning the documents** — a class the pause's original
four items do not cover, because nothing here is silent tooling or UI:

| Stale claim | Reality | How long |
|---|---|---|
| #42: frame position, pointed-at node, followed identifier are *"still below the reach of a link"* | all three shipped; **three fixture labs actively test them** | 2 days |
| #42: *"specimens become a medium of explanation… it is currently impossible"* | `.hrw-bridge/specimens/` shipped | 2 days |
| #45 audit: structural *"spans are dropped"* | fixed — and **step 1 of the same idea said so** | 3 days |
| `context-assembly.md`: *"not yet implemented"* | the Context Bar shipped 2026-07-28 | 4 days |

**Why this class is worse than it looks.** A wrong *positive* claim gets caught the moment
someone acts on it — you go to use the thing and it is not there. A wrong *negative* claim is
never caught, because acting on it means **not looking**. The natural response to "that is not
possible yet" is to build it, and #42 was two days from having its link vocabulary
re-implemented on top of itself. **It is the one error whose cost is paid in duplicated work
rather than a failed test.**

**The mechanism — a tag, because free prose cannot be checked.**

```markdown
Frame addressing is not built. <!-- unbuilt: hrw://stage/*/frame -->
Scratch specimens do not exist. <!-- unbuilt: App::scratch_specimens -->
```

The test resolves each `unbuilt:` target and **fails if it resolves** — a link slug against
`SubView::from_slug` and the fixture-lab corpus, a Rust path against the source. The failure
message says *"`ideas.md` line N claims X is unbuilt; it exists at Y"*. `doc_citations.rs`
already has the scanner, the boundary-matching and the workspace-root plumbing, so this is
mostly a second predicate over machinery that exists.

**And a non-vacuity guard, without which this is theatre.** Zero tags means zero failures,
which reads exactly like zero staleness. The test **prints its tag count**, and a companion
lint lists untagged negative phrases — *"not yet"*, *"currently impossible"*, *"does not
exist"*, *"below the reach"* — as a **warning, never a failure**, so the retrofit stays lazy
in the way provenance tags are. **Coverage is expected to be low and that is fine**; a *wrong*
tag fails, because a tag is a claim.

**Done when:** the tag is honoured by a test in the fast loop, the four claims above are tagged
or removed, the untagged-phrase lint prints, and the convention is in `CLAUDE.md` beside the
must-fire rule — which is the same principle, pointed at absence instead of silence.

**Explicitly out of scope:** understanding free-form prose. This catches what someone chose to
tag; it does not read English. That limit is why the lint exists.

**Delivered 2026-08-01** in `src/doc_citations.rs`, three tests in the fast loop:
`claims_of_absence_are_still_true` (fails when a tagged target resolves),
`the_unbuilt_tag_is_parsed_and_both_verdicts_fire` (proves both verdicts are reachable, so the
checker cannot pass by matching nothing), and `untagged_claims_of_absence_are_listed` (the
lint — prints, never fails). The convention is in `CLAUDE.md` beside the must-fire rule.

**Two bugs in the checker, both found by running it rather than reading it:**

1. **`*` matched exactly one segment**, so `hrw://stage/*/frame` failed against the real
   `hrw://stage/Structural/MatchingAnim/frame/41` — the check reported a *shipped* capability
   as absent, which is the very error it exists to catch, committed by the catcher. Now a
   subsequence match: named segments in order, `*` skipping any number.
2. **Example tags inside code fences were read as live claims.** This document's own worked
   examples are drawn from real stale claims, so the first run reported *the documentation of
   the mechanism* as a defect. Fences are skipped now — an example is not an assertion.

**Coverage is 1 tag, deliberately** (`survey_filter`), verified absent before tagging. The resolver
errs toward "still absent" on doubt, because this test fails the build: a false positive costs a
wrong failure, a false negative leaves a claim for the lint to surface.

*(`last_walked` was the second tag and was removed on 2026-09-01 along with the paragraph carrying
it. It marked the absence of run-tracking derived from the action trail — work Doug ruled must not
be done, having retired the `run:` markers on 2026-08-31 and the running discipline itself the
next day. **An absence tag on forbidden work is worse than none**: it reads as a to-do a later
session may pick up in good faith, which is exactly what an `unbuilt:` tag is designed to invite.)*

---

### 0c. Clear HRW's clippy warnings, then deny them — DONE 2026-08-01

**67 warnings, and the count is what makes them dangerous, not any one of them.** Measured
2026-08-01; 63 were noted informally on 2026-07-29, so it drifts upward unwatched. **A warning
count nobody reads is where a real warning hides** — and `cargo clippy --all-targets` is the
*only* check that covers the binary, which `cargo test` does not build.

**The evidence is not hypothetical, and it is close to home.** Among the 67:

> ```
> warning: items after a test module
>    --> hrw\src\canvas.rs:249:1
> 249 | mod tests {
> 476 | impl Canvas {
> ```

**`items_after_test_module` is the lint for the exact shape of the bug that broke Doug's
debugger launch on 2026-07-31** — code placed after `#[cfg(test)] mod tests`, where a
misapplied `#[cfg(test)]` let two helpers compile into `--bin hrw` referencing test-only
imports. Every test passed; the binary did not. **Clippy had a lint for it, the lint was
firing, and it was invisible in the noise.** That is the entire argument for this item.

**What the 67 actually are** — mostly mechanical, which is why this is cheap:

| Kind | Count | Note |
|---|---|---|
| `collapsible_if` | 18 | style |
| `field_reassign_with_default` | 7 | style |
| `map_or` simplifications | 6 | style |
| `manual_contains` | 4 | minor efficiency |
| **`assertions_on_constants`** | **4** | **not style** — four `#[test]`s assert relationships between compile-time constants (`MIN_ZOOM > 0.0`, `MAX_ZOOM > MIN_ZOOM`). As `const { assert!(…) }` they fail at **compile** time instead of test time, which is strictly better verification and squarely this pause's theme. |
| **`items_after_test_module`** | **1** | **not style** — see above |
| ~25 others | 1-3 each | incl. `manual_is_multiple_of` and other **toolchain drift**, not new bad code |

**`cargo clippy --fix` handles 34 of them**, so most of the work is review rather than typing.

**Then deny, or it comes back.** `hrw/Cargo.toml` already carries a `[lints.clippy]` block
allowing `excessive_nesting` and `too_many_arguments` — deliberately, since the Rumoca crates'
complexity budget governs a compiler and not a UI. **HRW does not inherit the workspace's
`all = "deny"`.** The fix is to opt in at the crate level, keeping those two allows.

**Done when:** `cargo clippy -p hrw --all-targets` is clean, HRW's `[lints.clippy]` denies by
default, and the two existing allows carry their recorded reason.

**The discipline that makes it stick:** a lint that is genuinely wrong for a UI crate gets an
**allow at crate level with a written reason**, never a scattered `#[allow]` at the call site.
A crate-level allow is one line someone can argue with; a sprinkling of local allows is
indistinguishable from the noise this item exists to remove.

**Out of scope:** the Rumoca crates. They are already clippy-clean under
`[workspace.lints]`'s `all = "deny"`, and that must stay true — a lint the instrumentation
introduces would fail upstream CI.

**Delivered 2026-08-01. 75 -> 0, and `[lints.clippy] all = "deny"` in `hrw/Cargo.toml`.**
(75, not the 67 measured two days earlier — *this session's own doc comments added eight*,
which is the drift in miniature.) `cargo clippy --fix` cleared 48; the rest were done by hand.
**Verified the deny actually fires** by injecting a violation: 5 errors, then removed.

**Three were not style, and two of those were real defects:**

| | |
|---|---|
| `assertions_on_constants` x4 | Three `#[test]`s asserting relationships between compile-time constants became `const` blocks. **They now fail the build rather than the test run** — a constant's range cannot be wrong only when tests happen to execute. Test count 416 -> 413 for this reason. |
| `items_after_test_module` | **`--fix` relocated `impl Canvas` above `mod tests`** — 142 lines moved, verified a pure move (added and removed line sets identical). This was the lint firing on the shape of the 2026-07-31 debugger-launch bug. |
| `doc_lazy_continuation` | **A doc block had been documenting the wrong function.** The paragraph describing `instantiate_and_typecheck` sat above `record_connection_frames`, because that function was inserted between it and its own — and a doc comment attaches to the *next* item. Silently wrong for however long, and clippy was pointing at it the whole time. |

**One allow, with its reason, item-level.** `large_enum_variant` on `FromWorker`: the lint's
remedy is boxing `Compiled`, which would add an allocation and force ~40 match sites to
dereference, to save memory nobody can observe — **one of these exists at a time.** Recorded
on the item rather than crate-wide, because the judgement is about that enum. The convention:
crate-level allow for a crate-wide judgement, item-level for an item-specific one, and
**never an undocumented `#[allow]`**.

---

### 1. Shorten the pre-commit suite (`docs/ideas.md` #48) — DONE 2026-08-01

**First, because every later phase pays this cost repeatedly.** Six minutes is long enough
that Claude runs the full suite less often than it should, which is its own risk.

Memoise compiled specimens: **37 `compile_specimen_shared` call sites cover only 12 distinct
models**, and `Drivetrain` is compiled five or six times per run.

**Done when:** the full suite is meaningfully under six minutes with all 470+ tests still
passing, and #48's recorded caveat is honoured — **one test still compiles a specimen fresh
and compares against the memoised result**, so memoisation cannot hide a reproducibility
failure.

**Delivered 2026-08-01. 375s -> 113s (3.3x), 476 tests passing.** `FromWorker` and `SimData`
gained `Clone`; `test_msl` memoises one compile per specimen per process and hands out copies;
`compile_specimen_uncached` is the opt-out. The caveat is honoured by
`compiling_a_specimen_twice_is_reproducible`, which compares **every compilation stage's
emitted JSON**.

**Two findings the work produced, both worth more than the speedup.**

1. **The guard test's first form was wrong, and the full suite caught it.** Comparing the memo
   against a *later* fresh compile failed on Resolve — because the shared session accumulates
   documents between the two, so they were never comparable. Now two back-to-back compiles.
   The session-dependence itself is logged in `tech-debt.md`.
2. **Inserting the test silently un-tested another one.** The new function landed between
   `a_broken_specimen_does_not_poison_the_next_compile`'s attributes and its `fn`, so it
   inherited a second `#[test]` while that function — the regression guard for upstream issue
   1 — quietly stopped being a test. Caught only because the duplicate name made the harness
   list 476 entries with 475 unique. **This is the second time an attribute attaching to the
   wrong item has caused a silent defect here**; the first broke the debugger launch on
   2026-07-31.

**Out of scope:** parallelism. Measured and ruled out on 2026-07-29 — the slow tests serialise
on a global mutex regardless, and going parallel would save about two seconds.

---

### 2. Headless UI testing with `egui_kittest` — the big one — CAPABILITY LANDED 2026-08-01

**The item that attacks the actual bottleneck.** Doug is the sole verifier of 11,939 lines,
and his attention-per-expectation is the scarce resource
(`docs/ideas.md` #49).

`egui_kittest` is egui's official test harness. It renders headlessly and lets a test query
the widget tree by label and role, and simulate clicks. **`accesskit` is already in the lock
file** (pulled in by eframe), so the groundwork is present. Version must match egui 0.35 —
verify at implementation time rather than assuming.

**Dev-dependency only**; nothing ships in the binary. Approved by Doug 2026-08-01.

**What it should catch — every one of these was found by hand:**

| Bug found by running a lab | The assertion that replaces it |
|---|---|
| "the tree node is not highlighted" | assert highlight state after a `PointAtNode` link |
| "the RHS doesn't re-initialise on a second lab" | assert the stage panel is empty after a mode switch |
| "stop 4 works only if I click 1-3 first" | drive each link in isolation and assert |
| "the notice was invisible" | assert a notice widget exists and is not styled as the idle hint |

**Done when:** the harness is established, **the mechanical assertions from the five fixture
labs are automated**, and the convention for writing them is documented.

**Explicitly OUT of scope** — this is what stops the item ballooning:

- Comprehensive UI coverage. 11,939 lines is months; this is a capability plus the
  highest-value assertions.
- **Image snapshot testing.** Adds wgpu to the test path and asserts on pixels, which is
  brittle and answers a question nobody asked.
- **Anything requiring judgement.** Whether a layout reads clearly, whether an expectation is
  violable, whether a view *teaches* — those stay Doug's, and the fixture labs stay.

**The point is not to replace the labs.** It is to convert their *mechanical* half — did the
click do the thing? — so Doug's attention goes only where judgement is required.

#### Landed 2026-08-01 — the capability plus five assertions

`egui_kittest` 0.35 as a dev-dependency, **default features only** (`snapshot`/`wgpu` off, so
no GPU enters the test path). New module `src/ui_tests.rs`; 482 tests total, clippy clean.

**One production change made it possible.** `eframe::App::ui` takes an `eframe::Frame` that
cannot be constructed outside eframe, so no harness could call it. The body moved to
`App::frame_ui(&mut Ui)` and the trait method delegates — **the parameter was already
`_frame`, unused.** One unused parameter was the whole barrier between ~12,000 lines of UI and
an automated test.

| Test | Replaces a bug found by running |
|---|---|
| `the_harness_renders_hrw_and_sees_widgets` | the non-vacuity guard for all the others |
| `the_lab_picker_shows_every_fixture_and_no_readme` | pins the README exclusion **at the rendered layer** |
| `switching_labs_clears_the_stage_side_on_screen` | *"the RHS doesn't re-initialise on a second lab"* |
| `a_station_needing_a_specimen_is_refused_with_a_visible_notice` | *"the notice was invisible"* |
| `a_lab_link_acts_when_clicked_in_isolation` | *"stop 4 works only if I click 1-3 first"* |

**Two harness facts, each of which first produced a wrong diagnosis:**

1. **A widget laid out off-screen is queryable but not clickable.** At the 800x600 default,
   HRW's panels push the central content out of the viewport: `query_by_label` found the lab
   links, `click()` landed on nothing, and the test read as *"the feature is broken"*. Hence
   1600x1200. **If a click appears to do nothing, check the layout before the logic.**
2. **`Harness::run` cannot be used** — `tick_prewarm` requests a repaint every frame awaiting a
   debugger ack that never comes in a test, so `run` exhausts its budget and panics. Correct
   behaviour from a polling UI; `run_steps` is the tool.

**And one test was wrong before it was right.** The isolation test originally clicked a stage
link on a *fresh* app and asserted the stage changed — which would have **asserted a bug into
existence**, since HRW deliberately refuses a stage link with no specimen. Probing the actual
behaviour instead of trusting the premise turned that into the notice test, which is one of
the four the plan set out to write.

**What remains for this item:** more of the mechanical lab assertions, as labs are run and
their checkable halves become clear. The capability is the deliverable; the assertions
accumulate.

---

### 3. Move the run drivers into Rust — RESOLVED 2026-08-01 (split)

`scripts/measure-fidelity.ps1` and `scripts/promote-run.ps1` meet all three conditions of the tech-debt
trigger: re-run repeatedly, can fail silently, and have already produced defects only a human
caught.

**Largest effort, lowest urgency — and there is a real counter-argument.** Scripts are
editable without a rebuild, which mattered on 2026-08-01: the fidelity binary was locked by a
running sweep and fixing the watchdog in PowerShell disturbed nothing. Converting costs that.

**Process memory sampling needs a crate.** Ask before adding it — approval for `egui_kittest`
does not extend here.

**Done when:** the driver is a Rust binary with tests, *or* a recorded decision that the
mid-run editability is worth more than the verification.

**Checkpoint before starting this one.** After items 0-2 land, re-ask whether it still earns
the time — the trigger's own standard is evidence, and by then the evidence will be a month
newer.

#### The checkpoint's answer: they are not the same case

Asked 2026-08-01 with items 0, 0b, 0c, 1 and 2 landed. **One condition of the trigger genuinely
weakened**: "re-run repeatedly" was true while sweeps were daily, but the corpus is now green
and the suite runs after a rebase, before a PR, or when stage-JSON emission changes — a few
times a year. The trigger requires all three conditions, so that matters.

But the two scripts differ in exactly the ways the item's own caveats name:

| | `promote-run` (115 lines) | `measure-fidelity.ps1` (334 lines) |
|---|---|---|
| Needs a memory-sampling crate | **no** — zero sampling calls | **yes** → needs Doug's approval |
| Mid-run editability matters | no — runs for seconds, *after* the sweep | **yes** — the watchdog was fixed mid-run on 2026-08-01 while the binary was locked |
| Writes a **published claim** | **yes** — the sidecar's `not_checked` sentence | no |

**The published claim decided it.** That sentence travels to a maintainer *with* the table and
is read as fact, and this project's rule is *speed on actions, **care on records***. Both of
the item's counter-arguments — the crate and the editability — apply to the **other** script.

**Done, as "the driver is a Rust binary with tests":** `src/promote.rs` holds the two guards,
the `not_checked` sentence and the profile parser, with three tests; `examples/promote_run.rs`
is the driver. `scripts/promote-run.ps1` is deleted.

**Verified as a port, not just as code:** run against the real 2,614-model snapshot, the
sidecar it writes is identical to the PowerShell one field for field, and the `not_checked`
sentence is **byte-identical**. Two incidental improvements: `run_verdicts` is now sorted
(deterministic across runs, where insertion order was not) and the file no longer carries a
UTF-8 BOM, which had forced readers to use `utf-8-sig`.

**Done, as "a recorded decision", for the watchdog:** it stays in PowerShell. Mid-run
editability is worth more than the verification *for that script*, and converting it would
also require a dependency for one feature. Its recorded gap stands in item 0's audit.

## What success looks like, measurably

The metric from `docs/tech-debt.md`: **who caught it?**

> Before: every UI defect and every silent-tooling defect was caught by Doug.
> After: UI mechanics and observer silence are caught by `cargo test`, and Doug's attention
> goes to judgement — does this teach, does this read, is this expectation violable.

If, a month from now, Doug is still the one finding *"the tree node is not highlighted"*, this
plan did not work.
