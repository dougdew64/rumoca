# Plan — making the environment verify more, so Doug verifies less

**Purpose:** the items that make the toolchain catch what Doug currently catches by hand.
**Status:** **live plan.** Update it as items land; delete it when all five are complete and
their conventions have moved into `CLAUDE.md` and `tech-debt.md`.
**Read when:** picking up the pause, or when tempted to add a plan item — the ordering rule is
*what makes the rest cheaper*, not what is biggest.

**A deliberate pause in feature work**, agreed 2026-08-01. Oracle testing and Test mode resume
when this is done. *(Item 0b added 2026-08-01 during the document cleanup, which produced a
new class of evidence — see below.)*

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
> by Doug walking tours.

`docs/tech-debt.md`'s second trigger names the property: **verifiability, not Rust.** This plan
is that trigger's first application.

## The five items, in order

Ordered by *what makes the rest cheaper*, not by size.

---

### 0. The must-fire convention — adopt now, not a phase

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

---

### 0b. Make a claim of *absence* checkable — the stale-negative test

**The mirror of `doc_citations.rs`.** That test asserts every cited path **exists**. This one
asserts that everything a document claims **does not exist** still does not.

**The evidence, found 2026-08-01 while cleaning the documents** — a class the pause's original
four items do not cover, because nothing here is silent tooling or UI:

| Stale claim | Reality | How long |
|---|---|---|
| #42: frame position, pointed-at node, followed identifier are *"still below the reach of a link"* | all three shipped; **three fixture tours actively test them** | 2 days |
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
`SubView::from_slug` and the fixture-tour corpus, a Rust path against the source. The failure
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

---

### 1. Shorten the pre-commit suite (`docs/ideas.md` #48)

**First, because every later phase pays this cost repeatedly.** Six minutes is long enough
that Claude runs the full suite less often than it should, which is its own risk.

Memoise compiled specimens: **37 `compile_specimen_shared` call sites cover only 12 distinct
models**, and `Drivetrain` is compiled five or six times per run.

**Done when:** the full suite is meaningfully under six minutes with all 470+ tests still
passing, and #48's recorded caveat is honoured — **one test still compiles a specimen fresh
and compares against the memoised result**, so memoisation cannot hide a reproducibility
failure.

**Out of scope:** parallelism. Measured and ruled out on 2026-07-29 — the slow tests serialise
on a global mutex regardless, and going parallel would save about two seconds.

---

### 2. Headless UI testing with `egui_kittest` — the big one

**The item that attacks the actual bottleneck.** Doug is the sole verifier of 11,939 lines,
and his attention-per-expectation is the scarce resource
(`docs/ideas.md` #49).

`egui_kittest` is egui's official test harness. It renders headlessly and lets a test query
the widget tree by label and role, and simulate clicks. **`accesskit` is already in the lock
file** (pulled in by eframe), so the groundwork is present. Version must match egui 0.35 —
verify at implementation time rather than assuming.

**Dev-dependency only**; nothing ships in the binary. Approved by Doug 2026-08-01.

**What it should catch — every one of these was found by hand:**

| Bug found by walking a tour | The assertion that replaces it |
|---|---|
| "the tree node is not highlighted" | assert highlight state after a `PointAtNode` link |
| "the RHS doesn't re-initialise on a second tour" | assert the stage panel is empty after a mode switch |
| "stop 4 works only if I click 1-3 first" | drive each link in isolation and assert |
| "the notice was invisible" | assert a notice widget exists and is not styled as the idle hint |

**Done when:** the harness is established, **the mechanical assertions from the five fixture
tours are automated**, and the convention for writing them is documented.

**Explicitly OUT of scope** — this is what stops the item ballooning:

- Comprehensive UI coverage. 11,939 lines is months; this is a capability plus the
  highest-value assertions.
- **Image snapshot testing.** Adds wgpu to the test path and asserts on pixels, which is
  brittle and answers a question nobody asked.
- **Anything requiring judgement.** Whether a layout reads clearly, whether an expectation is
  violable, whether a view *teaches* — those stay Doug's, and the fixture tours stay.

**The point is not to replace the tours.** It is to convert their *mechanical* half — did the
click do the thing? — so Doug's attention goes only where judgement is required.

---

### 3. Move the run drivers into Rust

`measure-fidelity.ps1` and `promote-run.ps1` meet all three conditions of the tech-debt
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

## What success looks like, measurably

The metric from `docs/tech-debt.md`: **who caught it?**

> Before: every UI defect and every silent-tooling defect was caught by Doug.
> After: UI mechanics and observer silence are caught by `cargo test`, and Doug's attention
> goes to judgement — does this teach, does this read, is this expectation violable.

If, a month from now, Doug is still the one finding *"the tree node is not highlighted"*, this
plan did not work.
