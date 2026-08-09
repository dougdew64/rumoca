# Solve lowering — where names become numbers

**This is the last compilation phase, and the one where the model stops being a model.**

Everything before it worked on *equations about named quantities*. What a numerical integrator
wants is a function it can call with a pointer and a length. Solve lowering is the translation —
`h` stops being a height and becomes **`Y[0]`, byte offset 0**.

> Every number below is read from the specimens' generated traces.

---

## The problem this phase exists to solve

An integrator does not know what a capacitor is. Its interface is roughly:

```
given  y (a flat array of state values) and t
return dy/dt   — or a residual to drive to zero
```

To use one you must decide **which state is index 0**, where the parameters live, and how to
evaluate the right-hand side without looking anything up by name at runtime. **That decision is
this phase's output**, and once made it is frozen into the generated code.

---

## Act 1 — Two states, five parameters, and a starting vector

[BouncingBall → Solve Lowering](hrw://load/BouncingBall/SolveLowering)

**Expected:** `state_scalar_count` **2**, `algebraic_scalar_count` **0**, `parameter_count` **5**.

**Expected:** the solver's name map reads exactly `["h", "v"]`, with `h → 0` and `v → 1`.

Now the layout, which is the interesting part:

| name | lives in | index | byte offset |
|---|---|---|---|
| `h` | `Y` | 0 | 0 |
| `v` | `Y` | 1 | 8 |
| `g` | `P` | 0 | 0 |
| `e` | `P` | 1 | 8 |

**Expected:** `initial_y` is `[1.0, 0.0]` and the parameter vector begins `[9.81, 0.8, …]`.

**Read those two arrays.** `[1.0, 0.0]` is the ball starting one metre up, at rest. `9.81` is
gravity and `0.8` the restitution. **The whole model is now four numbers and a function** — and
the byte offsets say this is a memory layout, not a metaphor. Generated code will index into it.

**`algebraic_scalar_count` is 0**, which makes `BouncingBall` a pure ODE: every unknown is a
state, `der(y) = f(y, t)`, nothing to solve at each step.

---

## Act 2 — A model with no dynamics at all

[ProportionalLoop → Solve Lowering](hrw://load/ProportionalLoop/SolveLowering)

**Expected:** `state_scalar_count` **0**, `algebraic_scalar_count` **3**, names
`["error", "command", "measurement"]`.

**Zero states.** Nothing in this model integrates; there is no `der` anywhere. It is three
algebraic equations in three unknowns, and "simulating" it means solving that system at each
output point.

**This is why `blt-ordering.md` found one coupled block of exactly these three.** The BLT
structure and the solver layout are two views of the same fact, and they agree — 3 unknowns, one
irreducible block, zero states.

`TwoLoops` gives the same reading with **0 states and 4 algebraic**, split by BLT into two
independent 2×2 blocks.

---

## Act 3 — One state carrying twenty-two algebraic variables

[RcCircuit → Solve Lowering](hrw://load/RcCircuit/SolveLowering)

**Expected:** `state_scalar_count` **1**, `algebraic_scalar_count` **22**, `parameter_count`
**7**.

**Expected:** the name map begins `C.v`, then `src.v`, `R.T_heatPort`, `C.i`, `R.R_actual`, …

**One state, twenty-two algebraic.** And `blt-ordering.md` reported **23 blocks, none coupled**
for this model — 1 + 22 = 23. **The same decomposition, counted twice.**

**This is the shape most physical models have.** The integrator advances one number. Everything
else is recomputed from it by forward substitution in the BLT order, which is why Act 1 of the
BLT tour mattered: that order *is* the evaluation schedule this phase emits.

---

## Act 4 — Three problems, not one

Open the `problem` record and note that it has three siblings:

| section | what it is |
|---|---|
| `continuous` | the running system — `implicit_rhs`, `residual`, `derivative_rhs`, and an `algebraic_projection_plan` |
| `initialization` | the `t = 0` system — its own `residual`, its own `projection_plan` |
| `discrete` | event-time work — `runtime_assignment_rhs`, `pre_modes`, `observation_refresh` |

**The initial system is a genuinely different problem over the same variables**, which is what
[`initialization.md`](initialization.md) counted states and conditions for. Here you can see it
has its own residual function: at `t = 0` the solver runs *that* one, then switches to
`continuous` and never returns.

**And `discrete` is where the next tour lives.** `pre_modes` and `runtime_assignment_rhs` are
the machinery for `when` clauses — the code that runs at an event and not otherwise.

---

## Act 5 — Why the layout is frozen at compile time

The alternative is a runtime symbol table: look up `"h"`, get a value. **Every serious solver
refuses this**, for reasons worth naming:

- **A residual evaluation happens millions of times.** A hash lookup per variable per evaluation
  is the difference between a simulation finishing and not.
- **Sparsity is expressed in indices.** The Jacobian pattern the integrator exploits is a set of
  `(row, column)` pairs, and those columns *are* the `Y` indices decided here.
- **The layout is an ABI.** Byte offsets appear in the trace because generated code and the
  runtime must agree on memory, not merely on meaning.

**So this phase is where the pipeline's whole argument gets cashed.** Matching decided which
equation owns which variable; BLT decided the order; tearing decided what iterates. All three
were symbolic reasoning whose only purpose was to make *this* layout small and *this* residual
cheap.

---

## What comes next in the chain

Events — `when` clauses, zero crossings, and the `pre` values that let a model remember what it
was doing before an event fired. `BouncingBall`'s bounce is the smallest example, and it is the
`discrete` section above brought to life.

---

## What this tour cannot check

**Whether the `problem` record is readable in the UI at all.** It is a deeply nested structure —
`layout`, `solve_layout`, then three sub-problems — and everything above assumes the tree view
makes `state_scalar_count` and `solver_maps.names` findable. If it does not, this tour is
describing JSON rather than a view, and should say so.

**An unresolved inconsistency, stated rather than written around.** `Drivetrain` was the natural
Act 3 for this tour and is deliberately absent: its index reduction demotes nine states to three,
while its `state_scalar_count` reads **9**, and `GearWithBrake` shows the same gap (2 versus 7).
Its initialization is also structurally singular. **I do not know whether the reduction fails to
propagate or the count means something else**, so no specimen with reduced states appears here.
It is recorded in `docs/upstream-issues.md` and needs a System Modeler adjudication before anyone
calls it a bug.

**Whether Act 5 earns its place.** It argues *why* rather than showing *what*, and it is the
kind of section that reads as obvious to someone who has written a solver and as hand-waving to
someone who has not.
