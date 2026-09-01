# The concepts — a week's walk through the pipeline

<!-- kind: hub -->

**Start here.** This is the map for the labs of the compiler phases, in the order
the compiler runs them.

**The thesis, in one line:** a Modelica model is a pile of equations with no order, no causality
and no idea which quantities are independent — and compiling it is six mathematical problems, each
with a named algorithm and a textbook behind it.

> Every lab below draws its numbers from the specimens' generated notebook traces.

---

## The route

**Each entry opens the lab in HRW.** The file links are for reading outside it.

1. [connect-expansion](hrw://lab/connect-expansion) — what does `connect` mean?
2. [dae-construction](hrw://lab/dae-construction) — what is a well-posed system?
3. [matching](hrw://lab/matching) — which equation solves which unknown?
   - **3a ·** [matching-live](hrw://lab/matching-live) — *(debugger)* what does the
     search look like from inside?
4. [blt-ordering](hrw://lab/blt-ordering) — in what order can they be solved?
5. [tearing](hrw://lab/tearing) — how do you solve a block that has no order?
6. [index-reduction](hrw://lab/index-reduction) — which states are actually
   independent?
7. [initialization](hrw://lab/initialization) — where does it start?
8. [solve-lowering](hrw://lab/solve-lowering) — how does a name become a number?
9. [events](hrw://lab/events) — what happens at an instant?

**Two orderings, and they differ.** The list above is *pipeline* order. If you would rather follow
the mathematics than the machinery, walk **3 → 4 → 5** first: matching, BLT and tearing are one
continuous argument on the same three-equation model, and they are the densest linear algebra in
the set.

---

## The four numbers that connect the labs

If you carry nothing else, carry these. Each appears in two labs and means the same thing in
both — checking that they agree is the fastest way to know a lab was read correctly.

**`RcCircuit` = 23 equations = 1 state + 22 algebraic.** Flattening makes 23 (7 from the connect
graph, 16 constitutive); BLT finds 23 blocks, none coupled; solve lowering reports 1 state and 22
algebraic. **Three labs, one decomposition.**

**`ProportionalLoop` = 3 equations, 0 states.** Matching pairs all three; BLT puts them in *one*
irreducible block because nothing can be ordered; tearing cuts it to a 1×1 iteration; solve
lowering confirms zero states — a model with no dynamics at all.

**`Drivetrain` = 97 → 20 → 3.** 97 equations after flattening, 20 after index reduction, 3
surviving states. The middle number is why index reduction exists; the last is how many degrees
of freedom the machine really has.

**`OverInitRc` = 1 state, 2 conditions, surplus +1.** The only over-determined specimen, and it
differs from `RcCircuit` by two lines.

---

## The one structural idea the whole pipeline turns on

**A permutation.**

Matching builds one — each equation paired with the unknown it solves. BLT permutes *again*, into
blocks, and the result is block lower triangular. Tearing splits each block and takes a Schur
complement. Index reduction discovers the state vector was the wrong size for the manifold the
system moves on.

**All four are the same kind of operation: rearranging a matrix until its structure is obvious,
before any number exists.** That is the sentence to hold onto if you are reading these alongside
a linear-algebra course — and the coarse form of matching-plus-SCCs has a name there, the
**Dulmage-Mendelsohn decomposition**.

---

## Three graphs, three classical questions

*(Added 2026-08-12, from a question on `connect-expansion.md`'s first paragraph. It lives here rather
than in any one lab because it spans four of them.)*

The word "graph" appears in three labs meaning three different things. They are easy to conflate and
the distinction is what makes each algorithm the obvious choice rather than an arbitrary one:

**1 · the connection graph** — `connect-expansion`

- **vertices:** connector variables
- **edges:** one edge per connector member, per `connect` — **undirected**
- **asks:** connected components · **union-find**

**2 · the incidence graph** — `matching`

- **vertices:** equations ∪ unknowns, **bipartite**
- **edges:** "this equation mentions this unknown"
- **asks:** maximum matching · **augmenting paths**

**3 · the dependency graph** — `blt-ordering`

- **vertices:** equations
- **edges:** derived from the matching — **directed**
- **asks:** *strongly* connected components · **Tarjan**

**The contrast worth carrying: the first and third ask the same question on different graphs.**
Both are *"find the components"* — and the only difference is **undirected versus directed**.
That is exactly why one is union-find and the other is Tarjan: union-find merges symmetric
relations and cannot express a dependency that runs one way, which is the whole content of an
evaluation order.

So when Tarjan arrives in `blt-ordering.md` it is not a new idea. It is this idea, on a digraph,
where "same node" has become "each needs the other".

---

## Three things worth knowing before you start

**Structural singularity means two opposite things.** `CapacitorLoop` is singular because the
model is wrong. `Drivetrain` is singular because its DAE has index > 1, which is ordinary and
curable. `matching.md` Act 3 shows the first; `index-reduction.md` Act 4 shows the second. **A
rank deficiency is a question, not a verdict** — probably the single most useful idea in the set.

**Only one algorithm here is a heuristic.** Matching, Tarjan and index reduction give answers
correct by construction. **Tearing is greedy** and can be beaten: a torn block always computes
the right numbers, and may iterate on more of them than it had to.

**One lab needs a debugger and the bridge extension.** `matching-live.md` alone — everything
else runs from HRW.

---

## Two open questions you may hit

**A state-count inconsistency.** `Drivetrain`'s index reduction demotes nine states to three,
while solve lowering reports **9**, and `GearWithBrake` shows the same gap. It is reproduced,
recorded in `docs/upstream-issues.md`, and **not diagnosed** — `solve-lowering.md` deliberately
omits its natural example because of it.

**An unexplained event count.** `RcCircuit` reports one `zero_crossing_condition` while having no
`when` clause at all. `events.md` Act 1 quotes only the four counts I can account for.

---

## What to tell me afterwards

The labs' closing sections each name two or three claims I could not check. **Those are the
questions, and they are mostly about rendering** — whether the Tarjan animation reads as a
search, whether the tearing frames show the greedy choice *as a choice*, whether Act 5 of
`initialization.md` is an interesting aside or a distraction.

**And the counts should all be right.** If one is not, that is the more valuable finding: it
means either a trace changed or I misread one, and both are worth knowing quickly.
