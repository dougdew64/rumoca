# Fixture tour — Tearing: guess one number, get the rest for free

[▲ The chain overview](hrw://tour/the-mathematics)

**A curriculum tour.** Walk [`blt-ordering.md`](blt-ordering.md) first — it produces the coupled
blocks this tour tries to shrink.

Every count below was read from the committed traces, never remembered.

---

## The problem this phase exists to solve

BLT ordering ended with some blocks that must be solved simultaneously. A block of size *n* means
handing *n* equations in *n* unknowns to a numerical solver, and Newton's cost grows faster than
linearly in *n* — so a smaller block is not a tidiness preference, it is the difference between a
model that simulates and one that crawls.

But a block's *size* is not fixed. Here is the trick, and it is worth seeing before the machinery:

> If you **guess** one of the unknowns, several of the others may follow by direct substitution —
> and then one leftover equation tells you whether the guess was right.

So the solver iterates on the guess alone. A 3×3 simultaneous solve becomes a 1×1 one.

**Tearing is the phase that chooses what to guess.** Five acts: the trick, the choice being made,
two blocks torn independently, all three kinds of block in one model, and what it costs once time
is moving.

---

## Act 1 — Guess one number and the rest falls out

`ProportionalLoop` gave you one coupled block of size 3, on `error`, `command` and `measurement`:

```
f_x[0]   0 = error - (reference - measurement)
f_x[1]   0 = command - controllerGain * error
f_x[2]   0 = measurement - plantGain * command
```

> **Predict.** Of the three unknowns, how many must the solver iterate on?

[▶ Look — ProportionalLoop → Structural → Tearing](hrw://load/ProportionalLoop/Structural/TearingAnim)

**Expected:** **one** — the tear variable is `command`, and the residual equation is `f_x[0]`.

**Falsified if:** two or three tear variables are reported, or the residual is `f_x[1]` or
`f_x[2]`.

*What just happened.* Follow it by hand, because the mechanism is the lesson:

1. **Guess** `command`.
2. `f_x[2]` gives `measurement` directly — it is `plantGain * command`.
3. `f_x[0]` … is held back. It becomes the **residual**: with `error` computed from `f_x[1]`, this
   last equation is the check, and it will not balance unless the guess was right.

So the run-time problem is *"find the one number `command` that makes `f_x[0]` come out zero"* —
one unknown, not three. **The block is still coupled; it is just cheaper.**

---

## Act 2 — Watch the choice being made

The tear variable was not arbitrary. Step the animation rather than reading its result.

> **Predict.** What would make one variable a better guess than another?

[▶ Look — ProportionalLoop → Structural → Tearing](hrw://load/ProportionalLoop/Structural/TearingAnim)

**Expected:** the frames carry, per candidate, how many equations it **appears** in and which
other variables it **competes** with — and `command` is chosen on those numbers.

**Falsified if:** the frames show a result and no candidates.

*What just happened.* **A variable that appears in many equations unlocks many substitutions**, so
guessing it collapses more of the block. That is the heuristic, and the frames show it being
applied rather than asserting it.

It is a *heuristic*, and that word is doing real work: choosing the tear set that minimises
iteration is NP-hard in general, so every Modelica compiler uses a greedy rule and none claims
optimality. Act 5 is about what that costs.

---

## Act 3 — Two blocks, torn independently

`TwoLoops` gave you two coupled blocks of size 2 rather than one of size 4.

> **Predict.** How many tear variables in total?

[▶ Look — TwoLoops → Structural → Spy plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** **two, one per block** — `errorA` with residual `f_x[0]`, and `errorB` with residual
`f_x[2]`.

**Falsified if:** one tear variable serves both blocks, or either block reports two.

*What just happened.* Tearing happens **per block**, after decomposition, and the two phases
compose: BLT already established that these are two 2×2 problems, and tearing shrinks each to
1×1 independently. Nothing about loop B's tear can affect loop A's.

The run-time shape is now: iterate one number, then iterate one number. Compare that with the
4×4 Newton solve a compiler without either phase would have produced.

---

## Act 4 — All three kinds of block in one model

`MixedLoop` is 5 equations, and it is the most realistic model in this tour.

> **Predict.** How many blocks, and of what kinds?

[▶ Look — MixedLoop → Structural → Spy plot](hrw://load/MixedLoop/Structural/SpyPlot)

**Expected:** **three** blocks — a scalar one, then a coupled block of **3** torn on `command`
with residual `f_x[1]`, then another scalar one.

**Falsified if:** the coupled block is a different size, or there are not exactly three blocks.

*What just happened.* This is what a real compiled model looks like: a little straight-line work,
a knot in the middle, a little more straight-line work. `f_x[0]` computes `setpoint` from the
reference; the loop is solved; `f_x[4]` computes `result` from `measurement`.

**The knot is the only expensive part, and it is one third of the model.** That ratio is the
argument for the whole phase: on a large model the coupled blocks are usually a small minority of
the equations, and tearing shrinks them further.

---

## Act 5 — What it costs once time is moving

Every specimen so far is **timeless** — no states, so each block is solved once and the model is
finished. That is a simplification, and it hides what the phase is really deciding.

`LoopWithInertia` is `ProportionalLoop` with the inertia restored: the same cycle, plus a state.

> **Predict.** The block is torn on one variable, as in Act 1. How many times will that torn block
> be solved over a simulation?

[▶ Look — LoopWithInertia → Structural → Spy plot](hrw://load/LoopWithInertia/Structural/SpyPlot)

**Expected:** **two** blocks — a coupled block of **3** torn on `error` with residual `f_x[1]`,
and a **scalar** block holding `f_x[0]`, which is the equation containing `der(w)`.

**Falsified if:** no scalar block appears, or no variable in this model carries a derivative.

*What just happened.* There is a state, so the integrator takes a step, and a step, and a step —
and **the torn block is re-solved between every pair of steps, for the whole run.**

That reframes Acts 1 to 4. Tearing is not a compile-time tidy-up whose benefit you count once. The
choice of *which single variable to guess* is a decision about the innermost loop of the
simulation, executed thousands of times. A greedy choice that is one variable worse than optimal
is one extra unknown in every Newton iteration of every time step.

**Which is why the phase bothers, and why "heuristic" in Act 2 is uncomfortable rather than
reassuring.**

---

## What this tour cannot check

**Whether the tearing animation shows the choice as a choice.** The frames carry `appearances` and
`competitors` precisely so Act 2 can ask *"why that variable?"* — but whether those numbers are
visible and legible on screen is the half no test reaches.

**Whether Act 4 is the right ending or the right beginning.** `MixedLoop` is the most realistic
model here and arguably the point; it is placed fourth because the machinery has to be understood
first, and that ordering may be exactly backwards for a reader who wants to know what a compiled
model looks like before learning how it got that way.

**Whether Act 5's claim about cost is believable without a measurement.** It asserts thousands of
re-solves and shows none. A profile would settle it and there is no profiler here.

---

## What comes next in the chain

Every model in this tour was already index-1: the states were independent and the system solvable
once ordered. That is not guaranteed. Connect two rotating bodies with an ideal gear and the
compiler will find *more states than degrees of freedom*, which no amount of ordering or tearing
can fix.

That is [`index-reduction.md`](index-reduction.md).

Or go back up: [▲ The chain overview](hrw://tour/the-mathematics)
