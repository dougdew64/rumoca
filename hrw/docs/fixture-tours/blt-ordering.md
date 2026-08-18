# Fixture tour — BLT: finding an order, and finding out there isn't one

<!-- kind: concept -->

[▲ The chain overview](hrw://tour/the-concepts)

**A concept tour.** Walk [`matching.md`](matching.md) first — it answers *which* equation
solves *which* unknown, and this tour asks *in what order*.

Every count below was read from the committed traces, never remembered.

---

## The problem this phase exists to solve

Matching gave every equation a job. That is still not a recipe.

To evaluate `f_x[9]` — Ohm's law, which determines `R.v` — you need `R.R_actual` and `R.i`, and
those come from other equations. So the equations have **dependencies**, and a dependency graph
either can be laid out in a line or it cannot.

If it can, the whole system is a sequence of direct assignments: compute this, then that, then the
next. No iteration anywhere. If it cannot, some group of equations is circular and has to be
solved **simultaneously**.

**This phase finds out which, and where.** Three stops: a system that orders completely, one that
does not order at all, and one that splits into independent pieces.

---

## Stop 1 — When an order exists

`RcCircuit` is 23 equations in 23 unknowns.

> **Predict.** How many groups will the compiler need to solve simultaneously?

[▶ Look — RcCircuit → Structural → Spy plot](hrw://load/RcCircuit/Structural/SpyPlot)

**Expected:** **23 blocks, every one of size 1**, and `coupled_block_count` is **0**.

**Falsified if:** any block has size greater than 1.

*What just happened.* A block of size 1 is one equation determining one unknown from values
already known. Twenty-three of them in a row is **forward substitution** — the solver walks the
list once, evaluating, and never iterates.

On the spy plot this is the diagonal: every marked cell on or below it, nothing above. That shape
is why the phase is named as it is, and Stop 4 comes back to it.

**This is the best possible outcome**, and it is worth knowing that a 23-equation circuit
achieves it. Wiring components together does not by itself create anything circular.

---

## Stop 2 — When no order exists

`ProportionalLoop` is 3 equations in 3 unknowns, and you already know from `matching.md` that a
perfect matching exists.

> **Predict.** Given that every equation has an unknown assigned, how many blocks?

[▶ Look — ProportionalLoop → Structural → Spy plot](hrw://load/ProportionalLoop/Structural/SpyPlot)

**Expected:** **one** block, **coupled**, of size **3** — containing all three equations and all
three unknowns.

**Falsified if:** three scalar blocks appear, or the block's size is other than 3.

*What just happened.* **A matching does not imply an order.** Each equation had a job and the jobs
could not be sequenced, because `command` needs `error`, `error` needs `measurement`, and
`measurement` needs `command`.

So the compiler stops trying to sequence and declares a **simultaneous block**: three equations
that must be solved together, which at run time means a numerical solve — Newton iteration for a
nonlinear block, one linear solve for a linear one.

**The circularity is real, not an artifact.** It came from closing a feedback loop with no
integrator in it. `LoopWithInertia` is the same loop with the inertia restored, and it is worth
comparing later: the loop survives, and it is re-solved at every time step.

---

## Stop 3 — When the system splits

`TwoLoops` is 4 equations in 4 unknowns, written as two independent controller loops.

> **Predict.** One coupled block of 4, or something else?

[▶ Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** **two** coupled blocks, each of size **2** — `{f_x[1], f_x[0]}` on
`{errorA, commandA}`, and `{f_x[3], f_x[2]}` on `{errorB, commandB}`.

**Falsified if:** one block of 4 appears.

*What just happened.* **The decomposition is the point of the phase.** These four equations are
not one 4×4 problem; they are two 2×2 problems, and the second depends on the first only through
`commandA`, which the first block produces.

So the solver does a 2×2 solve, then another 2×2 solve. Never a 4×4. On a large model that
difference is the difference between tractable and not — Newton's cost grows faster than linearly
in block size, so finding the *smallest* simultaneous groups is the whole game.

Notice also the ordering *between* blocks: loop A must go first. The phase produces both facts at
once — which equations are entangled, and what sequence the entangled groups go in.

---

## Stop 4 — What you have been building is a block triangular form

The three shapes you have seen have one name between them.

> **Predict.** Look again at `RcCircuit`'s spy plot and `TwoLoops`'s. What do both have in common
> that a single 4×4 block would not?

[▶ Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** in both, every marked cell lies **on or below the diagonal** — as blocks for
`TwoLoops`, as single cells for `RcCircuit`.

**Falsified if:** marks appear above the diagonal in either.

*What just happened.* Permute the rows and columns by the matching and the solve order, and the
incidence matrix becomes **block lower triangular**. That is the phase's output and its name:
**BLT**.

Reading it is mechanical once you know the shape. Each diagonal block is a group to solve
together; its size is how many equations at once; everything strictly below is a dependency
already computed. `RcCircuit` has 23 blocks of size 1, `ProportionalLoop` has one of size 3,
`TwoLoops` has two of size 2 — and the largest block is the honest measure of how hard the model
is to solve.

**The algorithm is Tarjan's strongly-connected-components**, on the graph whose vertices are
equations and whose edges follow the matching. An SCC *is* a simultaneous block, and Tarjan
returns them in reverse topological order, which is the solve order for free. The animated
stepper under **Structural → Tarjan** replays that search.

---

## What this tour cannot check

**Whether the spy plot reads as triangular.** It is custom-painted and unreachable by any
accessibility-tree test, so whether the orange coupled boxes and the diagonal are *visible* as the
shape Stop 4 describes is your report alone.

**Whether Stop 3 lands as the important one.** Decomposition matters more at scale than either
extreme, and this tour demonstrates it on four equations, where two 2×2 solves and one 4×4 feel
equally cheap.

**Whether Tarjan is named too late.** The algorithm arrives in Stop 4 after three stops of its
output, which is deliberate — but a reader who wanted the mechanism first will have spent three
stops wondering.

---

## What comes next in the chain

A coupled block of 3 is not the end of the story: the compiler will try to make it smaller before
handing it to a numerical solver, by guessing one variable and computing the rest.

That is [`tearing.md`](tearing.md).

Or go back up: [▲ The chain overview](hrw://tour/the-concepts)
