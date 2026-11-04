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

## Station 2 — The case that needs nothing, and there are two of those

`BouncingBall` has two states, `h` and `v`. `StabilizedPendulum` has four, and a constraint.

Neither one costs reduction a single round of work. That is the only thing they have in common,
and the pane does not say the same thing about them.

> **Predict.** Two models, no reduction performed on either. Before you look: can a pane that
> reports *nothing happened* be telling you two different things? If it can, what is the second
> one?

[Look — BouncingBall → Index reduction → Summary](hrw://load/BouncingBall/IndexReduction/Summary)

**Expected:** `borrowed`, **0 rounds**, and the three counts identical on both sides — 2 states,
0 algebraics, 2 equations. The verdict line says *already index-1, nothing rebuilt*.

Falsified if: the outcome word is anything but `borrowed`, or any count differs across the two
rows of the grid.

[Look — StabilizedPendulum → Index reduction → Summary](hrw://load/StabilizedPendulum/IndexReduction/Summary)

**Expected:** also **0 rounds**, and the outcome is **`sorted`, not `borrowed`** — with the counts
*different*: 4 states both sides, but 1 algebraic becoming **3** and 5 equations becoming **7**.
The verdict line does **not** say index-1.

Falsified if: this one also reads `borrowed`, or its equation counts match across the grid.

### What just happened

A ball can be anywhere at any speed, so no equation relates `h` to `v` without a derivative in
it. There is no constraint to differentiate, the distance to index 1 is zero, and reduction hands
the system straight back. `borrowed` is that: *I was given a system that already sorted, and I
returned the same one.*

The pendulum's system also sorted — but it was **rebuilt on the way through**, and that is what
the second word records. Something gave it two more equations and two more algebraic unknowns
before anyone looked at its index. Not reduction: reduction ran zero rounds.

**The grid is how you can tell, and the word alone is not.** `sorted` is reported both by a run
that reduced for five rounds and by this one, which reduced for none. So the honest reading of
the headline needs the round count beside it — which is why the pane puts them on the same line
rather than leaving you to notice.

> What rebuilt it is **derivative-alias normalization**, a pass that runs before reduction is
> asked anything. It extends an implicit derivative system so that the rates it needs exist as
> coordinates — which *adds* equations rather than removing them. Rumoca does not expose that pass
> directly, and the only way to see that it ran is this: `borrowed` means untouched, `sorted` with
> zero rounds means rebuilt.

Two kinds of nothing, then. It is worth establishing both before the next station, so that *"the
counts did not move"* does not become your test for whether anything happened — because in
Station 5 it will be wrong.

---

## Station 3 — The smallest model that needs something, and it points at a line you did not write

`BenchActuator` is a motor driving a load. Four states, and one of them is not free.

> **Predict.** The compiler is about to demote a state, which means it found a constraint saying
> that state was never independent. **You wrote this model's `.mo` file.** When the pane tells you
> which constraint it used, which file do you expect it to name?

[Look — BenchActuator → Index reduction → Summary](hrw://load/BenchActuator/IndexReduction/Summary)

**Expected:** 4 states before, **3** after; one pass; the selected candidate's location is

```text
hrw/vendor/msl/Modelica 4.1.0/Electrical/Analog/Basic/RotationalEMF.mo:41
    phi = flange.phi - internalSupport.phi
```

Falsified if: the location is a file under `hrw/specimens/`, or more than one pass is listed.

### What just happened

**The constraint is in the Modelica Standard Library, not in your model.** You wrote
`emf`, `load` and a few connects; the MSL's `RotationalEMF` defines its shaft angle as an offset
from its flange's, and that single line is what makes one of your four states bookkeeping.

This is most of what index reduction does in practice. A model assembled from library components
inherits their internal definitions, and those definitions are where the redundancy lives. You
cannot find it by reading your own file.

> That location is a *span* — a byte range the compiler carried from the source through flatten,
> DAE construction and into the reduction. It resolves back to the file and line because the
> span, not the name, is what identifies the candidate. Which is also why the pane shows you a
> line of text and no variable name: the ordinals in the record belong to the rebuilt system of
> that pass, so a name read off them would be the wrong variable's.

### And the number beside it

The pass opens at **residue 2** on a system with a rank deficiency of **1**, and the pane states
the definition under the heading: *both deficits summed*.

```text
residue = (equations - matched) + (unknowns - matched)
```

One unmatched equation and one unmatched unknown is a residue of 2. So **a residue is twice the
rank deficiency for a square system** — it is not an error count, and halving it in your head is
the only way to read it as one. The pane publishes Rumoca's number rather than a friendlier one,
with the definition next to it.

---

## Station 4 — Reduction is a search, and a pass that moves nothing is not a wasted pass

`Drivetrain` is a motor, an ideal gear, a shaft and a compliant mount: nine states, 97 equations,
and a rank deficiency of 4.

> **Predict.** Nine states will become five. Two parts to this one.
>
> **(a)** The system has 97 equations before reduction. How many after?
>
> **(b)** One of the six passes selects a candidate and the residue afterwards is **the same
> number** it was before. Was that pass wasted?

[Look — Drivetrain → Index reduction → Summary](hrw://load/Drivetrain/IndexReduction/Summary)

**Expected:** **97 equations before and 97 after.** States fall 9 → 5 and algebraics rise
88 → 92; the equation count does not move at all.

Falsified if: the after-count of equations is anything other than 97.

**Expected:** Pass 3 takes the gear ratio — `phi_a = ratio*phi_b`, in
`IdealGear.mo:16` — with the outcome **`held`** and residue **4 → 4**. Then open pass 4 and read
its unmatched equation: it is **`f_x[33] (w = der(phi))`**, where pass 3's was
`f_x[92] (SI.Angle phi)`. Pass 4 then reduces 4 → 2.

Falsified if: pass 4's unmatched equation is the same one pass 3 reported, or pass 3's outcome is
anything other than `held`.

### What just happened

**(a) Nothing is eliminated.** Demoting a state does not delete its equation — it moves the
unknown from the integrated set to the algebraic set, and the equation stays to determine it. Four
states became four algebraic unknowns, which is exactly the `88 → 92` beside it. A phase that
made systems *smaller* would be a different phase.

**(b) The residue held and the system still changed.** That is the whole point, and pass 4's
unmatched equation is the proof: pass 3 was stuck on `SI.Angle phi` and pass 4 is stuck on
`w = der(phi)`. The gear ratio did not reduce the count of things left to match — it changed
*which* things they were, and the new one had a candidate the old one did not.

So the search is not hill-climbing on a number. A pass that holds is accepted in the Direct lane
precisely because *held* is progress on something the residue cannot see.

> This is where the old version of this lab was wrong, and worth saying plainly because the
> replacement is not obviously better. Until 2026-11-04 this station asked *"`Drivetrain`
> differentiated 6 times — how many manufactured equations survive?"* and answered *"none; 97
> equations become 20."* Both halves described a pipeline Rumoca no longer runs. The coincidence
> that it now reports **6 passes** for the same model is a trap: a pass is one lane iteration that
> may try several candidates and may take one that changes nothing. It is not a differentiation.

Step it, if you want the accumulation rather than the table — the stepper opens on the system
before the search and advances one record at a time:

[Step it — Drivetrain → Index reduction → Reduction replay](hrw://load/Drivetrain/IndexReduction/Animate)

---

## Station 5 — Two lanes, and one of them sees what the other cannot

`CapacitorLoop` is a capacitor straight across an ideal voltage source. One state.

> **Predict.** The Direct lane searches first and finds **nothing** — zero candidates in both of
> its proof groups. Does that make this system irreducible?

[Look — CapacitorLoop → Index reduction → Summary](hrw://load/CapacitorLoop/IndexReduction/Summary)

**Expected:** two passes. Pass 1 is the Direct lane with `direct_admissible: 0 candidates` and
`direct_conditional: 0 candidates` — *nothing to try*. Pass 2 is the **Holonomic** lane, which
finds **2**, and its first attempt **sorts** the system, on

```text
hrw/vendor/msl/Modelica 4.1.0/Electrical/Analog/Interfaces/TwoPin.mo:10
    v = p.v - n.v
```

Falsified if: the Direct lane discovers any candidate, or the system is not admitted.

> **Now the part Station 2 warned you about.** Look at the before/after grid.

**Expected:** **every count is identical** — 1 state, 13 algebraics, 14 equations, both rows. And
the manifold column reads **1** where the before row has an em dash.

Falsified if: any of the three counts differs across the grid.

### What just happened

Reduction admitted a system it changed without resizing. **If the counts had been your test for
whether anything happened, you would have concluded nothing did** — and the pane says otherwise in
a sentence, because that conclusion is the one worth preventing.

What it added is a **manifold constraint**: a statement that the reduced system's states are not
free to wander, but must stay on the surface the original constraint describes. One state, one
constraint, the same 14 equations — and a matching that now completes.

**Why the Direct lane could not see it.** The Direct lane looks for a state that some equation
*defines* outright, and demotes it. `v = p.v - n.v` is not that shape: it relates three
quantities with no single one of them being defined by the others in a way reduction can take
directly. The Holonomic lane looks for the other shape — a constraint among coordinates that can
be differentiated into a statement about rates — which is what this is.

Two lanes, alternating as a fixed point until neither can move. `Drivetrain` ends the same way:
its first five passes are the Direct lane, which stalls at residue 2, and its sixth is the
Holonomic lane taking the rack-and-pinion in `IdealGearR2T.mo:9` and sorting what was left.

---

## Station 6 — The model Rumoca cannot reduce, and the exact reason

Back to the pendulum from Station 1. Every constraint in the three stations since has been a
component definition — one quantity written in terms of others, which reduction can take directly
or differentiate. `x² + y² = L²` is neither: it is nonlinear and ties two states together, so no
definition hides inside it.

**You already know what has to happen.** Station 1 differentiated that constraint twice by hand,
and the second differentiation is what brings `lambda` within reach.

> **Predict.** Rumoca will not reduce this system. There are two ways that can look in the pane,
> and they are not the same: it could **try** candidates and reject every one, or it could find
> **nothing to try**. Which do you expect?

[Look — CartesianPendulum → Index reduction → Summary](hrw://load/CartesianPendulum/IndexReduction/Summary)

**Expected:** not admitted, stopped `singular` at residue 2, and **no reduced side at all** — the
after row is em dashes. Three passes, alternating Direct → Holonomic → Direct, and **every
candidate group reports 0**. There is not one attempt record in the whole log.

Falsified if: any group discovers a candidate, or any attempt appears, or an after row has numbers
in it.

### What just happened

**Rumoca does not decline to differentiate the constraint. It never gets as far as considering
it.** Both lanes search, both find nothing admissible, the lanes alternate once more to be sure,
and the search halts with the residue exactly where it started.

That is a sharper statement than *"index reduction does not work here"*, and a more useful one: the
missing piece is not a decision made badly, it is **candidate discovery for this shape of
constraint**. Pantelides' algorithm is precisely a procedure for finding that candidate — take a
structurally singular subset and differentiate its equations — and the two lanes here each hunt a
narrower pattern.

Look at where the diagnosis comes from, because it is the same pair you found by hand:

[Point at the raw system's error](hrw://stage/IndexReduction/Tree/node/before.error)

**Expected:** `rank_deficiency: 1`, `unmatched_equations: ["f_x[4] (x ^ 2 + y ^ 2 = L ^ 2)"]`,
`unmatched_unknowns: ["lambda"]`.

Falsified if: the unmatched pair is anything other than that one equation and `lambda`.

**That is Station 1's answer, reached by algorithm.** There you read it off the incidence pattern —
an empty row, a column reachable from only two equations. Here the matching reports it by name,
and the reduction then confirms it has no move for it.

The simulation fails the way an unreduced high-index system does — *"step size is too small at
time = 0.0000477"* — with no mention of index anywhere.

This is not a defect claim. Whether general Pantelides is missing, deferred, or deliberately out
of scope is a question for Rumoca's maintainers, and it is filed in
[`upstream-issues.md`](hrw://doc/upstream-issues.md) as a question. What the station establishes is
the boundary: you now know what this compiler does, and exactly where it stops.

---

## What this lab cannot check

Whether the six specimens read as one idea. They are meant to: a system you reduce by hand, two
kinds of nothing, one constraint found in a library file, a search that holds and still advances,
a lane seeing what another cannot, and back to the first with the compiler finding no move.
Whether that lands as a progression or as six unrelated panes is your report and nothing else's.

Whether "index" survives as a distance rather than a score. Station 1 builds it that way and no
pane displays an index number anywhere, so the idea rests on that station's prose and the
arithmetic the reader does there.

**What a differentiated equation looks like.** This is the biggest hole in the lab, and it is in
the data rather than in the writing. The reduction log says *which candidate was taken* and *where
its constraint was declared*, and the before/after reports say what the system became — but
nothing publishes the differentiated row itself. So the corpus can show you that `s_rel =
flange_b.s - flange_a.s` was the constraint reduction used, and cannot show you the equation it
produced from it. Station 1's by-hand derivation is the only place in this lab where you see one.

**Whether a `held` pass is legible as progress.** Station 4 argues it from two unmatched-equation
names one pass apart, which is evidence rather than intuition — but it is evidence you have to
assemble by opening two sections and comparing a string. If that reads as arithmetic rather than
as insight, the station has failed at the thing it exists for and I want to know.

**Whether the two-lane structure means anything to you yet.** The lab shows the Direct lane
stalling and the Holonomic lane finishing, twice, and explains the shapes they look for in a
paragraph. It does not show you *why* those are the two shapes, because that is the Pantelides
material and this lab deliberately stops at the boundary.

Whether System Modeler reduces the pendulum. That is the calibration that would turn Station
6's reading into a fact, and it has not been run. `the-oracle.md` is the lab for that gesture.

---

## What comes next in the chain

The system is now index-1, square and ordered — or it is not, and you have seen what that looks
like. What remains is starting it: the integrator needs consistent values at *t* = 0, and the
states' `start` attributes are not automatically consistent with the algebraic equations.

That is [initialization](hrw://lab/initialization).
