# The pipeline — a week's run through the compiler

<!-- kind: hub -->

**Start here.** This is the map for the labs of the compiler phases, in the order
the compiler runs them.

The thesis, in one line: a Modelica model is a pile of equations with no order, no causality
and no idea which quantities are independent — and compiling it is six mathematical problems, each
with a named algorithm and a textbook behind it.

> Every lab below draws its numbers from the specimens' generated notebook traces.

---

## The one structural idea the whole pipeline turns on

A permutation.

Matching builds one — each equation paired with the unknown it solves. BLT permutes *again*, into
blocks, and the result is block lower triangular. Tearing splits each block and takes a Schur
complement. Index reduction discovers the state vector was the wrong size for the manifold the
system moves on.

All four are the same kind of operation: rearranging a matrix until its structure is obvious,
before any number exists. That is the sentence to hold onto if you are reading these alongside
a linear-algebra course — and the coarse form of matching-plus-SCCs has a name there, the
Dulmage-Mendelsohn decomposition.

---

## Three graphs, three classical questions

*(Added 2026-08-12, from a question on `connect-expansion.md`'s first paragraph. It lives here rather
than in any one lab because it spans four of them.)*

The word "graph" appears in three labs meaning three different things. They are easy to conflate and
the distinction is what makes each algorithm the obvious choice rather than an arbitrary one:

1 · the connection graph — `connect-expansion`

- vertices: connector variables
- edges: one edge per connector member, per `connect` — undirected
- asks: connected components · union-find

2 · the incidence graph — `matching`

- vertices: equations ∪ unknowns, bipartite
- edges: "this equation mentions this unknown"
- asks: maximum matching · augmenting paths

3 · the dependency graph — `blt-ordering`

- vertices: equations
- edges: derived from the matching — directed
- asks: *strongly* connected components · Tarjan

**Why the first and third need different algorithms.** Both ask *"which items belong
together?"* The difference is whether the lines have arrows.

**In the connection graph they don't.** `connect(R.n, C.p)` says R.n and C.p are the same
electrical node, and that is just as true read backwards. Belonging together is mutual, and it
chains: if R.n is joined to C.p and C.p to L.p, all three are one node, whatever order the
`connect`s were written in. **Union-find** is the algorithm for that: it keeps groups, and merges
two whenever a line joins them.

**In the dependency graph the lines are arrows.** After matching, each equation computes one
unknown; draw an arrow from A to B when B needs the value A computes:

```text
e1:  x = 1           needs nothing
e2:  y = 2*x         needs x  (from e1)
e3:  z = y + w       needs y  (from e2) and w (from e4)
e4:  w = z / 2       needs z  (from e3)
```

The arrows run e1 → e2 → e3, and e3 ⇄ e4. **The one-way arrows are information**: e1 → e2 says
e1 can be solved first, on its own. Equations belong together only when *each needs the other* —
here e3 and e4, which must be solved simultaneously. Such a group is a *strongly connected
component*, and **Tarjan's algorithm** finds them and returns them in an order where each comes
after everything it needs: `{e1} → {e2} → {e3, e4}`. That order *is* the evaluation order —
solve x, then y, then z and w together — and it is what `blt-ordering.md` builds.

**Union-find would get this wrong.** It ignores the arrows, so it would merge e1 with e2, e2 with
e3, and report all four as one group to solve at once: not false, but the order is gone, and
three easy steps have become one coupled solve.

---

## Three things worth knowing before you start

Structural singularity means two opposite things. `CapacitorLoop` is singular because the
model is wrong. `Drivetrain` is singular because its DAE has index > 1, which is ordinary and
curable. `matching.md` Act 3 shows the first; `index-reduction.md` Act 4 shows the second. A
rank deficiency is a question, not a verdict — probably the single most useful idea in the set.

Only one algorithm here is a heuristic. Matching, Tarjan and index reduction give answers
correct by construction. Tearing is greedy and can be beaten: a torn block always computes
the right numbers, and may iterate on more of them than it had to.

One lab needs a debugger and the bridge extension. `matching-live.md` alone — everything
else runs from HRW.

---

## The route

Each entry opens the lab in HRW. The file links are for reading outside it.

1. [connect-expansion](hrw://lab/connect-expansion) — what does `connect` mean?
2. [dae-construction](hrw://lab/dae-construction) — what is a well-posed system?
3. [matching](hrw://lab/matching) — which equation solves which unknown?
   - 3a · [matching-live](hrw://lab/matching-live) — *(debugger)* what does the
     search look like from inside?
4. [blt-ordering](hrw://lab/blt-ordering) — in what order can they be solved?
5. [tearing](hrw://lab/tearing) — how do you solve a block that has no order?
6. [index-reduction](hrw://lab/index-reduction) — which states are actually
   independent?
7. [initialization](hrw://lab/initialization) — where does it start?
8. [solve-lowering](hrw://lab/solve-lowering) — how does a name become a number?
9. [events](hrw://lab/events) — what happens at an instant?

Two orderings, and they differ. The list above is *pipeline* order. If you would rather follow
the mathematics than the machinery, run 3 → 4 → 5 first: matching, BLT and tearing are one
continuous argument on the same three-equation model, and they are the densest linear algebra in
the set.

---

## Two open questions you may hit

A state-count inconsistency. `Drivetrain`'s index reduction demotes nine states to three,
while solve lowering reports 9, and `GearWithBrake` shows the same gap. It is reproduced,
recorded in `docs/upstream-issues.md`, and not diagnosed — `solve-lowering.md` deliberately
omits its natural example because of it.

*(Resolved 2026-09-02, and it was never a Rumoca question.* `RcCircuit` reports one
`synthetic_root_conditions` while having no `when` clause because the field counts roots
Rumoca had to *synthesise* — here, one over the MSL Resistor's `R.T_heatPort`. HRW had
published that field as `zero_crossing_conditions`, which reads as a claim about all zero
crossings and is not one.*)*

---

## After the run — the four numbers that should agree

**Do not read this section before the labs.** Every number below is the answer to a
`**Predict.**` in two different labs, so reading it first replaces a prediction with a
recollection. *(It sat above the route until 2026-09-22, where it pre-announced the `Expected` of
at least four stations across four labs.)*

Afterwards it is the cross-check: each number appears in two labs and means the same thing in
both, and **checking that they agree is the fastest way to know a lab was read correctly.**

`RcCircuit` = 23 equations = 1 state + 22 algebraic. Flattening makes 23 (7 from the connect
graph, 16 constitutive); BLT finds 23 blocks, none coupled; solve lowering reports 1 state and 22
algebraic. Three labs, one decomposition.

`ProportionalLoop` = 3 equations, 0 states. Matching pairs all three; BLT puts them in *one*
irreducible block because nothing can be ordered; tearing cuts it to a 1×1 iteration; solve
lowering confirms zero states — a model with no dynamics at all.

`Drivetrain` = 97 → 20 → 3. 97 equations after flattening, 20 after index reduction, 3
surviving states. The middle number is why index reduction exists; the last is how many degrees
of freedom the machine really has.

`OverInitRc` = 1 state, 2 conditions, surplus +1. The only over-determined specimen, and it
differs from `RcCircuit` by two lines.

---

## What to tell me afterwards

The labs' closing sections each name two or three claims I could not check. Those are the
questions, and they are mostly about rendering — whether the Tarjan animation reads as a
search, whether the tearing frames show the greedy choice *as a choice*, whether Act 5 of
`initialization.md` is an interesting aside or a distraction.

And the counts should all be right. If one is not, that is the more valuable finding: it
means either a trace changed or I misread one, and both are worth knowing quickly.
