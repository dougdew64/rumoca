# UnclosedModel — the only hard failure in the pipeline

**Deliberately broken. Do not fix.**

Ten lines of valid Modelica with the final `end UnclosedModel;` removed.

## What it demonstrates

**Parse is the earliest stop.** Everything downstream of a failed parse is unreachable, so this
specimen shows a stage bundle at its emptiest: **all eleven compilation stages after Parse have
nothing at all**, and each says so rather than rendering blank.

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

**That finding no longer holds, and the correction is the lesson.** It was HRW painting stops as
flags. On the current build (the corpus outcome matrix, `the_corpus_outcome_matrix_is_unchanged`)
`UndefinedRef` *fails* at Resolve, `DimensionMismatch`'s flagged Typecheck stops the compile (every
later tab: *not reached (Typecheck failed earlier)*), and an unbalanced count fails DAE
construction. What *is* flagged and carried on is a singularity index reduction repairs. Parse is
simply the earliest stop, and this specimen is what makes it visible.

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

The corpus outcome matrix row is `X...........`: `UnclosedModel` is the only specimen reporting
`Failed` at Parse, it reports `Failed` there and nowhere else, and the eleven stages after it are
not reached.
