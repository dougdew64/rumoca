# Drill-Down: Initial Condition (IC) Plan

*Parent document: [structural_analysis.md](structural_analysis.md)*
*Source: `crates/rumoca-phase-structural/src/ic_plan.rs`*

---

## What Problem Does This Step Solve?

Before time integration starts, the simulator needs **consistent initial
values** for every continuous variable. "Consistent" means that the initial
state $(x_0, y_0)$ satisfies the algebraic constraints of the DAE — every
algebraic equation evaluates to zero — at $t = 0$. If you started a Modelica
simulation with `x = 1, y = 0` but an equation says `y = sqrt(x)`, you
haven't really started: the integrator would be solving a different system
than the one the modeler wrote.

The states $x_0$ are usually given (via `start` / `fixed` attributes in the
Modelica source). The hard problem is the **algebraic variables** $y_0$:
they must be solved from the algebraic equations, given the states. This
sub-problem is itself a nonlinear system that may be high-dimensional and
contain coupled blocks.

The **IC plan** is a precomputed recipe — produced at compile time — telling
the runtime *exactly how* to solve the initial algebraic subsystem: which
variables to compute first, which equations to use, which loops to tear, and
where to fall back to a generic Newton or Levenberg-Marquardt solver.

The plan is built once per model, used every time a simulation starts. It is
small, deterministic, and target-agnostic — the same plan can be executed by
the Rust simulator, an embedded C runtime, or any code generator that wants
to perform consistent initialization.

---

## The Algebraic Subsystem

The DAE arrives at this phase with the equations in
`dae.continuous.equations` reordered so that **ODE rows come first** (equations
with `der(x_i)` for some state $x_i$), followed by **algebraic rows**
(equations with no derivatives). With $n_x$ states, the algebraic equations
occupy indices $n_x \dots n_{eq} - 1$.

The IC plan only operates on the algebraic subsystem. The state derivatives
are not unknowns at $t = 0$; they will be determined by the integrator
implicitly from the residual equations once the initial values are
consistent. So the IC plan needs to:

- Build a fresh incidence over only the algebraic equations
  (`dae.continuous.equations[n_x..]`), with only the algebraic and output
  variables as unknowns.
- Apply matching, BLT, and tearing to that smaller subsystem.
- Convert each BLT block into an `IcBlock` recipe.

If the algebraic subsystem is empty (`n_eq <= n_x`), `build_ic_plan` returns
an empty plan and the runtime is free to use the states' start values
directly.

---

## The `IcBlock` Variants

```rust
pub enum IcBlock {
    ScalarDirect {
        var_idx:        usize,
        var_name:       String,
        solution_expr:  dae::Expression,
    },
    ScalarNewton {
        eq_idx:    usize,
        var_idx:   usize,
        var_name:  String,
    },
    TornBlock {
        tear_var_indices:    Vec<usize>,
        tear_var_names:      Vec<String>,
        causal_sequence:     Vec<CausalStep>,
        residual_eq_indices: Vec<usize>,
    },
    CoupledLM {
        eq_indices:  Vec<usize>,
        var_indices: Vec<usize>,
        var_names:   Vec<String>,
    },
}
```

The four variants form a fallback ladder, listed in order of decreasing
preference:

1. **`ScalarDirect`** — best case. The single equation could be solved
   *symbolically* for the unknown, yielding an explicit `solution_expr`. At
   runtime the simulator just evaluates that expression.
2. **`ScalarNewton`** — the equation is a single nonlinear scalar in one
   unknown. The runtime starts a Newton iteration on `f_x[eq_idx]` for the
   single variable `var_name`.
3. **`TornBlock`** — an algebraic loop that admits useful tearing. The
   runtime iterates Newton/LM on the `tear_vars` while evaluating the
   `causal_sequence` per outer step.
4. **`CoupledLM`** — fallback for loops that didn't tear. The runtime runs
   Levenberg-Marquardt over all variables and all equations in the block.

Within `TornBlock`, the `causal_sequence` is a list of `CausalStep`:

```rust
pub struct CausalStep {
    pub var_idx:       usize,
    pub var_name:      String,
    pub solution_expr: Option<dae::Expression>,    // None ⇒ use scalar Newton on eq_idx
    pub eq_idx:        usize,
}
```

Each step says "obtain `var_name` from `eq_idx`". If `solution_expr` is
`Some`, evaluate the symbolic form; if `None`, use a scalar Newton on the
equation. This is the same direct-vs-Newton distinction as the top-level
scalar blocks, applied per causal step inside a torn loop.

---

## Construction Pipeline

`build_ic_plan(dae, n_x)` performs five steps:

```
1. build_var_index_maps(dae)
        → name → solver-y-vector index (for the runtime)

2. build_algebraic_incidence(dae, n_x, var_name_to_idx)
        → fresh Incidence over algebraic eqs and algebraic+output vars only

3. matching::maximum_matching(...)
        → match each algebraic equation to one algebraic/output unknown

4. If matching imperfect:
       try_build_relaxed_ic_plan_for_singular(...)   ← see "Relaxed IC fallback"
       else return Err(StructuralError::Singular { ... })

5. build_blt_from_incidence(&incidence)
        → BLT blocks for the algebraic subsystem

6. convert_blt_blocks_to_ic(...)
        → translate each BLT block to an IcBlock
```

The matching/BLT/tearing machinery is reused unchanged from the main
structural-analysis pipeline; only the *inputs* are different (algebraic
subsystem only).

### Step 1: variable index maps

`build_var_index_maps` walks `dae.variables.states`, `dae.variables.algebraics`,
and `dae.variables.outputs`, in that order, and assigns each scalar a contiguous index in
the **solver y-vector**. Array variables are scalarised: `u[2]` becomes two
entries `u[1]` and `u[2]` at consecutive indices. The result is a forward
map `name → idx` and a reverse list `idx → name`.

The y-vector layout matters because the IC blocks store `var_idx` so the
runtime can write directly into the right slot.

### Step 2: algebraic-only incidence

`build_algebraic_incidence` constructs a separate `Incidence` for the
algebraic subsystem:

- **Equations** = `dae.continuous.equations[n_x..]`, indexed locally as
  `0..n_alg_eq`. The function tracks `alg_eq_offset = n_x` so it can later
  translate local indices back to global equation indices.
- **Unknowns** = scalar entries from `dae.algebraics` followed by
  `dae.outputs`. State variables are *not* unknowns for IC: their values
  come from `start` / `fixed` attributes.
- The walker uses `collect_expression_unknowns` (the same function the
  general incidence builder uses) but with a local resolver that maps only
  the algebraic and output names. References to state variables in
  algebraic equations resolve to nothing — they are treated as known
  constants at $t = 0$.

The output `(incidence, alg_eq_offset, alg_var_indices, alg_var_names)`
contains everything the rest of the pipeline needs to translate between
local and global indices and names.

### Step 3: matching

The same `maximum_matching` used for the general pipeline runs over the
algebraic-only incidence. If the match is perfect, we proceed; otherwise the
relaxed fallback (next subsection) is attempted.

### Steps 4–6: BLT and conversion

`build_blt_from_incidence` runs Tarjan and produces the BLT block list for
the algebraic subsystem. `convert_blt_blocks_to_ic` then runs each block
and decides which `IcBlock` variant fits.

---

## Scalar Block Conversion: Symbolic First, Newton as Fallback

For each `BltBlock::Scalar { equation, unknown }`:

```rust
let var_vn = VarName::new(&var_name);
match try_solve_for_unknown(&dae.continuous.equations[eq_idx].rhs, &var_vn) {
    Some(solution) if !expr_contains_var(&solution, &var_vn) => {
        ic_blocks.push(IcBlock::ScalarDirect { var_idx, var_name, solution_expr: solution });
    }
    _ => {
        ic_blocks.push(IcBlock::ScalarNewton { eq_idx, var_idx, var_name });
    }
}
```

`try_solve_for_unknown` is a symbolic-algebra helper from
`rumoca-phase-structural/src/eliminate/`. Given an expression `rhs` (interpreted
as `0 = rhs`) and a target variable `v`, it tries to algebraically rearrange
to `v = expr` and returns the solution expression if it succeeds. The
guard `!expr_contains_var(&solution, &var_vn)` rejects solutions where the
target variable still appears on the right (e.g., a transcendental equation
the solver couldn't actually isolate).

If a clean solution exists, it becomes a `ScalarDirect`; otherwise the
runtime is told to do a scalar Newton on the original equation. The scalar
Newton path always works (the matching guarantees the equation references
the variable), so this is a safe fallback.

---

## Algebraic-Loop Conversion: Try Tearing, Fall Back to LM

For each `BltBlock::AlgebraicLoop { equations, unknowns }`,
`build_loop_block` is called.

```rust
fn build_loop_block(dae, eq_indices, var_info, var_name_to_idx) -> IcBlock {
    // 1. Build local incidence over just this loop's equations and unknowns.
    // 2. Call tear_algebraic_loop(n, &local_eq_unknowns).
    // 3. If tearing succeeds, build a TornBlock; else build a CoupledLM.
}
```

### Local incidence

The loop has its own coordinate system: equations and unknowns are indexed
$0 \dots N - 1$ within the block, separate from the global y-vector or the
algebraic subsystem indexing. `build_loop_block` builds a fresh
`ScalarUnknownResolver` over the loop's unknowns and runs each equation's
expression to fill `local_eq_unknowns: Vec<HashSet<usize>>`.

### Tearing call

`tear_algebraic_loop(n, &local_eq_unknowns)` is the same function described
in [tearing.md](tearing.md). It returns `Some(TearingResult)` if the loop
admits useful tearing, `None` otherwise.

### TornBlock construction

When tearing succeeds, the result's local indices need to be translated to
global y-vector indices (for variables) and global equation indices (for
equations). `var_info[local]` and `eq_indices[local]` provide those
mappings.

For each entry in `causal_sequence`, the IC builder also tries
`try_solve_for_unknown` to see if the causal step admits a symbolic
solution. If yes, the `CausalStep`'s `solution_expr = Some(...)`; if no,
the runtime will use a scalar Newton on `eq_idx`. This is the same direct-
vs-Newton choice as for top-level scalar blocks, applied per causal step.

### `improve_causal_assignment` post-pass

Tearing makes its assignments based purely on **structural** incidence — it
sees only which variables appear in which equations, not what the
expressions look like. This can produce poor causal assignments. Consider:

```
eq A:   v = R * i               (bilinear: v, R, i appear coupled)
eq B:   R = R0 * (1 + alpha*(T - Tref))
                                (linear in R, easy to symbolically solve)
```

Structurally both equations might be candidates for solving for $R$. If
tearing picks eq A as the causal step for $R$, the runtime cannot solve `v = R*i`
for $R$ symbolically (it doesn't know whether $i$ is zero) and falls back to
Newton — slow and brittle. Eq B would have given a clean symbolic solution.

`improve_causal_assignment` runs each causal step that lacks a symbolic
solution and checks whether any **residual equation** references the same
variable and admits a symbolic solve for it. If so, the two equations are
swapped: the residual becomes the new causal equation (with its symbolic
solution) and the old causal becomes a residual instead.

This swap doesn't change the structural decomposition — same tear variables,
same number of residuals, same causal sequence length — but it makes the
causal sequence symbolically solvable wherever possible, dramatically
improving runtime robustness.

### CoupledLM fallback

If tearing returns `None`, the runtime is told to do a Levenberg-Marquardt
solve over the full block. This is the slowest path but always available;
it's the safety net for blocks that the structural heuristics can't simplify.

---

## Relaxed IC Fallback: Square but Singular Subsystems

A common Modelica modelling mistake produces a system that is **square** ($n$
equations, $n$ unknowns) but **structurally singular** (no perfect matching).
For example, two current sources writing the same current value into the same
node:

```
eq 1:    i - 1 = 0     (source A says i = 1)
eq 2:    i - 2 = 0     (source B says i = 2)
eq 3:    v_1 - v_2 = 0  (some other constraint)
```

There are three equations and three unknowns ($i, v_1, v_2$), but two of the
equations both fight over $i$, leaving $v_1$ and $v_2$ paired with one
remaining equation. Maximum matching produces size 2, not 3.

`try_build_relaxed_ic_plan_for_singular` attempts to **balance** such cases by
dropping the same number of redundant equations and unknowns. It is gated by
several conservative guards before any drop is attempted:

```rust
if incidence.n_eq != incidence.n_var
    || unmatched_equation_indices.is_empty()
    || unmatched_equation_indices.len() != unmatched_unknown_indices.len()
    || unmatched_equation_indices.len() > 32
{
    return None;
}
```

(The 32-equation cap is a safety net against combinatorial blow-up — the
selection has to consider many candidates per drop.)

When the system passes the guards, the algorithm:

1. **Builds a candidate row pool** ordered by likelihood of being a useful
   drop: unmatched equations first, then equations referencing dropped
   unknowns, then everything else. (`build_relaxed_candidate_rows`)
2. **Scores** each candidate by simulating its drop and re-running matching
   on the reduced incidence. The score measures how many additional
   equations get matched after the drop, plus extra credit for "preferred"
   drops that touch the originally-singular unknown set.
   (`score_relaxed_drop_candidate`)
3. **Tiers** candidates by which of several preference categories they fall
   into (full match touching the dropped unknown is best; full match
   regardless is next; partial-match "touching" rows next; etc.). The tier
   logic in `selection_tier_for_relaxed_drop` is defensive against various
   degenerate cases that arise in practice.
4. **Selects** drops greedily: pick the highest-tiered candidate, add it to
   the dropped set, repeat until the target number of drops is met.
   (`select_relaxed_drop_rows`)
5. **Optionally realigns the dropped unknown** if the chosen dropped row
   doesn't actually reference the originally-unmatched unknown — in that
   case `maybe_realign_single_relaxed_drop_unknown` checks whether dropping
   one of the row's *own* unknowns produces a fully-matched system, and if
   so swaps the drop target. This handles cases where the matching's view
   of "unmatched" doesn't align with the structural reality.
6. **Validates** the reduction by running matching on the reduced incidence
   one more time; if not fully matched, the relaxation is rejected.
7. If accepted, runs BLT and `convert_blt_blocks_to_ic` on the reduced
   incidence and returns the plan.

The dropped equation is essentially declared redundant ("the modeler told us
$i = 1$ *and* $i = 2$; we'll honour one and warn about the other"). The
dropped unknown is left to be **pinned** by some other initialisation
mechanism — typically its `start` attribute.

When the relaxed fallback succeeds, `build_ic_plan` returns the reduced plan
and the simulation proceeds. When it fails (or the gate is closed), the
`StructuralError::Singular` error fires with full diagnostic information:
which equations and unknowns were unmatched, by name, so the modeler can
locate the source bug.

A separate `build_ic_relaxation_hint` API exists for tools (e.g.
`rumoca-tool-dev`) to *report* the relaxation drop set without actually
applying it — useful for surfacing the diagnostic to a user before they
commit to the relaxed solve.

### Trace mode

Throughout the relaxation logic there are calls like

```rust
if ic_plan_trace_enabled() {
    eprintln!("[sim-trace] IC relaxed drop select: row_local={} ...");
}
```

Setting `RUMOCA_SIM_TRACE` or `RUMOCA_SIM_INTROSPECT` in the environment
enables verbose diagnostics about which rows were considered, scored, and
selected. This is invaluable when debugging unexpected IC behaviour on
real models.

---

## Where the Plan Plugs In

The output of `build_ic_plan` is re-exported through `rumoca-compile` and consumed
during **phase 8 (solve lowering)**, where IC blocks are lowered into the
`SolveProblem`'s initialization partition (see `rumoca-phase-solve`). At runtime,
the solver uses the lowered initialization data via `rumoca-solver-diffsol`'s
`init_projection` module (which operates on the already-lowered `SolveModel`).
See [the simulation document](../phase9_simulation/simulation.md#initial-condition-ic-solving).
At simulation startup, the runtime:

1. Seeds the y-vector with `start` values from `dae.variables.states`,
   `dae.variables.algebraics`, and `dae.variables.outputs`.
2. Walks the `IcBlock` list in order, executing each block:
   - `ScalarDirect` → evaluate `solution_expr`, write to `y[var_idx]`.
   - `ScalarNewton` → run scalar Newton on `f_x[eq_idx]` for `y[var_idx]`.
   - `TornBlock` → outer LM iteration on `y[tear_var_indices]`; inner causal
     sequence per LM step; convergence on residuals of `f_x[residual_eq_indices]`.
   - `CoupledLM` → full LM over `y[var_indices]` against `f_x[eq_indices]`.

After the block list executes, every algebraic and output variable holds a
value consistent with the algebraic constraints. The integrator then takes
over with consistent $(t, y, \dot y)$.

---

## Tests

Test coverage in `ic_plan.rs` exercises every variant:

- **Empty plan** (`test_build_ic_plan_no_algebraics`) — no algebraic
  equations; returns an empty plan.
- **Scalar chain** (`test_build_ic_plan_scalar_chain`) — two ScalarDirect
  blocks, one consuming the other, in dependency order.
- **2×2 loop** (`test_build_ic_plan_algebraic_loop`) — produces either a
  `TornBlock` or `CoupledLM` depending on whether tearing succeeds.
- **Scalarised arrays** (`test_build_ic_plan_handles_scalarized_array_reference_forms`)
  — exercises mixed `u[1]`, `u[2]`, and whole-array `u` references in
  algebraic equations.
- **Rectangular subsystem** (`test_build_ic_plan_flags_rectangular_algebraic_subsystem`)
  — non-square algebraic system reports `StructuralError::Singular` with
  the unmatched unknown by name.
- **Square singular relaxation** (`test_build_ic_plan_relaxes_square_singular_subsystem`
  and `test_build_ic_relaxation_hint_reports_drop_set_for_square_singular_subsystem`)
  — exercises the relaxed-drop fallback on a system with two conflicting
  current equations.

---

## Summary

- The IC plan is a compile-time recipe for solving the algebraic subsystem
  at $t = 0$, producing consistent initial values for algebraic and output
  variables before the integrator starts.
- Construction reuses matching, BLT, and tearing -- the same machinery as
  the main pipeline -- but applied to the algebraic-only subsystem
  (`dae.continuous.equations[n_x..]` against algebraic and output unknowns
  only).
- BLT blocks are translated into `IcBlock`s along a fallback ladder:
  symbolic direct → scalar Newton → torn loop → coupled LM. Symbolic
  solutions are tried at the top level *and* per causal step inside a
  TornBlock.
- A post-pass (`improve_causal_assignment`) swaps equations between causal
  steps and residuals when a residual equation provides a cleaner symbolic
  solution for a causal variable. Tearing alone uses only structural
  incidence; this pass adds expression-level intelligence.
- Square but singular algebraic subsystems are handled by a relaxed
  fallback that drops a balanced set of redundant equations and unmatched
  unknowns, gated by conservative guards and verified by re-running
  matching on the reduced incidence. When the relaxation succeeds the
  reduced plan is used; when it fails, a structural error is reported.
- The output is a `Vec<IcBlock>` consumed by the simulator at startup;
  the same plan format is target-agnostic and can be executed by any
  runtime that can evaluate expressions and run scalar Newton/LM.
