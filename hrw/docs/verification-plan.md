# Plan — making the environment verify more, so Doug verifies less

**A deliberate pause in feature work**, agreed 2026-08-01. Oracle testing and Test mode resume
when this is done. **This is a live plan: update it as items land, delete it when all four are
complete** and their conventions have moved into `CLAUDE.md` and `tech-debt.md`.

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

## The four items, in order

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
