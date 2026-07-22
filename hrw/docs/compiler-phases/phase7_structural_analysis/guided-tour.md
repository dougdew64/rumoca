# Structural Analysis — Guided Tour

A five-lesson interactive walkthrough of structural analysis using HRW and
specimens. Each lesson introduces one concept, uses specific specimens, and tells
you exactly what to do in HRW to see the concept in action.

Lessons 2, 3, and 5 use a **three-tier progression** to build understanding:

1. **Static snapshot** — read the result of the algorithm (overlays on the
   incidence matrix, spy-plot blocks). Understand *what* the algorithm produced.
2. **Recorded replay** — play/step through a pre-recorded animation. Understand
   *how* the algorithm arrives at the result, one decision at a time.
3. **Live-stepped execution** — click "Debug", set a breakpoint in the Rust
   source, and step through the actual algorithm code in the VS Code debugger
   while HRW animates each step in lockstep. Map *every line of code* to its
   algorithmic meaning. This is where understanding becomes complete.

Start with the static snapshot, then advance to recorded replay, then use
live-stepped execution last.

**Prerequisites:** HRW built and running (`cargo run -p hrw` from the workspace
root). Familiarity with loading specimens (click a name in the left panel).
For tier 3 (live-stepped): VS Code with rust-analyzer and CodeLLDB installed.

---

## Lesson 1: The incidence matrix

**Concept:** Before a DAE solver can determine a solve order, it needs to know
which equations reference which unknowns. The **incidence matrix** is this
dependency map: rows = equations, columns = unknowns, a filled cell means "this
equation mentions this variable."

**Specimen:** `SingleInertia`

**In HRW:**
1. Load `SingleInertia`. Navigate to the **Structural** tab.
2. Switch to the **Incidence** sub-view.
3. You'll see a tiny 2x2 matrix — SingleInertia is a minimal ODE
   (`der(w) = tau/J`, `der(phi) = w`), so it has only 2 equations and 2
   unknowns. Each cell is filled, meaning every equation references every
   unknown (a dense matrix — unusual, but expected for a system this small).
4. **Hover** a filled cell. The tooltip shows which equation (row) references
   which unknown (column). Hover an empty cell — "no reference."
5. **Zoom in** (scroll wheel) until labels appear (zoom >= 16). Read the equation
   names on the left and the unknown names on top.
6. Notice the caption: "2x2 incidence ... 2/2 matched (full rank)." We'll
   learn what "matched" means in Lesson 2.
7. To see sparsity in action, load **RotationalInertia** (an MSL-based model
   of the same physics but with connectors and components). Its incidence is
   12x12 and visibly sparse — most cells are empty because each equation
   references only a few of the system's 12 unknowns.

**Key insight:** The incidence matrix is a **bipartite adjacency matrix** — one
set of nodes is equations, the other is unknowns, and edges connect equations
to the unknowns they reference. Every structural analysis algorithm operates
on this matrix.

**Linear algebra connection:** The incidence matrix is the structural analogue
of a numerical matrix's sparsity pattern. A zero in the incidence matrix means
the partial derivative dF_i/dx_j is structurally zero (not just numerically
small — the equation literally doesn't contain that variable). This is why
structural analysis is cheap: it reasons about the *pattern* of nonzeros, not
their values.

---

## Lesson 2: Maximum matching

**Concept:** A **maximum matching** assigns each equation to one unknown it can
"own" — the variable it will determine. This is the heart of structural analysis:
it decides which equation solves for which unknown. The number of matched pairs
is the **structural rank** of the system.

**Specimens:** `SingleInertia` (full rank) → `ProportionalLoop` (full rank, with
displacement)

### Tier 1 — Static snapshot: read the matching result

1. Load `SingleInertia`, Structural tab, Incidence view. The **green circles**
   on the incidence matrix mark the matched pairs — each equation has one green
   dot showing which unknown it "owns." This is the **transversal**.
2. Count the green dots — there should be 2 (one per equation). The caption
   confirms "2/2 matched (full rank)."
3. Load **RotationalInertia**. Its 12x12 incidence has 12 green dots — every
   equation found a partner. The diagonal pattern of the dots shows how sparse
   the assignment is: most equations own a nearby variable, not a distant one.

At this tier you see the *result* — which equation owns which unknown — but not
*how* the algorithm decided it.

### Tier 2 — Recorded replay: watch Kuhn's algorithm work

1. Load `SingleInertia`, Structural tab. Switch to the **Matching** sub-view
   (the tab labeled "Matching ▶"). This is the animated stepper.
2. Click **Step ▶** to advance frame by frame. Watch the algorithm work:
   - **Yellow highlight** = the equation currently searching for an augmenting
     path (`TryEquation`)
   - **Yellow border** = the edge being explored — "can I take this variable?"
     (`Explore`)
   - **Green flash** = success — "this variable is free, I'll take it!"
     (`FoundFree`)
   - **Green circle** = a confirmed match (`Assign`)
3. For SingleInertia, most equations find a free variable immediately — there's
   little conflict. Now load **ProportionalLoop** and run the matching animation.
   Watch for a **displacement**: one equation tries a variable that's already
   matched, so the holder must find an alternative. You'll see:
   - `TryDisplace` — the holder's row lights up as the algorithm recurses into
     it, asking "can you move to another variable?"
   - `DisplaceOk` — the holder found an alternative, so the displacement
     succeeds and both equations get reassigned
   - Or `DisplaceFail` — the holder has no alternative, and the algorithm
     backtracks to try a different variable

At this tier you see the *process* — the augmenting-path search, the
displacements, the backtracking — but the animation is pre-recorded. You can
step through it at your own pace.

### Tier 3 — Live-stepped execution: map the code to the algorithm

This tier connects each line of Rust to its algorithmic meaning. You step
through the actual `matching.rs` code in the debugger while HRW animates each
step in lockstep.

**Setup:**

1. **Launch HRW under the debugger** (F5 with the HRW launch config) — but
   **do not set any breakpoints yet**.
2. In the running HRW, load **ProportionalLoop**. Navigate to Structural →
   Matching sub-view. You should see the recorded animation (tier 2) and the
   **"Debug"** button.
3. **Now** open `crates/rumoca-phase-structural/src/live_trace.rs` in VS Code.
   Find the function `live_trace_breakpoint` (near the bottom of the file) and
   set a breakpoint on the `black_box` line inside it. This dedicated
   `#[inline(never)]` function is the unambiguous breakpoint site — do **not**
   break on `LiveTrace::push` (at higher opt-levels the debugger can confuse it
   with other `Vec::push` calls).
4. Click the **"Debug"** button. The algorithm thread spawns. After a brief
   per-frame delay (which lets the HRW UI render each frame), it hits your
   breakpoint.
5. Each time you **Continue** (F5) in the debugger, the algorithm advances one
   step: the frame is pushed, the UI renders it during the inter-frame delay,
   and then the breakpoint fires again. You can re-run by clicking "Debug"
   again after the session finishes.

**What to inspect at each pause:**

When the debugger pauses on `LiveTrace::push`, look at the `frame` parameter.
Its `.step` field is a `MatchingStep` enum variant — this tells you exactly what
the algorithm just decided. The `.match_eq` field is a snapshot of the current
partial matching.

**Walking through `maximum_matching_with_trace` (matching.rs):**

The outer function iterates over equations in order:

```
for eq in 0..n_eq {                        // try each equation in turn
    emit_matching_frame(TryEquation(eq));   // → you see this in the debugger
    augment_traced(eq, ...);               // DFS for an augmenting path
    if !found { emit_matching_frame(EquationFailed(eq)); }
}
```

Each `emit_matching_frame` call pushes to your `LiveTrace`, so the debugger
pauses there. The `TryEquation(eq)` frame tells you: "the algorithm is now
trying to match equation `eq`."

**Walking through `augment_traced` — the recursive augmenting-path search:**

This is the heart of Kuhn's algorithm. The function tries to match equation
`eq` by exploring each variable it references:

```rust
fn augment_traced(eq, match_eq, match_var, eq_vars, visited, frames, live) {
    let vars = eq_vars[eq];          // variables this equation references
    for var in vars {                // try each variable
        if !visited[var] {           // skip already-explored variables
            visited[var] = true;
```

At the breakpoint, inspect `eq` (which equation is searching) and `var` (which
variable it's trying). The `visited` array shows which variables have already
been explored in this round — the algorithm never revisits a variable.

**The three outcomes at each variable:**

```rust
match match_var[var] {
    None => {
        // Variable is FREE — augmenting path found!
        emit(FoundFree { eq, var });    // ← pause here: green flash
        true
    }
    Some(holder) => {
        // Variable is TAKEN by `holder` — try to displace
        emit(TryDisplace { eq, var, holder });  // ← pause here
        let ok = augment_traced(holder, ...);   // RECURSE into holder
```

This is the key moment to understand. When `match_var[var]` is `Some(holder)`,
the algorithm doesn't give up — it asks: "can `holder` find a *different*
variable?" That question is answered by *recursing* — calling `augment_traced`
on `holder`. At the breakpoint, inspect `holder` to see which equation is being
displaced, and watch the recursion unfold as a nested sequence of `Explore` →
`FoundFree`/`TryDisplace` frames.

```rust
        if ok {
            emit(DisplaceOk { eq, var });   // ← holder found alternative
        } else {
            emit(DisplaceFail { eq, var }); // ← holder stuck, backtrack
        }
        ok
    }
};
if can_augment {
    match_eq[eq] = Some(var);    // record the assignment
    match_var[var] = Some(eq);
    emit(Assign { eq, var });    // ← pause here: green circle
    return true;
}
```

When displacement succeeds, the algorithm *unwinds the recursion*, and each
returning call records an `Assign` — so you see a chain of assignments as the
augmenting path is "flipped." Watch `match_eq` at each `Assign` frame: you'll
see the partial matching change as each equation gets its new variable.

**Locals to inspect in the debugger:**

| Local | Type | What it tells you |
|-------|------|-------------------|
| `eq` | `usize` | Which equation is currently searching |
| `var` | `usize` | Which variable it's trying right now |
| `match_eq` | `&mut [Option<usize>]` | Current matching: `match_eq[i] = Some(j)` means eq i → var j |
| `match_var` | `&mut [Option<usize>]` | Reverse mapping: `match_var[j] = Some(i)` means var j ← eq i |
| `visited` | `&mut [bool]` | Variables already explored this round (prevents infinite loops) |
| `frame.step` | `MatchingStep` | The algorithmic decision just made |

**Key insight:** The matching algorithm is **Kuhn's algorithm** — for each
unmatched equation, it tries to find an **augmenting path**: a chain of
matched/unmatched edges that ends at a free variable. If found, it "flips" the
path (matched edges become unmatched and vice versa), gaining one more match.

**Linear algebra connection:** The matched pairs define a **permutation matrix**
P such that PAQ has a nonzero on every diagonal entry. This is exactly the
pivoting step in LU factorization — choosing which entry to put on the diagonal.
The structural rank equals the maximum number of pivots.

---

## Lesson 3: BLT decomposition

**Concept:** After matching, equations are grouped into **blocks** that must be
solved together. A **scalar block** is one equation that can be solved
independently. A **coupled block** (algebraic loop) is a set of equations that
mutually depend on each other — they must be solved simultaneously (typically by
Newton's method). The blocks are ordered so each block's inputs come from
earlier (already-solved) blocks. This is the **BLT (block lower triangular)
decomposition**.

**Specimens:** `ProportionalLoop` (one big coupled block) → `MixedLoop`
(scalar + coupled + scalar)

### Tier 1 — Static snapshot: read the BLT structure

1. Load `ProportionalLoop`, Structural tab, **Spy-plot** view. You'll see the
   BLT spy-plot: one orange box covering all 3 equations — the entire system is
   one coupled block (algebraic loop). Hover it to see the tearing info.
2. Switch to the **Incidence** view. Notice the **amber outlines** on the
   incidence matrix — these mark the same BLT blocks you saw in the spy-plot.
   For ProportionalLoop, there's one big amber rectangle.
3. Now load **MixedLoop**. In the spy-plot, you'll see a different structure:
   a scalar block, then a coupled block, then another scalar block. The
   topological ordering means the first scalar feeds into the coupled block,
   which feeds into the last scalar.

At this tier you see the *result* — which equations are coupled, how many blocks
there are — but not how the algorithm found them.

### Tier 2 — Recorded replay: watch Tarjan's algorithm discover SCCs

1. Load **MixedLoop**, Structural tab. Switch to the **BLT ▶** animation tab.
2. Click **Step ▶** or **Play ▶** to watch Tarjan's algorithm discover the
   strongly connected components:
   - `Visit` — a node gets its DFS index and is pushed onto the stack
     (**yellow** highlight)
   - `TreeEdge` — DFS goes deeper along an unvisited edge (**green** edge)
   - `BackEdge` — an edge points to a node already on the stack, revealing a
     cycle (**red** edge). This is the moment a coupled block is detected: the
     back edge means "this equation depends on something that depends on me."
   - `Return` — DFS backtracks, updating `lowlink` (the lowest reachable
     index). When `lowlink[v] == index[v]`, node `v` is an SCC root.
   - `SccFound` — an SCC is popped off the stack. Its nodes **light up** in a
     distinct color. Singleton SCCs = scalar blocks; multi-node SCCs = coupled
     blocks (algebraic loops).
3. Compare `ProportionalLoop` (all nodes in one SCC — the entire system is
   coupled) vs `MixedLoop` (three SCCs of different sizes).

At this tier you see the DFS exploration, the back edges that reveal cycles,
and the moment each SCC is identified — the *process* of Tarjan's algorithm.

### Tier 3 — Live-stepped execution: map the code to the algorithm

This tier connects each line of Tarjan's DFS to its textbook description.

**Setup:**

1. **Launch HRW under the debugger** (F5) — **do not set any breakpoints yet**.
2. In the running HRW, load **MixedLoop**. Navigate to Structural → BLT
   sub-view.
3. **Now** set a breakpoint on the `live_trace_breakpoint` function in
   `crates/rumoca-phase-structural/src/live_trace.rs` (same site as matching).
4. Click **"Debug"**. The algorithm thread spawns, and after the inter-frame
   delay, hits your breakpoint.
5. **Continue** (F5) to advance one frame at a time. HRW updates between
   each pause.

**Walking through `tarjan_scc_with_trace` (tarjan.rs):**

The outer function drives the DFS:

```rust
fn tarjan_scc_with_trace(n, adj, live) {
    let mut state = TracedTarjanState::new(n, live);
    for v in 0..n {                     // visit every node
        if state.index[v].is_none() {   // skip already-visited
            state.strongconnect(v, adj);
        }
    }
}
```

The DFS starts from each unvisited node. Most nodes are reached by recursion
from the first `strongconnect` call, so this loop usually only fires once.

**Walking through `strongconnect` — the recursive DFS:**

This is Tarjan's algorithm. Each call processes one node `v`:

```rust
fn strongconnect(&mut self, v: usize, adj: &[Vec<usize>]) {
    // Assign the next DFS index to this node
    self.index[v] = Some(self.index_counter);
    self.lowlink[v] = self.index_counter;
    self.index_counter += 1;

    // Push onto Tarjan's stack
    self.stack.push(v);
    self.on_stack[v] = true;

    self.record(TarjanStep::Visit(v));    // ← first pause: node v discovered
```

At the breakpoint, inspect `v` (which node), `self.index_counter` (the DFS
discovery order), and `self.stack` (the current DFS path). The `Visit` frame
corresponds to the textbook "assign index and lowlink, push onto stack."

**Exploring edges — tree edges vs back edges:**

```rust
    for &w in &adj[v] {
        self.record(TarjanStep::ExploreEdge { from: v, to: w });

        if self.index[w].is_none() {
            // w is UNVISITED — this is a TREE EDGE
            self.record(TarjanStep::TreeEdge { from: v, to: w });
            self.strongconnect(w, adj);   // RECURSE deeper into the DFS

            // Back from recursion: inherit w's lowlink if it reaches higher
            self.lowlink[v] = self.lowlink[v].min(self.lowlink[w]);
            self.record(TarjanStep::Return { from: v, to: w });

        } else if self.on_stack[w] {
            // w is ON THE STACK — this is a BACK EDGE (cycle!)
            self.lowlink[v] = self.lowlink[v].min(self.index[w].unwrap());
            self.record(TarjanStep::BackEdge { from: v, to: w });
        }
    }
```

This is the core of Tarjan's insight. At a `TreeEdge` pause, you're about to
recurse — the DFS goes deeper. At a `BackEdge` pause, you've found a cycle:
node `w` is already on the stack, meaning there's a path from `w` to `v` (the
DFS path) *and* an edge from `v` back to `w`. These two paths form a cycle —
every node on the path between `w` and `v` must be in the same SCC.

Inspect `self.lowlink[v]` at each pause. The `lowlink` is the lowest DFS index
reachable from `v` — when a back edge is found, `lowlink` drops to
`self.index[w]`, propagating the "I can reach something higher" information
up the recursion.

**SCC detection — the lowlink == index test:**

```rust
    // After exploring all edges from v:
    if self.lowlink[v] == self.index[v].unwrap() {
        // v is an SCC ROOT — pop everything above v off the stack
        let mut scc = Vec::new();
        loop {
            let w = self.stack.pop().unwrap();
            self.on_stack[w] = false;
            scc.push(w);
            if w == v { break; }
        }
        self.record(TarjanStep::SccFound { root: v, members: scc.clone() });
        self.sccs.push(scc);
    }
```

When `lowlink[v] == index[v]`, it means no node in `v`'s subtree can reach
anything discovered *before* `v` — so `v` and everything above it on the stack
form a complete SCC. At the `SccFound` pause, inspect `scc` to see which nodes
are in this component. A singleton = scalar block; multiple nodes = coupled
block (algebraic loop).

**Locals to inspect in the debugger:**

| Local | Type | What it tells you |
|-------|------|-------------------|
| `v` | `usize` | Current node being processed |
| `w` | `usize` | Neighbor being explored |
| `self.index[v]` | `Option<usize>` | DFS discovery order of node v |
| `self.lowlink[v]` | `usize` | Lowest DFS index reachable from v's subtree |
| `self.stack` | `Vec<usize>` | Current DFS/Tarjan stack (nodes in potential SCC) |
| `self.on_stack[w]` | `bool` | Is node w on the stack? (true → back edge → cycle) |
| `self.sccs` | `Vec<Vec<usize>>` | SCCs found so far |
| `frame.step` | `TarjanStep` | The algorithmic event just recorded |

**Key insight:** The BLT decomposition converts a large system into a sequence
of smaller problems. Scalar blocks are cheap (one equation, one unknown — direct
substitution or one Newton step). Coupled blocks are expensive (Newton iteration
on the full sub-system). The fewer coupled blocks and the smaller they are, the
cheaper the solve.

**Linear algebra connection:** The BLT decomposition is the structural analogue
of the **Dulmage-Mendelsohn decomposition** of a sparse matrix. It is equivalent
to permuting the matrix to **block lower triangular form**: diagonal blocks are
the SCCs, and the sub-diagonal entries (from one block to an earlier one) are
the data dependencies that enforce the topological order.

---

## Lesson 4: Tearing

**Concept:** A coupled block (algebraic loop) requires Newton iteration over
*all* its unknowns simultaneously. **Tearing** reduces the Newton dimension by
selecting a small subset of unknowns (the **tear variables**) as iteration
variables, then solving the remaining unknowns causally (by substitution) given
the tear values. Newton only iterates over the tear variables.

**Specimens:** `ProportionalLoop` (1 tear variable) → `TwoLoops` (2 separate
coupled blocks, each with tearing)

**In HRW:**
1. Load `ProportionalLoop`, Structural tab, Spy-plot view. Hover the coupled
   block. The tooltip shows the tearing: **tear variable** = `command`,
   **residual** = `f_x[0]`. This means: guess `command`, then solve `error`
   and `measurement` causally, then check residual `f_x[0]`. Newton iterates
   only on the 1D `command` variable (not the full 3D system).
2. Load `TwoLoops`. The spy-plot shows two coupled blocks. Hover each — each
   has its own tear variable. The Newton dimension for each block is 1 (one
   tear variable), even though each block has 2 equations.
3. Compare the tree view (switch to **Tree** sub-view). Find the `blocks` array.
   Coupled blocks have `tearing` objects with `tear_vars` and `residual_equations`.

**Key insight:** Tearing exploits the fact that within a coupled block, some
equations can be solved *sequentially* once certain variables are fixed. The
tear variables are the ones whose removal breaks all cycles. This reduces a
Newton iteration from n unknowns to t unknowns (where t is the number of tear
variables, often much smaller than n).

**Linear algebra connection:** Tearing is equivalent to choosing a set of
variables that, when fixed, makes the remaining sub-system triangular. In matrix
terms: partition the coupled block's matrix into [A11 A12; A21 A22] where A11
is triangular (the causal part) and the tear variables correspond to x2. Newton
iterates on x2; given x2, x1 = A11\(b1 - A12*x2) is a triangular solve.

---

## Lesson 5: Structural singularity

**Concept:** When the maximum matching cannot match all equations, the system is
**structurally singular** — there are more constraints than unknowns (or
equivalent equations that can't all be assigned distinct unknowns). This is the
structural signature of a **high-index DAE**: the system needs **index reduction**
(differentiating constraints to introduce new unknowns) before it can be solved.

**Specimen:** `Drivetrain`

### Tier 1 — Static snapshot: see where the singularity lives

1. Load `Drivetrain`, navigate to the **Structural** tab.
2. In the **Incidence** view, you'll see **faint red bands** — these highlight
   unmatched equations (rows) and unmatched unknowns (columns). The caption reads
   something like "93/97 matched (rank deficiency 4)."
3. The red bands show you *where* the singularity lives: the 4 unmatched rows
   correspond to constraint forces at the ideal gears (connector flows/potentials).
   These are the algebraic constraints imposed by rigid coupling.
4. Now switch to the **Index Reduction** tab. Compare the incidence view there —
   no red bands! The caption shows full rank. Index reduction differentiated the
   constraints to introduce new state variables, resolving the singularity.

At this tier you see the *fact* of singularity (red bands, rank deficiency count)
and where it lives, but not the algorithmic moment when matching fails.

### Tier 2 — Recorded replay: watch matching fail

1. Load `Drivetrain`, Structural tab. Switch to the **Matching** sub-view.
2. **Play ▶** or step through the animation. Most equations find matches
   successfully (green circles accumulate). But watch the final 4 equations —
   they search exhaustively through all their variables:
   - `Explore` after `Explore` — every variable is already taken
   - `TryDisplace` — the algorithm tries to displace holders, recursing deeply
   - `DisplaceFail` — every displacement path is exhausted
   - **`EquationFailed`** — the red row highlight and warning icon. This equation
     has **no augmenting path**. By König's theorem, this means the matching is
     already maximum — no rearrangement of existing assignments can free a
     variable for this equation.
3. Count the `EquationFailed` frames — there should be 4, matching the rank
   deficiency.
4. Now switch to the **Index Reduction** tab and run the matching animation
   there. Every equation succeeds — no `EquationFailed` frames. Index reduction
   introduced enough new unknowns to resolve all the constraint conflicts.

At this tier you see the algorithm *try and fail* — the exhaustive search, the
fruitless displacements, the moment the algorithm gives up. The contrast with
the successful matching after index reduction makes the point: the singularity
was structural, not a bug in the algorithm.

### Tier 3 — Live-stepped execution: trace the failure path in the code

The matching failure path is the same code as the success path in Lesson 2 —
`augment_traced` — but now you watch it *exhaust every possibility* and return
`false`.

**Setup:**

1. **Launch HRW under the debugger** (F5) — **do not set any breakpoints yet**.
2. In the running HRW, load **Drivetrain**. Navigate to Structural → Matching
   sub-view.
3. **Now** set a breakpoint on `live_trace_breakpoint` in `live_trace.rs`.
4. Click **"Debug"**.

**What's different from Lesson 2:**

The code path is identical — `augment_traced` recurses through variables, tries
displacements, backtracks. But for the unmatched equations, *every path fails*:

```rust
for var in vars {               // try each variable this equation references
    if !visited[var] {
        visited[var] = true;
        // Explore { eq, var } — debugger pauses, you see the yellow border
        match match_var[var] {
            None => { ... true }        // never happens — all vars are taken
            Some(holder) => {
                // TryDisplace { eq, var, holder }
                let ok = augment_traced(holder, ...);  // recurse into holder
                // holder also can't move → DisplaceFail { eq, var }
                ok  // false
            }
        };
        // can_augment is false — try the next variable
    }
}
// Exhausted all variables: return false
```

After the `for` loop exits without returning `true`, control returns to the
outer function:

```rust
if !found {
    emit_matching_frame(EquationFailed(eq));  // ← the failure frame
}
```

At the `EquationFailed` breakpoint, inspect `match_eq`: all entries are
`Some(...)` except the current equation. The `visited` array is fully `true`
for every variable this equation could reach — the algorithm explored
everything. This is the *proof* (by exhaustive search) that no augmenting path
exists.

**What to watch for in the debugger:**

- **Deep recursion**: the displacement chain goes many levels deep as the
  algorithm tries to reshuffle existing assignments. Watch the call stack grow
  as `augment_traced` calls itself.
- **`visited` fills up**: each recursive call marks more variables as visited.
  When the array is full, no more paths exist.
- **The `false` propagates up**: each recursive `augment_traced` returns `false`,
  causing the caller's `DisplaceFail` and trying the next variable. Eventually
  the top-level call exhausts its `for` loop and returns `false`.

**Locals to inspect (same as Lesson 2, but now watch the failure):**

| Local | What to look for |
|-------|------------------|
| `eq` | The equation that will fail — its index matches one of the red bands |
| `match_eq` | Every slot except `eq` is `Some(...)` — there's no room |
| `match_var` | Every variable is already owned — no free variables exist |
| `visited` | Fills to all-true — every reachable variable was explored |
| Call stack depth | Deepens as displacement chains recurse, then unwinds as each returns `false` |

**Key insight:** Structural singularity is not a modeling error — it's the
structural signature of a higher-index differential-algebraic equation. The
ideal gear constraints in Drivetrain impose position relationships that make
4 constraint forces algebraically determined by the positions, not independently
solvable. Index reduction (Pantelides algorithm + dummy derivatives) resolves
this by promoting some algebraic variables to states and differentiating their
constraints.

**Linear algebra connection:** Structural rank deficiency means the incidence
matrix has fewer linearly independent rows than columns (or vice versa). The
rank deficiency count (4 in Drivetrain) equals the system's **differential
index minus 1** (for a linear system). The unmatched rows correspond to
equations that are algebraic consequences of other equations — they constrain
the solution but can't independently determine a new unknown.

---

## Summary

| Lesson | Concept | Tiers | HRW views used |
|--------|---------|-------|----------------|
| 1 | Incidence matrix (eq-unknown dependency) | Static only | Incidence view + hover |
| 2 | Maximum matching (transversal, structural rank) | Static → Replay → Live | Incidence overlays + Matching animation + Debug |
| 3 | BLT decomposition (SCCs, scalar vs coupled blocks) | Static → Replay → Live | Spy-plot + Incidence overlays + BLT animation + Debug |
| 4 | Tearing (Newton dimension reduction) | Static only | Spy-plot tooltips + Tree view |
| 5 | Structural singularity (rank deficiency, index reduction) | Static → Replay → Live | Incidence overlays (red bands) + Matching animation + Debug |
