# ArrayFamily — eight scalar blocks reported as one

A `for` equation over an array, and nothing else:

```modelica
Real x[8](each start = 1.0);
equation
  for i in 1:8 loop
    der(x[i]) = -x[i];
  end for;
```

## What it demonstrates

**A regular equation family, and the BLT report's compaction of one.**

After flattening, this is eight independent scalar equations — `der(x[1]) = -x[1]`,
`der(x[2]) = -x[2]`, and so on. Each determines exactly one unknown, so maximum matching pairs
them one to one and Tarjan finds **eight separate blocks of size one**. There is no coupling
anywhere: the incidence matrix is diagonal.

Reporting that as eight lines would be honest and useless. For `x[8]` it is eight lines; for a
2048-element array it is 2048 lines that bury everything else in the dump. So v0.10.0 added
`BlockReport::StructuredScalar`: when the blocks form a **regular family** — one equation shape
replicated across a domain — the report emits *one* entry carrying the counts instead.

```text
[   0] family   8 scalar block(s) (8 point(s) x 1) <- f_x[0] (i)
```

`point_count = 8` is the domain size, `equations_per_point = 1` is how many scalar equations each
point contributes, and `first_row`/`row_count` locate the family in the incidence.

## Why this is a specimen rather than a note in a lab

**It is the only specimen that reaches the compacted shape**, and that gap had consequences.

`fidelity::check_f4` asserts the BLT blocks *partition* the equations — every row in exactly one
block. It resolves a block to rows through the equation labels, and a compacted family has no
per-row label to resolve. So from v0.10.0 until 2026-11-06 F4 reported every family as **"names no
equations"** while *also* counting the family's rows as holes in the partition: 24 false violations
across 20 MSL models, and — the worse half — unable to detect a real partition defect in any model
containing a family.

It survived two releases because **nothing in the curated corpus produced the shape.** Only the
hours-long MSL sweep did, and that had not run since before the rebase. With this specimen in
`fidelity::MODELS`, the same regression now fails the pre-commit suite in seconds.

## What to look at

The **Structural** tab. One block where you might expect eight, with its counts — then the
incidence matrix beside it, which is a clean diagonal. The contrast is the lesson: the *system* has
eight blocks, the *report* has one, and the compaction is a presentation choice rather than a
structural claim.

Compare `TwoLoops`, where the blocks are genuinely coupled and no compaction is possible.

## Verified

Every phase produces IR — the corpus outcome matrix row is `OOOOOOOOOOO`. The published block reads
`first_row: 0, row_count: 8` against an 8-equation incidence, so the partition is complete and F4 is
quiet. Renaming those two keys so F4 cannot read them reproduces both halves of the old defect
exactly, which is how the repair was checked rather than assumed.
