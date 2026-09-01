# Fixture lab — Matching: which equation solves which unknown

<!-- kind: concept -->

[The chain overview](hrw://lab/the-concepts)

**A concept lab.** It teaches a step of the chain and uses HRW as the instrument. It is still
a test: every **Expected** line is violable.

Every count below was read from the committed traces under `docs/specimen-notebook/`, never
remembered.

---

## The problem this phase exists to solve

DAE construction handed the solver a **square** system: as many equations as unknowns. That is
necessary and it is not enough, because a square system does not come with instructions.

Nothing so far says *which* equation determines *which* unknown. And you cannot simply read it
off the page — Modelica equations are not assignments. `R.v - R.R_actual * R.i` relates three
quantities and privileges none of them; which one it is "for" depends on what the rest of the
model already determines.

**Matching is the phase that decides.** It pairs each equation with exactly one unknown, so that
no equation is used twice and no unknown is claimed twice. That pairing is what makes the next
phase — ordering — even askable.

Four stops: the easy case, the surprising case, the case with no answer, and what the answer is
called in the literature.

---

## Station 1 — The case where it is obvious

`BouncingBall` has two equations and two unknowns.

```
f_x[0]   0 = der(h) - v
f_x[1]   0 = der(v) - -g
```

> **Predict.** Which unknown does each equation get?

[Look — BouncingBall → Structural → Tree](hrw://load/BouncingBall/Structural/Tree)

[Point at `matching`](hrw://stage/Structural/Tree/node/matching)

**Expected:** `f_x[0]` → **`der(h)`**, and `f_x[1]` → **`der(v)`**.

**Falsified if:** either equation is matched to `h` or `v` undifferentiated.

*What just happened.* Nothing surprising, which is the point of starting here: each equation
contains exactly one unknown, so there is only one legal pairing. The unknowns are the
*derivatives*, as the DAE lab established — the integrator holds `h` and `v` and needs their
rates.

**A matching is a permutation, not a calculation.** Nothing was solved. All that happened is that
each equation was assigned a job.

---

## Station 2 — The case that is not obvious at all

`ProportionalLoop` is three equations in three unknowns:

```
f_x[0]   0 = error - (reference - measurement)
f_x[1]   0 = command - controllerGain * error
f_x[2]   0 = measurement - plantGain * command
```

Each one *looks* like it is "for" the variable written first: `f_x[0]` for `error`, `f_x[1]` for
`command`, `f_x[2]` for `measurement`.

> **Predict.** Will the matching agree with that reading? Write down your three pairs before
> looking.

[Look — ProportionalLoop → Structural → Tree](hrw://load/ProportionalLoop/Structural/Tree)

[Point at `matching`](hrw://stage/Structural/Tree/node/matching)

**Expected:** every pair is **shifted**:

| equation | matched unknown |
|---|---|
| `f_x[0]` | `measurement` |
| `f_x[1]` | `error` |
| `f_x[2]` | `command` |

**Falsified if:** `f_x[0]` is matched to `error`.

*What just happened.* **The left-hand variable is not the answer**, and this is the stop to
remember. `f_x[0]` was written as *"error is reference minus measurement"*, and the compiler used
it to determine `measurement`. Algebraically that is the same equation read backwards, which
Modelica permits because an equation is a relation, not an assignment.

**Why it had to shift.** The three equations form a cycle, so no assignment is more natural than
another — the matching algorithm found *a* legal perfect pairing, and a different implementation
could legitimately find a different one. What matters is that one exists.

That the assignment is arbitrary here, and forced in Station 1, is the difference between a system
that can be solved step by step and one that cannot. Which is the next lab.

---

## Station 3 — The case with no answer

`CapacitorLoop` is 14 equations in 14 unknowns. Square, like everything else so far.

> **Predict.** Will every equation get an unknown?

[Look — CapacitorLoop → Structural](hrw://load/CapacitorLoop/Structural)

**Expected:** **no.** The stage reports a failure:

```
structurally singular system: 13 matched out of 14 equations and 14 unknowns;
unmatched equations: f_x[13]; unmatched unknowns: gnd.p.i
```

with `rank_deficiency: 1`.

**Falsified if:** 14 of 14 are matched, or the deficiency is other than 1.

*What just happened.* **Square was never sufficient, and this is the proof the DAE lab promised
and did not show.** The counts agree — 14 and 14 — and there is still no way to assign the
equations without leaving one on the table.

Read the two leftovers together, because the pairing is the diagnosis:

- the unmatched equation is `f_x[13]`, `0 = C.n.v - gnd.p.v` — a *potential* equation from a
  connection;
- the unmatched unknown is `gnd.p.i` — a *flow*.

The model over-determines the voltages and under-determines one current: connecting the capacitor
in a loop made two potential equations say the same thing, and no equation is left to determine
the ground pin's current. **A rank deficiency of 1 means exactly one such pair.**

And note *where* this was caught: at matching, before anything was solved. Nothing numeric ran.

---

## Station 4 — What this is called, and why the name helps

The thing you have been looking at has a standard name, and knowing it opens the literature.

> **Predict.** The incidence matrix marks which unknowns appear in which equations. In those
> terms, what is a matching?

[Look — CapacitorLoop → Structural → Incidence](hrw://load/CapacitorLoop/Structural/Incidence)

**Expected:** a matrix of filled cells, with 13 of the 14 rows carrying a marked pairing.

**Falsified if:** every row is paired.

*What just happened.* The incidence matrix is the **bipartite graph** of equations against
unknowns: a filled cell is an edge. A matching is a *maximum matching* on that graph, and a
perfect one uses every row and every column exactly once.

So "structurally singular" means **no perfect matching exists**, and `rank_deficiency` is the
gap. That is a graph-theory result, not a numerical one — it depends only on *which* variables
appear, never on their values. A model can be structurally fine and still numerically singular,
which is a different failure and a different lab
([structural-vs-numerical-rank](hrw://lab/structural-vs-numerical-rank)).

The algorithm is **augmenting paths** — take an unmatched equation, run alternately along
unmatched and matched edges, and if you reach an unmatched unknown, flip the path to gain one
pair. [matching-live](hrw://lab/matching-live) steps that search in the debugger, where the call
stack *is* the path.

---

## What this lab cannot check

**Whether Station 2 reads as a surprise or as pedantry.** It is the load-bearing stop — the moment
"equations are not assignments" stops being a slogan — and it rests on you having written down
three pairs first. Skipping the prediction makes it a table of facts.

**Whether the incidence view in Station 4 is legible.** It is a custom-painted matrix that no
accessibility-tree test can reach, so whether 13 marked pairings out of 14 rows is *visible* as a
near-miss is your report and nothing else.

**Whether "a different implementation could match differently" invites the right question.** It
is true and it is unsettling; the lab states it and does not explore it.

---

## What comes next in the chain

Each equation now has a job. It still is not known **what order to do them in** — and in
`ProportionalLoop` no order exists at all, which is the discovery
[blt-ordering](hrw://lab/blt-ordering) is built on.

Or go back up: [The chain overview](hrw://lab/the-concepts)
