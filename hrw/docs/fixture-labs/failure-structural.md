# Failure lab — Structural analysis, where counting stops being enough

<!-- kind: failure -->

**Specimens:** `TwiceDefined` and `CapacitorLoop`. Both are flagged `singular`. **They are not
the same problem, and telling them apart is the point of this lab.**

Run `failure-flatten.md` first. It shows the balance check passing or failing on a *count*.
This lab is about a system that passes the count and is still unsolvable.

---

## Station 1 — Square, and singular anyway

[Load TwiceDefined → Dae](hrw://load/TwiceDefined/Dae)

**Expected:** DAE construction succeeded. The note reports equal equations and unknowns —
the MLS §4.9 balance check passed.

```modelica
Real a;
Real b;
equation
  a = 1.0;
  a = time;
```

Two equations, two unknowns. Arithmetic says fine.

---

## Station 2 — What matching finds

[Structural analysis](hrw://load/TwiceDefined/Structural)

**Expected:** flagged `singular`. The incidence matrix above the blocks shows
`1/2 matched (rank deficiency 1)`.

Look at the matrix itself. `b`'s column is empty — no equation mentions it. And the second row
mentions nothing at all: the Structural tab analyses the system *after* the Simplify tab's
transforms, and constant folding put `a = 1` into `a = time`, so that row reads
`0 = 1 - time`. Only `f_x[0]` can be matched, and `b` is reachable from nothing.

Structural rank 1 < 2. This is the cheapest possible demonstration of what matching is
searching for, and of why a count cannot answer it: the count is blind to *which* unknowns the
equations touch.

---

## Station 3 — The same flag, a different cause

[Load CapacitorLoop → Structural](hrw://load/CapacitorLoop/Structural)

**Expected:** flagged `singular` here too — the same word — and again no BLT blocks.

But the incidence matrix looks nothing like `TwiceDefined`'s. No column is empty. Every
unknown is mentioned; the equations are genuinely coupled, and the coupling is the physics of a
capacitor directly across an ideal voltage source — the source fixes the voltage the capacitor's
state would otherwise integrate.

[Index reduction](hrw://load/CapacitorLoop/IndexReduction)

**Expected:** Index reduction is not red, and its note reads
`reducer sorted after 1 round(s); … integrates 0 state coordinate(s)`. Then open
[TwiceDefined's](hrw://load/TwiceDefined/IndexReduction): flagged, and its note begins
`preparation failed: DAE structural proof failed`.

| | `TwiceDefined` | `CapacitorLoop` |
|---|---|---|
| Cause | an authoring mistake | a modelling structure |
| Matrix | an empty column | fully populated |
| Next | the compile fails at Solve lowering | one reduction round repairs it |

The repair is the lesson of `index-reduction.md`: a single demotion turns the capacitor's
voltage from a state into an algebraic unknown, after which nothing is integrated and the model
simulates. Same pane, same word, two different outcomes. The matrix is where they separate, and
it separates them at a glance.

---

## Station 4 — What "no blocks" means

Still on `CapacitorLoop`, look below the matrix.

**Expected:** the BLT area states that no blocks were built. It does not show blocks.

Until 2026-08-04 it did — HRW computed a decomposition itself and drew it, for a system the
compiler had refused to decompose. The Tarjan animation drew a non-empty SCC run for a model that
produced none.

**That was the single worst defect this project has found**, and this specimen is the one that
exposed it. What you see now is the compiler's silence, reported as silence.

---

## What to bring back

- Is `singular` doing too much work as a word? Two specimens, two causes, one label.
- The empty column in `TwiceDefined` is obvious once pointed at. Should HRW point at it — mark
  unmatched columns, rather than leaving you to spot the gap?
