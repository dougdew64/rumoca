# Rumoca: High-Level Overview

> This document is the starting point of a top-down, iterative series of notes
> exploring how Rumoca works. Each subsequent document drills deeper into a
> specific phase or algorithm.

---

## What Rumoca Is

**Rumoca** is a Modelica compiler written in Rust. Its primary goal is not to be
a simulator (like OpenModelica or Dymola), but to act as a **semantic frontend**
that transforms Modelica models into portable symbolic systems usable across
modern scientific-computing ecosystems (CasADi, Julia/ModelingToolkit, JAX,
SymPy, FMI, ONNX, and others).

Version: 0.9.20 | License: Apache 2.0

---

## The Big Picture: What Goes In, What Comes Out

```
Modelica source (.mo)
        │
        ▼
  ┌──────────────────────────────────────────────┐
  │           Rumoca Compiler Pipeline           │
  │  Parse → Resolve → Instantiate →              │
  │  Typecheck → Flatten → DAE →                  │
  │  Structural Analysis → Solve Lowering        │
  └──────────────────────────────────────────────┘
        │                    │
        ▼                    ▼
  Simulation            Generated Code
  (BDF/ESDIRK/          (CasADi/Julia/JAX/FMI/
   TR-BDF2/RK45)         ONNX/C/CUDA/WGSL/…)
```

Two intermediate forms anchor the pipeline:

- The **DAE** (Differential-Algebraic Equation system), formulated exactly
  per **MLS Appendix B**. The DAE is what's produced by phase 6 and consumed
  by both the symbolic-codegen path and the solve-lowering path.
- The **SolveProblem** (new in v0.9.x), a tensor compute graph produced by
  phase 8 and consumed by runtime execution backends (Cranelift JIT, MLIR,
  CUDA, WebGPU) and by execution-oriented codegen templates.

---

## The Nine Compiler Phases

| # | Phase crate | Input | Output | Doc dir |
|---|-------------|-------|--------|---------|
| 1 | `rumoca-phase-parse` | `.mo` text | `StoredDefinition` (AST) | `phase1_parsing_and_ast/` |
| 2 | `rumoca-phase-resolve` | AST | AST + `DefId`s + scope tree | `phase2_resolve_and_scope/` |
| 3 | `rumoca-phase-instantiate` | Resolved AST + model name | Instance tree (hierarchy applied) | `phase4_instantiate/` |
| 4 | `rumoca-phase-typecheck` | Instance tree | AST + `TypeId`s, array dims | `phase3_typecheck_and_dims/` |
| 5 | `rumoca-phase-flatten` | Instance tree | `Model` (flat equations, qualified names) | `phase5_flatten/` |
| 6 | `rumoca-phase-dae` | `Model` | `Dae` (MLS B.1 variable/equation partition; index-reduced) | `phase6_dae_construction/` |
| 7 | `rumoca-phase-structural` | `Dae` | `SortedDae` (BLT blocks, IC plan, matching) | `phase7_structural_analysis/` |
| 8 | `rumoca-phase-solve` | `Dae` (+ structural artifacts) | `SolveProblem` (tensor compute graph) | `phase8_solve_lowering/` |
| 9 | `rumoca-phase-codegen` | DAE or `SolveProblem` + template | Source code (multiple files per target) | `phase10_codegen_templates/` |

Phases are **strictly sequential with no back-edges**: each phase consumes
the previous phase's IR crate and produces its own. This enables caching,
incremental compilation, and clear error attribution.

**Why instantiation runs before type checking:** Rumoca intentionally
typechecks *after* instantiation so that array dimensions and modifier
values are evaluated with full modification context — a dimension like
`n` can only be resolved once the enclosing model's modifications have
been applied (MLS §10.1). The doc directories retain their original
numbering (`phase3_typecheck…`, `phase4_instantiate…`) for link stability,
but the actual pipeline order is Instantiate then Typecheck, as defined
in `rumoca-compile/PIPELINE_INVARIANTS.md`.

The runtime simulator (`rumoca-sim`) is not a numbered phase crate — it is a
separate consumer of either the `Dae` or the `SolveProblem`. The doc
directory series gives it its own slot (`phase9_simulation/`) for
navigability between phase 8 (solve lowering) and phase 10 (codegen).

**v0.9.x note on naming.** The crate that used to be `rumoca-phase-solve`
(which did structural analysis) was renamed `rumoca-phase-structural` in
v0.9.x. The name `rumoca-phase-solve` was reused for the new
solve-lowering crate that produces the `SolveProblem` IR. This is why
phase 7's crate name changed and phase 8 is a new phase entirely.

**v0.9.x note on index reduction.** Index reduction — symbolic
differentiation of constraint equations to make a high-index DAE
solvable — runs as part of phase 6 (DAE construction). In earlier
versions the implementation lived in `rumoca-sim`; v0.9.x moved it into
`rumoca-phase-structural::dae_prepare` where it conceptually belongs.
The orchestrating driver still lives in `rumoca-sim` for the simulation
path and in `rumoca-phase-dae::prepare_dae_for_codegen` for the codegen
path; both call the same `rumoca-phase-structural::dae_prepare`
helpers. See [index_reduction.md](phase6_dae_construction/index_reduction.md)
for details.

---

## The Six IR Crates

| Crate | Role |
|-------|------|
| `rumoca-core` | Shared primitives: `Token`, `OpBinary`, `Variability`, `Causality`, `BuiltinFunction`, `Span`, `Reference`, `Expression` building blocks |
| `rumoca-ir-ast` | Class tree: `ClassDef`, `Component`, `Expression`, `Equation`, scope tree, type table |
| `rumoca-ir-flat` | Flat model: `Variable` (qualified names), equations in residual form, algorithm sections |
| `rumoca-ir-dae` | DAE system (MLS B.1): nested partition structs for variables, continuous/discrete/condition/event/clock partitions |
| `rumoca-ir-solve` | Solve IR (v0.9.x): `SolveProblem`, `VarLayout`, `ComputeBlock` / `ComputeNode` (tensor IR for execution backends) |
| `rumoca-ir-galec` | GALEC/eFMI IR: Algorithm Code description for embedded code generation (eFMI standard) |

---

## The Canonical DAE Form (MLS Appendix B)

This is the mathematical core that everything flows toward.

### Variable Partitions

| Symbol | Name | Meaning |
|--------|------|---------|
| `x(t)` | states | Continuous variables that appear differentiated (`der(x)`) |
| `ẋ(t)` | derivatives | Time derivatives of states |
| `y(t)` | algebraics | Continuous variables that do *not* appear differentiated |
| `u(t)` | inputs | Externally-driven continuous signals |
| `w(t)` | outputs | Exposed continuous signals |
| `p` | parameters | Fixed scalars set before simulation |
| `c` | constants | Compile-time fixed scalars |
| `z(tₑ)` | discrete reals | Real-valued variables updated only at events |
| `m(tₑ)` | discrete-valued | Boolean/Integer variables updated only at events |

### Equation Groups

```
Continuous (between events):
  0 = f_x( x, ẋ, y, u, p, t, z, m, pre(z), pre(m) )    [implicit DAE]

At events (tₑ):
  z  := f_z( x, y, u, p, t, z, m, pre(z), pre(m) )     [discrete Real update]
  m  := f_m( x, y, u, p, t, z, m, pre(z), pre(m) )     [discrete-valued update]
  c  := f_c( relation(x, y, u, p, t, z, m) )            [condition update]
```

`pre(z)` and `pre(m)` are the values of those variables at the *previous* event
instant — this is how Modelica models memory across events.

---

## Parsing (Phase 1)

Rumoca parses Modelica source text into an Abstract Syntax Tree (AST) using the
[**Parol**](https://crates.io/crates/parol) LL(k) parser generator. The Modelica
grammar is written in Parol's DSL; Parol generates the parse-table state machine
and a `ModelicaGrammarTrait` at build time, and Rumoca implements that trait with
one method per production to assemble higher-level IR nodes from the lexer
output. The result is a `StoredDefinition` — the AST that subsequent phases
consume.

Full treatment: [parsing_and_ast.md](phase1_parsing_and_ast/parsing_and_ast.md).

---

## Name Resolution and Scope (Phase 2)

Resolution turns string-based name references in the AST into stable integer
**`DefId`s** and builds the **scope tree** that encodes which names are visible
where. Builtins (`Real`, `Integer`, `Boolean`, `der`, `sin`, …) receive the
lowest DefIds so `id < BUILTIN_CUTOFF` is an O(1) builtin check. Resolution
also processes `import` statements, follows `extends` clauses to bring inherited
members into scope, and detects inheritance cycles. The output is a
`ResolvedTree` — the same AST with every reference annotated by its DefId and
every declaration placed in a scope.

Full treatment: [resolve_and_scope.md](phase2_resolve_and_scope/resolve_and_scope.md).

---

## Instantiation (Phase 3)

Instantiation takes the resolved AST plus the name of a top-level model and
produces an **`InstancedTree`** — a concrete object built by recursively
applying all modifications, evaluating structural parameters, processing
`extends` clauses, and resolving `inner`/`outer` references (MLS §5.4).
Modifications override outside-in: an enclosing modifier takes priority over an
inner default. Redeclarations are resolved before descending into children. The
result is a tree of component instances with concrete types and parameter values.

Full treatment: [instantiate.md](phase4_instantiate/instantiate.md).

---

## Type Checking and Dimensions (Phase 4)

Type checking assigns a **`TypeId`** to every expression and component,
evaluates array dimensions via the multi-pass dimension evaluator required by
MLS §10.1, identifies **structural parameters** (those whose values determine
other variables' shapes), and validates variability and causality constraints.
TypeIds are interned in a `TypeTable` so type comparisons are O(1). Typecheck
runs *after* instantiation so that dimensions and modifiers are validated with
full modification context. The output is a `TypedTree`.

Full treatment: [typecheck_and_dims.md](phase3_typecheck_and_dims/typecheck_and_dims.md).

---

## Flattening (Phase 5)

Flattening walks the instance tree and emits a **`flat::Model`** — a single
flat structure with no class hierarchy. Variable names become globally
qualified (e.g. `body.position.x`); `connect()` statements expand into equality
equations for potential variables and sum-to-zero equations for `flow`
variables (MLS §9.2 — the Kirchhoff-style conservation laws); all equations
are converted to **residual form** `0 = expr`; for-loop and algorithm
structure is preserved as metadata for downstream consumers.

Full treatment: [flatten.md](phase5_flatten/flatten.md).

---

## DAE Construction (Phase 6)

DAE construction converts the flat model into the canonical MLS Appendix B form
described above. Variables are classified into the eight partitions (states,
algebraics, inputs, outputs, parameters, constants, discrete reals,
discrete-valued); equations are routed into the four groups (`f_x`, `f_z`,
`f_m`, `f_c`); `when`-clauses are expanded into conditional assignments;
algorithm sections are lowered; relational expressions used as guards become
zero-crossing relations.

This phase also includes a **DAE-preparation sub-pass** that runs after
`to_dae()` produces the raw DAE: index reduction (symbolic differentiation of
algebraic constraints to expose missing state derivatives), state-demotion
sweeps, derivative-alias elimination, and compound-derivative expansion. The
output is an **index-1** DAE ready for structural analysis. As of v0.9.x the
prep code lives in `rumoca-phase-structural::dae_prepare`; see the index
reduction drill-down for the full treatment.

Full treatment: [dae_construction.md](phase6_dae_construction/dae_construction.md);
drill-down: [index_reduction.md](phase6_dae_construction/index_reduction.md).

---

## Structural Analysis (Phase 7)

Before simulation, Rumoca analyses the DAE algebraically to determine a
**solution order**. The phase 7 directory has a parent
[structural_analysis.md](phase7_structural_analysis/structural_analysis.md) and
six topical drill-downs:

1. **Incidence matrix** ([drill-down](phase7_structural_analysis/incidence_matrix.md)) — which unknowns appear in which equations
2. **Maximum bipartite matching** ([drill-down](phase7_structural_analysis/maximum_bipartite_matching.md)) — Kuhn's augmenting-path algorithm; assigns one unknown to each equation
3. **Tarjan SCC** ([drill-down](phase7_structural_analysis/tarjan_scc.md)) — finds algebraic loops in the dependency graph
4. **BLT reordering** ([drill-down](phase7_structural_analysis/blt.md)) — packages SCCs as Block-Lower-Triangular evaluation blocks
5. **Tearing** ([drill-down](phase7_structural_analysis/tearing.md)) — reduces algebraic loops from $N \times N$ to $K \times K$ Newton iterations via Cellier's greedy heuristic
6. **IC planning** ([drill-down](phase7_structural_analysis/ic_plan.md)) — sequences how to compute consistent initial values at $t = 0$

---

## Solve Lowering (Phase 8)

Solve lowering produces an execution-ready tensor IR (`SolveProblem`) from
the DAE. It runs `dae_prepare` cleanups, performs forward-mode AD,
eliminates dummy derivatives, lowers residual / derivative / discrete /
event / clock partitions to `ComputeBlock`s of typed nodes (`MatMul`,
`LinSolve`, `Map`, `AffineStencil`, `ScalarPrograms`), and produces a
schema-versioned serialisable artifact ready for backends to consume
directly.

The output is consumed by both the simulator (`simulate_solve_model`)
and the solve-IR-based codegen templates (`c-solve`, `casadi-solve`,
`cuda-c`, `wgsl-solve`, `mlir`, `rust-solve`, `jax-solve`,
`cranelift-solve-jit`, `cuda-nvrtc-solve-jit`).

Full treatment: [solve_lowering.md](phase8_solve_lowering/solve_lowering.md).

---

## Simulation (Phase 9)

The simulation stack is split across several crates as of v0.9.x:

- `rumoca-sim` — facade and orchestration
- `rumoca-solver` — backend-neutral primitives (`SimResult`, `SimOptions`,
  `DiffsolMethod`, `PreparedMassMatrix`)
- `rumoca-solver-diffsol` — BDF / ESDIRK34 / TR-BDF2 backend
- `rumoca-solver-rk45` — explicit RK45 backend
- `rumoca-eval-dae`, `rumoca-eval-solve` — residual / Jacobian / root
  evaluation for the two paths

Key capabilities:

- Exact Jacobians via forward-mode AD
- Mass-matrix support for index-1 DAEs (`PreparedMassMatrix` — Identity,
  Diagonal, Dense)
- Event detection via zero-crossing functions
- `when`-clause handling (discrete update at events)
- `reinit()` support
- Systematic initialization (IC solver consuming the BLT-sequenced IC
  plan from phase 7)
- **Interactive stepping** via `SimulationSession` (renamed from the
  earlier `SimStepper`)
- **Zero-state simulation** for models with no continuous states
  (discrete-only event handling with root-finding bisection)
- **Scheduled simulation** via `rumoca-sim/src/scheduled_sim/`
- **NaN-tracing** runtime via `rumoca-eval-solve::nan_trace` for
  diagnosing non-finite intermediate values (auto-retry on failure)

Full treatment: [simulation.md](phase9_simulation/simulation.md).

---

## Code Generation (Phase 10)

Code generation is **template-driven** (minijinja). Each backend is a
directory under `rumoca-phase-codegen/src/templates/` with a
`target.toml` manifest plus one or more `.jinja` files. Two parallel
families coexist:

**DAE-path targets** consume the symbolic DAE expression trees:

| Target | Description |
|--------|-------------|
| `casadi-sx`, `casadi-mx` | Python/CasADi scalar / matrix symbolics |
| `julia-mtk` | Julia/ModelingToolkit.jl |
| `jax` | Python/JAX + Diffrax |
| `sympy` | Python/SymPy |
| `symforce` | SymForce factor-graph backend |
| `onnx` | ONNX computation graph |
| `embedded-c` | Bare-metal C (discrete-only) |
| `embedded-c-galec` | Embedded C via GALEC intermediate |
| `fmi2`, `fmi3` | Complete FMI packages (C + XML + CMake + driver) |
| `galec`, `galec-production` | GALEC intermediate representation (dev / production) |
| `dae-modelica`, `flat-modelica`, `base-modelica`, `modelica` | Modelica round-trips |

**Solve-path targets** consume the `SolveProblem` tensor IR (new in v0.9.x):

| Target | Description |
|--------|-------------|
| `c-solve` | Portable C with hand-rolled residual/Jacobian |
| `casadi-solve` | CasADi structures from the compute graph |
| `jax-solve` | JAX with explicit residual/Jacobian functions |
| `rust-solve` | Standalone Rust harness |
| `cuda-c` | CUDA C kernels (AOT) |
| `cuda-nvrtc-solve-jit` | CUDA NVRTC JIT (via `rumoca-exec-mlir`) |
| `cranelift-solve-jit` | Cranelift native-code JIT (via `rumoca-exec-cranelift`) |
| `mlir` | MLIR dialect output for further compilation |
| `rust-fixed-solve` | Standalone Rust harness (fixed-step) |
| `wgsl-solve` | WebGPU compute shaders |

Full treatment: [codegen_templates.md](phase10_codegen_templates/codegen_templates.md).

---

## Codebase Layout

```
rumoca/
├── crates/
│   ├── rumoca/                  # CLI entry point
│   ├── rumoca-compile/          # Top-level compilation session (replaces rumoca-session)
│   ├── rumoca-core/             # Shared primitives (Token, Expression, Span, …)
│   ├── rumoca-phase-parse/      # Phase 1
│   ├── rumoca-phase-resolve/    # Phase 2
│   ├── rumoca-phase-instantiate/# Phase 3 (runs before typecheck)
│   ├── rumoca-phase-typecheck/  # Phase 4 (runs after instantiate)
│   ├── rumoca-phase-flatten/    # Phase 5
│   ├── rumoca-phase-dae/        # Phase 6
│   ├── rumoca-phase-structural/ # Phase 7 (matching, BLT, tearing, IC, dae_prepare)
│   ├── rumoca-phase-solve/      # Phase 8 (DAE → SolveProblem lowering)
│   ├── rumoca-phase-codegen/    # Phase 10 (templates)
│   ├── rumoca-ir-{ast,flat,dae,solve,galec}/ # IR crates
│   ├── rumoca-eval-{ast,flat,dae,solve}/     # Expression evaluators
│   ├── rumoca-sim/              # Simulator facade
│   ├── rumoca-solver/           # Backend-neutral solver primitives
│   ├── rumoca-solver-diffsol/   # BDF / ESDIRK34 / TR-BDF2 backend
│   ├── rumoca-solver-rk45/      # Explicit RK45 backend
│   ├── rumoca-exec-cranelift/   # Cranelift JIT execution backend
│   ├── rumoca-exec-mlir/        # MLIR + CUDA execution backend
│   ├── rumoca-exec-wasm/        # WebAssembly execution backend
│   ├── rumoca-codec/            # Codec abstractions
│   ├── rumoca-codec-flatbuffers/# FlatBuffer codec
│   ├── rumoca-signal-frame/     # Shared signal-frame structures
│   ├── rumoca-input/            # Input-source abstractions
│   ├── rumoca-input-{gamepad,keyboard}/  # Input drivers
│   ├── rumoca-transport-{udp,websocket,zenoh}/ # Transport layers
│   ├── rumoca-worker/           # Background simulation worker
│   ├── rumoca-bind-python/      # Python bindings (abi3 wheels)
│   ├── rumoca-bind-wasm/        # WebAssembly bindings
│   ├── rumoca-bind-wasm-diffsol/# Lazy WASM diffsol addon
│   ├── rumoca-bind-wasm-galec/  # WASM GALEC addon
│   ├── rumoca-ir-galec/         # GALEC/eFMI algorithm code IR
│   ├── rumoca-galec-codegen/    # GALEC code generation
│   ├── rumoca-opt/              # Optimization passes
│   ├── rumoca-tool-lsp/         # Language server
│   ├── rumoca-tool-galec-lsp/   # GALEC language server
│   ├── rumoca-tool-fmt/         # Formatter
│   ├── rumoca-tool-lint/        # Linter
│   ├── rumoca-tool-docs/        # Documentation generator
│   ├── rumoca-lsp-position/     # LSP position utilities
│   ├── rumoca-contracts/        # MLS compliance test framework
│   └── rumoca-test-msl/         # MSL parity test infrastructure
├── spec/                        # Formal specifications (SPEC_NNNN_*.md)
├── docs/                        # Documentation
│   └── compiler-phases/         # ← These documents live here
└── examples/                    # Example Modelica models
```

---

## Document Map

Each phase has one parent document; phases 6 and 7 also have topical
drill-downs that go deeper into algorithms and design decisions. Indented
entries are drill-downs; their parent document summarises the topic and links
to them.

- [`phase1_parsing_and_ast/parsing_and_ast.md`](phase1_parsing_and_ast/parsing_and_ast.md) — How Modelica is parsed; the parol LL(k) grammar; AST node types
- [`phase2_resolve_and_scope/resolve_and_scope.md`](phase2_resolve_and_scope/resolve_and_scope.md) — Name resolution, scope trees, DefIds, import/extends handling
- [`phase4_instantiate/instantiate.md`](phase4_instantiate/instantiate.md) — Modification application, redeclare, inner/outer, instance tree (pipeline phase 3)
- [`phase3_typecheck_and_dims/typecheck_and_dims.md`](phase3_typecheck_and_dims/typecheck_and_dims.md) — Type inference, MLS §10.1 array dimensions, variability rules (pipeline phase 4)
- [`phase5_flatten/flatten.md`](phase5_flatten/flatten.md) — Connection expansion, qualified naming, residual-form equations, flat IR
- [`phase6_dae_construction/dae_construction.md`](phase6_dae_construction/dae_construction.md) — Variable classification, equation partitioning, when-clause lowering, zero-crossings
    - [`index_reduction.md`](phase6_dae_construction/index_reduction.md) — Differential index, symbolic chain-rule differentiator, state demotion sweeps, prep pipeline
- [`phase7_structural_analysis/structural_analysis.md`](phase7_structural_analysis/structural_analysis.md) — Pipeline summary; matching, BLT, tearing, IC plan as drill-down pointers
    - [`incidence_matrix.md`](phase7_structural_analysis/incidence_matrix.md) — Sparse incidence storage, three-map array-aware resolver, the `der()` argument subtlety, dependency graph
    - [`maximum_bipartite_matching.md`](phase7_structural_analysis/maximum_bipartite_matching.md) — Kuhn's augmenting-path algorithm; line-by-line annotation of `augment()`; determinism via sorting
    - [`tarjan_scc.md`](phase7_structural_analysis/tarjan_scc.md) — Index/lowlink/stack mechanics; reverse-topological emission as BLT order
    - [`blt.md`](phase7_structural_analysis/blt.md) — Matrix view of BLT; `Scalar` vs `AlgebraicLoop` blocks; SCC-to-block conversion
    - [`tearing.md`](phase7_structural_analysis/tearing.md) — Cellier's greedy heuristic; causal/tear-pick alternation; deterministic tie-breaking
    - [`ic_plan.md`](phase7_structural_analysis/ic_plan.md) — Algebraic-only subsystem; `IcBlock` variants; `improve_causal_assignment` post-pass; relaxed-IC fallback
- [`phase8_solve_lowering/solve_lowering.md`](phase8_solve_lowering/solve_lowering.md) — `SolveProblem` tensor IR, `ComputeBlock`/`ComputeNode`, lowering pipeline, dummy-derivative elimination, forward-mode AD, downstream consumers
- [`phase9_simulation/simulation.md`](phase9_simulation/simulation.md) — Two entry points (`simulate_with_diagnostics` / `simulate_solve_model`), BDF / ESDIRK34 / TR-BDF2 / RK45, `PreparedSimulation`, IC solving, event handling, `SimulationSession`, NaN-tracing, zero-state simulation
- [`phase10_codegen_templates/codegen_templates.md`](phase10_codegen_templates/codegen_templates.md) — minijinja template engine, DAE-path and Solve-path template families, per-target `target.toml` manifests
