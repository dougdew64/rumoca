# Fixture tour — DAE construction: the count that decides everything

**A curriculum tour.** It teaches a step of the chain
(`docs/compiler-phases/the-chain-of-problems.md`) and uses HRW as the instrument rather than the
subject. It is **still a test**: every **Expected** line is violable, and a lesson built on a
wrong number teaches the wrong thing.

Every count below was read from `docs/specimen-notebook/{SingleInertia,UnbalancedShaft}/trace/`,
never remembered. **Notices appear in the status bar**, along the bottom of the window.

---

## The problem this phase exists to solve

You have just come out of **flattening**, which crushed a hierarchy of components into one flat
namespace and one flat list of equations — including the connection equations you watched being
generated. What comes out is faithful, and it is still written in *your* vocabulary: named
variables, `der()` calls, equations with a left side and a right side.

A numerical integrator cannot use that. It wants a specific shape. At each instant it knows where
the system currently **is**, and it needs to be told how fast everything is **changing**, so it
can step forward and repeat.

So something has to sort every variable into a role — carried forward through time, fixed for the
whole run, or solved for afresh at each instant — and then make one claim about the result:
**this system is square.** Everything downstream is entitled to assume that claim.

**DAE construction is that phase.** Five acts: the sorting, why it sorts that way, what the solver
is really solving for, the claim, and what happens when the claim fails.

---

## Act 1 — Which declarations carry the past?

`SingleInertia` declares four things:

```modelica
parameter Real J = 1.0;
parameter Real tau = 1.0;
Real phi(start = 0.0);
Real w(start = 0.0);
```

The DAE tab shows Rumoca's own partition, under the names the Modelica specification uses in
Appendix B: **`x`** for states, **`y`** for algebraics, **`p`** for parameters.

> **Predict.** How many of those four end up in `x`, and which?

[▶ Look — SingleInertia → DAE](hrw://load/SingleInertia/Dae)

[Point at `x`](hrw://stage/Dae/node/x)

**Expected:** `x` holds exactly **two** — `phi` and `w`. `p` holds `J` and `tau`. **`y` is
empty.**

**Falsified if:** `tau` appears in `x`, or `y` has any member at all.

*What just happened.* Two different mechanisms produced that split, and only one of them read
your declarations.

`J` and `tau` are in `p` because you **wrote `parameter`**. That is a declaration keyword and the
compiler takes you at your word.

But nothing you wrote says `phi` and `w` are states. There is no `state` keyword in Modelica.
That half of the partition was **derived from the equations**, and Act 2 is where you can see
which ones.

---

## Act 2 — What makes a variable a state?

You met the rule this morning in another pane: a variable is a state exactly when some equation
**differentiates** it. The equation sheet's **Why** column names the equation that did it.

> **Predict.** `SingleInertia` has two equations. Which one makes `phi` a state, and which makes
> `w` one?

[▶ Look — SingleInertia → Flatten → Equations](hrw://load/SingleInertia/Flatten/EquationSheet)

**Expected:** exactly two rows in the Why column are non-blank —

| variable | Why | the equation |
|---|---|---|
| `phi` | `der in f_x[0]` | `der(phi) - w` |
| `w` | `der in f_x[1]` | `J * der(w) - tau` |

**Falsified if:** a third variable carries a `der in …`, or `J` or `tau` does.

*What just happened.* **The classification is a property of the equations, not of the
declarations.** `phi` and `w` are declared identically to any other `Real`; what makes them
states is that `der()` is applied to them somewhere in the model.

That is why this phase must run *after* flattening. Before flattening, some of those `der()`
calls are inside components you never see, and the connection equations that tie components
together do not exist yet. Only a flat equation list can answer *is this variable
differentiated?* for the whole model at once.

---

## Act 3 — What is the solver actually solving for?

Here is the part that usually surprises people. Structural analysis lists the system's
**unknowns**, and the model has four variables, two of which are states.

> **Predict.** Name the unknowns you expect to see listed. Write them down before looking.

[▶ Look — SingleInertia → Structural → Tree](hrw://load/SingleInertia/Structural/Tree)

[Point at `incidence.unknown_names`](hrw://stage/Structural/Tree/node/incidence.unknown_names)

**Expected:** two unknowns, and they are **`der(phi)` and `der(w)`** — the *derivatives*, not
`phi` and `w`.

**Falsified if:** the list contains `phi` or `w` undifferentiated, or contains `J` or `tau`.

*What just happened.* At any single instant, `phi` and `w` are **not unknown**. The integrator is
holding their current values — that is precisely what "carries the past" means. What it does not
know, and what it must be told at every step, is **how fast they are changing.**

So the system handed to the solver is not *"find phi and w"*. It is *"given phi and w right now,
find `der(phi)` and `der(w)`"*. Integrate those, and you have new values of `phi` and `w` a
moment later; repeat.

**A state is therefore two things at once** — a known value on the way in, and an unknown rate on
the way out. That double role is why states are counted separately from everything else, and it
is what the next act's count is really about.

---

## Act 4 — The claim

Every downstream phase assumes one thing about this system.

> **Predict.** How many equations and how many unknowns will Structural report?

[▶ Look — SingleInertia → Structural → Tree](hrw://load/SingleInertia/Structural/Tree)

[Point at `n_unknowns`](hrw://stage/Structural/Tree/node/n_unknowns)

**Expected:** `n_equations` is **2** and `n_unknowns` is **2**.

**Falsified if:** the two numbers differ.

*What just happened.* **Square** means one equation per unknown. It is a necessary condition for
a well-posed problem, not a sufficient one — a square system can still be unsolvable, which is
what `blt-ordering.md` and `structural-vs-numerical-rank.md` are about.

But the *count* is checkable immediately, cheaply, and before any hard work. Rumoca checks it
here, at the end of DAE construction, and that timing is the subject of the last act.

---

## Act 5 — What the compiler says when the claim fails

`UnbalancedShaft` is `SingleInertia` with one extra line:

```modelica
Real tau "drive torque — NO EQUATION DETERMINES THIS, and that is the point";
```

`tau` is declared and never assigned. It is the most common Modelica authoring error there is:
declare a variable, forget its equation.

> **Predict.** The DAE tab is about to open on a model that cannot be built. What will it show —
> a partition with something missing, or something else?

[▶ Look — UnbalancedShaft → DAE](hrw://load/UnbalancedShaft/Dae)

**Expected:** **no partition at all.** The stage holds an error, reading:

```
unbalanced model: 2 equations, 3 unknowns (balance = -1)
```

with `n_equations: 2`, `n_unknowns: 3`, `balance: -1`, and the reading *"fewer equations than
unknowns — some variable has nothing to determine it"*.

**Falsified if:** the DAE tab shows `x`, `y` and `p` for this model, or reports a balance other
than −1.

*What just happened.* **Absence is stated rather than filled in.** There is no partial DAE to
look at, because a partition of an unbalanced system would be a fiction — and the pane says so
instead of showing you a plausible-looking one.

`balance = -1` is the arithmetic: two equations, three unknowns. The sign tells you which
mistake it is. Negative means *not enough equations* — a variable with nothing to determine it,
which is this model. Positive would mean *too many* — over-constrained, a different bug entirely.

---

## Act 6 — Where it fails, and why that is the right place

> **Predict.** Structural analysis is the phase that finds singular systems. Will it report this
> one?

[▶ Look — UnbalancedShaft → Structural](hrw://load/UnbalancedShaft/Structural)

**Expected:** Structural, Index reduction, Initialization, Events and Solve lowering all read
**`not reached (ToDae failed earlier)`**.

**Falsified if:** Structural reports a singularity, or any later stage produced a result.

*What just happened.* The balance check runs **before** matching, so a missing equation is
reported *as a missing equation* — at the phase that noticed, naming the count.

Had it run later, the same bug would have surfaced as a **structural singularity**: true, much
harder to act on, and pointing at the matching algorithm rather than at your model. The earlier
and more specific diagnosis is the better one, and the specimen exists to demonstrate exactly
that difference. `CapacitorLoop` is its mirror image — balanced by count, singular by structure —
and that is the case Structural *is* for.

---

## Two excursions, if you want them

Neither is required, and both leave HRW. They are here because at these two points a symbolic
result and a real trajectory answer better than a paragraph.

- [Open the notebook](hrw://notebook/dae-balance.nb) — the balance argument in Wolfram, where
  you can add and remove equations and watch the count move.
- [Open SingleInertia in System Modeler](hrw://systemmodeler/SingleInertia) — what "state" looks
  like when it runs: `phi` and `w` as trajectories rather than as list entries.

---

## What this tour cannot check

**Whether Act 3 lands.** The `der(phi)`/`der(w)` result is the one genuinely counter-intuitive
thing here, and it is asserted in one line. If it reads as a technicality rather than as the
point, the act is too short rather than wrong.

**Whether the DAE tree is legible.** Acts 1 and 5 send you to a generic serde tree. Whether `x`,
`y` and `p` read as a partition — or as three collapsed nodes among thirty — is a rendering
question no test reaches.

**Whether Act 6's contrast with `CapacitorLoop` is worth a stop of its own.** It is asserted in
prose here and demonstrated nowhere in this tour.

---

## What comes next in the chain

The system is square, and the solver has been told which quantities carry the past. It still does
not know **which equation to use for which unknown**, or **what order to solve them in** — a
square system does not come with an assignment.

That is matching and BLT ordering: [`matching.md`](matching.md), then
[`blt-ordering.md`](blt-ordering.md).
