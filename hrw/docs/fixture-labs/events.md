# Fixture lab — Events: the equations that are not always true

<!-- kind: concept -->

**Every phase so far treated the model as one system of equations, true for all time.** Ordering,
tearing and index reduction all rest on that: a permutation computed once is valid forever.

Physical models break the assumption constantly. A ball bounces — its velocity reverses, but only
at the instant it touches the floor. A brake engages. A diode conducts. In each case some equation
holds *sometimes*, and the moment it starts or stops holding is not known in advance, because it
depends on the solution.

That is genuinely hard, and it is worth being precise about why. The integrator advances in steps.
If the bounce happens between two steps, integrating straight through it produces nonsense — the
ball ends up below the floor, moving down. So the solver must detect the instant, halt there,
apply the change, and restart. The Events tab shows what can change and what has to be watched.
It is not a phase of its own: DAE construction records all of it, and the tab reads that
record — and, under `lowered`, what solve lowering made of it in the run that simulates.

Run [solve-lowering](hrw://lab/solve-lowering) first; this is the last lab in the chain. Every
count below is read from a generated trace, so if one disagrees with your screen, the lab is wrong
and I want to know. The counts are in the tree's `summary`, under Rumoca's own names.

---

## Station 1 — A model with a real event

`BouncingBall` is two equations plus a `when` clause that reverses velocity at the floor.

> **Predict.** What does the compiler have to extract from that `when` clause for a solver to
> handle it?

[Look — BouncingBall → Events](hrw://load/BouncingBall/Events)

**Expected:** `summary` reads `conditions: 1`, `relations: 1`, `roots: 1` and
`event_actions: 1`. The one relation is `h <= 0`, and the one event action has kind
`reinitialize` and detail `v := -(e*pre(v))`.

Falsified if: `event_actions` is 0, or no relation `h <= 0` is listed.

*What just happened.* The `when` clause was taken apart into separate things, and the split is
the record's whole content:

- a relation — the comparison that decides, `h <= 0`;
- a root — that relation, watched for the instant it crosses, so the solver can find the time;
- an event action — what happens when it fires, here `reinit` reversing `v`.

The condition is not the action. The solver needs to watch the root continuously, to find *when*
the relation flips; it needs the action only once, at that instant. Keeping them apart is what
lets the integrator search for the crossing time without applying anything.

---

## Station 2 — A model with no `when`, and what the pane says

`RcCircuit` has no `when` clauses at all.

> **Predict.** What will the Events tab show for a model with no `when`?

[Look — RcCircuit → Events](hrw://load/RcCircuit/Events)

**Expected:** not an empty record. `summary` reads `relations: 1`, `roots: 1` and
`event_actions: 1`. The relation is `1 + R.alpha*(R.T_heatPort - R.T_ref) >= …` (a tiny
positive number), and the event action has kind `assert` with detail
`"Temperature outside scope of model!"`.

Falsified if: `relations` is 0, or no `assert` action is listed.

*What just happened.* You wrote no event, and the model still has one to watch. It comes from the
MSL `Resistor`, whose temperature-dependent resistance carries an `assert` that the temperature
stays in range. An assertion is a condition that can become false mid-run, so Rumoca records it,
watches its relation as a root, and keeps the assert as the action to take if it fires. The
relation is over `R.T_heatPort`, the Resistor's heat port, which nothing in your model mentions.

*(Until 2026-09-02 this lab said a number here "does not fit" and pointed at an upstream issue.
The fault was HRW's: the pane published a Rumoca count under a name of its own,
`zero_crossing_conditions`, which read as a claim about all zero crossings. The summary now
publishes Rumoca's own names, and the relation is listed, so there is nothing to explain away.)*

---

## Station 3 — A model with several

`GearWithBrake` is a drivetrain whose brake engages and releases:

```modelica
when load.w > maxSpeed then braking = true;
elsewhen load.w < maxSpeed * 0.5 then braking = false;
end when;
brakeTorque.tau = if braking then (if load.w > 0 then -brakeForce else brakeForce) else 0;
```

> **Predict.** How many relations, and is what changes a *number* or a *mode*?

[Look — GearWithBrake → Events](hrw://load/GearWithBrake/Events)

**Expected:** `summary` reads `conditions: 3`, `relations: 3`, `roots: 3`,
`discrete_value_definitions: 1` and `discrete_real_equations: 0`. The relations are
`load.w > 0`, `load.w > maxSpeed` and `load.w < maxSpeed*0.5`.

Falsified if: the counts match `BouncingBall`'s, or `discrete_value_definitions` is 0.

*What just happened.* Three relations for one brake. One threshold engages it, a lower one
releases it — two thresholds, so the brake does not chatter on and off at a single speed
(hysteresis) — and the third decides which way the torque opposes the motion.

And what changes is discrete-valued, not discrete-real: the Boolean `braking`, a mode flag rather
than a number. That is the same distinction as Station 1's split, one level up: `BouncingBall`
changes a *value* when it fires, `GearWithBrake` changes which *equations apply*.

Three roots means three things to watch on every step, and each is a potential event time the
integrator has to locate. This is why event handling dominates the run time of models with many
switches, and why a model that looks small can simulate slowly.

---

## What this lab cannot check

Whether Station 2 reads as a discovery or as noise. An event you did not write is surprising, and
the lab bets that tracing it to the Resistor's `assert` is worth the surprise.

Whether "the integrator locates the crossing" is believable as described. The mechanism —
bisection or interpolation on the root, back up, restart — is asserted in prose and demonstrated
nowhere, because nothing in HRW shows the solver's step-by-step search.

---

## Where the chain ends

The compiler now knows everything: states, order, tears, initial values, memory layout, and what
can change mid-run. [The pipeline](hrw://lab/the-pipeline) visits every one of those tabs again on
one model, end to end.
