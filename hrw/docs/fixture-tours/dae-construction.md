# Fixture tour — DAE construction: the count that decides everything

**This is the first *curriculum* tour, and it is still a test.** The others verify an HRW
capability. This one teaches a step of the chain — `docs/compiler-phases/the-chain-of-problems.md`,
the leftmost item Doug asked to understand on 2026-08-03 — and every expectation below is
still violable, because a lesson built on a wrong number teaches the wrong thing.

Every value was read from `docs/specimen-notebook/{SingleInertia,UnbalancedShaft}/trace/`,
not remembered.

**The two models differ by one line.** That is the whole design: one variable's worth of
difference, so the concept is not buried in detail.

*(Rewritten 2026-08-03. The first version taught DAE construction from its neighbours because
HRW had no DAE tab — writing the tour is what exposed that. It now references the DAE
directly.)*

```modelica
// SingleInertia                    // UnbalancedShaft
parameter Real tau = 1.0;           Real tau;   // no equation determines this
```

**Notices appear in the status bar**, along the bottom of the window. Two stops expect one.

---

## Stop 1 — The DAE itself

[SingleInertia → DAE](hrw://load/SingleInertia/Dae)

**Expected:** the DAE tree, and a tab note reading
**`2 state(s), 0 algebraic(s), 2 continuous equation(s)`**.

This is the artifact the whole chain is organised around, and until 2026-08-03 HRW built it
and never showed it. Open `x` and `p`:

[Point at `x`](hrw://stage/Dae/node/x) — the **states**: `phi` and `w`.

[Point at `p`](hrw://stage/Dae/node/p) — the **parameters**: `J` and `tau`.

**A variable is a state because `der()` is applied to it**, not because it seems important.
`J` is every bit as necessary to the physics and is not a state, because nothing differentiates
it. That distinction is the partition, and the partition is DAE construction's job.

The names are the MLS Appendix B vocabulary, which is why they are single letters: `x` states,
`y` algebraics, `u` inputs, `p` parameters, `z`/`m` discretes. `y` is **empty** here — this
model has no variable that must be solved for at each instant without being differentiated.

## Stop 2 — The equations, and the count

[Point at `f_x`](hrw://stage/Dae/node/f_x)

**Expected:** two continuous equations — the residual form of `der(phi) = w` and
`J * der(w) = tau`.

`f_x` is the partition MLS calls `0 = f_x(v, c)`. **Two equations, two states.**

[SingleInertia → Structural → Tree](hrw://load/SingleInertia/Structural/Tree)

[Point at `n_unknowns`](hrw://stage/Structural/Tree/node/n_unknowns)

**Expected:** `n_unknowns` is **2**, matching `n_equations` beside it.

**This equality is the precondition for everything downstream.** Matching — the next chain
item — assigns *one equation to each unknown*, and no such assignment can exist unless the
counts agree. So DAE construction ends by making a claim: *here is a square system*. The rest
of the pipeline is entitled to assume it.

## Stop 3 — The same model, one unknown more

[UnbalancedShaft → Flatten → EquationSheet](hrw://load/UnbalancedShaft/Flatten/EquationSheet)

`UnbalancedShaft` is `SingleInertia` with `tau` changed from a parameter to an unknown, and no
equation added to determine it. **Two equations, three unknowns.**

**Expected:** the Flatten tab is **red**, and carries the note:

> `unbalanced model: 2 equations, 3 unknowns (balance = -1)`

Read that as the count from Stop 2 failing by exactly one. Nothing about the physics is
wrong — a shaft driven by an unspecified torque is a perfectly sensible *question*. It is
not a **simulable model**, and this is the step that decides that.

**This is the most common Modelica authoring error there is**: declare a variable, forget its
equation. `docs/specimen-notebook/UnbalancedShaft/purpose.md` records that it also taught
something in passing — Rumoca's balance check fires *before* structural analysis, which is
earlier and more specific than the structural singularity Claude had predicted.

## Stop 4 — Where the chain stops

[UnbalancedShaft → Structural → Tree](hrw://load/UnbalancedShaft/Structural/Tree)

**Expected:** nothing to show — Structural produced no IR for this model, and a notice in the
status bar says so.

**That is the lesson, not a defect.** Structural analysis is *entitled* to a square system.
Handed a system that is not square, the honest thing is to refuse rather than to produce a
matching over the wrong number of unknowns. The chain is a sequence of contracts, and this is
the first one being enforced.

## Stop 5 — What the DAE does not yet tell you

Go back to [SingleInertia → DAE](hrw://load/SingleInertia/Dae) and open `metadata`.

[Point at `metadata`](hrw://stage/Dae/node/metadata)

**Expected:** `is_partial`, `class_type`, and `variable_starts` — carrying the `start` value of
each of `J`, `tau`, `phi`, `w`. All of it describes **what was declared**, and none of it
describes **how any particular equation came to be**.

The DAE is a *result*. It says `phi` is a state; it does not say which flat equation caused
that, or in what order the partitioning happened. **That is the difference between a boundary
IR and a phase's internals** — everything here is what DAE construction *produced*, not what
it *did*.

The stages that do show their work — matching, Tarjan, tearing — show it as **frames**, and
`examples/frame_index` will tell a tour author which frame handles which identifier. DAE
construction has no such replay, and whether it needs one is exactly the sort of thing this
tour exists to find out.

---

## What this cannot check

Whether the equation sheet's classification is *legible* — whether "state" and "parameter"
read as different kinds of thing at a glance, or as two rows in a table. Whether the red
Flatten tab draws the eye before the note is read. And whether Stop 5's absence is noticeable
at all, or whether a missing tab is exactly the kind of thing nobody sees.

## What comes next in the chain

`SingleInertia` is index-1: the square system is immediately solvable. **`Drivetrain` is not** —
its ideal gears constrain two inertias to move together, so a state turns out not to be
independent, and the count being right is no longer enough. That is index reduction, and it is
the next tour.
