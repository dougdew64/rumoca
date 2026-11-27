# Fixture lab — Matching: which equation solves which unknown

<!-- kind: concept -->

**A Modelica equation is not an assignment.** `R.v - R.R_actual * R.i` relates three quantities
and privileges none of them; which one it is *for* depends on what the rest of the model already
determines. So you cannot read a system's recipe off the page.

That is why DAE construction handing the solver a square system — as many equations as unknowns —
is necessary and not enough. Nothing so far says *which* equation determines *which* unknown.
Matching is the phase that decides, pairing each equation with exactly one unknown so that no
equation is used twice and no unknown is claimed twice. That pairing is what makes the next
phase, ordering, askable at all.

Every count below is read from a generated trace, so if one disagrees with your screen, the lab is
wrong and I want to know.

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

**Expected:** `f_x[0]` → `der(h)`, and `f_x[1]` → `der(v)`.

Falsified if: either equation is matched to `h` or `v` undifferentiated.

*What just happened.* Nothing surprising, which is the point of starting here: each equation
contains exactly one unknown, so there is only one legal pairing. The unknowns are the
*derivatives*, as the DAE lab established — the integrator holds `h` and `v` and needs their
rates.

A matching is a permutation, not a calculation. Nothing was solved. All that happened is that
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

**Expected:** every pair is shifted:

| equation | matched unknown |
|---|---|
| `f_x[0]` | `measurement` |
| `f_x[1]` | `error` |
| `f_x[2]` | `command` |

Falsified if: `f_x[0]` is matched to `error`.

*What just happened.* The left-hand variable is not the answer, and this is the station to
remember. `f_x[0]` was written as *"error is reference minus measurement"*, and the compiler used
it to determine `measurement`. Algebraically that is the same equation read backwards, which
Modelica permits because an equation is a relation, not an assignment.

Why it had to shift. The three equations form a cycle, so no assignment is more natural than
another — the matching algorithm found *a* legal perfect pairing, and a different implementation
could legitimately find a different one. What matters is that one exists.

That the assignment is arbitrary here, and forced in Station 1, is the difference between a system
that can be solved step by step and one that cannot. Which is the next lab.

---

## Station 3 — The case with no answer

`CapacitorLoop` is 14 equations in 14 unknowns. Square, like everything else so far.

> **Predict.** Will every equation get an unknown?

[Look — CapacitorLoop → Structural](hrw://load/CapacitorLoop/Structural)

**Expected:** no. The stage is flagged `singular`, and its error reads
`structurally singular system: 13 matched out of 14 equations and 14 unknowns`. In the tree,
`error.unmatched_equation_ids` is `["f_x[6]"]`, `error.unmatched_unknowns` is `["gnd.p.i"]`,
and `error.rank_deficiency` is `1`.

[Point at the error](hrw://stage/Structural/Tree/node/error)

Falsified if: 14 of 14 are matched, or the deficiency is other than 1.

*What just happened.* Square was never sufficient, and this is the proof the DAE lab promised
and did not show. The counts agree — 14 and 14 — and there is still no way to assign the
equations without leaving one on the table.

Read the two leftovers together, because the pairing is the diagnosis:

- the unmatched equation is `f_x[6]`, `0 = C.v - (src.p.v - 0)` — the capacitor's own
  `v = p.v - n.v`. `C.v` is a state, so it is not an unknown here; the only unknown this row
  mentions is `src.p.v`;
- the unmatched unknown is `gnd.p.i` — a *flow*.

The model over-determines the voltages and under-determines one current.
`error.over_determined_block` names it: three equations — the source's `v = p.v - n.v` (`f_x[2]`), its `v = V` (`f_x[3]`) and
the capacitor's `f_x[6]` — all fix `src.v` and `src.p.v`, two unknowns between them, so one is
surplus. On the current side one unknown, `gnd.p.i`, is left with no equation of its own. A rank
deficiency of 1 means exactly one such pair.

*(The equation numbers are the Structural tab's, not the DAE tab's: Structural analyses the
system after Simplify, whose alias quotient eliminated eight variables here — `C.p.v` onto
`src.p.v`, for one — so the DAE tab's `f_x[13]` is a different row.)*

And note *where* this was found: at matching, before anything was solved. Nothing numeric ran.

**"Structurally singular" is a finding, not a verdict, and it can mean two opposite things.**
`CapacitorLoop` is not a broken model. A capacitor straight across a voltage source is ordinary
physics, and the leftover pair is the sign of a constraint the solver cannot use as written:
the capacitor's voltage is a state, and the source already fixes it. Index reduction repairs that
— one Direct-lane demotion, after which the model integrates no state at all
([index-reduction](hrw://lab/index-reduction)) — and the model then simulates.
Compare `TwiceDefined`, which really is broken:

```modelica
a = 1.0;
a = time;
```

Both equations define `a`, nothing defines `b`, and no later stage can fix that. By the time
Structural sees it, Simplify has folded `a = 1` into the second equation, which reads
`0 = 1 - time` — an equation with no unknown in it at all. Matching reports the two models the
same way. **You find out which kind you have at index reduction**, which either repairs the
system or reports *still singular after reduction*.

---

## Station 4 — What this is called, and why the name helps

The thing you have been looking at has a standard name, and knowing it opens the literature.

> **Predict.** The incidence matrix marks which unknowns appear in which equations. In those
> terms, what is a matching?

[Look — CapacitorLoop → Structural → Incidence](hrw://load/CapacitorLoop/Structural/Incidence)

**Expected:** a matrix of filled cells, with 13 of the 14 rows carrying a marked pairing.

Falsified if: every row is paired.

*What just happened.* The incidence matrix is the bipartite graph of equations against
unknowns: a filled cell is an edge. A matching is a *maximum matching* on that graph, and a
perfect one uses every row and every column exactly once. *(This is not the graph
[connect-expansion](hrw://lab/connect-expansion) merges sets on. That one joins connector
variables to each other; this one only ever joins an equation to an unknown, which is what
"bipartite" means.)*

So "structurally singular" means no perfect matching exists, and `rank_deficiency` is the
gap. That is a graph-theory result, not a numerical one — it depends only on *which* variables
appear, never on their values. A model can be structurally fine and still numerically singular,
which is a different failure and a different lab
([structural-vs-numerical-rank](hrw://lab/structural-vs-numerical-rank)).

The algorithm is augmenting paths — take an unmatched equation, run alternately along
unmatched and matched edges, and if you reach an unmatched unknown, flip the path to gain one
pair. [matching-live](hrw://lab/matching-live) steps that search in the debugger, where the
search's `stack` local *is* the path.

---

## What this lab cannot check

Whether Station 2 reads as a surprise or as pedantry. It is the load-bearing station — the moment
"equations are not assignments" stops being a slogan — and it rests on you having written down
three pairs first. Skipping the prediction makes it a table of facts.

Whether the incidence view in Station 4 is legible. It is a custom-painted matrix that no
accessibility-tree test can reach, so whether 13 marked pairings out of 14 rows is *visible* as a
near-miss is your report and nothing else.

Whether "a different implementation could match differently" invites the right question. It
is true and it is unsettling; the lab states it and does not explore it.

---

## What comes next in the chain

Each equation now has a job. It still is not known what order to do them in — and in
`ProportionalLoop` no order exists at all, which is the discovery
[blt-ordering](hrw://lab/blt-ordering) is built on.
