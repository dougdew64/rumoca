# Phase 7: Structural Analysis

## Overview

The structural analysis phase takes the `Dae` and determines a **solution order**
for the continuous equations. This makes it possible to evaluate the system
sequentially rather than solving everything simultaneously.

- Implementation: `crates/rumoca-phase-structural/`
- Primary entry point: `pub fn sort_dae(dae: &Dae) -> Result<SortedDae, StructuralError>`
- Diagnostic entry point: `pub fn analyze_structure(dae: &Dae) -> StructuralDiagnostics`
- Report entry point: `pub fn build_structural_report(dae: &Dae) -> Result<StructuralReport, StructuralError>`

`sort_dae` transforms the DAE into BLT-sorted block form for sequential
simulation (errors on singular systems). `analyze_structure` performs
diagnostic-only analysis for CasADi workflows where BLT ordering is not
required. `build_structural_report` produces a named structural report with
matching, BLT blocks, and tearing details, backing the `rumoca sim --structure`
debug dump.

Two additional public APIs allow building BLT blocks from arbitrary incidence
matrices rather than a full DAE:

- `pub fn build_blt_from_incidence(incidence: &Incidence) -> Result<Vec<BltBlock>, StructuralError>` -- wraps maximum matching, dependency graph construction, Tarjan SCC, and BLT block assembly. Used by the IC solver to decompose arbitrary subsystems (e.g. algebraic-only).
- `pub fn maximum_regular_subsystem(incidence: &Incidence) -> Result<RegularSubsystem, StructuralError>` -- selects the largest structurally regular square subsystem supported by the incidence matrix's maximum matching. Useful when initialization projection contains redundant rows and unconstrained variables.

The key results are:
1. A **maximum matching** pairing each equation to one unknown
2. **BLT blocks** giving a sequenced evaluation order
3. **Tearing** for algebraic loops that cannot be decoupled
4. An **IC plan** for consistent initialization

---

## Big Picture: Input and Output

```
  Dae  (index-1, from phase 6)
        │
        ▼
  ┌─────────────────────────────────────┐
  │     Phase 7: Structural Analysis    │
  │                                     │
  │  • Build incidence matrix           │
  │  • Maximum bipartite matching       │
  │    (Kuhn's augmenting paths)        │
  │  • Tarjan SCCs over dep. graph      │
  │  • BLT block packaging              │
  │  • Cellier tearing of loops         │
  │  • IC plan for t=0 initialization   │
  └─────────────────────────────────────┘
        │
        ▼
  SortedDae  (BLT blocks + IC plan + matching)
```

---

## The Incidence Matrix

The incidence matrix captures which unknowns appear in which continuous
equations. It is built only over `dae.continuous.equations` (discrete update
equations are handled separately by event settling) and is stored sparsely as
`Vec<HashSet<usize>>` — one set per equation, listing the column indices of
the unknowns it references.

The columns are the unknowns the solver must determine at each step: state
*derivatives* `der(x)` (not the states themselves, which are integrator inputs
known at each step), algebraic variables, and output variables. They are laid
out in three contiguous groups in that order, matching the layout of the
solver's state vector and Jacobian.

The construction lives in
`crates/rumoca-phase-structural/src/incidence.rs` and walks
each residual expression to collect referenced unknowns. The most important
subtlety is that the walker **does not descend into the argument of `der()`** —
inside `der(x)`, the symbol `x` is a *known* state value at this time step, not
an unknown column. The `der(x)` dependency is recorded through a separate
collection path that maps state names to their `DerState` columns.

A full walk-through — concept-level motivation, the three-map array-aware name
resolver, line-by-line treatment of `collect_equation_unknowns`, the
`der()`-argument subtlety with its supporting test, the dependency-graph
construction that feeds Tarjan, and a worked two-mass-spring example — is in
the drill-down document:

→ [Drill-down: The Incidence Matrix](incidence_matrix.md)

---

## Maximum Bipartite Matching

The matching step pairs each continuous equation with exactly one of the
unknowns it references, producing two inverse maps `match_eq[i] = j` and
`match_var[j] = i`. This pairing is what later steps use to say "equation A
depends on equation B" — A references variable `v`, and B is the equation
matched to (i.e., responsible for) `v`.

Rumoca implements **Kuhn's augmenting-path algorithm** in
`crates/rumoca-phase-structural/src/matching.rs`. The outer
loop walks equations in index order and, for each one, runs a depth-first
search to find an *augmenting path* — a chain of displacements that ends at a
free unknown. Toggling the matched/unmatched status of every edge along the
path grows the matching by exactly one.

A subtlety worth flagging: the per-equation candidate list is sorted by index
before iteration. The incidence is stored as `Vec<HashSet<usize>>` and Rust's
default hasher uses a randomised seed per process, so unsorted iteration would
yield a non-deterministic matching — and any change in the matching cascades
into different BLT order, different generated code, and unstable golden
tests.

If maximum matching is imperfect, the system is **structurally singular**
(over- or under-determined); the caller reports the unmatched equations and
unknowns by name so the modeler can locate the problem.

A full walk-through — bipartite-graph framing, the augmenting-path theorem,
line-by-line annotation of `augment()`, and a worked example showing the
recursion stack as an augmenting path — is in the drill-down document:

→ [Drill-down: Maximum Bipartite Matching](maximum_bipartite_matching.md)

---

## Tarjan's Strongly-Connected-Components Algorithm

After matching, the next question is *what order* to evaluate equations in.
Most pairs are independent — equation A produces a variable that equation B
consumes, so A goes first — but sometimes the dependencies form a cycle: A
needs B, B needs C, C needs A. Such a cycle is an **algebraic loop**: those
equations cannot be ordered relative to each other; they must be solved
jointly.

Detecting cycles is the job of a strongly-connected-components (SCC)
algorithm. Rumoca uses **Tarjan's algorithm** in
`crates/rumoca-phase-structural/src/tarjan.rs`. It runs in
$O(V + E)$ with a single depth-first pass, using two per-node integers
(`index` for DFS discovery time and `lowlink` for the oldest reachable
ancestor on the DFS stack) plus a separate SCC stack.

The dependency graph is built from the matching before running Tarjan: there
is an edge $A \to B$ iff equation $A$ references a variable that equation
$B$ is matched to. That is, **edges point from consumer to producer** — A
depends on B. With this convention, Tarjan's emission order (reverse
topological order of the condensation DAG) places producers first and
consumers last, which is already the BLT evaluation order. No reversal is
needed.

A full walk-through — index/lowlink/stack mechanics, why "on stack" is
distinct from "visited", the SCC-root condition, the directionality argument,
a worked four-node example, and complexity analysis — is in the drill-down:

→ [Drill-down: Tarjan's Strongly-Connected-Components Algorithm](tarjan_scc.md)

---

## BLT (Block-Lower-Triangular) Form

BLT block construction is the contract between structural analysis and
everything downstream (code generation, simulation, IC solving). Each SCC
emitted by Tarjan becomes one `BltBlock` — `Scalar` for size-1 SCCs (one
equation, one unknown, evaluable directly) or `AlgebraicLoop` for size-$N$
SCCs with $N > 1$ (a coupled set requiring joint solution). The blocks are
emitted in the order Tarjan returned them, which is already the correct
evaluation order (dependencies first).

The name "block lower triangular" comes from the matrix view: arrange the
matched incidence matrix so that variable $v_i$ is column $i$ and its matched
equation $e_i$ is row $i$. Without algebraic loops, all off-diagonal nonzeros
sit below the diagonal — pure lower-triangular, solvable by back-substitution.
With loops, the loop equations form **diagonal blocks** of size $> 1$ that
cannot be triangularised; everything outside those blocks remains lower
triangular.

The construction in
`crates/rumoca-phase-structural/src/blt.rs` is a thin wrapper over
Tarjan: it calls `tarjan_scc`, then walks each SCC and converts it into the
appropriate `BltBlock` variant by looking up the `EquationRef`s and matched
`UnknownId`s from the incidence.

A full walk-through — the matrix view of BLT, why no reordering of Tarjan's
output is needed, the `scc_to_block` conversion logic, and a worked
mixed-scalar-plus-loop example — is in the drill-down:

→ [Drill-down: BLT (Block-Lower-Triangular) Form](blt.md)

---

## Tearing of Algebraic Loops

A naive solve of an $N$-equation algebraic loop costs roughly $O(N^3)$ per
Newton step (from the $N \times N$ Jacobian factorisation). **Tearing**
reduces this by identifying a small subset of $K$ "tear" variables that, if
treated as known, allow the remaining $N - K$ equations to be solved one at a
time — *causally*, with no iteration. Only the $K$ tear variables are
iterated, with the leftover $K$ equations' residuals driving the iteration.
For loops where $K = 1$, the per-step cost drops dramatically.

Rumoca implements **greedy Cellier tearing** in
`crates/rumoca-phase-structural/src/tearing.rs`. The algorithm
alternates two phases until every equation is accounted for: **causal
resolution** (find equations with exactly one remaining unknown and append
them to the causal sequence; break ties by preferring the equation with the
fewest total unknowns and lowest index) and **tear-variable selection**
(when causal resolution stalls, declare the most-frequently-referenced
remaining unknown a tear variable and resume). Importantly, tearing a
variable does *not* remove the equations that mention it — only the variable
leaves the unknown set; those equations may then become causal candidates in
the next sweep, or end up as residuals.

The function returns `None` when no useful reduction is achievable (every
unknown appears in every equation, or the algorithm makes no progress). The
caller falls back to coupled Levenberg-Marquardt over the whole block.

A full walk-through — the Cellier idea explained from first principles, the
two helper functions (`resolve_causal_equations`, `count_var_appearances`),
why `BTreeSet`/`BTreeMap` are used for determinism, why equations stay in
`remaining_eqs` after tearing, the post-condition checks, and a worked 3×3
loop example showing the full alternation — is in the drill-down:

→ [Drill-down: Tearing of Algebraic Loops](tearing.md)

---

## Initial Condition (IC) Plan

Before time integration starts, the simulator needs **consistent initial
values** for every continuous variable: the algebraic equations must already
hold at $t = 0$. The states $x_0$ usually come from `start`/`fixed`
attributes, but the algebraic and output variables $y_0$ must be solved from
the algebraic subset of the equations. The IC plan is a precomputed recipe —
generated once at compile time — telling the runtime exactly how to do this:
which variables to compute first, which equations to use, which loops to
tear, and where to fall back to a generic Newton or Levenberg-Marquardt
solve.

The construction in
`crates/rumoca-phase-structural/src/ic_plan.rs` reuses the
matching, BLT, and tearing machinery, but applied to the **algebraic-only
subsystem** (the algebraic equations from `dae.continuous.equations` against
algebraic and output variables only -- states are treated as known constants
at $t = 0$). Each BLT block is translated into an `IcBlock` variant along a
fallback ladder:

- **`ScalarDirect`** when the equation can be symbolically rearranged for
  the unknown (cheapest — pure expression evaluation).
- **`ScalarNewton`** when no symbolic solution exists (single-variable
  iterative solve).
- **`TornBlock`** for algebraic loops where tearing succeeds (small
  iteration on tear variables + a causal sequence of inner solves).
- **`CoupledLM`** for loops where tearing fails (full Levenberg-Marquardt
  over the block — slowest but always available).

A subtle but important post-pass (`improve_causal_assignment`) inspects each
causal step inside a `TornBlock` that lacks a symbolic solution and checks
whether any *residual* equation provides a cleaner symbolic solve for the
same variable; if so, the two equations are swapped. Tearing alone uses only
structural incidence and cannot see this — the post-pass adds expression-
level intelligence on top.

A separate **relaxed-IC fallback** handles the common modelling mistake of a
*square but structurally singular* algebraic subsystem (e.g., two current
sources both constraining the same node current). When matching fails on a
square system, the fallback tries to drop a balanced subset of redundant
equations and unmatched unknowns to recover a solvable subsystem; the dropped
unknowns are pinned by `start` attributes.

A full walk-through — the algebraic-only incidence build, the y-vector
indexing scheme, the four IcBlock variants in detail, the
`improve_causal_assignment` post-pass with motivating example, the relaxed-
fallback selection algorithm with its tier system and trace mode, and how
the plan plugs into the simulator at startup — is in the drill-down:

→ [Drill-down: Initial Condition (IC) Plan](ic_plan.md)

---

## SortedDae Output

```rust
pub struct SortedDae<'a> {
    /// Reference to the original DAE.
    pub dae: &'a dae::Dae,
    /// BLT blocks in evaluation order.
    pub blocks: Vec<BltBlock>,
    /// Full matching: each pair `(equation, unknown)` from the maximum matching.
    pub matching: Vec<(EquationRef, UnknownId)>,
    /// Diagnostic warnings (e.g. algebraic loop notifications).
    pub diagnostics: Vec<Diagnostic>,
}
```

Diagnostics include information about detected algebraic loops so the user can
be informed which variables are coupled and may benefit from tearing.

## StructuralReport Output

`build_structural_report` produces a `StructuralReport` containing named
matching pairs, BLT blocks (as `BlockReport` variants: `Scalar` or `Coupled`),
and tearing details for each coupled block. This report is displayed by
`rumoca sim --structure` and captures the full structural decomposition in a
human-readable form.

```rust
pub struct StructuralReport {
    pub n_equations: usize,
    pub n_unknowns: usize,
    pub matching: Vec<(String, String)>,  // (equation label, unknown name)
    pub blocks: Vec<BlockReport>,         // Scalar or Coupled with tearing
}
```

---

## Complete Example: RC Circuit

```modelica
model RC
  Real u;         // source voltage
  Real u_C;       // capacitor voltage
  parameter Real R = 1.0, C = 1.0;
equation
  u = sin(time);
  R * (u - u_C) / R = C * der(u_C);
end RC;
```

**After DAE construction:**
```
states:     u_C
algebraics: u
continuous.equations:
  eq1: 0 = sin(time) - u              (algebraic: u = sin(t))
  eq2: 0 = (u - u_C)/R - C*der(u_C)  (ODE: der(u_C))
```

**Incidence matrix** (rows = equations, cols = [der(u_C), u]):
```
      der(u_C)  u
eq1:      0     1
eq2:      1     1
```

**Matching:** eq1 → u, eq2 → der(u_C)  (perfect matching)

**Dependency graph:** eq2 references u (matched to eq1), so eq2 → eq1.

**Tarjan:** {eq1}, {eq2} — no cycles.

**BLT order:** [eq1, eq2] — evaluate u first, then integrate u_C.

---

## Key Files

### Core pipeline

| File | Purpose |
|------|---------|
| `rumoca-phase-structural/src/lib.rs` | Entry points `sort_dae()`, `analyze_structure()`, `build_structural_report()`, `build_blt_from_incidence()`, `maximum_regular_subsystem()`; orchestration |
| `rumoca-phase-structural/src/types.rs` | `SortedDae`, `BltBlock`, `EquationRef`, `UnknownId`, `StructuralError` type definitions |
| `rumoca-phase-structural/src/incidence.rs` | Incidence matrix construction and dependency graph building |
| `rumoca-phase-structural/src/matching.rs` | Augmenting-path maximum matching (Kuhn's algorithm) |
| `rumoca-phase-structural/src/tarjan.rs` | Tarjan SCC algorithm |
| `rumoca-phase-structural/src/blt.rs` | BLT block assembly from Tarjan output |
| `rumoca-phase-structural/src/tearing.rs` | Greedy Cellier tearing of algebraic loops |
| `rumoca-phase-structural/src/ic_plan.rs` | Initial condition plan construction |

### Supporting modules

| File / directory | Purpose |
|------|---------|
| `rumoca-phase-structural/src/eliminate/` | Algebraic elimination and substitution framework (boundary resolution, BLT scalar-block elimination, solve-for-unknown, substitution application) |
| `rumoca-phase-structural/src/scalarize/` | Array-to-scalar equation expansion (scalarization of vector/matrix equations) |
| `rumoca-phase-structural/src/dae_prepare/` | DAE preparation: alias demotion, dummy-state reduction, structural preprocessing |
| `rumoca-phase-structural/src/projection_maps.rs` | Projection map utilities for scalarized component-array and function-output references |
| `rumoca-phase-structural/src/report.rs` | `StructuralReport`, `BlockReport`, `TearingReport` for the `--structure` debug dump |
| `rumoca-phase-structural/src/runtime_defined.rs` | Enumerates unknowns defined at event/clock runtime rather than by the continuous solver |
| `rumoca-phase-structural/src/variable_scope.rs` | Variable scope and shape resolution utilities |
| `rumoca-phase-structural/src/diagnostics.rs` | Diagnostic collection for structural warnings (singularity, algebraic loops) |
