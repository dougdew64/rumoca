# Fixture tour — seeking to a frame

<!-- kind: feature -->

**This is a test, not an explanation.** It verifies that a link can stop an animation on
a specific frame — the capability that lets a stop point at *the moment a decision is
made* rather than at the view containing it.

Pick it from the Tours list. Every link below is parsed on every test run by
`fixture_tour_links_all_resolve`.
**Notices appear in the status bar**, along the bottom of the HRW window. Several stops below expect one; that is where to look.


**Frame numbers in links match the counter on screen.** `frame/41` puts the view on
"Frame 41". They were 0-based until 2026-07-29, and this file *documented* the
off-by-one in a parenthetical rather than treating it as the bug it was — Doug spotted
that while walking the stops in order.

*Every matching replay gained an opening frame on 2026-08-23, so each number below now
lands one step earlier than it used to. The numbers were **left as they are**: this tour
seeks 41 and 6 to prove that a jump forwards and a jump backwards work, not because
either is a particular moment in the algorithm. A tour that cited a frame for **what it
shows** would have had to be re-derived with `cargo run -p hrw --example frame_index`.*

---

## Stop 0 — A stop clicked out of order

**Do this before Stop 1**, with no specimen loaded. (Arriving here from another tour
clears the previous one, so that is already true. If you have walked a stop since, switch
to a different tour and back.)

[Seek frame 5 — with nothing loaded](hrw://stage/Structural/MatchingAnim/frame/5)

**Expected:** a notice in the status bar saying no specimen is loaded and to start at the first stop.

**And nothing else happens.** No stage change, no view change. Then click Stop 1 below:
it must behave normally, **not** jump to frame 5 — a refused stop leaves nothing armed to
fire later.

*(Doug found this by clicking a tour's fourth stop first: the link silently did nothing,
because with no specimen the stage area returns early. Silence is indistinguishable from
a broken link, which is the one outcome a tour cannot survive.)*

## Stop 1 — A replay, unstarted

[MotorWithBrake → Structural → Matching](hrw://load/MotorWithBrake/Structural/MatchingAnim)

48 equations, structurally singular, so the search will fail — that is the interesting
part, and the reason this view is reachable at all (`ideas.md` #44).

**Expected:** frame 1 of many, paused, and it is the **starting point** — a clapper-board
icon, "48 equations, 48 unknowns, nothing matched yet", and an empty matrix. Frame 1 is
the system before the search, not its first move.

## Stop 2 — Jump into the middle

[Seek to frame 41](hrw://stage/Structural/MatchingAnim/frame/41)

**Expected:** the frame counter reads **41/…** — the same number as the link — and the
matrix shows a partly-built matching. Playback is **paused**; if it starts running, the
seek is not holding.

## Stop 3 — Jump backwards

[Seek to frame 6](hrw://stage/Structural/MatchingAnim/frame/6)

**Expected:** the counter reads **6/…** — down from 41. A seek is not "advance to", it
is "go to". If Stops 2 and 3 both land forward, the cursor is being clamped rather than
set.

## Stop 4 — Seek past the end

[Seek to frame 99999](hrw://stage/Structural/MatchingAnim/frame/99999)

**Expected:** nothing moves, and a notice in the status bar says how many frames the replay actually has.
A tour naming a frame that does not exist is a bug *in the tour*; landing on the last
frame would look deliberate and hide it.

## Stop 5 — Seek in a different animation

[Index Reduction → Reduction replay, frame 3](hrw://stage/IndexReduction/Animate/frame/3)

**Expected:** the stage switches, the reduction replay opens **on the first click**, and
it sits on **frame 3** of its own trace. All eight animated views share one `Playback::seek`, so this exercises
that they are genuinely the same mechanism rather than eight lookalikes.

## Stop 6 — Seek a view that has no animation

[Structural → Incidence, frame 2](hrw://stage/Structural/Incidence/frame/2)

*(Incidence has no replay, so there is no counter to match.)*

**Expected:** the Incidence view opens and **nothing else happens** — no notice, no
error. The link still navigates; the seek is simply not applicable. Degrading to "the
right view, unsought" beats failing the stop.

---

## What this cannot check

Whether the frame it lands on is the *interesting* one. That is a judgement about the
algorithm, not about the mechanism, and it is the half a tour author gets wrong.
