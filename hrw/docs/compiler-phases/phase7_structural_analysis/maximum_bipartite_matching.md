# Drill-Down: Maximum Bipartite Matching

*Parent document: [structural_analysis.md](structural_analysis.md)*
*Source: `crates/rumoca-phase-structural/src/matching.rs`*

---

## What Problem Does This Step Solve?

After the [incidence matrix](incidence_matrix.md) has been built, we have a
record of which unknowns appear in which equations. We now need to **assign one
unknown to each equation** so that every equation has a single variable it is
"responsible for" computing.

This pairing is what allows downstream phases to reason about evaluation order:
once an equation owns a variable, that variable's value can be thought of as
the *output* of that equation, and any other equation that mentions the same
variable becomes *dependent* on the owning equation.

Concretely, the matching feeds two later steps:

1. **Tarjan's SCC algorithm** uses the assignment to build a directed
   dependency graph between equations.
2. **BLT ordering and tearing** use the SCC partition to produce an evaluation
   sequence.

Without an assignment, there is no way to say "equation A depends on equation
B" — both equations might mention the same variable, and there would be no
canonical way to decide which equation produces it and which consumes it.

---

## The Underlying Mathematical Object: A Bipartite Graph

The incidence matrix is most usefully viewed as a **bipartite graph**:

- One set of vertices = equations (rows of the incidence matrix).
- The other set of vertices = unknowns (columns).
- An edge connects equation $e$ to unknown $v$ iff $v$ appears in $e$.

```
   equations           unknowns
     eq0  ────────────  v0
            ╲
             ╲────────  v1
     eq1  ──╱
           ╳───────────  v2
     eq2  ╱  ╲
                ────────  v3
```

A **matching** is a subset of these edges with the constraint that no two
chosen edges share a vertex. Intuitively: each equation is paired with at most
one unknown, and each unknown is claimed by at most one equation.

A **maximum matching** is a matching that contains as many edges as possible.

A **perfect matching** is one in which every equation is paired (and, if there
are equally many unknowns and equations, every unknown is claimed too). For a
well-posed continuous DAE, the matching must be perfect — otherwise the system
is *structurally singular*.

---

## Why Greedy Doesn't Work

The first instinct is greedy: run equations in order, and for each equation
pick any unknown that is still free. This often produces a matching, but not
always a *maximum* one. Here is a small example that defeats greedy assignment:

```
eq0  references  {v0, v1}
eq1  references  {v0}
```

A greedy run in order:
1. eq0 sees v0 free, claims v0.
2. eq1 sees v0 taken (its only option) — fails.

Result: matching of size 1. But a perfect matching of size 2 exists: assign
eq0 → v1 and eq1 → v0.

The fix is that when a later equation gets stuck, it must be allowed to
**displace** an earlier equation's assignment, provided the earlier equation
can find a replacement. That displacement chain is called an **augmenting
path**.

---

## Augmenting Paths

An **augmenting path** in the current partial matching is an alternating
sequence

$$ e_0 - v_0 - e_1 - v_1 - e_2 - v_2 - \dots - v_k $$

where:

- $e_0$ is an unmatched equation (the one we are currently trying to pair).
- Edges of the form $e_i - v_i$ are *unmatched* edges (incidence-matrix entries
  not currently chosen).
- Edges of the form $v_i - e_{i+1}$ are *matched* edges (currently chosen).
- $v_k$ is an unmatched unknown (free).

If we toggle every edge along this path — turn the unmatched edges *into*
matched edges, and the matched edges into unmatched ones — three things happen:

1. The constraint "no shared vertex" is preserved (each equation/unknown
   participates in exactly one edge on the path before and after).
2. Every equation $e_i$ on the path is still matched (just to a different
   unknown).
3. The previously-unmatched equation $e_0$ becomes matched, and the previously-
   unmatched unknown $v_k$ becomes claimed.

So the matching grows by exactly **one**.

A foundational theorem of matching theory (König / Berge) states:

> A matching is maximum if and only if no augmenting path exists.

This gives a direct algorithm:

```
start with the empty matching
while some augmenting path exists:
    find one and toggle it
```

Each iteration grows the matching by 1, and there are at most `min(n_eq, n_var)`
iterations, so the loop terminates quickly.

---

## Kuhn's Algorithm

*Verified 2026-07-30 against `crates/rumoca-phase-structural/src/matching.rs`* — read while building the animation. `MatchingStep::EquationFailed` records the give-up, which is what makes a rank deficiency watchable rather than merely reported.

Kuhn's algorithm is the classical implementation of the above idea, and it is
exactly what `matching.rs` does. It iterates over equations in index order,
and for each unmatched equation, performs a depth-first search to find an
augmenting path starting at that equation. If one is found, the matching is
toggled along the path; if not, that equation simply remains unmatched (and
will end up in the unmatched-equations diagnostic if the matching turns out to
be imperfect).

The clever part of Kuhn's algorithm is that it does **not** explicitly build or
run the augmenting path. Instead, the path is encoded implicitly in the call
stack of a recursive function: each level of recursion represents one "step"
along the path, and when the recursion reaches a free unknown, the assignment
cascade on the way back up performs the toggling.

---

## The Code, Step by Step

Here is the recursive helper from `matching.rs`, annotated:

```rust
fn augment(
    eq: usize,                            // the equation we are trying to pair
    match_eq:  &mut [Option<usize>],      // match_eq[i]  = Some(j) if eq i is matched to var j
    match_var: &mut [Option<usize>],      // match_var[j] = Some(i) if var j is matched to eq i
    eq_vars:   &[HashSet<usize>],         // eq_vars[i] = unknowns referenced by eq i (the incidence)
    visited:   &mut [bool],               // visited[v] = true if we've already tried var v on this DFS
) -> bool {
    let mut vars: Vec<usize> = eq_vars[eq].iter().copied().collect();
    vars.sort_unstable();
    for var in vars {
        if !visited[var] {
            visited[var] = true;
            let can_augment = match match_var[var] {
                None => true,
                Some(matched_eq) =>
                    augment(matched_eq, match_eq, match_var, eq_vars, visited),
            };
            if can_augment {
                match_eq[eq]   = Some(var);
                match_var[var] = Some(eq);
                return true;
            }
        }
    }
    false
}
```

### What each piece does

**The two arrays `match_eq` and `match_var`** are the matching itself, stored
as a pair of inverse mappings. They are kept in sync — every assignment writes
to both. Storing both directions lets us answer two questions in O(1):

- "Which unknown is equation `e` matched to?" — `match_eq[e]`
- "Which equation owns unknown `v`?" — `match_var[v]`

We need both, because the augmenting search has to look up the current owner
of a candidate unknown to decide whether to displace it.

**The `visited` array** is the standard DFS guard. Within one top-level call,
each unknown is examined at most once. Without this guard the recursion could
visit the same unknown along two different chains and loop forever (or simply
do redundant work). It is reset to all-`false` at the top of each new outer
iteration in `maximum_matching`, because each new starting equation deserves a
fresh search.

**The body of the loop**, for each candidate unknown `var`:

1. **`if !visited[var]`** — skip unknowns we've already tried in this DFS.
2. **`visited[var] = true`** — mark before recursing, so the recursive call
   doesn't try the same unknown again from below.
3. **The `match match_var[var]`** checks the current owner of `var`:
   - **`None`** → `var` is free. We've found the end of an augmenting path:
     just return `true` and let the caller perform the assignment.
   - **`Some(matched_eq)`** → `var` already belongs to `matched_eq`. Recurse:
     ask whether `matched_eq` can find a *different* unknown to claim instead.
     If yes, the recursive call will have already updated `match_eq[matched_eq]`
     and `match_var[var]` is about to be overwritten by us. If no, we cannot
     use `var` and must try the next candidate.
4. **`if can_augment`** → either the unknown was free, or its owner found
   another option. In both cases, the augmenting path continues through `eq`
   to `var`. We claim `var` for `eq` (overwriting whatever was there) and
   return `true`.

The unwinding of the recursion is what performs the "toggle every edge along
the path" operation. Each recursive return assigns one new (eq, var) pair,
overwriting the previous owner of that var. The cumulative effect is exactly
the toggle — every edge on the path flips its matched/unmatched status.

### How the implicit augmenting path corresponds to the recursion

Imagine `augment(eq0)` is called. It picks unknown `v0`, finds it owned by
`eq1`, and recursively calls `augment(eq1)`. That call picks `v1`, finds it
owned by `eq2`, and recurses again into `augment(eq2)`. That call picks `v2`,
which is free, and returns `true`.

The augmenting path implied by this recursion is

```
eq0 --(unmatched edge)--> v0 --(matched edge)--> eq1
    --(unmatched edge)--> v1 --(matched edge)--> eq2
    --(unmatched edge)--> v2  (free)
```

As the recursion unwinds:

- `augment(eq2)` returns `true` after setting `match_eq[eq2] = Some(v2)` and
  `match_var[v2] = Some(eq2)`.
- `augment(eq1)` then sets `match_eq[eq1] = Some(v1)` and `match_var[v1] = Some(eq1)`.
- `augment(eq0)` finally sets `match_eq[eq0] = Some(v0)` and `match_var[v0] = Some(eq0)`.

Every edge along the path has been toggled, and the matching has grown by 1.

---

## The Outer Driver

```rust
pub(crate) fn maximum_matching(
    n_eq: usize,
    n_var: usize,
    eq_vars: &[HashSet<usize>],
) -> (Vec<Option<usize>>, Vec<Option<usize>>) {
    let mut match_eq:  Vec<Option<usize>> = vec![None; n_eq];
    let mut match_var: Vec<Option<usize>> = vec![None; n_var];

    for eq in 0..n_eq {
        let mut visited = vec![false; n_var];
        augment(eq, &mut match_eq, &mut match_var, eq_vars, &mut visited);
    }

    (match_eq, match_var)
}
```

This is Kuhn's outer loop. For each equation in order, it allocates a fresh
`visited` array and calls `augment`. If `augment` returns `false`, that
equation simply stays unmatched and contributes to a structural-singularity
diagnostic later. The loop never reverses — once an equation is matched (even
indirectly via displacement), it stays matched, although the *which-variable*
of its assignment may change as later equations augment through it.

The complexity is $O(V \cdot E)$ where $V$ is the number of equations and $E$
is the total number of incidence entries. For the sparse models that Modelica
produces, this is effectively linear.

---

## Why the Sort Matters: Determinism

The two lines

```rust
let mut vars: Vec<usize> = eq_vars[eq].iter().copied().collect();
vars.sort_unstable();
```

deserve careful attention. The incidence is stored as `Vec<HashSet<usize>>`,
and `HashSet`'s iteration order is **not deterministic** across runs — Rust's
default hasher uses a random seed per process. If the algorithm iterated the
hash set directly, the order in which candidate unknowns are tried would
differ between runs.

That non-determinism would not affect *whether* a maximum matching exists, but
it would change *which* maximum matching is found when multiple maxima are
valid. Since the matching feeds Tarjan, BLT, and tearing, any change in the
matching cascades into a different evaluation order and different generated
code. Reproducible builds and stable golden-output tests both require a
deterministic matching.

The fix is to copy the candidates into a `Vec` and sort by integer index
before iterating. Sort order is the unknown's column index, which is stable
because `build_unknown_map` assigns indices deterministically.

The test `test_maximum_matching_is_deterministic_under_ties` in `matching.rs`
pins this behaviour:

```rust
// Both equations reference {v0, v1}.
// Deterministic order: eq0 visits v0 first, claims it; eq1 augments
// through eq0 (which then takes v1), so eq1 → v0, eq0 → v1.
let eq_vars = vec![HashSet::from([0, 1]), HashSet::from([0, 1])];
let (match_eq, match_var) = maximum_matching(2, 2, &eq_vars);
assert_eq!(match_eq,  vec![Some(1), Some(0)]);
assert_eq!(match_var, vec![Some(1), Some(0)]);
```

Without the sort, this assertion would fail intermittently.

---

## Worked Example: Walking an Augmenting Path

Take the incidence

```
eq0 references {v0, v1}
eq1 references {v0, v2}
eq2 references {v1}
```

State the empty matching: `match_eq = [None, None, None]`, `match_var = [None, None, None]`.

**Iteration 1, `augment(eq0)`** with `visited = [F, F, F]`:

- Sorted candidates: `[0, 1]`. Try `v0`: unvisited → mark visited.
  `match_var[0] = None` → free → return `true`. Assign `match_eq[0] = Some(0)`,
  `match_var[0] = Some(0)`.

Matching after: `match_eq = [Some(0), None, None]`, `match_var = [Some(0), None, None]`.

**Iteration 2, `augment(eq1)`** with `visited = [F, F, F]`:

- Sorted candidates: `[0, 2]`. Try `v0`: mark visited.
  `match_var[0] = Some(0)` → owned by `eq0`. Recurse into `augment(eq0)`.
  - Sorted candidates of `eq0`: `[0, 1]`. Try `v0`: already visited, skip.
    Try `v1`: mark visited. `match_var[1] = None` → free → return `true`.
    Assign `match_eq[0] = Some(1)`, `match_var[1] = Some(0)`.
  - `augment(eq0)` returns `true`.
  Back in the outer call: `can_augment = true`. Assign `match_eq[1] = Some(0)`,
  `match_var[0] = Some(1)`. Return `true`.

Matching after: `match_eq = [Some(1), Some(0), None]`, `match_var = [Some(1), Some(0), None]`.

The augmenting path that just executed was $eq_1 - v_0 - eq_0 - v_1$: it
displaced `eq0` from `v0` (forcing it to take `v1`) and made room for `eq1`
to take `v0`.

**Iteration 3, `augment(eq2)`** with `visited = [F, F, F]`:

- Sorted candidates: `[1]`. Try `v1`: mark visited.
  `match_var[1] = Some(0)` → owned by `eq0`. Recurse into `augment(eq0)`.
  - Sorted candidates of `eq0`: `[0, 1]`. Try `v0`: mark visited.
    `match_var[0] = Some(1)` → owned by `eq1`. Recurse into `augment(eq1)`.
    - Sorted candidates of `eq1`: `[0, 2]`. Try `v0`: already visited, skip.
      Try `v2`: mark visited. `match_var[2] = None` → free → return `true`.
      Assign `match_eq[1] = Some(2)`, `match_var[2] = Some(1)`.
    - `augment(eq1)` returns `true`.
    Back in `augment(eq0)`: assign `match_eq[0] = Some(0)`, `match_var[0] = Some(0)`.
    Return `true`.
  - Try `v1`: already visited (we marked it before recursing into `eq0`), skip.
  Wait — actually re-trace: in `augment(eq0)` we marked `v0` visited *and then*
  recursed; the inner call returned `true` so we never reach `v1`. `augment(eq0)`
  returns `true`.
  Back in `augment(eq2)`: assign `match_eq[2] = Some(1)`, `match_var[1] = Some(2)`.
  Return `true`.

Final matching: `match_eq = [Some(0), Some(2), Some(1)]`,
`match_var = [Some(0), Some(2), Some(1)]`. Perfect — three equations matched
to three unknowns.

The augmenting path on this iteration was $eq_2 - v_1 - eq_0 - v_0 - eq_1 - v_2$:
two displacements, one new pairing, matching grew by one.

---

## Observed Under the Debugger

*Measured 2026-08-08 by stepping `ProportionalLoop` live in `cppvsdbg`, reading
`.hrw-bridge/debug-state.json` at every stop (`docs/ideas.md` #73).* Everything above this section
is derived from reading the source; everything in it was read off a running stack.

**This is the traced twin, `augment_traced`**, not the `augment` shown above — same algorithm with
`emit_matching_frame` calls interleaved. The emit sites are what make a live session legible,
because at the breakpoint anchor every stop looks identical:

> **The emit-site table and the per-specimen ledgers live in
> [`matching-live-reference.md`](matching-live-reference.md), which is GENERATED.**
> They were written out here until 2026-08-08 and deliberately are not any more: **a line number
> written in two places goes stale in one of them**, and `EquationFailed` had already been
> published as 137 when the call is at 133. `matching_ledger.rs` derives every number from
> `matching.rs` *as compiled*, and `the_generated_reference_is_current` fails — naming the
> regeneration command and the first line that moved — the moment the code shifts.
> **Cite that file; do not copy from it.**

Two properties of that table belong here rather than there, because they are facts about the
*algorithm* rather than about line numbers:

- **`TryEquation` and `EquationFailed` are emitted outside `augment_traced` entirely**, by the
  driver loop. A stop for either has **no `augment_traced` frame on the stack at all**, which is
  how you tell "starting an equation" from "exploring within one".
- **`DisplaceOk` and `DisplaceFail` share a single emit**, whose `step:` is an `if` expression.
  The site says *where*, never *which* — **a line number cannot tell you whether a displacement
  succeeded**, only the frame can.

**Recursion depth is directly readable as the number of `augment_traced` frames**, and the line
number of each *non-innermost* one is always **210** — the recursive call site. So an outer frame
parked at 210 is an equation that has been asked to move and has not yet answered.

### The claim "the call stack is the augmenting path" is exact, with a caveat on counting

Observed at depth 2: `augment_traced:181` (equation 0, probing unknown 2) over
`augment_traced:210` (equation 1, waiting) over `maximum_matching_with_trace:123`. The path is

$$ eq_1 - v_0 - eq_0 - v_2 $$

**Two frames, three edges.** In general **N frames = N equation-nodes, N unmatched edges,
N − 1 matched edges, 2N − 1 edges total** — matching the $e_0 - v_0 - e_1 - \dots - v_k$ form
above. Counting frames as edges is the easy error and it erases the alternation, which is the
content.

### `visited` is shared down the recursion, and that is visible

At the depth-2 stop, `visited = [true, false, true]` in equation 0's frame — **unknown 0 was
marked `true` by equation 1**, before it recursed (line 180 runs before line 210). So equation 0
re-runs its own search over the *same* candidate list `[0, 2]` it used the first time, skips
unknown 0 at line 177, and takes a different branch purely because the context changed. Without
that shared mark, equation 0 would reclaim unknown 0 from equation 1 and the two would displace
each other forever.

**That is the difference between a path and a cycle, and it is three booleans.**

### One `Assign` per return, which is the toggle happening

Rows 9 and 11 of the observed run are both `Assign`, at depths 2 and 1, with `DisplaceOk` between
them. The inner frame commits `eq0 → v2` and returns `true`; the outer frame then commits
`eq1 → v0`. **Nothing runs a stored path** — confirming the claim above from a live stack rather
than from reading.

There is a moment mid-unwind when the two arrays genuinely disagree: after the inner `Assign`,
`match_eq[0]` is `Some(2)` while `match_var[0]` still says equation 0 owns unknown 0, until the
outer frame overwrites it. Consumers must not read the pair as consistent mid-recursion. *(Derived
from lines 231-232, not observed — an anchor stop exposes only `frame_index`.)*

### The failure path, and two steps that emit nothing at all

*Measured 2026-08-08 stepping `TwiceDefined` — two equations that both mention only `a`, so `b` is
reachable from nothing. Nine frames; the whole run fits in one run.*

**Two real algorithm steps never reach the frame stream.** Both are `augment_traced` returning
`false` from line 243 — the bare `false` after the `for` loop:

- **The inner one** is the displaced equation refusing. Equation 0 is asked to move, its only
  candidate is `a`, `visited[a]` is already `true`, so line 177 `continue`s and the loop ends
  **without reaching a single `emit_matching_frame`**. Observed at depth 2, stack `243 → 210 → 123`.
- **The outer one** is equation 1 exhausting its candidate list, at depth 1.

So a trace of a failing search reads `TryDisplace` → `DisplaceFail` → `EquationFailed` with
nothing between, while the algorithm made two decisions in the gaps. **Anything reasoning from the
frame stream alone will under-report a failure**; only the call stack shows the refusal.

**Their debugger signature is `var` and `iter` both reading unavailable.** Inside the loop both are
live; after it, they are gone — which distinguishes *"returning from inside the loop"* from
*"fell out of it"*.

### Alternating versus augmenting, as two stacks

The same depth-2 stack shape means opposite things, and the difference is the innermost line:

| | `ProportionalLoop` (succeeds) | `TwiceDefined` (fails) |
|---|---|---|
| stack | `181 → 210 → 123` | `243 → 210 → 123` |
| path | eq1 → a → eq0 → **free variable** | eq1 → a → eq0 → **dead end** |
| | **augmenting** | merely **alternating** |
| the unwind | commits one edge per frame | commits nothing |

The theorem quoted above — *a matching is maximum iff no augmenting path exists* — is exactly this
distinction. Alternating paths are easy to find; the terminal condition is the whole content.

**And a failed search is side-effect-free on the matching.** `match_eq` and `match_var` are
identical before and after equation 1's attempt. Only `visited` is mutated, and it is reset per
equation at line 122 — which is why the outer loop never has to backtrack or repair.

### Hall's condition, visible as an empty column

`TwiceDefined`'s incidence has **2 entries in a 2×2, both in column `a`**. Column `b` is empty, so
no permutation can place a nonzero on its diagonal position — the failure is not a search
shortcoming but an impossibility.

**Hall's marriage theorem** states it exactly: a perfect matching exists iff |N(S)| ≥ |S| for every
set S of equations. Here S = {eq0, eq1} and N(S) = {a}, so 1 < 2. Such an S is a *Hall violator*,
and it is what the compiler reports as the unmatched pair.

**`b` is never visited by any frame** — `visited[1]` stays `false` for the entire run, because no
equation mentions it. The unmatched *unknown* is diagnosed by absence at the end, never discovered
by the search, which is why `unmatched_equations` and `unmatched_unknowns` are reported together
rather than derived from the trace.

---

## Determining Structural Singularity

After `maximum_matching` returns, the caller in `lib.rs` counts how many
equations were matched. If that count is less than `min(n_eq, n_var)`, the
system is structurally singular:

- More equations than the matching covers ⇒ overdetermined: at least one
  equation has no unknown to "own".
- Unmatched unknowns ⇒ underdetermined: at least one unknown is never the
  output of any equation.

The caller produces a `StructuralError::Singular { unmatched_equations,
unmatched_unknowns, … }` with both lists by name, so the modeler can identify
the offending pair.

The test `test_maximum_matching_imperfect` in `matching.rs` exercises this
case:

```rust
// eq0 and eq1 both reference {v0}; only one of them can win.
// eq2 freely takes v1 or v2.
let eq_vars = vec![
    HashSet::from([0]),
    HashSet::from([0]),
    HashSet::from([1, 2]),
];
let (match_eq, _match_var) = maximum_matching(3, 3, &eq_vars);
let size = match_eq.iter().filter(|m| m.is_some()).count();
assert_eq!(size, 2);   // eq0 or eq1 plus eq2; the other stays unmatched.
```

No augmenting path can rescue the loser of the eq0/eq1 contest because there
is nowhere for it to go — `v0` is its only neighbour.

---

## Summary

- The matching step pairs each continuous equation with exactly one of the
  unknowns it references, so that downstream phases can speak of "the equation
  responsible for variable v".
- The bipartite graph comes directly from the incidence matrix; the goal is
  a maximum (ideally perfect) matching.
- Greedy assignment is insufficient because later equations may need to
  displace earlier ones; Kuhn's augmenting-path algorithm handles displacement.
- The recursive `augment` function is a depth-first search whose call stack
  encodes the augmenting path; the assignment performed during stack unwinding
  is exactly the path-toggling that grows the matching.
- Sorting the per-equation candidate list before iterating is essential for
  determinism, because `HashSet` iteration order is process-randomised.
- An imperfect matching signals structural singularity, which is reported with
  the names of the unmatched equations and unknowns so the modeler can fix the
  source.
