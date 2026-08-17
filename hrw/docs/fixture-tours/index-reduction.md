# Fixture tour — Index reduction: more states than freedoms

**A curriculum tour.** Walk [`blt-ordering.md`](blt-ordering.md) and
[`tearing.md`](tearing.md) first. Every model in those was already solvable once ordered; this
tour is about the models that are not.

Every count below was read from the committed traces, never remembered.

---

## The problem this phase exists to solve

A **state** is a quantity that carries the past forward, and the DAE tour established how one is
identified: some equation differentiates it. Count the states and you have counted the numbers the
integrator steps through time.

Except that the count can be wrong — not miscounted, but *wrong as a description of the system*.

Connect two rotating bodies with an ideal gear. Each body has an angle and a velocity, so the
compiler sees four states. But the gear ratio means the second body's angle is a fixed multiple of
the first's: knowing one tells you the other. **There are four states and two freedoms**, and the
two surplus states are not independent quantities at all.

A solver handed that system fails, and not for a reason ordering can fix. The equations that
constrain the states to each other are **algebraic constraints among states** — the definition of a
higher-index DAE.

**Index reduction is the phase that finds and removes them.** Four acts: the case where nothing
needs doing, the case that does, what actually happened to the surplus, and what "index" means.

---

## Act 1 — The case that needs nothing

`BouncingBall` has two states, `h` and `v`.

> **Predict.** Are they independent — can you know one without the other?

[▶ Look — BouncingBall → Index reduction](hrw://load/BouncingBall/IndexReduction)

**Expected:** the stage reports **already index-1**, states **2 before and 2 after**, nothing
demoted, and `differentiated_rows` empty. The note reads *"the reduction funnel is a no-op here."*

**Falsified if:** any state is demoted, or the counts differ.

*What just happened.* Height and velocity are genuinely independent: a ball can be anywhere at any
speed. No equation relates them without a derivative in it, so there is no algebraic constraint
among states and nothing to remove.

**This is the common case**, and it is worth establishing first so the next act reads as a
discovery rather than as routine. Most models are index-1 and this phase does nothing to them.

---

## Act 2 — The case that does

`Drivetrain` is a motor driving a load through an ideal gear, with a compliant mount.

> **Predict.** The model has nine states. How many independent freedoms do you expect a motor,
> gear, shaft and mount to have?

[▶ Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

**Expected:** **9 states before, 3 after.** The three survivors are `L.i`, `shaft.w` and
`mount.s_rel`; the six demoted are `emf.phi`, `rotor.phi`, `rotor.w`, `shaft.phi`, `load.s` and
`load.v`.

**Falsified if:** the after-count is other than 3, or `shaft.w` is among the demoted.

*What just happened. **Six of the nine were never independent.** The gear ties the rotor's angle to
the shaft's; the rack-and-pinion ties the load's position to the shaft's angle; differentiating
those relations ties the velocities too. So the system's real freedoms are one electrical (the
inductor current), one mechanical rotation (the shaft speed), and one mechanical translation (the
mount deflection).

Look at which survived, because it is physically legible: **exactly one state per independent
energy store.** The inductor stores magnetic energy, the shaft inertia stores kinetic energy, the
mount spring stores potential energy. Everything else was bookkeeping about where things are
relative to each other.

**The compiler discovered that from the equations alone**, with no knowledge of gears.

---

## Act 3 — What actually happened to the surplus

The obvious mechanism for index reduction is **differentiation**: differentiate the offending
constraint until it can be solved. That is what Pantelides' algorithm does and what every textbook
describes.

> **Predict.** How many equations did the compiler differentiate to get from 9 states to 3?

[▶ Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

[Point at `reduction`](hrw://stage/IndexReduction/node/reduction)

**Expected:** **zero.** `differentiated_rows` is empty. What did the work is **77 eliminations**
and the demotion steps, and the system went from **97 equations to 20**.

**Falsified if:** `differentiated_rows` is non-empty, or the reduced system is not 20 equations.

*What just happened.* **The textbook mechanism was not needed.** These constraints are *alias
relations* — one variable equals another times a constant — and an alias can be eliminated by
substitution instead of differentiated. Cheaper, exact, and it shrinks the system rather than
growing it.

That is why 97 equations became 20 while the state count fell from 9 to 3: most of those 97 were
connector bookkeeping that substitution removes outright.

**Differentiation is the general answer and the last resort.** A compiler that reached for it first
would produce a larger system than it started with. That this specimen never needs it is a fact
about ideal gears, not about the algorithm — a model with a genuine non-alias constraint would show
`differentiated_rows` filled.

---

## Act 4 — What "index" counts, and why 1 is the target

The phase is named for a number nothing on screen displays.

> **Predict.** `Drivetrain` arrived high-index and left index-1. What do you think the number
> counts?

[▶ Look — Drivetrain → Structural](hrw://load/Drivetrain/Structural)

**Expected:** the Structural stage reports **singular** before reduction, and index reduction
reports *"index-reduced from a structurally singular (high-index) system — now solvable."*

**Falsified if:** Structural reports a solvable system before reduction.

*What just happened.* **The differentiation index is how many times you must differentiate the
system before it becomes an ODE you can integrate.** Index 1 means the algebraic part can be solved
for at each instant, given the states — which is exactly what the solver needs. Index 2 means one
round of differentiation stands between you and that, and so on.

So "high-index" and "structurally singular before reduction" are the same observation from two
angles: the matching could not find a perfect pairing precisely because some equations constrained
states to each other rather than determining anything new.

**And note the order of the pipeline.** Matching runs, fails, and its failure is the *input* to
this phase — which is why a structural singularity is a diagnosis and not a verdict. Five of the
eight specimens that report `singular` are rescued here; three are not, and those are genuinely
ill-posed models.

---

## What this tour cannot check

**Whether Act 3 lands as the surprise it is.** Zero differentiations in the tour named for the
algorithm that differentiates is the most interesting fact here, and it is asserted in one line
against a JSON field that must be found in a tree.

**Whether the reduction view reads as a funnel.** It is a summary of ten steps, most of which
demote nothing, and whether the progression is visible or reads as noise is your report.

**Whether `Drivetrain` is too large to learn from.** 97 equations and 88 algebraics is a real
model, and the counts are honest, but nothing here lets you see the gear relation being eliminated
— only that 77 eliminations happened. A two-body specimen would show the mechanism; there is not
one.

---

## A number downstream disagrees with this tour, and it is a real open question

Act 2 established **3 states after reduction**, and that is what this stage reports. Walk on to
[`solve-lowering.md`](solve-lowering.md) and look at the same model, and you will find
`state_scalar_count` reading **9** — the count *before* reduction.

| specimen | `index_reduction` says | `solve_lowering` says |
|---|---|---|
| `Drivetrain` | **3** | **9** |
| `GearWithBrake` | **2** | **7** |
| `BouncingBall` | 2 | 2 (nothing was demoted) |

**The two agree exactly when nothing is demoted**, which is what makes it look like the demoted
states are still being counted downstream.

**Not diagnosed, and there are two innocent readings.** Either the demoted states genuinely still
occupy slots — a dummy-derivative scheme keeps them as algebraic variables, so a *scalar count* of
them is not obviously wrong — or the field means "states before reduction" and is merely named
badly. Only one of those is a bug.

The investigation is in [`upstream-issues.md`](../upstream-issues.md), written to be filed. It is
named here rather than left for you to trip over, because a tour that asserts 3 and a pane that
shows 9 is exactly the situation where you cannot tell which of the two to trust.

---

## What comes next in the chain

The system is now index-1, square and ordered. It still cannot start: the integrator needs a
consistent set of values at *t* = 0, and the states' `start` attributes are not automatically
consistent with the algebraic equations.

That is [`initialization.md`](initialization.md).
