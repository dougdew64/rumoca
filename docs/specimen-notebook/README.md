# HRW Lab Notebook

One directory per specimen — `docs/specimen-notebook/<Model>/` — recording how Rumoca
compiles it, anchored to the specimen's actual IR. This is **HRW's own
specimen-driven record**, kept deliberately distinct from `docs/compiler-phases`
(Doug's canonical, *general* explanation of each compiler phase). The notebook is
*specimen-specific*; `docs/compiler-phases` is *phase-generic*, and the narratives
link back to it.

Each entry has two parts:

- **`trace/`** — the durable **compilation trace**: the IR of every pipeline stage
  (`parse … structural`) as JSON, plus a `manifest.json` stamping the Rumoca rev
  and an FNV-1a hash of the specimen. This is *ground truth*, produced by the app's
  own worker path so it is byte-identical to what the running observatory shows.
- **`narrative.md`** — the **compilation narrative**: the grounded story of *this*
  specimen's trip through the pipeline, foregrounding the phenomenon the specimen
  was authored to trigger, citing specific trace locations, and linking outward to
  the relevant `docs/compiler-phases` chapters and external math references. Claude
  writes and maintains it against the trace; every "interesting" claim points at a
  trace entry or Rumoca source, so a trace diff flags any prose that has gone stale.

## Adding / regenerating an entry

```text
cargo run --example gen_trace -- <Model>     # (re)writes docs/specimen-notebook/<Model>/trace/
```

Then write `narrative.md` (start from [`_TEMPLATE.md`](_TEMPLATE.md)), grounded in
the freshly generated trace. On a Rumoca pin bump, regenerate every entry's trace
and re-read its narrative against the diff — see
[`docs/updating-rumoca.md`](../updating-rumoca.md) step 5.

In the app, the right-hand panel's **"Read: specimen narrative"** button opens the
current specimen's `narrative.md` (shown only when one exists) — the visual channel
pointing straight at the durable story.

## Entries

Roughly in order of increasing structural interest:

- [`SingleInertia`](SingleInertia/narrative.md) — the minimal index-1 ODE
  (self-contained); what a *state* looks like in the DAE. All scalar blocks.
- [`RotationalInertia`](RotationalInertia/narrative.md) — same physics via **MSL
  connectors**; the connector-expansion story (still index-1, all scalar blocks).
- [`ProportionalLoop`](ProportionalLoop/narrative.md) — idealized algebraic
  feedback loop → one **coupled** block (tearing). The pilot entry.
- [`NonlinearLoop`](NonlinearLoop/narrative.md) — the same loop with a **nonlinear**
  plant: *structurally identical*, numerically Newton. Structure ≠ numerics.
- [`MixedLoop`](MixedLoop/narrative.md) — a loop **bracketed by scalar solves** →
  scalar + coupled + scalar; makes **BLT ordering** visible.
- [`TwoLoops`](TwoLoops/narrative.md) — two algebraic loops in series → **two
  coupled blocks**, sequenced by their data dependency.
- [`Drivetrain`](Drivetrain/narrative.md) — cross-domain train with **ideal gears**
  → **high index** (structurally singular), then **index-reduced** to a solvable
  system (Arc 4): the Structural vs Index-reduction tabs show before → after.
- [`RcCircuit`](RcCircuit/narrative.md) — an RC circuit; the **Initialization / IC
  planning** specimen (Arc 5): the t=0 solve plan + the ground-redundancy
  relaxation hint.
- [`CapacitorLoop`](CapacitorLoop/narrative.md) — the Arc-5 **blow-up**: a capacitor
  across an ideal source can't be consistently initialized; Structural + Index
  reduction both stay singular (contrast Drivetrain: reducible).
