# Fixture tour — Events: the equations that are not always true

**A curriculum tour.** Walk [`initialization.md`](initialization.md) first. Everything so far has
assumed one fixed set of equations; this tour is about models where the equations change.

Every count below was read from the committed traces, never remembered.

---

## The problem this phase exists to solve

Every phase so far treated the model as one system of equations, true for all time. Ordering,
tearing and index reduction all rest on that: a permutation computed once is valid forever.

Physical models break the assumption constantly. A ball bounces — its velocity reverses, but only
at the instant it touches the floor. A brake engages. A diode conducts. In each case some equation
holds *sometimes*, and the moment it starts or stops holding is not known in advance: it depends on
the solution.

That is genuinely hard, and it is worth being precise about why. The integrator advances in steps.
If the bounce happens between two steps, integrating straight through it produces nonsense — the
ball ends up below the floor, moving down. So the solver must **detect** the instant, stop there,
apply the change, and restart.

**This phase finds what can change and what has to be watched.** Three acts: a model with a real
event, a model with none, and a model with several.

---

## Act 1 — A model with a real event

`BouncingBall` is two equations plus a `when` clause that reverses velocity at the floor.

> **Predict.** What does the compiler have to extract from that `when` clause for a solver to
> handle it?

[▶ Look — BouncingBall → Events](hrw://load/BouncingBall/Events)

**Expected:** the summary reads `condition_equations: 1`, `relations: 1`,
`discrete_real_updates: 1`, and `zero_crossing_conditions: 0`.

**Falsified if:** the discrete update count is 0, or no relation is reported.

*What just happened.* The `when` clause was taken apart into **three separate things**, and the
split is the phase's whole output:

- a **relation** — the comparison that decides, `h <= 0`;
- a **condition equation** — that relation given a name the solver can evaluate every step;
- a **discrete update** — what changes when it fires, here the reversal of `v`.

**The condition is not the action.** The solver needs to evaluate the condition continuously, to
find *when* it flips; it needs the action only once, at that instant. Keeping them apart is what
lets the integrator search for the crossing time without applying anything.

---

## Act 2 — A model with none, and what the pane says

`RcCircuit` has no `when` clauses at all.

> **Predict.** What will the Events stage show for a purely continuous model?

[▶ Look — RcCircuit → Events](hrw://load/RcCircuit/Events)

**Expected:** `condition_equations: 0`, `relations: 0`, and both update counts **0**. The note
reads *"no events — this model is a smooth (continuous) system."*

**Falsified if:** the pane is blank, or reports a condition equation.

*What just happened.* **Absence is stated, not left blank** — the pane says the model is smooth
rather than showing an empty list you would have to interpret. That distinction has been earned:
an empty pane and a broken pane look identical.

**One number here does not fit, and the tour will not pretend otherwise.**
`zero_crossing_conditions` reads **1** for this model, which has no `when` clause anywhere.

It is worth knowing what is and is not established about it. Across the corpus, **every specimen
containing an MSL `Resistor` reports exactly 1, and no specimen without one reports any** — and in
this model the collections behind the count, `equations_f_c` and `relations`, are both **empty**.
So the count names something the event partition does not contain. The suspect is the `assert`
inside `Resistor.mo`, which holds the component's only relation; that part is a **hypothesis**, not
a finding.

The whole investigation is in [`upstream-issues.md`](../upstream-issues.md), written to be filed.
**A tour that smoothed this over would be teaching you something false about a number you can
see** — and this one is a genuine Rumoca question, not an HRW defect.

---

## Act 3 — A model with several

`GearWithBrake` is a drivetrain whose brake engages and releases.

> **Predict.** More conditions than `BouncingBall`, or the same shape scaled up?

[▶ Look — GearWithBrake → Events](hrw://load/GearWithBrake/Events)

**Expected:** `condition_equations: 4`, `relations: 4`, `discrete_valued_updates: 1`, and
`discrete_real_updates: 0`.

**Falsified if:** the counts match `BouncingBall`'s, or no discrete-valued update appears.

*What just happened.* **Four conditions for one brake**, which is the interesting part. A brake is
not one comparison: it is stuck or sliding, and sliding in one direction or the other, so the model
needs several relations to say which regime it is in.

And the update is **discrete-valued**, not discrete-real — a mode flag rather than a number. That
is the same distinction as Act 1's split, one level up: `BouncingBall` changes a *value* when it
fires, `GearWithBrake` changes which *equations apply*.

**Four conditions means four things to watch on every step**, and each is a potential event time
the integrator has to locate. This is why event handling dominates the run time of models with
friction, and why a model that looks small can simulate slowly.

---

## What this tour cannot check

**Whether Act 2's anomaly is the right thing to include.** A tour that names an unexplained number
risks reading as *"the tool is unreliable"* rather than *"this is a real open question."* It is
here because you can see the number, and a tour that omitted it would be the less honest choice.

**Whether the event counts mean anything without the equations behind them.** The pane reports how
many relations and updates exist, not what they say. `BouncingBall`'s single relation is guessable;
`GearWithBrake`'s four are not, and this tour does not show them.

**Whether "the integrator locates the crossing" is believable as described.** The mechanism —
bisection or interpolation on the condition, back up, restart — is asserted in prose and
demonstrated nowhere, because nothing in HRW shows the solver's step-by-step search.

---

## What comes next in the chain

The compiler now knows everything: states, order, tears, initial values, and what can change
mid-run. All of it is still expressed in *your names*.

Turning those into memory addresses is [`solve-lowering.md`](solve-lowering.md).
