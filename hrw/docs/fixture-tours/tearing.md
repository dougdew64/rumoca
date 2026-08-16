# Tearing — turning a 3×3 solve into a 1×1 one

**Walk [`blt-ordering.md`](blt-ordering.md) first.** It ended with a coupled block of three
equations and the observation that a block says *"solve these together"* while saying nothing
about **how**. This tour is the how.

**And this is the first algorithm in the phase that is a *heuristic*.** Matching finds a maximum
— provably. Tarjan finds the components — provably. Tearing makes a **greedy guess**, and can be
beaten. That difference is worth as much as the technique.

> Every variable name and sequence below is read from the specimens' generated traces.

---

## The problem this step exists to solve

`ProportionalLoop`'s block holds three equations and three unknowns — `command`, `error`,
`measurement` — with no order among them. The honest reading is: give all three to a nonlinear
solver, which will build a **3×3 Jacobian** and factor it at every Newton iteration.

For three equations that is cheap. For a coupled block of 300 — which real thermo-fluid models
produce — it is not, because dense factorisation costs O(n³).

**So the question is whether the block is really as coupled as it looks.** Usually it is not:
most of those equations are perfectly explicit once you know *a few* of the values, and only a
handful are genuinely simultaneous.

---

## Act 1 — Guess one number and the rest falls out

[ProportionalLoop → Structural → Spy-plot](hrw://load/ProportionalLoop/Structural/SpyPlot)

**Expected:** one outlined box covering the whole 3×3 — one coupled block, size 3, unknowns
`command`, `error`, `measurement`.

**Hover the box.** The tearing report is in the hover text, not in a pane of its own — that is
where the three rows below are read from.

| | |
|---|---|
| **tear variable** | `command` |
| **causal** | `f_x[1]` → `error`, then `f_x[2]` → `measurement` |
| **residual** | `f_x[0]` |

**Expected:** exactly one tear variable, two causal assignments and one residual equation.

Read as a procedure, that is:

1. **Guess** `command`.
2. `f_x[1]` now has only one unknown left — solve it directly for `error`.
3. `f_x[2]` now has only one unknown left — solve it directly for `measurement`.
4. `f_x[0]` has nothing left to solve; whatever it evaluates to is the **residual**.
5. Hand the solver *one* number to vary (`command`) and *one* residual to drive to zero.

**The 3×3 solve became a 1×1 solve.** Two of the three equations are no longer part of the
iteration at all — they are ordinary assignments evaluated inside it. The Jacobian went from nine
entries to one.

**Nothing was approximated.** The answer is identical; only the shape of the work changed. That
is the property that makes tearing worth doing rather than a trade-off to weigh.

---

## Act 2 — Watch the choice being made

[ProportionalLoop → Structural → Tearing](hrw://load/ProportionalLoop/Structural/TearingAnim)

The algorithm records a reason for every step, not just the outcome. The vocabulary is in
`crates/rumoca-phase-structural/src/tearing.rs`:

| step | what it means |
|---|---|
| `Causal { equation, variable, competitors }` | an equation had **exactly one** unknown left, so it is solved directly — no guessing needed. `competitors` is how many equations *could* have solved that variable |
| `Torn { variable, appearances, remaining_equations }` | **nothing** had a single unknown left, so the loop must be cut. The chosen variable appears in `appearances` of the remaining equations — the most of any candidate |
| `Complete { tears, residuals }` | the size of the iteration the solver is left with |

**Expected:** the run ends with `Complete` reporting **1 tear** and **1 residual**.

**The greedy criterion is the whole algorithm:** when forced to cut, cut the variable that appears
in the most remaining equations. Removing it makes the most equations closer to being solvable
directly, so it is the locally best move.

**`competitors` is the interesting field.** When more than one equation could solve for a
variable, the tie is broken toward the equation with fewer total unknowns — the one that leaves
the least behind.

---

## Act 3 — Two blocks, torn independently

[TwoLoops → Structural → Spy-plot](hrw://load/TwoLoops/Structural/SpyPlot)

**Expected:** **two** outlined boxes on the diagonal, each 2×2 and not touching — two coupled
blocks of size 2, each torn to **1 tear, 1 residual** (hover each box for its report):

| block | tear | causal | residual |
|---|---|---|---|
| 0 | `errorA` | `f_x[1]` → `commandA` | `f_x[0]` |
| 1 | `errorB` | `f_x[3]` → `commandB` | `f_x[2]` |

**Expected:** neither block's tear variable appears in the other's causal sequence.

**Tearing happens per block, and that is the composition worth noticing.** BLT already proved
the two loops independent, so tearing never has to consider them together. Four equations that
began as a potential 4×4 solve are now **two separate 1×1 iterations** — and the reduction came
from two different algorithms, each doing its own job.

---

## Act 4 — All three kinds of block in one model

[MixedLoop → Structural → Spy-plot](hrw://load/MixedLoop/Structural/SpyPlot)

Five equations. **Expected:** a single cell, then a 3×3 outlined box, then a single cell — three
blocks, in this order:

| block | kind | content |
|---|---|---|
| 0 | scalar | `f_x[0]` → `setpoint` |
| 1 | **coupled, size 3** | tear `command`; causal `f_x[2]` → `error`, `f_x[3]` → `measurement`; residual `f_x[1]` |
| 2 | scalar | `f_x[4]` → `result` |

**This is what a compiled model actually looks like.** Not "a system to solve" but a *schedule*:
evaluate `setpoint` directly, then run a small iteration on one variable, then evaluate `result`
directly. Two of the five equations never enter a solver at all, and a third and fourth enter it
only as assignments inside the loop body.

**Expected:** block 2 comes after the coupled block, because `result` depends on values the loop
produces.

---

## Act 5 — The linear algebra: this is a Schur complement

Take the coupled block's 3×3 incidence and split the unknowns into the **tear** set *T* =
{`command`} and the **causal** set *C* = {`error`, `measurement`}. Order the equations so the
causal ones come first. The block becomes

$$
\begin{bmatrix} A & B \\ C & D \end{bmatrix}
\begin{bmatrix} x_C \\ x_T \end{bmatrix} = 0
$$

where **A is lower triangular** — that is exactly what "each causal equation has one unknown
left when it is reached" means, and it is why the causal sequence has an order at all.

Because *A* is triangular it is invertible by substitution, costing no factorisation. Eliminating
$x_C$ leaves

$$
(D - C A^{-1} B)\, x_T = 0
$$

and $D - CA^{-1}B$ is the **Schur complement** of *A*. It is 1×1 here.

**So tearing is block elimination, chosen structurally rather than numerically.** A general
solver would discover this by pivoting during factorisation, at runtime, on every iteration. The
compiler does it **once, symbolically, before any numbers exist** — and the generated code then
carries the smaller problem forever.

**And the residual equation is the Schur complement row.** `f_x[0]` is not "the leftover"; it is
the equation whose satisfaction certifies the whole block, once the others have been used to
define everything else in terms of `command`.

---

## Act 6 — Greedy, and what greedy costs

**Finding the *minimum* tear set is the hard part**, and Rumoca does not attempt it. The
algorithm is *greedy Cellier-style* tearing: solve everything that can be solved, cut the
most-connected variable when stuck, repeat.

**Greedy can be beaten.** A different cut can leave a smaller residual system, and nothing here
searches for one — every choice is made on local information, which is exactly why the animation
records `appearances` and `competitors` rather than just the outcome. A step with several equally
good candidates is a place where a different choice was available.

**This is the phase's only heuristic**, and worth holding onto: matching and BLT give answers
that are *correct by construction*, while tearing gives an answer that is *valid but not
necessarily best*. A torn block always computes the right numbers. It may simply iterate on more
of them than it had to.

**`NoProgress` is the honest failure**: when every equation references every unknown, no cut
shrinks anything, and the algorithm says so rather than tearing pointlessly.

---

## What comes next in the chain

The blocks and their tear structure are a **schedule**, and a schedule still has to become code.
Solve lowering turns it into residual programs, a layout of state and algebraic variables, and
the Jacobian sparsity the integrator will use — which is where `der(x)` finally becomes a number
the solver moves.

---

## OWED: a final act on `LoopWithInertia` — do this when converting the tour

**Every specimen in this tour is timeless.** `ProportionalLoop`, `TwoLoops` and `MixedLoop` have
no state, so each loop is torn and solved **once** and that is the whole story. The tour therefore
never confronts the question its own subject raises:

> **What does a coupled block cost when time is advancing?**

[`LoopWithInertia`](../specimen-notebook/LoopWithInertia/purpose.md) is `ProportionalLoop` with
the idealization removed — the same 3-cycle `command → measurement → error → command`, but now
with `der(w)` beside it. So the torn block is re-solved **between every pair of integrator steps,
for the whole simulation**. Tearing stops being a compile-time tidy-up and becomes a decision
about the inner loop of the run.

That reframes Act 1 rather than repeating it, which is why it is **one act and not a tenth tour**
(`README.md`: one tour per capability, narrow — the scarce resource is Doug's attention per
expectation).

**Doug, 2026-08-16, having asked whether the specimen deserved its own tour:** *"Eventually, I
will want very much to add LoopWithInertia to the tearing tour, as you've recommended. Please
ensure that we do that."* He is walking the tours in compiler-phase order and is on Connections →
DAE, so tearing is some way off — which is exactly why this is written here and enforced by
`doc_citations::the_tearing_tour_gains_its_dynamic_loop_when_it_is_converted` rather than left as
a promise in a conversation that scrolls away. That test fails the moment this tour is converted
to the Predict/Look/Falsified template without the act.

## What this tour cannot check

**Whether Act 5's matrices help.** It is the most mathematical page in any tour so far, and it
assumes the block-partition notation reads easily. If it does not, the Schur complement is better
introduced *after* your linear-algebra course reaches it rather than before.

**Whether the tearing animation shows the greedy choice as a choice.** The frames carry
`appearances` and `competitors` precisely so a viewer can ask "why that variable?" — but whether
those numbers are visible and legible on screen is the half no test reaches.

**Whether Act 4 is the right ending or the right beginning.** `MixedLoop` is the most realistic
model in this tour and arguably the whole point; it is placed fourth because the machinery has to
be understood first, and that ordering may be exactly backwards for a reader who wants to know
what a compiled model looks like before learning how it got that way.
