# Fixture lab — Solve lowering: names become indices

<!-- kind: concept -->

**A solver does not look variables up by name.** It is handed arrays and a function: *here is the
current state vector, fill in the derivatives*.

Every phase so far has worked in **your** vocabulary — `C.v`, `inertia.flange_b.tau`,
`R.R_actual`, hierarchical names that are meaningful to a modeller and useless to a numerical
integrator. So every one of them must become an index into an array, and every equation must
become arithmetic on those slots.

This phase does the translation, and it is the last chance to get it wrong: after it there are no
names left to check against, only numbers, which is why the mapping itself is worth looking at.
The Solve lowering tab is the lowered problem from the same run the simulation integrates.

Run [initialization](hrw://lab/initialization) first. Every count below is read from a generated
trace, so if one disagrees with your screen, the lab is wrong and I want to know.

---

## Station 1 — Where your variables went

`BouncingBall` has two continuous variables, `h` and `v`.

> **Predict.** What will `h` and `v` have become?

[Look — BouncingBall → Solve lowering](hrw://load/BouncingBall/SolveLowering)

[Point at `layout.bindings`](hrw://stage/SolveLowering/node/layout.bindings)

**Expected:** each name binds to a slot in the continuous vector `Y` or the parameter vector `P`:

| name | binding |
|---|---|
| `h` | `Y` index 0, byte offset 0 |
| `v` | `Y` index 1, byte offset 8 |
| `g` | `P` index 0, byte offset 0 |
| `e` | `P` index 1, byte offset 8 |

and nothing else — four entries.

Falsified if: `h` and `v` bind to `P` slots, or share an index.

*What just happened.* This table is the Rosetta stone, and it is the reason the phase publishes
it. From here on the model is `Y[0]` and `Y[1]`; if you ever need to know what a number in a
trajectory *means*, this mapping is the only thing that says so.

Two details worth noticing. The byte offsets (0 and 8) say these are 64-bit floats laid out
contiguously — the vector is a real block of memory, not a dictionary. And `time` is not in the
table at all: the solver supplies it rather than storing it in a slot.

---

## Station 2 — What else is in the arrays

`BouncingBall` declares two parameters, `g` and `e`.

> **Predict.** How many slots will the parameter vector `P` have?

[Look — BouncingBall → Solve lowering](hrw://load/BouncingBall/SolveLowering)

[Point at `solve_layout`](hrw://stage/SolveLowering/node/solve_layout)

**Expected:** two counts that disagree, and the disagreement is the answer.

| field | value | what it means |
|---|---|---|
| `parameter_count` | `2` | the parameters *the model declares* — `g` and `e` |
| `compiled_parameter_len` | `6` | the length of the vector the solver is handed |

**Four slots exist that `BouncingBall` never declared, and the problem names every one:**

- **`P[2]`** — `pre_param_bindings` reads `dest_p_index: 2` with source `Y[1]`, which
  `layout.bindings` names `v`. So `P[2]` holds `pre(v)`, the previous value of `v`.
- **`P[3]`** — under the top-level `events` node, `condition_memory_parameter_indices` reads
  `[3]`: the slot where the `when` condition remembers whether it was true.
- **`P[4]`** — `relation_memory_parameter_indices` (in `solve_layout`) reads `[4]`, and
  `events.root_relation_memory_targets` agrees: where the relation `h <= 0` remembers itself.
- **`P[5]`** — `initial_event_parameter_index` reads `5`: a flag for the first event, which is
  special — it fires at *t* = 0, during initialization.

Falsified if `compiled_parameter_len` equals `parameter_count`, or if `pre_param_bindings`
is empty.

*What just happened.* The event machinery needs storage, and it gets it here. `BouncingBall`'s
`when` clause reverses `v` at the bounce — the [events](hrw://lab/events) lab, next, takes that
clause apart — and doing that requires knowing what `v` *was*, so `pre(v)` needs somewhere to
live. It lives in a parameter slot — a value that is constant *between* events and updated *at*
them, which is exactly what a parameter slot is for. The model gains machinery it never
declared, which is the honest version of what "lowering" means.

> **This station's numbers changed twice.** Before Rumoca 0.10.0 it expected seven slots,
> listed by names like `__pre__.v` and `__rumoca.initial_event`; until 2026-11-27 it said five,
> with `P[3]` "an open question". Neither name list nor gap exists now: the layout states the
> relationships by index — *which slot* holds the previous value of *which variable*, which
> slot a condition or relation remembers itself in, which slot is the initial-event flag.
> Naming a slot `__pre__.v` and *binding* `P[2]` to `Y[1]` are not the same quality of evidence.

---

## Station 3 — The same mapping at scale

`RcCircuit` is 23 continuous variables and seven declared parameters.

> **Predict.** How large will `Y` be — 1, for the single state, or something else?

[Look — RcCircuit → Solve lowering](hrw://load/RcCircuit/SolveLowering)

**Expected:** under `layout`, `y_scalars` is 23. Under `solve_layout`, `state_scalar_count` is 1
and `algebraic_scalar_count` is 22; `parameter_count` is 7 and `compiled_parameter_len` is 12.

Falsified if: `y_scalars` is 1.

*What just happened.* `Y` holds every continuous variable, not only the states. `RcCircuit` has
one state and twenty-two algebraics, and all twenty-three get slots — because the solver has to
*store* every quantity it computes, even the ones it recomputes from scratch at each step.

That is worth holding next to the DAE lab's Station 3. The unknowns the solver solves for are the
derivatives; the vector it carries is every continuous variable. Two different counts, both
correct, easy to conflate.

Compare the parameter counts: `BouncingBall` needed 6 slots for 2 declared parameters, and
`RcCircuit` needs 12 for 7 — five manufactured slots, although you wrote no `when` clause. They
are condition memory (`P[7]`–`P[9]`), relation memory (`P[10]`) and the initial-event flag
(`P[11]`), and they exist because the MSL `Resistor` carries an `assert` that the temperature
stays in range: a condition that can become false mid-run, watched like any other. The events
lab's Station 2 shows that relation.

---

## What this lab cannot check

Whether the bindings table is findable. Stations 1 and 2 send you into a generic serde tree to a
nested path. Whether `layout.bindings` reads as a mapping or as a wall of JSON is your report, and
it is the one thing this lab depends on being legible.

What the generated code actually looks like. This phase's output is a layout plus a straight-line
instruction program, and the lab goes no further than the layout. Whether the residual function
reads as recognisable arithmetic on `Y[0]` and `Y[1]` is left to the tree's `continuous` node.

---

## The state count agrees across the tabs

Index reduction, this stage and the simulation are readings of one run of solve lowering, so
they report the same number of integrated states:

| specimen | Index reduction: integrates | `state_scalar_count` | simulation |
|---|---|---|---|
| `Drivetrain` | `mount.s_rel`, `L.i`, `mount.v_rel` | 3 | 3 |
| `GearWithBrake` | `spring.phi_rel`, `spring.w_rel` | 2 | 2 |
| `BouncingBall` | `h`, `v` | 2 | 2 |

*(Until 2026-11-26 this section explained a disagreement — the Index reduction tab said
`Drivetrain` went from 9 states to 5 while this stage said 3. The 5 came from HRW running the
reducer itself on the DAE before solve lowering's simplifications, which is not a run Rumoca
makes. Before Rumoca 0.10.4 the gap ran the other way.)*

This lab's stations use `BouncingBall` and `RcCircuit`, which reduce nothing.

---

## What comes next in the chain

This is the last compiler phase. After it come the Simulation tab — the solver stepping the vector
through time — and the last lab, [events](hrw://lab/events): what can change mid-run, and how the
slots in Station 2 are used.
