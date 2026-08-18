# Fixture tour — Index reduction: when differentiating is the only way out

<!-- kind: concept -->

[▲ The chain overview](hrw://tour/the-concepts)

**A concept tour.** Walk [`blt-ordering.md`](blt-ordering.md) and [`tearing.md`](tearing.md)
first. Every model in those was solvable once ordered; this tour is about models that are not,
and it ends with one that Rumoca cannot rescue at all.

**This tour assumes only that you know what a derivative is.** Everything else — what "index"
counts, why differentiating a constraint helps, why solvers want index 1 — is built here.

Every count below was read from the committed traces, never remembered. **If one disagrees with
your screen, the tour is wrong and I want to know.**

---

## The problem this phase exists to solve

A **state** carries the past forward, and the DAE tour established how one is identified: some
equation differentiates it. Count the states and you have counted the numbers the integrator
steps through time.

Except the count can be wrong — not miscounted, but *wrong as a description of the system*.

Connect two rotating bodies with an ideal gear. Each has an angle and a velocity, so the compiler
sees four states. But the gear ratio fixes the second angle as a multiple of the first: knowing
one tells you the other. **Four states, two freedoms.**

### Why a solver cannot simply be told about the constraint

This is the part usually skipped, and it is the whole phase.

**An integrator can be asked for exactly one thing.** You hand it the current states and it
tells you their *rates*; it multiplies those by a small step and adds. That is its only move.

Now hand it `phi_2 = 5 * phi_1` and ask it to respect that. **It has no lever.** The constraint
talks about *positions*, and the integrator's move is about *rates*. Nothing in the step it takes
can be adjusted to honour a statement about the quantities it is not computing. Take one step and
the two angles drift apart; nothing pulls them back, because nothing was ever told how.

**Differentiating converts the sentence into the integrator's vocabulary.** Differentiate both
sides of `phi_2 = 5 * phi_1` and you get `w_2 = 5 * w_1` — the same physical fact, now stated
about the rates the solver actually computes. *That* it can honour.

**And that is all "index" counts: how many differentiations it takes to get there.** A system
already speaking about rates is index 1, which is why index 1 is the target. It is not a quality
score; it is a distance, measured in differentiations.

**Index reduction is the phase that walks that distance.** Five stops: a model needing nothing,
the smallest model that needs something, the same idea at scale, what the compiler actually
reaches for, and a model it cannot reduce.

---

## Stop 1 — The case that needs nothing

`BouncingBall` has two states, `h` and `v`.

> **Predict.** Height and velocity. Is either one determined by the other?

[▶ Look — BouncingBall → Index reduction](hrw://load/BouncingBall/IndexReduction)

**Expected:** **2 states before, 2 after.** Nothing demoted, and the pane reports **no
differentiation**.

**Falsified if:** any state is demoted, or the counts differ.

*What just happened.* A ball can be anywhere at any speed, so no equation relates `h` to `v`
without a derivative in it. There is no constraint to differentiate and the distance to index 1
is already zero.

**This is the common case**, and it is worth establishing first so the next stop reads as a
discovery rather than as routine.

---

## Stop 2 — The smallest model that needs something

`BenchActuator` is a motor driving a load — four states, and one of them is not free.

> **Predict.** Four states. If the compiler removes one, how many equations do you think it has
> to differentiate to do it — none, one, or several?

[▶ Look — BenchActuator → Index reduction](hrw://load/BenchActuator/IndexReduction)

**Expected:** **4 states before, 3 after**, the demoted state is **`emf.phi`**, and the pane
reports **1 differentiation performed**.

**Falsified if:** more than one state is demoted, or the differentiation count is 0.

*What just happened.* **One constraint, one differentiation, one state removed** — the whole
mechanism at a size you can hold in your head.

`emf.phi` is the motor shaft's angle, and the model already determines it from the rotor's. So it
was never an independent quantity; it was bookkeeping. Removing it needs the constraint restated
in terms of rates, and that restatement is the differentiation the pane counts.

**Notice what the pane says about survival.** It reports the differentiation happening, and then
that **none of the manufactured equations survive to the end**. Hold that; Stop 4 is about it.

---

## Stop 3 — The same idea, at a scale you could not do by hand

`Drivetrain` is a motor, an ideal gear, a shaft and a compliant mount.

> **Predict.** Nine states. How many independent freedoms does that machine really have?

[▶ Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

**Expected:** **9 states before, 3 after**, and **6 differentiations performed**. The survivors
are `L.i`, `shaft.w` and `mount.s_rel`; the system goes from **97 equations to 20**.

**Falsified if:** the after-count is other than 3, or `shaft.w` is among the demoted.

*What just happened.* **Six of the nine were never independent.** The gear ties the rotor's angle
to the shaft's, the rack-and-pinion ties the load's position to the shaft's angle, and
differentiating those ties the velocities too.

Look at *which* three survived, because it is physically legible: **exactly one state per
independent energy store.** The inductor stores magnetic energy, the shaft inertia kinetic, the
mount spring potential. Everything else was bookkeeping about where things sit relative to each
other.

**The compiler discovered that from the equations alone**, knowing nothing about gears.

---

## Stop 4 — What the compiler actually reached for

Here is the stop this tour exists for, and it is about a number that used to be read wrong.

> **Predict.** `Drivetrain` differentiated 6 times. How many of those manufactured equations do
> you expect to find in the final system?

[▶ Look — Drivetrain → Index reduction](hrw://load/Drivetrain/IndexReduction)

[Point at `reduction`](hrw://stage/IndexReduction/node/reduction)

**Expected:** `n_differentiations` is **6** and `differentiated_rows` is **empty** — none of them
survive. The step list shows `reduce_constrained_dummy_derivatives` demoting **all 6**, and
`eliminate_trivial` removing **77 equations**.

**Falsified if:** the demotions come from any step other than
`reduce_constrained_dummy_derivatives`, or `n_differentiations` is 0.

*What just happened.* **The differentiated equations were built and then thrown away**, and both
halves are correct.

Differentiating gives the solver a usable statement — but once the surplus state is demoted, the
manufactured row often says something a *simpler* row already says, and `eliminate_trivial`
substitutes it out. 97 equations become 20 that way. **The differentiation did its job by
existing, not by surviving.**

**The step that did the demoting is named for the method.**
`reduce_constrained_dummy_derivatives` is the **dummy-derivative** method — Mattsson and
Söderlind's, the standard companion to Pantelides. When differentiating would leave you with more
equations than unknowns, you demote a derivative to an ordinary unknown — a *dummy* — to keep the
count square. That is what those six demotions are.

> ### Why this stop is worded so carefully
>
> Until 2026-08-17 it read *"**zero** differentiations — the textbook mechanism was not needed"*,
> because `differentiated_rows` is empty and I read the name instead of the semantics. **It
> counts survivors, not differentiations.** The pane now reports both, and it reports them
> because this stop was wrong.

---

## Stop 5 — The model Rumoca cannot reduce

`CartesianPendulum` is a point mass on a rigid rod, in Cartesian coordinates:

```modelica
der(x) = vx;
der(y) = vy;
m * der(vx) = -lambda * x;
m * der(vy) = -lambda * y - m * g;
x ^ 2 + y ^ 2 = L ^ 2;
```

Five equations, five unknowns — `x`, `y`, `vx`, `vy`, and `lambda`, the rod's tension.

**Every constraint you have seen so far was an alias**: one variable equal to another times a
constant, which substitution can remove. **`x² + y² = L²` is not.** It is nonlinear and involves
two states at once, so no substitution touches it. Differentiation is the only route — which
makes this the textbook case, and the first real test of the phase.

> **Predict.** This is the canonical example every treatment of index reduction opens with. How
> many differentiations will Rumoca perform?

[▶ Look — CartesianPendulum → Index reduction](hrw://load/CartesianPendulum/IndexReduction)

**Expected:** **zero.** States **4 before, 4 after**, nothing demoted, and every step reports 0.
The pane says the funnel **did not act on the system**.

**Falsified if:** any state is demoted, or the differentiation count is above 0.

*What just happened.* **Rumoca does not reduce this model**, and the diagnosis names the physics
exactly. Look at the Structural tab:

[▶ Look — CartesianPendulum → Structural](hrw://load/CartesianPendulum/Structural)

**Expected:** `structurally singular system: 4 matched out of 5 equations and 5 unknowns;
unmatched equations: f_x[4]; unmatched unknowns: lambda`.

**Falsified if:** the unmatched pair is anything other than one equation and `lambda`.

`f_x[4]` **is** the constraint and `lambda` **is** its force. **That pair is the signature of a
high-index system.** The constraint mentions no derivative and no `lambda`, so nothing can pair
with it; `lambda` appears only in the two force equations, which are already matched to `der(vx)`
and `der(vy)`. Differentiating the constraint twice would bring accelerations into it — and with
them `lambda` — at which point the pair matches and the system is index 1.

**Rumoca's index reduction is pattern-based, not general Pantelides.** Read the step names:
exact aliases, direct assignments, constrained dummy derivatives, states missing a derivative
row. Each hunts a *shape*. The pendulum matches none of them: all four states have derivative
rows, and the constraint is nonlinear.

The simulation then fails the way an unreduced high-index system does — *"step size is too small
at time = 0.0000477"* — with no mention of index anywhere.

**This is not a defect claim.** Whether general reduction is missing, deferred, or deliberately
out of scope is a question for Rumoca's maintainers, and it is filed in
[`upstream-issues.md`](../upstream-issues.md) as a question. What the stop establishes is the
**boundary**: you now know what this compiler does, and what it does not.

---

## What this tour cannot check

**Whether the four specimens read as one idea.** They are meant to: nothing needed, one
differentiation, six, and then a model where differentiation is required and absent. Whether that
lands as a progression or as four unrelated panes is your report and nothing else's.

**Whether "index" survives as a distance rather than a score.** The opening builds it that way
and no pane displays an index number anywhere, so the idea rests entirely on prose.

**What a differentiated equation looks like.** The pane counts them and, on every specimen that
makes any, they are gone by the end — so the corpus can tell you six were made and cannot show
you one.

**Whether System Modeler reduces the pendulum.** That is the adjudication that would turn Stop
5's reading into a fact, and it has not been run. `the-oracle.md` is the tour for that gesture.

---

## What comes next in the chain

The system is now index-1, square and ordered — or it is not, and you have seen what that looks
like. What remains is starting it: the integrator needs consistent values at *t* = 0, and the
states' `start` attributes are not automatically consistent with the algebraic equations.

That is [`initialization.md`](initialization.md).

Or go back up: [▲ The chain overview](hrw://tour/the-concepts)
