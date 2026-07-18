# Drill-Down: Tearing of Algebraic Loops

*Parent document: [structural_analysis.md](structural_analysis.md)*
*Source: [crates/rumoca-phase-structural/src/tearing.rs](../../../crates/rumoca-phase-structural/src/tearing.rs)*

---

## What Problem Does This Step Solve?

When [BLT decomposition](blt.md) produces an `AlgebraicLoop` block of size $N$,
the literal interpretation is "solve $N$ coupled nonlinear equations in $N$
unknowns." The standard tool for that is a Newton iteration, which on each
step

- Evaluates all $N$ residuals.
- Builds and factorises the $N \times N$ Jacobian.
- Solves a linear system to update all $N$ unknowns at once.

The cost grows roughly as $N^3$ per Newton step (from the linear solve), and
the size of the trust region matters. For large blocks this is expensive and
sometimes ill-conditioned.

**Tearing** replaces the $N$-dimensional Newton solve with a $K$-dimensional
one, where $K \ll N$ in practice. The remaining $N - K$ unknowns are solved
*causally* — one at a time, in order, each from a single equation — once the
$K$ "tear" variables are known. The Newton (or Levenberg-Marquardt) iteration
runs only on the $K$ tear variables, driving the residuals of the leftover
$N - K$ equations toward zero.

If $K = 1$ for an $N \times N$ loop, this turns an $O(N^3)$ Newton step into
an $O(1)$ scalar Newton step plus $O(N)$ causal evaluations. The savings
compound over every solver step, so even modest tearing yields large
speedups.

---

## The Cellier Tearing Idea

The algorithm in
[tearing.rs](../../../crates/rumoca-phase-structural/src/tearing.rs) follows
Cellier's greedy strategy. The idea has three pieces:

### 1. Causal resolution

If an equation references exactly one unknown variable (and all the others in
its expression are already known constants or previously computed), it can be
*solved causally* for that variable — no iteration needed, just a single
evaluation. After such a solve, that variable becomes known, which might
expose another equation as having only one remaining unknown. Repeating this
sweep is exactly the "shrinking incidence" view of Gaussian elimination
applied to the structural problem.

### 2. Tear-variable selection

Eventually causal resolution stalls: every remaining equation has at least
two remaining unknowns, and no further direct solves are possible. To break
the deadlock, *pretend* one unknown is already known — call it a **tear
variable**. With one variable removed from the "unknown" set, some equation
may again have exactly one remaining unknown, and causal resolution can
resume.

The choice of which unknown to tear is the heuristic part. Cellier's rule:
pick the unknown that appears in the most remaining equations. The intuition
is that taking that variable out of the unknown set frees up the most
equations.

### 3. Iterate

Resume causal resolution; if it stalls again, pick another tear variable.
Continue until every equation has been categorised either as **causal**
(solved sequentially given the tear variables) or **residual** (its mismatch
drives the Newton iteration).

For a well-behaved loop, $K$ — the number of tear variables — is small. In
the limit, $K = 1$ for many physically meaningful coupled systems (e.g.
electrical circuits with one free node voltage).

---

## What Tearing Produces

```rust
pub struct TearingResult {
    pub tear_var_local_indices:    Vec<usize>,
    pub residual_eq_local_indices: Vec<usize>,
    pub causal_sequence:           Vec<(usize, usize)>,
}
```

- `tear_var_local_indices` — the $K$ unknowns the iterative solver iterates
  over. "Local" means indices into the loop block's own unknown list, not the
  global DAE index.
- `residual_eq_local_indices` — the $K$ equations whose residuals form the
  Newton/LM target. There are exactly as many residual equations as tear
  variables; that balance is required for a square Newton step.
- `causal_sequence` — the $N - K$ steps that solve the remaining unknowns
  sequentially, given the tear variables. Each entry is `(eq_local, var_local)`:
  "evaluate this equation to obtain this variable."

When the tearing function returns `None`, the loop is **untearable** — every
unknown appears in every equation, or the algorithm cannot make progress. The
caller falls back to a coupled $N \times N$ Levenberg-Marquardt solve.

---

## The Two Helper Functions

The main routine is built on two helpers worth examining individually.

### `resolve_causal_equations`

```rust
fn resolve_causal_equations(
    remaining_eqs:      &mut BTreeSet<usize>,
    remaining_unknowns: &mut BTreeSet<usize>,
    causal_sequence:    &mut Vec<(usize, usize)>,
    eq_unknowns:        &[HashSet<usize>],
) {
    let mut changed = true;
    while changed {
        changed = false;
        let mut var_to_eqs: BTreeMap<usize, Vec<(usize, usize)>> = BTreeMap::new();
        for &eq in remaining_eqs.iter() {
            let live: Vec<usize> = eq_unknowns[eq]
                .iter()
                .copied()
                .filter(|v| remaining_unknowns.contains(v))
                .collect();
            if live.len() == 1 {
                var_to_eqs
                    .entry(live[0])
                    .or_default()
                    .push((eq, eq_unknowns[eq].len()));
            }
        }
        for (var, mut candidates) in var_to_eqs {
            if !remaining_unknowns.contains(&var) { continue; }
            candidates.sort_by_key(|&(eq, total)| (total, eq));
            let (best_eq, _) = candidates[0];
            causal_sequence.push((best_eq, var));
            remaining_eqs.remove(&best_eq);
            remaining_unknowns.remove(&var);
            changed = true;
        }
    }
}
```

This sweeps the remaining equations as long as anything changes:

1. **Find single-unknown equations.** For every still-pending equation, count
   how many of its referenced unknowns are still pending (some may already be
   solved or torn). If that count is 1, the equation is a candidate for causal
   solve, and it goes into a `var_to_eqs` map keyed by the lone remaining
   unknown.

2. **Resolve conflicts.** Several equations might all be down to the same
   single unknown. The map collects all of them. The chosen one is the one
   with the **fewest total unknowns** (simplest equation: less chance of
   masking conditioning issues), with ties broken by lower equation index for
   determinism.

3. **Solve.** Append `(best_eq, var)` to the causal sequence and remove both
   from the remaining sets. Set `changed = true` so the loop runs again — a
   variable becoming known may expose new single-unknown equations.

The use of `BTreeSet` and `BTreeMap` instead of `HashSet`/`HashMap` is
deliberate: their iteration order is sorted, which keeps the algorithm
deterministic across runs without needing to sort manually.

### `count_var_appearances`

```rust
fn count_var_appearances(
    remaining_eqs:      &BTreeSet<usize>,
    eq_unknowns:        &[HashSet<usize>],
    remaining_unknowns: &BTreeSet<usize>,
) -> BTreeMap<usize, usize> {
    let mut var_count: BTreeMap<usize, usize> = BTreeMap::new();
    for &eq in remaining_eqs {
        for &v in &eq_unknowns[eq] {
            if remaining_unknowns.contains(&v) {
                *var_count.entry(v).or_insert(0) += 1;
            }
        }
    }
    var_count
}
```

Plain frequency counting: for each pending equation, walk its unknowns; for
each unknown that is itself still pending, bump its count. The map's value is
"how many remaining equations still reference this remaining variable." The
tear-variable selector consumes this map.

---

## The Main Algorithm

```rust
pub fn tear_algebraic_loop(n: usize, eq_unknowns: &[HashSet<usize>]) -> Option<TearingResult> {
    if n == 0 { return None; }

    let mut remaining_eqs:      BTreeSet<usize> = (0..n).collect();
    let mut remaining_unknowns: BTreeSet<usize> = (0..n).collect();
    let mut causal_sequence: Vec<(usize, usize)> = Vec::new();
    let mut tear_vars:       Vec<usize>          = Vec::new();

    loop {
        // Phase 1: causal resolution
        resolve_causal_equations(
            &mut remaining_eqs,
            &mut remaining_unknowns,
            &mut causal_sequence,
            eq_unknowns,
        );

        if remaining_eqs.is_empty() { break; }

        // Phase 2: tear-variable selection
        let var_count = count_var_appearances(&remaining_eqs, eq_unknowns, &remaining_unknowns);
        if var_count.is_empty() { break; }   // no progress possible

        let &tear_var = var_count
            .iter()
            .max_by_key(|&(v, count)| (*count, std::cmp::Reverse(*v)))
            .map(|(v, _)| v)
            .unwrap();

        tear_vars.push(tear_var);
        remaining_unknowns.remove(&tear_var);
        // equations are NOT removed — they may become causal once the tear var is "known"
    }

    let mut residual_eqs: Vec<usize> = remaining_eqs.into_iter().collect();
    residual_eqs.sort_unstable();

    // Useful only if we strictly reduced the dimension
    if tear_vars.is_empty() || tear_vars.len() >= n { return None; }

    // The system is square: residual count must equal tear count
    if residual_eqs.len() != tear_vars.len() { return None; }

    Some(TearingResult {
        tear_var_local_indices:    tear_vars,
        residual_eq_local_indices: residual_eqs,
        causal_sequence,
    })
}
```

### How the two phases interleave

The outer loop alternates Phase 1 (causal sweep) and Phase 2 (tear pick) until
either every equation is accounted for or no more tear-variable candidates
exist.

**Initial state.** Every equation and every unknown are pending. `tear_vars`
and `causal_sequence` start empty.

**Phase 1 (causal sweep).** Repeatedly find single-unknown equations and
solve them causally. Each iteration may change the picture, so the helper
loops internally until no more single-unknown equations exist.

**Termination check.** If `remaining_eqs` is now empty, we're done. The
trivial case where every equation can be solved causally falls out here with
zero tear variables; the function later rejects this as "not really a loop"
by returning `None`.

**Phase 2 (tear pick).** Build the appearance counts. The chosen variable
maximises `(count, Reverse(index))`:

- Higher count wins. Choosing the variable that appears in the most equations
  frees up the most causal opportunities.
- Ties broken by **lower** index (the `Reverse` flips the comparison so the
  iterator's `max_by_key` picks the smallest index). Deterministic tie-break.

If no candidate exists (`var_count.is_empty()`), the loop is structurally
stuck and we break out — the function will return `None` because of the size
checks.

**Effect of tearing.** Push the chosen variable into `tear_vars` and remove
it from `remaining_unknowns`. Note carefully: **no equation is removed.** The
equations are kept; they may now have one fewer remaining unknown, so the
next Phase 1 sweep may dispatch some of them causally.

**Loop again.** The next Phase 1 sweep runs with the updated remaining sets.
The cycle continues until everything settles.

### Why equations are not removed when a variable is torn

This is a subtle but critical point. After Phase 2 picks `tear_var`, the
equations that contained it are still pending — they have one less *unknown*,
not one less *equation*. The next Phase 1 either:

- Solves the equation causally for whichever variable it still has pending
  (good outcome — adds it to `causal_sequence`); or
- Leaves it pending because it still has more than one unknown (it remains a
  candidate for being declared a residual or for further tearing).

When Phase 1 + Phase 2 finally settles, whatever equations remain in
`remaining_eqs` — that is, equations that never became single-unknown — are
the **residual equations**. Their residuals are what the iterative solver
drives to zero by adjusting the tear variables.

### The post-conditions

After the loop:

- `causal_sequence` lists the $N - K$ causal solves.
- `remaining_eqs` (after sorting) becomes `residual_eqs`, listing the $K$
  residual equations.
- `tear_vars` lists the $K$ tear variables.

Three sanity checks gate the return:

1. **`tear_vars.is_empty()`** — no tearing happened. Either everything was
   already causal (the block isn't truly a loop), or nothing made progress.
   Either way return `None` so the caller decides what to do.
2. **`tear_vars.len() >= n`** — degenerate case where tearing didn't reduce
   the problem size. Return `None`.
3. **`residual_eqs.len() != tear_vars.len()`** — the resulting system isn't
   square (mismatch between residuals and tear variables). The Newton step
   wouldn't be well-defined, so return `None`.

---

## Worked Example: 3×3 Loop with One Tear

From
[`test_tear_3x3_with_one_tear`](../../../crates/rumoca-phase-structural/src/tearing.rs#L194-L209):

```
eq0 references {v0, v1}
eq1 references {v1, v2}
eq2 references {v0, v2}
```

Every equation has 2 unknowns; nothing is causal at the start. Phase 1 finds
no single-unknown equation — `var_to_eqs` is empty, the helper exits without
changes.

Phase 2 builds appearance counts:
- `v0` appears in eq0, eq2 → count 2
- `v1` appears in eq0, eq1 → count 2
- `v2` appears in eq1, eq2 → count 2

A three-way tie. Tie-break selects the lowest index — `v0`. Push to
`tear_vars`, remove from remaining.

State now: `remaining_unknowns = {v1, v2}`, `tear_vars = [v0]`.

Phase 1 again:
- eq0's live unknowns: `{v1}` (one!). Candidate to solve for v1.
- eq1's live unknowns: `{v1, v2}` (two).
- eq2's live unknowns: `{v2}` (one!). Candidate to solve for v2.

`var_to_eqs = { v1: [(eq0, total=2)], v2: [(eq2, total=2)] }`. Both resolve
in this sweep:

- Append `(eq0, v1)` to causal_sequence; remove eq0 and v1.
- Append `(eq2, v2)` to causal_sequence; remove eq2 and v2.

State now: `remaining_eqs = {eq1}`, `remaining_unknowns = {}`,
`tear_vars = [v0]`, `causal_sequence = [(eq0, v1), (eq2, v2)]`.

The inner sweep changed things, so it runs once more — but with no remaining
unknowns, no equation has any live unknowns, nothing is single, nothing
changes. Helper exits.

Top of the outer loop: `remaining_eqs` is non-empty (it still contains eq1),
so we go to Phase 2. But `count_var_appearances` returns an empty map
(`remaining_unknowns` is empty), and the `var_count.is_empty()` guard breaks
out.

Final accounting:
- `tear_vars = [v0]` (length 1)
- `residual_eqs = [eq1]` (length 1) — passes the square-system check.
- `causal_sequence = [(eq0, v1), (eq2, v2)]` (length 2)

The simulator's iteration plan reads as:

1. **Outer loop:** guess `v0`.
2. **Causal step 1:** evaluate eq0 to obtain `v1`.
3. **Causal step 2:** evaluate eq2 to obtain `v2`.
4. **Residual:** evaluate eq1 with `v0, v1, v2` and check `|residual| < tol`.
5. If not, Newton-update `v0` and go to step 2.

The 3×3 system has been reduced to a 1×1 Newton iteration plus two scalar
solves per step.

---

## When Tearing Cannot Help

Two failure modes return `None`:

### Linear chain with no loop

[`test_tear_linear_chain`](../../../crates/rumoca-phase-structural/src/tearing.rs#L167-L179):
`eq0 = {v0}`, `eq1 = {v0, v1}`, `eq2 = {v1, v2}`. Phase 1 immediately
resolves all three causally — no tear variables are ever picked. The function
returns `None` because `tear_vars.is_empty()`.

In the BLT pipeline this case shouldn't arise: such a chain would be a chain
of size-1 SCCs, not a single loop block. But the function is safe against it.

### Fully dense block

If every equation references every unknown (e.g. an $N \times N$ block of
all-ones in the incidence), Phase 1 always finds zero single-unknown
equations, and Phase 2 tears variables one by one. Eventually `tear_vars.len()`
hits $n$ and the size-reduction check (`tear_vars.len() >= n`) returns `None`.
The caller treats this as untearable and falls back to a coupled solve.

---

## Tests

The test module covers three structural shapes:

- **Linear chain** — pure causality, no tearing produced; returns `None`.
- **2×2 dense loop** — exactly one tear variable, one residual equation, one
  causal step.
- **3×3 with one tear** — the worked example above; one tear, two causal
  steps, one residual.

Each test asserts both the dimensions of the result and (for the structurally
loaded cases) that the size has been reduced.

---

## Why Tearing Is Run on the IC Plan, Not Just the BLT

[Building the BLT block list](blt.md) does not invoke tearing; it just
identifies algebraic loops. Tearing is applied at two later moments:

- The simulation runtime tears each loop's evaluation kernel for
  per-time-step solving (covered in the [simulation document](../phase9_simulation/simulation.md)).
- The [IC plan](ic_plan.md) tears each algebraic loop in the initial-condition
  subsystem to produce a `TornBlock` recipe for consistent initialisation.

In both cases the input is identical — the local incidence within a single
loop block — and the same `tear_algebraic_loop` function is used.

---

## Summary

- An algebraic-loop block of size $N$ is too expensive to solve directly when
  $N$ is large. Tearing reduces it to a $K$-dimensional iteration plus
  $N - K$ causal evaluations.
- The Cellier algorithm alternates **Phase 1** (resolve any single-unknown
  equations causally) and **Phase 2** (declare the most-referenced remaining
  unknown a tear variable). It loops until no equations remain or no progress
  is possible.
- Conflict resolution in Phase 1 prefers equations with fewer total unknowns
  (simpler, better-conditioned). Tie-breaks throughout use lowest index.
- Phase 2 does **not** remove the equation containing the torn variable; only
  the variable is removed from the unknown set. The equation may either turn
  into a causal step in the next Phase 1 sweep or end up as a residual.
- The post-conditions enforce a square Newton system: `|residual_eqs| =
  |tear_vars|`. If they don't match, or no reduction occurred, the function
  returns `None` and the caller falls back to a coupled Levenberg-Marquardt.
- Use of `BTreeSet`/`BTreeMap` keeps iteration order deterministic without
  manual sorting.
