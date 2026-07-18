# Phase 4: Instantiation

## Overview

These two phases — instantiation (this document) and
[flattening](../phase5_flatten/flatten.md) — bridge the hierarchical class-based
world of Modelica and the flat equation-based world needed for analysis and
simulation. Instantiation comes first: it applies modifications and resolves
the class hierarchy for one specific model, producing an `InstancedTree`.
Flattening then walks that tree and emits a globally-qualified flat model.

- Implementation: `crates/rumoca-phase-instantiate/`
- Output IR: `InstancedTree` (typed AST + per-instance overlay)

---

## Big Picture: Input and Output

```
  TypedTree  (from phase 3)  +  root model name
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
  │    flow via stacks                  │
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

## InstantiateContext (`lib.rs:139–209`)

The instantiator threads a mutable context through the recursion:

```rust
struct InstantiateContext {
    mod_env: ModificationEnvironment,       // active modifier bindings
    known_int_params: FxHashMap<String, i64>, // evaluated integer parameters
    next_instance_id: u32,                  // monotone counter for unique IDs
    inner_scopes: Vec<IndexMap<String, InnerDeclaration>>, // inner/outer stack
    variability_stack: Vec<Variability>,    // inherited variability
    causality_stack: Vec<Causality>,        // inherited causality
    flow_stack: Vec<bool>,                  // inherited flow prefix
    stream_stack: Vec<bool>,                // inherited stream prefix
    expandable_stack: Vec<bool>,            // inside expandable connector?
    overconstrained_stack: Vec<Option<(usize, String)>>, // OC connector info
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

This is implemented via stacks:
- Before entering a record component with `parameter` variability, push
  `parameter` onto `variability_stack`
- All components instantiated while this is on the stack inherit `parameter`
- Pop on exit

The same mechanism handles `input`, `output`, `flow`, `stream`.

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
2. When encountering an `outer` reference, walk `inner_scopes` from innermost
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

The next phase, [flattening](../phase5_flatten/flatten.md), walks this tree and
produces the flat `Model` IR consumed by DAE construction.
