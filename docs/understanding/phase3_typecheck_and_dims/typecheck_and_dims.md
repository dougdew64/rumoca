# Phase 3: Type Checking and Dimension Evaluation

## Overview

The type-checking phase assigns a **TypeId** to every expression and component,
evaluates array dimensions, identifies structural parameters, and validates
variability and causality constraints.

- Implementation: `crates/rumoca-phase-typecheck/src/lib.rs` (~1,982 lines)
- Key sub-file: `src/typechecker/late_methods.rs` (~1,829 lines)

Input: `ResolvedTree`  
Output: `TypedTree` — same tree with `TypeId`s populated and `TypeTable` complete

---

## Big Picture: Input and Output

```
  ResolvedTree  (from phase 2)
        │
        ▼
  ┌─────────────────────────────────────┐
  │  Phase 3: Typecheck and Dimensions  │
  │                                     │
  │  • Multi-pass dimension evaluation  │
  │    (MLS §10.1)                      │
  │  • Identify structural parameters   │
  │  • Assign TypeIds (interned types)  │
  │  • Validate variability and         │
  │    causality constraints            │
  └─────────────────────────────────────┘
        │
        ▼
  TypedTree  (ResolvedTree + TypeIds + TypeTable)
```

---

## TypeIds and the TypeTable

### What is a TypeId?

A `TypeId` is a 32-bit integer that identifies a specific Modelica type. Like
`DefId` for definitions, it enables O(1) type comparisons instead of string
matching.

TypeIds are allocated and stored in the `TypeTable` embedded in the `ClassTree`.

### Types in the TypeTable

| TypeTable entry | Represents |
|-----------------|-----------|
| `BuiltinType(Real / Integer / Boolean / String)` | The four Modelica primitives |
| `ClassType { def_id, fields }` | A user-defined record or model |
| `ArrayType { element_type_id, dims }` | An array of any base type |
| `EnumerationType { def_id, literals }` | An enum |
| `FunctionType { inputs, outputs }` | A function's signature |

### TypeId Allocation

The `build_type_context()` method (lines 660–733) registers types:

1. Register all built-in types first (stable TypeIds for `Real`, `Integer`, etc.)
2. Walk all user-defined classes:
   - Records → `register_class_type()` (line 801)
   - Enumerations → `register_enumeration_type()` (line 782)
   - Type aliases (e.g., `type Velocity = Real`) → `TypeAlias` placeholder, resolved in a second pass (line 711)

**Type alias resolution strategy (line 745–850)**:
- Direct qualified-name lookup
- DefId-anchored resolution (via `name_map`)
- Dotted-suffix fallback index for imported types like `SI.Reluctance` that
  may not be fully qualified in scope

---

## Array Dimension Evaluation (MLS §10.1)

This is one of the more complex parts of type checking because dimensions can
depend on parameters that are themselves computed from other parameters.

### Multi-Pass Loop (`evaluate_all_dimensions_multi_pass`, lines 1738–1775)

The phase runs up to **10 passes** until no new dimensions are resolved:

Each pass performs four sequential steps in order:

1. **Explicit dimension evaluation** (line 1751):
   - Evaluates non-colon subscripts like `Real x[3]` or `Real x[n+1]`
   - Uses `eval_dimension_with_fallback()` — tries evaluating the expression,
     skips if it depends on a not-yet-resolved parameter
   - Scope-aware: looks up parameters in the component's declaring scope

2. **Colon inference** (line 1754):
   - Infers dimensions for variables declared as `Real x[:]`
   - Checks the variable's binding expression or start value
   - Uses `infer_dimensions_from_binding_with_scope()` from `rumoca-eval-ast`

3. **Integer re-evaluation** (line 1758):
   - Re-evaluates dimensions that depend on Integer parameters
   - Handles patterns like `parameter Integer n = size(a, 1)`

4. **Boolean/Real re-evaluation** (line 1762):
   - Enables if-expression evaluation:
     `Real x[if flag then 3 else 5]`

### Why Multiple Passes?

Consider:
```modelica
parameter Integer m = 2;
parameter Integer n = m + 1;   // depends on m — needs pass 2
Real x[n];                     // depends on n — needs pass 3
```

Each pass resolves one more layer of dependency. The loop terminates when a
full pass makes no progress (convergence) or when the limit (10) is reached.

### Dimension Validation

After the loop, any component that still has an unresolved (colon) dimension
and is not a connector input triggers error **ET004**:

```
ET004: dimension of 'x' could not be evaluated — it depends on a parameter
       that is not computable at translation time
```

Input variables with colon dims are permitted because connections will
supply the concrete size at instantiation time.

---

## Structural Parameters (MLS §18.3)

### Definition

A parameter is **structural** if it appears in:
- An array dimension expression (`Real x[n]`)
- A `for`-loop range (`for i in 1:n loop ...`)
- An `if`-condition that controls compilation decisions

Structural parameters must be fully evaluable at **translation time**; they
cannot be changed after the model is compiled. The flag is `Component.is_structural: bool`.

### Identification (`mark_structural_parameters`, lines 1254–1281)

1. Collect all `ComponentReference`s that appear in dimension expressions
2. Collect references from for-loop ranges and if-conditions
3. For each collected reference that resolves to a `parameter` variability
   component, set `is_structural = true`

---

## Type Inference for Expressions

`infer_expression_type()` (lines 1426–1449 in `late_methods.rs`) walks the
expression tree recursively:

| Expression kind | Type inference rule |
|-----------------|-------------------|
| `Terminal::Real` | `TypeId::Real` |
| `Terminal::Integer` | `TypeId::Integer` |
| `Terminal::Boolean` | `TypeId::Boolean` |
| `Terminal::String` | `TypeId::String` |
| `ComponentReference` | Lookup type via DefId → TypeTable |
| `BinaryOp(+, -, *, /)` | Both operands must be numeric; result type follows Modelica promotion rules |
| `FunctionCall` | Lookup function signature; return type is output type |
| `FieldAccess(base, field)` | Resolve base type to `ClassType`, then lookup `field` in its fields |
| `Array { elements }` | All elements must share a type; result is `ArrayType` |
| `If { branches, else_branch }` | All branches must have the same type |

**Record constructors**: `MyRecord(x=1, y=2)` is treated as a function call
whose return type is `MyRecord`'s TypeId (line 1451).

**Component reference resolution** (lines 1554–1591): Uses longest-prefix
matching through the dot-separated parts, resolving each segment to the correct
field of the previous type.

---

## Variability Checking

Modelica defines a total order on variability:

```
constant  <  parameter  <  discrete  <  continuous
(most fixed)                            (most free)
```

### Rules Enforced (lines 1283–1308)

1. **Binding expression** must have variability ≤ the component's declared
   variability. You cannot bind a `parameter` to a `continuous` expression.

2. **Start modification** (when specified via a modifier) must also respect
   variability.

Variability of an expression is the **maximum** variability of any variable it
references. A pure numeric literal has variability `constant`.

### Error

**ET003** is emitted when a binding expression's variability exceeds the
component's declared variability.

---

## Causality Validation

- **Inputs** should not have explicit bindings — their value comes from
  connections. If an input has a binding, warning **ET005** is emitted (soft
  error; does not stop compilation).

- **Output** balance (one equation per output) is checked at the DAE level, not here.

---

## Error Catalogue

| Code | Meaning |
|------|---------|
| `ET001` | Type name not found |
| `ET002` | Type mismatch (e.g., assigning Real to Integer) |
| `ET003` | Variability violation (binding too free for component) |
| `ET004` | Array dimension could not be evaluated |
| `ET005` | Input variable has explicit binding (warning) |

---

## Key Files

| File | Purpose |
|------|---------|
| `rumoca-phase-typecheck/src/lib.rs` | Entry point; `TypeChecker` struct; phase orchestration |
| `rumoca-phase-typecheck/src/typechecker/late_methods.rs` | All major algorithms: type inference, dimension eval, variability check |
| `rumoca-ir-ast/src/lib.rs` | `TypeTable`, `TypeId` definitions (embedded in ClassTree) |
| `rumoca-eval-ast/` | Compile-time expression evaluator used for dimension inference |
