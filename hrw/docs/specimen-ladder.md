# The specimen ladder — discovery order, measured

**Purpose:** which specimen to study at which point, ordered by the problems the field met in turn
rather than by the order the compiler runs. Each rung adds **one** new difficulty.
**Status:** reference, **and it rots** — every row was measured against Rumoca 0.9.20 on 2026-09-24.
The 0.10.0 rebase changes at least four of them; see *When this goes stale*.
**Read when:** choosing what to study next, building a lab, or picking a specimen to exercise new
HRW simulation support.

*Why discovery order at all: [`question-ledger.md`](question-ledger.md), 2026-09-24 — Doug learned
more in two hours from [`dae-pendulum.md`](dae-pendulum.md) than in months of pipeline-ordered lab
work, and diagnosed the reason as beginning with problems and reaching solutions in the order they
were invented. The labs lead with problems **internal to the compiler**; this ladder leads with
problems about the world.*

---

## The ladder

| rung | specimen | shape | what is new | simulates? |
|---|---|---|---|---|
| **1** | `SingleInertia` | 2 states, **0 algebraics**, 2 eqs | nothing — a plain ODE | ✅ 2 of 2 move |
| **2** | `LoopWithInertia` | 1 state, 3 algebraics, 4 eqs | unknowns with **no rate equation** | ✅ 4 of 4 move |
| **2′** | `RotationalInertia` | 2 states, 9 algebraics, 12 eqs | the same at MSL scale | ✅ 5 of 12 move |
| **3** | `ProportionalLoop` | 0 states, 3 algebraics | a block that **cannot be ordered** | steady state |
| **3′** | `NonlinearLoop`, `MixedLoop`, `TwoLoops` | 0 states | nonlinear loop; mixed; two independent blocks | steady state |
| **4** | `BenchActuator` | 4 states, 44 algebraics, 48 eqs | **high index, and Rumoca fixes it** | ✅ 35 of 48 move |
| **5** | `BouncingBall` | 2 states, 0 algebraics | the equations **change at an instant** | ✅ 2 of 3 move |
| **6** | `CartesianPendulum` | 4 states, 1 algebraic, 5 eqs | **index 3 — Rumoca stops** | ❌ step size too small |

### Rung 1 is genuinely a plain ODE

`SingleInertia` reports **0 algebraics**, no events, and index reduction that calls itself a no-op.
There is nothing to solve simultaneously and nothing to enforce — Aside 1a's `z′ = f(z, t)`, with
`phi` and `w`. Everything in [`dae-pendulum.md`](dae-pendulum.md)'s Steps 5 and 9–14 applies to it
with a well-behaved Jacobian, which is what makes it the right vehicle for exercising new
simulation support: **if something is wrong, it is the instrument, not the model.**

### Rung 4 is the showpiece

`BenchActuator`'s stage readouts tell the whole arc without a word of prose:

```
Structural       Flagged   singular
IndexReduction   Ok        index-reduced from a structurally singular
                           (high-index) system — now solvable
```

Structural analysis fails, the reduction repairs it, the result simulates. **It is the only
specimen where the complete failure-and-repair story is visible and ends in a moving trajectory.**

### A trap in reading the table

Rungs 3 and 3′ have **0 states**, so "no series move" is *correct* — they are steady-state
algebraic systems, not broken simulations. Do not confuse them with the flat-lines below.

## What to avoid on 0.9.20, and why

These are flat-lined or killed by the **initialization defect** (`der(x) → 0`), not by anything
conceptual:

| specimen | symptom | rung it belongs to |
|---|---|---|
| `Drivetrain` | 9 states, **0 of 97** series move | 4 |
| `GearWithBrake` | 7 states, 0 of 49 move | 4/5 |
| `RcCircuit` | dies at t = 0.014 | 2 |
| `OverInitRc` | dies at t = 0.014 | 2 |

**All four come back with the 0.10.0 rebase**, confirmed on 2026-09-19. So the rebase mostly buys
breadth at rungs 2 and 4 — which is exactly where discovery order spends its time, and is the
strongest argument for revisiting the *don't rebase until tagged* ruling under the new sequence.

**`CartesianPendulum` is a different failure and the rebase does not fix it.** It needs general
Pantelides, which Rumoca does not implement — [`ideas.md`](ideas.md) **#83** is Doug's own proposal
to write it.

## Scaffolding: what exists, and where the hole is

| rung | lab |
|---|---|
| 1, 2 | **none** — this is where simulation would be studied, and there is no simulation lab |
| 3 | [`blt-ordering.md`](fixture-labs/blt-ordering.md), [`tearing.md`](fixture-labs/tearing.md) |
| 4 | [`index-reduction.md`](fixture-labs/index-reduction.md) — `BenchActuator` is its Station 3 |
| 5 | [`events.md`](fixture-labs/events.md) |
| 6 | [`index-reduction.md`](fixture-labs/index-reduction.md) Station 1 and 6, and `dae-pendulum.md` |

**The rungs Doug most wants are the ones with the least scaffolding**, which is the same gap as
`dae-pendulum.md`'s unwritten chapter seen from the other side. `ideas.md` **#68** carries the
instrument work.

## When this goes stale

Re-measure after **any** of these, and update the table rather than trusting it:

- **the 0.10.0 rebase** — four rows above change outright
- **a new specimen** — it needs a rung, or it is unreachable from here
- **Rumoca gaining general Pantelides** — rung 6 stops being a wall

How it was measured, both reproducible:

```
cargo run -p hrw --example stage_outcomes -- specimens/<Model>.mo
```

plus a one-off `simulate_specimen` probe over `specimens/`, **with the MSL loaded** — without
libraries every MSL-based model reports *"the pipeline produced no simulable result"*, which reads
as a Rumoca failure and is not one. That mistake was made and caught on 2026-09-24.
