# Phase 8: Solve Lowering

## Overview

The solve-lowering phase takes the structurally-analysed DAE (an Appendix B
`Dae` plus the structural artifacts from phase 7) and lowers it into an
**execution-ready tensor IR**: the `SolveProblem`. Where the DAE IR is a
mathematical description of the system, the `SolveProblem` is a compute graph
the runtime can dispatch directly to a JIT, an MLIR/CUDA pipeline, a WebGPU
kernel, or a code-generation template that emits Python/C/Rust.

- Implementation: `crates/rumoca-phase-solve/`
- Output IR: `crates/rumoca-ir-solve/` — `SolveProblem`
- Entry point: `pub fn lower_solve_problem(dae_model: &dae::Dae) -> Result<solve::SolveProblem, LowerError>`

This phase is a **new addition in v0.9.x**. The crate name was reused: in
earlier versions `rumoca-phase-solve` housed the structural-analysis machinery,
which now lives in the renamed `rumoca-phase-structural` (phase 7). The current
`rumoca-phase-solve` does a different job entirely.

---

## Big Picture: Input and Output

```
  Dae  (index-1, from phase 6;
        structurally analysed by phase 7)
        │
        ▼
  ┌─────────────────────────────────────┐
  │     Phase 8: Solve Lowering         │
  │                                     │
  │  • Validate function calls          │
  │  • Build variable layout (y, p)     │
  │  • Eliminate dummy derivatives      │
  │  • Lower residual / derivative_rhs  │
  │    / implicit_rhs to ComputeBlocks  │
  │  • Build AD (forward-mode Jacobian) │
  │  • Lower events, clocks, discrete   │
  │    updates, initialization          │
  └─────────────────────────────────────┘
        │
        ▼
  SolveProblem  (tensor compute graph;
                schema-versioned, serialisable)
```

---

## Why a Separate Phase?

The DAE IR is excellent for mathematical reasoning — it preserves symbolic
expressions, equation origins, MLS Appendix B partitions, and connects back to
the source model. It is *not* a good direct target for high-performance
execution: every consumer (BDF integrator, CUDA kernel, MLIR pipeline, code
generator) would otherwise re-walk the expression trees, re-build variable
layouts, re-compute Jacobians, and re-decide how to lay out compute graphs.

`SolveProblem` is the canonical execution-facing artifact. It captures, once:

- A flat **variable layout** (`VarLayout`) — every scalar in the system has a
  specific slot in the state vector `y` or the parameter vector `p`.
- The **continuous residual**, **derivative right-hand side**, and
  **implicit-residual right-hand side** as `ComputeBlock`s — sequences of
  tensor-aware compute nodes.
- A **forward-mode AD** of those residuals (Jacobian-vector products), built
  symbolically during lowering rather than re-derived at runtime.
- **Initialization**, **discrete updates**, **event actions**, and **clock
  partitions** in matching `ComputeBlock` form.

`SolveProblem` is **schema-versioned** (`SOLVE_SCHEMA_VERSION` in
`rumoca-ir-solve`) and serde-serialisable, which lets it round-trip through
JSON/binary boundaries — important for the cross-language codegen and
WebAssembly workflows that consume the IR directly.

---

## The SolveProblem IR

```rust
pub struct SolveProblem {
    pub schema_version: u16,
    pub layout:         VarLayout,
    pub solve_layout:   SolveLayout,
    pub continuous:     ContinuousSolveSystem,   // residual, derivative_rhs, implicit_rhs
    pub initialization: InitializationSolveSystem,
    pub discrete:       DiscreteSolveSystem,
    pub events:         SolveEventPartition,
    pub clocks:         SolveClockPartition,
}
```

The five sub-systems correspond to the temporal partitions of the original
DAE:

- **`continuous`** holds the equations the integrator solves between events:
  - `residual`: `r(t, y, ẏ, p) = 0` for implicit DAE solvers.
  - `derivative_rhs`: `ẏ = f(t, y, p)` for explicit ODE solvers (when index 0).
  - `implicit_rhs`: helpers for the mass-matrix formulation.
- **`initialization`** holds the IC solve plan, sequenced for the runtime.
- **`discrete`** holds the assignments that fire only at event times
  (`f_z`, `f_m` from the DAE).
- **`events`** holds the zero-crossing functions, event actions (reinit,
  reset), and pre/post values.
- **`clocks`** holds clock-partition metadata for clocked Modelica systems
  (MLS §16).

### VarLayout: One Slot per Scalar

`VarLayout` assigns every scalar in the system a `ScalarSlot`. Each
`ScalarSlot` is one of four variants:

- **`Time`** — the current simulation time (a single implicit scalar).
- **`Y { index, byte_offset }`** — a slot in the state/algebraic vector `y`.
- **`P { index, byte_offset }`** — a slot in the parameter vector `p`.
- **`Constant(f64)`** — a compile-time constant folded into the layout.

Vector and array variables are scalarised, so `body.position[3]` becomes
three separate slots. This layout is the contract between the IR and every
downstream consumer: once the layout is fixed, residual code, AD code, and
integrator buffers all agree on the same indexing.

The layout is built by `build_var_layout` (in `layout.rs`), which walks the
DAE and emits a deterministic ordering (Modelica state vars first, then
algebraics, then outputs, then parameters).

---

## ComputeBlock and ComputeNode: A Tensor IR

The bodies of every residual / RHS / discrete update / event action are
expressed as `ComputeBlock`s:

```rust
pub struct ComputeBlock {
    pub nodes: Vec<ComputeNode>,
}

pub enum ComputeNode {
    ScalarPrograms(ScalarProgramBlock),  // sequence of scalar ops
    MatMul {
        lhs_ops, lhs_start, rhs_ops, rhs_start,
        m, k, n,
        lhs_sparsity: SparsityPattern,   // Dense | Diagonal | Explicit
        rhs_sparsity: SparsityPattern,
        metadata, span,
    },
    LinSolve  { setup_ops, matrix_start, rhs_start, n, next_reg, metadata, span },
    Map       { domain, output_map, base_ops, load_strides, const_strides, metadata, span },
    AffineStencil { domain, output_map, base_ops, load_strides, const_strides, metadata, span },
}
```

The five node kinds preserve enough structure to be lowered both to scalar
fallback code (for CPU backends with no tensor primitives) and to native
tensor ops (CUDA cuBLAS, WebGPU compute pipelines, MLIR `linalg`, CasADi
matrices). The serde representation is tagged-enum so a downstream consumer
can pattern-match on `ComputeNode::MatMul` and choose `gemm` instead of a
nested scalar loop. `MatMul` carries per-operand `SparsityPattern`
annotations (`Dense` by default, plus `Diagonal` and `Explicit` variants)
so backends can emit specialised kernels — for example a diagonal multiply
instead of a full GEMM — when the lowering phase proves a sparser structure.

`ScalarProgramBlock` itself is a sequence of three-address SSA ops over the
state, parameter, and scratch registers — basically a tiny tree-walk IR for
arithmetic, comparisons, builtin calls, and conditional assignments.

---

## The Lowering Pipeline

`lib.rs` orchestrates several focused lowering passes (each in its own
module). The most significant:

| Module | Purpose |
|--------|---------|
| `function_validation` | Compile-time preflight: every function call references a defined function with the right arity |
| `layout` | Build `VarLayout` — assign every scalar to a `y` or `p` slot |
| `dummy_derivative` | Eliminate `di = der(x)` alias equations (see below) |
| `lower` | Main DAE-to-`ComputeBlock` lowering for residual / derivative_rhs / implicit_rhs |
| `implicit_rhs` | Build implicit RHS rows for the mass-matrix formulation |
| `ad` | Build forward-mode automatic differentiation (Jacobian-vector product) symbolically over the lowered residual |
| `capacity` | Pre-size buffers and reserve solver slots |
| `continuous_row_targets` | Pair each continuous equation with its target row in the residual |
| `initial_values` | Lower the IC plan into the `initialization` partition |
| `discrete_pre_modes` | Compute `pre(z)` / `pre(m)` modes for discrete partitions |
| `dynamic_events`, `event_actions` | Lower zero-crossings, reinit, reset actions |
| `observation_refresh` | Build discrete observation refresh flags |
| `path_utils` | Path handling helpers for lowered variable names |
| `projection_suffix` | Projection suffix helpers for algebraic projection |
| `residual_compute_block` | Build the `ContinuousSolveSystem.residual` ComputeBlock |
| `runtime_assignments` | Lower runtime-defined assignments (parameter writes, observability) |
| `stencil` | Detect and lower affine stencils (e.g. PDE finite differences) |
| `subscript_indices` | Subscript index helpers for array scalarisation |
| `solve_model` | Optional whole-program transformation: package as a SolveModel for serialisation/codegen |
| `timing` | Lowering stage timing and profiling instrumentation |
| `appendix_b_validation` | Post-lowering structural validation against MLS Appendix B |

### Dummy-Derivative Elimination (`dummy_derivative.rs`)

When a model contains an algebraic alias for a state derivative —
`di = der(x)` is the canonical case, common in coupled-inductor or
constraint-form models — the state derivative appears in *two* roles in `f_x`:
the trivial defining equation, and constitutive equations like `v = L*der(x)`.
A naïve treatment flags the defining equation as the state derivative row and
drops it, leaving `di` with no determining equation.

Mirroring OpenModelica, Rumoca's pass instead treats `di` as an algebraic
unknown determined by the constitutive equations, with `der(x) = di` kept as
the trivial derivative link. Concretely, every occurrence of `der(x)` in
**other** equations is rewritten as `di`, so the defining equation remains
the only equation carrying `der(x)` and the constitutive equations become
derivative-free algebraic constraints for `di`.

This is **not** Mattsson-Söderlind dummy-derivative *selection* (the general
technique for choosing which derivatives become states and which become
algebraics in genuinely index-2 systems). It handles the narrower aliasing
case that arises in well-formed Modelica networks.

### Forward-Mode AD (`ad/`)

The Jacobian-vector product `J · v` is built symbolically over the lowered
`ComputeBlock`s by traversing each compute node and producing the
corresponding tangent. The result is itself a `ComputeBlock`, so the
integrator can request `J · v` evaluations through the same dispatch
machinery as residual evaluations — no separate AD runtime is needed.

Forward-mode is chosen over reverse-mode because most stiff DAE integrators
need a small number of directional derivatives per step (one per Newton
iteration column), and forward-mode is asymptotically cheaper when the
output dimension dominates.

---

## Downstream Consumers

Two main consumers receive the `SolveProblem`:

### Simulation (`rumoca-sim`)

`simulate_solve_model` (in `rumoca-sim/src/lib.rs`) runs the integrator
against a `SolveProblem`. The continuous residual and AD compute blocks are
evaluated by the configured execution backend (Cranelift JIT, the tree-walk
`rumoca-eval-solve`, MLIR, …), and the integrator (BDF, RK45) drives the
state vector forward.

### Codegen (`rumoca-phase-codegen`, "-solve" templates)

A subset of codegen templates target the `SolveProblem` rather than the DAE
directly:

- `c-solve` — portable C with a hand-rolled residual and Jacobian
- `casadi-solve` — CasADi structures populated from the compute graph
- `cranelift-solve-jit` — Rust harness around a Cranelift JIT
- `cuda-c`, `cuda-nvrtc-solve-jit` — CUDA kernels (AOT C and NVRTC JIT)
- `jax-solve` — Python/JAX with explicit residual / Jacobian functions
- `mlir` — MLIR dialect output for further compilation
- `rust-solve` — Standalone Rust harness
- `wgsl-solve` — WebGPU shader kernels

These templates use `SolveTemplateRenderer` /
`render_solve_template_with_name` (re-exported from `rumoca-compile`) rather
than the DAE-based `render_template` API. See
[phase 10 codegen](../phase10_codegen_templates/codegen_templates.md) for
how the two paths coexist.

### Execution Backends (`rumoca-exec-*`)

Three execution-backend crates consume `SolveProblem` directly without going
through a Jinja template:

- `rumoca-exec-cranelift` — JIT-compile compute blocks to native machine code
- `rumoca-exec-mlir` — emit MLIR + drive a CUDA/CPU pipeline (the actual CUDA
  driver lives here, not in `rumoca-exec-cuda`)
- `rumoca-exec-wasm` — emit WebAssembly for browser/edge execution

These are the "runtime" consumers used by the simulator's
`simulate_solve_model` path; the codegen templates are the "ahead-of-time"
consumers that emit source code for a different host.

---

## Why This Phase Is Separate from Phase 7

Phases 7 and 8 both consume the DAE and both produce intermediate
representations, but they answer different questions:

| | Phase 7 (Structural Analysis) | Phase 8 (Solve Lowering) |
|-|-------------------------------|--------------------------|
| Output | `SortedDae` (BLT blocks, IC plan, matching) | `SolveProblem` (compute graph) |
| Operates on | Equation/variable *structure* | Equation *content* (expressions) |
| Mutates DAE? | No (produces analysis alongside) | No (produces new IR) |
| Output is a contract for | Code generation logic | Runtime execution |
| Schema-versioned? | No | Yes |

Phase 7's output tells you *which* equation solves *which* unknown and *in
what order*. Phase 8's output is the actual residual code that, when paired
with an integrator, runs the simulation.

A given DAE can be lowered to `SolveProblem` even without phase 7 having
run, but phase 8 internally relies on several phase-7 utilities
(`Incidence`, `BltBlock`, `EquationRef`, `UnknownId`) — the two phases share
the same crate boundary (`rumoca-phase-structural`) for these structural
types.

---

## Key Files

| File | Purpose |
|------|---------|
| `rumoca-phase-solve/src/lib.rs` | Public API: `lower_solve_problem`, `lower_solve_artifacts`, etc. |
| `rumoca-phase-solve/src/layout.rs` | Build `VarLayout` (variable → y/p slot) |
| `rumoca-phase-solve/src/lower/` | Main lowering pipeline |
| `rumoca-phase-solve/src/ad/` | Forward-mode AD over compute blocks |
| `rumoca-phase-solve/src/dummy_derivative.rs` | Eliminate `di = der(x)` aliases |
| `rumoca-phase-solve/src/residual_compute_block.rs` | Build continuous residual block |
| `rumoca-phase-solve/src/event_actions.rs` | Lower event-time actions |
| `rumoca-phase-solve/src/stencil/` | Detect and lower affine stencils (PDEs) |
| `rumoca-ir-solve/src/lib.rs` | `SolveProblem`, `ComputeBlock`, `ComputeNode` types |
| `rumoca-ir-solve/src/visitor.rs` | Visitor trait for walking SolveProblem |
| `rumoca-eval-solve/` | Tree-walk evaluator for SolveProblem (reference backend) |
