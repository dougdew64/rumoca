# Fixture lab — BLT: finding an order, and finding out there isn't one

<!-- kind: concept -->

**Giving every equation a job is not yet a recipe.** In `RcCircuit`, `f_x[9]` is Ohm's law,
`0 = R.v - R.R_actual*-src.i`, and the matching uses it to determine `src.i`. To evaluate it you
need `R.v` and `R.R_actual`, and those come from other equations. So the equations have
dependencies, and a dependency graph either lays out in a line or it does not.

If it does, the whole system is a sequence of direct assignments: compute this, then that, then
the next, with no iteration anywhere. If it does not, some group of equations is circular and has
to be solved simultaneously. This phase finds out which, and where.

Run [matching](hrw://lab/matching) first — it answers *which* equation solves *which* unknown, and
this lab asks *in what order*. Every count below is read from a generated trace, so if one
disagrees with your screen, the lab is wrong and I want to know.

---

## Station 1 — When an order exists

`RcCircuit` is 23 equations in 23 unknowns.

> **Predict.** How many groups will the compiler need to solve simultaneously?

[Look — RcCircuit → Structural → Spy plot](hrw://load/RcCircuit/Structural/SpyPlot)

**Expected:** 23 blocks, every one of size 1, and `coupled_block_count` is 0.

Falsified if: any block has size greater than 1.

*What just happened.* A block of size 1 is one equation determining one unknown from values
already known. Twenty-three of them in a row is forward substitution: each is evaluated once,
from values already known, and nothing iterates.

*(The run does not hand all 23 to the same place. Solve lowering's
`continuous.algebraic_projection_plan` holds **22** of them; the 23rd, `f_x[14]`
(`i = C*der(v)`), determines `der(C.v)` — the state's derivative — which is what the integrator is
given rather than an algebraic value it evaluates along the way.)*

On the spy plot this is the diagonal: every marked cell on or below it, nothing above. That shape
is why the phase is named as it is, and Station 4 comes back to it.

This is the best possible outcome, and it is worth knowing that a 23-equation circuit
achieves it. Wiring components together does not by itself create anything circular.

---

## Station 2 — When no order exists

`ProportionalLoop` is 3 equations in 3 unknowns, and you already know from `matching.md` that a
perfect matching exists.

> **Predict.** Given that every equation has an unknown assigned, how many blocks?

[Look — ProportionalLoop → Structural → Spy plot](hrw://load/ProportionalLoop/Structural/SpyPlot)

**Expected:** one block, coupled, of size 3 — containing all three equations and all
three unknowns.

Falsified if: three scalar blocks appear, or the block's size is other than 3.

*What just happened.* A matching does not imply an order. Each equation had a job and the jobs
could not be sequenced. Read them the way the matching assigned them: `f_x[1]` determines
`error` and needs `command`, `f_x[2]` determines `command` and needs `measurement`, and `f_x[0]`
determines `measurement` and needs `error`. Each needs the next, round the loop.

So the compiler stops trying to sequence and declares a simultaneous block: three equations
that must be solved together, which at run time means a numerical solve — Newton iteration for a
nonlinear block, one linear solve for a linear one.

The circularity is real, not an artifact. It came from closing a feedback loop with no
integrator in it. `LoopWithInertia` is the same loop with the inertia restored, and it is worth
comparing later: the loop survives, and it is re-solved at every time step.

---

## Station 3 — When the system splits

`TwoLoops` is 4 equations in 4 unknowns, written as two independent controller loops.

> **Predict.** One coupled block of 4, or something else?

[Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** two coupled blocks, each of size 2 — `{f_x[1], f_x[0]}` on
`{errorA, commandA}`, and `{f_x[3], f_x[2]}` on `{errorB, commandB}`.

Falsified if: one block of 4 appears.

*What just happened.* The decomposition is the point of the phase. These four equations are
not one 4×4 problem; they are two 2×2 problems, and the second depends on the first only through
`commandA`, which the first block produces.

So the solver does a 2×2 solve, then another 2×2 solve. Never a 4×4. On a large model that
difference is the difference between tractable and not — Newton's cost grows faster than linearly
in block size, so finding the *smallest* simultaneous groups is the whole game.

Notice also the ordering *between* blocks: loop A must go first. The phase produces both facts at
once — which equations are entangled, and what sequence the entangled groups go in.

---

## Station 4 — What you have been building is a block triangular form

The three shapes you have seen have one name between them.

> **Predict.** Look again at `RcCircuit`'s spy plot and `TwoLoops`'s. What do both have in common
> that a single 4×4 block would not?

[Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** in both, every marked cell lies on or below the diagonal — as blocks for
`TwoLoops`, as single cells for `RcCircuit`.

Falsified if: marks appear above the diagonal in either.

*What just happened.* Permute the rows and columns by the matching and the solve order, and the
incidence matrix becomes block lower triangular. That is the phase's output and its name:
BLT.

Reading it is mechanical once you know the shape. Each diagonal block is a group to solve
together; its size is how many equations at once; everything strictly below is a dependency
already computed. `RcCircuit` has 23 blocks of size 1, `ProportionalLoop` has one of size 3,
`TwoLoops` has two of size 2 — and the largest block is the honest measure of how hard the model
is to solve.

The algorithm is Tarjan's strongly-connected-components, on the graph whose vertices are
equations and whose edges follow the matching. An SCC *is* a simultaneous block. Rumoca draws
each edge from an equation to the equations it *needs* (`blt.rs`), and on that graph Tarjan's
output — reverse topological order — puts every dependency first, which gives *a* valid solve
order for free. Since Rumoca 0.11.0 solve lowering may then reorder the algebraic blocks by what
each one reads (`order_algebraic_projection_by_execution_reads`), so the run can execute a
different order — always one that still puts every dependency first. The animated stepper under Structural → Tarjan replays that search.

**The edges are arrows, and the arrows are the information.** After matching, each equation
computes one unknown. For reading, draw the arrows the way values flow — from A to B when B needs
the value A computes (the reverse of Rumoca's edges, so here the solve order is the arrows'
order):

```text
e1:  x = 1           needs nothing
e2:  y = 2*x         needs x  (from e1)
e3:  z = y + w       needs y  (from e2) and w (from e4)
e4:  w = z / 2       needs z  (from e3)
```

The arrows run e1 → e2 → e3, and e3 ⇄ e4. A one-way arrow says e1 can be solved first, on its
own. Equations belong together only when *each needs the other* — here e3 and e4. Tarjan returns
`{e1} → {e2} → {e3, e4}`, and that is a valid evaluation order: solve x, then y, then z and w
together. Any order the run uses must keep those three dependencies; it need not be this exact
list.

**This is why it is not union-find**, the algorithm [connect-expansion](hrw://lab/connect-expansion)
groups with. Union-find ignores direction, so it would merge e1 with e2, e2 with e3, and report
all four as one group: not false, but the order is gone, and three easy steps have become one
coupled solve. A `connect` has no direction, so there union-find loses nothing; here the
direction is the whole answer.

---

## What this lab cannot check

Whether the spy plot reads as triangular. It is custom-painted and unreachable by any
accessibility-tree test, so whether the orange coupled boxes and the diagonal are *visible* as the
shape Station 4 describes is your report alone.

Whether Station 3 lands as the important one. Decomposition matters more at scale than either
extreme, and this lab demonstrates it on four equations, where two 2×2 solves and one 4×4 feel
equally cheap.

Whether Tarjan is named too late. The algorithm arrives in Station 4 after three stations of its
output, which is deliberate — but a reader who wanted the mechanism first will have spent three
stations wondering.

---

## What comes next in the chain

A coupled block of 3 is not the end of the story: the compiler will try to make it smaller before
handing it to a numerical solver, by guessing one variable and computing the rest.

That is [tearing](hrw://lab/tearing).
