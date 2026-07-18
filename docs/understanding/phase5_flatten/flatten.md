# Phase 5: Flattening

## Overview

Flattening is the second half of the bridge from Modelica's hierarchical
class-based world to the flat equation-based world. The first half,
[instantiation](../phase4_instantiate/instantiate.md), produced an
`InstancedTree` with all modifications resolved and the class hierarchy intact.
Flattening walks that tree and emits a `flat::Model` — a globally-qualified
flat structure with no remaining class hierarchy.

- Implementation: `crates/rumoca-phase-flatten/`
- Entry point: `pub fn flatten(instanced: ast::InstancedTree) -> Result<flat::Model, FlattenError>`
- Output IR: `crates/rumoca-ir-flat/`

---

## Big Picture: Input and Output

```
  InstancedTree  (from phase 4)
        │
        ▼
  ┌─────────────────────────────────────┐
  │        Phase 5: Flatten             │
  │                                     │
  │  • Walk tree, build qualified       │
  │    names ("body.position.x")        │
  │  • Expand connect() into equality   │
  │    and flow-sum equations           │
  │    (MLS §9.2)                       │
  │  • Convert equations to residual    │
  │    form: 0 = expr                   │
  │  • Preserve for-loops/algorithms    │
  │    as structured metadata           │
  └─────────────────────────────────────┘
        │
        ▼
  flat::Model  (qualified names, residual eqs)
```

---

## What Flattening Does

The output `flat::Model` has:

- No class hierarchy
- Globally qualified variable names (e.g., `"body.position.x"`)
- All connections expanded into equations
- All for-loops expanded (or preserved with metadata)
- All equations in **residual form**: `0 = residual`

---

## Qualified Name Generation

As the flattener descends into nested components it builds a path:

- At root: `""` (empty)
- Enter component `body`: `"body"`
- Enter `body.position`: `"body.position"`
- Reach `body.position.x`: qualified name is `"body.position.x"`

Array subscripts are appended: `"sensors[1].v"`.

---

## Connection Expansion (MLS §9.2)

Each `connect(c1, c2)` statement is expanded into equations during flattening.
Connections are gathered into a **connection graph** first, then expanded.

The expansion rules depend on the variable's prefix:

### Potential variables (no prefix)

For n connected variables, generate n−1 equality equations:

```
connect(a, b, c)  →  0 = a - b
                     0 = b - c
```

These ensure all connected potentials are equal.

### Flow variables (`flow` prefix)

One sum-to-zero equation across all connected flow variables:

```
connect(p1, p2, p3)  →  0 = p1 + p2 + p3
```

**Sign convention** (MLS §9.2):
- Inside a component boundary (port): sign = +1
- At the model's own boundary (top-level port): sign = −1

This correctly implements Kirchhoff's current law for electrical circuits,
conservation of mass for fluid systems, etc.

**Scalar count**: The number of scalar equations generated equals the number of
scalar elements in the variable (product of its dimensions).

### Subrange connections

When arrays of different sizes are connected, the equation covers only the
smaller size: `connect(a[1:2], b[1:3])` uses size 2.

---

## For-Loop Equations (SPEC_0019 — Array Preservation)

Rumoca preserves for-loop structure rather than always scalarizing:

```rust
pub struct ForEquation {
    pub index_names: Vec<String>,            // iteration variable names
    pub first_equation_index: usize,         // index into flat equations vec
    pub iterations: Vec<ForEquationIteration>, // one entry per iteration
    pub span: Span,
    pub origin: EquationOrigin,
}
```

The actual equations are stored in the main equations vector, but the
`ForEquation` metadata lets downstream consumers (code generators, simulators)
reconstruct the loop structure if needed.

---

## Algorithm Sections (SPEC_0020)

Algorithm sections are **preserved as structured objects**, not converted to
individual assignment equations:

```rust
pub struct Algorithm {
    pub statements: Vec<Statement>,  // the algorithm body
    pub outputs: Vec<VarName>,       // LHS variables (for balance counting)
    pub span: Span,
    pub origin: String,
}
```

`outputs` is computed automatically from the assignment statements via
`extract_algorithm_outputs()`. Each output variable counts as one equation for
the degree-of-freedom balance check (MLS §4.4.2).

---

## The Flat Model IR (`rumoca-ir-flat`)

### Model struct (`lib.rs:263–355`)

```rust
pub struct Model {
    pub variables: IndexMap<VarName, Variable>,
    pub equations: Vec<Equation>,               // all in residual form (0 = expr)
    pub for_equations: Vec<ForEquation>,        // loop metadata
    pub assert_equations: Vec<AssertEquation>,
    pub initial_equations: Vec<Equation>,
    pub initial_for_equations: Vec<ForEquation>,
    pub algorithms: Vec<Algorithm>,
    pub initial_algorithms: Vec<Algorithm>,
    pub when_clauses: Vec<WhenClause>,
    pub functions: IndexMap<VarName, Function>,
    pub is_partial: bool,
    pub class_type: ClassType,
    pub top_level_connectors: HashSet<String>,
    pub top_level_input_components: HashSet<String>,
    pub enum_literal_ordinals: IndexMap<String, i64>,
    // connection graph metadata (for overconstrained connectors)
    pub definite_roots: HashSet<String>,
    pub branches: Vec<(String, String)>,
    pub optional_edges: Vec<(String, String)>,
    pub potential_roots: Vec<(String, i64)>,
}
```

### Variable struct (`lib.rs:444–535`)

```rust
pub struct Variable {
    pub name: VarName,              // globally qualified: "body.position.x"
    pub type_id: TypeId,
    pub variability: Variability,   // constant | parameter | discrete | continuous
    pub causality: Causality,       // input | output | (none)
    pub flow: bool,
    pub stream: bool,
    pub dims: Vec<i64>,             // resolved array dimensions
    pub connected: bool,            // appears in a connect() statement
    pub binding: Option<Expression>,
    pub evaluate: bool,             // structural parameter?
    pub is_discrete_type: bool,     // Boolean/Integer → discrete by default
    pub is_primitive: bool,         // primitive vs record type
    // plus: start, fixed, min, max, nominal, unit, state_select, ...
}
```

### Equation struct (`lib.rs:1224–1310`)

All equations are in **residual form**: the equation `x = y + 1` becomes
`0 = y + 1 - x`.

```rust
pub struct Equation {
    pub residual: Expression,    // the expression E such that the equation is 0 = E
    pub span: Span,
    pub origin: EquationOrigin,  // describes why this equation exists
    pub scalar_count: usize,     // number of scalar equations (>1 for arrays)
}

pub enum EquationOrigin {
    ComponentEquation { component: String },
    Connection { lhs: String, rhs: String },
    FlowSum { description: String },
    UnconnectedFlow { variable: String },
    Algorithm { component: String },
    Reinit { state: String },
    WhenAssignment { target: String },
    Binding { variable: String },
}
```

### Flat Expression IR (`lib.rs:150–226`)

Compared to the AST `Expression`, the flat `Expression`:
- Uses `VarName` (globally qualified strings) instead of `ComponentReference`
- Distinguishes `BuiltinCall` (e.g., `der`, `sin`) from `FunctionCall`
  (user-defined)
- Has no `ComponentReference` variant — name resolution is complete
- Has no parentheses — tree structure encodes precedence

```rust
pub enum Expression {
    Binary { op: OpBinary, lhs: Box<Expression>, rhs: Box<Expression> },
    Unary { op: OpUnary, rhs: Box<Expression> },
    VarRef { name: VarName, subscripts: Vec<Subscript> },
    BuiltinCall { function: BuiltinFunction, args: Vec<Expression> },
    FunctionCall { name: VarName, args: Vec<Expression>, is_constructor: bool },
    Literal(Literal),
    If { branches: Vec<(Expression, Expression)>, else_branch: Box<Expression> },
    Array { elements: Vec<Expression>, is_matrix: bool },
    Tuple { elements: Vec<Expression> },
    Range { start, step, end },
    ArrayComprehension { expr, indices, filter },
    Index { base: Box<Expression>, subscripts: Vec<Subscript> },
    FieldAccess { base: Box<Expression>, field: String },
    Empty,
}
```

---

## Summary

After flattening, the compiler has a complete picture of a single model
instance as a flat system:
- Every variable has a globally unique name and resolved dimensions
- Every equation is `0 = residual`
- Connect statements are gone (replaced by equality and sum equations)
- Class hierarchy is gone
- For-loop structure is preserved as metadata but doesn't block analysis

The next phase, [DAE construction](../phase6_dae_construction/dae_construction.md),
classifies these variables and equations into the MLS Appendix B partition.
