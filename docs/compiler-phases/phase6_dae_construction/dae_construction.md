# Phase 6: DAE Construction

## Overview

The DAE construction phase converts the flat equation model into the canonical
**Differential-Algebraic Equation** form specified in MLS Appendix B. This
partition is the mathematical heart of Rumoca — every downstream consumer
(simulator, code generator) works from this representation.

- Implementation: `crates/rumoca-phase-dae/`
- IR definition: `crates/rumoca-ir-dae/src/lib.rs`
- Entry point: `pub fn to_dae(flat: &Model) -> Result<Dae, ToDaeError>`

---

## Big Picture: Input and Output

```
  flat::Model  (from phase 5)
        │
        ▼
  ┌─────────────────────────────────────┐
  │     Phase 6: DAE Construction       │
  │                                     │
  │  • Classify variables into MLS B.1  │
  │    partitions (x,y,u,w,p,c,z,m)     │
  │  • Route equations into f_x, f_z,   │
  │    f_m, f_c                         │
  │  • Expand when-clauses, lower       │
  │    algorithm sections               │
  │  • Extract zero-crossing relations  │
  │  • Index reduction + state          │
  │    demotion (DAE prep)              │
  └─────────────────────────────────────┘
        │
        ▼
  Dae  (MLS Appendix B form, index-1)
```

---

## The DAE Mathematical Form

The complete hybrid DAE system (MLS Appendix B) partitions variables and
equations into four groups:

### Variable Partitions

| Symbol | Name | Description |
|--------|------|-------------|
| `x(t)` | states | Continuous variables that appear under `der()` |
| `ẋ(t)` | derivatives | Time derivatives of states |
| `y(t)` | algebraics | Continuous variables without derivatives |
| `u(t)` | inputs | Externally provided continuous signals |
| `w(t)` | outputs | Computed continuous signals exposed to the outside |
| `p` | parameters | Fixed scalars set before simulation starts |
| `c` | constants | Evaluated at compile time; never change |
| `z(tₑ)` | discrete reals | Real-valued variables updated only at event instants |
| `m(tₑ)` | discrete-valued | Boolean/Integer/enum variables updated at events |

### Equation Groups

```
Continuous integration (between events):
  0 = f_x( x, ẋ, y, u, p, t, z, m, pre(z), pre(m) )     [B.1a — implicit DAE]

At event instants (tₑ):
  z  := f_z( x, y, u, p, t, z, m, pre(z), pre(m) )       [B.1b — discrete Real]
  m  := f_m( x, y, u, p, t, z, m, pre(z), pre(m) )       [B.1c — discrete-valued]
  c  := f_c( relation(x, y, u, p, t, z, m) )              [B.1d — conditions]
```

`pre(z)` and `pre(m)` are the values at the **previous** event instant — this
is how Modelica implements memory across discontinuities.

---

## Variable Classification

### Step 1: Detect State Variables (`analysis/classification.rs`)

A variable becomes a **state** if its name appears as the argument to `der()`.

The scanner walks all equations, initial equations, and variable bindings
(bindings are checked here because they are converted to equations *after* state
detection):

```
state_vars ← {}
for each expression in { all equations, initial equations, all bindings }:
    for each BuiltinCall(Der, [VarRef(name)]) found:
        state_vars.insert(name)
```

### Step 2: Classify Each Variable (`analysis/classification.rs`)

Rules are applied in priority order:

| Priority | Condition | Result |
|----------|-----------|--------|
| 1 | `variability == Constant` | `VariableKind::Constant` |
| 2 | `variability == Parameter` | `VariableKind::Parameter` |
| 3 | `causality == Input` | `VariableKind::Input` (even if `der(u)` exists — the input is still external) |
| 4 | `variability == Discrete` OR type is Boolean/Integer/enum | `VariableKind::Discrete` |
| 5 | `causality == Output` AND name ∈ state_vars | `VariableKind::State` |
| 6 | `causality == Output` | `VariableKind::Output` |
| 7 | name ∈ state_vars | `VariableKind::State` |
| 8 | (fallback) | `VariableKind::Algebraic` |

There is also a `VariableKind::Derivative` variant used for derivative
variables (the `der(x)` companions of states).

After classification each variable is placed in the corresponding `IndexMap`
in the `DaeVariables` sub-struct: `states`, `algebraics`, `inputs`, `outputs`,
`parameters`, `constants`, `discrete_reals` (discrete Real), or
`discrete_valued` (Boolean/Integer/enum).

---

## Equation Group Assignment

### Conversion Pipeline (`rumoca-phase-dae/src/lib.rs`)

The `to_dae_with_options` function orchestrates many subphases via
`run_todae_phase`. The major steps, in order:

1. **Flat IR validation**: Reject unresolved function calls; validate shape contract.

2. **Variable classification**: Build classification indexes (state detection,
   prefix children, connected inputs), then classify all variables.

3. **Binding conversion**: Variable binding expressions are turned into
   explicit equations `0 = binding - var` so they participate in structural
   analysis.

4. **Equation classification**: Each equation from `flat.equations` is routed
   to `f_x`, `f_z`, or `f_m` based on the kind of variable(s) it assigns.

5. **Initial equations**: Initial equations are converted and added to
   `dae.initialization.equations`.

6. **When-clause conversion**: `when` clauses are expanded (see below)
   and their equations routed into `f_z` / `f_m`.

7. **Algorithm lowering**: Algorithm sections are converted to explicit
   equation lists (see below) and classified into the appropriate group.

8. **Discrete canonicalization**: Discrete assignment equations are
   canonicalized.

9. **Enum literal lowering**: Enum literals are lowered to ordinal values.

10. **Parameter variable promotion**: Time-invariant algebraics are promoted
    to derived parameters before condition lowering.

11. **Assertion actions**: Assert/terminate equations are lowered to event
    actions.

12. **Condition extraction**: Relational expressions used as guards are
    extracted into `f_c` / `relation`.

13. **Pre-operator lowering**: `pre()` calls are lowered to dedicated
    parameter symbols.

14. **Finalization**: Parameter sorting, phantom vector scalarization,
    start-value folding, algebraic dependency sorting, and runtime
    precomputation (clock schedules, timing inference).

### Routing Rules

| Equation assigns … | Goes into |
|--------------------|-----------|
| A discrete Real variable | `f_z` |
| A Boolean/Integer/enum variable | `f_m` |
| A continuous variable | `f_x` |
| A condition (relational expression) | `f_c` |

---

## When-Clause Expansion (`when_conversion.rs`)

A `when` clause in Modelica evaluates its body equations only at the event
instants when its condition transitions from `false` to `true`.

Rumoca expands each when-clause by converting it into a conditional assignment
using `if`-expressions:

```modelica
// Input:
when time > 1.0 then
  x := pre(x) + 1;
end when;

// Expanded to f_m:
x := if time > 1.0 then pre(x) + 1 else pre(x);
```

When a variable is only assigned inside `when`-clauses, it is classified as
discrete automatically.

**Multiple when-clauses assigning the same variable** trigger an error
(duplicate assignment detection in `when_conversion.rs`).

Array when-targets: `when_target_scalar_count()` in `when_conversion.rs`
infers the scalar count for array assignments, consistent with MLS §8.4.

---

## Algorithm Section Lowering (`algorithm_lowering.rs`)

Algorithm sections contain imperative statements. For the purposes of equation
counting and structural analysis, each algorithm **output variable** counts as
one equation.

The `Algorithm.outputs` list is pre-computed from the assignment statements by
`extract_algorithm_outputs()`. During DAE construction, the whole algorithm is
kept as a unit in the DAE's `functions` map and referenced by residual equations.

**Discrete algorithm outputs** are routed through
`canonicalize_discrete_assignment_equations()` to ensure they appear in `f_m`.

---

## Zero-Crossing Functions / Relations (`condition_lowering.rs`)

Relational conditions in `if`/`when` guards that depend on continuous variables
are **zero-crossing functions** — the simulator must detect when they change
sign to locate event times precisely.

Extraction process:
1. Collect all `if`-condition candidates from equations
2. Skip `initial()`, `noEvent()`, `smooth()` wrappers (not zero-crossings)
3. For each remaining relational operator (`<`, `>`, `<=`, `>=`, `==`, `!=`):
   form a zero-crossing expression: `lhs - rhs` (changes sign at the event)
4. Deduplicate
5. Store in `dae.conditions.relations` (the expression list)
6. Build corresponding `f_c` equations in `dae.conditions.equations`: `c[i] := (relation[i] > 0)`

---

## Initial Equations (`initial.rs`)

Initial equations specify the consistent initial state at `t = 0`. They are
separate from the continuous equations and only active during initialization.

Equation scalar counts are re-derived to handle:
- Explicit count of 0 → skip
- Inferred count of 0 without array marker → skip
- Otherwise: `max(explicit_count, inferred_count, 1)`

All surviving initial equations go into `dae.initialization.equations`.

---

## The Dae Struct (`rumoca-ir-dae/src/lib.rs`)

The `Dae` struct uses nested partition sub-structs rather than flat fields.
Access patterns use paths like `dae.variables.states`, `dae.continuous.equations`,
`dae.discrete.real_updates`, etc.

```rust
pub const DAE_SCHEMA_VERSION: u16 = 7;

pub struct Dae {
    pub schema_version: u16,           // must equal DAE_SCHEMA_VERSION
    pub variables: DaeVariables,
    pub continuous: DaeContinuousPartition,
    pub initialization: DaeInitializationPartition,
    pub discrete: DaeDiscretePartition,
    pub conditions: DaeConditionPartition,
    pub events: DaeEventPartition,
    pub clocks: DaeClockPartition,
    pub symbols: DaeSymbolTable,
    pub metadata: DaeMetadata,
}
```

### Variable partition (`DaeVariables`)

```rust
pub struct DaeVariables {
    pub states: IndexMap<VarName, Variable>,          // x
    pub algebraics: IndexMap<VarName, Variable>,      // y
    pub inputs: IndexMap<VarName, Variable>,           // u
    pub outputs: IndexMap<VarName, Variable>,          // w
    pub parameters: IndexMap<VarName, Variable>,       // p
    pub constants: IndexMap<VarName, Variable>,
    pub discrete_reals: IndexMap<VarName, Variable>,   // z
    pub discrete_valued: IndexMap<VarName, Variable>,  // m
}
```

### Continuous partition (`DaeContinuousPartition`)

```rust
pub struct DaeContinuousPartition {
    pub equations: Vec<Equation>,                         // f_x (MLS B.1a)
    pub structured_equations: Vec<StructuredEquationFamily>,
}
```

### Initialization partition (`DaeInitializationPartition`)

```rust
pub struct DaeInitializationPartition {
    pub equations: Vec<Equation>,                         // initial equations
    pub structured_equations: Vec<StructuredEquationFamily>,
}
```

### Discrete partition (`DaeDiscretePartition`)

```rust
pub struct DaeDiscretePartition {
    pub real_updates: Vec<Equation>,     // f_z (MLS B.1b)
    pub valued_updates: Vec<Equation>,   // f_m (MLS B.1c)
}
```

### Condition partition (`DaeConditionPartition`)

```rust
pub struct DaeConditionPartition {
    pub equations: Vec<Equation>,        // f_c (MLS B.1d)
    pub relations: Vec<Expression>,      // zero-crossing expressions
}
```

### Event partition (`DaeEventPartition`)

```rust
pub struct DaeEventPartition {
    pub synthetic_root_conditions: Vec<Expression>,
    pub scheduled_time_events: Vec<f64>,
    pub scheduled_root_conditions: Vec<DaeScheduledRootCondition>,
    pub event_actions: Vec<DaeEventAction>,  // Assert/Terminate kinds
}
```

`DaeEventAction` carries a `condition`, a `kind` (`Assert { message }` or
`Terminate { message }`), a source `span`, and an `origin` string.

### Clock partition (`DaeClockPartition`)

```rust
pub struct DaeClockPartition {
    pub constructor_exprs: Vec<Expression>,
    pub schedules: Vec<ClockSchedule>,
    pub triggered_conditions: Vec<Expression>,
    pub intervals: IndexMap<String, f64>,
    pub timings: IndexMap<String, ClockSchedule>,  // phase-bearing companion to intervals
}
```

### Symbol table (`DaeSymbolTable`)

```rust
pub struct DaeSymbolTable {
    pub functions: IndexMap<VarName, Function>,
    pub enum_literal_ordinals: IndexMap<String, i64>,
}
```

### Metadata (`DaeMetadata`)

```rust
pub struct DaeMetadata {
    pub is_partial: bool,
    pub class_type: ClassType,
    pub variable_starts: IndexMap<String, Expression>,
    pub discrete_input_names: Vec<String>,
    pub interface_flow_count: usize,
    pub overconstrained_interface_count: i64,
    pub oc_break_edge_scalar_count: usize,
    pub model_description: Option<String>,
    pub symbol_ancestry: SymbolAncestryMap,
}
```

### Scalar counts helper (`RuntimePartitionScalarCounts`)

```rust
pub struct RuntimePartitionScalarCounts {
    pub p: usize,  // parameters + constants
    pub t: usize,  // always 1
    pub x: usize,  // states
    pub y: usize,  // algebraics + outputs
    pub z: usize,  // discrete Reals
    pub m: usize,  // discrete-valued
}
```

### Variable struct (`rumoca-ir-dae/src/lib.rs`)

```rust
pub struct Variable {
    pub name: VarName,
    pub component_ref: Option<ComponentReference>,  // structured source reference
    pub source_span: Span,
    pub dims: Vec<i64>,
    pub start: Option<Expression>,
    pub start_span: Option<Span>,
    pub fixed: Option<bool>,         // note: bool, not Expression
    pub min: Option<Expression>,
    pub min_span: Option<Span>,
    pub max: Option<Expression>,
    pub max_span: Option<Span>,
    pub nominal: Option<Expression>,
    pub nominal_span: Option<Span>,
    pub unit: Option<String>,
    pub state_select: StateSelect,
    pub description: Option<String>,
    pub causality: VariableCausality, // Input | Output | Local | Parameter | ...
    pub is_tunable: bool,            // FMI 3.0: tunable at runtime
    pub origin: VariableOrigin,      // Source | Generated
}
```

`VariableCausality` is an enum with variants `Input`, `Output`, `Local`,
`Parameter`, `CalculatedParameter`, `Independent`. `VariableOrigin`
distinguishes compiler-generated slots (`Generated`) from source Modelica
components (`Source`, the default).

### Equation struct (`rumoca-ir-dae/src/lib.rs`)

```rust
pub struct Equation {
    pub lhs: Option<Reference>,  // Some for explicit form (z := rhs), None for residual
    pub rhs: Expression,         // either the residual or the RHS
    pub span: Span,
    pub origin: String,
    pub scalar_count: usize,     // MLS §8.4 scalar equation count
}
```

The `lhs` field (type `Option<Reference>`, not `Option<VarName>`) distinguishes:
- **Residual form**: `lhs = None`, equation is `0 = rhs` -- used in `f_x`
- **Assignment form**: `lhs = Some(ref)`, equation is `ref := rhs` -- used in `f_z`, `f_m`

The `Reference` type preserves the structured component reference that produced
the lhs, so semantic phases can use it without reparsing variable names.

### Other notable types (`rumoca-ir-dae/src/lib.rs`)

- **`WhenClause`** -- Carries a trigger `condition`, a list of `equations`
  active when triggered, per-equation `equation_inactive_rhs` values
  (`Pre` or `Current`), runtime `actions`, `span`, and `origin`.

- **`Algorithm`** -- Wraps an algorithm section's `statements`, derived
  `outputs` (from `extract_algorithm_outputs`), `span`, and `origin`.

- **`WhenEquationInactiveRhs`** -- Enum (`Pre` | `Current`) indicating what
  value a when-assigned variable holds between event instants.

- **`ClockSchedule`** -- Solver-agnostic periodic clock descriptor with
  `period_seconds`, `phase_seconds`, and `source_span`.

- **`RuntimePartitionScalarCounts`** -- Computed scalar sizes for the
  canonical runtime variable partitions (`p`, `t`, `x`, `y`, `z`, `m`).

- **`StructuredEquationFamily`** -- Tracks which scalar equations in
  `f_x` / `initial_equations` originated from a single structured source
  equation (used by `DaeContinuousPartition` and
  `DaeInitializationPartition`).

---

## Index Reduction and State Demotion

The `Dae` produced by `to_dae()` is in canonical Appendix B form, but it may
have **differential index > 1**. A high-index DAE has at least one state
whose derivative `der(x)` does not appear in any of the continuous equations
— the state is implicitly defined by an algebraic constraint that the
integrator cannot exploit directly. Standard ODE/DAE solvers (including
`diffsol`, which Rumoca uses) require an index-1 problem: every state must
have an equation of the form `... = der(x) - ...` from which the integrator
can read out a derivative value at each step.

Rumoca handles this by running a **DAE-preparation pass** after `to_dae()`
produces the raw `Dae` and before phase 7 (structural analysis) consumes it.
The pass mutates the Appendix B form directly:

- **Equation rewriting in `f_x`.** For each state with no `der(state)`
  equation, the pass finds an algebraic constraint that mentions the state
  and replaces it with its symbolic time derivative. The old equation
  `0 = h(x, y)` becomes `0 = ḣ(x, ẋ, y, ẏ)` *in place* in `f_x`.
- **State partition shifts.** A companion sweep demotes states that — even
  after differentiation — cannot be assigned a derivative-bearing row;
  these variables move from `dae.variables.states` to `dae.variables.algebraics`.
- **Origin tagging.** Differentiated equations carry a marker like
  `"index_reduction:d_dt_for_x"` in their `origin` field so downstream
  diagnostics can identify them.

Because the pass rewrites the Appendix B form itself (rather than producing
an analysis artifact alongside it, as phase 7 does), it is conceptually a
continuation of DAE construction. As of v0.9.x, the implementation lives in
`crates/rumoca-phase-structural/src/dae_prepare/` (it was historically in
`rumoca-sim`; the v0.9.x refactor moved it into the phase-crate layer
where it conceptually belongs). The orchestrating driver --
`prepare_dae_for_structural_analysis` in `rumoca-sim/src/solve_lowering/structural_lowering.rs` --
runs the prep stages in order for the simulation path. A separate
`prepare_dae_for_codegen` in `rumoca-phase-dae/src/dae_lowering.rs` serves
the codegen path. Both call the same `rumoca-phase-structural::dae_prepare`
helpers, so every downstream consumer (simulator, template codegen,
solve-IR lowering) receives an index-1 DAE built by the same code paths.

A full walk-through — what differential index means, the
`index_reduce_missing_state_derivatives_once` algorithm step by step, the
chain-rule symbolic differentiator, the three state-demotion sweeps
(orphan / no-derivative / no-assignable-row), the broader prep-pipeline
ordering, the worked-test example, and how this differs from a full
Pantelides implementation — is in the drill-down:

→ [Drill-down: Index Reduction and State Demotion](index_reduction.md)

---

## Example: Bouncing Ball

```modelica
model Ball
  Real h(start = 1.0);
  Real v(start = 0.0);
equation
  der(h) = v;
  der(v) = -9.81;
algorithm
  when h <= 0.0 then
    v := -0.8 * pre(v);
  end when;
end Ball;
```

After DAE construction:

```
states:         h, v
algebraics:     (none)
inputs:         (none)

f_x:
  0 = v - der(h)       (from der(h) = v)
  0 = -9.81 - der(v)   (from der(v) = -9.81)

f_m:
  v := if h <= 0.0 then -0.8 * pre(v) else pre(v)

relation:
  h - 0.0              (zero-crossing for h <= 0)

initial_equations:
  h(0) = 1.0
  v(0) = 0.0
```

---

## Key Files

### DAE IR definition

| File | Purpose |
|------|---------|
| `rumoca-ir-dae/src/lib.rs` | `Dae`, `Variable`, `Equation`, partition structs, `WhenClause`, `Algorithm`, `VariableKind` |
| `rumoca-ir-dae/src/types.rs` | `StructuredEquationFamily`, `StructuredEquationSlot` |
| `rumoca-ir-dae/src/expr_query.rs` | Expression query helpers (`expr_contains_var`, `expr_contains_der_of`, ...) |
| `rumoca-ir-dae/src/visitor.rs` | DAE expression/statement visitor traits and collectors |
| `rumoca-ir-dae/src/event_threshold.rs` | Event constant-threshold detection utilities |

### DAE construction (`rumoca-phase-dae`)

| File | Purpose |
|------|---------|
| `rumoca-phase-dae/src/lib.rs` | Entry point `to_dae()`; conversion pipeline orchestration |
| `rumoca-phase-dae/src/analysis/classification.rs` | Variable classification algorithm |
| `rumoca-phase-dae/src/analysis/definition_analysis.rs` | Algorithm-defined and record-equation-defined variable collection |
| `rumoca-phase-dae/src/analysis/discrete_partition.rs` | Discrete partition analysis |
| `rumoca-phase-dae/src/analysis/variable_analysis.rs` | Variable analysis helpers |
| `rumoca-phase-dae/src/binding_conversion.rs` | Variable binding-to-equation conversion |
| `rumoca-phase-dae/src/equation_conversion.rs` | Equation classification and routing |
| `rumoca-phase-dae/src/when_conversion.rs` | When-clause expansion |
| `rumoca-phase-dae/src/algorithm_lowering.rs` | Algorithm section lowering (with submodules for discrete rewrite, for-lowering, slice-lowering, etc.) |
| `rumoca-phase-dae/src/condition_lowering.rs` | Zero-crossing extraction |
| `rumoca-phase-dae/src/initial.rs` | Initial equation handling |
| `rumoca-phase-dae/src/assertion_actions.rs` | Assert/terminate equation lowering to event actions |
| `rumoca-phase-dae/src/pre_lowering.rs` | `pre()` operator lowering |
| `rumoca-phase-dae/src/dae_lowering.rs` | Codegen preparation (`prepare_dae_for_codegen`), record-param decomposition, scalarization |
| `rumoca-phase-dae/src/runtime_precompute/` | Clock schedule computation and timing inference |
| `rumoca-phase-dae/src/promote_parameter_variable.rs` | Parameter-variable algebraic promotion |
| `rumoca-phase-dae/src/fold_start_values.rs` | Start-value constant folding and algebraic dependency sorting |
| `rumoca-phase-dae/src/convert.rs` | Conversion helpers |

### DAE preparation (`rumoca-phase-structural`)

| File | Purpose |
|------|---------|
| `rumoca-phase-structural/src/dae_prepare/mod.rs` | DAE-prep helpers: alias elimination, derivative expansion, demotion sweeps |
| `rumoca-phase-structural/src/dae_prepare/state_row_reduction.rs` | Index reduction, sign normalization, derivative-row demotion |
| `rumoca-phase-structural/src/dae_prepare/symbolic.rs` | Symbolic time differentiator (chain rule) |
| `rumoca-phase-structural/src/dae_prepare/connection_alias.rs` | Connection-component fixed defining expression resolution |
| `rumoca-phase-structural/src/dae_prepare/direct_demotion.rs` | Direct-assignment state demotion |
| `rumoca-phase-structural/src/dae_prepare/dummy_state_metadata.rs` | Constrained dummy-state identification |
| `rumoca-phase-structural/src/dae_prepare/row_shape.rs` | DAE variable sizing and residual scalar-width helpers |

### Pipeline driver

| File | Purpose |
|------|---------|
| `rumoca-sim/src/solve_lowering/structural_lowering.rs` | `prepare_dae_for_structural_analysis` -- runs all prep stages in order |

### Specifications

| File | Purpose |
|------|---------|
| `spec/SPEC_0007_IR_PIPELINE.md` | Rationale for the DAE IR schema design |
