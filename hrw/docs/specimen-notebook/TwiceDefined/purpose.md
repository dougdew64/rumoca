# TwiceDefined — square by count, singular by structure

**Deliberately broken. Do not fix.**

Two equations, two unknowns, and **both equations mention only `a`**:

```modelica
Real a;
Real b;
equation
  a = 1.0;
  a = time;
```

## What it demonstrates

**Counting is not enough, which is the entire reason matching exists as a phase.**

The MLS §4.9 balance check passes — two equations, two unknowns — so DAE construction succeeds.
This is *not* `UnbalancedShaft`, which fails the count outright (2 equations, 3 unknowns).

Then maximum matching runs and can pair at most one equation with `a`. Nothing reaches `b`.
**Structural rank 1 < 2**, and the system is singular despite being square.

*What the Structural tab shows is sharper still* (2026-11-26: it shows the system solve lowering
analyses, after Simplify). `fold_constant_values` has carried `a = 1` into the second equation,
which becomes `0 = 1 - time` and mentions **no unknown at all**. So matching pairs `0 = a - 1`
with `a` and gives up on the second at once.

## Why it is a separate specimen from `CapacitorLoop`

Both are flagged singular at Structural, so the *flag* is identical. The **cause** is not, and
neither, now, is what happens next:

- `CapacitorLoop` — the physics pins the capacitor's state to the source. Index reduction repairs
  it (one Direct demotion), and the model simulates.
- **`TwiceDefined`** — no loop at all. An over-specified variable and an untouched one, which is
  an *authoring* mistake rather than a modelling structure. Nothing repairs it: the compile fails
  at Solve lowering.

Same pane, same word "singular", two different things to do about it. The incidence matrix is
where they separate: `TwiceDefined` has an **empty column** for `b`, visible at a glance.

## What to look at

The incidence matrix on the Structural tab. `b`'s column is empty — no equation mentions it — and
so is the second row, `0 = 1 - time`; `a`'s column has its one entry, in `0 = a - 1`. **The empty
column is the defect**, and it is the cheapest possible demonstration of what maximum matching is
searching for.

## Verified

The corpus outcome matrix (`the_corpus_outcome_matrix_is_unchanged`) reads `OOOOOOOFF.OX`:
flagged at Structural and Index reduction, no initialization plan, `Failed` at Solve lowering on
*structurally singular system: 1 matched out of 2 equations and 2 unknowns*. It reaches DAE
construction successfully, which is the point.
