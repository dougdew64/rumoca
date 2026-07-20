# MixedLoop — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- MixedLoop`, Rumoca `rev 8cdc74198` (v0.9.20),
> specimen fnv1a `2ed4068b62d27261` (see [`trace/manifest.json`](trace/manifest.json)).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`MixedLoop` ([`specimens/MixedLoop.mo`](../../../specimens/MixedLoop.mo)) is
[`ProportionalLoop`](../ProportionalLoop/narrative.md) with a scalar computation
*bracketing* the loop — a pre-scaling of the reference and a post-scaling of the
output:

```modelica
setpoint    = sensorGain * reference;     // before the loop
error       = setpoint - measurement;     // ┐
command     = controllerGain * error;     // ├ the loop
measurement = plantGain * command;        // ┘
result      = outputGain * measurement;   // after the loop
```

Neither bracketing equation is part of the loop, so this is the notebook's first
**mixed** structure: scalar solves *and* a coupled block in the same system. It
exists to make **BLT ordering visible** — the earlier specimens are either all
scalar ([`SingleInertia`](../SingleInertia/narrative.md),
[`RotationalInertia`](../RotationalInertia/narrative.md)) or one all-consuming
coupled block ([`ProportionalLoop`](../ProportionalLoop/narrative.md)), so nothing
forced the block *order* to matter. Here it does.

---

## The pipeline, stage by stage

Parse / Resolve / Instantiate / Typecheck are generic and near pass-throughs for a
self-contained scalar model (see [`docs/understanding`](../../understanding/) and
the [`SingleInertia`](../SingleInertia/narrative.md) walk). The action is at the
end.

### Flatten → [`trace/flatten.json`](trace/flatten.json)
Ten variables — five parameters and five unknowns (`setpoint`, `error`, `command`,
`measurement`, `result`) — and five residual equations, all algebraic (rendered
from the trace):

| slot | residual (`0 =`) | i.e. |
|------|------------------|------|
| `f_x[0]` | `setpoint − sensorGain·reference` | `setpoint = sensorGain·reference` |
| `f_x[1]` | `error − (setpoint − measurement)` | `error = setpoint − measurement` |
| `f_x[2]` | `command − controllerGain·error` | `command = Kp·error` |
| `f_x[3]` | `measurement − plantGain·command` | `measurement = plantGain·command` |
| `f_x[4]` | `result − outputGain·measurement` | `result = outputGain·measurement` |

`setpoint` depends only on a parameter; `result` depends only on `measurement`.
Neither is in the loop. [Phase 5](../../understanding/phase5_flatten/flatten.md).

### Structural → [`trace/structural.json`](trace/structural.json)
Matching + Tarjan sort the five equations into **three blocks in BLT order**:

```
1.  scalar   f_x[0]  →  setpoint                     (source: needs only a parameter)
2.  COUPLED  {error, command, measurement}           (the 3×3 algebraic loop)
       tear = command · residual = f_x[1]
       causal: error ← f_x[2],  measurement ← f_x[3]
3.  scalar   f_x[4]  →  result                        (sink: needs only the loop's result)
```

`coupled_block_count: 1`, but now **two scalar blocks bracket it**. In the
spy-plot this is a `5×5` matrix reading **green cell · orange 3×3 box · green
cell** down the diagonal — the first entry where you *see* the block sequence.

The ordering is the lesson: BLT is *block-lower-triangular*, so a block may only
depend on blocks solved before it. `setpoint` has no prerequisites → it goes
first. The loop needs `setpoint` → it goes second (and is torn on `command`, just
as in `ProportionalLoop`). `result` needs the loop's `measurement` → it goes last.
A solver walks the blocks top to bottom; the coupled one is the only place it must
iterate.

---

## Contrast across the notebook

- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): the loop is *identical*
  (same 3×3 coupled block, same tear on `command`); MixedLoop just wraps it in
  scalar work, so the spy-plot gains the bracketing green cells and a visible order.
- vs [`SingleInertia`](../SingleInertia/narrative.md): that model is *all* scalar
  blocks (a straight line, no loop); this one interleaves a loop into the line.
- vs [`TwoLoops`](../TwoLoops/narrative.md): TwoLoops has *two* coupled blocks in
  sequence; MixedLoop has one, flanked by scalars. Between them they exercise every
  BLT arrangement short of high index.

## References
[Flatten](../../understanding/phase5_flatten/flatten.md) ·
[Structural analysis](../../understanding/phase7_structural_analysis/structural_analysis.md).
On BLT (block-lower-triangular) ordering via SCCs: R. E. Tarjan, "Depth-first
search and linear graph algorithms," *SIAM J. Comput.* 1(2):146–160, 1972
([doi:10.1137/0201010](https://epubs.siam.org/doi/10.1137/0201010)); F. E. Cellier
& E. Kofman, *Continuous System Simulation*, Springer, 2006
([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)).
