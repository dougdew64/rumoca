# Drill-Down: Tarjan's Strongly-Connected-Components Algorithm

*Parent document: [structural_analysis.md](structural_analysis.md)*
*Source: [crates/rumoca-phase-structural/src/tarjan.rs](../../../crates/rumoca-phase-structural/src/tarjan.rs)*

---

## What Problem Does This Step Solve?

After the [matching](maximum_bipartite_matching.md) step, every equation owns
exactly one unknown. The next question is: **in what order should the equations
be evaluated?**

Most of the time the answer is "in dependency order" — if equation A's owned
variable is referenced by equation B, evaluate A first, then B. But sometimes
the dependencies form a cycle: A's variable appears in B, B's variable appears
in C, and C's variable appears in A. Such a cycle is an **algebraic loop**:
the three equations must be solved *simultaneously* because no one of them can
be evaluated first.

So before producing an evaluation order, the solver must:

1. Find every cycle in the dependency graph (and group its equations together
   for joint solving).
2. Order the resulting groups so that all dependencies of group $k$ come before
   group $k$.

These two questions are answered jointly by computing the **strongly
connected components** of the dependency graph, in **reverse topological
order**. That is exactly what Tarjan's SCC algorithm produces in a single
linear-time pass.

---

## Definitions

### The Dependency Graph

The dependency graph is a directed graph built from the matching:

- Nodes are equations.
- Add an edge $A \to B$ iff equation $A$ references a variable that equation $B$
  is matched to (i.e., $B$ is the equation responsible for producing that
  variable).

Edge direction in this code: **dependent points to dependency** (consumer
points to producer). $A \to B$ reads as "$A$ depends on $B$."

### Strongly Connected Component

A strongly connected component (SCC) of a directed graph is a maximal subset
of nodes $S$ such that for every pair $(u, v) \in S$ there is a directed path
from $u$ to $v$ *and* from $v$ to $u$.

Operationally, two nodes are in the same SCC iff they can reach each other.
Singleton nodes (with no self-loop) are SCCs of size 1.

For a dependency graph, an SCC of size > 1 is exactly an **algebraic loop**:
every equation in the SCC depends (directly or transitively) on every other
one, so they cannot be ordered.

### Topological Order on the Condensation

Collapse each SCC to a single super-node. The resulting graph (the
**condensation**) is acyclic by construction (any cycle would have been inside
an SCC). An acyclic directed graph admits a topological order — a linear order
in which every edge points from earlier to later.

For our dependency graph, the topological order on the condensation places
*producers* before *consumers* (dependencies first), which is exactly the
order in which equation blocks must be evaluated.

---

## Why Tarjan?

Several SCC algorithms exist; the two most common are Kosaraju's (two DFS
passes plus a graph reversal) and Tarjan's (single DFS pass). Both are
$O(V + E)$. Two practical advantages make Tarjan the right choice here:

1. **Single pass.** No need to build the reversed graph, no extra storage for
   it.
2. **Output order is already correct.** Tarjan emits each SCC at the moment its
   "root" is identified during DFS. Because of how the DFS unwinds, this
   produces SCCs in **reverse topological order of the condensation** — i.e.,
   *dependencies first*. That is precisely the BLT evaluation order, with no
   reordering or reversal needed.

---

## Index, Lowlink, and the Stack

Three pieces of bookkeeping carry the algorithm. They are easy to confuse, so
take a moment with each.

### `index[v]`

A monotonic counter assigned the first time the DFS visits node $v$. It is
the node's *DFS discovery time*. Indices are unique and never change after
assignment.

In the code, `index` is `Vec<Option<usize>>`; `None` marks unvisited nodes.

### `lowlink[v]`

The smallest `index` value reachable from $v$ via any sequence of forward
edges followed by *at most one* back edge to a node still on the DFS stack.
Equivalently: the index of the oldest ancestor (in DFS terms) that $v$ can
reach.

Initially `lowlink[v] = index[v]`. As the DFS proceeds, `lowlink[v]` is
lowered whenever a child or a back-edge target proves that $v$ can reach an
older node.

The crucial property: at the moment DFS finishes exploring $v$,
`lowlink[v] == index[v]` iff $v$ is the **root** of a strongly connected
component — that is, $v$ cannot reach any older node still on the stack, so
$v$ together with the nodes on the stack above it form one SCC.

### The DFS stack

A separate stack (not the recursion stack) holds nodes whose SCC has not yet
been emitted. A node is pushed when first visited. It is popped — together
with everything above it — when an SCC root is identified. The companion
`on_stack` boolean array makes "is this node on the stack?" an $O(1)$ check.

Why is `on_stack` separate from "visited"? A visited node may already have been
emitted as part of an earlier SCC; in that case it is *no longer on the
stack*, and an edge into it tells us nothing about cycles. Only edges into
nodes still on the stack indicate that we're closing a cycle.

---

## The Code

The implementation in
[tarjan.rs](../../../crates/rumoca-phase-structural/src/tarjan.rs) closely mirrors
the textbook formulation. Here is the entry point:

```rust
pub(crate) fn tarjan_scc(n: usize, adj: &[Vec<usize>]) -> Vec<Vec<usize>> {
    let mut state = TarjanState::new(n);
    for v in 0..n {
        if state.index[v].is_none() {
            state.strongconnect(v, adj);
        }
    }
    state.sccs
}
```

The outer loop ensures every node gets visited even if the graph has multiple
DFS roots (i.e., is not a single connected component).

The recursive heart is `strongconnect`:

```rust
fn strongconnect(&mut self, v: usize, adj: &[Vec<usize>]) {
    self.index[v]   = Some(self.index_counter);
    self.lowlink[v] = self.index_counter;
    self.index_counter += 1;
    self.stack.push(v);
    self.on_stack[v] = true;

    for &w in &adj[v] {
        if self.index[w].is_none() {
            self.strongconnect(w, adj);
            self.lowlink[v] = self.lowlink[v].min(self.lowlink[w]);
        } else if self.on_stack[w] {
            self.lowlink[v] = self.lowlink[v].min(self.index[w].expect(...));
        }
    }

    if self.lowlink[v] == self.index[v].expect(...) {
        self.pop_scc(v);
    }
}
```

### Step by step

1. **First-visit bookkeeping.** Assign `index[v]` and `lowlink[v]` from the
   counter; push $v$ onto the SCC stack; mark `on_stack[v]`.

2. **Iterate edges.** For each successor $w$ of $v$ (recall: an edge $v \to w$
   means $v$ depends on $w$):

   - **Tree edge** (`index[w] is None`): $w$ has not been visited yet. Recurse
     into it. After the recursive call returns, fold `lowlink[w]` into
     `lowlink[v]` — anything reachable from $w$ is also reachable from $v$
     through the tree edge.
   - **Back edge to a node on the stack** (`on_stack[w]`): we've found a path
     back to an ancestor. Use `index[w]`, not `lowlink[w]`, to update
     `lowlink[v]`. The classical formulation allows either, but using `index`
     is slightly safer and easier to reason about, and is what the code does.
   - **Cross edge into a finished SCC** (visited but not on stack): ignore it.
     That SCC has already been emitted, and there is no cycle to close.

3. **SCC root test.** When all of $v$'s outgoing edges have been processed, if
   `lowlink[v] == index[v]`, then $v$ is the root of an SCC. The SCC consists
   of $v$ and everything pushed onto the stack since $v$ was. Pop them off and
   record them as one component.

### `pop_scc`

```rust
fn pop_scc(&mut self, root: usize) {
    let mut scc = Vec::new();
    loop {
        let w = self.stack.pop().expect(...);
        self.on_stack[w] = false;
        scc.push(w);
        if w == root { break; }
    }
    self.sccs.push(scc);
}
```

This is bookkeeping: pop until you've reached the root inclusive, mark each
popped node as no longer on the stack, and record the SCC. Importantly,
`on_stack[w]` is cleared exactly when $w$ leaves the stack, so future edges
into $w$ are correctly classified as cross-edges into a finished SCC.

---

## Why the Output Order Is Already Reverse-Topological

Tarjan emits SCCs in the order their roots finish DFS. The deepest SCCs in the
dependency graph (the ones with no outgoing edges to nodes outside the SCC)
finish first; the shallowest (those that depend on others) finish last.

Because edges in our dependency graph point from *consumer* to *producer*
(dependent → dependency), the producers are the "leaves" of the DFS, and they
finish first. So the first SCC out of Tarjan is the most-depended-upon block,
and the last SCC is the one nothing else depends on.

That is exactly the order you'd want to evaluate blocks in: producers before
consumers. No reversal is needed before handing the SCC list to the BLT
constructor.

`blt.rs` documents this directly:

```rust
// Tarjan emits SCCs in reverse topological order of the condensation DAG.
// Since dependency edges point from dependent → dependency, this output
// order is already the correct BLT evaluation order (dependencies first).
```

---

## Worked Example

Consider a small dependency graph with four equations:

```
adj[0] = [1]      // eq0 depends on eq1
adj[1] = [2]      // eq1 depends on eq2
adj[2] = [0]      // eq2 depends on eq0   (cycle: 0 ↔ 1 ↔ 2)
adj[3] = []       // eq3 depends on nothing
```

Visualised:

```
   eq0 ─→ eq1 ─→ eq2
    ↑              │
    └──────────────┘

   eq3 (isolated)
```

Tracing `tarjan_scc(4, adj)`:

1. **Outer loop, v = 0** — unvisited, call `strongconnect(0)`.
   - `index[0]   = 0`, `lowlink[0] = 0`. Stack = `[0]`. `on_stack = [T, F, F, F]`.
   - Edge `0 → 1`. Unvisited; recurse.
     - `index[1]   = 1`, `lowlink[1] = 1`. Stack = `[0, 1]`.
     - Edge `1 → 2`. Unvisited; recurse.
       - `index[2]   = 2`, `lowlink[2] = 2`. Stack = `[0, 1, 2]`.
       - Edge `2 → 0`. Visited and on stack: back edge. Update
         `lowlink[2] = min(2, index[0]) = min(2, 0) = 0`.
       - Loop done. Test `lowlink[2] == index[2]`? `0 == 2`? **No** — not a
         root, do not pop.
       - Return from `strongconnect(2)`.
     - Back in `strongconnect(1)`: fold `lowlink[1] = min(1, lowlink[2]) = min(1, 0) = 0`.
     - Loop done. Test `lowlink[1] == index[1]`? `0 == 1`? **No** — not a root.
     - Return from `strongconnect(1)`.
   - Back in `strongconnect(0)`: fold `lowlink[0] = min(0, lowlink[1]) = min(0, 0) = 0`.
   - Loop done. Test `lowlink[0] == index[0]`? `0 == 0`? **Yes** — eq0 is a
     root. Call `pop_scc(0)`:
     - Pop 2, push to scc. Pop 1, push to scc. Pop 0, push to scc — equal to
       root, stop.
     - Emit SCC `[2, 1, 0]`.
   - Stack now empty.

2. **Outer loop, v = 1, v = 2** — already visited, skip.

3. **Outer loop, v = 3** — unvisited, call `strongconnect(3)`.
   - `index[3]   = 3`, `lowlink[3] = 3`. Stack = `[3]`.
   - No outgoing edges.
   - `lowlink[3] == index[3]`? Yes. Emit SCC `[3]`.

Final result: `sccs = [[2, 1, 0], [3]]`.

The first SCC is the algebraic loop (size 3); the second is the singleton
eq3. Reading the result as evaluation order: **first** solve the loop, **then**
evaluate eq3. This matches the dependency structure — eq3 depends on nothing,
so it can be solved any time, but the loop members must be solved together.

If the dependency graph had instead been `adj[3] = [0]` (eq3 depending on the
loop), the loop would still be emitted first and eq3 second, which is exactly
what BLT requires.

---

## Complexity

The algorithm performs $O(1)$ work per node and $O(1)$ work per edge, for a
total of $O(V + E)$ time and $O(V)$ space (for the index, lowlink, on_stack
arrays and the SCC stack). For sparse graphs from physical models, this is
effectively linear in the model size.

---

## Tests

Three tests in
[tarjan.rs](../../../crates/rumoca-phase-structural/src/tarjan.rs#L77-L106) pin the
expected behaviour:

```rust
// No cycles: every SCC is a singleton.
let adj  = vec![vec![1], vec![2], vec![]];
let sccs = tarjan_scc(3, &adj);
assert!(sccs.iter().all(|scc| scc.len() == 1));

// Single 3-cycle: one SCC containing all three nodes.
let adj  = vec![vec![1], vec![2], vec![0]];
let sccs = tarjan_scc(3, &adj);
let loops: Vec<_> = sccs.iter().filter(|scc| scc.len() > 1).collect();
assert_eq!(loops.len(), 1);
assert_eq!(loops[0].len(), 3);

// Two disjoint 2-cycles.
let adj  = vec![vec![1], vec![0], vec![3], vec![2]];
let sccs = tarjan_scc(4, &adj);
let loops: Vec<_> = sccs.iter().filter(|scc| scc.len() > 1).collect();
assert_eq!(loops.len(), 2);
```

These cover the three structural cases the BLT builder needs to distinguish:
all-scalar (no loops), one big loop, and multiple independent loops.

---

## Summary

- The dependency graph encodes "equation A needs equation B's variable" as a
  directed edge $A \to B$.
- Singleton SCCs in this graph are scalar blocks (one equation, one
  variable, evaluable directly).
- SCCs of size $> 1$ are algebraic loops that must be solved jointly (later,
  by [tearing](tearing.md) or coupled Newton/LM).
- Tarjan's algorithm finds all SCCs in a single $O(V + E)$ DFS using `index`,
  `lowlink`, and a separate SCC stack.
- The output ordering is already correct for BLT — dependencies first — so no
  post-processing is needed before producing the BLT block list.
