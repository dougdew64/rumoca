# Fixture tour — camera aiming

<!-- kind: feature -->

**This is a test, not an explanation.** It exists so Doug can verify the half of camera
aiming that Claude cannot see: whether the camera actually lands where a link says.

Unlike an ad hoc tour (`.hrw-bridge/tour.md`, gitignored, regenerated per question), a
fixture tour is **kept and versioned**, because it has a pass/fail criterion rather than
prose that would rot. `fixture_tour_links_all_resolve` parses every link below on every
test run, so a vocabulary change breaks the build instead of breaking this file quietly.

**To run it:** open Tour mode and pick **camera-aiming** from the row of tours at the top
of the panel. (Before 2026-07-29 this had to be copied over `.hrw-bridge/tour.md` first;
Doug asked for in-app selection once it was clear the fixtures would accumulate.)
**Notices appear in the status bar**, along the bottom of the HRW window. Several stops below expect one; that is where to look.


---

## Stop 1 — Load, and note where the camera starts

[RcCircuit → Structural → BLT](hrw://load/RcCircuit/Structural/TarjanAnim)

23 equations, non-singular, so the BLT view is available and the graph is big enough
that "centred on one node" looks different from "fitted to everything".

**Expected:** the whole graph fitted in view, with **equation 0 at the top-left of the
grid** — not at the centre of the canvas. That is the baseline the next two stops move
away from.

## Stop 2 — Aim at the first equation

[Aim at equation 0](hrw://stage/Structural/TarjanAnim/equation/0)

**Expected:** the view recentres so **equation 0** — top-left of the grid — sits in the
**middle** of the canvas. Most of the graph is now off to the right and below.

**Zoom must not change.** Aiming says *where* to look, not how far in.

## Stop 3 — Aim at the far corner

[Aim at equation 22](hrw://stage/Structural/TarjanAnim/equation/22)

**Expected:** the view recentres on the **last** equation, bottom-right of the grid. If
Stops 2 and 3 look identical, aiming is not being applied.

## Stop 4 — Aim at something that is not there

[Aim at equation 999](hrw://stage/Structural/TarjanAnim/equation/999)

**Expected:** the view does **not** move, and a notice in the status bar says there is no
equation 999. A tour that names a missing equation is a bug in the tour, so it must be
visible rather than silently aiming somewhere plausible.

## Stop 5 — Aiming survives a resize

[Aim at equation 11](hrw://stage/Structural/TarjanAnim/equation/11)

Now resize the HRW window horizontally.

**Expected:** the refit re-frames the graph (that is the intended behaviour), and the
camera is no longer centred on 11. Aiming is **one-shot** — it does not re-pin the view
every frame. A sticky aim would fight the scrollbar and the drag, which is the 2026-07-29
sideways-drift bug in a different guise.

---

## What this cannot check

Whether the *right* node is highlighted, and whether the centring looks natural rather
than merely arithmetically correct. Those need eyes. `pan_to_center_puts_the_target_in_the_middle`
and `equation_positions_tile_a_square_grid` cover the arithmetic.
