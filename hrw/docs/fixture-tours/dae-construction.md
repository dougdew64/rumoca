# Fixture tour — DAE construction: the count that decides everything

**A curriculum tour.** Most tours here verify an HRW capability; this one teaches a step of the
chain (`docs/compiler-phases/the-chain-of-problems.md`) and uses HRW as the instrument rather
than the subject. It is **still a test** — every `**Expected:**` line is violable, and a lesson
built on a wrong number teaches the wrong thing.

**The prose is load-bearing** (Doug's instruction, 2026-08-03). Stops are not captions on a UI;
they are the explanation, and the UI is the evidence. Two stops leave HRW entirely, for Wolfram
and System Modeler, because at those two points a symbolic result and a trajectory answer
better than any paragraph could.

Every number below was read from `docs/specimen-notebook/{SingleInertia,UnbalancedShaft}/trace/`
or from the Rumoca source, never remembered. **Notices appear in the status bar**, along the
bottom of the window.

---

## The problem this phase exists to solve

Before the first stop, the thing worth having in mind.

You have just come out of **flattening**, which took a hierarchy of components — instances
inside instances, connectors joined by `connect` — and crushed it into one flat namespace with
one flat list of equations. What comes out is faithful to what you wrote, and it is still
written in *your* vocabulary: named variables, `der()` calls, equations with a left and a right
side.

A numerical integrator cannot use that. An integrator wants a very specific shape. At each
instant it knows where the system currently *is*, and it needs to be told how fast everything
is *changing*, so it can take a small step forward and repeat. It needs, at minimum:

- which quantities carry a value forward through time (**states**),
- which are fixed for the whole run (**parameters**),
- which must be solved for at each instant without being differentiated (**algebraics**),
- and the residual functions relating them.

Nothing in flat Modelica says which is which. `parameter Real J` and `Real phi` are two
declarations; the distinction that matters to the integrator — *is this differentiated?* — is a
property of the **equations**, not of the declarations.

**DAE construction is the phase that makes that classification.** It partitions every variable
and every equation into the categories the Modelica Language Specification names in Appendix B,
and then it makes a claim about the result: *this system is square*. Everything downstream is
entitled to assume that claim.

This tour walks the classification, then the claim, then what happens when the claim fails.

---

## Stop 1 — The DAE itself

[SingleInertia → DAE](hrw://load/SingleInertia/Dae)

**Expected:** the DAE tree, and a tab note reading exactly
**`2 state(s), 0 algebraic(s), 2 continuous equation(s)`**.

That note is the whole phase in one line, and it is worth pausing on the fact that you can read
it without opening anything.

The tree's top-level keys look cryptic — `x`, `y`, `u`, `p`, `z`, `m`, `f_x`, `f_z`, `f_m`,
`f_c` — and they are single letters for a reason: **they are the Modelica Language
Specification's own notation, from Appendix B.** Rumoca is not inventing a vocabulary here; it
is building the structure the specification describes, and naming the parts what the
specification names them. When you later read MLS §8.6 or a paper on DAE solvers, these are the
same symbols.

The ones that matter today:

| | Holds | Why it is its own category |
|---|---|---|
| `x` | states | differentiated, so the integrator carries them through time |
| `y` | algebraics | solved at each instant, never differentiated |
| `p` | parameters | fixed before the run starts |
| `f_x` | continuous equations | the residuals `0 = f_x(v, c)` |

[Point at `x`](hrw://stage/Dae/node/x)

**Expected:** exactly two entries, `phi` and `w`.

[Point at `p`](hrw://stage/Dae/node/p)

**Expected:** exactly two entries, `J` and `tau`.

Here is the sentence this stop exists for. **A variable is a state because `der()` is applied
to it — not because it seems important, and not because of how it was declared.**

`J` is every bit as necessary to the physics as `phi`. Delete it and the model is meaningless.
It is not a state, because nothing in the equation section differentiates it. That is the only
test, and it is a test on the *equations*.

[Point at `y`](hrw://stage/Dae/node/y)

**Expected:** empty.

Worth looking at precisely *because* it is empty. An algebraic variable is one that must be
solved for at every instant but is never differentiated — a pressure that follows instantly
from a flow, a force that follows instantly from a displacement. `SingleInertia` has none, and
that is why it is the baseline specimen: the smallest model where the partition has anything to
say at all. When `y` stops being empty in a later tour, that is the model acquiring constraints
that must be satisfied *now* rather than integrated.

---

## Stop 2 — What the solver is actually solving for

This is the stop most likely to correct something you did not know you assumed.

[SingleInertia → Structural → Tree](hrw://load/SingleInertia/Structural/Tree)

[Point at `incidence.unknown_names`](hrw://stage/Structural/Tree/node/incidence.unknown_names)

**Expected:** two entries, and they are **`der(phi)` and `der(w)`** — not `phi` and `w`.

The natural assumption is that a simulator solves for the variables. It does not. At any given
instant the integrator already **knows** `phi` and `w` — they are the state, that is what
"state" means, it carried them in from the previous step. What it does not know is how fast
they are changing *right now*. So the unknowns at each instant are the **derivatives**, and the
equations are a system in those derivatives.

Read the two equations again with that in mind:

```modelica
der(phi) = w;        // knowing w, this GIVES der(phi)
J * der(w) = tau;    // knowing J and tau, this GIVES der(w)
```

Each equation determines one derivative from things already known. Solve both, and you have the
full rate of change of the system; step forward; repeat. **That loop is what a simulation is**,
and DAE construction is what puts the equations into a shape where the loop is possible.

This also explains why the count that matters is *equations against unknowns* rather than
equations against variables. `SingleInertia` has four variables (`J`, `tau`, `phi`, `w`) and
two equations, and it is perfectly well-posed — because two of those four are parameters and
are never unknown.

---

## Stop 3 — The equations, and the claim

[SingleInertia → DAE](hrw://load/SingleInertia/Dae)

[Point at `f_x`](hrw://stage/Dae/node/f_x)

**Expected:** two entries, and the first has **`lhs: null`** with its `rhs` a subtraction whose
left operand is `Der(phi)`.

That `lhs: null` is not a gap in the data. It is the **residual form**, and it is the second
shape change this phase makes.

You wrote `der(phi) = w`. The DAE stores `der(phi) - w`, with nothing on the left, because the
left side is understood to be zero. Every equation becomes an expression that must **evaluate
to zero** when the values are right. This is what MLS Appendix B writes as `0 = f_x(v, c)`, and
it is why the whole thing is called a system of *residuals*: what you compute is how far from
satisfied each equation currently is.

The reason is uniformity. `der(phi) = w` and `J * der(w) = tau` look like different kinds of
statement — one is an assignment-shaped definition, the other a balance. A numerical solver
wants neither; it wants a vector-valued function it can evaluate and drive to zero. Residual
form erases a distinction that was never real, and every equation in the system becomes the
same kind of object.

Now the claim.

[SingleInertia → Structural → Tree](hrw://load/SingleInertia/Structural/Tree)

[Point at `n_unknowns`](hrw://stage/Structural/Tree/node/n_unknowns)

**Expected:** `n_unknowns` is **2**, and `n_equations` immediately beside it is also **2**.

**This equality is the precondition for everything downstream, and it is the last thing DAE
construction does.** The next item in the chain is *matching* — assigning one equation to each
unknown, so the system can be solved in an order rather than all at once. No such assignment
can possibly exist unless the counts agree: with fewer equations than unknowns some unknown
gets nothing; with more, some equation goes unused and is either redundant or contradictory.

So DAE construction ends by handing the rest of the pipeline a promise: *here is a square
system*. The next stop is what happens when it cannot make that promise.

---

## Stop 4 — The counterexample, and what the compiler says

[UnbalancedShaft → DAE](hrw://load/UnbalancedShaft/Dae)

**Expected:** the DAE tab is **red** and carries the note
**`unbalanced model: 2 equations, 3 unknowns (balance = -1)`**. There is no DAE tree, and the
tab explains why instead of being blank.

`UnbalancedShaft` is `SingleInertia` with one line changed. `tau` went from `parameter Real tau
= 1.0` to plain `Real tau`, and no equation was added to determine it.

[Show the line](hrw://source/18)

**Expected:** line 18, `Real tau "drive torque — NO EQUATION DETERMINES THIS, and that is the
point"`.

One word deleted — `parameter` — and one unknown appears. The equations are untouched; there
are still two. Two equations, three unknowns.

[Point at `error`](hrw://stage/Dae/node/error)

**Expected:** `error_code` is **`rumoca::todae::ED001`**, `n_equations` is **2**, `n_unknowns`
is **3**, `balance` is **-1**, and `reading` says *"fewer equations than unknowns — some
variable has nothing to determine it"*.

Several things here are worth naming.

**The error comes from DAE construction, not from structural analysis.** It is defined in
`crates/rumoca-phase-dae/src/errors.rs` as `ToDaeError::Unbalanced`, and its help text cites
**MLS §4.9**, which is where the specification defines a balanced model. This is good compiler
behaviour and it is why this specimen exists separately from `CapacitorLoop`: a *missing
equation* is caught by counting, which is cheap, before any structural analysis runs. You get
"you forgot an equation" rather than "the system is singular", and the first is far more
actionable.

**Nothing about the physics is wrong.** A shaft driven by an unspecified torque is a
completely sensible thing to wonder about. It is a perfectly good *question*. It is not a
**simulable model**, and this is the phase that decides which of those it is.

**This is the most common Modelica authoring error there is** — declare a variable, forget its
equation. It is common precisely because the model looks finished: every line is
well-formed, the types check, flattening succeeds. Nothing is *wrong* anywhere you would think
to look. Only the count catches it.

Now walk one tab further:

[UnbalancedShaft → Structural → Tree](hrw://load/UnbalancedShaft/Structural/Tree)

**Expected:** no IR, and the note reads **`not reached (ToDae failed earlier)`**.

**That is the lesson, not a defect.** Structural analysis is *entitled* to a square system —
the promise from Stop 3 is a precondition it is allowed to rely on. Handed a system that is not
square, the honest response is to refuse rather than to produce a matching over the wrong
number of unknowns. The chain is a sequence of contracts, and this is the first one being
enforced.

---

## Stop 5 — Why "2 equations, 3 unknowns" is not "no solution"

[Open the notebook](hrw://notebook/dae-balance.nb)

**Expected:** Wolfram Desktop opens `dae-balance.nb`. Evaluate the cells in order.

This stop leaves HRW because prose here would be asking you to take my word for something you
have every reason to misremember — and one line of Wolfram settles it.

The natural way to hear `balance = -1` is *"the equations are inconsistent; there is no
solution."* **That is exactly backwards.** Ask Wolfram to solve the same two equations for all
three unknowns and the answer is not empty. It is `{dphi -> w, tau -> dw J}` — a solution for
*any value of `dw` you choose*. Pick any angular acceleration you like; there is a torque that
produces it.

The problem is not that there is no answer. **The problem is that there are infinitely many and
nothing chooses between them.** A simulator cannot proceed, not because it is stuck, but
because it would have to invent a torque — and a simulation that silently invents physics is
worse than one that refuses.

Wolfram says so in its own vocabulary, which is worth seeing because it is an entirely
independent implementation reaching Rumoca's conclusion:

```
Solve::svars: Equations may not give solutions for all "solve" variables.
```

Rumoca calls it `balance = -1`. Wolfram calls it `svars`. Same finding.

**Then the notebook connects it to linear algebra**, which is the part likely to stay with you.
Write the system as a matrix acting on the unknown vector:

- balanced, unknowns `{dphi, dw}` — a 2×2 matrix, rank 2, **nullspace empty**
- unbalanced, unknowns `{dphi, dw, tau}` — a 2×3 matrix, rank 2, **nullspace one-dimensional**,
  spanned by `{0, -1, -J}`

That nullspace vector *is* the ambiguity, written as a vector. It says: leave `dphi` alone,
change `dw` by one unit, change `tau` by `J` units, and both equations still hold exactly —
which is just `tau = J·dw` restated, with nothing to pin down either side.

And the identity worth carrying into every later phase:

> **balance = −(nullity)**, when the equations are independent.

Rumoca's integer is reporting the **dimension of the space of solutions it would have had to
choose from**. By rank-nullity, `rank + nullity = 3`: two independent equations kill two of the
three degrees of freedom, and the survivor is `tau`.

The notebook also states two caveats honestly — these systems are linear and real ones usually
are not (in general it is the *Jacobian* whose rank matters, evaluated at a point), and
Rumoca's actual check is far cheaper than any of this: it compares two integers and never
builds a matrix. That cheapness is deliberate and correct, and it has a price — counting cannot
see a system that is **square but structurally singular**. That failure is real, it has its own
specimen (`CapacitorLoop`), and it is caught later by structural analysis.

**Counting is a necessary condition, not a sufficient one.** That is precisely why there is
another phase after this one.

---

## Stop 6 — What "state" looks like when it runs

[Open SingleInertia in System Modeler](hrw://systemmodeler/SingleInertia)

**Expected:** System Modeler opens `SingleInertia.mo`. Simulate to `t = 2` and plot `phi`, `w`,
and `J`.

**Expected values, from `DSolve` in the notebook:** `w(t) = t` and `phi(t) = t²/2`, so at
`t = 2` you should read `w = 2.0` and `phi = 2.0` exactly; at `t = 1`, `phi = 0.5`.

Stop 1 asserted that `phi` and `w` are states and `J` is not. Prose can only restate the
definition, and a restated definition is not understanding. **This makes it ostensive.**

`phi` and `w` trace curves. `J` is a flat line. A state is *the thing that carries a value
forward through time* — that is what the integrator is integrating, and it is why `der()`
applies to those two and to nothing else. `J` matters to the physics exactly as much, and has
no trajectory at all.

Constant torque on constant inertia gives constant angular acceleration, so velocity ramps
linearly and angle goes quadratic. Nothing surprising — and that is the point. The partition
HRW showed you as two lists of names is *this*, and now you have seen both faces of it.

**The second reason to run it here** is that System Modeler is an independent implementation.
When a later tour has us disagreeing with Rumoca about something subtle, this is the
adjudicator, and it is worth having used it on a model where the answer is not in doubt.

---

## Stop 7 — What the DAE does not tell you

[SingleInertia → DAE](hrw://load/SingleInertia/Dae)

[Point at `metadata`](hrw://stage/Dae/node/metadata)

**Expected:** `is_partial`, `class_type`, and `variable_starts` — the last carrying the `start`
value of each of `J`, `tau`, `phi`, `w`. All of it describes **what was declared**. None of it
describes **how any particular equation came to be**.

The DAE is a **result**. It says `phi` is a state; it does not say which flat equation caused
that conclusion, or in what order the partitioning happened, or what was considered and
rejected. That is the difference between a **boundary IR** — the artifact a phase hands to the
next one — and a phase's **internals**.

The distinction matters for what tours can do. Stages that show their work — matching, Tarjan,
tearing — expose it as **frames**, replayable step by step, and `examples/frame_index` tells a
tour author which frame handles which identifier. DAE construction has no such replay. Whether
it needs one is exactly the kind of question this tour exists to raise: **if walking this left
you wanting to watch the partition being made rather than reading it after the fact, that is a
feature request, and a well-evidenced one.**

One thing `metadata` *does* carry that will matter two phases from now: `variable_starts`. Those
`start` values are not initial conditions yet — they are candidates. Turning them into a
consistent initial state is the **initialization** phase's job, and it is a harder problem than
it looks, because the `start` values you wrote may not satisfy the equations.

---

## What this tour cannot check

Whether the DAE tree is *legible* — whether `x` and `p` read as different kinds of thing at a
glance, or as two folders that happen to have different names. Whether the red tab draws the eye
before the note is read. Whether the MLS single-letter names help (they connect to the
literature) or hinder (they are opaque on first meeting), which is a genuine trade-off I cannot
resolve from this side.

And most of all: **whether the two excursions were worth leaving HRW for**, or whether the break
in flow cost more than the symbolic result and the trajectory bought. That is the calibration
question for this tour, and it is the one thing the test suite cannot answer.

## What comes next in the chain

`SingleInertia` is **index-1**: the square system from Stop 3 is immediately solvable, and
counting was enough.

**`Drivetrain` is not.** Its ideal gears constrain two inertias to move together — so a variable
that *looks* like an independent state turns out not to be. The counts still agree. The system
is still square. And it still cannot be integrated, because a constraint among the states means
the state vector has fewer genuine degrees of freedom than it has entries.

That is **index reduction**, it is the next tour, and Stop 5's rank-nullity framing is the tool
it will be built on: this time the deficiency is not in the count but in the *rank*.

**A head start already exists.** [`structural-vs-numerical-rank.md`](structural-vs-numerical-rank.md)
walks a model with **full structural rank and numerical singularity** — the counting argument
and even the connectivity argument both satisfied, and the system still unsolvable. It was
written as a capability tour, so it is short and does not teach the phase; but it is the same
distinction Stop 5 opened, taken one step further, and it is worth walking before the index
reduction tour exists.
