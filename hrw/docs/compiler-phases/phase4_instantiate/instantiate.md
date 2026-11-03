# Phase 4: Instantiation

## Overview

These two phases — instantiation (this document) and
[flattening](../phase5_flatten/flatten.md) — bridge the hierarchical class-based
world of Modelica and the flat equation-based world needed for analysis and
simulation. Instantiation comes first: it applies modifications and resolves
the class hierarchy for one specific model, producing an `InstancedTree`.
Flattening then runs that tree and emits a globally-qualified flat model.

**Pipeline position:** Instantiation runs *before* type checking in the Rumoca
pipeline (despite the doc directory numbering). This is intentional — typecheck
needs the modification context that instantiation provides in order to evaluate
array dimensions correctly (MLS §10.1).

- Implementation: `crates/rumoca-phase-instantiate/`
- Output IR: `InstancedTree` (resolved AST + per-instance overlay)

---

## Big Picture: Input and Output

```
  ResolvedTree  (from phase 2)  +  root model name
        │
        ▼
  ┌─────────────────────────────────────┐
  │      Phase 4: Instantiate           │
  │                                     │
  │  • Apply modifications outside-in   │
  │    (MLS §7.2)                       │
  │  • Resolve redeclare clauses        │
  │    (MLS §7.3)                       │
  │  • Process extends with caching     │
  │  • Inherit variability/causality/   │
  │    flow via scope frames            │
  │  • Resolve inner/outer (MLS §5.4)   │
  └─────────────────────────────────────┘
        │
        ▼
  InstancedTree  (TypedTree + per-instance overlay)
```

---

## What "Instantiation" Means

In Modelica, a model is a class. To simulate it you instantiate it: create one
concrete object (the "root instance") by recursively applying all modifications
and evaluating all structural parameters. The result is a tree of component
instances with concrete types and parameter values.

**Key rule**: modifications override from outside in. An enclosing modifier
takes priority over an inner default.

```modelica
model System
  Body b(mass = 2.0);   -- override b's mass parameter
end System;
```

Here `mass = 2.0` in `System` overrides `Body`'s default value for `mass`.

---

## InstantiateContext (in `lib.rs`)

The instantiator threads a mutable context through the recursion. Earlier
versions of this struct used separate parallel stacks for each inherited
prefix (variability, causality, flow, stream, expandable, overconstrained).
These have been consolidated into a single `scope_frames: Vec<ScopeFrame>`
stack, making push/pop logic simpler and ensuring all per-scope state moves
together.

### ScopeFrame

Each frame captures the inherited prefixes and connector metadata for one
level of the component nesting:

```rust
struct ScopeFrame {
    variability: Option<Variability>,
    causality: Option<Causality>,
    flow: bool,
    stream: bool,
    expandable: bool,
    overconstrained: Option<(usize, String)>,
    protected: bool,
}
```

A `ScopeFrame` is created via `ScopeFrame::inherited_from_component()` which
inspects the component's declared prefixes and only stores values that should
propagate inward (e.g., `parameter` and `constant` variability propagate, but
`discrete` does not).

### InstantiateContext

```rust
pub struct InstantiateContext {
    pub diags: Diagnostics,                          // diagnostics collector
    context_path: Vec<(String, Vec<i64>)>,           // current instance path
    next_instance_id: u32,                           // monotone counter for unique IDs
    mod_env: ModificationEnvironment,                // active modifier bindings
    inner_scopes: Vec<IndexMap<String, InnerDeclaration>>, // inner/outer stack
    missing_inners: Vec<MissingInnerInfo>,           // unresolved outer references
    scope_frames: Vec<ScopeFrame>,                   // unified inherited-prefix stack
    template_cache: ClassTemplateCache,              // avoid recomputing identical instances
    known_int_params: FxHashMap<String, i64>,        // evaluated integer parameters
    allow_partial_instantiation: bool,               // partial class support
    options: InstantiateOptions,                     // session/caller behavior config
    active_instantiations: Vec<InstantiationFrame>,  // recursion detection
    source_scope_index: SourceScopeIndex,            // declaration scopes by DefId
    active_type_overrides: Vec<TypeOverrideMap>,     // redeclare type/package overrides
    // plus additional fields
}
```

---

## Modification Application

When instantiating a component `b` of type `Body`:

1. Push `b`'s modification environment (e.g., `mass → 2.0`) onto `mod_env`
2. Instantiate `Body` with that environment active
3. Inside `Body`, whenever a component has a binding or default value, check
   `mod_env` first; the outer modifier wins (MLS §7.2)
4. Pop the environment when done

**Redeclarations** (MLS §7.3): Override inherited types. The
`build_type_override_map()` and `apply_type_override()` functions resolve
redeclare modifiers before descending.

---

## Extends Clause Processing

Each extends clause is resolved via `class_extends_cached()` (a cached version
of `process_extends()`). The cache key is the (class DefId, active modifier
hash), ensuring different instantiations with different modifiers compute
different effective members.

Inheritance is applied *before* instantiation proceeds, so that inherited
components and equations are visible to the rest of the body.

---

## Inherited Variability and Causality (MLS §4.4.2)

Record fields inherit prefix qualifiers from their enclosing context:

```modelica
parameter MyRecord r;  -- all fields of r become parameters
input MyConnector c;   -- all fields of c become inputs
```

This is implemented via the `scope_frames` stack:
- Before entering a record component with `parameter` variability, push a
  `ScopeFrame` with `variability: Some(Parameter)` onto `scope_frames`
- All components instantiated while this frame is on the stack inherit
  `parameter`
- Pop the frame on exit

The same mechanism handles `input`, `output`, `flow`, `stream`, `expandable`,
and `protected` -- all captured in a single `ScopeFrame` rather than in
separate stacks.

---

## Inner/Outer Resolution (MLS §5.4)

The `inner`/`outer` mechanism lets a deeply-nested component share state with a
top-level declaration:

```modelica
model System
  inner World world;    -- the "real" instance, declared at top
end System;

model Body
  outer World world;    -- reference to System.world
end Body;
```

**Resolution**:
1. When encountering an `inner` declaration, register it in `inner_scopes` with
   its path and DefId (`register_inner()`).
2. When encountering an `outer` reference, run `inner_scopes` from innermost
   to outermost looking for a matching name (`find_inner()`).
3. If no matching `inner` is found and the spec requires one: **synthetic inner
   synthesis** — the root instantiation is re-run after pre-registering a
   synthesized `inner` component at the root scope.

---

## Output: InstancedTree

`InstancedTree` is the typed AST plus an `InstanceOverlay`:
- Every component has an instance ID and resolved parameter values
- Class override maps (redeclare results) are recorded
- The overlay carries per-instance data without duplicating the class structure

The next phase, [flattening](../phase5_flatten/flatten.md), runs this tree and
produces the flat `Model` IR consumed by DAE construction.

---

## Key Files

The instantiate crate has grown to 25+ source files (excluding tests).
The most important ones:

| File | Purpose |
|------|---------|
| `lib.rs` | Entry point; `InstantiateContext` and `ScopeFrame` structs; top-level orchestration |
| `mod_env.rs` | Modification environment push/pop and modifier resolution (MLS 7.2) |
| `inheritance.rs` | Extends clause processing and member merging (MLS 7.1) |
| `array_expansion.rs` | Array component expansion (e.g., `Resistor r[100]`) |
| `type_overrides.rs` | Redeclare type/package override resolution (MLS 7.3) |
| `type_lookup.rs` | Component type specifier resolution and subtype checks |
| `nested_scope.rs` | Nested class and redeclare-class modifier handling |
| `dims.rs` | Array dimension evaluation during instantiation |
| `connections.rs` | Connection extraction from equations (MLS 9) |
| `templates.rs` | Class template caching for repeated identical instances |
| `attributes.rs` | Component attribute and binding extraction |
| `component_loop.rs` | Per-class component instantiation loop |
| `source_scope.rs` | Source declaration scope tracking |
| `instance_sections.rs` | Algorithm/equation conversion to instance representation |
| `plug_compat.rs` | Plug-compatibility checks for redeclarations (MLS 6.4-6.6) |
| `evaluate_annotation.rs` | `annotation(Evaluate=true)` detection (MLS 18.3) |
| `errors.rs` | Phase-local error types (EI0xx codes) |
| `package_constant_imports.rs` | Package constant alias resolution through type overrides |
| `path_utils.rs` | Qualified class-name parsing utilities |
| `traversal_adapter.rs` | AST traversal helpers for nested classes and modifications |
