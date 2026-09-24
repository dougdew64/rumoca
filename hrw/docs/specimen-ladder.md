# The specimen ladder — discovery order, verified and measured

**Purpose:** which specimen to study at which point, ordered by the problems the field met in turn
rather than by the order the compiler runs. Each rung names **one development**, the capability it
created, and a specimen that could not be modelled or simulated before it.
**Status:** reference, and it rots in two different ways — the **history** was verified against
primary citations on 2026-09-24 and does not rot; the **measurements** were taken against Rumoca
0.9.20 on 2026-09-24 and do. See *When this goes stale*.
**Read when:** choosing what to study next, building a lab or a chapter, or picking a specimen to
exercise new HRW simulation support.

*Why discovery order at all: [`question-ledger.md`](question-ledger.md), 2026-09-24 — Doug learned
more in two hours from [`dae-pendulum.md`](dae-pendulum.md) than in months of pipeline-ordered lab
work, and diagnosed the reason as beginning with problems and reaching solutions in the order they
were invented. The labs lead with problems **internal to the compiler**; this ladder leads with
problems about the world.*

---

## First, a correction to the premise — and it is a useful one

"Discovery order" and "the order that makes these ideas learnable" are **not the same order**, and
the ladder below is the second one. The chronology is real and is given in full further down, but
three facts make strict chronology the wrong spine for a curriculum:

- **Gear named the differential-algebraic equation in 1971**, six years before Elmqvist's Dymola
  and twenty-six before Modelica 1.0. The solver was ready for DAEs long before any language made
  people write them by accident.
- **Baumgarte's constraint stabilisation (1972) predates Pantelides (1988) by sixteen years.** The
  index problem was being solved by hand for a generation before an algorithm existed.
- **Tarjan's strongly-connected-components algorithm (1972)** was a graph-theory result with no
  simulation in view; it became block-lower-triangular ordering when Duff and Reid implemented it
  for sparse matrices in 1978, and became a Modelica compiler phase later still.

What Doug actually wants — and what made `dae-pendulum.md` work — is **problem-before-solution
order**: never meet a mechanism before meeting the thing it fixes. That is *dependency* order. It
is close to chronological and diverges in named places, which are flagged below. **Saying "this is
how it happened" where it is really "this is how it builds" would be the kind of plausible
falsehood this project exists to avoid.**

---

## The ladder

Each rung's specimen is chosen so that **the rung below it cannot express or solve that model.**

| rung | the development | what became possible | specimen | on Rumoca 0.9.20 |
|---|---|---|---|---|
| **1** | Euler, *Institutiones calculi integralis*, **1768–70** | solving an initial-value problem **numerically at all**, when no closed form is available | `SingleInertia` | ✅ 2 states, 0 algebraics |
| **2** | Runge **1895**, Heun **1900**, Kutta **1901** | **accuracy per step** — buying error reduction with function evaluations instead of with smaller `h` | `HarmonicOscillator` | ✅ 2 states, 0 algebraics |
| **3** | Curtiss & Hirschfelder **1952**; Dahlquist **1963** | stepping **past a dead fast mode** — implicit methods, and the stability theory that justifies them | `StiffDecay` | ✅ 2 states, 0 algebraics |
| **4** | Gear **1971** | handing the solver `F(t, z, z') = 0` — **algebraic unknowns need not be eliminated first**; Petzold's DASSL (**1982**) makes it production software | `LoopWithInertia` | ✅ 1 state, 3 algebraics, 4 of 4 move |
| **5** | Baumgarte **1972** | **constrained mechanics simulated** — by the *modeller* differentiating the constraint and damping the drift | `StabilizedPendulum` | ❌ see below — and this is the find |
| **6** | Elmqvist **1978** (Dymola); Modelica 1.0 **Sept 1997** | **declaring** a model by connecting components, instead of deriving `z' = f(z,t)` by hand | `RotationalInertia`, `RcCircuit` | ✅ 5 of 12 move / ⚠ `RcCircuit` dies at t = 0.014 |
| **7** | Tarjan **1972**, implemented by Duff & Reid **1978** | recovering computational order from declared equations automatically — **BLT**, and with it the discovery that some blocks *cannot* be ordered | `ProportionalLoop` | ✅ steady state, 0 states |
| **8** | Elmqvist & Otter **1994** | solving a large coupled block by iterating on **a few** variables — tearing | `NonlinearLoop`, `MixedLoop`, `TwoLoops` | ✅ steady state, 0 states |
| **9** | Gear **1988**, Pantelides **1988**, Mattsson & Söderlind **1993** | **the compiler** detecting and removing high index, so the modeller no longer does rung 5's work by hand | `BenchActuator` (works), `CartesianPendulum` (wall) | ✅ 35 of 48 move / ❌ step size too small |
| **10** | Cellier **1979** | continuous and discrete behaviour in **one** model — state events and zero crossings | `BouncingBall` | ✅ 2 of 3 move |

**Where this diverges from chronology**, and why:

- **Rungs 4 and 5 (1971, 1972) sit below rungs 6–8 (1978–97)**, which came later in time. The
  language rungs are placed after the solver rungs because an acausal model is unreadable until you
  know what the solver it feeds will accept.
- **Rung 7's Tarjan (1972) is placed after rung 6's Modelica (1997)** — a 25-year inversion. BLT is
  impossible to motivate before there is a declared, unordered equation set to order.
- **Rung 10's Cellier (1979)** is placed last though it is early, because events are orthogonal to
  everything above and mixing them in earlier obscures the index story.

---

## Rung 5 is where this ladder earned its keep

`StabilizedPendulum` was written for this ladder on 2026-09-24, and it found something.

The model is `CartesianPendulum`'s physics with the constraint `x² + y² = L²` differentiated twice
**by hand** and fed back as `C̈ + 2αĊ + β²C = 0` — Baumgarte's 1972 construction, in portable
Modelica, with no compiler feature required. The expectation was that Rumoca, lacking Pantelides,
would simulate it happily.

Half of that was right:

```
Structural       Ok
IndexReduction   Ok        already index-1 — the reduction funnel is a no-op here
Initialization   Flagged   IC planning failed: 0 matched out of 1; unmatched unknowns: lambda
simulation       FAILED    BDF: Failed to factorise matrix: SymbolicSingular { index: 4 }
```

Where `CartesianPendulum` reports `Structural … singular` and then `still singular after index
reduction`, this model **passes both**, and Rumoca correctly calls it already index 1. Baumgarte's
construction does exactly what it claims, confirmed by a compiler with no idea it was applied.

**And it still does not run.** Initialization cannot match `lambda`, so `lambda` never receives a
consistent initial value, and the iteration matrix is singular at index 4 — which is `lambda`.

So **Rumoca's inability to simulate a constrained pendulum is not only the missing Pantelides.**
There is a second, independent stop downstream, in initialization of an algebraic unknown, and it
is reached even when the index problem has already been removed. The specimen was validated in
**System Modeler first** — it runs there, `lambda` matches the analytic tension to ~1e−9, and the
constraint residual *shrinks* from 1.5e−8 to 5.6e−12 across the run — so the failure is Rumoca's,
not the model's. Full evidence:
[`specimen-notebook/StabilizedPendulum/purpose.md`](specimen-notebook/StabilizedPendulum/purpose.md).

**Open question, not a claim:** whether the 0.10.0 initialization fix (which revives `RcCircuit`
and three others) also matches `lambda` here. If it does, rung 5 becomes reachable and Doug can
simulate a constrained pendulum on Rumoca **without waiting for Pantelides**. That is worth
measuring the day the rebase happens, and it is the strongest argument yet for doing it.

## Rung 9 is the showpiece

`BenchActuator`'s stage readouts tell the whole arc without a word of prose:

```
Structural       Flagged   singular
IndexReduction   Ok        index-reduced from a structurally singular
                           (high-index) system — now solvable
```

Structural analysis fails, the reduction repairs it, the result simulates. **It is the only
specimen where the complete failure-and-repair story is visible and ends in a moving trajectory** —
which is to say it is the only place the rung-9 capability can be *seen* rather than described.

## Two traps in reading the table

**Rungs 7 and 8 have 0 states**, so "no series move" is *correct* — they are steady-state algebraic
systems, not broken simulations. Do not confuse them with the flat-lines below.

**Rung 1's specimen is deliberately the dullest model in the corpus.** `SingleInertia` has the
exact solution `phi = t²/2`, `w = t`, reports 0 algebraics and no events, and index reduction calls
itself a no-op. There is nothing to solve simultaneously and nothing to enforce. That is the point:
**if something is wrong while running it, it is the instrument, not the model.** It is the right
vehicle for exercising new simulation support for exactly that reason.

Its dullness is also why rung 2 needed a new specimen. `phi = t²/2` is a polynomial, and several
integrators reproduce a polynomial to machine precision — so integrator *order*, the whole content
of rung 2, is invisible on it. `HarmonicOscillator` solves to `cos(t)`, where every method has a
truncation error and orders separate measurably.

---

## The verified chronology

Every row below was checked against a primary citation on **2026-09-24**. Anything that could not
be substantiated is not in this table.

| year | who | what | where |
|---|---|---|---|
| 1768–70 | Euler | the method now named after him, in a text on integral calculus | *Institutiones calculi integralis*, 3 vols |
| 1895 | Runge | extends Euler to higher accuracy; midpoint, Heun and trapezoid schemes | *Math. Ann.* **46**:167–178 |
| 1900 | Heun | order conditions taken as far as order 4 | *Z. Math. Phys.* **45**:23–38 |
| 1901 | Kutta | order 5; complete classification of order-4 methods; **the** classical RK4 | *Z. Math. Phys.* **46**:435–453 |
| 1952 | Curtiss & Hirschfelder | **coin "stiff"**, and introduce what is now called BDF | *PNAS* **38**(3):235–243 |
| 1963 | Dahlquist | **A-stability** named and characterised | *BIT* **3**:27–43 |
| 1967 / 1971 | Gear | BDF formalised, then popularised as variable-order, variable-step | *Numerical Initial Value Problems in ODEs*, Prentice-Hall 1971 |
| **1971** | **Gear** | **"differential-algebraic equation" enters the literature, in this title** | *IEEE Trans. Circuit Theory* **18**(1):89–95 |
| 1972 | Baumgarte | constraint stabilisation — hand index reduction with feedback | *Comput. Methods Appl. Mech. Eng.* **1**:1–16 |
| 1972 | Tarjan | strongly connected components in O(n+m) by depth-first search | *SIAM J. Comput.* **1**(2):146–160 |
| 1978 | Duff & Reid | Tarjan's algorithm implemented for **block triangularization** of a sparse matrix | *ACM TOMS* **4**:137–147 |
| 1978 | Elmqvist | **Dymola** — object-oriented acausal modelling, in a PhD thesis at Lund | Lund University |
| 1979 | Cellier | combined continuous/discrete simulation — the hybrid problem stated | ETH Zürich, Diss. ETH No. 6483 |
| 1982 | Petzold | **DASSL** — a production solver for `F(t,y,y') = 0` | Sandia report SAND82-8637, Sept 1982 |
| 1988 | Gear | index transformations; the differentiation index made precise | *SIAM J. Sci. Stat. Comput.* **9**(1):39–47 |
| **1988** | **Pantelides** | **the structural algorithm — consistent initialization and index detection** | *SIAM J. Sci. Stat. Comput.* **9**(2):213–231 |
| 1989 | Brenan, Campbell & Petzold | the DAE textbook that consolidated the theory | North-Holland |
| 1993 | Mattsson & Söderlind | **dummy derivatives** — index reduction that keeps the original variables | *SIAM J. Sci. Comput.* **14**(3):677–692 |
| 1994 | Elmqvist & Otter | **tearing** in object-oriented modelling | Proc. ESM'94, Barcelona, 326–332 |
| Spring 1996 | — | talks begin to unify Dymola, Omola, NMF and Allan | — |
| Fall 1996 | — | first Modelica design meeting | — |
| **Sept 1997** | — | **Modelica 1.0** released | — |

Two corrections this verification produced, both to claims previously asserted without a source:

- **BDF was not Gear's invention.** Curtiss & Hirschfelder introduced it in 1952, unnamed; Gear
  formalised it in 1967 and popularised it in 1971, which is why it is widely called "Gear's
  method". Writing "Gear 1971 invented BDF" would have been wrong by nineteen years.
- **Gear 1971's contribution to DAEs is the *framing*, not the formula.** The paper that coined the
  term is about applying an existing method to the mixed systems arising in network transient
  analysis — which is exactly the rung-4 capability, and a sharper claim than "Gear handled
  algebraic equations".

---

## The shape of a rung: one document and one lab

Each rung is taught by **two artefacts that do different jobs**, and neither replaces the other.
This is the architecture settled on 2026-09-24, and it is a direct answer to the question of why
`dae-pendulum.md` taught so much faster than the labs did.

| | the document | the lab |
|---|---|---|
| **shape** | a derivation, read start to finish | stations, worked in any order |
| **order** | problem → attempt → failure → fix | claim → prediction → specimen → verdict |
| **what it does well** | motivates; a reader sees *why* the mechanism had to exist | **checks**; a reader finds out they were wrong |
| **what it cannot do** | being wrong while reading is **invisible** | cannot motivate — a station assumes you know why you are there |
| **source of truth** | the derivation, hand-worked and oracle-checked | the specimen's measured stage output |

The document is written first, because it determines what the stations are *for*. The lab is
written second, from the document's commitments: **every sentence in the document that a specimen
could settle becomes a station** — the same rule the lab overviews already follow
([`fixture-labs/README.md`](fixture-labs/README.md)), pointed at a new source.

**`dae-pendulum.md` is the pattern, and it is currently carrying material that belongs lower.** Its
Aside 5a (Newton from scratch), Step 5 (backward Euler from scratch) and Aside 9a (predictors) are
**rung 1–4 content** sitting inside the rung-9 chapter, because it was written standalone with
nothing beneath it. When the lower chapters exist, that material moves down and the DAE chapter is
left to introduce only **what breaks** — which is what a rung is supposed to do.

## Scaffolding: what exists, and where the hole is

| rung | document | lab |
|---|---|---|
| 1 Euler | — | — |
| 2 order | — | — |
| 3 stiffness | — | — |
| 4 DAE | — | — |
| 5 hand index reduction | partly, inside `dae-pendulum.md` | — |
| 6 acausal modelling | — | [`connect-expansion.md`](fixture-labs/connect-expansion.md) and others |
| 7 BLT | — | [`blt-ordering.md`](fixture-labs/blt-ordering.md) |
| 8 tearing | — | [`tearing.md`](fixture-labs/tearing.md) |
| 9 automatic index reduction | [`dae-pendulum.md`](dae-pendulum.md) | [`index-reduction.md`](fixture-labs/index-reduction.md) |
| 10 events | — | [`events.md`](fixture-labs/events.md) |

**The rungs Doug most wants are the ones with the least scaffolding.** Rungs 1–4 have neither
artefact, and they are where simulation itself is learned; there is still no simulation lab.
[`ideas.md`](ideas.md) **#68** carries the instrument work, and its 2026-08-05 sequencing is
inverted by this ladder.

Note the asymmetry: the **labs cluster at rungs 6–10** (the compiler) and the **documents at rung
9** (the mathematics). That is the pipeline-ordered history of this project showing through, and it
is precisely the imbalance discovery order is meant to correct.

## What to avoid on 0.9.20, and why

These are flat-lined or killed by the **initialization defect** (`der(x) → 0`), not by anything
conceptual:

| specimen | symptom | rung it belongs to |
|---|---|---|
| `Drivetrain` | 9 states, **0 of 97** series move | 9 |
| `GearWithBrake` | 7 states, 0 of 49 move | 9/10 |
| `RcCircuit` | dies at t = 0.014 | 6 |
| `OverInitRc` | dies at t = 0.014 | 6 |

**All four come back with the 0.10.0 rebase**, confirmed on 2026-09-19. So the rebase mostly buys
breadth at rungs 6 and 9 — and possibly rung 5 as well, per the open question above. Under the new
sequence that is a stronger argument for revisiting the *don't rebase until tagged* ruling than it
was when the ruling was made.

**`CartesianPendulum` is a different failure and the rebase does not fix it.** It needs general
Pantelides, which Rumoca does not implement — [`ideas.md`](ideas.md) **#83** is Doug's own proposal
to write it. **`StabilizedPendulum` may be a third case again**, and that is the thing to measure.

## When this goes stale

The **chronology does not rot** — it is sourced and dated. Everything else does. Re-measure after
**any** of these, and update the tables rather than trusting them:

- **the 0.10.0 rebase** — five rows change outright, and rung 5's open question resolves
- **a new specimen** — it needs a rung, or it is unreachable from here
- **Rumoca gaining general Pantelides** — rung 9's wall comes down
- **a new rung** — if it has no specimen that the rung below cannot express, it is not a rung

How it was measured, all reproducible:

```
cargo run -p hrw --example stage_outcomes -- specimens/<Model>.mo
cargo run -p hrw --example gen_trace -- <Model>
```

plus a one-off `simulate_specimen` probe over `specimens/`, **with the MSL loaded** — without
libraries every MSL-based model reports *"the pipeline produced no simulable result"*, which reads
as a Rumoca failure and is not one. That mistake was made and caught on 2026-09-24.

Oracle values for `StabilizedPendulum` came from Wolfram System Modeler via `WSMLink`. Load the
package **before** the first reference to `WSMSimulate`, or the symbol binds to the session context
and the call silently returns unevaluated — which looks like a modelling failure and is not one.
