# Drill-Down: The Incidence Matrix

*Parent document: [structural_analysis.md](structural_analysis.md)*  
*Source: `crates/rumoca-phase-structural/src/incidence.rs`*

---

## Why Is an Incidence Matrix Needed?

When a Modelica model is flattened and classified into a DAE, the result is a
set of equations and a set of unknown variables — but no ordering. The equations
may have been written in any order by the modeler, and they reference the
unknowns in arbitrary combinations.

To simulate the system, the solver needs to *evaluate* these equations, which
requires knowing **which unknowns appear in which equations**. This
variable-equation dependency information is the incidence matrix.

The immediate need for it is the **maximum bipartite matching** step that
follows: matching assigns one unknown to each equation, establishing which
equation is "responsible for" solving which unknown. But the incidence matrix
is also used directly by the solver to build a sparse Jacobian structure
(which non-zero entries to allocate in the Jacobian matrix), and by the IC
plan to decompose the initialization problem.

Without this information there is no way to determine a solution order, detect
algebraic loops, or build an efficient solver.

---

## What Is the Incidence Matrix, Concretely?

The incidence matrix is a **bipartite graph** between equations and unknowns.
It is stored as a sparse structure — a list of sets, one per equation — rather
than a dense 2D array, because physical models are typically sparse (each
equation involves only a handful of the total unknowns).

```
Rows    = equations from dae.continuous.equations  (the continuous DAE equations)
Columns = unknowns  (state derivatives and algebraic/output variables)
Entry (i, j) = 1  if equation i references unknown j
               0  otherwise
```

Only the **continuous** equations (`dae.continuous.equations`) participate. The
discrete update equations are handled separately by the event settling logic
and do not need a structural analysis of their own.

### The Unknowns

The columns of the incidence matrix represent **what the solver must determine
at each time step**. These are exactly the quantities that appear differentiated
or undifferentiated in the continuous equations:

| Column kind | Variable partition | What it represents |
|-------------|-------------------|--------------------|
| `DerState(x)` | `dae.variables.states` | The time derivative `der(x)` — what the integrator advances |
| `Variable(y)` | `dae.variables.algebraics` | An algebraic variable with no derivative |
| `Variable(w)` | `dae.variables.outputs` | An output variable treated as an algebraic |

The columns are ordered in three contiguous groups (see `build_unknown_map`
in `incidence.rs`):

```
[  der(x₀), der(x₁), …  |  y₀, y₁, …  |  w₀, w₁, …  ]
 ←── states (DerState) ──  ←── algebraics ──  ←── outputs ──
```

This ordering is significant: it matches the column order of the Jacobian matrix
that the solver allocates. The solver state vector `y` is arranged in the same
order.

**Why are `der(x)` the unknowns, not `x` itself?**

Because the continuous equations are implicit in the derivative:

```
0 = f_x( x, ẋ, y, t )
```

At each time step, given current values of `x`, `y`, and `t`, the solver must
find `ẋ` (and any algebraic `y`) that satisfies `f_x = 0`. So `ẋ` is what is
truly unknown at each step. The integrator then advances `x` by integrating `ẋ`
over the time step.

---

## How the Incidence Is Built (`build_incidence`)

The entry point is `build_incidence(dae)` in `incidence.rs`:

```rust
pub(crate) fn build_incidence(dae: &dae::Dae) -> Incidence {
    let (_unknown_map, unknown_names, unknown_spans) = build_unknown_map(dae);
    let (der_resolver, variable_resolver) = build_unknown_resolvers(&unknown_names);

    let mut equation_refs = Vec::new();
    let mut equations = Vec::new();

    for (i, eq) in dae.continuous.equations.iter().enumerate() {
        equation_refs.push(EquationRef(i));
        equations.push(eq);
    }

    let eq_unknowns: Vec<HashSet<usize>> = equations
        .iter()
        .map(|eq| collect_equation_unknowns(eq, &der_resolver, &variable_resolver))
        .collect();

    Incidence { n_eq, n_var, eq_unknowns, unknown_names, unknown_spans, equation_refs }
}
```

**Step 1**: `build_unknown_map` assigns a column index to every unknown (states
first, then algebraics, then outputs) and records each unknown's source span
for diagnostics.

**Step 2**: `build_unknown_resolvers` creates two lookup structures -- one for
`DerState` columns and one for `Variable` columns. These are used during
expression running to translate a variable name to its column index.

**Step 3**: For each equation in `dae.continuous.equations`, extract the set of
unknown column indices that appear in its residual expression.

**Step 4**: Package everything into the `Incidence` struct.

---

## The ScalarUnknownResolver: Array-Aware Name Lookup

A variable reference in an expression can name a scalar or an array element.
The resolver maps these references to column indices, handling several cases:

```rust
pub(crate) struct ScalarUnknownResolver {
    exact:       HashMap<String, usize>,        // "x"       → idx
    base_all:    HashMap<String, Vec<usize>>,   // "x"       → [idx for x[1], x[2], …]
    base_unique: HashMap<String, usize>,        // "x"       → idx  (only if x is scalar)
}
```

### Why Three Maps?

Consider a model with an array state `u[2]` (two-element array). In the DAE,
this generates two scalar unknowns: `u[1]` at index 0 and `u[2]` at index 1.

An equation might reference:
- `u[1]` — references exactly column 0
- `u[2]` — references exactly column 1
- `u` (whole array) — references both columns 0 and 1

The three maps handle these cases:

| Map | Used when |
|-----|-----------|
| `exact` | The expression references a specific element: `u[1]` → `0` |
| `base_unique` | The variable is scalar: `x` → its single index |
| `base_all` | The expression references the whole array: `sum(u)` → `[0, 1]` |

**Example** (from `incidence.rs` test `test_build_solver_sparsity_triplets_maps_whole_array_refs_to_all_scalars`):

```rust
// array state u[2] → columns 0 and 1
// equation: 0 = y - product(u)   (product sums all u elements)
// incidence: eq0 references columns 0, 1, 2  (u[1], u[2], y)
assert_eq!(triplets, vec![(0, 0), (0, 1), (0, 2)]);
```

When the expression says `product(u)` (a builtin that consumes the whole
array), the resolver returns both columns via `base_all`.

### Name Variants for Indexed Components

Modelica names can appear in different forms due to array subscripts or
qualified connector paths:

- `"support.phi"` — scalar field of a record component
- `"support[1].phi"` — same field but accessed through an array instance

The resolver normalises these via `component_base_name()` (a function from
`rumoca-ir-dae`) which strips trailing subscripts to find the base name.
The test `test_build_solver_sparsity_triplets_resolves_indexed_component_names`
in `incidence.rs` verifies this:

```rust
// state "support.phi" in DAE
// equation references "support[1].phi"  → still resolves to the same column
assert_eq!(triplets, vec![(0, 0)]);
```

---

## Expression Walking: `collect_equation_unknowns`

For each equation, the code calls `collect_equation_unknowns` (in
`incidence.rs`), which uses several collectors:

1. A `DerOperandCollector` runs the equation's RHS expression to find all
   operands of `der(...)` calls, preserving subscripts so that `der(p[1])`
   resolves to the exact scalar `der` unknown rather than the un-subscripted
   base. These are resolved through the `der_resolver` to yield `DerState`
   column indices.

2. `collect_expression_unknowns(expr, variable_resolver, &mut result)` -- runs
   every other variable reference in the expression and resolves them through
   `variable_resolver`.

3. If the equation has an explicit LHS target, `collect_equation_lhs_unknown`
   resolves the LHS reference to its unknown column as well.

### The Crucial Subtlety: `der(x)` Arguments Are Not Variable References

This is the most important design decision in the incidence builder. In
`ExpressionUnknownCollector::visit_builtin_call` (in `incidence.rs`), the
`Der` case simply returns without descending:

```rust
fn visit_builtin_call(&mut self, function: &BuiltinFunction, args: &[Expression]) {
    if *function == BuiltinFunction::Der {
        return;   // do not descend into der() argument
    }
    for arg in args {
        self.visit_expression(arg);
    }
}
```

When the walker encounters `der(x)`, it **stops** — it does not descend into
the argument `x`. This is intentional.

**Why?** Consider the equation `0 = der(x) - y`. The unknowns are `der(x)`
(a `DerState`) and `y` (a `Variable`). If the walker descended into `der(x)`
and treated `x` as a variable reference, it would record a dependency on
*column for x* — but `x` itself is not unknown at this step; it is a *known*
quantity (the current state value). The unknown is `der(x)`, the time rate of
change.

The `DerOperandCollector` in step 1 above is what picks up the `DerState`
dependency. It collects the name and subscripts from `der(x)`, then looks them
up in the `der_resolver` (which maps state names to `DerState` column indices).
So the dependency on `der(x)` is recorded correctly -- but via a separate path
that specifically handles the semantics of `der()`.

The test `test_build_solver_sparsity_triplets_skips_derivative_argument_dependencies`
in `incidence.rs` confirms this:

```rust
// equation 0:  0 = der(x) - z
// equation 1:  0 = z - (x + 1)
//
// unknowns: col 0 = der(x), col 1 = z (algebraic)
//
// eq0 depends on: der(x) [col 0] and z [col 1]
// eq1 depends on: x ← wait, x is a STATE (known), not a DerState
//                 z [col 1]
//
assert!(triplets.contains(&(0, 1)));  // eq0 → z
assert!(triplets.contains(&(1, 0)));  // eq1 → x? No! col 0 is der(x), not x.
// Actually the test says row1 depends on col 0 = x (state column in solver vector)
// and col 1 = z
assert!(!triplets.contains(&(0, 0))); // eq0 does NOT depend on x as a plain variable
```

Wait -- re-reading: the `build_solver_sparsity_triplets` function (in
`incidence.rs`) uses a *different* resolver that maps both states and algebraics to
columns in the solver state vector (all variables, not just DerState). This is
used for the Jacobian sparsity pattern, where `∂F/∂x` is also needed. The
`build_incidence` function used for structural ordering only cares about
`der(x)` as unknowns for the matching step.

This is why there are two separate functions:
- `build_incidence` — for structural analysis (matching, BLT, tearing)
- `build_solver_sparsity_triplets` — for Jacobian sparsity (the solver's
  internal sparse-matrix allocation)

---

## The Full Expression Walker

`ExpressionUnknownCollector` (which implements `ExpressionVisitor`) handles
every expression variant via its visitor methods:

| Expression form | Action |
|-----------------|--------|
| `VarRef { name, subscripts }` | Resolve all indices for `name[subscripts]` via `resolve_var_ref_all`; also run subscripts themselves (they may contain unknowns) |
| `Index { base, subscripts }` | If base is a `VarRef`, combine base and outer subscripts to form a canonical key like `u[1]`; otherwise recurse into base |
| `Binary { lhs, rhs }` | Recurse into both sides |
| `Unary { rhs }` | Recurse into rhs |
| `BuiltinCall(Der, …)` | **Stop — do nothing** (see above) |
| `BuiltinCall(_, args)` | Recurse into all args |
| `FunctionCall { args }` | Recurse into all args |
| `If { branches, else_branch }` | Recurse into all conditions and value branches (a conditional may reference unknowns in any branch) |
| `Array { elements }` | Recurse into all elements |
| `Range { start, step, end }` | Recurse into all three |
| `ArrayComprehension { expr, indices, filter }` | Recurse into range expressions, body, and filter |
| `FieldAccess { base }` | Recurse into base |
| `Literal` / `Empty` | No unknowns |

The walker is **conservative**: it always records a dependency. If a conditional
`if cond then a else b` appears in an equation, both `a` and `b` contribute
to the incidence even though at runtime only one branch executes. This matches
the structural analysis philosophy — it analyses what *could* be needed, not
what will be needed at a specific time.

---

## Building the Dependency Graph

After matching, the incidence is used again to build the **directed dependency
graph** that feeds Tarjan's SCC algorithm (`build_dependency_graph` in
`incidence.rs`):

```rust
pub fn build_dependency_graph(
    eq_unknowns: &[HashSet<usize>],
    match_var: &[Option<usize>],  // match_var[col] = Some(row) if col is matched to row
    n_eq: usize,
) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n_eq];
    for (eq_a, unknowns) in eq_unknowns.iter().enumerate() {
        for var_idx in unknowns (sorted) {
            let eq_b = match_var[var_idx];   // which equation "owns" this unknown?
            if eq_b != eq_a:
                adj[eq_a].push(eq_b);        // eq_a depends on eq_b
        }
    }
}
```

**Interpretation**: if equation `A` references variable `v`, and variable `v`
was matched to equation `B` (meaning `B` is responsible for computing `v`),
then `A` depends on `B`. Equation `A` cannot be evaluated until equation `B`
has produced a value for `v`.

This dependency graph has the same sparsity as the incidence matrix, so it is
cheap to build.

---

## Structural Singularity

If maximum matching fails to pair every equation with a unique unknown, the
system is **structurally singular**. This is a model error: the equations do
not uniquely determine all unknowns.

Common causes:
- **Over-constrained**: two equations both constrain the same variable with no
  other unknown to absorb the extra constraint
- **Under-determined**: an unknown appears in no equation (physically: a
  variable is never computed)

Rumoca reports the unmatched equations and unmatched unknowns explicitly
so the modeler can locate the problem:

```rust
return Err(StructuralError::Singular {
    n_equations: inc.n_eq,
    n_unknowns: inc.n_var,
    n_matched: matching_size,
    unmatched_equations: ...,        // names of equations with no assigned unknown
    unmatched_unknowns: ...,         // names of unknowns that no equation covers
    unmatched_unknown_spans: ...,    // source spans of unmatched unknowns for traceability
});
```

---

## The Incidence Struct

```rust
pub struct Incidence {
    pub n_eq: usize,
    pub n_var: usize,
    pub eq_unknowns: Vec<HashSet<usize>>,           // row i -> set of column indices
    pub unknown_names: Vec<UnknownId>,               // col j -> DerState, Variable, or SolverY name
    pub unknown_spans: Vec<Option<rumoca_core::Span>>, // col j -> source span (for diagnostics)
    pub equation_refs: Vec<EquationRef>,              // row i -> EquationRef(i)
}
```

`eq_unknowns` is the primary data. `unknown_names` and `equation_refs` are
index-to-name maps that allow error messages to say `"f_x[2] depends on
der(body.v)"` rather than `"row 2 depends on column 5"`. `unknown_spans`
carries the source location of each unknown so that structural-singularity
errors can be traced back to the offending model variable.

---

## Worked Example: Two-Mass Spring System

```modelica
model TwoMass
  Real x1, x2;   // positions  (states)
  Real v1, v2;   // velocities (states)
  Real F;        // spring force (algebraic)
  parameter Real m1 = 1, m2 = 2, k = 100;
equation
  der(x1) = v1;                         // eq0
  der(x2) = v2;                         // eq1
  m1 * der(v1) = -F;                    // eq2
  m2 * der(v2) =  F;                    // eq3
  F = k * (x2 - x1);                   // eq4
end TwoMass;
```

**Unknowns** (columns):
```
col 0: der(x1)   col 1: der(x2)   col 2: der(v1)   col 3: der(v2)   col 4: F
```

**Incidence** (rows = equations):

```
      der(x1) der(x2) der(v1) der(v2)  F
eq0:    1       0       0       0       0      (references der(x1))
eq1:    0       1       0       0       0      (references der(x2))
eq2:    0       0       1       0       1      (references der(v1), F)
eq3:    0       0       0       1       1      (references der(v2), F)
eq4:    0       0       0       0       1      (references F; x1, x2 are states = known)
```

**Stored as `eq_unknowns`**:
```
eq_unknowns[0] = {0}
eq_unknowns[1] = {1}
eq_unknowns[2] = {2, 4}
eq_unknowns[3] = {3, 4}
eq_unknowns[4] = {4}
```

**Matching** (one assignment per equation):
```
eq0 → der(x1)   eq1 → der(x2)   eq2 → der(v1)   eq3 → der(v2)   eq4 → F
```
Perfect matching — no structural singularity.

**Dependency graph** (from matching + incidence):
```
eq2 depends on eq4  (eq2 references F, F is matched to eq4)
eq3 depends on eq4  (same reason)
```

**Tarjan SCCs / BLT order**: {eq0}, {eq1}, {eq4}, {eq2}, {eq3} — or any order
that puts eq4 before eq2 and eq3. No algebraic loops.

---

## Summary of Key Design Choices

| Choice | Reason |
|--------|--------|
| Sparse set-of-sets storage | Physical models are sparse; dense matrix would waste memory |
| Three-map resolver (exact, base_all, base_unique) | Handles scalars, whole-array refs, and element refs uniformly |
| `der(x)` argument not recursed into | `x` is a known state; `der(x)` is the unknown — different columns |
| Two separate functions (incidence vs sparsity triplets) | Structural analysis needs DerState unknowns; Jacobian needs all-variable sparsity |
| Sorted traversal of unknowns in dependency graph | Ensures deterministic BLT ordering across runs |
| Conservative walker (all if-branches included) | Structural analysis must not miss dependencies due to runtime branching |
