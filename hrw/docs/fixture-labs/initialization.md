# Fixture lab — Initialization: the values at t = 0

<!-- kind: concept -->

[The chain overview](hrw://lab/the-concepts)

**A concept lab.** Run [index-reduction](hrw://lab/index-reduction) first — this phase runs on
the reduced, index-1 system.

Every count below was read from the committed traces, never remembered.

---

## The problem this phase exists to solve

The system is square, ordered, and index-1. The integrator still cannot take its first step,
because it does not have a starting point.

That sounds like a non-problem: every state has a `start` attribute, so surely those are the
starting values. They are not sufficient, for two reasons that pull in opposite directions.

Too little. A state's `start` value fixes the state, but the *algebraic* variables are not
free — they satisfy equations. Setting `C.v = 0` does not tell you what `R.i` is at *t* = 0; that
has to be solved for, from the algebraic part of the system, before the first step.

Too much. Nothing stops a model from specifying a state's initial value twice, in ways that
disagree. Modelica lets you write `initial equation` blocks *and* `start` attributes, and the
compiler must notice when they over-determine the problem rather than quietly preferring one.

This phase settles both. Three stops: the case with nothing to solve, the case with a real
initialization system, and the case that specifies too much.

---

## Station 1 — The case with nothing to solve

`BouncingBall` has two states and two equations.

> **Predict.** How large is its initialization problem?

[Look — BouncingBall → Initialization](hrw://load/BouncingBall/Initialization)

**Expected:** `n_states` is 2, `n_equations` is 2, and `block_count` is 0. The note
reads *"no algebraic initialization subsystem (equations ≤ states)."*

Falsified if: any initialization blocks are reported.

*What just happened.* Both equations are differential — each determines a derivative — so once `h`
and `v` are given, there is nothing left to compute. The `start` attributes *are* the whole answer.

Zero blocks is a real result, stated rather than left blank. A model with no algebraic
variables has no initialization system, and the pane says so instead of showing an empty list you
would have to interpret.

---

## Station 2 — The case with a real initialization system

`RcCircuit` has one state and twenty-two algebraic variables.

> **Predict.** One state, 23 equations. How many of those equations does initialization have to
> solve, and for what?

[Look — RcCircuit → Initialization](hrw://load/RcCircuit/Initialization)

**Expected:** `n_states` 1, `n_equations` 23, and `block_count` 21. The determinacy
verdict reads *"well-posed (remaining states initialize from their start attributes)"*, with
`explicit_initial_conditions: 0` and `surplus_over_states: -1`.

Falsified if: `block_count` is 0, or the verdict is not well-posed.

*What just happened.* 21 blocks is a whole solve before time starts. Given `C.v` from its
`start` attribute, every other quantity in the circuit — every pin voltage, every branch current,
the resistor's dissipated power — must be computed to satisfy the algebraic equations at *t* = 0.
That is a system in its own right, and it gets its own BLT decomposition, which is what those 21
blocks are.

Read `surplus_over_states: -1` carefully, because the sign convention repeats the DAE lab's
lesson. Zero explicit initial conditions against one state is a deficit, and a deficit is
fine: the unspecified state falls back to its `start` attribute, which is Modelica's default and
the verdict says so.

A surplus is the problem, not a deficit. Which is Station 3.

---

## Station 3 — The case that specifies too much

`OverInitRc` is `RcCircuit` with initial conditions added.

> **Predict.** The model has one state. What happens if two initial equations both constrain it?

[Look — OverInitRc → Initialization](hrw://load/OverInitRc/Initialization)

**Expected:** the stage reports

```
OVER-DETERMINED initialization: 2 explicit initial condition(s)
(2 initial equation(s) + 0 fixed start(s)) for 1 state(s) — 1 too many
```

Falsified if: the model initializes successfully, or the surplus is other than 1.

*What just happened.* Two constraints on one freedom. The counting is the same argument as DAE
construction's balance check, applied to a different system: one state accepts exactly one initial
condition, and two either agree — in which case one is redundant — or disagree, in which case there
is no solution at all.

The compiler does not try to work out which. It reports the count and stops, because *"these two
initial conditions conflict"* is a modelling question and the compiler cannot know which one you
meant.

Notice the breakdown in the message: 2 initial equations + 0 fixed starts. Modelica has two
mechanisms for specifying an initial value, and over-determination is usually a *mixture* of them —
someone adds an `initial equation` to a model whose state already had `fixed = true`. Reporting the
two sources separately is what makes the fix obvious.

---

## Station 4 — Square and still singular, again

`RotationalInertia` is a torque source driving an inertia with one flange left unconnected.

> **Predict.** Its continuous system is 12 equations, 12 unknowns, fully matched. Will its
> initialization succeed?

[Look — RotationalInertia → Initialization](hrw://load/RotationalInertia/Initialization)

**Expected:** it fails:

```
IC planning failed: structurally singular system: 8 matched out of 10 equations and 10 unknowns;
unmatched unknowns: inertia.flange_b.phi, torque.flange.phi
```

Falsified if: initialization succeeds, or the unmatched unknowns are currents rather than
angles.

*What just happened.* Ten and ten, and still singular — the third time this pattern appears,
and the DAE lab's *"square is necessary, not sufficient"* now has three demonstrations.

But look at *which* unknowns have nothing to determine them: two angles. Yesterday's asymmetry
explains it exactly. The unconnected flange's *torque* is determined — the zero-flow rule
manufactures `inertia.flange_b.tau = 0`. Its *angle* is determined only relative to the body,
by the inertia's own equation. Nothing anywhere fixes where the shaft actually is.

And that is correct physics, not a compiler defect. A free-floating inertia's absolute angle is
genuinely undetermined; you have to say where it starts. In the continuous system it never mattered,
because only differences of angle appear. Initialization is where *"relative to what?"* becomes a
question that has to be answered.

---

## What this lab cannot check

Whether Station 2's 21 blocks read as significant. A whole BLT decomposition running before time
starts is the least-known thing in this lab, and it is one number in a tree.

Whether Station 4 is a fourth stop or a second lab. It is the most interesting failure here and it
depends on `connect-expansion.md`'s potential/flow asymmetry, which not every reader will have
fresh.

Whether the over-determined message is actionable. It names counts and sources; it does not
name *which two* initial conditions conflict. Whether that is enough to find them in a real model
is untested — `OverInitRc` is small enough that it does not matter.

---

## What comes next in the chain

Everything is now decided: which variables are states, what order to solve in, what to guess, and
what the values are at *t* = 0. What remains is turning names into memory.

That is [solve-lowering](hrw://lab/solve-lowering).

Or go back up: [The chain overview](hrw://lab/the-concepts)
