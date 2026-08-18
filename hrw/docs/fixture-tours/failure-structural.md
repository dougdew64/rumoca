# Failure tour — Structural analysis, where counting stops being enough

<!-- kind: failure -->

**Specimens:** `TwiceDefined` and `CapacitorLoop`. Both are flagged `singular`. **They are not
the same problem**, and telling them apart is the point of this tour.

**Walk `failure-flatten.md` first.** It shows the balance check passing or failing on a *count*.
This tour is about a system that passes the count and is still unsolvable.

---

## Stop 1 — Square, and singular anyway

[Load TwiceDefined → Dae](hrw://load/TwiceDefined/Dae)

**Expected:** DAE construction **succeeded**. The note reports equal equations and unknowns —
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

## Stop 2 — What matching finds

[Structural analysis](hrw://load/TwiceDefined/Structural)

**Expected:** flagged **`singular`**. The incidence matrix above the blocks shows
**`1/2 matched (rank deficiency 1)`**.

Look at the matrix itself. **`b`'s column is empty** — no equation mentions it. Both equations
mention only `a`, so maximum matching can pair at most one of them, and `b` is reachable from
nothing.

**Structural rank 1 < 2.** This is the cheapest possible demonstration of what matching is
searching for, and of why a count cannot answer it: the count is blind to *which* unknowns the
equations touch.

---

## Stop 3 — The same flag, a different cause

[Load CapacitorLoop → Structural](hrw://load/CapacitorLoop/Structural)

**Expected:** flagged `singular` here too — **the same word** — and again no BLT blocks.

But the incidence matrix looks nothing like `TwiceDefined`'s. **No column is empty.** Every
unknown is mentioned; the equations are genuinely coupled, and the coupling is the physics of two
capacitors in a loop.

| | `TwiceDefined` | `CapacitorLoop` |
|---|---|---|
| Cause | an authoring mistake | a modelling structure |
| Matrix | **an empty column** | fully populated |
| Fix | change the equations | index reduction, or accept the loop |

**Same pane, same word, two different things to do.** The matrix is where they separate, and it
separates them at a glance.

---

## Stop 4 — What "no blocks" means

Still on `CapacitorLoop`, look below the matrix.

**Expected:** the BLT area **states that no blocks were built**. It does not show blocks.

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
