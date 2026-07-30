# Fixture tour — seeking to a frame

**This is a test, not an explanation.** It verifies that a link can stop an animation on
a specific frame — the capability that lets a stop point at *the moment a decision is
made* rather than at the view containing it.

Pick it from the Tours list. Every link below is parsed on every test run by
`fixture_tour_links_all_resolve`.

---

## Stop 1 — A replay, unstarted

[MotorWithBrake → Structural → Matching](hrw://load/MotorWithBrake/Structural/MatchingAnim)

48 equations, structurally singular, so the search will fail — that is the interesting
part, and the reason this view is reachable at all (`ideas.md` #44).

**Expected:** frame 1 of many, paused, nothing yet matched.

## Stop 2 — Jump into the middle

[Seek to frame 40](hrw://stage/Structural/MatchingAnim/frame/40)

**Expected:** the frame counter reads **41** (frames are 0-based in links, 1-based in the
display) and the matrix shows a partly-built matching. Playback is **paused** — if it
starts running, the seek is not holding.

## Stop 3 — Jump backwards

[Seek to frame 5](hrw://stage/Structural/MatchingAnim/frame/5)

**Expected:** the counter goes **down**. A seek is not "advance to"; it is "go to". If
Stops 2 and 3 both land forward, the cursor is being clamped rather than set.

## Stop 4 — Seek past the end

[Seek to frame 99999](hrw://stage/Structural/MatchingAnim/frame/99999)

**Expected:** nothing moves, and a notice says how many frames the replay actually has.
A tour naming a frame that does not exist is a bug *in the tour*; landing on the last
frame would look deliberate and hide it.

## Stop 5 — Seek in a different animation

[Index Reduction → Reduction replay, frame 2](hrw://stage/IndexReduction/Animate/frame/2)

**Expected:** the stage switches, the reduction replay opens, and it sits on frame 3 of
its own trace. All eight animated views share one `Playback::seek`, so this exercises
that they are genuinely the same mechanism rather than eight lookalikes.

## Stop 6 — Seek a view that has no animation

[Structural → Incidence, frame 2](hrw://stage/Structural/Incidence/frame/2)

**Expected:** the Incidence view opens and **nothing else happens** — no notice, no
error. The link still navigates; the seek is simply not applicable. Degrading to "the
right view, unsought" beats failing the stop.

---

## What this cannot check

Whether the frame it lands on is the *interesting* one. That is a judgement about the
algorithm, not about the mechanism, and it is the half a tour author gets wrong.
