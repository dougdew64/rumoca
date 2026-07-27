# GearWithBrake — a compilation narrative

*A per-specimen lab-notebook entry: the story of one specimen's trip through the
Rumoca pipeline, told against the committed [`trace/`](trace/) (ground truth) and
foregrounding what **this** specimen is designed to make interesting.*

> **Provenance.** Written against `trace/` from
> `cargo run --example gen_trace -- GearWithBrake`, Rumoca `rev bbe947fe` (v0.9.20);
> specimen hash + per-stage status in [`trace/manifest.json`](trace/manifest.json).
> Regenerate on a specimen edit or pin bump, then re-read against the diff.

---

## Why this specimen exists

`GearWithBrake` ([`specimens/GearWithBrake.mo`](../../../specimens/GearWithBrake.mo))
is the **end-to-end tour specimen** — designed to exercise *every* compiler phase with
non-trivial content. A constant-torque motor drives a rotor through an ideal gear
(5:1 ratio) to a load inertia restrained by a spring-damper anchored to a fixed frame.
When the load speed exceeds a threshold, a braking torque engages via a `when` clause;
the brake releases at half the threshold, producing a limit-cycle with discrete events.

```modelica
when load.w > maxSpeed then
  braking = true;
elsewhen load.w < maxSpeed * 0.5 then
  braking = false;
end when;
```

What makes it interesting: the ideal gear constraint (`phi_a = ratio * phi_b`) creates
a **high-index** DAE requiring index reduction, while the brake logic creates **discrete
events** — so this single specimen forces nontrivial work in structural analysis, index
reduction, *and* events.

---

## The pipeline, stage by stage

### Parse → Resolve → Instantiate → Typecheck → Flatten

These early phases are clean but non-trivial — the model uses seven MSL rotational
components (`ConstantTorque`, `Inertia` ×2, `IdealGear`, `SpringDamper`, `Fixed`,
`Torque`) with parameter modifications (`J = 0.01`, `ratio = 5`, `c = 100`, etc.).
Flattening expands the rotational connectors (each carrying `phi` / `tau` as
potential / flow) into explicit connection equations: equality constraints on `phi`
and flow-sum-to-zero on `tau` at each connection point.

### Structural → fails (expected)

The structural analysis **fails**: 41 matched out of 44 equations and 44 unknowns.
Three equations remain unmatched — this is the signature of a **high-index** system.
The ideal gear's position constraint `phi_a = ratio * phi_b` introduces an algebraic
relationship between positions that propagates to velocities and accelerations, making
the system structurally singular at the position level.

This failure is expected and is what makes index reduction necessary.

### Index reduction → [`trace/index_reduction.json`](trace/index_reduction.json)

Index reduction is the headline: the system starts with **7 states** and ends with **2**.
Five states are demoted to algebraic variables:

| Demoted state | Why |
|---|---|
| `motor.phi` | Constrained by the gear ratio to `rotor.phi` |
| `rotor.phi` | Constrained via the gear to `load.phi` |
| `rotor.w` | Velocity-level consequence of the position constraint |
| `load.phi` | Constrained through the spring to the fixed frame |
| `load.w` | Velocity-level consequence |

The two surviving states are `spring.phi_rel` and `spring.w_rel` — the spring's
relative displacement and velocity, which are the true independent degrees of freedom
after the gear and fixed-frame constraints are absorbed.

The reduction uses Pantelides' algorithm: it differentiates the constraint equations
(the gear ratio and fixed-frame connections), introduces dummy derivatives for the
demoted variables, and eliminates them. The `funnel_completed: true` flag confirms
the algorithm converged. After reduction, the 11-equation, 11-unknown system has a
complete matching and 7 BLT blocks (1 coupled).

### Initialization → fails

IC planning fails with structural singularity: 33 matched out of 37 equations and
37 unknowns. The unmatched variables (`motor.phi_support`, `motor.w`, `motor.tau`,
`rotor.flange_a.phi`) are related to the gear constraint chain — the initialization
system inherits the position-level coupling from the continuous equations. The model
simulates regardless because the simulation can start from the initial values provided
in the component declarations.

### Events → [`trace/events.json`](trace/events.json)

The brake logic produces **4 condition relations** — corresponding to the `when` /
`elsewhen` guards and the `if` expressions in the brake torque calculation:

```
condition_equations      : 4
relations                : 4
discrete_valued_updates  : 1    → the Boolean `braking` variable
```

The `braking` Boolean is a **discrete valued update** (not a discrete real): it
changes at event instants when `load.w` crosses the speed thresholds. The brake
torque itself (`brakeTorque.tau`) is a continuous algebraic variable controlled by
`if braking then ...`, so it changes value at events but is not itself a discrete
variable.

### Solve lowering → [`trace/solve_lowering.json`](trace/solve_lowering.json)

The DAE is lowered to a `SolveModel` with 7 states. The `has_discontinuities: true`
flag reflects the discrete brake events, which signal to the solver that it must
handle reinitializations at event crossings.

### Simulation → [`trace/simulation.json`](trace/simulation.json)

501 time points over `t = [0, 2]`, 49 variables, 7 states. The simulation uses BDF
(backward differentiation) — appropriate for this stiff system (the spring constant
`c = 100` combined with the 5:1 gear ratio creates stiff dynamics). The
`has_discontinuities: true` flag causes the simulator to perform event detection and
state reinitialisation at brake engagement/release transitions.

---

## What to look for in HRW

1. **Structural tab** — shows the structurally singular system (41/44 match) and the
   three unmatched equations, demonstrating *why* index reduction is needed.
2. **Index Reduction tab** — the before/after split view: 7 states → 2, with the
   demoted states and elimination steps visible.
3. **Events tab** — the 4 conditions and the discrete `braking` variable.
4. **Simulation plot** — look for the discontinuous speed transitions when the brake
   engages/releases; the step-mode plotting (enabled by `has_discontinuities`) renders
   them as breaks in the velocity trace rather than interpolated ramps.

---

## Cross-references

- [Compiler phases: index reduction](../../compiler-phases/06-structural-index-reduction/)
  — theory of Pantelides' algorithm and the concept of structural index.
- [Compiler phases: events](../../compiler-phases/08-events/) — how `when` / `reinit` /
  `pre` lower into the DAE's discrete overlay.
- [BouncingBall narrative](../BouncingBall/narrative.md) — the simpler hybrid specimen;
  compare its single `when` + `reinit` with GearWithBrake's multi-condition brake logic.
- [Drivetrain narrative](../Drivetrain/narrative.md) — the other index-reduction specimen;
  compare its Pantelides reduction with GearWithBrake's more aggressive 7→2 demotion.
