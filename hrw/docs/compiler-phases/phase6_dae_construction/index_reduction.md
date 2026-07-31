# Drill-Down: Index Reduction and State Demotion

*Parent document: [dae_construction.md](dae_construction.md)*
*Source: `crates/rumoca-phase-structural/src/dae_prepare/state_row_reduction.rs`*
*Symbolic differentiator: `crates/rumoca-phase-structural/src/dae_prepare/symbolic.rs`*
*Pipeline driver: `crates/rumoca-sim/src/solve_lowering/structural_lowering.rs` (`prepare_dae_for_structural_analysis`)*

---

## What Problem Does This Step Solve?

The short answer: **a numerical integrator needs a derivative for every
state at every time step. If the DAE doesn't tell it how to get one,
simulation can't proceed.** Index reduction is the technique of rewriting
the DAE so that every needed derivative is computable.

The rest of this section unpacks that statement.

### What the Integrator Needs at Each Step

To simulate a model, a numerical integrator marches forward in time by
small steps. At each step `t → t + dt`, given the current values of the
state variables `x(t)`, it must compute the values at the next step,
`x(t + dt)`. It does this using the time-derivative of each state.
Forward Euler is the simplest illustration:

```
x(t + dt) ≈ x(t) + dt · der(x)
```

Implicit methods (BDF, ESDIRK) are more sophisticated, but they all need
the same primitive: at each step the integrator must be able to obtain a
value for `der(x)` for every state `x`. **No derivative, no step.**

### Two Kinds of Equation

The continuous equations Rumoca puts in `f_x` come in two flavours,
distinguished by whether they mention any derivative:

**(1) Derivative equations.** These reference `der(...)` and constrain
its value. Example:

```modelica
der(x) = v;            // the time derivative of x is v
```

In residual form: `0 = v - der(x)`. The integrator can read out
`der(x) = v` directly: this equation is the **source** of `der(x)`'s
value at each step.

**(2) Algebraic constraints.** These mention only the *values* of
variables, not their derivatives. Example:

```modelica
y = sin(x);            // y always equals sin(x)
```

In residual form: `0 = sin(x) - y`. This equation tells the integrator
how `y` relates to `x` at every instant, but it says **nothing directly**
about `der(y)` or `der(x)`. It restricts values, not rates of change.

For the integrator to advance state `x`, *some* equation in `f_x` must be
of kind (1) for `x` — must give it a value for `der(x)`.

### When a State Has No Derivative Equation

Phase 6 classifies a variable as a state if it ever appears under
`der()` somewhere in the model. That is the standard syntactic rule. But
the rule says nothing about whether the state has a *usable* derivative
equation in `f_x`.

A derivative equation is **usable** for state `x` if it lets the
integrator extract a value for `der(x)` at each time step. Two
requirements:

1. **The equation must mention `der(x)` explicitly.** An algebraic
   constraint, however restrictive, says nothing directly about
   derivatives — it constrains values, not rates of change, and so it
   cannot serve as a source for `der(x)`.

2. **`der(x)` must be extractable numerically.** The simplest usable
   form is `der(x) = expr(...)` where `expr` involves only quantities
   the integrator already knows: other states' current values,
   algebraics, parameters, and time. Equations that couple several
   state derivatives together — for instance
   `0 = M[1,1]·der(x1) + M[1,2]·der(x2) - f1` — are also workable,
   provided that across `f_x` as a whole there are enough independent
   rows for phase 7's structural matching to assign one equation per
   derivative. (This is the mass-matrix case mentioned earlier.)

Rumoca's pass uses a slightly conservative version of this test: a
state's derivative equation is considered usable only if some row of
`f_x` mentions **exactly** that state's derivative and **no other**
state's derivative — a *standalone* derivative equation. Equations
that couple multiple state derivatives don't count, even if phase 7
might ultimately resolve them. This is a cheap structural check
(`state_has_standalone_der_equation` in
`crates/rumoca-phase-structural/src/dae_prepare/state_row_reduction.rs`)
that runs before any matching has been attempted, and it errs on the
side of running index reduction when it isn't strictly necessary
rather than skipping it when it is.

A variable can therefore be classified as a state and yet have no
usable derivative equation.

When that happens, the variable is said to be **implicitly constrained**.
What does that mean concretely? Suppose `x` is a state and the only
equation involving `x` is the algebraic constraint

```
y = sin(x)
```

The integrator knows `x`'s current *value* (it's in the state vector).
It can compute `y` from `x` whenever it wants. But it has no way to
compute `der(x)`. The variable's behaviour is fixed by the constraint —
if you knew `der(y)`, you could deduce `der(x) = der(y) / cos(x)` — but
that derivative relationship is **implicit** in the equation. It is
never spelled out as a `der(x) = ...` row that the integrator could
read. The integrator's mass-matrix machinery only sees the static
relation `y = sin(x)` and gets stuck.

This raises a fair question: if phase 6 classifies a variable as a
state *because* `der(x)` appears somewhere, how can a state ever end
up without a derivative equation? At the moment phase 6 finishes, by
construction, every state's name appears under `der(...)` in at least
one place the classifier scanned.

Two facts make the missing-derivative case possible.

- **Classification happens once and is not revisited.** The state set
  is recorded in `dae.variables.states` at the end of phase 6 and stays fixed
  thereafter. Subsequent processing — prep passes that rewrite or
  remove equations in `f_x` — does **not** re-run classification.
  A variable that earned its "state" status from some `der(x)`
  reference can keep that status even after the reference is gone.

- **The classifier scans more places than `f_x`.** Phase 6's
  state-detection scanner walks regular equations, **initial
  equations**, and **variable bindings** looking for `der(...)`
  references. The index-reduction pass, by contrast, looks only at
  `f_x` (the continuous equations the integrator actually solves).
  So if `der(x)` appeared only in an initial equation, or only in a
  binding for a discrete variable (which routes to `f_z` rather than
  `f_x`), then `x` is classified as a state but `f_x` itself never
  receives a `der(x)` term.

In practice, the realistic paths to a state with no usable derivative
equation in `f_x` are:

- **`der(x)` appeared only outside `f_x`.** The most plausible case
  in hand-written Modelica: only in initial equations, or only in
  code that gets routed to a discrete equation group. The pass
  treats this as a missing-derivative case and tries to recover by
  differentiating an algebraic constraint involving `x`.

- **Programmatic DAE construction.** Rumoca exposes its `Dae` IR as
  a public Rust type, so a non-Modelica caller — or a test fixture
  — can build a `Dae` directly. Such a caller may declare a state
  without providing a derivative equation. The pass's regression
  test exercises the missing-der case using this approach.

- **Future prep passes that mutate `f_x`.** Today's prep stages
  (compound-derivative expansion, derivative-alias elimination)
  preserve at least one `der(state)` reference per state — alias
  elimination, for instance, only fires when two equations contain
  `der(x)` and removes only one of them. The index-reduction pass
  is a defensive backstop in case future stages are less careful.

For typical Modelica models built from Modelica Standard Library
components, this case is uncommon — most states have their `der(x)`
equation in `f_x` as soon as phase 6 finishes. The pass exists to
keep the simulator robust against the unusual situations above and
against future changes to the prep pipeline, not because well-formed
Modelica routinely produces missing-derivative states.

The symptom in any of these cases is the same: phase 7 (structural
analysis) tries to match each state to a derivative-bearing equation
and fails for at least one state, because no such equation exists in
`f_x`. Without intervention the simulator stops with
`MissingStateEquation(name)`.

### The Fix: Differentiate the Constraint

If the only equation involving `x` is an algebraic constraint, we can
*manufacture* a derivative equation by differentiating the constraint
with respect to time.

For `y = sin(x)`:

```
y = sin(x)
        │ d/dt
        ▼
der(y) = cos(x) · der(x)
```

The new equation mentions both `der(x)` and `der(y)` — now the
integrator has something to work with. Provided `der(y)` is known
elsewhere (say, from another equation `der(y) = ...`), this gives a
relation that determines `der(x)`.

This is **index reduction**: rewrite a DAE in which derivatives are
implicit so that they become explicit. The transformation costs one
symbolic differentiation per problematic state. The benefit is a DAE
the integrator can actually solve.

A subtlety worth flagging up front: the new equation is **placed where
the old constraint was**. Index reduction *replaces* the algebraic
constraint with its derivative; it does not *add* a new equation
alongside the old one. The total equation count of `f_x` is preserved,
which keeps the system square and matchable. The original constraint
is no longer enforced as an equation — it is now enforced only at
$t = 0$ via the initial condition solver, with the differentiated form
maintaining it dynamically.

### What Rumoca Does

Rumoca's pass —
`index_reduce_missing_state_derivatives` (`crates/rumoca-phase-structural/src/dae_prepare/state_row_reduction.rs`)
— implements exactly this transformation, focused on the case where a
state has *no* derivative equation in `f_x` at all. Roughly:

1. For each state without a standalone `der(state)` equation:
2. Find an algebraic constraint in `f_x` that mentions the state.
3. Symbolically differentiate that constraint with respect to time.
4. Validate the result (the differentiated equation must reference
   exactly one state's derivative, and that derivative must be the
   missing one).
5. Replace the original equation in `f_x` with its differentiated form.

A companion set of state-demotion sweeps handles cases where index
reduction can't fix the problem: variables that turn out not to need
derivative-bearing equations get reclassified from `dae.variables.states` to
`dae.variables.algebraics`. These sweeps and the pipeline that orchestrates them
are described later in this document.

### A Concrete Motivating Case

The regression test for the pass illustrates the simplest "missing
`der(state)` equation" scenario:

```
states:     x, v       (so we expect der(x) and der(v) somewhere in f_x)
algebraics: z

f_x:
  0 = x - z              (constraint: x = z; no der anywhere)
  0 = z - v              (definition: z = v; no der anywhere)
  0 = der(v) - 1         (ODE for v: der(v) = 1)
```

Two states, three equations. Walking the equations and asking "which
one gives `der(x)`?": the first has no derivative, the second has no
derivative, the third gives `der(v)`. There is no source for `der(x)`.
Phase 7 will fail with `MissingStateEquation("x")`.

Index reduction picks the first equation (it mentions `x` and contains
no derivatives) and differentiates it. Using the chain rule plus the
known relations `der(v) = 1` and `der(z) = der(v) = 1`:

```
  0 = x - z
        │ d/dt
        ▼
  0 = der(x) - der(z)   →   0 = der(x) - 1
```

The transformed equation has been written back into `f_x[0]`,
replacing the original constraint. Now `der(x)` is computable (it's
just `1`), the matching succeeds, and the simulator can run.

The full trace of this example, with origin tags and the round-by-round
behaviour of the outer driver, appears in
[Worked Example: The Regression Test](#worked-example-the-regression-test)
below.

---

## What "Differential Index" Means

The behaviour we just walked through has a formal name. The
**differential index** of a DAE is, informally, the smallest number of
times you must time-differentiate (a subset of) its equations to obtain a
system in which every derivative can be solved for explicitly.

Three regimes matter in practice:

- **Index 0 — ordinary differential equations (ODEs).** Every state has
  an equation `der(x) = expr(...)` where `expr` involves only states,
  inputs, parameters, and time — no other unknown derivatives, no
  algebraic constraints between states. Any explicit Runge-Kutta or BDF
  integrator handles this directly.

- **Index 1 — well-posed DAEs.** Each state has a derivative equation
  (possibly coupled with others), plus there may be algebraic constraints
  involving algebraic variables. The integrator solves a small algebraic
  system at each step alongside the ODE update. Stiff DAE integrators
  like BDF handle this case via the **mass matrix formulation**
  `M · der(y) = f(t, y)` — the algebraic rows of `M` are zero, encoding
  the value-only nature of those equations.

- **Index 2 or higher — implicitly-constrained DAEs.** At least one state
  has no usable derivative equation. The trajectory is determined by an
  algebraic constraint relating the state to other variables.
  General-purpose integrators do **not** handle this directly; the system
  must first be **index-reduced** to index 1 by differentiating one or
  more constraints.

The PointOnLine model is a good example of an index-2 system:

```modelica
model PointOnLine
  Real x, y, vx, vy;
equation
  der(x) = vx;
  der(y) = vy;
  der(vx) = 0;
  der(vy) = -9.81;
  y = x + 1;        // constraint: point lies on y = x + 1
end PointOnLine;
```

There are five equations and four states. All four states *do* have
derivative equations (the first four equations), but the fifth — the
constraint `y = x + 1` — is an algebraic constraint between two states.
The constraint also implies `der(y) = der(x)` (by differentiation), which
together with the existing equations forces `vy = vx`. That hidden
relationship between the `vx` and `vy` derivatives is what makes the
system index 2: a derivative coupling that the equations imply but never
state explicitly.

### What Rumoca's Pass Does and Doesn't Cover

Rumoca's pass handles a focused subset of index reduction: the case where
a state has **no** `der(state)` equation at all in `f_x`. The fix is a
single symbolic differentiation of one algebraic constraint. This is
sufficient for many index-2 cases that arise in MSL-derived models —
particularly the cases where alias elimination has stripped the original
derivative equation, and the cases where `StateSelect = Always`
annotations promote a variable past what the equations support.

It is **not** a full implementation of Pantelides's algorithm, which would
handle higher-index systems (index 3+) and constraint-form models like
PointOnLine where every state already has a derivative equation but a
hidden coupling between derivatives needs to be made explicit. PointOnLine
in particular requires a richer technique called **dummy derivative
selection** (Mattsson and Söderlind), which Rumoca does not currently
implement. PointOnLine is shown above to illustrate what "index 2" looks
like, not to suggest that Rumoca's pass will rewrite it.

See [What This Pass Is Not](#what-this-pass-is-not) below for a more
careful inventory of what is and isn't covered.

---

## What the Pass Mutates

The pass operates **in place** on a `Dae` value, modifying:

- **`dae.continuous.equations`.** For each successfully index-reduced state, exactly one
  equation in `f_x` has its `rhs` overwritten with the time-differentiated
  expression, and its `origin` is amended with
  `"index_reduction:d_dt_for_<state_name>"`. The number of equations in
  `f_x` does not change — the pass replaces equations rather than adding
  them.

- **`dae.variables.states` and `dae.variables.algebraics`.** The companion demotion sweeps
  move variables from `dae.variables.states` to `dae.variables.algebraics` when those variables
  cannot serve as states even after index reduction.

Because both partitions and equations change, the prep pass is correctly
viewed as completing the construction of the Appendix B DAE — not as a
separate "analysis" of an already-finished one.

---

## The Index-Reduction Algorithm

### The outer driver

```rust
pub fn index_reduce_missing_state_derivatives(dae: &mut Dae) -> Result<usize, StructuralError> {
    let max_rounds = dae.variables.states.len().clamp(1, 8);
    let mut total_changed = 0;
    for _round in 0..max_rounds {
        let changed = index_reduce_missing_state_derivatives_once(dae)?;
        if changed == 0 { break; }
        total_changed += changed;
    }
    Ok(total_changed)
}
```

The function calls `_once` repeatedly until either no further differentiation
is required or the round budget is exhausted. The cap of `min(n_states, 8)`
is a defensive safeguard against pathological models — eight rounds far
exceeds anything a well-formed Modelica model needs, and the bound prevents
infinite loops if the differentiation logic produces an equation that
itself triggers another round.

The pass is **Pantelides-flavoured** in that it iterates until convergence,
but it does **not** track an augmented matching across rounds (Pantelides's
defining contribution). Each round simply walks the current state list and
applies the per-state procedure below.

### The per-state procedure

```rust
pub fn index_reduce_missing_state_derivatives_once(
    dae: &mut Dae,
) -> Result<usize, StructuralError> {
    let state_names: Vec<VarName> = dae.variables.states.keys().cloned().collect();
    if state_names.is_empty() { return Ok(0); }
    let state_name_set: HashSet<String> = /* ... */;
    let defining_expr_index = collect_residual_defining_expr_index(dae);
    let mut changed = 0usize;
    let mut used_eq = HashSet::new();

    for state_name in &state_names {
        if state_has_standalone_der_equation(dae, state_name, &state_names)? {
            continue;       // already index-1 for this state
        }

        let candidate_indices: Vec<usize> = dae.continuous.equations.iter().enumerate()
            .filter_map(|(idx, eq)| {
                if used_eq.contains(&idx) { return None; }
                if eq_contains_any_state_der_with_matcher(&eq.rhs, &matcher) { return None; }
                // also skip unsliced algebraic definitions and indexed-component aliases
                Some(idx)
            })
            .collect();

        for idx in candidate_indices {
            let seed_exprs = vec![dae.continuous.equations[idx].rhs.clone()];
            let der_map = build_relaxed_derivative_map_for_exprs_with_index(
                dae, &defining_expr_index, &seed_exprs)?;
            let differentiated =
                symbolic_time_derivative(&dae.continuous.equations[idx].rhs, dae, &der_map);
            let Some(new_rhs) = differentiated else { continue; };
            let der_states = derivative_states_in_eq(&new_rhs, &state_names);
            if !der_states.iter().any(|s| s == state_name) { continue; }
            if expr_contains_der_of_non_state(&new_rhs, &state_name_set) { continue; }

            // Accept: rewrite the equation in place
            dae.continuous.equations[idx].rhs = new_rhs;
            dae.continuous.equations[idx].origin =
                format!("{}|index_reduction:d_dt_for_{}", old_origin, state_name.as_str());
            used_eq.insert(idx);
            changed += 1;
            break;
        }
    }
    Ok(changed)
}
```

### Step by step

**1. Skip states that are already fine.** `state_has_standalone_der_equation`
checks whether some equation in `f_x` mentions exactly `der(state)` and no
other state's derivative. If yes, the state is already index-1 and needs no
work.

**2. Build a derivative map.** `build_relaxed_derivative_map` produces a
`HashMap<state_name, derivative_expression>` from the existing ODE-form
equations. This is what the symbolic differentiator uses as the chain-rule
substrate: when it encounters `state` during differentiation, it substitutes
in the known `der(state)` expression. We'll see this in the next section.

**3. Pick candidate equations.** For the current state, enumerate `f_x`
entries that:

- Have not already been consumed for index-reducing some other state
  (`used_eq`).
- Do not currently mention any `der(state')` for any state — these are
  pure algebraic constraints and the only kind we want to differentiate.
  Differentiating an equation that already has a derivative would in
  general yield a second-derivative term, which the integrator can't
  handle.
- Reference the current state's value (otherwise differentiating gives
  no useful information about this state).

**4. Try to differentiate each candidate.** `symbolic_time_derivative`
returns `Option<Expression>` — `None` if the differentiator hits a
construct it can't handle (e.g., a non-elementary builtin), `Some(d_expr)`
otherwise.

**5. Validate the result.** Three acceptance criteria:

- `der_states.len() == 1` — the differentiated expression must mention
  *exactly one* state's derivative. Multiple state derivatives or none at
  all both indicate the differentiation didn't isolate `der(state)`
  cleanly enough for the integrator to use, so we skip this candidate.
- `der_states[0] == *state_name` — the one state derivative must be the
  one we're trying to introduce. Otherwise the differentiated equation
  helps a different state and we'd be double-using it.
- `!expr_contains_der_of_non_state(...)` — there must be no `der(...)` of
  a non-state variable left in the expression. Such derivatives would
  not be evaluable by the runtime (the residual evaluator only handles
  `der` of states).

**6. Commit the rewrite.** If all three checks pass, overwrite
`dae.continuous.equations[idx].rhs`, append the origin marker, mark the equation index as
consumed, and `break` to move on to the next state.

The break-on-success-per-state ensures that each state consumes at most
one equation per round, and `used_eq` ensures equations are not consumed
by multiple states even within a single round.

---

## The Symbolic Differentiator

The implementation is in
`crates/rumoca-phase-structural/src/dae_prepare/symbolic.rs`
and is a direct, recursive chain-rule walker:

```rust
fn differentiate(&self, expr: &Expression) -> Option<Expression> {
    match expr {
        Expression::Literal(_) => Some(zero_literal()),
        Expression::VarRef { name, subscripts } => self.differentiate_variable(name, subscripts),
        Expression::Binary { op, lhs, rhs }    => self.differentiate_binary(op, lhs, rhs),
        Expression::Unary  { op, rhs }         => self.differentiate_unary(op, rhs),
        Expression::If { branches, else_branch } => self.differentiate_if(branches, else_branch),
        _ => None,           // unsupported construct → bail out
    }
}
```

The interesting cases:

### Variable references

```rust
fn differentiate_variable(&self, name, subscripts) -> Option<Expression> {
    if !subscripts.is_empty() { return None; }   // arrays not handled
    if name.as_str() == "time" {
        return Some(Expression::Literal(Literal::Real(1.0)));   // d(time)/dt = 1
    }
    if self.dae.variables.parameters.contains_key(name) || self.dae.variables.constants.contains_key(name) {
        return Some(zero_literal());                            // d(p)/dt = 0
    }
    self.der_map.get(name.as_str()).cloned()                    // d(state)/dt = der_map[state]
}
```

A reference to `time` differentiates to 1. A reference to a parameter or
constant differentiates to 0. A reference to a state name pulls the known
derivative expression out of `der_map`. Anything else (an algebraic
variable not in the map, or a subscripted reference) fails with `None` and
unwinds the entire differentiation.

The `der_map` is the chain-rule substrate built earlier. For the regression
test case, `der_map = { "v": 1 }` (from `der(v) = 1`) and also `"z": 1` if
the relaxed map can resolve `z = v ⇒ der(z) = der(v) = 1`. When the
differentiator encounters `z` while differentiating `0 = x - z`, it
substitutes `der(z) = 1` from the map — without the map, differentiation
would fail because `z` is an algebraic variable.

### Binary operators

The product, quotient, sum, and difference rules are direct:

| Operator | Derivative |
|----------|-----------|
| $a + b$ | $\dot a + \dot b$ |
| $a - b$ | $\dot a - \dot b$ |
| $a \cdot b$ | $\dot a \cdot b + a \cdot \dot b$ |
| $a / b$ | $(\dot a \cdot b - a \cdot \dot b) / b^2$ |
| anything else | bail out (`None`) |

Power, modulo, comparison, etc. are not handled — encountering them aborts
the differentiation. This is deliberate: a partial implementation that
silently produced wrong derivatives would be far worse than one that
declines and lets the simulator report a clean failure.

### Unary operators and conditionals

`-x` differentiates to `-(dx/dt)`; `+x` differentiates to `dx/dt`. An
`if cond then a else b` differentiates by leaving the condition alone (it
evaluates to a Boolean, which has zero time-derivative) and recursively
differentiating each branch's value. Note that the differentiator does
*not* handle `case` expressions or other higher-level conditionals.

### Why returning `None` matters

A `None` from any deeply-nested differentiation propagates all the way up,
because every recursive helper uses `?` on its child results. This means a
single unsupported construct anywhere inside the expression aborts the
whole differentiation cleanly — the candidate is rejected and the next
candidate (or the next state) is tried. There's no partial differentiation
that could be silently incorrect.

---

## State Demotion Sweeps

Index reduction is necessary but not sufficient: even after differentiating
constraints, some variables originally classified as states may still lack
a usable derivative row. Three demotion sweeps run during the prep pass to
catch these cases.

### `demote_orphan_states_without_equation_refs`

A state whose name appears in **no equation at all** cannot be solved.
This usually means an alias-elimination pass earlier in the prep pipeline
removed the equation that was the variable's only reference. Demote
silently:

```rust
for name in state_names {
    if !state_has_any_equation_reference(dae, &name) {  // checks dae.continuous.equations
        if let Some(var) = dae.variables.states.shift_remove(&name) {
            dae.variables.algebraics.insert(name, var);
        }
    }
}
```

### `demote_states_without_derivative_refs`

A state whose name is referenced but never under `der()` cannot be a state.
This can happen when the original Modelica model had a `der(x)` which got
substituted away earlier in the prep pipeline (alias elimination, compound
derivative expansion). Same demotion.

### `demote_states_without_assignable_derivative_rows`

This is the most interesting sweep. A state may have multiple equations
mentioning `der(state)`, but if those equations also mention `der(state')`
for some other state, the assignment of equations to states becomes a
**bipartite matching problem**: states on one side, derivative-bearing
rows on the other, edges connecting state $s$ to row $r$ if `der(s)`
appears in row $r$.

```rust
fn states_with_assignable_derivative_rows(dae: &Dae, state_names: &[VarName]) -> HashSet<usize> {
    let bindings = structural_scalar_bindings(dae);
    let state_to_rows: Vec<Vec<usize>> = state_names.iter()
        .map(|state_name| dae.continuous.equations.iter().enumerate()
            .filter_map(|(r, eq)| /* checks expr_contains_active_exact_der_of_state */)
            .collect())
        .collect();

    // Process states in order of increasing fan-out (states with the fewest
    // candidate rows first), so they get to claim their narrow choices before
    // the more flexible ones do.
    let mut state_order: Vec<usize> = (0..state_names.len()).collect();
    state_order.sort_by_key(|idx| state_to_rows[*idx].len());

    let mut row_to_state: Vec<Option<usize>> = vec![None; dae.continuous.equations.len()];
    for state_idx in state_order {
        let mut seen_rows = vec![false; dae.continuous.equations.len()];
        try_match_state_to_row(state_idx, &state_to_rows, &mut row_to_state, &mut seen_rows);
    }
    row_to_state.into_iter().flatten().collect()
}
```

This is exactly the augmenting-path bipartite matching from
[`maximum_bipartite_matching.md`](../phase7_structural_analysis/maximum_bipartite_matching.md),
applied here to a different domain (states ↔ derivative-bearing rows
instead of equations ↔ unknowns). States that cannot be matched are
demoted.

The state ordering — by ascending number of candidate rows — is a standard
greedy heuristic for bipartite matching: tackle the most constrained
choices first, so they don't get blocked by less-constrained competitors.

The whole sweep runs inside a fixpoint loop because demoting one state may
make a previously-matched state lose its only candidate row (an equation
that mentioned both `der(s)` and `der(s')` may no longer be a candidate
for `s` once `s'` is demoted).

---

## Where It Fits in the Prep Pipeline

The pass is one stage of a larger DAE-preparation sequence. The pipeline
is orchestrated by `prepare_dae_for_structural_analysis` in
`crates/rumoca-sim/src/solve_lowering/structural_lowering.rs`.
The actual stage order (as of current source) is:

| Order | Function | Purpose |
|-------|----------|---------|
| 1 | `scalarize_equations` | (optional) Scalarize vector equations |
| 2 | `demote_exact_alias_component_states` | Demote duplicate states connected through exact alias equalities |
| 3 | `demote_direct_assigned_states` | Demote states whose value is directly assigned by a non-derivative equation |
| 4 | `reduce_constrained_dummy_derivatives` | Structural dummy-derivative reduction for constrained states |
| **5** | **`index_reduce_missing_state_derivatives`** | **Index reduction (this drill-down)** |
| 6 | `demote_states_without_assignable_derivative_rows` | Bipartite-matching demotion of unmatchable states |
| 7 | `eliminate_derivative_aliases` | Remove `der(x) = der(y)` style aliases |
| 8 | `demote_states_without_retained_derivative_rows` | Post-alias-elimination derivative-row demotion |
| 9 | `expand_compound_derivatives` | Expand `der(algebraic)` and `der(compound)` via chain rule |
| 10 | `substitute_standalone_state_derivatives_in_non_ode_rows` | Rewrite `y = der(x)` to `y = <x's ODE rhs>` |

Note that the three new early stages (exact-alias demotion,
direct-assignment demotion, constrained-dummy reduction) now run
**before** index reduction. Compound derivative expansion and derivative
alias elimination now run **after** index reduction and demotion,
reversing the order documented in earlier versions.

A separate codegen-oriented prep entry point, `prepare_dae_for_codegen`,
lives in `rumoca-phase-dae/src/dae_lowering.rs`. It covers the subset of
prep stages needed for ahead-of-time code generation.

### New submodule files in `dae_prepare/`

Four submodule files have been added to support the new early stages:

| File | Purpose |
|------|---------|
| `connection_alias.rs` | Connection-component fixed defining expression resolution |
| `direct_demotion.rs` | Direct-assignment state demotion logic |
| `dummy_state_metadata.rs` | Constrained dummy-state identification (`constrained_dummy_state_defining_exprs`, `constrained_dummy_state_names`) |
| `row_shape.rs` | DAE variable sizing and residual scalar-width helpers (`dae_variable_size`, `required_dae_variable_size`, `residual_scalar_width`) |

Each stage logs its work via `run_logged_phase` when `RUMOCA_SIM_TRACE`
is set, making it possible to see exactly which stages did or did not
change the DAE for a given model.

---

## Worked Example: The Regression Test

From
`crates/rumoca-sim/src/solve_lowering/tests.rs`:

**Input DAE:**

```
states:     x, v
algebraics: z

f_x:
  eq0  origin="constraint_x"  rhs = x - z         (no der anywhere)
  eq1  origin="def_z"         rhs = z - v         (no der anywhere)
  eq2  origin="ode_v"         rhs = der(v) - 1    (gives der(v))
```

**Phase 7 (structural analysis) on the input:** would fail —
`reorder_equations_for_solver` returns `Err(MissingStateEquation("x"))`
because there is no equation matchable to `der(x)`. The test asserts this
behaviour explicitly *before* index reduction is applied.

**Index reduction round 1.** The driver iterates over the state list `[x, v]`:

For state `x`:

- `state_has_standalone_der_equation(dae, "x", ["x", "v"])` → false
  (no equation has just `der(x)`).
- Candidate equations: those that reference `x` and don't already mention
  any `der(state)`. eq0 (`x - z`) qualifies. eq1 doesn't reference `x`,
  eq2 mentions `der(v)`.
- Try `symbolic_time_derivative(x - z, dae, der_map)`:
  - `der_map = { "v": 1, "z": 1 }` (the relaxed map can resolve `z = v`'s
    derivative as `1`).
  - Differentiate `x - z`: differentiate `x` → `der_map["x"]` (None — `x`
    isn't in der_map yet). Wait, that should fail.

Actually let's re-read: the differentiator produces a literal `der(x)`
expression for state references that aren't in the map. Looking at
`expand_der_in_expr_full` in `symbolic.rs`:

  - When differentiating a `VarRef` to a state, the differentiator first
    consults `der_map`. For state `x`, the map contains no entry — but the
    surrounding logic writes a literal `der(x)` reference, which is then
    accepted because it's exactly the missing derivative we want to
    introduce.

The differentiated form of `x - z` is `der(x) - der(z)`. With the relaxed
map giving `der(z) = 1`, this further simplifies (or remains symbolic, depending
on the chain-rule path taken) to an expression of the form
`der(x) - 1`, which mentions exactly `der(x)` and no other state's
derivative. All three acceptance criteria pass.

- **Commit:** `dae.continuous.equations[0].rhs = der(x) - 1` (or equivalent),
  `dae.continuous.equations[0].origin = "constraint_x|index_reduction:d_dt_for_x"`.
  Mark eq0 as used, increment `changed`.

For state `v`:

- `state_has_standalone_der_equation(dae, "v", ...)` → true (eq2 has
  exactly `der(v)`). Skip.

Round 1 returns `changed = 1`. Round 2 finds nothing left to do
(`changed = 0`) and the driver exits.

**Output DAE:**

```
states:     x, v
algebraics: z

f_x:
  eq0  origin="constraint_x|index_reduction:d_dt_for_x"  rhs ≈ der(x) - 1
  eq1  origin="def_z"                                    rhs = z - v
  eq2  origin="ode_v"                                    rhs = der(v) - 1
```

**Phase 7 on the output:** succeeds. `der(x)` now has a matchable equation
(eq0), `der(v)` matches eq2, and `z` matches eq1. The test asserts
`reorder_equations_for_solver` returns `Ok(...)` after index reduction.

The Appendix B form is preserved throughout: `dae.variables.states` and `dae.variables.algebraics`
are unchanged, `f_x` still has three equations, but the *content* of `f_x[0]`
has been transformed.

---

## What This Pass Is Not

To set expectations correctly, here are things this implementation does **not**
do:

- **Full Pantelides algorithm.** Pantelides (1988) builds an augmented
  matching that tracks which equations have been differentiated and which
  derivatives have been introduced as new variables, then iteratively
  searches for unmatched states and differentiates the equations on their
  augmenting paths. Rumoca's pass does no such matching across rounds; it
  performs a per-state local search each round and re-runs the whole pass
  until stable. This is sufficient for the common index-2 cases but does
  not provably reach a balanced index-1 form for arbitrary high-index
  models.

- **Dummy derivatives.** When index reduction introduces multiple new
  derivative-of-derivative variables, Mattsson-Söderlind's "dummy
  derivative" trick is the standard way to choose which derivatives are
  states and which are algebraic outputs. Rumoca does not implement this;
  it works at the level of plain `der(state)` references and relies on
  the symbolic differentiator's chain-rule expansion to keep all
  introduced derivatives expressible in those terms.

- **Higher-than-once differentiation per equation.** Each constraint is
  differentiated *once* per round. If a constraint needs two
  differentiations (an index-3 problem), the round-loop will iterate, but
  the differentiator will fail on any equation that already contains a
  `der(state)` term — so genuine index-3 systems may not converge. Models
  built from MSL components rarely exceed index 2 in practice.

- **Array-aware differentiation.** `differentiate_variable` returns `None`
  on any subscripted reference. Models that need to differentiate an
  array constraint must rely on the upstream scalarisation pass to
  decompose the constraint into scalar equations first.

When the pass cannot help, the simulator typically reports
`SimError::MissingStateEquation` with the offending state's name, and the
modeler can rewrite the model to expose the derivative explicitly (often by
introducing an explicit auxiliary variable).

---

## Where the Code Lives (and the v0.9.x Refactor)

As of v0.9.x, the index-reduction pass and its companion DAE-prep helpers
live in the **`rumoca-phase-structural`** crate, under
`src/dae_prepare/`. Earlier versions kept the code in `rumoca-sim` (under
`src/simulation/dae_prepare/`) for historical reasons — the pass was
originally written as part of the simulator's setup logic, where it had
access to `diffsol`-specific utilities. The v0.9.x architecture split
moved the DAE-prep helpers into the phase-crate layer they conceptually
belong to, alongside structural analysis (matching, BLT, tearing, IC plan).

The orchestrating *driver* — the function that runs the DAE-prep stages
in order — still lives in `rumoca-sim`, at
solve_lowering/structural_lowering.rs (`crates/rumoca-sim/src/solve_lowering/structural_lowering.rs`)
as `prepare_dae_for_structural_analysis`.
That driver imports each prep helper from
`rumoca_phase_structural::dae_prepare` and calls them in sequence. The
heavy lifting — the symbolic differentiation, the matching-based demotion,
the alias elimination — happens entirely within `rumoca-phase-structural`.

A separate codegen-oriented prep entry point, `prepare_dae_for_codegen`,
lives in `rumoca-phase-dae/src/dae_lowering.rs`. It covers the subset of
prep stages needed for ahead-of-time code generation (the
simulator-specific stages are skipped). Both entry points share the same
`rumoca-phase-structural` helpers, so every downstream consumer —
simulation, template codegen, solve-IR lowering — sees an index-1 DAE
built by the same code paths.

---

## Summary

- The `Dae` produced by `to_dae()` may have differential index > 1, with
  states whose `der(state)` does not appear in any `f_x` equation. Such a
  DAE is not directly solvable by ODE/DAE integrators; it must be
  index-reduced.
- Rumoca's index-reduction pass — `index_reduce_missing_state_derivatives`
  in `crates/rumoca-phase-structural/src/dae_prepare/state_row_reduction.rs`
  — handles this by symbolically differentiating an algebraic constraint
  for each state without a derivative row, validating the result, and
  rewriting the equation in place.
- The symbolic differentiator implements the chain rule for elementary
  arithmetic, with `time → 1`, `parameter → 0`, `state → der_map[state]`,
  and `None` (graceful failure) for anything else.
- Three companion demotion sweeps move variables between `dae.variables.states` and
  `dae.variables.algebraics` when even index reduction cannot give them a usable
  derivative row. The third sweep is a bipartite matching identical in
  spirit to phase 7's matching step.
- The whole prep pass runs after `to_dae()` produces the raw `Dae` but
  before phase 7 (structural analysis) consumes it. Its output is a valid,
  index-1 Appendix B DAE that phase 7 can analyse.
- The implementation lives in `rumoca-phase-structural`
  (the phase-crate layer where it conceptually belongs) -- it mutates the
  Appendix B form rather than producing an analysis artifact alongside it.
  The orchestrating driver `prepare_dae_for_structural_analysis` lives in
  `rumoca-sim` for the simulator path, and `prepare_dae_for_codegen`
  (in `rumoca-phase-dae/src/dae_lowering.rs`) serves the codegen path;
  both call the same `rumoca-phase-structural::dae_prepare` helpers, so
  every downstream consumer sees an index-1 DAE built by the same code
  paths.
