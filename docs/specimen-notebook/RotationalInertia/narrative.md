# RotationalInertia — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- RotationalInertia`, Rumoca `rev 8cdc74198`
> (v0.9.20); see [`trace/manifest.json`](trace/manifest.json). Regenerate on a
> specimen edit or Rumoca pin bump, then re-read against the diff.

---

## Why this specimen exists

`RotationalInertia` ([`specimens/RotationalInertia.mo`](../../../specimens/RotationalInertia.mo))
is one rotating inertia driven by an ideal torque source — the **same physics** as
[`SingleInertia`](../SingleInertia/narrative.md) — but assembled from **MSL
components wired by `connect`**:

```modelica
Modelica.Mechanics.Rotational.Components.Inertia inertia(J = 1.0);
Modelica.Mechanics.Rotational.Sources.Torque torque;
Modelica.Blocks.Sources.Constant tau(k = 1.0);
equation
  connect(tau.y, torque.tau);
  connect(torque.flange, inertia.flange_a);
```

Its purpose is to show what **connectors and library components** do to a model on
the way down — the machinery `SingleInertia` deliberately avoids. Same answer, very
different flat model: `SingleInertia` has 2 equations, this has **12**. The extra
ten are the connector semantics made explicit, and Rumoca *labels* each one — which
makes this the notebook's clearest window on `connect`.

---

## The pipeline, stage by stage

- **Parse → [`trace/parse.json`](trace/parse.json)** — three component
  declarations (typed by their MSL paths) and two `connect(...)` equations, verbatim.
- **Resolve → [`trace/resolve.json`](trace/resolve.json)** — the payoff of loading
  the MSL as source roots: each component's *type* resolves to its library class
  (`inertia` → `…Rotational.Components.Inertia`, `torque` → `…Sources.Torque`,
  `tau` → `…Blocks.Sources.Constant`). These are the `type_def_id`s the app shows
  inline and lets you "Go to". [Phase 2](../../compiler-phases/phase2_resolve_and_scope/resolve_and_scope.md).
- **Instantiate → [`trace/instantiate.json`](trace/instantiate.json)** — here the
  stage *earns its keep* (unlike in the self-contained specimens): each MSL class is
  expanded into an instance, pulling in its flanges, internal variables, and
  equations. [Phase 4](../../compiler-phases/phase4_instantiate/instantiate.md).
- **Typecheck → [`trace/typecheck.json`](trace/typecheck.json)** — resolves
  component `type_id`s and evaluates dimensions on the instanced overlay; diff vs
  `instantiate.json` is the app's green highlight.
  [Phase 3](../../compiler-phases/phase3_typecheck_and_dims/typecheck_and_dims.md).

### Flatten → [`trace/flatten.json`](trace/flatten.json)
15 variables and **12** residual equations. The single hub-and-torque physics is
still in there — `f_x[4]` is Newton's law, `inertia.J · inertia.a =
inertia.flange_a.tau + inertia.flange_b.tau`, and `f_x[2]`, `f_x[3]` are the state
relations `inertia.w = der(inertia.phi)`, `inertia.a = der(inertia.w)` — but it is
now surrounded by the **connector-generated** equations. Three kinds are worth
naming (Rumoca's own labels, visible in the structural report, say exactly what
they are):

- **Potential equality** — connected potentials are set equal:
  `f_x[10]: torque.flange.phi = inertia.flange_a.phi` (the shaft angle is shared).
- **Flow sum** — connected flows sum to zero (Kirchhoff for mechanics):
  `f_x[8]: torque.flange.tau + inertia.flange_a.tau = 0`.
- **Unconnected flow** — an open flange carries no flow:
  `f_x[11]: inertia.flange_b.tau = 0`.

Plus the signal `connect(tau.y, torque.tau)` becoming `f_x[9]: tau.y = torque.tau`,
and the `Constant`'s own `f_x[7]: tau.y = tau.k`. That is the entire content of
"connectors": potentials equate, flows balance, opens are zero.
[Phase 5](../../compiler-phases/phase5_flatten/flatten.md).

### Structural → [`trace/structural.json`](trace/structural.json)
Despite the richer flat model, the *structure* is as simple as `SingleInertia`:
**12 scalar blocks, `coupled_block_count: 0`** — no algebraic loop. The BLT order
is a clean causal chain; reading the report's blocks top to bottom:

```
tau.y ← f_x[7]  →  torque.tau ← f_x[9]  →  torque.flange.tau ← f_x[6]  →
inertia.flange_a.tau ← f_x[8, flow sum]  →  … →  der(inertia.phi), der(inertia.w)
```

Signal source first, propagated through the connections into the torque balance,
ending at the two derivatives for the integrator. In the spy-plot: **12 green
diagonal cells, no orange box.** So connectors added *equations* but no *coupling* —
an index-1 ODE, same class as `SingleInertia`, just with more bookkeeping.

*(This is the specimen behind the earlier debugger/`explain` walkthrough — `f_x[7]`,
`tau.y = tau.k`, was the scalar block captured from the spy-plot.)*

---

## Contrast across the notebook

- vs [`SingleInertia`](../SingleInertia/narrative.md): identical physics, 2 vs 12
  equations. The delta is *entirely* connector semantics — nothing physical.
- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): all scalar blocks here
  vs one coupled block there. States + a tree of connections sort sequentially; a
  feedback loop of algebraic relations does not.
- vs [`Drivetrain`](../Drivetrain/narrative.md): same MSL/connector style, but
  Drivetrain's ideal gears push it to **high index** (structurally singular). This
  specimen's gear-free single inertia stays cleanly index-1.

## References
[Flatten](../../compiler-phases/phase5_flatten/flatten.md) ·
[Structural analysis](../../compiler-phases/phase7_structural_analysis/structural_analysis.md).
Connector semantics (potential equality / flow sum): **Modelica Language
Specification** §9, [specification.modelica.org](https://specification.modelica.org/).
Structural analysis / BLT of the resulting DAE: F. E. Cellier & E. Kofman,
*Continuous System Simulation*, Springer, 2006
([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3));
R. E. Tarjan, "Depth-first search and linear graph algorithms," *SIAM J. Comput.*
1(2):146–160, 1972 ([doi:10.1137/0201010](https://epubs.siam.org/doi/10.1137/0201010)).
