# Phase 2: Name Resolution and Scope Trees

## Overview

The resolution phase turns string-based name references in the AST into stable,
integer **DefIds**. It builds the **scope tree** that encodes which names are
visible where, resolves imports, and detects inheritance cycles. It is
implemented in `crates/rumoca-phase-resolve/`.

Input: `StoredDefinition` (raw AST from parse phase)  
Output: `ResolvedTree` — same tree with all `def_id`, `scope_id` fields populated

---

## Big Picture: Input and Output

```
  StoredDefinition  (raw AST from phase 1)
        │
        ▼
  ┌─────────────────────────────────────┐
  │    Phase 2: Resolve and Scope       │
  │                                     │
  │  • Allocate DefIds for every        │
  │    definition (builtins first)      │
  │  • Build the scope tree             │
  │  • Resolve imports and extends      │
  │  • Detect inheritance cycles        │
  └─────────────────────────────────────┘
        │
        ▼
  ResolvedTree  (AST + DefIds + scope tree)
```

---

## Core Identifiers

### DefId

A `DefId` is a 32-bit integer that uniquely identifies any definition (class,
component, enum literal, builtin) in the compilation session.

- Allocated sequentially during **Phase 1 (registration)**
- Builtins (`Real`, `Integer`, `Boolean`, `String`, `der`, `sin`, …) receive
  the lowest DefIds so `id < BUILTIN_CUTOFF` is an O(1) builtin check
- Used as hashmap keys in all downstream phases instead of string comparisons

### ScopeId

A `ScopeId` is a 32-bit index into `ScopeTree.scopes: Vec<Scope>`. `ScopeId(0)`
is always the global scope.

---

## Three-Phase Resolution Architecture

Resolution runs in three sequential sub-phases:

### Sub-phase 1: Registration (`registration.rs`)

Walk all classes recursively, allocating DefIds and creating scopes:

```
For each class (depth-first):
  1. Allocate DefId for the class
  2. Create a Scope for the class
  3. Register the class name in its parent scope
  4. Allocate DefIds for each component
  5. Register component names in the class scope
  6. Recurse into nested classes
```

After registration the maps are:
- `def_map: IndexMap<DefId, String>` — DefId → qualified name (`"Pkg.Model"`)
- `name_map: IndexMap<String, DefId>` — qualified name → DefId

### Sub-phase 2a: Extends Resolution (`extends.rs`)

Process inheritance clauses **breadth-first by nesting depth** so that parent
classes are fully resolved before their children:

```
Queue: all top-level classes at depth 0
For each class at current depth:
  1. Resolve imports in this class (MLS §13.2)
  2. Resolve each extends clause:
     a. Qualified-name lookup, excluding self (prevents trivial cycles)
     b. Record inheritance edge: (this_class_def_id, base_def_id)
     c. Update class_to_bases index
  3. Enqueue nested classes at depth+1
```

The "exclusion" mechanism (step 2a) handles the pattern:

```modelica
package Derived extends Base
  redeclare record extends State end State;   -- "State" must find Base.State,
end Derived;                                  --  not Derived.State (which is itself)
```

### Sub-phase 2b: Contents Resolution (`contents.rs`)

With the complete inheritance graph from 2a, resolve everything inside class
bodies: equations, statements, expressions, component type references,
function call names.

### Sub-phase 3: Cycle Detection (`cycles.rs`)

DFS over the inheritance graph built in 2a:
- Maintain `visited` and `in_path` sets
- If an edge points to a node already in `in_path`, a cycle is found
- Report the full cycle path in the error message

Direct cycles (A extends A) are caught earlier in 2a via a
`resolving_extends: HashSet<DefId>` guard.

---

## The Scope Tree

### Scope struct

```rust
pub struct Scope {
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    pub members: IndexMap<String, DefId>,    // names defined directly here
    pub imports: Vec<scope::Import>,         // imports active in this scope
}
```

### ScopeKind and Lookup Semantics

| Kind | Lookup behaviour |
|------|-----------------|
| `Global` | Root; no parent; builtins live here |
| `Package` | Normal hierarchical lookup |
| `Class` | Normal hierarchical lookup |
| `Encapsulated` | Lookup **stops** here unless target is in `Modelica.*` (MLS §5.3.1) |
| `Function` | Lookup sees inputs/outputs; then parent |
| `ForLoop` | Local iteration variables; then parent |

---

## Multi-Level Name Lookup

Given a simple name `"Real"` and a starting `ScopeId`:

```
lookup("Real", scope_id):
  1. Check scope.members["Real"]     → hit? return DefId
  2. Check each import in scope:
       Qualified/Renamed: does import alias match "Real"?
       Unqualified: is "Real" a child of the imported package?
  3. If scope.kind == Encapsulated:  stop (unless Modelica.*)
  4. If scope.parent.is_some():      recurse with parent scope
  5. Not found → error ER001 (undefined name)
```

For a qualified name `"A.B.C"`:
- Resolve first segment `"A"` via the scope chain
- Then use `name_map["A.B"]` → `name_map["A.B.C"]` (O(1) map lookups)

---

## Import Resolution

Imports are resolved during Sub-phase 2a and converted from syntactic
`ast::Import` variants into semantic `scope::Import` variants stored directly
in the scope's `imports` list.

| Syntactic form | Semantic scope entry |
|----------------|---------------------|
| `import A.B.C;` | `Qualified { alias: "C", def_id: id_of_C }` |
| `import D = A.B.C;` | `Renamed { alias: "D", def_id: id_of_C }` |
| `import A.B.*;` | `Unqualified { children: IndexMap<name, DefId> }` |
| `import A.B.{C,D};` | Expanded to two `Qualified` entries |

The global `package_children` index (`IndexMap<pkg_name, IndexMap<child, DefId>>`)
allows wildcard import lookup to stay O(1).

---

## Extends Resolution Detail

For a class `MyModel extends Base.Physical.Body`:

1. Lookup `"Base"` in `MyModel`'s scope → DefId for `Base`
2. Map lookup: `name_map["Base.Physical"]` → DefId, `name_map["Base.Physical.Body"]` → DefId
3. Record edge `(MyModel_def_id, Body_def_id)` in `inheritance_edges`
4. Add Body's members to MyModel's scope (inherited name resolution)

**Special case — inherited member lookup**: When a class member's extends clause
refers to a name that exists only in an ancestor:

```modelica
package Pkg extends Base
  redeclare record extends State ... end State;   -- State is in Base, not Pkg
end Pkg;
```

If normal scope lookup fails, the resolver tries `lookup_inherited_member(container, name)`:
- Walks the container's bases via `class_to_bases`
- Returns the first base that defines the name
- Allows the extends clause to refer to an inherited definition

---

## Output: ResolvedTree

`ResolvedTree` is a newtype wrapper around `ClassTree`:

```rust
pub struct ResolvedTree(pub ClassTree);
```

After this phase, the following fields are populated **everywhere** in the tree:

| Field | Type | Populated on |
|-------|------|-------------|
| `ClassDef.def_id` | `Option<DefId>` | every class |
| `ClassDef.scope_id` | `Option<ScopeId>` | every class |
| `Component.def_id` | `Option<DefId>` | every component |
| `ComponentReference.def_id` | `Option<DefId>` | first part of each reference |
| `Extend.base_def_id` | `Option<DefId>` | every extends clause |
| `Name.def_id` | `Option<DefId>` | every type name reference |

Still `None` at this point (set by downstream phases):
- `Component.type_id`, `ClassDef.type_id` — set by typecheck
- `TypeTable` entries — built during typecheck

---

## Error Catalogue

| Code | Meaning |
|------|---------|
| `ER001` | Undefined name |
| `ER002` | Import target not found |
| `ER003` | Ambiguous name (multiple imports match) |
| `ER004` | Inheritance cycle detected |
| `ER005` | Redeclare target not found in base |

---

## Key Files

| File | Purpose |
|------|---------|
| `rumoca-phase-resolve/src/lib.rs` | Top-level `resolve()` entry point; phase orchestration |
| `rumoca-phase-resolve/src/registration.rs` | DefId/ScopeId allocation |
| `rumoca-phase-resolve/src/extends.rs` | Extends and import resolution (sub-phase 2a) |
| `rumoca-phase-resolve/src/contents.rs` | Body expression resolution (sub-phase 2b) |
| `rumoca-phase-resolve/src/lookup.rs` | Scope chain lookup, inherited member lookup |
| `rumoca-phase-resolve/src/cycles.rs` | DFS cycle detection |
| `rumoca-ir-ast/src/scope.rs` | `ScopeTree`, `Scope`, `ScopeKind` type definitions |
