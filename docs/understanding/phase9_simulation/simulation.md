# Phase 9: Simulation

## Overview

The simulation subsystem takes a `Dae` (or, increasingly, a pre-lowered
`SolveModel` from phase 8) and integrates it forward in time, handling
continuous dynamics, discrete events, and consistent initialization.

As of v0.9.x the simulation stack is split across several focused crates:

- `crates/rumoca-sim/` — facade and orchestration: dispatches to the
  configured backend, manages the runner loop, exposes `SimStepper`
- `crates/rumoca-solver/` — backend-neutral primitives: `SimResult`,
  `SimOptions`, `SimSolverMode`, `DiffsolMethod`, runtime schedules,
  timeout budgets
- `crates/rumoca-solver-diffsol/` — the diffsol-based BDF / ESDIRK34 /
  TR-BDF2 backend (gated by the `solver-diffsol` feature)
- `crates/rumoca-solver-rk45/` — the explicit RK45 backend (gated by
  the `solver-rk45` feature)
- `crates/rumoca-eval-dae/` — residual, Jacobian, and mass-matrix
  evaluation for the DAE path
- `crates/rumoca-eval-solve/` — equivalent evaluator for the SolveModel
  path (also exposes the `nan_trace` runtime)
- External: **diffsol** — the actual ODE/DAE integrator wrapped by
  `rumoca-solver-diffsol`

---

## Big Picture: Input and Output

```
  Dae  (index-1, from phase 6)              SolveModel  (from phase 8)
        │                                            │
        └──────────────┬─────────────────────────────┘
                       ▼
  ┌─────────────────────────────────────┐
  │        Phase 9: Simulation          │
  │                                     │
  │  • Lower DAE → SolveModel if        │
  │    needed (via phase 8)             │
  │  • Build PreparedSimulation         │
  │    (compile residual/Jacobian)      │
  │  • IC solving via the IC plan       │
  │  • diffsol BDF/ESDIRK/TRBDF or RK45 │
  │  • Mass matrix for index-1 DAEs     │
  │  • Event detection (zero crossings) │
  │  • when-clause/reinit at events     │
  └─────────────────────────────────────┘
        │
        ▼
  SimResult  (time series + variable trajectories)
```

---

## Two Entry Points

`rumoca-sim` exposes two top-level entry points:

- `simulate_with_diagnostics(dae_model, opts)` — takes a DAE, lowers it
  to a `SolveModel` internally via phase 8, then simulates. This is the
  CLI's path and the one used by most callers.
- `simulate_solve_model(model, opts)` — takes an already-lowered
  `SolveModel`. Used by the lazy WASM addon and by callers that
  serialise the `SolveModel` to JSON and ship it across a process
  boundary; the addon deserialises and simulates without carrying the
  compiler.

Both entry points dispatch to the same backends based on
`opts.solver_mode: SimSolverMode`.

---

## Solver Methods Available

`SimSolverMode` chooses the broad solver family, and (for BDF-family
solvers) `DiffsolMethod` chooses the specific integrator:

| Mode | Backend crate | Method | Best for |
|------|--------------|--------|---------|
| `SimSolverMode::Bdf` + `DiffsolMethod::Bdf` | `rumoca-solver-diffsol` | BDF (Backward Differentiation Formula) | Stiff systems (most DAEs) |
| `SimSolverMode::Bdf` + `DiffsolMethod::Esdirk34` | `rumoca-solver-diffsol` | ESDIRK 3(4) | Moderately stiff with smoothness |
| `SimSolverMode::Bdf` + `DiffsolMethod::TrBdf2` | `rumoca-solver-diffsol` | TR-BDF2 | Switching, fast dynamics |
| `SimSolverMode::RkLike` | `rumoca-solver-rk45` | Adaptive RK45 | Non-stiff or testing |
| `SimSolverMode::Auto` | (selects automatically) | — | Default for the CLI |

ESDIRK34 and TR-BDF2 were added in v0.9.x via PR #227 ("Expose ESDIRK34 /
TR-BDF2 implicit solvers").

The `Auto` mode falls back to diffsol/BDF if the `solver-diffsol` feature
is enabled, otherwise to RK45. Both solver crates are independently
feature-gated so a WASM build can ship one without the other.

---

## PreparedSimulation

`PreparedSimulation` (in `rumoca-sim/src/diffsol.rs`, behind the
`solver-diffsol` feature) separates the expensive build step from the
cheap run step:

```rust
pub struct PreparedSimulation {
    // … backend-specific state, kernels, layout …
}
```

**Build once, run many times**. After `build_simulation(dae, opts)` returns
a `PreparedSimulation`, you can:

- Call `run_prepared_simulation(prepared, opts)` multiple times with
  different time spans
- Adjust parameters via `refresh_prepared_vectors` for sweeps without
  recompiling the residual / Jacobian kernels

This avoids re-running the DAE→SolveModel lowering, the AD construction,
and any JIT compilation for each run. Parameter sweeps in particular
benefit substantially.

---

## Residual Evaluation (rumoca-eval-dae)

The continuous DAE is in implicit form:

```
F(t, y, ẏ) = 0
```

where `y = [x; algebraics; outputs]` is the combined state vector and
`ẏ = [ẋ; 0; 0]` (only states have derivatives).

### Compiled Kernels

`rumoca-eval-dae` JIT-compiles evaluation kernels at build time from the DAE's
expressions:

- `eval_compiled_runtime_residual()` — evaluates `F(t, y, ẏ)` → residual vector
- `eval_compiled_initial_residual()` — same but with `initial() = true`
- `eval_compiled_runtime_jacobian()` — evaluates `∂F/∂y` (Jacobian matrix)

The compilation happens inside `build_compiled_runtime_newton_context()` during
`PreparedSimulation` construction.

---

## Jacobian Computation

Rumoca computes **exact Jacobians** via forward-mode automatic differentiation
(not finite differences).

### Dual Numbers

Forward-mode AD works by pairing each floating-point value with a derivative
component:

```
x = (value, ∂x/∂seed)
```

Arithmetic on these `Dual` pairs propagates derivatives through every operation.
To compute the full Jacobian (n outputs, n inputs), run the residual n times,
each time with a different "seed" direction.

### Two Evaluation Modes

```rust
fn eval_init_jacobian_vector(ctx, v, out) {
    if ctx.use_initial {
        eval_compiled_initial_jacobian(...)  // t=0: initial() = true
    } else {
        eval_compiled_runtime_jacobian(...)  // t>0: normal
    }
}
```

The distinction matters because some Modelica expressions like `if initial() then ...`
branch differently during initialization.

---

## Mass Matrix for Index-1 DAEs

### The Problem

diffsol expects problems in the form `M·ẏ = F(t, y)` where M is a mass matrix.
For a Modelica DAE:
- ODE rows (explicit derivative): `M[i,i] = 1` → `ẋᵢ = F_i(t, y)`
- Algebraic rows (constraint): `M[i,i] = 0` → `0 = F_i(t, y)`

### Coefficient Extraction (during phase 8 solve lowering)

The mass-matrix coefficients are now extracted during **solve lowering**
(phase 8) and shipped as part of the `SolveProblem` — see
`lower_solve_artifacts_with_mass_matrix` in `rumoca-phase-solve`. The
runtime then wraps the result as a `PreparedMassMatrix` (in
`rumoca-solver/src/runtime/mass_matrix.rs`) with three kinds: `Identity`,
`Diagonal`, and `Dense`.

For each equation `0 = f_x[i]`, Rumoca symbolically extracts the coefficient
of `der(xⱼ)` to build M:

```
coeff_expr_for_derivative(equation_rhs, state_name)
```

This handles:
- Unary minus: `0 = -der(x) + …` → coefficient = −1
- Additive chain: `0 = a*der(x) + b*der(y) + …` → coefficient of `der(x)` = a
- Products and nested sums

The resulting coefficient expressions are evaluated at (t, y) to build M
dynamically if the coefficients depend on state (common in mechanical systems
with coordinate transformations).

---

## Event Detection (Zero-Crossing Functions)

Modelica uses **zero-crossing detection** to find the exact time when a
condition changes. Rumoca registers root conditions with diffsol:

```rust
.root_conditions(
    move |y, p, t, roots| {
        call_compiled_root_conditions(
            &compiled_synthetic_root,
            y.as_slice(), p.as_slice(), t,
            roots.as_mut_slice(),
        );
    },
)
```

Each `Dae.relation[i]` expression becomes one component of the `roots` vector.
diffsol monitors sign changes in `roots[i]` during integration and triggers
event handling when a sign change is detected.

---

## Event Settling (`diffsol/event_settle.rs`)

When diffsol detects that a root condition crossed zero at time `tₑ`:

1. **Locate event time**: diffsol bisects to find the precise `tₑ`
2. **Pre-event snapshot**: Capture `pre(z)` and `pre(m)` values (the values
   from the previous event instant, needed by Modelica's `pre()` operator)
3. **Evaluate discrete updates**: Apply `f_m` equations at `tₑ` to compute
   new `m` values. Apply `f_z` to compute new `z` values.
4. **Reinitialize algebraics**: Re-solve the algebraic equations with the new
   discrete variable values to restore consistency
5. **Resume integration** from `tₑ` with updated state

Multiple simultaneous events (e.g., several clock ticks at the same time) are
grouped and settled together.

---

## Reinit()

The Modelica `reinit(x, expr)` operator sets a continuous state to a new value
at an event:

```modelica
when h < 0 then
  reinit(v, -0.8 * pre(v));   // bounce: reverse velocity
end when;
```

Reinit assignments appear in the `f_m` equations after DAE construction (since
they modify state at events). During event settling, the corresponding state
component of the `y` vector is updated in-place before the solver resumes.

---

## Initial Condition (IC) Solving

### Problem

Before integration starts at `t = 0`, Rumoca must find a **consistent**
initial state — one that satisfies both `f_x(x₀, ẋ₀, y₀, …) = 0` and the
initial equations `initial_equations`.

### Process (`solver-diffsol/init_projection.rs`)

1. **Set states from start values**: Use `Variable.start` and `Variable.fixed`
   attributes to seed initial values for `x` and `y`.

2. **Forward-substitute parameters and constants**: Evaluate the parameter/
   constant portion of the initial equation system in order (easy, no iterations).

3. **Sequence algebraic initialization** using the IC plan from structural
   analysis (see [phase 7](../phase7_structural_analysis/ic_plan.md)):
   - `ScalarDirect`: substitute symbolically
   - `ScalarNewton`: single-variable Newton solve
   - `TornBlock`: iterate on tear variables
   - `CoupledLM`: Levenberg-Marquardt for the full coupled system

4. **Consistent projection**: After setting initial state values, solve the
   algebraic equations to make `y₀` consistent with `x₀`.
   Uses the compiled Newton context with `use_initial = true`.

All IC solving is wrapped in a **timeout budget** (`TimeoutBudget`). If the IC
solver does not converge within the budget, a descriptive error is returned.

---

## SimResult

```rust
pub struct SimResult {
    pub times: Vec<f64>,                          // output sample times
    pub names: Vec<String>,                       // variable names
    pub data: Vec<Vec<f64>>,                      // data[var_idx][time_idx]
    pub n_states: usize,                          // count of continuous states
    pub variable_meta: Vec<SimVariableMeta>,      // metadata per variable
}

pub struct SimVariableMeta {
    pub name: String,
    pub role: String,         // "state", "algebraic", "output", "parameter", …
    pub is_state: bool,
    pub value_type: Option<String>,
    pub variability: Option<String>,
    pub unit: Option<String>,
    pub start: Option<String>,
    pub min, max, nominal, fixed, description: …
}
```

### Collection During Integration

- At each **output time** (controlled by output step size), `OutputBuffers::record()`
  appends the current `(t, y)` to the time and data vectors.
- **Discrete channels** are evaluated after integration completes (post-hoc) by
  re-running `evaluate_runtime_discrete_channels()` over the stored trajectory.
- **Algebraic channels** are refreshed at each observation instant to satisfy
  the algebraic equations (so output includes consistent algebraic values, not
  solver-internal temporary values).
- **Eliminated variables**: Variables that were simplified away during structural
  analysis are reconstructed from their defining equations via
  `reconstruct_eliminated()`.

---

## Integration Flow Diagram

```
prepare(dae)
    │
    ├── build compiled residual/Jacobian kernels
    ├── extract mass matrix coefficients
    ├── compile root condition functions
    └── produce PreparedSimulation

run(start, stop, step)
    │
    ├── 1. IC Solving
    │      ├── seed from start values
    │      ├── forward-substitute params/constants
    │      └── IC plan: direct → Newton → torn → LM
    │
    ├── 2. Integration loop (diffsol BDF/RK)
    │      ├── residual: F(t, y, ẏ) using compiled kernel
    │      ├── Jacobian: ∂F/∂y via forward-mode AD
    │      ├── mass matrix: M from coefficient extraction
    │      └── root conditions monitored for sign changes
    │
    ├── 3. Event handling (on zero-crossing)
    │      ├── snapshot pre() values
    │      ├── evaluate f_m, f_z at tₑ
    │      ├── apply reinit() updates
    │      └── re-project algebraics
    │
    └── 4. Output collection
           ├── record y at each output time
           ├── post-process discrete channels
           └── reconstruct eliminated variables
               └── return SimResult
```

---

---

## Realtime Stepper API (`SimStepper`)

For interactive or software-in-the-loop (SIL) use cases, `rumoca-sim` exposes
`SimStepper` — a step-by-step interface where external code drives time and
injects inputs between steps.

```rust
pub struct SimStepper { ... }

impl SimStepper {
    pub fn set_input(&mut self, name: &str, value: f64) -> Result<(), SimError>;
    pub fn set_inputs(&mut self, inputs: &[(&str, f64)]) -> Result<(), SimError>;
    pub fn step(&mut self, dt: f64) -> Result<(), SimError>;
    // state inspection via StepperState / state_json()
}
```

Key behaviour:

- When `set_input()` changes a value, a **dirty flag** is set. On the next
  `step()` call, the BDF solver's polynomial history is flushed
  (`reset_solver_history()`) before stepping so the discontinuous input does not
  cause extrapolation divergence.
- Repeated `set_input()` calls with the same value do **not** trigger a history
  reset (dirty flag only set on actual change).
- `dt ≤ 0` is guarded; floating-point time accumulation is handled explicitly.

`SimStepper` is re-exported from `rumoca-sim/src/lib.rs` alongside `StepperOptions`
and `StepperState`.

---

## FlatBuffer SIL Simulation (split across multiple crates in v0.9.x)

The CLI command `rumoca sim-fb` runs hardware-in-the-loop /
software-in-the-loop simulation where an external autopilot process
communicates via FlatBuffers. In v0.9.x what was previously the single
`rumoca-sim-fb` crate has been broken into focused crates aligned with
the responsibilities involved:

- `rumoca-codec-flatbuffers` — FlatBuffer pack/unpack via `.bfbs` schema
  reflection (no codegen required)
- `rumoca-signal-frame` — signal-frame data structures shared across
  codecs and transports
- `rumoca-input-keyboard`, `rumoca-input-gamepad`, `rumoca-input` —
  pluggable input sources for RC channels and keyboard control
- `rumoca-transport-udp`, `rumoca-transport-websocket` — transports
  for the autopilot connection and the browser visualisation
- `rumoca-worker` — background simulation worker used by the playground
  and the runner

The simulation loop itself lives in `rumoca-sim`'s `runner/` module
(feature-gated on `runner`). Architecture is unchanged:

- Rust lockstep physics loop: drain motor commands → `SimStepper.step()` →
  send sensor readings
- Autopilot process management: auto-start, restart on reset, clean shutdown
- 3D browser visualization via WebSocket state broadcast
- Realtime toggle accessible from browser

Usage: `rumoca sim-fb --config sil_config.toml MyModel.mo`

---

## WASM Compatibility

The `TimeoutBudget` used by the IC solver previously relied on `std::time::Instant`,
which panics on `wasm32` targets. This has been replaced with the `instant` crate,
which transparently falls back to `performance.now()` in browser environments.
This makes `rumoca-sim` usable from `rumoca-bind-wasm` without modification.

---

## Output Variable Preservation

During structural analysis (Phase 7), the solver may **eliminate** variables
whose defining equation is trivially solvable. Output variables (those with
`causality = Output`) are now **exempt** from elimination, even if their
equation is trivially solvable. This keeps them visible in codegen output and
in `SimResult`.

The exemption applies to both single-equation elimination and BLT-block
elimination paths. Non-trivial output expressions (e.g., `y = max(a, b)`) were
always preserved; only trivial alias outputs (a single variable reference or its
negation) were previously at risk.

---

## Key Files

| File | Purpose |
|------|---------|
| `rumoca-sim/src/lib.rs` | Facade: `simulate_with_diagnostics`, `simulate_solve_model`, dispatch |
| `rumoca-sim/src/diffsol.rs` | diffsol-backed `PreparedSimulation`, `build_simulation` |
| `rumoca-sim/src/sim_stepper.rs` | `SimStepper`, `StepperState`, realtime stepping API |
| `rumoca-sim/src/solve_lowering/` | DAE→SolveModel lowering for the simulation path; probes & diagnostics |
| `rumoca-sim/src/runner/` | Interactive runner loop, SIL orchestration |
| `rumoca-solver/src/lib.rs` | `SimResult`, `SimOptions`, `SimSolverMode`, `DiffsolMethod`, `TimeoutBudget` |
| `rumoca-solver/src/runtime/mass_matrix.rs` | `PreparedMassMatrix`, `solve_mass_matrix` (Identity/Diagonal/Dense) |
| `rumoca-solver-diffsol/src/lib.rs` | Diffsol backend: BDF / ESDIRK34 / TR-BDF2 |
| `rumoca-solver-diffsol/src/init_projection.rs` | IC solver / consistent initialisation |
| `rumoca-solver-rk45/src/lib.rs` | Explicit RK45 backend |
| `rumoca-eval-dae/src/lib.rs` | Compiled residual / Jacobian / root-condition evaluation (DAE path) |
| `rumoca-eval-solve/src/lib.rs` | Equivalent for SolveModel path; `nan_trace` |
| `rumoca-codec-flatbuffers/` | FlatBuffer codec for SIL |
| `rumoca-transport-{udp,websocket}/` | Transport layers for SIL |
| `rumoca-input-{gamepad,keyboard}/` | Input sources for interactive simulation |
