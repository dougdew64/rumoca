# Drivetrain — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- Drivetrain`, Rumoca `rev 8cdc74198` (v0.9.20);
> see [`trace/manifest.json`](trace/manifest.json). Note: `structural.json` is
> **intentionally absent** — structural analysis *fails* (singular) on the raw DAE,
> and that failure is the point; `index_reduction.json` then shows it *resolved*.
> Regenerate on edit / pin bump, then re-read.

---

## Why this specimen exists

`Drivetrain` ([`specimens/Drivetrain.mo`](../../../specimens/Drivetrain.mo)) is a
full **cross-domain power train** — electrical → rotational → translational —
built from MSL components:

```
ConstantVoltage → R → L → RotationalEMF   (electrical: a DC motor's windings)
  → rotor Inertia → IdealGear(ratio=5) → shaft Inertia
    → IdealGearR2T(ratio=200) → Mass(load) → SpringDamper → Fixed(wall)
```

It exists to trigger the phenomenon the earlier specimens *avoid*: **high
differential index**. The two **ideal** gears (`gear`, `r2t`) are *rigid* — they
impose exact ratio constraints (`φ_rotor = 5·φ_shaft`, `φ_shaft = 200·s_load`)
rather than modelling gear compliance. Rigid constraints between components that
each carry their own inertia make the accelerations *dependent*, and that pushes
the DAE above index 1. Where [`RotationalInertia`](../RotationalInertia/narrative.md)
uses the *same* MSL/connector style and stays cleanly index-1, `Drivetrain` does
not — and Rumoca's structural phase says so, precisely.

---

## The pipeline, stage by stage

The front of the pipeline succeeds fully — this is a *valid* Modelica model, and it
flattens. The verdict only lands at the structural phase.

- **Parse / Resolve / Instantiate / Typecheck →
  [`parse`](trace/parse.json) · [`resolve`](trace/resolve.json) ·
  [`instantiate`](trace/instantiate.json) · [`typecheck`](trace/typecheck.json)** —
  all succeed. Resolve binds each component to its MSL class across three domains
  (`Electrical.Analog`, `Mechanics.Rotational`, `Mechanics.Translational`);
  Instantiate expands them and their connectors. This is the heaviest instantiation
  in the collection — worth diffing against the self-contained specimens to see how
  much a cross-domain component model unfolds.
- **Flatten → [`trace/flatten.json`](trace/flatten.json)** — a large flat DAE:
  **124 variables, 94 equations**, with the full connector expansion across all
  three domains (electrical potentials/currents, rotational angles/torques,
  translational positions/forces, and the ideal-gear constraint equations). It
  flattens without error — the model is well-formed. [Phase 5](../../compiler-phases/phase5_flatten/flatten.md).

### Structural → *no report* (see [`trace/manifest.json`](trace/manifest.json))
Here the compiler stops with a **structural singularity**. The manifest records the
exact message:

> `structurally singular system: 93 matched out of 97 equations and 97 unknowns;`
> `unmatched equations: f_x[90], f_x[92], f_x[94], f_x[96];`
> `unmatched unknowns: emf.p.v, shaft.flange_a.tau, load.flange_a.f, wall.flange.f`

Read this carefully — it is a *diagnosis*, not a bug:

- **93 of 97 matched.** Maximum matching got almost all the way, then could not
  assign four equations to four remaining unknowns. A structurally non-singular
  index-1 system matches 100%; falling short is the structural signature of **high
  index** (or a genuine modelling error — here it is the former, by design).
- **The four unmatched unknowns are constraint forces/potentials at the rigid
  couplings:** `emf.p.v` (a voltage), `shaft.flange_a.tau` (the torque the ideal
  `gear` transmits into the shaft), `load.flange_a.f` (the force the ideal `r2t`
  transmits into the load), `wall.flange.f` (the reaction at the fixed wall). These
  are precisely the quantities a *rigid* constraint leaves undetermined by the
  equations at hand — you cannot solve for them until you differentiate the
  position/velocity constraints the ideal gears impose.

That differentiation is **index reduction** (Pantelides' algorithm; dummy
derivatives). So the honest output of the matching/BLT
phase on this specimen is "I cannot schedule this as-is," which is why the trace's
`structural.json` is absent while every earlier stage is present.

### Index reduction → [`trace/index_reduction.json`](trace/index_reduction.json)
This is where index reduction resolves what structural analysis could only diagnose. HRW runs Rumoca's
dummy-derivative funnel (`worker::index_reduce_for_structural_analysis`, mirroring
rumoca-sim's `prepare_dae_for_structural_analysis`: demote states → differentiate
constraints → eliminate derivative aliases → expand compound derivatives), then
re-runs the structural report on the *reduced* DAE. The verdict flips:

```
Structural (raw)      : SINGULAR — 4 constraint forces unmatched (index > 1)
Index reduction (▶)   : OK — 97 equations, 87 BLT blocks (86 scalar + 1 coupled)
```

The four undetermined constraint forces (`emf.p.v`, `shaft.flange_a.tau`,
`load.flange_a.f`, `wall.flange.f`) become solvable once the rigid position
constraints are differentiated and the redundant states are demoted to dummy
derivatives. In the app the two tabs sit side by side — **Structural** shows the
singular diagnosis, **Index reduction** shows the reduced system's BLT spy-plot —
so you watch a high-index DAE become schedulable. (For an already index-1 model the
two tabs are identical; the reduction is a no-op there.)

---

## Contrast across the notebook

- vs [`RotationalInertia`](../RotationalInertia/narrative.md): same MSL/connector
  style and both flatten fine, but that specimen is gear-free → index-1 → 12 clean
  scalar blocks; this one's ideal gears → high index → structurally singular. The
  difference is *rigid constraints*, nothing else.
- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): both are "harder than a
  plain ODE," but in different ways the structural phase distinguishes sharply —
  ProportionalLoop is **fully matched** with a coupled block (an algebraic loop you
  *tear*); Drivetrain is **under-matched** (a high-index system you must *reduce*).
  Coupled ≠ singular.
- vs [`SingleInertia`](../SingleInertia/narrative.md): the baseline index-1 ODE.
  Drivetrain is what happens when you chain such inertias through *ideal* gears.

## References
[Flatten](../../compiler-phases/phase5_flatten/flatten.md) ·
[Structural analysis](../../compiler-phases/phase7_structural_analysis/structural_analysis.md).
- C. C. Pantelides, "The consistent initialization of differential-algebraic
  systems," *SIAM J. Sci. Stat. Comput.* 9(2):213–231, 1988
  ([doi:10.1137/0909014](https://epubs.siam.org/doi/10.1137/0909014)) — the
  structural test for high index and how differentiation resolves it.
- S. E. Mattsson & G. Söderlind, "Index reduction in differential-algebraic
  equations using dummy derivatives," *SIAM J. Sci. Comput.* 14(3):677–692, 1993
  ([doi:10.1137/0914043](https://epubs.siam.org/doi/10.1137/0914043)) — the method
  Rumoca uses for index reduction.
- F. E. Cellier & E. Kofman, *Continuous System Simulation*, Springer, 2006, ISBN
  978-0-387-26102-7 ([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3))
  — DAE index, structural analysis, and index reduction in context.
- **Modelica Language Specification** §9 (connectors) —
  [specification.modelica.org](https://specification.modelica.org/).
