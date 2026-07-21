# RcCircuit — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- RcCircuit`, Rumoca `rev 8cdc74198` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`RcCircuit` ([`specimens/RcCircuit.mo`](../../../specimens/RcCircuit.mo)) is a
resistor–capacitor circuit — a constant voltage source driving `R` into `C`,
returned to ground:

```modelica
ConstantVoltage src(V = 5)  →  Resistor R(R = 100)  →  Capacitor C(C = 1e-3)  →  (src.n, ground)
```

It is the **Arc 5** (initialization / IC planning) specimen. A dynamic model needs
a **consistent initial state at t = 0** before the integrator can step: every one
of the flat DAE's variables — node voltages, branch currents, and the one true
state (`C.v`) — must be assigned values that satisfy *all* the equations
simultaneously. `RcCircuit` exists to show how Rumoca **plans** that initial solve
(`build_ic_plan`), including the one structural subtlety every grounded circuit
has (below). It is index-1 and simulates cleanly — the interest is entirely at
t = 0. *(The charter's RC/RL "blow-up" failure case is a deliberate later
iteration; this entry establishes the IC-plan mechanism first.)*

---

## The pipeline, stage by stage

Parse / Resolve / Instantiate / Typecheck resolve the MSL electrical components
(`Analog.Sources.ConstantVoltage`, `Analog.Basic.Resistor/Capacitor/Ground`) and
expand their connectors — the same machinery as
[`Drivetrain`](../Drivetrain/narrative.md)'s electrical stage; see
[`docs/compiler-phases`](../../compiler-phases/).

- **Flatten → [`trace/flatten.json`](trace/flatten.json)** — a flat DAE of **23
  continuous equations** in **1 state** (`C.v`, the capacitor voltage; `C.i =
  C.C·der(C.v)`). The rest are algebraic: Ohm's law, the connector potential
  equalities and flow (Kirchhoff current) sums, and the source/ground pins.
- **Structural → [`trace/structural.json`](trace/structural.json)** — index-1,
  fully matched (no algebraic loop of interest here).
- **Index reduction → [`trace/index_reduction.json`](trace/index_reduction.json)**
  — a no-op (already index-1), identical to Structural.

### Initialization → [`trace/initialization.json`](trace/initialization.json)  *(Arc 5)*
This is the arc's phase. `build_ic_plan(dae, n_states)` returns the **ordered
solve plan** for the initial values — **21 blocks**:

- **20 `ScalarDirect`** — each computes one variable by a symbolic formula from
  already-known ones, in causal order: `src.v = src.V`, `gnd.p.v = 0`,
  `src.n.v = gnd.p.v`, `src.p.v = src.v − src.n.v`, … `C.p.v = C.v − C.n.v`,
  `C.i = C.C·der(C.v)`, down to the branch currents. Read the block list as the
  literal recipe a solver runs once at t = 0.
- **1 `ScalarNewton`** — `R.i` is solved from equation 9 by a scalar Newton
  iteration (the one place a direct symbolic solve wasn't available).

**The structural subtlety — the relaxation hint.** The report also carries a
`relaxation_hint`: **drop equation 17, pin `gnd.p.i`**. This is the classic
grounded-circuit redundancy: summing every node's Kirchhoff current law and the
source/ground constraints leaves the system *structurally singular* at
initialization (one equation is a linear combination of the others), with the
ground current `gnd.p.i` undetermined. Rumoca detects this
(`build_ic_relaxation_hint`), drops the redundant equation, and pins the ground
current — exactly the "select a balanced subset" move needed to make the initial
subsystem square. Watching *that* decision is much of Arc 5's value: it is where a
young compiler's initialization logic most often goes wrong (charter §4.2.5).

---

## Contrast across the notebook

- vs the dynamic specimens ([`SingleInertia`](../SingleInertia/narrative.md) et
  al.): those are about the *trajectory*; this one is about the *starting point*.
  The IC plan is what must succeed before any of them can take a first step.
- vs [`Drivetrain`](../Drivetrain/narrative.md): same MSL electrical/connector
  machinery, but here the focus is the **Initialization** tab, not the structural
  singularity — RcCircuit is a clean index-1 circuit whose only structural
  subtlety is the initialization-time ground redundancy.

## References
[Structural analysis · IC planning](../../compiler-phases/phase7_structural_analysis/ic_plan.md).
- C. C. Pantelides, "The consistent initialization of differential-algebraic
  systems," *SIAM J. Sci. Stat. Comput.* 9(2):213–231, 1988
  ([doi:10.1137/0909014](https://epubs.siam.org/doi/10.1137/0909014)) — the
  foundational treatment of *consistent initial conditions* for a DAE.
- F. E. Cellier & E. Kofman, *Continuous System Simulation*, Springer, 2006
  ([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3))
  — initialization and the algebraic solve at t = 0 in context.
- **Modelica Language Specification** §8.6 (initialization, `initial equation`) —
  [specification.modelica.org](https://specification.modelica.org/).
