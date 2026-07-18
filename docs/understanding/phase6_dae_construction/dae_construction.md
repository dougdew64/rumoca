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

### Step 1: Detect State Variables (`classification.rs:22–44`)

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

### Step 2: Classify Each Variable (`classification.rs:69–115`)

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

After classification each variable is placed in the corresponding `IndexMap` in
the `Dae` struct: `states`, `algebraics`, `inputs`, `outputs`, `parameters`,
`constants`, `discrete_reals` (discrete Real), or `discrete_valued`
(Boolean/Integer/enum).

---

## Equation Group Assignment

### Conversion Pipeline (`lib.rs:145–275`)

1. **Binding conversion** (line 210–220): Variable binding expressions are
   turned into explicit equations `0 = binding - var` so they participate in
   structural analysis.

2. **Equation classification** (line 226–228): Each equation from `flat.equations`
   is routed to `f_x`, `f_z`, or `f_m` based on the kind of variable(s) it
   assigns.

3. **When-clause conversion** (line 241–247): `when` clauses are expanded (see
   below) and their equations inserted into `f_z` / `f_m`.

4. **Algorithm lowering** (line 250–259): Algorithm sections are converted to
   explicit equation lists (see below) and classified into the appropriate group.

5. **Condition extraction** (line 260–262): Relational expressions used as
   guards are extracted into `f_c` / `relation`.

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
(duplicate assignment detection, line 60–78 of `when_conversion.rs`).

Array when-targets: `when_target_scalar_count()` (line 16–25) infers the
scalar count for array assignments, consistent with MLS §8.4.

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
5. Store in `Dae.relation` (the expression list)
6. Build corresponding `f_c` equations: `c[i] := (relation[i] > 0)`

---

## Initial Equations (`initial.rs`)

Initial equations specify the consistent initial state at `t = 0`. They are
separate from the continuous equations and only active during initialization.

Equation scalar counts are re-derived to handle:
- Explicit count of 0 → skip
- Inferred count of 0 without array marker → skip
- Otherwise: `max(explicit_count, inferred_count, 1)`

All surviving initial equations go into `Dae.initial_equations`.

---

## The Dae Struct (`rumoca-ir-dae/src/lib.rs:118–266`)

```rust
pub struct Dae {
    // Variable partitions
    pub states: IndexMap<VarName, Variable>,
    pub algebraics: IndexMap<VarName, Variable>,
    pub inputs: IndexMap<VarName, Variable>,
    pub outputs: IndexMap<VarName, Variable>,
    pub parameters: IndexMap<VarName, Variable>,
    pub constants: IndexMap<VarName, Variable>,
    pub discrete_reals: IndexMap<VarName, Variable>,
    pub discrete_valued: IndexMap<VarName, Variable>,
    pub derivative_aliases: IndexMap<VarName, Variable>, // from explicit ODE eqs

    // Equation groups (MLS B.1)
    pub f_x: Vec<Equation>,
    pub f_z: Vec<Equation>,
    pub f_m: Vec<Equation>,
    pub f_c: Vec<Equation>,
    pub relation: Vec<Expression>,              // zero-crossing expressions
    pub synthetic_root_conditions: Vec<Expression>,
    pub initial_equations: Vec<Equation>,

    // Event/clock metadata
    pub scheduled_time_events: Vec<f64>,
    pub clock_constructor_exprs: Vec<Expression>,
    pub clock_schedules: Vec<ClockSchedule>,
    pub triggered_clock_conditions: Vec<Expression>,
    pub clock_intervals: IndexMap<String, f64>,

    // Metadata
    pub is_partial: bool,
    pub class_type: ClassType,
    pub functions: IndexMap<VarName, Function>,
    pub enum_literal_ordinals: IndexMap<String, i64>,
    pub interface_flow_count: usize,
    pub overconstrained_interface_count: i64,
    pub oc_break_edge_scalar_count: usize,
}
```

### Variable struct (`lib.rs:310–362`)

```rust
pub struct Variable {
    pub name: VarName,
    pub dims: Vec<i64>,
    pub start: Option<Expression>,
    pub fixed: Option<Expression>,
    pub min: Option<Expression>,
    pub max: Option<Expression>,
    pub nominal: Option<Expression>,
    pub unit: Option<String>,
    pub state_select: StateSelect,
    pub is_tunable: bool,     // FMI 3.0: tunable at runtime
    // ...
}
```

### Equation struct (`lib.rs:364–380`)

```rust
pub struct Equation {
    pub lhs: Option<VarName>,   // Some for explicit form (z := rhs), None for residual
    pub rhs: Expression,        // either the residual or the RHS
    pub span: Span,
    pub origin: String,
    pub scalar_count: usize,    // MLS §8.4 scalar equation count
}
```

The `lhs` field distinguishes:
- **Residual form**: `lhs = None`, equation is `0 = rhs` — used in `f_x`
- **Assignment form**: `lhs = Some(name)`, equation is `name := rhs` — used in `f_z`, `f_m`

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
  these variables move from `dae.states` to `dae.algebraics`.
- **Origin tagging.** Differentiated equations carry a marker like
  `"index_reduction:d_dt_for_x"` in their `origin` field so downstream
  diagnostics can identify them.

Because the pass rewrites the Appendix B form itself (rather than producing
an analysis artifact alongside it, as phase 7 does), it is conceptually a
continuation of DAE construction. As of v0.9.x, the implementation lives in
`crates/rumoca-phase-structural/src/dae_prepare/` (it was historically in
`rumoca-sim`; the v0.9.x refactor moved it into the phase-crate layer
where it conceptually belongs). The orchestrating driver — the function
that runs the prep stages in order — lives in `rumoca-sim` for the
simulation path and in `rumoca-phase-dae::prepare_dae_for_codegen` for the
codegen path; both call the same `rumoca-phase-structural::dae_prepare`
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

| File | Purpose |
|------|---------|
| `rumoca-phase-dae/src/lib.rs` | Entry point `to_dae()`; conversion pipeline |
| `rumoca-phase-dae/src/classification.rs` | Variable classification algorithm |
| `rumoca-phase-dae/src/when_conversion.rs` | When-clause expansion |
| `rumoca-phase-dae/src/algorithm_lowering.rs` | Algorithm → equation conversion; discrete routing |
| `rumoca-phase-dae/src/condition_lowering.rs` | Zero-crossing extraction |
| `rumoca-phase-dae/src/initial.rs` | Initial equation handling |
| `rumoca-ir-dae/src/lib.rs` | `Dae`, `Variable`, `Equation` type definitions |
| `spec/SPEC_0003_HYBRID_DAE.md` | Formal specification of the hybrid DAE form |
| `spec/SPEC_0007_LEAN_DAE.md` | Rationale for the DAE IR schema design |
