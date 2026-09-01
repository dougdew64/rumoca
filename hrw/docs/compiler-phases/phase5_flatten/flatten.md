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
  InstancedTree  (from instantiation + typecheck)
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

*Verified 2026-07-30 against `crates/rumoca-phase-flatten/src/connections/mod.rs`* — read while instrumenting it. Note that `validate_type_compatibility` exists there and does **not** fire for connectors with differing member sets: see `docs/upstream-issues.md` #2.

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

## Structured Equation Families (SPEC_0019 — Array Preservation)

Rumoca preserves for-loop structure rather than always scalarizing. The flat
IR records a `StructuredEquationFamily` for each source for-equation (see
`crates/rumoca-ir-flat/src/lib.rs`):

```rust
pub struct StructuredEquationFamily {
    pub domain: StructuredIndexDomain,       // compact index domain
    pub first_equation_index: usize,         // index into flat equations vec
    pub equation_counts: Vec<usize>,         // scalar count per domain point
    pub span: Span,
    pub origin: EquationOrigin,
    pub regular: Option<RegularForFamily>,    // affine stencil metadata
    pub template: Option<ComprehensionTemplate>, // canonical comprehension body
    pub interiors_materialized: bool,         // whether all cells carry full bodies
}
```

The actual equations are still stored in the main equations vector, but the
`StructuredEquationFamily` metadata lets downstream consumers (code generators,
simulators) reconstruct the loop structure. When a family is regular (all
array accesses are affine in the binders), the `regular` field carries stride
metadata that the Solve-IR lowering uses to build compact kernels without
materializing one row per index tuple.

---

## Algorithm Sections (SPEC_0020)

Algorithm sections are **preserved as structured objects**, not converted to
individual assignment equations:

```rust
pub struct Algorithm {
    pub statements: Vec<Statement>,  // the algorithm body
    pub outputs: Vec<Reference>,     // LHS variables (for balance counting)
    pub span: Span,
    pub origin: String,
}
```

`outputs` is computed automatically from the assignment statements via
`extract_algorithm_outputs()`. Each output variable counts as one equation for
the degree-of-freedom balance check (MLS §4.4.2).

---

## The Flat Model IR (`rumoca-ir-flat`)

### Model struct (`crates/rumoca-ir-flat/src/lib.rs`)

```rust
pub struct Model {
    pub variables: VarNameIndexMap<Variable>,
    pub variable_type_names: VarNameIndexMap<String>,   // resolved type name per variable
    pub variable_final_flags: VarNameIndexMap<bool>,     // MLS 7.2.6 final qualifier
    pub equations: Vec<Equation>,                        // all in residual form (0 = expr)
    pub structured_equations: Vec<StructuredEquationFamily>, // source loop metadata
    pub assert_equations: Vec<AssertEquation>,
    pub initial_equations: Vec<Equation>,
    pub initial_structured_equations: Vec<StructuredEquationFamily>,
    pub initial_assert_equations: Vec<AssertEquation>,
    pub algorithms: Vec<Algorithm>,
    pub initial_algorithms: Vec<Algorithm>,
    pub when_clauses: Vec<WhenClause>,
    pub functions: VarNameIndexMap<Function>,
    pub is_partial: bool,
    pub class_type: ClassType,
    pub model_description: Option<String>,
    pub top_level_connectors: IndexSet<String>,
    pub top_level_input_components: IndexSet<String>,
    pub enum_literal_ordinals: IndexMap<String, i64>,
    pub symbol_ancestry: SymbolAncestryMap,
    // connection graph metadata (for overconstrained connectors)
    pub definite_roots: IndexSet<String>,
    pub branches: Vec<(String, String)>,
    pub optional_edges: Vec<(String, String)>,
    pub potential_roots: Vec<(String, i64)>,
    pub oc_break_edge_scalar_count: usize,
}
```

`VarNameIndexMap<V>` is a type alias for `IndexMap<VarName, V, FxBuildHasher>`,
using a fast deterministic hasher. `IndexSet` (from the `indexmap` crate)
replaces the earlier `HashSet` usage to guarantee deterministic iteration order
across compilations.

### Variable struct (`crates/rumoca-ir-flat/src/lib.rs`)

```rust
pub struct Variable {
    pub name: VarName,                  // globally qualified: "body.position.x"
    pub component_ref: Option<ComponentReference>, // structured source reference
    pub source_span: Span,
    pub type_id: TypeId,
    pub variability: Variability,       // constant | parameter | discrete | continuous
    pub causality: Causality,           // input | output | (none)
    pub flow: bool,
    pub stream: bool,
    pub dims: Vec<i64>,                 // resolved array dimensions
    pub connected: bool,                // appears in a connect() statement
    pub binding: Option<Expression>,
    pub binding_from_modification: bool,
    pub evaluate: bool,                 // structural parameter (Evaluate=true or final)
    pub is_discrete_type: bool,         // Boolean/Integer -> discrete by default
    pub is_primitive: bool,             // primitive vs record type
    pub from_expandable_connector: bool,
    pub is_overconstrained: bool,       // belongs to an overconstrained connector
    pub is_protected: bool,             // declared in protected section
    pub oc_record_path: Option<String>, // enclosing overconstrained record path
    pub oc_eq_constraint_size: Option<usize>,
    // plus: start, fixed, min, max, nominal, unit, display_unit,
    //       quantity, description, state_select
}
```

### Equation struct (`crates/rumoca-ir-flat/src/lib.rs`)

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

### Flat Expression IR (`crates/rumoca-core/src/ir_primitives.rs`)

The `Expression` type is defined in `rumoca-core` and shared by the Flat and
DAE IRs. Compared to the AST `Expression`:
- Uses `Reference` (a structured semantic reference with cached display name,
  optional `ComponentReference`, and optional resolved function info) instead
  of raw `ComponentReference`
- Distinguishes `BuiltinCall` (e.g., `der`, `sin`) from `FunctionCall`
  (user-defined)
- Has no `ComponentReference` variant -- name resolution is complete
- Has no parentheses -- tree structure encodes precedence
- Every variant carries an optional `span` field for diagnostics

```rust
pub enum Expression {
    Binary { op: OpBinary, lhs: Box<Expression>, rhs: Box<Expression>, span: Span },
    Unary { op: OpUnary, rhs: Box<Expression>, span: Span },
    VarRef { name: Reference, subscripts: Vec<Subscript>, span: Span },
    BuiltinCall { function: BuiltinFunction, args: Vec<Expression>, span: Span },
    FunctionCall { name: Reference, args: Vec<Expression>, is_constructor: bool, span: Span },
    Literal { value: Literal, span: Span },
    If { branches: Vec<(Expression, Expression)>, else_branch: Box<Expression>, span: Span },
    Array { elements: Vec<Expression>, is_matrix: bool, span: Span },
    Tuple { elements: Vec<Expression>, span: Span },
    Range { start, step, end, span: Span },
    ArrayComprehension { expr, indices, filter, span: Span },
    Index { base: Box<Expression>, subscripts: Vec<Subscript>, span: Span },
    FieldAccess { base: Box<Expression>, field: String, span: Span },
    Empty { span: Span },
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

---

## Two different "connection graphs", and only one is confined to this phase

*Verified 2026-08-12 against `crates/rumoca-phase-flatten/src/connections/mod.rs` and
`crates/rumoca-phase-dae/src/overconstrained_interface.rs`* — read while answering Doug's question
*"is the connection graph something which exists only in the Flatten stage?"* The private types, the
two union-finds and the DAE phase's separate spanning-tree check were all read directly; §3 below is
explicitly marked as not verified.

**Held here rather than in `fixture-labs/connect-expansion.md` — Doug's call:** *"past the level of
useful detail for this lab."* It is the right answer for a later, deeper pass.

**1. The `connect`-expansion graph — confined to Flatten, and destroyed with it.**

Vertices are connector variables, edges are the `connect` statements, undirected; the question is
connected components. Built by a **private** `UnionFind` (path compression, union-by-rank) over
`VarName` indices, with a **private** `ConnectionSet { variables, kind, scope, span }` — both
non-`pub` in `crates/rumoca-phase-flatten/src/connections/mod.rs`. Discarded when the phase returns.
`ConnectionKind` is `{Flow, Potential, Stream}`, and there are two union-finds, `potential_uf` and
`stream_uf`.

**CORRECTION 2026-08-13 — the "two union-finds" description above was wrong, and so was the
inference beneath it.** *Verified against
`crates/rumoca-phase-flatten/src/connections/mod.rs`* (`connect_primitive_vars`, and the set
extraction near the end of `build_connection_sets`). Doug's question — *"the connections are
actually between the connector variables rather than between the connectors, correct?"* — made it
load-bearing, so it was traced rather than left as an inference.

**A `connect` is expanded into pairs of matching MEMBER VARIABLES first**
(`expand_connector_connection`), and each pair is then routed by kind into **one of three**
mechanisms:

| both members are | goes to | grouped |
|---|---|---|
| `flow` | `flow_pairs`, a plain `Vec<(VarName, VarName)>` | later, by its **own** union-find, **per scope** |
| `stream` | `stream_uf` | globally |
| neither | `potential_uf` | globally |

**So flow is NOT a union-find at collection time, and flow sets do NOT share membership with
potential sets** — which is exactly what the earlier note inferred and marked as unverified. Flow
pairs are accumulated as a list and grouped separately afterwards, and a flow set is emitted only
when `vars.len() >= 2`. Potential sets are extracted globally, with the source noting that merging
or splitting them does not change the equation count — *N−1 for N variables either way*.

**The provenance convention did its job here.** The claim was tagged *"Inferred, not traced"*, so
when it mattered it was checked, and it was false. An untagged version of the same sentence would
have been read as fact.

**What survives the graph's destruction:**

- **Potential equations** — *n − 1* per component, each one edge of a **spanning tree** of that
  component.
- **Flow sum equations** — one per component, naming **every member**
  (`0 = C.n.i + src.n.i + gnd.p.i`). These are a verbatim record of the partition, so the components
  are recoverable from the flat model even though the graph is not stored.
- **Spans** — each generated equation carries the originating `connect()` span (SPEC_0008), enforced
  by `crates/rumoca/tests/architecture_hardening/.../source_named_spans.rs`.

**Consequence for HRW:** `worker::record_connection_frames` must **re-run** instantiate + typecheck
+ flatten with an observer to show this graph, because the frames exist only while the pass runs.
A labelled derived view, permitted precisely because it is labelled.

**2. The overconstrained-connector graph (MLS §9.4) — outlives Flatten.**

A different question: *is the user's declared spanning tree legal?* Driven by
`Connections.branch/root/potentialRoot`. The flat model **retains** `branches` and `definite_roots`,
and `crates/rumoca-phase-dae/src/overconstrained_interface.rs` runs its **own** union-find over
`flat.branches` to reject a required edge that closes a cycle (CONN-014) and two definite roots in
one tree (CONN-015), via `ToDaeError::InvalidConnectionGraph`.

So "does the connection graph survive Flatten?" has opposite answers for the two graphs, and the
curated specimens exercise only the first — `RcCircuit` has no `Connections.branch` at all.

**3. Unverified, and a candidate for `upstream-issues.md` rather than a claim.**

`crates/rumoca-ir-flat/src/connections.rs` declares and exports a fuller family —
`ConnectionSet`, `ConnectionSets`, `ConnectionGraph`, `GraphNode`, `GraphEdge`, `SpanningTree`,
`SpanningTreeEdge` — and **grep found nothing in the workspace that constructs any of them**; the
flatten phase uses its own private type of the same name. That *suggests* declared-but-unpopulated
IR, but absence of a constructor by grep is not proof (a `Default`, a struct-update or a
deserialization would not match the patterns searched). **Do not report this upstream without
checking properly.**

### The five connection validations, and the one thing none of them checks

*Verified 2026-08-12 against `crates/rumoca-phase-flatten/src/connections/mod.rs`* — read while
answering Doug's question about connector type checking. The lab carries the conceptual version;
this is the code-level list.

`validate_connections` (line 499) applies these to **each pair of variables being joined**:

| fn | MLS rule | rejects |
|---|---|---|
| `validate_flow_consistency` | CONN-001 homogeneity, CONN-003 flow-to-flow | one `flow`, one not |
| `validate_quantity_compatibility` | CONN-005 | two non-empty `quantity` attributes that differ |
| `validate_type_compatibility` | CONN-002 | different primitive types, **only when both are known** |
| `validate_dimension_compatibility` | CONN-008 | scalar-to-array, or mismatched array dims |
| `validate_expanded_connector_connection` | — | (expandable connectors) |

**All five are pairwise on members that got paired.** `validate_type_compatibility` additionally
canonicalises through a *second* union-find (`type_roots` / `canonical_type_id`) so that two names
for one type compare equal.

**The gap is ALREADY KNOWN — `docs/upstream-issues.md` #2**, found 2026-07-29 and adjudicated by
System Modeler, and the "Connection Expansion" section above has recorded it since 2026-07-30.
Nothing verifies that the two connectors have the **same member set**: `IncompatibleConnect` joins
`PinA {v, flow i}` to `PinB {v}`, `a.v` and `b.v` pair and pass every check, `a.i` is never paired,
and flatten **succeeds**. The wiring error then surfaces at Structural as a *singularity*.

So the honest summary: **Rumoca checks that paired variables are compatible; it does not check that
the pairing is complete.**

*Added here because the enumeration of the five validations was not written down; the gap itself was.
**Claude re-derived it on 2026-08-12 without checking this file first** — which is what appending to
a teaching-database page without reading it produces, and the reason the duplication above was
trimmed to the part that is new.*
