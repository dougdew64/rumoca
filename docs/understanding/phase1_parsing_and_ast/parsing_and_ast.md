# Phase 1: Parsing and the AST

## Overview

The parsing phase transforms raw Modelica source text into an in-memory tree
called the **Abstract Syntax Tree (AST)**. It is implemented in:

- `crates/rumoca-phase-parse/` — the parser
- `crates/rumoca-ir-ast/` — the AST type definitions

---

## Big Picture: Input and Output

```
  Modelica source text (.mo)
        │
        ▼
  ┌─────────────────────────────────────┐
  │       Phase 1: Parsing              │
  │                                     │
  │  • Tokenize via Parol lexer         │
  │  • Apply LL(k) grammar productions  │
  │  • Run grammar-action modules to    │
  │    assemble AST nodes               │
  └─────────────────────────────────────┘
        │
        ▼
  StoredDefinition  (the AST)
```

---

## Parser Generator: Parol

Rumoca uses the **[Parol](https://crates.io/crates/parol)** LL(k) parser generator.

- The Modelica grammar is written in Parol's DSL.
- Parol generates two files at build time:
  - `src/generated/modelica_parser.rs` — the generated parse table / state machine
  - `src/generated/modelica_grammar_trait.rs` — the `ModelicaGrammarTrait` trait with one method per grammar production
- Rumoca implements `ModelicaGrammarTrait` in action modules (see below). Each
  method receives the already-parsed children and assembles the higher-level IR node.

### Grammar Action Modules

The grammar actions are split by topic:

| Module | Grammar rules handled |
|--------|----------------------|
| `definitions.rs` | Class definitions, extends, imports, element composition |
| `expressions.rs` | All expression forms and operator precedence |
| `equations.rs` | Equation and statement structures |
| `components.rs` | Component declarations |
| `references.rs` | Component references (dotted paths) |
| `sections.rs` | Algorithm and equation sections |
| `helpers.rs` | Shared utilities |

---

## Key Grammar Rules

### Class Definitions

A Modelica class can be:
`model`, `class`, `block`, `connector`, `record`, `type`, `package`, `function`

With optional prefixes: `encapsulated`, `partial`, `expandable`, `operator`,
`pure`/`impure`, `final`, `replaceable`.

Each class is parsed into a `ClassDef`. The top-level file is a `StoredDefinition`
containing an `IndexMap<String, ClassDef>`.

### Expressions and Operator Precedence

Parol's LL(k) grammar encodes precedence via layered rule nesting (lowest
precedence outermost):

1. `or`
2. `and`
3. Comparisons: `==  <>  <  <=  >  >=`
4. Addition: `+  -  .+  .-`
5. Multiplication: `*  /  .*  ./`
6. Exponentiation: `^  .^` (right-associative)
7. Unary: `-  +  not`
8. Postfix: function call, array index, field access

### Special Expression Forms

```modelica
a[i, j]           -- ArrayIndex  
(func())[2]       -- ArrayIndex on a non-name base  
(func()).field    -- FieldAccess on a non-name base  
{expr for i in 1:n if cond}  -- ArrayComprehension  
```

### Equations

```modelica
lhs = rhs           -- Simple
connect(c1, c2)     -- Connect
for i in r loop … end for   -- For
when cond then … elsewhen … end when  -- When
if cond then … elseif … else … end if -- If
f(args);            -- FunctionCall (as statement)
assert(cond, msg)   -- Assert
```

### Algorithm Sections

Algorithm blocks contain `Statement`s:
`Assignment`, `Return`, `Break`, `For`, `While`, `If`, `When`,
`FunctionCall`, `Reinit`, `Assert`.

Multiple output assignment: `(a, b) := func(x)`.

---

## AST Type Definitions

All types live in `crates/rumoca-ir-ast/src/lib.rs`.

### StoredDefinition (line ~480)

```rust
pub struct StoredDefinition {
    pub classes: IndexMap<String, ClassDef>,
    pub within: Option<Name>,   // from the `within` clause at top of file
}
```

The `within` clause declares the package context (e.g., `within Modelica.Math;`).

### ClassDef (line ~651)

```rust
pub struct ClassDef {
    pub def_id: Option<DefId>,            // set by resolve phase
    pub scope_id: Option<ScopeId>,        // set by resolve phase
    pub name: Token,
    pub class_type: ClassType,
    pub encapsulated: bool,
    pub partial: bool,
    pub extends: Vec<Extend>,            // inheritance (MLS §7.1)
    pub imports: Vec<Import>,            // imports (MLS §13.2)
    pub classes: IndexMap<String, ClassDef>, // nested class definitions
    pub components: IndexMap<String, Component>,
    pub equations: Vec<Equation>,
    pub initial_equations: Vec<Equation>,
    pub algorithms: Vec<Vec<Statement>>,
    pub initial_algorithms: Vec<Vec<Statement>>,
    pub enum_literals: Vec<EnumLiteral>,  // for enumeration types
    pub is_replaceable: bool,
    pub constrainedby: Option<Name>,
    pub array_subscripts: Vec<Subscript>, // for type aliases: `type Vec3 = Real[3]`
    pub external: Option<ExternalFunction>, // MLS §12.9 C/Fortran interface
}
```

### Component (line ~487)

```rust
pub struct Component {
    pub type_name: Name,
    pub variability: Variability,   // constant | parameter | discrete | (continuous)
    pub causality: Causality,       // input | output | (none)
    pub connection: Connection,     // flow | stream | (none)
    pub shape: Vec<usize>,          // literal array dimensions: Real x[3]
    pub shape_expr: Vec<Subscript>, // parametric dimensions: Real x[n]
    pub modifications: IndexMap<String, Expression>,
    pub binding: Option<Expression>,  // explicit: Real x = expr;
    pub start: Expression,
    pub condition: Option<Expression>,
    // plus: final, inner, outer, each, is_replaceable, ...
}
```

Key distinction: `shape` holds dimensions that are **already-known integers** at
parse time; `shape_expr` holds dimensions given as expressions (e.g., `Real x[n]`
where `n` is a parameter). The typecheck phase evaluates `shape_expr`.

### Expression (line ~1039)

18 variants:

| Variant | Example |
|---------|---------|
| `Terminal { terminal_type, token }` | `3.14`, `true`, `"hello"` |
| `ComponentReference { parts, subscripts }` | `body.v[1]` |
| `BinaryOp { operator, lhs, rhs }` | `x + 1` |
| `UnaryOp { operator, operand }` | `-x` |
| `FunctionCall { name, arguments }` | `sin(x)`, `der(x)` |
| `If { branches, else_branch }` | `if c then a else b` |
| `Array { elements }` | `{1, 2, 3}` |
| `ArrayIndex { base, subscripts }` | `a[i]`, `(f())[2]` |
| `FieldAccess { base, field }` | `r.x`, `(f()).field` |
| `ArrayComprehension { expr, indices, filter }` | `{i^2 for i in 1:n}` |

`Arc<Expression>` is used for sub-expressions in hot paths to reduce clone cost.

### Equation (line ~972)

```rust
pub enum Equation {
    Empty,
    Simple { lhs: Expression, rhs: Expression },
    Connect { lhs: ComponentReference, rhs: ComponentReference },
    For { indices: Vec<ForIndex>, equations: Vec<Equation> },
    When(Vec<EquationBlock>),      // one block per when/elsewhen branch
    If(Vec<EquationBlock>),
    FunctionCall { name: Name, arguments: Vec<Expression> },
    Assert { condition: Expression, message: Expression, level: Option<Expression> },
}
```

`EquationBlock` pairs a condition with a list of body equations (used for
`when`/`elsewhen` and `if`/`elseif`).

`ForIndex` is `{ ident: Token, range: Expression }`. Multiple indices give
multi-dimensional iteration: `for i in 1:m, j in 1:n loop ...`.

### Subscript (line ~1594)

```rust
pub enum Subscript {
    Expression(Expression),  // concrete: a[3], a[i]
    Range,                   // wildcard: a[:]
    Empty,
}
```

Used both for array indexing and for dimension declarations.

### Import (line ~814)

Four import forms (MLS §13.2):

| Form | Syntax | Variant |
|------|--------|---------|
| Qualified | `import A.B.C;` | `Qualified` — last component available as `C` |
| Renamed | `import D = A.B.C;` | `Renamed` — available as `D` |
| Unqualified (wildcard) | `import A.B.*;` | `Unqualified` — all children imported |
| Selective | `import A.B.{C, D};` | treated as multiple `Qualified` imports |

### Extend (line ~765)

```rust
pub struct Extend {
    pub base_name: Name,               // unresolved at parse time
    pub base_def_id: Option<DefId>,    // set by resolve phase
    pub modifications: Vec<ExtendModification>,
    pub break_names: Vec<String>,      // selective extension (MLS §7.4)
}
```

### ComponentReference (line ~912)

```rust
pub struct ComponentReference {
    pub parts: Vec<ComponentRefPart>,  // one per dotted segment: a.b.c → [a, b, c]
    pub def_id: Option<DefId>,         // set by resolve for the FIRST part only
    pub local: bool,                   // leading dot for local lookup
}
```

Each `ComponentRefPart` has `ident: Token` and `subscripts: Vec<Subscript>`,
allowing `a[1].b[2]` to be represented naturally.

---

## Error Recovery

Recovery is a two-phase strategy (`crates/rumoca-phase-parse/src/lib.rs:297–387`):

1. **Full parse**: Run the complete Parol parser. Collect errors.
2. **Recovery fallback**: If errors occurred, switch to a lightweight recovery
   parser (`recovery.rs`) that:
   - Repeatedly inserts missing semicolons at expected locations
   - Re-parses after each insertion (up to 32 passes)
   - Produces a best-effort AST with the inserted corrections noted
   - Maps error locations back to original source positions

The recovery parser understands class headers and `within`/`end` structure,
so it can reconstruct class boundaries even when the body contains errors.

**Error types** (`errors.rs`):

| Variant | Meaning |
|---------|---------|
| `SyntaxError { message, expected, unexpected, span }` | Parse failure with context |
| `NoAstProduced` | Parse failed and recovery also failed |
| `IoError { path, message }` | File I/O failure |

---

## Summary

After this phase the compiler holds a `StoredDefinition` (the raw AST). All
`def_id`, `scope_id`, and `type_id` fields on every node are `None` — those are
filled in by the resolve and typecheck phases respectively. The AST preserves the
**full syntactic structure** of the source, including nested class hierarchies,
import declarations, extends clauses, and all expression forms.
