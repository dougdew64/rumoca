# Fixture lab — Initialization: the values at t = 0

<!-- kind: concept -->

**The system is square, ordered and index-1 — and the integrator still cannot take its first
step**, because it does not have a starting point.

That sounds like a non-problem: every state has a `start` attribute, so surely those are the
starting values. They are not sufficient, for two reasons that pull in opposite directions.

*Too little.* A `start` attribute is only a *guess* unless it says `fixed = true`, and a model
may fix a value on a variable that is not a state at all — a sensor reading, say. Then the
state's value has to be worked out from that variable before the first step.

*Too much.* Nothing stops a model from specifying a state's initial value twice, in ways that
disagree. Modelica lets you write `initial equation` blocks **and** `fixed` starts, and the
compiler must notice when they over-determine the problem rather than quietly preferring one.

This phase settles both. Rumoca lowers an **initialization plan** — a set of residual rows, each
with a role — and the Initialization tab shows that plan, from the same solve-lowering run the
simulation integrates. Run [index-reduction](hrw://lab/index-reduction) first — the plan is built
on the reduced, index-1 system. Every count below is read from a generated trace, so if one
disagrees with your screen, the lab is wrong and I want to know.

---

## Station 1 — The case with nothing to solve

`BouncingBall` has two states, `h` and `v`, each with a `start`.

> **Predict.** How many rows does its initialization plan need?

[Look — BouncingBall → Initialization](hrw://load/BouncingBall/Initialization)

**Expected:** the tab note reads *0 initialization row(s), 0 surplus check(s)*, and in the tree
`rows`, `unknowns` and `given_states` are all empty lists.

Falsified if: any row is listed.

*What just happened.* No initial equation and no `fixed` start, so there is nothing the plan
must satisfy: the states begin at their `start` values and the integrator takes it from there.
An empty plan is a real result, stated rather than left blank. `RcCircuit` reads the same —
[RcCircuit → Initialization](hrw://load/RcCircuit/Initialization) — although it has 22
algebraic variables: they are computed along with everything else once the run starts, and
Rumoca's plan carries no row for them.

---

## Station 2 — The case with a real plan

`MeasuredOscillator` has three states, settled three different ways:

```modelica
Real x(start = 0.5, fixed = true);        // fixed by its own declaration
Real v;                                    // no start of its own
Real v_meas(start = 1.0, fixed = true);   // a tachometer reading, v_meas = v
Real v_filt(start = 0.0);                 // a bare start: only a guess
```

> **Predict.** Which of `x`, `v` and `v_filt` does the plan have to *solve* for, and which
> does it simply take as given?

[Look — MeasuredOscillator → Initialization](hrw://load/MeasuredOscillator/Initialization)

**Expected:** the note reads *1 initialization row(s), 0 surplus check(s)*. In the tree,
`given_states` is `["x"]` and `unknowns` is `["v"]`, and `rows` holds one row: role `Solved`,
`solves` `v`, and `at` pointing at line 20 — `Real v_meas(start = 1.0, fixed = true)`.

Falsified if: `v` is listed under `given_states`, or the one row's `at` points at `v`'s own
declaration.

*What just happened.* Three mechanisms, three places in the plan. `x` fixes itself, so the plan
takes it as given and spends no row on it. `v` fixes nothing, but `v_meas = v` and `v_meas` is
fixed — so the plan solves for `v`, and the row's location is `v_meas`'s line, because that is
where the value comes from. `v_filt` appears nowhere: a bare `start` is not a condition the plan
has to meet.

Notice that the row list alone would suggest one settled state. `x` is the state a pane listing
only rows would silently omit — which is why `given_states` is shown beside the rows.

---

## Station 3 — The case that specifies too much

`OverInitRc` is `RcCircuit` plus an `initial equation` section of two lines:

```modelica
initial equation
  C.v = 0;
  der(C.v) = 0;
```

> **Predict.** The model has one state. What does Rumoca do with two conditions on it — refuse,
> pick one, or something else?

[Look — OverInitRc → Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** the note reads *2 initialization row(s), 1 surplus check(s)*. In the tree, row 0 has
role `Solved`, solves `C.v`, at line 13 (`C.v = 0`); row 1 has role `SurplusAlgebraicCheck`,
solves nothing, at line 14 (`der(C.v) = 0`).

Falsified if: the plan has one row, or the surplus row is row 0.

*What just happened.* Rumoca does not refuse at a count. It uses one condition to solve for the
state and keeps the other as a **consistency check**: a row that reads only values something else
fixed, so all it can do is be satisfied or not (MLS 3.6 §8.6). Whether it is satisfied is a
number, and only the run computes it.

> **Predict.** With `C.v = 0`, what is `der(C.v)` at *t* = 0? (The source is 5 V, the resistor
> 100 Ω, the capacitor 1 mF.)

[Simulate — OverInitRc → Simulation](hrw://load/OverInitRc/Simulation)

**Expected:** the run fails before its first step. The message says the initial variable
projection did not satisfy the residual system, names the owner as `surplus-check`, and gives
`value=5.000000e1`.

Falsified if: the simulation runs, or the value is anything but 50.

*What just happened.* An empty capacitor across 5 V through 100 Ω draws 5 / 100 = 0.05 A, and
`der(C.v)` is that current over 1 mF: 0.05 / 0.001 = 50. The check asked for 0 and got 50. The two
conditions disagree, and the compiler cannot know which one you meant — so it carried both as far
as it could and let the numbers decide.

---

## What this lab cannot check

Whether `given_states` beside `rows` reads as one plan, or as two lists you have to reconcile.
Station 2 depends on reading them together.

Whether Station 3's failure is actionable. The plan names both conflicting conditions by source
line, and the run names the surplus row and its residual; whether that is enough to find the
conflict in a large model is untested — `OverInitRc` is small enough that it does not matter.

---

## What comes next in the chain

Everything is now decided: which variables are states, what order to solve in, and what the
plan must satisfy at *t* = 0. What remains is turning names into memory.

That is [solve-lowering](hrw://lab/solve-lowering).
