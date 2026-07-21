# CapacitorLoop — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- CapacitorLoop`, Rumoca `rev 8cdc74198` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> Note: `structural.json` and `index_reduction.json` are **absent** — both fail
> (singular) on this specimen, which is the point. Regenerate on edit / pin bump.

---

## Why this specimen exists

`CapacitorLoop` ([`specimens/CapacitorLoop.mo`](../../../specimens/CapacitorLoop.mo))
is a capacitor connected **directly across an ideal voltage source** — no resistor:

```modelica
ConstantVoltage src(V = 5)  →  Capacitor C  (src.p→C.p, src.n→C.n, ground)
```

It is the *blow-up* specimen — the "here's where it fails and why" case,
the RC counterpart to the charter's resurrected 2025 initialization bug. A
capacitor's voltage `C.v` is a **state**: it wants a free initial value and
evolves by `C.i = C·der(C.v)`. But wiring it straight across an ideal source
**algebraically pins** it, `C.v = src.V = 5`. So `C.v` is simultaneously a free
state *and* a fixed algebraic quantity — there is **no consistent initial state**
(and, physically, the capacitor is inert, pinned to 5 V, doing nothing). This is a
degenerate model, and a good compiler should refuse to schedule it. The
observatory shows exactly that.

---

## The pipeline — where it fails

- **Flatten → [`trace/flatten.json`](trace/flatten.json)** — succeeds. The model
  is *syntactically* fine: 14 continuous equations, `C.v` recorded as the one
  state. Nothing here signals trouble — the pathology is structural, not textual.
- **Structural → *singular*** (see [`trace/manifest.json`](trace/manifest.json);
  no `structural.json`). Maximum matching fails: **13 of 14** equations matched,
  with `gnd.p.i` (the ground current) left unmatchable. The pinned-state loop
  leaves the current/reference subsystem structurally deficient — the DAE cannot
  be scheduled as-is.
- **Index reduction → *still singular*** (no `index_reduction.json`). This is the
  decisive contrast with [`Drivetrain`](../Drivetrain/narrative.md): there, index
  reduction turned a singular high-index DAE into a solvable one, because its
  singularity came from *linear gear constraints* the funnel can differentiate and
  demote. Here the funnel runs and the system is **still singular** (7 of 8 after
  reduction, same `gnd.p.i`). Index reduction rescues a genuine high-index system;
  it does not rescue an **ill-posed** one. That failure-to-rescue is the blow-up,
  made visible.
- **Initialization → [`trace/initialization.json`](trace/initialization.json)** —
  `build_ic_plan` still emits a plan (9 blocks, including a torn algebraic block)
  because it plans the algebraic subsystem regardless. **Do not trust it here:** it
  sits on a structurally singular DAE, and the red Structural / Index-reduction
  verdicts are the real story. (An honest gap this exposes — see below.)

---

## Contrast across the notebook

- vs [`RcCircuit`](../RcCircuit/narrative.md): add a resistor and the same circuit
  is clean index-1 with a well-formed IC plan. Remove it — pin the capacitor to the
  source — and it becomes unschedulable. One component is the difference between a
  solvable initialization and a blow-up.
- vs [`Drivetrain`](../Drivetrain/narrative.md): both are singular, but Drivetrain
  is *reducible* (linear high index) and CapacitorLoop is *ill-posed* (index
  reduction can't help). Same phase, opposite outcome — the two together teach what
  index reduction can and cannot do.

## Two kinds of blow-up

CapacitorLoop's failure surfaces at **Structural / Index reduction**, not in the
**Initialization** tab — and the Initialization tab still shows a (untrustworthy)
plan. A *different* initialization failure — a **user-over-determined** init, e.g.
conflicting `initial equation`s on an otherwise clean index-1 DAE — is a distinct
class the [`OverInitRc`](../OverInitRc/narrative.md) specimen covers: the
Initialization stage now flags it via its **determinacy** check
([`docs/ideas.md`](../../ideas.md) #6, implemented). CapacitorLoop is the
*structural* blow-up; OverInitRc is the *initialization-determinacy* blow-up.

## References
[Structural analysis · IC planning](../../compiler-phases/phase7_structural_analysis/ic_plan.md).
- C. C. Pantelides, "The consistent initialization of differential-algebraic
  systems," *SIAM J. Sci. Stat. Comput.* 9(2):213–231, 1988
  ([doi:10.1137/0909014](https://epubs.siam.org/doi/10.1137/0909014)) — consistent
  vs inconsistent initial conditions for a DAE.
- **Modelica Language Specification** §8.6 (initialization) —
  [specification.modelica.org](https://specification.modelica.org/).
