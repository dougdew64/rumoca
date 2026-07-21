# BenchActuator — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- BenchActuator`, Rumoca `rev 8cdc74198` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> The **Simulation** tab is on-demand (run it in the app; its output isn't part of
> the committed trace). Regenerate on a specimen edit or pin bump.

---

## Why this specimen exists

`BenchActuator` ([`specimens/BenchActuator.mo`](../../../specimens/BenchActuator.mo))
is a DC motor spinning up an inertial load — a voltage source through the winding
(`R`, `L`) into a `RotationalEMF` coupled to an `Inertia`:

```
ConstantVoltage(12) → R(1Ω) → L(1e-4 H) → RotationalEMF(k=0.1) → Inertia(J=0.05)
```

It is the **Arc 7** capstone — the **stiff** specimen, and the first one you *run*
rather than inspect. Its two subsystems evolve on wildly different timescales:

- the **winding** is fast — electrical time constant `L/R ≈ 1e-4 s`;
- the **rotor** is slow — it takes a long time for `J = 0.05` to spin up.

That ~1000× separation is **stiffness**, the reason implicit BDF integration exists
(an explicit solver would be forced to tiny steps by the fast mode for the whole
slow run). This specimen is where BDF earns its keep.

---

## The pipeline — a full-arc round trip

BenchActuator exercises *every* arc's stage in one model:

- **Flatten → [`trace/flatten.json`](trace/flatten.json)** — the flat cross-domain
  DAE (electrical + rotational), like [`Drivetrain`](../Drivetrain/narrative.md)'s
  front end but gear-free.
- **Structural → *singular*** (`trace/manifest.json`; 47/48 matched, one unmatched
  — the grounded-circuit reference redundancy). As raw DAE it isn't schedulable.
- **Index reduction → *resolved***
  ([`trace/index_reduction.json`](trace/index_reduction.json)) — **unlike**
  [`CapacitorLoop`](../CapacitorLoop/narrative.md), this singularity *is* reducible
  (a linear reference redundancy, à la Drivetrain's gears), so the funnel makes it
  solvable.
- **Solve lowering → [`trace/solve_lowering.json`](trace/solve_lowering.json)** —
  the reduced DAE lowered to a `SolveModel` (residual programs, mass matrix, layout):
  the exact object the integrator runs.
- **Simulation** *(run it)* — the payoff.

### Simulation — the stiff spin-up *(Arc 7)*
Open the **`▶ Simulation`** tab and Run (stop time 0.5 s). The Auto solver picks
**BDF** (diffsol). Two trajectories tell the stiffness story:

- **`L.i`** (winding current) jumps almost instantly toward `V/R = 12 A` on the
  fast `L/R` timescale, then eases down as the growing back-EMF (`k·w`) opposes it —
  ~10.9 A at t = 0.5.
- **`load.w`** (rotor speed) climbs **slowly** from 0 — ~11.4 rad/s at t = 0.5,
  still early in its long ramp toward the no-load speed `V/k = 120 rad/s`.

One plot, two timescales three orders of magnitude apart, integrated in a handful
of BDF steps. That is what "the simulation core" buys, and why the charter's Arc-7
specimen is a *stiff* one.

---

## Contrast across the notebook

- vs every prior specimen: they end at *compiled IR you read*; BenchActuator is the
  first you *run* and *plot*. The observatory has crossed from "how it compiles" to
  "how it runs".
- vs [`Drivetrain`](../Drivetrain/narrative.md): same electrical/rotational domains,
  but Drivetrain's **ideal gears** make it high-index and (as-is) unsimulable;
  BenchActuator's direct motor→inertia coupling is reducible and runs.
- vs [`BouncingBall`](../BouncingBall/narrative.md): the other runnable specimen —
  *hybrid* (a discontinuity at each bounce) rather than *stiff* (smooth but
  multi-timescale). Between them they cover the two hard things a simulator must
  do. (A refinement still open: **step-mode plotting**, so BouncingBall's velocity
  jump renders as a true discontinuity — `docs/ideas.md`.)

## References
[Solve lowering](../../compiler-phases/phase8_solve_lowering/solve_lowering.md) ·
[Simulation](../../compiler-phases/phase9_simulation/simulation.md).
- E. Hairer & G. Wanner, *Solving Ordinary Differential Equations II: Stiff and
  Differential-Algebraic Problems*, Springer, 1996 — the standard reference on
  stiffness and BDF.
- C. W. Gear, "Simultaneous numerical solution of differential-algebraic
  equations," *IEEE Trans. Circuit Theory* 18(1):89–95, 1971
  ([doi:10.1109/TCT.1971.1083221](https://doi.org/10.1109/TCT.1971.1083221)) — BDF
  for DAEs.
- F. E. Cellier & E. Kofman, *Continuous System Simulation*, Springer, 2006
  ([doi:10.1007/0-387-30260-3](https://link.springer.com/book/10.1007/0-387-30260-3)).
