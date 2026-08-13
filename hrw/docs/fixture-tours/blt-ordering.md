# BLT — finding an order, and finding out there isn't one

**Walk [`matching.md`](matching.md) first.** Matching answered *which* equation solves *which*
unknown. It said nothing about **when** to solve them, and that is this tour.

Three models, three answers. `RcCircuit` can be solved one equation at a time, in an order.
`ProportionalLoop` cannot be solved one at a time at all. `TwoLoops` splits into two independent
pieces. **All three come out of the same algorithm**, and the algorithm is Tarjan's
strongly-connected-components search.

> Every count below is read from the specimens' generated notebook traces, not estimated.

---

## The problem this step exists to solve

A matching pairs equation `f_x[0]` with unknown `error`. That makes `error` the **output** of that
equation — so any *other* equation mentioning `error` now depends on it.

Those dependencies form a directed graph over the equations. If the graph has **no cycles**, a
topological order exists: solve `f_x[3]`, then `f_x[15]`, and so on, each equation needing only
values already computed. That is the cheapest possible thing a solver can do — one unknown at a
time, no iteration.

**If the graph has a cycle, no such order exists**, and the equations in that cycle have to be
solved *together* as a system. Finding the cycles is therefore the whole question, and a cycle in
a digraph is exactly a **strongly connected component**.

---

## Act 1 — When an order exists

[RcCircuit → Structural → Spy-plot](hrw://load/RcCircuit/Structural/SpyPlot)

A resistor, a capacitor, a voltage source and a ground. 23 equations, 23 unknowns.

**Expected:** the spy-plot shows **23 cells on the diagonal and no outlined boxes** — 23 blocks,
none coupled. *(The Summary view is not offered here: it exists to explain a singular system, and
this one is not. Hover any cell for that block's equation.)*

Twenty-three blocks for twenty-three equations means **every block holds exactly one equation**.
The system was fully ordered: there is a sequence in which each unknown can be computed from
values already known.

Read the first block and the last:

| block | equation | solves for |
|---|---|---|
| **0** | `f_x[3]` (equation from `src`) | `src.v` |
| **1** | `f_x[15]` (equation from `gnd`) | `gnd.p.v` |
| **2** | `f_x[22]` (connection: `src.n.v = gnd.p.v`) | `src.n.v` |
| … | | |
| **22** | `f_x[18]` (flow sum: `C.n.i + src.n.i + gnd.p.i = 0`) | `gnd.p.i` |

**Expected:** block 0 solves `src.v` and block 22 solves `gnd.p.i`.

**The order is not the equation order.** Block 0 is `f_x[3]`, block 1 is `f_x[15]`, block 2 is
`f_x[22]`. The compiler permuted them. `src.v` comes first because the voltage source's value
depends on nothing — it is a known constant — and `gnd.p.i` comes last because the ground's
current is whatever is left over once everything else is determined.

**That permutation is the output of this phase.** It is not cosmetic; it is the evaluation order
the generated code will run in.

---

## Act 2 — When no order exists

[ProportionalLoop → Structural → Spy-plot](hrw://load/ProportionalLoop/Structural/SpyPlot)

Three equations: `error = reference - measurement`, `command = controllerGain * error`,
`measurement = plantGain * command`.

**Expected:** the spy-plot shows **one outlined box covering the whole 3×3** — 1 block, and it is
coupled. Contrast Act 1's twenty-three separate cells: same picture language, opposite verdict.

One block for three equations. **Nothing was ordered at all.**

**Expected:** that block has size **3**, and lists unknowns `command`, `error`, `measurement` —
every unknown in the model.

Follow the dependency by hand and you will see why: `error` needs `measurement`, `measurement`
needs `command`, `command` needs `error`. A cycle of length three. There is no equation you can
solve first, because each one needs a value only another can produce.

**So the solver must treat all three as one simultaneous system** — and that is a categorically
more expensive thing than Act 1's 23 direct assignments. Act 1 evaluates; Act 2 must *solve*.

[Watch the search find it](hrw://load/ProportionalLoop/Structural/TarjanAnim)

**Expected:** the animation ends with all three equation nodes in a single component.

---

## Act 3 — When the system splits

[TwoLoops → Structural → Summary](hrw://load/TwoLoops/Structural/TarjanAnim)

Two independent proportional loops in one model — an `A` loop and a `B` loop, sharing nothing.

**Expected:** **2 blocks**, both **coupled**, each of size **2**.

| block | unknowns |
|---|---|
| 0 | `errorA`, `commandA` |
| 1 | `errorB`, `commandB` |

**Expected:** no block mixes an `A` unknown with a `B` one.

**This is the result worth pausing on.** Four equations that a naive solver would throw at one
4×4 nonlinear solve are actually **two independent 2×2 problems**. Solving two 2×2 systems is
dramatically cheaper than one 4×4 — and it parallelises, which a single 4×4 does not.

**Tarjan found that decomposition without being told the model had two loops in it.** Nothing in
the Modelica source says "these are separable"; it falls out of the graph.

---

## Act 4 — What you have been building is a block triangular form

Act 1 gave a permutation with 23 blocks of size 1. Act 2 gave one block of size 3. Act 3 gave two
blocks of size 2. **These are the same object at different extremes.**

Take the incidence matrix, apply the matching's permutation to the columns, and apply the block
order to the rows. The result is **block lower triangular** — everything above the diagonal
blocks is zero:

```
Act 1 (RcCircuit)        Act 2 (ProportionalLoop)     Act 3 (TwoLoops)
■ . . . .                ┌─────┐                      ┌───┐ . .
■ ■ . . .                │ ■ ■ ■│                     │■ ■│ . .
■ ■ ■ . .                │ ■ ■ ■│                     └───┘ . .
■ ■ ■ ■ .                │ ■ ■ ■│                     . . ┌───┐
■ ■ ■ ■ ■                └─────┘                      . . │■ ■│
                                                      . . └───┘
23 blocks of size 1      1 block of size 3            2 blocks of size 2
fully triangular         irreducible                  block diagonal
```

**Every diagonal block is irreducible** — it cannot be split further, which is precisely what
"strongly connected" means. A size-1 block is the ideal case: solve by substitution. A size-*n*
block is an *n*×*n* system that must be solved as a unit.

**The zeros above the diagonal are the whole payoff.** They say that when you reach block *k*,
every value it needs has already been computed. That is what makes forward substitution valid,
and it is the same structure that makes a triangular matrix cheap to solve in linear algebra —
here obtained by permutation rather than by elimination.

**The name.** BLT is *Block Lower Triangular*. In the numerical-linear-algebra literature the same
decomposition is the **Dulmage-Mendelsohn decomposition**, and the coarse version is exactly
"matching, then SCCs of the resulting digraph" — the two phases you have now walked.

---

## Act 5 — How Rumoca spells it

`crates/rumoca-phase-structural/src/tarjan.rs`, and it is **Tarjan's algorithm**, published 1972.

**One depth-first pass.** Each node gets an index when first visited and a *lowlink* — the
smallest index reachable from its subtree. When a node's lowlink equals its own index, that node
roots a strongly connected component, and the component is everything above it on a stack.

**Two properties matter here, and both are why this algorithm rather than another:**

- **It is O(V + E)** — one pass, linear in the graph. For a model with thousands of equations
  that is the difference between practical and not.
- **It emits components in reverse topological order.** So the SCCs come out *already sorted* —
  Tarjan does not find the cycles and then sort them; the order is a side effect of the DFS.
  **Act 1's block order is free.**

[The search itself, on the coupled case](hrw://load/ProportionalLoop/Structural/TarjanAnim)

**Expected:** the animation's frames are the DFS — visiting a node, following an edge, and
popping a completed component.

**The dependency graph is built from the matching**, which is why this tour required the previous
one. Equation A points at equation B when A mentions a variable that B was matched to. Change the
matching and you change the graph; that is why `matching.md` insisted on the sort that makes the
matching deterministic.

---

## What comes next in the chain

A coupled block of size 3 says *"solve these together"*, and says nothing about **how**.
Act 2's block is a 3×3 nonlinear system, and solving it directly means a 3×3 Jacobian at every
iteration.

**Tearing** asks whether you can guess one variable, solve the rest by substitution, and be left
with a single residual to drive to zero — turning a 3×3 solve into a 1×1 one. That is a Schur
complement, it is the next tour, and `NonlinearLoop` and `TwoLoops` both carry torn blocks to
look at.

---

## What this tour cannot check

**Whether the Tarjan animation reads as a search.** The matching animation draws on a matrix,
where the moving highlight has an obvious meaning. This one draws a node graph, and whether the
DFS is legible as *descending and backing up* — or just as dots changing colour — is the half no
test reaches.

**Whether Act 4's ASCII matrices help or patronise.** They are the one place this tour draws
rather than points, and a reader who already sees the permutation may find them noise.

**Whether Act 3 lands as remarkable.** "Tarjan found two independent loops without being told" is
the strongest claim here. It may read as obvious in a model whose name is `TwoLoops` — the
decomposition is impressive precisely when nobody knew it was there, which a four-equation
example cannot demonstrate.
