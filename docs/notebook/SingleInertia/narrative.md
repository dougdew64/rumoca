# SingleInertia — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- SingleInertia`, Rumoca `rev 8cdc74198`
> (v0.9.20); see [`trace/manifest.json`](trace/manifest.json). Regenerate on a
> specimen edit or Rumoca pin bump, then re-read against the diff.

---

## Why this specimen exists

`SingleInertia` ([`specimens/SingleInertia.mo`](../../../specimens/SingleInertia.mo))
is the **smallest dynamic model** in the collection: one rotating inertia driven
by a constant torque, written *by hand* in the portable subset — no MSL, no
connectors.

```modelica
parameter Real J = 1.0;   parameter Real tau = 1.0;
Real phi(start = 0.0);    Real w(start = 0.0);
equation
  der(phi) = w;
  J * der(w) = tau;
```

Its job in the notebook is to show, in the cleanest possible form, **what a
*state* looks like once the model is a DAE** — and to be the self-contained twin
of [`RotationalInertia`](../RotationalInertia/narrative.md), which builds the
*same physics* out of MSL components. Reading the two side by side isolates
exactly what connectors and library components cost (and add). It is also the
opposite pole from [`ProportionalLoop`](../ProportionalLoop/narrative.md): that
specimen has *no* states and an algebraic loop; this one is a pure ODE that sorts
into a straight line.

---

## The pipeline, stage by stage

The early stages are generic (see [`docs/understanding`](../../understanding/));
the interesting content is at Flatten → Structural.

- **Parse → [`trace/parse.json`](trace/parse.json)** — the AST as written: two
  parameters, two `Real` variables with `start` attributes, two equations. All
  `def_id`s null. [Phase 1](../../understanding/phase1_parsing_and_ast/parsing_and_ast.md).
- **Resolve → [`trace/resolve.json`](trace/resolve.json)** — identities assigned;
  each `Real` resolves to the builtin, each component gets a `def_id`.
  [Phase 2](../../understanding/phase2_resolve_and_scope/resolve_and_scope.md).
- **Instantiate / Typecheck → [`trace/instantiate.json`](trace/instantiate.json),
  [`trace/typecheck.json`](trace/typecheck.json)** — near pass-throughs: the model
  is already flat and scalar, so there is no hierarchy to unfold and dimensions are
  trivially scalar. [Phase 4](../../understanding/phase4_instantiate/instantiate.md) ·
  [Phase 3](../../understanding/phase3_typecheck_and_dims/typecheck_and_dims.md).

### Flatten → [`trace/flatten.json`](trace/flatten.json)
Four variables — `J`, `tau` (**parameters**, known) and `phi`, `w` (**states**) —
and two residual equations (rendered from the trace; `der(·)` shows as a builtin
call):

| slot | residual (`0 =`) | i.e. |
|------|------------------|------|
| `f_x[0]` | `der(phi) − w` | `der(phi) = w` |
| `f_x[1]` | `J · der(w) − tau` | `J · der(w) = tau` |

The unknowns the structural phase must solve for are the **highest derivatives**
`der(phi)` and `der(w)` — *that* is what a state contributes to the DAE: not the
state value itself (the integrator supplies that), but its derivative, to be
determined each step. [Phase 5](../../understanding/phase5_flatten/flatten.md).

### Structural → [`trace/structural.json`](trace/structural.json)
Matching pairs each equation with the derivative it determines, and Tarjan finds
**no cycles** — two independent scalar blocks:

```
f_x[0]  →  der(phi)      (scalar block)
f_x[1]  →  der(w)        (scalar block)
```

`coupled_block_count: 0`. In spy-plot terms this is **two green diagonal cells and
no orange box** — the whole system is a two-step sequential evaluation: read the
states, compute `der(w) = tau/J` and `der(phi) = w`, hand both to the integrator.
This is the textbook shape of an **explicit, index-1 ODE**, and the baseline every
other specimen is measured against.

---

## Contrast across the notebook

- vs [`RotationalInertia`](../RotationalInertia/narrative.md): *identical physics*,
  but built from MSL `Inertia` + `Torque` + `Constant` through connectors. That
  specimen flattens to **12** equations (connector flow-sums, potential equalities,
  unconnected-flow zeros) where this one has **2** — the price and structure of
  component-based modeling, for the same answer.
- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): that model has zero
  states and an algebraic loop (one coupled block); this one is all states and no
  loop (all scalar blocks). States *break* algebraic loops — the integrator is the
  "tear."
- vs [`Drivetrain`](../Drivetrain/narrative.md): still index-1 and fully matched
  here; Drivetrain is high-index (structurally singular) because its ideal gears
  add rigid constraints. Same phase, opposite verdict.

## References
[Flatten](../../understanding/phase5_flatten/flatten.md) ·
[Structural analysis](../../understanding/phase7_structural_analysis/structural_analysis.md).
For DAE index and why an *explicit ODE* is index-1: F. E. Cellier & E. Kofman,
*Continuous System Simulation*, Springer, 2006, ISBN 978-0-387-26102-7
([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)).
