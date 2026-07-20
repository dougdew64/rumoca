# Phase 10: Code Generation and Templates

## Overview

Rumoca's code generation is **template-driven**: a Jinja2-compatible template
file receives the full IR (DAE, flat model, AST, or — new in v0.9.x — the
`SolveProblem` from phase 8) as a JSON object and renders it as target source
code. No Rust code is needed to add a new output target — just write a new
template directory and add it to the registry.

- Implementation: `crates/rumoca-phase-codegen/`
- Template engine: [minijinja](https://crates.io/crates/minijinja)

As of v0.9.x, **each backend is a directory under `src/templates/`** containing
a `target.toml` manifest plus one or more `.jinja` files. This replaces the
earlier convention of a single `.jinja` file per backend, and lets a single
target emit multiple coordinated files (e.g. FMI 2.0, which produces a C
implementation plus an XML model description plus a CMake build script plus a
test driver).

A `target.toml` looks like:

```toml
version = 1
ir = "dae"                 # "dae" | "flat" | "ast" | "solve"
name = "casadi-sx"
description = "CasADi SX Python DAE export"
execution_mode = "symbolic"
deployment_class = "symbolic"

[[files]]
path = "{{ model_name }}_casadi_sx.py"
template = "casadi_sx.py.jinja"
```

The `ir` field selects which IR the template receives: traditional `dae` /
`flat` / `ast` for templates that walk the DAE expression trees, or `solve`
for the new tensor-IR backends that consume the `SolveProblem` directly.

---

## Big Picture: Input and Output

```
  Dae IR  or  SolveProblem  +  template directory (target.toml + .jinja files)
        │
        ▼
  ┌─────────────────────────────────────┐
  │     Phase 10: Code Generation       │
  │                                     │
  │  • Serialise IR to JSON             │
  │  • minijinja environment with       │
  │    custom filters/functions         │
  │  • render_expr walks expression     │
  │    trees (DAE path); render_solve   │
  │    walks ComputeNodes (solve path)  │
  │  • ExprConfig parameterises per-    │
  │    target syntax                    │
  │  • Multi-file outputs via target.toml│
  └─────────────────────────────────────┘
        │
        ▼
  Source code (one or more files per target)
  (CasADi, Julia/MTK, JAX, FMI, ONNX, embedded C, CUDA, MLIR, WebGPU, …)
```

---

## The Template Environment

`create_environment()` (in `codegen/mod.rs`) sets up a minijinja
`Environment` with:

- **Strict undefined behavior**: referencing a missing key is an error
  (not silent `None`)
- **Custom filters** (callable as `value | filter_name`)
- **Custom functions** (callable as `function_name(args)`)

### Custom Filters

| Filter | What it does | Example |
|--------|-------------|---------|
| `sanitize` | Replace `.` and special chars with `_` | `"body.v"` → `"body_v"` |
| `product` | Multiply a list of integers | `{{ var.dims \| product }}` → `6` for `[2,3]` |
| `last_segment` | Extract the last dot-separated component | `"A.B.C"` → `"C"` |
| `xml_escape` | Escape `<`, `>`, `&`, `"`, `'` for XML attribute/text content | `"a<b"` → `"a&lt;b"` |
| `xs_double` | Format `f64` as an XML Schema `xs:double` literal | `1.0` → `"1"`, `Inf` → `"INF"` |

### Custom Functions

| Function | Purpose |
|----------|---------|
| `render_expr(expr, cfg)` | Render a JSON expression tree as a string |
| `render_equation(eq, cfg)` | Render one equation |
| `render_statement(stmt, cfg)` | Render one algorithm statement |
| `ode_rhs(dae, cfg)` | Render the full ODE RHS for C templates |
| `alg_rhs_for_var(dae, var, cfg)` | Render the algebraic RHS for a specific variable |
| `fail(message)` | Abort template rendering with an error; used to reject unsupported model classes at render time |

---

## IR Passed to Templates

Templates receive one of three IR forms:

```rust
pub enum CodegenInput<'a> {
    Dae(&'a dae::Dae),       // most common: post-flattening, post-classification
    Flat(&'a flat::Model),   // pre-DAE (flat model, useful for OMC comparison)
    Ast(&'a ast::ClassTree), // pre-flatten (full class hierarchy)
}
```

Most templates use the **DAE** because it contains the complete variable
partition (states, algebraics, etc.) and equation groups (f_x, f_z, f_m, f_c).

### Serialization to JSON

The DAE is converted to a `serde_json::Value` and injected into the template
context as the `dae` variable:

```rust
pub fn dae_template_json(dae: &dae::Dae) -> serde_json::Value {
    let mut value = serde_json::to_value(dae)?;
    // inject enum type names (not directly in Dae struct)
    value.as_object_mut().insert("enum_type_names", ...);
    value
}
```

Inside a template, the DAE is accessed as a plain object:
```jinja
{{ dae.x }}           -- states
{{ dae.y }}           -- algebraics
{{ dae.f_x }}         -- continuous equations
{{ dae.f_z }}         -- discrete Real equations
{{ dae.f_m }}         -- discrete-valued equations
{{ dae.f_c }}         -- condition equations
{{ dae.initial_equations }}
{{ dae.enum_literal_ordinals }}
```

---

## Expression Rendering

### The ExprConfig Object

Because different target languages have different syntax, expression rendering
is parameterized by an `ExprConfig` dictionary passed to `render_expr()`:

```jinja
{% set cfg = {
    "prefix":        "ca.",        -- function call prefix (e.g., ca.sin for CasADi)
    "power":         "**",         -- power operator (Python: **, Julia: ^, C: pow())
    "power_fn":      "ca.power",   -- function-form power (avoids ** on ints in MX)
    "if_style":      "function",   -- "function": if(c,t,e)  or  "ternary": c?t:e
    "mul_elem_fn":   "ca.times",   -- element-wise multiply (for CasADi MX)
    "and_op":        "ca.logic_and", -- logical AND operator or function
    "or_op":         "ca.logic_or",
    "not_op":        "ca.logic_not",
    "size_fn":       "_size",      -- user-provided Modelica size() function
    "sum_fn":        "_sum",       -- user-provided Modelica sum() function
    "python_range":  true,         -- use Python-style range (1-based adjustment)
    "reserved_words": "False,None,True,...",
    "float_literals":  true,         -- emit float32 suffixes on numeric literals (C: 1.0f)
} %}
```

### How Expressions Are Rendered (`codegen/render_expr.rs`)

`render_expr()` walks the JSON expression tree recursively:

```
Binary { op, lhs, rhs }  → "(" + render(lhs) + op_string(op, cfg) + render(rhs) + ")"
Unary { op, rhs }        → op_prefix(op) + render(rhs)
VarRef { name }          → name | sanitize
BuiltinCall { Der, [x] } → "d" + (x | sanitize) + "_dt"
BuiltinCall { Sin, [x] } → cfg.prefix + "sin(" + render(x) + ")"
Literal { Real, v }      → v.to_string()
If { branches, else }    → if_style(cfg, cond, then, else)
```

The `sanitize` filter ensures names like `body.velocity` become valid
identifiers (`body_velocity`) in the target language.

### der() Naming Convention

`der(x)` is rendered as `d{x_sanitized}_dt` in most templates. For a state
named `body.v`, the derivative becomes `dbody_v_dt`.

---

## Built-in Templates

All built-in templates live in `crates/rumoca-phase-codegen/src/templates/`,
one directory per target. Targets are registered via the static
`builtin_targets()` API and can be discovered programmatically with
`builtin_target(name)`.

### Targets that consume the DAE IR

These templates walk the symbolic DAE expression trees via `render_expr`
and produce target-language source. Most users encounter this set.

| Target | Files | Description |
|--------|-------|-------------|
| `casadi-sx` | `casadi_sx.py.jinja` | Python/CasADi scalar symbolics |
| `casadi-mx` | `casadi_mx.py.jinja` | Python/CasADi matrix symbolics |
| `julia-mtk` | `julia_mtk.jl.jinja` | Julia/ModelingToolkit.jl |
| `jax` | `jax.py.jinja` | Python/JAX + Diffrax |
| `sympy` | `sympy.py.jinja` | Python/SymPy |
| `symforce` | `symforce.py.jinja` | SymForce (factor-graph optimisation) |
| `onnx` | `onnx.py.jinja` | ONNX computation graph |
| `embedded-c` | `model.c.jinja`, `model.h.jinja` | Bare-metal C (discrete-only; see below) |
| `fmi2` | `model.c.jinja`, `modelDescription.xml.jinja`, `test_driver.c.jinja`, `CMakeLists.txt.jinja`, `build.sh.jinja` | FMI 2.0 complete package |
| `fmi3` | adds `buildDescription.xml.jinja` | FMI 3.0 complete package |
| `dae-modelica` | `dae_modelica.mo.jinja` | DAE form re-exported as Modelica |
| `flat-modelica` | `flat_modelica.mo.jinja` | Flat model re-exported as Modelica |
| `base-modelica` | `base_modelica.mo.jinja` | "Base Modelica" subset |
| `modelica` | `modelica.mo.jinja` | Full Modelica round-trip |
| `galec` | `model.alg.jinja`, `manifest.xml.jinja`, `__content.xml.jinja` | eFMI Algorithm Code export: eFMU container with GALEC `.alg` + manifest + schemas |
| `galec-production` | `model.alg.jinja`, `model.h.jinja`, `model.c.jinja`, manifests | eFMI Production Code export: eFMU with generated C99 + Algorithm Code representation |
| `embedded-c-galec` | `model.h.jinja`, `model.c.jinja` | GALEC-derived embedded C (startup/recalibrate/dostep block methods); not an eFMU container |

### Targets that consume the SolveProblem IR (new in v0.9.x)

These templates walk the [SolveProblem from phase 8](../phase8_solve_lowering/solve_lowering.md)
— a tensor compute graph — rather than the DAE's symbolic expressions.
They're well-suited to execution-oriented targets (JIT, GPU, MLIR
pipelines) where the high-level `ComputeNode` structure (`MatMul`,
`LinSolve`, `Map`, `AffineStencil`, `ScalarPrograms`) maps directly to
the target's native operations. The target.toml manifest has `ir =
"solve"` for these.

| Target | Files | Description |
|--------|-------|-------------|
| `c-solve` | `model_solve.c.jinja`, `model_solve.h.jinja` | Portable C with hand-rolled residual/Jacobian |
| `casadi-solve` | `casadi_solve.py.jinja` | CasADi structures populated from the compute graph |
| `jax-solve` | `jax_solve.py.jinja` | Python/JAX with explicit residual/Jacobian functions |
| `rust-solve` | `model_solve.rs.jinja` | Standalone Rust harness |
| `cuda-c` | `model_solve.cu.jinja` | CUDA C kernels (AOT) |
| `cuda-nvrtc-solve-jit` | (JIT-only, via `rumoca-exec-mlir`) | CUDA NVRTC JIT pipeline |
| `cranelift-solve-jit` | (JIT-only, via `rumoca-exec-cranelift`) | Cranelift native-code JIT |
| `mlir` | `mlir.mlir.jinja` | MLIR dialect output for further compilation |
| `rust-fixed-solve` | `model_fixed_solve.rs.jinja` | Fixed-size Rust kernel for no-alloc CPU hot paths |
| `wgsl-solve` | `model_solve.wgsl.jinja`, `model_layout.json.jinja` | WebGPU compute shaders |

The JIT targets (`cranelift-solve-jit`, `cuda-nvrtc-solve-jit`) don't
emit source code in the usual sense — their `target.toml` carries
metadata only, and the IR is consumed at runtime by an `rumoca-exec-*`
crate that JIT-compiles and dispatches the compute graph directly.

### CasADi Scalar (casadi-sx)

Generates Python code using CasADi's `SX` symbolic type (scalar symbolic
expressions). Each Modelica variable becomes a CasADi SX symbol.

**Structure**:
1. Preamble helper functions (`pre()`, `Clock()`, `hold()`, etc.)
2. `ExprConfig` for CasADi/Python syntax
3. State/algebraic symbol creation:
   ```python
   t = ca.SX.sym('t')
   {% for name, var in dae.x | items %}
   {{ name | sanitize }} = ca.SX.sym('{{ name }}')
   {% endfor %}
   ```
4. Residual assembly:
   ```python
   {% for eq in dae.f_x %}
   f_x_{{ loop.index0 }} = {{ render_expr(eq.rhs, cfg) }}
   {% endfor %}
   f = ca.vertcat(f_x_0, f_x_1, ...)
   ```
5. `ca.Function` wrapping for export

### CasADi Matrix (casadi_mx.py.jinja)

Similar but uses `MX` (matrix expressions). Arrays stay as vectors rather than
being scalarized. Uses `ca.power()` instead of `**` to avoid CasADi MX issues
with integer exponents.

### Julia/ModelingToolkit (julia_mtk.jl.jinja)

Generates Julia code using the `ModelingToolkit.jl` package. States become
`@variables`, equations use `~` syntax:

```julia
@variables t x(t) v(t)
D = Differential(t)
eqs = [D(x) ~ v, D(v) ~ -9.81]
@named sys = ODESystem(eqs, t)
```

### JAX/Diffrax (jax.py.jinja)

Generates Python/JAX code for differentiable simulation. Uses `jnp` array
operations. Suitable for gradient-based optimization over simulation trajectories.

### SymPy (sympy.py.jinja)

Generates pure symbolic Python using `sympy.symbols`. Useful for analytical
manipulation, simplification, or export to LaTeX.

### ONNX (onnx.py.py.jinja)

Generates an ONNX computation graph for ML interop. Reduced precision (float32).

### Embedded C Header/Implementation

The embedded C backend is split into two files rendered by the dedicated
`export-embedded-c` CLI subcommand (`rumoca export-embedded-c`).

**embedded_c.h.jinja** — dimension macros, struct definition, and prototypes:

```c
typedef struct {
    real_t x[N_X];      // continuous states
    real_t pre_x[N_X];  // pre() values
    real_t y[N_Y];      // algebraics
    real_t u[N_U];      // inputs
    real_t p[N_P];      // parameters
    real_t z[N_Z];      // discrete reals
    real_t pre_z[N_Z];
    real_t m[N_M];      // discrete-valued (bool/int)
    real_t pre_m[N_M];
} ModelName_state_t;
```

**embedded_c_impl.c.jinja** — function bodies and discrete update logic. Always
emits `float` types (no toggle); uses `float_literals = true` in `ExprConfig` so
numeric literals carry an `f` suffix (`1.0f`).

**Constraint**: Only discrete models are supported. If `dae.f_x` is non-empty
(i.e., the model has continuous derivatives), both templates call `fail()` at
render time and abort with an explanatory error. Continuous nonlinear functions
evaluated inside a discrete loop (e.g., EKF propagation) are still permitted
because they appear in `f_z`/`f_m`, not `f_x`.

### FMI 2.0 and 3.0

FMI templates generate three files:
- `fmi2_model_description.xml.jinja` — metadata: variable names, causality,
  variability, types
- `fmi2_model.c.jinja` — full FMI API implementation
- `fmi2_test_driver.c.jinja` — standalone simulation harness that produces CSV

FMI 3.0 variants follow the same pattern for the updated API.

### Modelica Round-trip (dae_modelica.mo.jinja, flat_modelica.mo.jinja)

Re-export the DAE or flat model as Modelica source. Used for debugging and
OpenModelica comparison. The generated file is valid Modelica (modulo
simplifications) that other tools can load.

---

## Array Handling in Templates

Templates that support arrays generate element aliases for array subscript
access. For a 2D array `A[2,3]`:

```jinja
{% macro array_element_aliases(name, var, vec, base_offset) -%}
{%- for i in range(var.dims[0]) -%}
{%- for j in range(var.dims[1]) %}
{{ name | sanitize }}_{{ i + 1 }}_{{ j + 1 }}_ = {{ vec }}[{{ base_offset + i * var.dims[1] + j }}]
{%- endfor -%}
{%- endfor -%}
{%- endmacro %}
```

This maps the flat storage (column-major) back to the Modelica-conventional
1-based subscripts.

Templates that target vectorized languages (Julia, JAX, CasADi MX) can keep
arrays as vectors and avoid this aliasing entirely.

---

## Writing a Custom Template

1. Create a new directory under `crates/rumoca-phase-codegen/src/templates/`
   (or, for out-of-tree templates, just a directory anywhere)
2. Write a `target.toml` manifest naming the files and IR kind (`dae`,
   `flat`, `ast`, or `solve`)
3. Write the `.jinja` template files
4. For DAE-path templates: modify the `cfg` ExprConfig dictionary to
   match the target language's syntax; call `render_expr(expr, cfg)`
   everywhere an expression needs to be emitted
5. Invoke via the Rust API:

```rust
// DAE-path template
let code = render_template_with_name(&dae, template_source, "MyModel")?;

// Solve-path template
let code = render_solve_template_with_name(&solve_problem, template_source, "MyModel")?;
```

The template receives the IR as a JSON object — `dae` (or `flat` / `ast`)
in the DAE-path case, `solve` in the solve-path case.

---

## Rust API

The codegen API has two parallel families of entry points: one for
templates that consume DAE / Flat / AST IRs (the original path), and one
for templates that consume the `SolveProblem` from phase 8 (added in
v0.9.x).

### DAE-path templates

```rust
pub fn render_template(dae: &dae::Dae, template: &str)
    -> Result<String, CodegenError>;

pub fn render_template_with_name(dae: &dae::Dae, template: &str, model_name: &str)
    -> Result<String, CodegenError>;

pub fn render_template_with_dae_json(dae_json: serde_json::Value, template: &str)
    -> Result<String, CodegenError>;

pub fn render_flat_template_with_name(model: &flat::Model, template: &str, model_name: &str)
    -> Result<String, CodegenError>;

pub fn render_ast_template_with_name(tree: &ast::ClassTree, template: &str, model_name: &str)
    -> Result<String, CodegenError>;

// Generic dispatch over CodegenInput::{Dae, Flat, Ast}
pub fn render_template_for_input(input: CodegenInput<'_>, template: &str)
    -> Result<String, CodegenError>;
```

### Solve-path templates (new in v0.9.x)

```rust
pub use codegen::{SolveTemplateRenderer, render_solve_template_with_name};

pub fn render_solve_template_with_name(
    problem: &solve::SolveProblem,
    template: &str,
    model_name: &str,
) -> Result<String, CodegenError>;
```

`SolveTemplateRenderer` is the underlying builder that holds the
`SolveProblem`, the model name, and any per-target configuration; the
free function is a thin wrapper for the common case.

### Built-in target registry

```rust
pub fn builtin_target(name: &str) -> Option<&'static BuiltinTarget>;
pub fn builtin_targets() -> &'static [BuiltinTarget];
pub fn builtin_template_source(target: &str, template: &str) -> Option<&'static str>;
```

`BuiltinTarget` carries the parsed `target.toml` plus an embedded copy of
every `.jinja` file as `&'static str` — the templates are baked into the
crate at compile time so the CLI doesn't need to ship a separate
templates directory.

`CodegenError` propagates template syntax errors, undefined variable
references, and expression rendering failures.

---

## GALEC / eFMI Ecosystem

The `galec`, `galec-production`, and `embedded-c-galec` targets produce
eFMI-conformant artifacts using two supporting crates:

- **`rumoca-ir-galec`** — the GALEC intermediate representation: a typed
  AST for the eFMI Algorithm Code language, with parser, printer, and
  round-trip fidelity tests. The DAE is projected into a GALEC block
  before rendering.
- **`rumoca-galec-codegen`** — code generation utilities for GALEC
  targets: the typed C printer (`c_mangle` / `c_print`), the
  `xml_escape` and `xs_double` canonical implementations, and the
  manifest-authoring logic that produces schema-valid eFMU packaging XML.

The `galec` target emits an eFMU container (directory + `.efmu` zip) with
an Algorithm Code representation; `galec-production` extends this with a
Production Code representation containing generated C99 and a
LogicalData mapping manifest. `embedded-c-galec` produces standalone
GALEC-derived C files without any eFMU packaging. All three share the
same DAE capability gates (discrete-only models with clock partitions)
and the same `xml_escape` / `xs_double` filters registered in the
template environment.

---

## Key Files

| File | Purpose |
|------|---------|
| `rumoca-phase-codegen/src/lib.rs` | Public API (re-exports); `builtin_target` / `builtin_targets` registry |
| `rumoca-phase-codegen/src/codegen/mod.rs` | Template environment setup; IR serialisation; `SolveTemplateRenderer` |
| `rumoca-phase-codegen/src/codegen/render_expr.rs` | DAE expression-tree walker |
| `rumoca-phase-codegen/src/codegen/render_c.rs` | C-specific ODE/algebraic rendering helpers |
| `rumoca-phase-codegen/src/templates/<target>/target.toml` | Per-target manifest: IR kind, file list, metadata |
| `rumoca-phase-codegen/src/templates/<target>/*.jinja` | Per-target template files |
| `rumoca-phase-codegen/src/errors.rs` | `CodegenError` |
| `rumoca-ir-galec/` | GALEC IR: typed AST, parser, printer, round-trip tests |
| `rumoca-galec-codegen/` | GALEC code generation: C printer, `xml_escape`, `xs_double`, manifest authoring |
