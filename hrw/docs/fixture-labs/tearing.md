# Fixture lab — Tearing: guess one number, get the rest for free

<!-- kind: concept -->

**A coupled block of size *n* means handing *n* equations in *n* unknowns to a numerical solver**,
and Newton's cost grows faster than linearly in *n*. So a smaller block is not a tidiness
preference — it is the difference between a model that simulates and one that crawls.

But a block's *size* is not fixed, and the reason is worth seeing before the machinery:

> If you guess one of the unknowns, several of the others may follow by direct substitution — and
> then one leftover equation tells you whether the guess was right.

The solver iterates on the guess alone. Tearing is the phase that chooses what to guess.

Run [blt-ordering](hrw://lab/blt-ordering) first — it produces the coupled blocks this lab tries
to shrink. Every count below is read from a generated trace, so if one disagrees with your screen,
the lab is wrong and I want to know.

---

## Station 1 — Guess one number and the rest falls out

`ProportionalLoop` gave you one coupled block of size 3, on `error`, `command` and `measurement`:

```
f_x[0]   0 = error - (reference - measurement)
f_x[1]   0 = command - controllerGain * error
f_x[2]   0 = measurement - plantGain * command
```

> **Predict.** Of the three unknowns, how many must the solver iterate on?

[Look — ProportionalLoop → Structural → Tearing](hrw://load/ProportionalLoop/Structural/TearingAnim)

**Expected:** one — the tear variable is `command`, and the residual equation is `f_x[0]`.
Below the animation, in amber, a line begins *"The run does not iterate on this plan. It tears
error"* — leave that for Station 2.

Falsified if: two or three tear variables are reported, or the residual is `f_x[1]` or
`f_x[2]`.

*What just happened.* Follow it by hand, because the mechanism is the lesson:

1. Guess `command`.
2. `f_x[1]` now has one unknown left, so it gives `error` directly — `command / controllerGain`.
3. `f_x[2]` gives `measurement` directly — it is `plantGain * command`.
4. `f_x[0]` … is held back. It becomes the residual: with `error` and `measurement` both
   computed, this last equation is the check, and it will not balance unless the guess was right.

That is the order Rumoca records, in the block's `tearing.causal_sequence`.

So the structural phase's problem is *"find the one number `command` that makes `f_x[0]` come out
zero"*: one unknown, not three. The block is still coupled; it is just cheaper. **But that is not
the problem the simulation solves**, and the reason is in step 2.

---

## Station 2 — What the run does with that plan

Step 2 above says `error = command / controllerGain`. That is a division by a parameter.

> **Predict.** Is there any value of `controllerGain` for which step 2 cannot be done? And if a
> compiler cannot rule that value out, what should it do?

[Look — ProportionalLoop → Structural → Spy plot](hrw://load/ProportionalLoop/Structural/SpyPlot),
then hover over the orange block.

**Expected:** the block's tooltip lists the structural plan (tear `command`, residual `f_x[0]`), and
then *"the run iterates on a different plan"*, with **run tear vars `error`** and **run residual
equations `f_x[2]`**. A note begins *"solve lowering re-tore this block"*.

Falsified if: the tooltip says *"the run iterates on this same plan"*, or the run's tear variable
is `command`.

*What just happened.* At `controllerGain = 0`, step 2 divides by zero. Nothing in the model forbids
that value, and Rumoca's rule is to take a causal step only with a **proof** that the number it
divides by is nonzero (`proven_tearing.rs`, SPEC_0043 §4). A parameter carries no such proof, so
solve lowering throws the structural plan away and tears the block again. It keeps only steps it
can prove:

1. Guess `error`.
2. `f_x[1]` gives `command = controllerGain * error`. This is a multiplication, and the unknown
   enters with coefficient 1 (`Unit`).
3. `f_x[0]` gives `measurement = reference - error`. The coefficient is −1 (`Literal`).
4. `f_x[2]`, `measurement = plantGain * command`, is the residual.

The run's plan is still one guess and one check, with the same size and cost as the structural
plan. It is simply safe for every parameter value. The proofs are on screen: the Structural tree's
`blocks[0].run_tearing.causal_sequence` names each step's `coefficient`, and those are the values
the simulation's Newton uses. *(Solve lowering's own tab holds the same plan in its
`continuous.algebraic_projection_plan`, as solver indices.)*

**This is why the structural phase's answer is a proposal.** It works from the sparsity pattern
alone (`structural-vs-numerical-rank` is about exactly that), and a pattern cannot tell
multiplying by a parameter from dividing by one. Solve lowering can. *(Until 2026-11-28 this lab
taught the structural plan as the run's, and HRW showed nothing else.)*

---

## Station 3 — Watch the choice being made

The tear variable was not arbitrary. Step the animation rather than reading its result.

> **Predict.** What would make one variable a better guess than another?

[Look — ProportionalLoop → Structural → Tearing](hrw://load/ProportionalLoop/Structural/TearingAnim)

**Expected:** a 🎲 frame *"Planning the 3-equation loop by cutting whichever unknown frees the
most equations to be solved directly"*, then ✂ *"Tore `command` — it appears in 2 of the 3
unsolved equations"*, then the causal steps (`error` from `f_x[1]`, then `measurement` from
`f_x[2]`), and a 🏁 frame *"Adopted the plan from the free-the-most pass: 1 tear and 1 residual
equation"*.

Falsified if: the tear frame names anything but `command`, or the plan adopted has two tears.

*What just happened.* The rule is: cut whichever unknown frees the most other equations to be
solved directly, and break a tie on how many unsolved equations it appears in. Guessing a
well-connected variable collapses more of the block. A second pass, cutting on appearances
first, runs only if the first needs more than one tear — here it does not, which is what the
*"free-the-most pass"* in the last frame tells you.

And notice what decided *this* choice: nothing. `ProportionalLoop` is a symmetric three-cycle,
so every unknown frees the same number of equations and appears in two of the three. On a full
tie Rumoca takes the unknown with the lowest index in the block, which is `command`. The frames
show the winner's numbers, not a table of every candidate's — so a tie like this one is visible
only by working the other two out yourself.

It is a *heuristic*, and that word is doing real work: choosing the tear set that minimises
iteration is NP-hard in general, so every Modelica compiler uses a greedy rule and none claims
optimality. Station 6 is about what that costs. *(And Station 2 is a second reason the greedy
answer is not final: the run keeps only the steps it can prove.)*

It is also the only heuristic you meet in these labs. Matching and Tarjan give exact answers:
if every equation can be paired, matching finds a pairing, and Tarjan's blocks are the only ones
that pairing allows. Tearing's choice can be beaten. But a worse tear still computes the same
values. It only iterates on more unknowns than it had to.

---

## Station 4 — Two blocks, torn independently

`TwoLoops` gave you two coupled blocks of size 2 rather than one of size 4.

> **Predict.** How many tear variables in total?

[Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** two, one per block — `errorA` with residual `f_x[0]`, and `errorB` with residual
`f_x[2]`. Hover over either block: the tooltip says *"the run iterates on this same plan"*.

Falsified if: one tear variable serves both blocks, or either block reports two.

*What just happened.* Tearing happens per block, after decomposition, and the two phases
compose: BLT already established that these are two 2×2 problems, and tearing shrinks each to
1×1 independently. Nothing about loop B's tear can affect loop A's.

And here the run keeps both plans, for Station 2's reason. Each block tears `error` and computes
`command = gain * error` causally, which is a multiplication with coefficient 1. There is nothing to
prove, so nothing to replace.

The run-time shape is now: iterate one number, then iterate one number. Compare that with the
4×4 Newton solve a compiler without either phase would have produced.

---

## Station 5 — All three kinds of block in one model

`MixedLoop` is 5 equations, and it is the most realistic model in this lab.

> **Predict.** How many blocks, and of what kinds?

[Look — MixedLoop → Structural → Spy plot](hrw://load/MixedLoop/Structural/SpyPlot)

**Expected:** three blocks — a scalar one, then a coupled block of 3 torn on `command`
with residual `f_x[1]`, then another scalar one. The coupled block's tooltip says the run
iterates on a different plan: **`error`**, residual **`f_x[3]`**.

Falsified if: the coupled block is a different size, or there are not exactly three blocks.

*What just happened.* This is what a real compiled model looks like: a little straight-line work,
a knot in the middle, a little more straight-line work. `f_x[0]` computes `setpoint` from the
reference; the loop is solved; `f_x[4]` computes `result` from `measurement`. The knot is
`ProportionalLoop`'s cycle again, so the run re-tears it for Station 2's reason.

The knot is the only expensive part: one of the three blocks, holding three of the five
equations. On a model this small the knot is most of it, and that ratio inverts with size — which
is the argument for the whole phase: on a large model the coupled blocks are usually a small
minority of the equations, and tearing shrinks them further.

---

## Station 6 — What it costs once time is moving

Every specimen so far is timeless — no states, so each block is solved once and the model is
finished. That is a simplification, and it hides what the phase is really deciding.

`LoopWithInertia` is `ProportionalLoop` with the inertia restored: the same cycle, plus a state.

> **Predict.** The block is torn on one variable, as in Station 2. How many times will that torn block
> be solved over a simulation?

[Look — LoopWithInertia → Structural → Spy plot](hrw://load/LoopWithInertia/Structural/SpyPlot)

**Expected:** two blocks — a coupled block of 3 torn on `error` with residual `f_x[1]`,
and a scalar block holding `f_x[0]`, which is the equation containing `der(w)`. The coupled
block's tooltip says the run iterates on this same plan: both its causal steps already have a
proof (`Literal` and `Unit`).

Falsified if: no scalar block appears, or no variable in this model carries a derivative.

*What just happened.* There is a state, so the integrator takes a step, and a step, and a step —
and the torn block is re-solved between every pair of steps, for the whole run.

That reframes Stations 1 to 5. Tearing is not a compile-time tidy-up whose benefit you count once. The
choice of *which single variable to guess* is a decision about the innermost loop of the
simulation, executed thousands of times. A greedy choice that is one variable worse than optimal
is one extra unknown in every Newton iteration of every time step.

Which is why the phase bothers, and why "heuristic" in Station 3 is uncomfortable rather than
reassuring. It is also why Station 2's proof matters: a division by zero in that inner loop does
not fail once, it fails at whatever instant the parameter lets it.

---

## What this lab cannot check

Whether the tearing animation shows the choice as a choice. The tear frame carries the winner's
`appearances`, and each causal frame its `competitors` (how many equations could have solved
that variable), so Station 3 can ask *"why that variable?"* — but whether those numbers are
visible and legible on screen is the half no test reaches.

Whether the amber line under the animation is noticed. It is drawn below the search, which is
where Station 1 sends you, but a reader stepping frames may never scroll to it. And whether the
run's re-tear deserves its own animation: the search Rumoca records is the structural phase's,
and the re-tear has no recorded frames, so HRW can show its result but not its steps.

Whether Station 5 is the right ending or the right beginning. `MixedLoop` is the most realistic
model here and arguably the point; it is placed fifth because the machinery has to be understood
first, and that ordering may be exactly backwards for a reader who wants to know what a compiled
model looks like before learning how it got that way.

Whether Station 6's claim about cost is believable without a measurement. It asserts thousands of
re-solves and shows none. A profile would settle it and there is no profiler here.

---

## What comes next in the chain

Every model in this lab was already index-1: the states were independent and the system solvable
once ordered. That is not guaranteed. Connect two rotating bodies with an ideal gear and the
compiler will find *more states than degrees of freedom*, which no amount of ordering or tearing
can fix.

That is [index-reduction](hrw://lab/index-reduction).
