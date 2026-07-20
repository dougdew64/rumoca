# NonlinearLoop — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- NonlinearLoop`, Rumoca `rev 8cdc74198`
> (v0.9.20), specimen fnv1a `37e9687e1cf22aeb` (see [`trace/manifest.json`](trace/manifest.json)).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`NonlinearLoop` ([`specimens/NonlinearLoop.mo`](../../../specimens/NonlinearLoop.mo))
is [`ProportionalLoop`](../ProportionalLoop/narrative.md) with one change: the plant
is **nonlinear** in the loop variable.

```modelica
error       = reference - measurement;
command     = controllerGain * error;
measurement = plantGain * command * command;   // command², not command
```

It exists to isolate a point that is easy to state and easy to forget: **structural
analysis is blind to nonlinearity.** The incidence matrix only asks *which unknowns
appear in which equation* — not how. So `command²` and `command` have the *same*
incidence, and the matching / BLT / tearing are **identical** to the linear loop.
The difference is entirely *numerical*, and it surfaces only once a solver tries to
close the torn loop — which makes this specimen the bridge from the structural arc
to the simulation/convergence work ([`docs/ideas.md`](../../ideas.md) #1).

---

## The pipeline, stage by stage

Front stages are generic (see [`docs/understanding`](../../understanding/)); the
comparison with `ProportionalLoop` is the point.

### Flatten → [`trace/flatten.json`](trace/flatten.json)
Three unknowns (`error`, `command`, `measurement`), three algebraic residuals — the
only difference from `ProportionalLoop` is in `f_x[2]` (from the trace):

| slot | residual (`0 =`) |
|------|------------------|
| `f_x[0]` | `error − (reference − measurement)` |
| `f_x[1]` | `command − controllerGain·error` |
| `f_x[2]` | `measurement − (plantGain·command)·command` |

`f_x[2]` references `command` (twice, but *structurally* once) and `measurement` —
the same incidence as the linear plant's `measurement = plantGain·command`.
[Phase 5](../../understanding/phase5_flatten/flatten.md).

### Structural → [`trace/structural.json`](trace/structural.json)
Identical to [`ProportionalLoop`](../ProportionalLoop/narrative.md): one coupled
block of size 3, torn on `command`.

```
COUPLED {error, command, measurement}
   tear = command · residual = f_x[0]
   causal: error ← f_x[1],  measurement ← f_x[2]
```

`coupled_block_count: 1`. Same orange box, same tear variable — because the
structural phase never looks at the `·command` versus `·command·command`. That is
exactly the intended lesson.

### Where the nonlinearity actually bites
Follow the tearing to its consequence. Guess `command`; then
`error = command/Kp` (linear, `f_x[1]`) and
`measurement = plantGain·command²` (nonlinear, `f_x[2]`); the residual `f_x[0]`
is `error − (reference − measurement) = command/Kp − reference + plantGain·command²`.
Setting that to zero is a **quadratic in `command`** — the torn 1-D solve is
nonlinear, so a real solver must **Newton-iterate** it (with initial guess, possible
multiple roots, and convergence that can fail). In the linear `ProportionalLoop`
the same residual is linear and solves in one step. Structurally a twin;
numerically a different animal — and the seam where a *convergence* narrative
(future work) would begin.

---

## Contrast across the notebook

- vs [`ProportionalLoop`](../ProportionalLoop/narrative.md): structurally identical
  (same block, same tear); the divergence is numerical (linear vs Newton solve).
  The cleanest possible demonstration that structure ≠ numerics.
- vs [`Drivetrain`](../Drivetrain/narrative.md): two different "hard" cases the
  structural phase treats oppositely — Drivetrain is *structurally* hard (singular,
  high index); NonlinearLoop is structurally *easy* but *numerically* hard. Knowing
  which kind of hard you have is the whole diagnostic value of the phase.

## References
[Flatten](../../understanding/phase5_flatten/flatten.md) ·
[Structural analysis](../../understanding/phase7_structural_analysis/structural_analysis.md)
· [`docs/ideas.md`](../../ideas.md) (simulation / convergence narratives).
On why structural analysis is incidence-only (independent of the equations'
nonlinearity), and on solving the resulting torn systems: F. E. Cellier & E.
Kofman, *Continuous System Simulation*, Springer, 2006
([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)).
