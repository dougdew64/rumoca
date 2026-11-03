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

## Why it is a separate specimen from `CapacitorLoop`

Both are flagged singular at Structural, so the *outcome* is identical. The **cause** is not, and
the cause is what a learner needs:

- `CapacitorLoop` — a genuine algebraic loop; the physics couples equations.
- **`TwiceDefined`** — no loop at all. An over-specified variable and an untouched one, which is
  an *authoring* mistake rather than a modelling structure.

Same pane, same word "singular", two different things to do about it. The incidence matrix is
where they separate: `TwiceDefined` has an **empty column** for `b`, visible at a glance.

## What to look at

The incidence matrix on the Structural tab. `b`'s column is empty — no equation mentions it — and
`a`'s has two entries. **The empty column is the defect**, and it is the cheapest possible
demonstration of what maximum matching is searching for.

## Verified

`cargo run -p hrw --example failure_map` — flagged at Structural and Index reduction, no `Failed`
stage. It reaches DAE construction successfully, which is the point.
