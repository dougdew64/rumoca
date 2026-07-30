# HRW Lab Notebook

One directory per specimen — `docs/specimen-notebook/<Model>/` — recording how Rumoca
compiles it, anchored to the specimen's actual IR. This is **HRW's own
specimen-driven record**, kept distinct from `docs/compiler-phases` (the
*phase-generic* material). **Both are written by Claude** — an earlier version of
this file credited the phase docs to Doug, which was wrong; see `hrw/CLAUDE.md`.

Each entry has two parts:

- **`trace/`** — the durable **compilation + simulation trace**: the IR of every
  pipeline stage (`parse … solve_lowering`) as JSON, a `simulation.json` with
  trajectory summaries (variable names, initial/final/min/max values, state vs
  algebraic, discontinuity flag), and a `manifest.json` stamping the Rumoca rev,
  an FNV-1a hash of the specimen, and the simulation outcome. This is *ground
  truth*, produced by the app's own worker path so it is byte-identical to what
  the running observatory produces. Specimens that don't compile through solve
  lowering skip simulation; the manifest records the skip reason.
- **`purpose.md`** — **why this specimen exists**, plus the questions it has
  answered. Short by design.

  It replaced `narrative.md` on **2026-07-29**. That file told the story of the
  specimen's trip through the pipeline, and it was retired for the reason recorded
  in `docs/ideas.md` #42: **Claude regenerates that explanation on demand, so
  storing it buys nothing and costs staleness.** The failure was not theoretical —
  `end_to_end_tour.md` described a 7x7 incidence matrix on a tab that shows 48
  equations, and nothing caught it because nothing checks prose.

  **Narrowed again 2026-07-29**, after Doug asked what the Purpose tab was actually
  for. The first conversion kept ~25 lines per specimen explaining the *mechanism* —
  which is the same regenerable prose the narratives were retired for, surviving in
  the file that replaced them. Evidence it was doing no work: when Doug asked why
  `CapacitorLoop`'s structural phase failed, Claude never opened its `purpose.md`
  and read the stage files instead. 630 lines became 317.

  So each entry now answers exactly two questions:

  - **Authored to trigger** — the phenomenon someone made this file *for*. Genuinely
    unregenerable: the code never says what it is for.
  - **Where it has been used** — **links into** [`../question-ledger.md`](../question-ledger.md),
    never copies. The per-specimen log the first conversion created duplicated the
    ledger, and two places to append means the forgotten one eventually becomes the
    one somebody trusts.

## Adding / regenerating an entry

```text
cargo run --example gen_trace -- <Model>     # (re)writes docs/specimen-notebook/<Model>/trace/
```

Then write `purpose.md` (start from [`_TEMPLATE.md`](_TEMPLATE.md)) — **intent only**:
a few lines on the phenomenon it was authored to trigger, and no walkthrough, no
mechanism, no numbers. Numbers live in `trace/`; mechanism is regenerated on demand.
When the specimen answers a question, add a one-line *link* under "Where it has been
used" and put the entry itself in the ledger.

On a Rumoca pin bump, regenerate every entry's trace. **There is no longer prose
to re-read against the diff**, which is the point: the trace is generated and the
purpose note makes no claims a pin bump can invalidate. See
[`docs/updating-rumoca.md`](../updating-rumoca.md) step 5.

HRW renders `purpose.md` itself — the **Purpose** tab of the specimen view — so it
does not need to be opened separately in VS Code.

## Entries

Roughly in order of increasing structural interest:

- [`SingleInertia`](SingleInertia/purpose.md) — the minimal index-1 ODE
  (self-contained); what a *state* looks like in the DAE. All scalar blocks.
- [`RotationalInertia`](RotationalInertia/purpose.md) — same physics via **MSL
  connectors**; the connector-expansion story (still index-1, all scalar blocks).
- [`ProportionalLoop`](ProportionalLoop/purpose.md) — idealized algebraic
  feedback loop → one **coupled** block (tearing). The pilot entry.
- [`NonlinearLoop`](NonlinearLoop/purpose.md) — the same loop with a **nonlinear**
  plant: *structurally identical*, numerically Newton. Structure ≠ numerics.
- [`MixedLoop`](MixedLoop/purpose.md) — a loop **bracketed by scalar solves** →
  scalar + coupled + scalar; makes **BLT ordering** visible.
- [`TwoLoops`](TwoLoops/purpose.md) — two algebraic loops in series → **two
  coupled blocks**, sequenced by their data dependency.
- [`Drivetrain`](Drivetrain/purpose.md) — cross-domain train with **ideal gears**
  → **high index** (structurally singular), then **index-reduced** to a solvable
  system: the Structural vs Index-reduction tabs show before → after.
- [`RcCircuit`](RcCircuit/purpose.md) — an RC circuit; the **Initialization / IC
  planning** specimen: the t=0 solve plan + the ground-redundancy relaxation hint.
- [`CapacitorLoop`](CapacitorLoop/purpose.md) — the **initialization blow-up**: a
  capacitor across an ideal source can't be consistently initialized; Structural +
  Index reduction both stay singular (contrast Drivetrain: reducible).
- [`OverInitRc`](OverInitRc/purpose.md) — the **initialization-determinacy
  blow-up**: a clean RC with conflicting `initial equation`s → the Initialization
  tab flags an **over-determined** init (idea #6). Structurally fine; wrong at t=0.
- [`BouncingBall`](BouncingBall/purpose.md) — the **hybrid** specimen: a
  `when h <= 0 then reinit(v, …)` bounce → the Events tab shows the condition
  (`h <= 0`) + the discrete reinit. The first specimen with a non-empty Events tab.
- [`BenchActuator`](BenchActuator/purpose.md) — the **stiff** specimen: a DC
  motor spinning up an inertial load (fast winding L/R vs slow rotor J). The first
  specimen you **run** — the Simulation tab plots the trajectories (BDF).
