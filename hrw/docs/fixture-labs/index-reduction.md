# Fixture lab — Index reduction: when differentiating is the only way out

<!-- kind: concept -->

**Counting a model's states can give you the wrong number — not miscounted, but wrong as a
description of the machine.** Connect two rotating bodies with an ideal gear: each has an angle
and a velocity, so the compiler sees four states, but the gear ratio fixes the second angle as a
multiple of the first. Knowing one tells you the other. Four states, two freedoms.

A state carries the past forward, and you already established how one is identified — nothing
declares it, some equation *differentiates* it, and the equation sheet's Why column names which:

[◂ Re-read it — DAE construction, Station 2](hrw://lab/dae-construction/station/station-2-what-makes-a-variable-a-state)

**This lab assumes only that you know what a derivative is.** Everything else — what "index"
counts, why differentiating a constraint helps, why solvers want index 1 — is built at the
stations, starting with one you work out by hand before the compiler is asked anything.

Run [blt-ordering](hrw://lab/blt-ordering) and [tearing](hrw://lab/tearing) first; every model in
those was solvable once ordered. Every count below is read from a generated trace, so if one
disagrees with your screen, the lab is wrong and I want to know.

---

## Station 1 — Why can five equations in five unknowns be unsolvable?

`CartesianPendulum` is a point mass on a rigid rod, in Cartesian coordinates:

```modelica
der(x) = vx;
der(y) = vy;
m * der(vx) = -lambda * x;
m * der(vy) = -lambda * y - m * g;
x ^ 2 + y ^ 2 = L ^ 2;
```

**The difficulty is not that there is a constraint.** A DAE solver handles algebraic constraints
— that is its job. It solves `F(t, y, y') = 0`, constraints included, and index-1 systems are
full of them. Any explanation that says "the solver cannot cope with a constraint" is hiding the
real difficulty, and the real one needs nothing but the matching you have already run.

Start from what is actually unknown at an instant. A state is two things at once, a known value
on the way in and an unknown rate on the way out:

[◂ Re-read it — DAE construction, Station 3](hrw://lab/dae-construction/station/station-3-what-is-the-solver-actually-solving-for)

So when a step begins the solver already *has* `x`, `y`, `vx` and `vy`. What it must work out is
the four rates — plus `lambda`, the rod's tension, which is on the list because nothing carries
it forward. **Five unknowns, and the model has five equations.**

An equation can only help determine a quantity it actually mentions. The Incidence view draws
exactly that: one row per equation, marked with the unknowns it touches.

> **Predict.** Square is supposed to be the thing you check. Before you look: of those five
> equations, how many touch one of the five unknowns — and if the answer is not five, which one
> does not, and what is left unclaimed?

[Look — CartesianPendulum → Structural → Incidence](hrw://load/CartesianPendulum/Structural/Incidence)

[Point at the constraint row, `f_x[4]`](hrw://stage/Structural/Incidence/equation/4)

**Expected:** `f_x[4]`'s row is **empty** — it touches none of the five unknowns — while
`lambda`'s column is marked in exactly two rows, `f_x[2]` and `f_x[3]`.

Falsified if: `f_x[4]` marks any unknown, or `lambda` appears in a number of rows other than two.

### What just happened

The constraint `x^2 + y^2 - L^2` is built from `x`, `y` and `L` — every one already known when
the step begins. It is a true statement that cannot do any work. And that strands `lambda`, which
appears in only two equations, both already needed for `der(vx)` and `der(vy)`.

One equation that can pair with nothing, one unknown that nothing is left to determine. Five
equations, five unknowns, unsolvable.

**That is what high index means:** not *"there is a constraint"*, but *"a constraint mentions
none of the quantities being solved for."*

### Why differentiating is the fix

The constraint is true at every instant, so it stays true when you differentiate it — and
differentiating is exactly the operation that turns a statement about positions into one about
rates. Do it once, by the chain rule:

```text
x^2 + y^2 = L^2      differentiates to      2*x*vx + 2*y*vy = 0
```

Better — it now mentions `vx` and `vy` — but those are *known* at this instant, so it still helps
with none of our five unknowns. Differentiate once more and the rates of `vx` and `vy` appear:

```text
vx^2 + x*der(vx) + vy^2 + y*der(vy) = 0
```

Now it mentions `der(vx)` and `der(vy)`, which are on our list. Substitute what the two force
equations say those equal, and the whole thing collapses to a formula for the one unknown nothing
could reach:

```text
lambda = m * (vx^2 + vy^2 - g*y) / L^2
```

Every unknown now has an equation that mentions it, the matching completes, and the system is
solvable at each step.

**Two differentiations were needed — which is where "index 3" comes from.** So *index* is a
distance, not a quality score: how far a system is from one whose constraints talk about the
unknowns. Index 1 means it is already there, which is why index 1 is the target and why this
phase exists.

> If you know the numerical version, this is the same statement: the solver finds the unknowns
> by Newton iteration, the constraint contributes a row of zeros to the Jacobian with respect to
> them, and a zero row makes it singular. The matching is that pattern found by counting rather
> than by arithmetic — which is why a compiler can detect it before any number is computed.

Index reduction is the phase that runs that distance. The next four stations watch it run — and
then **Station 6 comes back to this pendulum**, where you now know the answer the compiler ought
to reach.

---

## Station 2 — The case that needs nothing

`BouncingBall` has two states, `h` and `v`.

> **Predict.** Height and velocity. Is either one determined by the other?

[Look — BouncingBall → Index reduction](hrw://load/BouncingBall/IndexReduction)

**Expected:** 2 states before, 2 after. Nothing demoted, and the pane reports no
differentiation.

Falsified if: any state is demoted, or the counts differ.

*What just happened.* A ball can be anywhere at any speed, so no equation relates `h` to `v`
without a derivative in it. There is no constraint to differentiate and the distance to index 1
is already zero.

This is the common case, and it is worth establishing first so the next station reads as a
discovery rather than as routine.

---

## Station 3 — The smallest model that needs something

`BenchActuator` is a motor driving a load — four states, and one of them is not free.

> **Predict.** Four states. If the compiler removes one, how many equations do you think it has
> to differentiate to do it — none, one, or several?

[Look — BenchActuator → Index reduction](hrw://load/BenchActuator/IndexReduction)

**Expected:** 4 states before, 3 after, the demoted state is `emf.phi`, and the pane
reports 1 differentiation performed.

Falsified if: more than one state is demoted, or the differentiation count is 0.

*What just happened.* One constraint, one differentiation, one state removed — the whole
mechanism at a size you can hold in your head.

`emf.phi` is the motor shaft's angle, and the model already determines it from the rotor's. So it
was never an independent quantity; it was bookkeeping. Removing it needs the constraint restated
in terms of rates, and that restatement is the differentiation the pane counts.

Notice what the pane says about survival. It reports the differentiation happening, and then
that none of the manufactured equations survive to the end. Hold that; Station 5 is about it.

---

## Station 4 — The same idea, at a scale you could not do by hand

`Drivetrain` is a motor, an ideal gear, a shaft and a compliant mount.

> **Predict.** Nine states. How many independent freedoms does that machine really have?

[Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

**Expected:** 9 states before, 3 after, and 6 differentiations performed. The survivors
are `L.i`, `shaft.w` and `mount.s_rel`; the system goes from 97 equations to 20.

Falsified if: the after-count is other than 3, or `shaft.w` is among the demoted.

*What just happened.* Six of the nine were never independent. The gear ties the rotor's angle
to the shaft's, the rack-and-pinion ties the load's position to the shaft's angle, and
differentiating those ties the velocities too.

Look at *which* three survived, because it is physically legible: exactly one state per
independent energy store. The inductor stores magnetic energy, the shaft inertia kinetic, the
mount spring potential. Everything else was bookkeeping about where things sit relative to each
other.

The compiler discovered that from the equations alone, knowing nothing about gears.

---

## Station 5 — What the compiler actually reached for

Here is the station this lab exists for, and it is about a number that used to be read wrong.

> **Predict.** `Drivetrain` differentiated 6 times. How many of those manufactured equations do
> you expect to find in the final system?

[Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

[Point at `reduction`](hrw://stage/IndexReduction/Tree/node/reduction)

**Expected:** `n_differentiations` is 6 and `differentiated_rows` is empty — none of them
survive. The step list shows `reduce_constrained_dummy_derivatives` demoting all 6, and
`eliminate_trivial` removing 77 equations.

Falsified if: the demotions come from any step other than
`reduce_constrained_dummy_derivatives`, or `n_differentiations` is 0.

*What just happened.* The differentiated equations were built and then thrown away, and both
halves are correct.

Differentiating gives the solver a usable statement — but once the surplus state is demoted, the
manufactured row often says something a *simpler* row already says, and `eliminate_trivial`
substitutes it out. 97 equations become 20 that way. The differentiation did its job by
existing, not by surviving.

The step that did the demoting is named for the method.
`reduce_constrained_dummy_derivatives` is the dummy-derivative method — Mattsson and
Söderlind's, the standard companion to Pantelides. When differentiating would leave you with more
equations than unknowns, you demote a derivative to an ordinary unknown — a *dummy* — to keep the
count square. That is what those six demotions are.

> ### Why this station is worded so carefully
>
> Until 2026-08-17 it read *"zero differentiations — the textbook mechanism was not needed"*,
> because `differentiated_rows` is empty and I read the name instead of the semantics. It
> counts survivors, not differentiations. The pane now reports both, and it reports them
> because this station was wrong.

---

## Station 6 — The model Rumoca cannot reduce

Back to the pendulum from Station 1. Every constraint in the three stations since was an alias:
one variable equal to another times a constant, which substitution can remove. `x² + y² = L²` is
not. It is nonlinear and involves two states at once, so no substitution touches it.
Differentiation is the only route, which makes this the textbook case and the first real test of
the phase.

**You already know what has to happen.** Station 1 differentiated that constraint twice by hand,
and the second differentiation is what brings `lambda` within reach.

> **Predict.** So: how many differentiations will Rumoca perform?

[Look — CartesianPendulum → Index reduction](hrw://load/CartesianPendulum/IndexReduction)

**Expected:** zero. States 4 before, 4 after, nothing demoted, and every step reports 0.
The pane says the funnel did not act on the system.

Falsified if: any state is demoted, or the differentiation count is above 0.

*What just happened.* Rumoca does not reduce this model, and the diagnosis names the physics
exactly. Look at the Structural tab:

[Look — CartesianPendulum → Structural](hrw://load/CartesianPendulum/Structural)

**Expected:** `structurally singular system: 4 matched out of 5 equations and 5 unknowns;
unmatched equations: f_x[4]; unmatched unknowns: lambda`.

Falsified if: the unmatched pair is anything other than one equation and `lambda`.

**That is the same pair you found in Station 1**, arrived at by a different route. There you
read it off the incidence pattern — an empty row, a column reachable from only two equations.
Here the matching reports it as a failure, by name. The compiler found by algorithm what you
found by reading five equations.

Rumoca's index reduction is pattern-based, not general Pantelides. Read the step names:
exact aliases, direct assignments, constrained dummy derivatives, states missing a derivative
row. Each hunts a *shape*. The pendulum matches none of them: all four states have derivative
rows, and the constraint is nonlinear.

The simulation then fails the way an unreduced high-index system does — *"step size is too small
at time = 0.0000477"* — with no mention of index anywhere.

This is not a defect claim. Whether general reduction is missing, deferred, or deliberately
out of scope is a question for Rumoca's maintainers, and it is filed in
[`upstream-issues.md`](hrw://doc/upstream-issues.md) as a question. What the station establishes is the
boundary: you now know what this compiler does, and what it does not.

---

## What this lab cannot check

Whether the four specimens read as one idea. They are meant to: a model that must be
differentiated by hand before anything else happens, then nothing needed, one differentiation,
six, and back to the first with the compiler declining. Whether that
lands as a progression or as four unrelated panes is your report and nothing else's.

Whether "index" survives as a distance rather than a score. Station 1 builds it that way and no
pane displays an index number anywhere, so the idea rests on that station's prose and the
arithmetic the reader does there.

What a differentiated equation looks like. The pane counts them and, on every specimen that
makes any, they are gone by the end — so the corpus can tell you six were made and cannot show
you one.

Whether System Modeler reduces the pendulum. That is the calibration that would turn Station
6's reading into a fact, and it has not been run. `the-oracle.md` is the lab for that gesture.

---

## What comes next in the chain

The system is now index-1, square and ordered — or it is not, and you have seen what that looks
like. What remains is starting it: the integrator needs consistent values at *t* = 0, and the
states' `start` attributes are not automatically consistent with the algebraic equations.

That is [initialization](hrw://lab/initialization).
