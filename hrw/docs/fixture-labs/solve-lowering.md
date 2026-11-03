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

Run [events](hrw://lab/events) first. Every count below is read from a generated trace, so if one
disagrees with your screen, the lab is wrong and I want to know.

---

## Station 1 — Where your variables went

`BouncingBall` has two continuous variables, `h` and `v`.

> **Predict.** What will `h` and `v` have become?

[Look — BouncingBall → Solve lowering](hrw://load/BouncingBall/SolveLowering)

[Point at `layout.bindings`](hrw://stage/SolveLowering/node/layout.bindings)

**Expected:** each name binds to a slot in the continuous vector `Y`:

| name | binding |
|---|---|
| `h` | `Y` index 0, byte offset 0 |
| `v` | `Y` index 1, byte offset 8 |
| `time` | `Time` — not a slot at all |

Falsified if: `h` and `v` bind to `P` slots, or share an index.

*What just happened.* This table is the Rosetta stone, and it is the reason the phase publishes
it. From here on the model is `Y[0]` and `Y[1]`; if you ever need to know what a number in a
trajectory *means*, this mapping is the only thing that says so.

Two details worth noticing. The byte offsets (0 and 8) say these are 64-bit floats laid out
contiguously — the vector is a real block of memory, not a dictionary. And `time` is not in the
array: it is a distinguished binding, because the solver supplies it rather than solving for it.

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
| `compiled_parameter_len` | `5` | the length of the vector the solver is handed |

**Three slots exist that `BouncingBall` never declared**, and v0.10.0 says what two of them
are for, in the same node:

- **`pre_param_bindings`** reads `dest_p_index: 2` with source `Y[1]`. So `P[2]` holds the
  previous value of `Y[1]`, which `layout.bindings` names `v` — this is `pre(v)`.
- **`relation_memory_parameter_indices`** reads `[4]`. So `P[4]` is where a relation
  remembers itself: the `h <= 0` crossing.

**`P[3]` is not named by either field, and this lab does not know what it is.** Said plainly
rather than guessed at: three fields account for four of the five slots, and the fifth is
an open question worth asking.

Falsified if `compiled_parameter_len` equals `parameter_count`, or if `pre_param_bindings`
is empty.

*What just happened.* The event machinery needs storage, and it gets it here. `events.md`
established that a `when` clause reverses `v` at the bounce; doing that requires knowing what `v`
*was*, so `pre(v)` needs somewhere to live. It lives in a parameter slot — a value that is constant
*between* events and updated *at* them, which is exactly what a parameter slot is for.

So the phases compose visibly: the discrete update found in `events.md` is the reason `P` is
longer than the model's parameter list.

> **This station's numbers changed in Rumoca 0.10.0, and so did how it reads them.**
>
> It pointed at a node called `parameters` and expected **seven** slots, listing each by
> name — `__pre__.c`, `__pre__.h`, `__pre__.v`, `c`, `__rumoca.initial_event`. There is
> no `parameters` node now, the total is **five**, and the compiler-introduced slots are
> no longer given `__pre__`-style names to read off.
>
> **What replaced the names is better: the layout states the relationships.** The old
> table was a list of strings a reader had to trust; `pre_param_bindings` says *which
> slot* holds the previous value of *which variable*, by index, and
> `relation_memory_parameter_indices` says which slot a relation remembers itself in.
> Naming a slot `__pre__.v` and *binding* `P[2]` to `Y[1]` are not the same quality of
> evidence.
>
> The teaching point is untouched: the parameter vector is longer than the model's
> parameter list, and the event machinery is why.

And `__rumoca.initial_event` is a compiler-manufactured variable — nothing in the model
mentions it. The first event is special (it fires at *t* = 0 during initialization), and the
generated code needs a flag to say so. Names beginning `__` are the phase's own bookkeeping, and
seeing them is the honest version of what "lowering" means: the model gains machinery it never
declared.

---

## Station 3 — The same mapping at scale

`RcCircuit` is 23 continuous variables and a handful of parameters.

> **Predict.** How large will `Y` be — 1, for the single state, or something else?

[Look — RcCircuit → Solve lowering](hrw://load/RcCircuit/SolveLowering)

**Expected:** `initial_y` has 23 entries and `parameters` has 8. `visible_names` is also
23.

Falsified if: `initial_y` has 1 entry.

*What just happened.* `Y` holds every continuous variable, not only the states. `RcCircuit` has
one state and twenty-two algebraics, and all twenty-three get slots — because the solver has to
*store* every quantity it computes, even the ones it recomputes from scratch at each step.

That is worth holding next to the DAE lab's Station 3. The unknowns the solver solves for are the
derivatives; the vector it carries is every continuous variable. Two different counts, both
correct, easy to conflate.

Compare the parameter counts: `BouncingBall` needed 7 slots for 2 declared parameters, because it
has events. `RcCircuit` needs 8 for 7 declared, because it has none — one manufactured slot instead
of five. The overhead is event machinery, and a smooth model barely pays it.

---

## What this lab cannot check

Whether the bindings table is findable. Stations 1 and 2 send you into a generic serde tree to a
nested path. Whether `problem.layout.bindings` reads as a mapping or as a wall of JSON is your
report, and it is the one thing this lab depends on being legible.

Whether the enum constants are noise or context. The bindings map also contains dozens of
entries like `StateSelect.never -> Constant(1)` from the MSL. They are real and this lab ignores
them, which may make the pane look busier than the stations suggest.

What the generated code actually looks like. This phase's output is a layout, and the lab
stops there. Whether the residual function reads as recognisable arithmetic on `Y[0]` and `Y[1]` is
not shown anywhere in HRW.

---

## One count here contradicts the index-reduction lab

Do not run `Drivetrain` through this stage without knowing this. Index reduction demoted its
nine states to three, and reports 3. This stage's `state_scalar_count` reads 9.

| specimen | `index_reduction` says | this stage says |
|---|---|---|
| `Drivetrain` | 3 | 9 |
| `GearWithBrake` | 2 | 7 |
| `BouncingBall` | 2 | 2 (nothing was demoted) |

The two agree exactly when nothing is demoted, which is why it looks like demoted states are still
counted here.

It is not diagnosed and there are innocent readings — a dummy-derivative scheme keeps demoted
states around as algebraic variables, so a *scalar* count of them may be correct and merely named
confusingly. The investigation is in [`upstream-issues.md`](hrw://doc/upstream-issues.md).

This lab deliberately used `BouncingBall` and `RcCircuit` for its stations, both of which demote
nothing, so every number in Stations 1–3 is unaffected. That was a choice to keep the stations clean, and
saying so is better than letting you discover the discrepancy on your own model and doubt the
whole lab.

---

## What comes next in the chain

Nothing — this is the last compiler phase. What follows is Simulation: the solver stepping the
vector through time, which is the Plot tab rather than a stage.
