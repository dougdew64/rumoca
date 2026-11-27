# The pipeline — a week's run through the compiler

<!-- kind: hub -->

**Start here.** This is the map for the labs of the compiler phases, in the order
the compiler runs them.

> Every lab below draws its numbers from the specimens' generated notebook traces.

---

## One model, through every tab

Rumoca gets from your text to a running simulation in two big steps:

- **Compile** turns the text into a DAE, the model as one system of equations. HRW shows its
  phases as tabs 1–6.
- **Solve lowering** turns the DAE into programs a solver can run. It first simplifies the
  DAE (tab 7), then does the structural work — matching and ordering (tab 8), index
  reduction and choosing which variables to integrate (tab 9), the starting values (tab
  10) — and produces the programs (tab 12). All of those tabs show one run of it, the same
  run the simulation uses.

Tab 11 is not a step of its own, and tab 13 runs the result. The quickest way to see the whole
pipeline is to send one small model through every tab.

[Load HarmonicOscillator](hrw://load/HarmonicOscillator/Parse), then click each tab in turn:

```modelica
model HarmonicOscillator
  parameter Real omega = 1.0;
  Real x(start = 1.0) "position";
  Real v(start = 0.0) "velocity";
equation
  der(x) = v;
  der(v) = -omega ^ 2 * x;
end HarmonicOscillator;
```

This model is so simple that most stages have little to do. That is useful: you see what each
stage *produces* without the clutter, and each entry names the lab whose model makes that stage
work hard.

**1 · [Parse](hrw://load/HarmonicOscillator/Parse)** reads the text into a syntax tree: one
model, three declarations, two equations. Nothing is understood yet, only structured.

**2 · [Resolve](hrw://load/HarmonicOscillator/Resolve)** connects every name to its
declaration. The `omega` inside `der(v) = -omega ^ 2 * x` now points at `parameter Real omega`.

**3 · [Instantiate](hrw://load/HarmonicOscillator/Instantiate)** builds the model's component
tree with modifiers applied. This model has no components, so the tree is just its three
variables.

**4 · [Typecheck](hrw://load/HarmonicOscillator/Typecheck)** checks that every expression's type
and size fit. Here everything is a scalar `Real`, so there is nothing to report.

**5 · [Flatten](hrw://load/HarmonicOscillator/Flatten)** produces one flat list: three variables
(`omega`, `x`, `v`) and two equations. In a model built from components, this is where every
`connect` becomes equations — [connect-expansion](hrw://lab/connect-expansion).

**6 · [DAE](hrw://load/HarmonicOscillator/Dae)** sorts the variables by role. `x` and `v` appear
inside `der()`, so they are *states*; `omega` is a parameter; there are no other unknowns.
Result: 2 states, 0 algebraic variables, 2 equations —
[dae-construction](hrw://lab/dae-construction).

**7 · [Simplify](hrw://load/HarmonicOscillator/Simplify)** is where solve lowering begins: four
transforms that tidy the DAE before any structural work — substitute parameters that can be
fixed now, inline marked functions, merge variables that are only copies of each other (`a =
b`), fold constants. None of the four changes this model; `Drivetrain`'s aliases are where they
matter.

**Predict, before you click Structural:** the next stage pairs each equation with the one unknown
it will compute. What are the two unknowns?

**8 · [Structural](hrw://load/HarmonicOscillator/Structural)** shows that the unknowns are
`der(x)` and `der(v)`, *not* `x` and `v`. At every instant the integrator already knows the
states; what the equations must supply is how fast they are changing. Matching pairs
`der(x) = v` with `der(x)`, and the other equation with `der(v)`. Each forms its own block of
size 1, and none need solving together. A model where equations *do* need each other is
`ProportionalLoop` — [matching](hrw://lab/matching), [blt-ordering](hrw://lab/blt-ordering),
[tearing](hrw://lab/tearing). To watch matching's search from inside Rumoca, step by step, there
is [matching-live](hrw://lab/matching-live): the only lab that needs a debugger and the bridge
extension, where every other lab runs from HRW alone.

**9 · [Index reduction](hrw://load/HarmonicOscillator/IndexReduction)** has nothing to reduce:
the reducer's outcome is *borrowed* — already index-1, both derivatives computed directly — and
state selection keeps `x` and `v` as the two states to integrate. A model where the derivatives
cannot be computed directly is the pendulum, whose rod forces the bob to stay on a circle —
[index-reduction](hrw://lab/index-reduction).

**10 · [Initialization](hrw://load/HarmonicOscillator/Initialization)** shows the plan for the
starting values, and here it is empty: no row to solve, nothing given. `x` and `v` have `start`
values, but nothing *fixes* them — no `fixed = true`, no `initial equation` — so the start
values are only guesses, and the simulation starts from them: `x = 1`, `v = 0` —
[initialization](hrw://lab/initialization).

**11 · [Events](hrw://load/HarmonicOscillator/Events)** lists the moments where the model
switches behaviour. It is not a step of its own: it shows the event data DAE construction (tab 6)
recorded, and what solve lowering made of it. Every count is zero: there is no `if` or `when`
here. `BouncingBall` has one — [events](hrw://lab/events).

**12 · [Solve lowering](hrw://load/HarmonicOscillator/SolveLowering)** turns the equations into
small programs over numbered slots: `omega` is `P[0]`, `x` is `Y[0]`, `v` is `Y[1]`. The
program for `der(x)` is *load `Y[1]`*. The one for `der(v)` loads `P[0]`, multiplies it by
itself, multiplies by `Y[0]` and negates — `omega ^ 2` has become `omega * omega` —
[solve-lowering](hrw://lab/solve-lowering).

**13 · [Simulation](hrw://load/HarmonicOscillator/Simulation)** runs those programs from 0 to
2 s. The exact solution is `x = cos t`, `v = −sin t`, so it can be checked: at 2 s Rumoca gives
`x = −0.416146` against `cos 2 = −0.416147`, and `v = −0.909292` against
`−sin 2 = −0.909297`.

So compile (tabs 1–6) turns your text into equations, solve lowering (tabs 7–10 and 12) turns
the equations into programs that compute the derivatives, and the simulation (tab 13) runs them.

The labs linked above can be run in the order they appear. If you would rather follow the
mathematics than the machinery, run matching, blt-ordering and tearing first: they are one
continuous argument, and the densest linear algebra in the set.

---

## A question you may hit

`RcCircuit` has no `if` and no `when`, yet its Events tab shows `relations: 1` and `roots: 1`.
The relation is `1 + R.alpha*(R.T_heatPort - R.T_ref) >= …` (a tiny positive number): the MSL
Resistor asserts its temperature stays in range, and Rumoca watches that condition for a
crossing — `event_actions: 1` is the assert. A model with no events of its own can still have a
root, brought in by a library component. *(Before 0.10.0 HRW showed this as a
`synthetic_root_conditions` count, which it had renamed `zero_crossing_conditions`; that field
no longer exists.)*

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

`Drivetrain` = 97 equations and 9 states → 3 integrated. Simplify merges 53 aliases; the
reducer then runs 6 rounds, and state selection reduces to a basis of 3 — `mount.s_rel`, `L.i`
and `mount.v_rel` — which is exactly what Solve lowering and the simulation integrate.

`OverInitRc` = 1 state, 2 initial equations, 1 too many. It is `RcCircuit` plus an
`initial equation` section of two lines. The Initialization tab shows Rumoca's plan: `C.v = 0`
is *Solved* for `C.v`, and `der(C.v) = 0` is a `SurplusAlgebraicCheck` — a surplus check the
model must satisfy. It does not: with the capacitor
empty, current flows, and the run fails at initialization with the check's residual at 50.

---

## What to tell me afterwards

The labs' closing sections each name two or three claims I could not check. Those are the
questions, and they are mostly about rendering — whether the Tarjan animation reads as a
search, whether the tearing frames show the greedy choice *as a choice*, whether Act 5 of
`initialization.md` is an interesting aside or a distraction.

And the counts should all be right. If one is not, that is the more valuable finding: it
means either a trace changed or I misread one, and both are worth knowing quickly.
