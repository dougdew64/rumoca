# UnclosedModel — the only hard failure in the pipeline

**Deliberately broken. Do not fix.**

Ten lines of valid Modelica with the final `end UnclosedModel;` removed.

## What it demonstrates

**Parse is the one phase that genuinely stops.** Everything downstream of a failed parse is
unreachable, so this specimen shows a stage bundle at its emptiest: **nine of the eleven stages
have nothing at all**, and each says so rather than rendering blank.

That makes it the reference case for *"absence is stated, never filled"* — the rule that came
out of the 2026-08-04 accuracy work. If a pane ever shows content for this model, the content
was invented.

## What it taught us, which was not what it was written for

Written for `docs/ideas.md` #46 as the Parse entry in a failure specimen per phase. Building the
set and running `cargo run -p hrw --example failure_map` produced a finding that reframed the
whole idea:

> **Rumoca is a recovering compiler.** Almost nothing *fails*. Resolve errors, typecheck
> diagnostics, structural singularity and over-determined initialization are all **flagged** —
> recorded, with the artifact still produced and downstream phases still running.

`DimensionMismatch` does not stop at Typecheck; it is flagged there. `UndefinedRef` does not stop
at Resolve; it is flagged there and stops later, at Flatten. **Parse is the exception**, and this
specimen is what makes the exception visible.

## The distinction it anchors

| Outcome | Meaning | Example |
|---|---|---|
| `Failed` | stopped here, no artifact | **UnclosedModel** at Parse |
| `Flagged` | problem recorded, work continued | `Drivetrain` singular at Structural, then reduced |
| `Ok` | nothing to report | `SingleInertia` throughout |

**`Flagged` is not a lesser `Failed`.** Every model needing index reduction is flagged singular
at Structural and then fixed — `Drivetrain`, `MotorWithBrake` and `BenchActuator` all are.
Reading a flag as a failure would call four healthy specimens broken, which is exactly what the
first version of `failure_map` did.

## Verified

`cargo run -p hrw --example failure_map` — `UnclosedModel` is the only specimen reporting
`Failed` at Parse, and it reports `Failed` at nine stages in total.
