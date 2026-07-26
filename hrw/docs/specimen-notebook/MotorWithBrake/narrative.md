# MotorWithBrake — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- MotorWithBrake`, Rumoca `rev bbe947fe` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> The **Simulation** tab is on-demand (run it in the app; its output isn't part of
> the committed trace). Regenerate on a specimen edit or pin bump.

---

## Why this specimen exists

`MotorWithBrake` ([`specimens/MotorWithBrake.mo`](../../../specimens/MotorWithBrake.mo))
is the **end-to-end tour specimen** — a DC motor driving an inertial load with a
speed-limit event. It is designed to exercise *every* compiler phase in a single
model while producing dynamic simulation trajectories:

```
ConstantVoltage(12) → R(1Ω) → L(1e-4 H) → RotationalEMF(k=0.1) → Inertia(J=0.05)
                                                                    + when load.w > 30
```

The model combines three phenomena that drive different compiler phases:

- **Cross-domain MSL connectors** (electrical pins + rotational flanges) — exercises
  Parse, Resolve, Instantiate, Typecheck, and Flatten (connector expansion, flow sums).
- **The EMF's internal support constraint** — creates a position-level coupling that
  makes the DAE index > 1, exercising Index Reduction (1 demotion: `emf.phi`).
- **A `when`/`elsewhen` clause** (speed-limit detection) — exercises Events
  (2 conditions, 1 zero-crossing, 1 discrete-valued update).
- **Stiffness** — fast electrical (L/R ~ 0.1 ms) coupled to slow mechanical
  (J·R/k² ~ 5 s), requiring BDF integration.

It is based on [`BenchActuator`](../BenchActuator/narrative.md)'s proven EMF structure
(which simulates dynamically despite Rumoca's IC limitation), with the addition of
event logic for the Events phase.

---

## The pipeline — a full round trip

### Parse → Resolve → Instantiate → Typecheck → Flatten

The front end is straightforward: 56 lines of Modelica with 6 MSL components expand
to 84 instance entries and flatten to **61 variables and 47 equations**. The six
`connect()` statements generate equality constraints (shared voltages, shared angular
positions) and flow-sum equations (KCL for currents, torque balance at flanges). The
EMF couples the two domains: `v = k·w` and `tau = -k·i`.

### Structural → *singular* (47/48 matched)

The raw DAE is structurally singular — 47 of 48 equations match, but `emf.p.v`
remains unmatched. This is the EMF's internal support: a fixed reference frame that
constrains `emf.phi` without providing a derivative equation. The structural analysis
correctly identifies this as a high-index system.

### Index Reduction → *resolved* (1 demotion, 41 eliminations → 7×7)

The index reduction pipeline runs 10 steps:
1. **Constrained dummy derivative demotion** — demotes `emf.phi` from state to
   algebraic (the EMF support constraint).
2. **Trivial elimination** — substitutes 41 variables determined by single equations
   (e.g. `src.v → src.V`, `emf.w → load.w`, `L.p.i → L.i`).

Result: 4 states → 3 states (`L.i`, `load.phi`, `load.w`), 47 equations → 7
equations, **no algebraic loops** (all 7 BLT blocks are scalar).

### Initialization → *fails* (41/44 matched)

The IC planner finds a structurally singular initialization subsystem (41 of 44
matched). Three variables remain unmatched: `src.i`, `emf.internalSupport.flange.tau`,
`gnd.p.i`. This is a known Rumoca limitation with MSL support-flange models —
the IC system doesn't properly account for the index-reduced constraints. The
simulation succeeds anyway because the zero start values are physically consistent
(motor at rest, no current).

### Events → 2 conditions, 1 discrete update

The `when`/`elsewhen` clause expands to:
- **c[1]:** `load.w > maxSpeed` (speed-limit trigger)
- **c[2]:** `load.w < maxSpeed * 0.5` (speed-limit release)
- **overSpeed** discrete-valued update: toggled by the rising edges of c[1] and c[2]

One zero-crossing condition guides the event locator.

### Solve Lowering → SolveModel (51 variables, 48 slots)

The reduced DAE is lowered to a `SolveModel` with a flat variable layout (Y[0]
through Y[50]), compiled residual blocks, and a symbolically-derived Jacobian.

### Simulation — the stiff spin-up with events

Open the **`▶ Simulation`** tab and Run. The trajectory tells the electromechanical
energy conversion story:

- **`L.i`** (winding current) starts at 12 A (= V/R, initial back-EMF is zero),
  then drops toward ~8 A as the growing back-EMF `k·w` opposes the source voltage.
- **`load.w`** (rotor speed) climbs from 0 toward ~40 rad/s in the simulation
  window, still early in its long ramp toward the no-load speed `V/k = 120 rad/s`.
- **`overSpeed`** triggers when `load.w` crosses 30 rad/s, marking the event.

The solver diagnostics show stiffness in action: the step size starts tiny (forced
by the fast L/R = 0.1 ms electrical transient), then grows as the solver recognizes
the smooth mechanical ramp.

---

## Contrast across the notebook

- vs [`BenchActuator`](../BenchActuator/narrative.md): same EMF structure, but
  MotorWithBrake adds events (the `when`/`elsewhen` clause). BenchActuator is
  purely smooth; MotorWithBrake has discrete state transitions.
- vs [`GearWithBrake`](../GearWithBrake/narrative.md): GearWithBrake uses IdealGear
  (5 demotions, richer index reduction) but its simulation produces all-constant
  trajectories due to a failed IC solve. MotorWithBrake trades index-reduction
  depth for working simulation.
- vs [`BouncingBall`](../BouncingBall/narrative.md): BouncingBall has *reinit*
  events (state reinitialization) and dramatic trajectory discontinuities, but no
  index reduction. MotorWithBrake has index reduction but simpler events (Boolean
  tracking, no reinit). Between them they cover the two event flavors.
- vs [`Drivetrain`](../Drivetrain/narrative.md): Drivetrain has richer multi-domain
  structure (electrical + rotational + translational) and deeper index reduction
  (6 demotions), but also produces constant trajectories. MotorWithBrake is the
  tour specimen because it *simulates*.

## References
[End-to-end tour](../../compiler-phases/end_to_end_tour.md) ·
[Index reduction](../../compiler-phases/phase6_dae_construction/index_reduction.md) ·
[Structural analysis](../../compiler-phases/phase7_structural_analysis/structural_analysis.md) ·
[Simulation](../../compiler-phases/phase9_simulation/simulation.md).
