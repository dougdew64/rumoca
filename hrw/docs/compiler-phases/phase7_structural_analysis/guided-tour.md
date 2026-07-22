# Structural Analysis — Guided Tour

A five-lesson interactive walkthrough of structural analysis using HRW and
specimens. Each lesson introduces one concept, uses specific specimens, and tells
you exactly what to do in HRW to see the concept in action.

**Prerequisites:** HRW built and running (`cargo run -p hrw` from the workspace
root). Familiarity with loading specimens (click a name in the left panel).

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

**In HRW:**
1. Load `SingleInertia`, Structural tab, Incidence view. The **green circles**
   on the incidence matrix mark the matched pairs — each equation has one green
   dot showing which unknown it "owns." This is the **transversal**.
2. Count the green dots — there should be 12 (one per equation). The caption
   confirms "12/12 matched (full rank)."
3. Now switch to the **Matching** sub-view (the tab labeled "Matching ▶"). This
   is the animated stepper.
4. Click **Step ▶** to advance frame by frame. Watch the algorithm work:
   - **Yellow highlight** = the equation currently searching for an augmenting path
   - **Yellow border** = the edge being explored ("can I take this variable?")
   - **Green flash** = success ("this variable is free, I'll take it!")
   - **Green circle** = a confirmed match
5. For SingleInertia, most equations find a free variable immediately — there's
   little conflict. Now load **ProportionalLoop** and run the matching animation.
   Watch for a **displacement**: one equation tries a variable that's already
   matched, so the holder must find an alternative. You'll see the "try to
   displace" step (the holder's row lights up) followed by success or failure.

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

**In HRW:**
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
4. Switch to the **BLT ▶** animation tab. Watch Tarjan's algorithm discover
   the strongly connected components:
   - **Yellow** nodes are on the DFS stack (being explored)
   - **Green tree edges** show the DFS going deeper
   - **Red back edges** reveal cycles (mutual dependencies)
   - When an SCC is complete, its nodes **light up** in a distinct color
   - Singleton SCCs = scalar blocks; multi-node SCCs = coupled blocks (algebraic
     loops)

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

**In HRW:**
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
5. Run the matching animation on both tabs. On the Structural tab, watch the
   last few equations fail to find augmenting paths (red row highlight, warning
   icon). On the Index Reduction tab, every equation succeeds.

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

| Lesson | Concept | HRW views used |
|--------|---------|----------------|
| 1 | Incidence matrix (eq-unknown dependency) | Incidence view + hover |
| 2 | Maximum matching (transversal, structural rank) | Incidence overlays + Matching animation |
| 3 | BLT decomposition (SCCs, scalar vs coupled blocks) | Spy-plot + Incidence overlays + BLT animation |
| 4 | Tearing (Newton dimension reduction) | Spy-plot tooltips + Tree view |
| 5 | Structural singularity (rank deficiency, index reduction) | Incidence overlays (red bands) + Matching animation |
