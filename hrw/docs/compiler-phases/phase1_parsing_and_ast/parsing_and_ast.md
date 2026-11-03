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

All types live in `crates/rumoca-ir-ast/src/nodes.rs` (re-exported via `lib.rs`).

### StoredDefinition

```rust
pub struct StoredDefinition {
    pub classes: AstIndexMap<String, ClassDef>,
    pub within: Option<Name>,   // from the `within` clause at top of file
}
```

The `within` clause declares the package context (e.g., `within Modelica.Math;`).

### ClassDef

```rust
pub struct ClassDef {
    pub def_id: Option<DefId>,            // set by resolve phase
    pub scope_id: Option<ScopeId>,        // set by resolve phase
    pub name: Token,
    pub class_type: ClassType,
    pub encapsulated: bool,
    pub partial: bool,
    pub expandable: bool,                 // expandable connectors (MLS §9.1.3)
    pub operator_record: bool,            // operator record classes (MLS §14)
    pub pure: bool,                       // function purity (MLS §12.3)
    pub causality: Causality,             // from type alias (e.g., `connector RealInput = input Real`)
    pub description: Vec<Token>,
    pub extends: Vec<Extend>,             // inheritance (MLS §7.1)
    pub imports: Vec<Import>,             // imports (MLS §13.2)
    pub classes: AstIndexMap<String, ClassDef>, // nested class definitions
    pub components: AstIndexMap<String, Component>,
    pub equations: Vec<Equation>,
    pub initial_equations: Vec<Equation>,
    pub algorithms: Vec<Vec<Statement>>,
    pub initial_algorithms: Vec<Vec<Statement>>,
    pub enum_literals: Vec<EnumLiteral>,  // for enumeration types
    pub annotation: Vec<Expression>,      // class-level annotations
    pub is_replaceable: bool,
    pub is_redeclare: bool,
    pub constrainedby: Option<Name>,
    pub array_subscripts: Vec<Subscript>, // for type aliases: `type Vec3 = Real[3]`
    pub external: Option<ExternalFunction>, // MLS §12.9 C/Fortran interface
    // plus additional fields for: is_final, is_inner, is_outer, is_protected,
    // location, class_type_token, purity_declared, end_name_token, and
    // keyword tokens for equation/algorithm sections
}
```

### Component

```rust
pub struct Component {
    pub def_id: Option<DefId>,        // set by resolve phase
    pub type_def_id: Option<DefId>,   // DefId of the type class, set by resolve
    pub name: String,
    pub type_name: Name,
    pub variability: Variability,     // constant | parameter | discrete | (continuous)
    pub causality: Causality,         // input | output | (none)
    pub connection: Connection,       // flow | stream | (none)
    pub description: Vec<Token>,
    pub shape: Vec<usize>,            // literal array dimensions: Real x[3]
    pub shape_expr: Vec<Subscript>,   // parametric dimensions: Real x[n]
    pub modifications: AstIndexMap<String, Expression>,
    pub binding: Option<Expression>,  // explicit: Real x = expr;
    pub start: Expression,
    pub condition: Option<Expression>, // conditional component (MLS §4.4.5)
    pub inner: bool,                  // inner prefix (MLS §5.4)
    pub outer: bool,                  // outer prefix (MLS §5.4)
    pub annotation: Vec<Expression>,
    pub is_replaceable: bool,
    pub is_redeclare: bool,
    pub constrainedby: Option<Name>,
    // plus additional fields for: is_final, is_protected, is_structural,
    // each_modifications, final_attributes, source_modifications, location,
    // name_token, shape_is_modification, start_is_modification, start_has_each,
    // has_explicit_binding
}
```

Key distinction: `shape` holds dimensions that are **already-known integers** at
parse time; `shape_expr` holds dimensions given as expressions (e.g., `Real x[n]`
where `n` is a parameter). The typecheck phase evaluates `shape_expr`.

### Expression

17 variants, all carrying a `Span` field for source location tracking:

| Variant | Example |
|---------|---------|
| `Empty` | (sentinel for optional positions) |
| `Terminal { terminal_type, token }` | `3.14`, `true`, `"hello"` |
| `ComponentReference(ComponentReference)` | `body.v[1]` |
| `Binary { op, lhs, rhs }` | `x + 1` |
| `Unary { op, rhs }` | `-x` |
| `Range { start, step, end }` | `1:10`, `0:0.1:1` |
| `FunctionCall { comp, args }` | `sin(x)`, `der(x)` |
| `ClassModification { target, modifications }` | `i(x = 2)` (in extends/declarations) |
| `NamedArgument { name, value }` | `func(param = value)` |
| `Modification { target, value }` | `x = 2` (in extends/declarations) |
| `If { branches, else_branch }` | `if c then a else b` |
| `Array { elements, is_matrix }` | `{1, 2, 3}`, `[a; b]` |
| `Tuple { elements }` | `(a, b)` (multi-output calls) |
| `Parenthesized { inner }` | `(x + 1)` |
| `ArrayComprehension { expr, indices, filter }` | `{i^2 for i in 1:n}` |
| `ArrayIndex { base, subscripts }` | `a[i]`, `(f())[2]` |
| `FieldAccess { base, field }` | `r.x`, `(f()).field` |

`Arc<Expression>` is used for sub-expressions in hot paths to reduce clone cost.

### Equation

```rust
pub enum Equation {
    Empty,
    Simple { lhs: Expression, rhs: Expression },
    Connect { lhs: ComponentReference, rhs: ComponentReference },
    For { indices: Vec<ForIndex>, equations: Vec<Equation> },
    When(Vec<EquationBlock>),      // one block per when/elsewhen branch
    If { cond_blocks: Vec<EquationBlock>, else_block: Option<Vec<Equation>> },
    FunctionCall { comp: ComponentReference, args: Vec<Expression> },
    Assert { condition: Expression, message: Expression, level: Option<Expression> },
}
```

`EquationBlock` pairs a condition with a list of body equations (used for
`when`/`elsewhen` and `if`/`elseif`).

`ForIndex` is `{ ident: Token, range: Expression }`. Multiple indices give
multi-dimensional iteration: `for i in 1:m, j in 1:n loop ...`.

### Subscript

```rust
pub enum Subscript {
    Empty,
    Expression(Expression),   // concrete: a[3], a[i]
    Range { token: Token },   // wildcard: a[:]
}
```

Used both for array indexing and for dimension declarations.

### Import

Four import forms (MLS §13.2):

| Form | Syntax | Variant |
|------|--------|---------|
| Qualified | `import A.B.C;` | `Qualified` — last component available as `C` |
| Renamed | `import D = A.B.C;` | `Renamed` — available as `D` |
| Unqualified (wildcard) | `import A.B.*;` | `Unqualified` — all children imported |
| Selective | `import A.B.{C, D};` | treated as multiple `Qualified` imports |

### Extend

```rust
pub struct Extend {
    pub base_name: Name,               // unresolved at parse time
    pub base_def_id: Option<DefId>,    // set by resolve phase
    pub location: Location,
    pub modifications: Vec<ExtendModification>,
    pub break_names: Vec<String>,      // selective extension (MLS §7.4)
    pub is_protected: bool,            // protected extends (MLS §7.1.2)
    pub annotation: Vec<Expression>,
}
```

### ComponentReference

```rust
pub struct ComponentReference {
    pub local: bool,                   // leading dot for local lookup
    pub parts: Vec<ComponentRefPart>,  // one per dotted segment: a.b.c → [a, b, c]
    pub span: Span,                    // source location
    pub def_id: Option<DefId>,         // set by resolve for the FIRST part only
}
```

Each `ComponentRefPart` has `ident: Token` and `subs: Option<Vec<Subscript>>`,
allowing `a[1].b[2]` to be represented naturally. In the `Expression` enum,
component references appear as the tuple variant `ComponentReference(ComponentReference)`.

---

## Error Recovery

Recovery is a two-phase strategy (see `crates/rumoca-phase-parse/src/lib.rs`):

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
