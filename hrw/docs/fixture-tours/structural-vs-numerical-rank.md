# Structural rank vs numerical rank

<!-- kind: adjudication -->

**The first cross-platform tour.** Two stops in HRW, then a notebook — because the point
it makes cannot be made in either place alone.

**The question:** the `CapacitorLoop` tour rests on structural rank being *a property of
the pattern, not the values*. HRW can show you the pattern. It cannot show you a matrix
with full structural rank that is numerically singular — the case that makes the
distinction matter.

Each stop says where it happens. 📐 = HRW · 🧮 = Wolfram Desktop.

---

## 📐 Stop 1 — The pattern, from a real model

[ProportionalLoop → Structural → Incidence](hrw://load/ProportionalLoop/Structural/Incidence)

**Expected:** a 3×3 block of marks. Each equation touches **exactly two** of `error`,
`command`, `measurement`, and every unknown is touched twice — the 3-cycle that makes this
block coupled.

That shape is the entire input to structural analysis. No coefficient appears in it.

## 📐 Stop 2 — What HRW concludes from it

[Structural → Tearing](hrw://stage/Structural/TearingAnim)

**Expected:** the replay tears **`command`**, then makes `f_x[1]` and `f_x[2]` causal in
turn, finishing with 1 tear and 1 residual equation.

Everything you just watched was decided from the pattern alone. HRW never evaluated
`controllerGain` or `plantGain` — and that is the claim the notebook tests.

## 🧮 Stop 3 — Where the values go in

[Open the notebook in Wolfram Desktop](hrw://notebook/structural-vs-numerical-rank.nb)

**Expected:** Wolfram Desktop opens the notebook — **not** a web browser. Then evaluate
the cells in order.

*(A plain markdown link to a `.nb` gets handed to the browser, which does nothing useful
with it; Doug hit that walking this tour on 2026-07-30. The **notebook verb** hands the
file to the OS association instead, so HRW never needs to know where Wolfram is
installed.)*

*(The notebook is versioned beside this tour rather than written to the gitignored bridge
directory. An **ad hoc** notebook is ephemeral like an ad hoc tour; a **fixture** notebook
has expected outcomes, and a test that vanishes on a fresh checkout is not a test.)*

**Expected, in the notebook:**

- §2 — structural rank **3**, computed from the pattern by maximum matching. The same
  number HRW reports.
- §4 — the determinant is **`1 + k p`**, so the block's whole behaviour is the loop gain.
- §5 — at the specimen's own gains (10 and 2): determinant **21**, rank **3**.
- §6 — at loop gain **−1**: *same sparsity pattern*, structural rank still 3, determinant
  **0**, numerical rank **2**.
- §7 — a one-dimensional null space and **no solution**.

**§6 is the stop.** If the pattern comparison there returns `False`, the example is broken
and the tour proves nothing.

## 📐 Stop 4 — Back to HRW, and what it would say

[Structural → Spy-plot](hrw://stage/Structural/SpyPlot)

**Expected:** the block structure, with the 3×3 coupled block on the diagonal and no
error anywhere. `ProportionalLoop` is structurally non-singular.

**It would look identical at loop gain −1.** Same pattern, same blocks, same tearing —
because **nothing in this view can tell the difference.**

*(This stop linked to `Structural/Summary` until Doug walked the tour: that tab exists
only when a model is* singular*, so the link named a view with no tab. HRW now refuses
such a link and says so, rather than selecting it and rendering the singular summary for
a non-singular model.)*

That is not a defect. It is the boundary of what a structural phase is for, and knowing
where the boundary lies is the point of the tour.

---

## What each side uniquely holds

| | |
|---|---|
| **HRW** | the sparsity pattern of a *real compiled model*, and the graph theory on it — matching, BLT, tearing |
| **Wolfram** | what happens once the values are in — rank, determinant, null space |

Neither is redundant, and neither should grow the other's half: `ideas.md` #17 was
originally scoped to build rank and conditioning *into HRW*, and this tour is the argument
for not doing that.

## What this cannot check

Whether the notebook's cells evaluate cleanly on your machine, and whether §8's
`Manipulate` is responsive. Claude evaluated every result above through the Wolfram
kernel, but a notebook is not a kernel session.
