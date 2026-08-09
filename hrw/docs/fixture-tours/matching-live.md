# Matching, live — standing inside the search

**Walk [`matching.md`](matching.md) first.** That tour shows the algorithm running, what it
builds, and what its code is called. This one puts you *inside* it, stopped, with the state in
front of you — and shows you two things the animation **cannot** show, because they emit no
frame at all.

**Two models, same machinery, opposite answers.** `ProportionalLoop` displaces an equation and
succeeds. `TwiceDefined` displaces an equation and fails. The stacks are the same shape; one
number differs. That difference is the whole of structural singularity.

> **Line numbers in this tour are checked.** Every `matching.rs:NNN` and `live_trace.rs:NNN`
> below is verified against the source by
> `matching_ledger::tests::every_line_the_live_tour_cites_is_a_real_anchor`, and the full table
> is generated into
> [`matching-live-reference.md`](../compiler-phases/phase7_structural_analysis/matching-live-reference.md).
> If the code moves, the test fails instead of you following a stale number.

---

## Scene 0 — Two things must be true before any of this works

**A debugger must be attached, and the bridge extension must be alive.** They are independent,
and for twelve days in August 2026 one machine had the first without the second — the Debug
button looked completely normal and nothing ever stopped.

1. Launch HRW under **Debug HRW Observatory (cppvsdbg)** (F5), not from a terminal.
2. Open the **HRW Bridge** output channel in VS Code.

**Expected:** the channel's first lines read `HRW Debugger Bridge activated` and
`Watching …\hrw\.hrw-bridge for breakpoint requests`. If it says
`No .hrw-bridge directory found`, the extension is running but pointed elsewhere.

If the extension is not installed at all, **HRW will tell you when you press Debug** rather than
running silently — a notice naming the bridge. That notice is the feature; a silent successful-looking
run is what it replaced.

---

## Scene 1 — Arm it, and learn to name a stop

[ProportionalLoop → Structural → Matching animation](hrw://load/ProportionalLoop/Structural/MatchingAnim)

**Expected:** the matching animation, unstarted, captioned `Matched 3 of 3`.

**Now — and only now — set a breakpoint at `matching.rs:189`**, the `match match_var[var]`
expression inside `augment_traced`.

> **Set it after the model has compiled, never before.** `augment_traced` also runs during the
> ordinary compile, where nothing is animated and `observer` is `None`. A breakpoint set too
> early stops you there instead, several times, before the animation exists.

Press **Debug**.

**Expected:** VS Code stops at `live_trace.rs:173`, and the `frame_index` local reads
**18446744073709551615**. That is `usize::MAX`, the startup gate — the call at
`live_trace.rs:97`, made *before any algorithm work*. A `0` here would mean you missed the gate.

**Every live stop lands on that same line.** So the anchor cannot tell you *what* just happened —
**the calling frame does.** Look one frame down the call stack:

| the frame below the anchor | the step that was just recorded |
|---|---|
| `matching.rs:114` | `TryEquation` — the outer loop starting an equation |
| `matching.rs:181` | `Explore` — reaching a variable |
| `matching.rs:191` | `FoundFree` — that variable has no holder |
| `matching.rs:202` | `TryDisplace` — it has one, and the recursive call is next |
| `matching.rs:213` | `DisplaceOk` **or** `DisplaceFail` — that call returned |
| `matching.rs:233` | `Assign` — both arrays written |
| `matching.rs:133` | `EquationFailed` — the equation was given up on |

Continue twice.

**Expected:** `frame_index` `0` with `matching.rs:114` below the anchor, then `frame_index` `1`
with `matching.rs:181`. **`TryEquation` has no `augment_traced` frame at all** — the driver
emits it before the call.

**Expected:** the animation reads **Frame 2** while the debugger says `frame_index` **1**. They
are both right: `frame_index` is 0-based, the screen counts from 1.

---

## Scene 2 — The call stack *is* the augmenting path

Continue once more; you land on `matching.rs:189`, your own breakpoint.

**Expected:** `eq` is `0`, `var` is `0`, `vars` is `[0, 2]`, `match_var` is all `None`.

Continue until `frame_index` reads `5` — three presses, through `FoundFree`, `Assign` and the
next `TryEquation`. Then one more, to `189` again.

**Expected:** `eq` is `1`, `vars` is `[0, 1]`, and **`match_var[0]` is `Some`.** Equation 1 wants
a variable equation 0 already holds. This is the collision.

Continue. **Expected:** anchor stop, `frame_index` `6`, with `matching.rs:202` below it —
`TryDisplace`. Equation 1 has announced it will ask equation 0 to move, and has not asked yet.

**Continue once more. This is the stop the tour exists for.**

**Expected:** the call stack shows **two `augment_traced` frames** — the inner one at
`matching.rs:181`, the outer at `matching.rs:210`, the recursive call site.

Read it downward and it is a path:

```
eq1  ──wants──▶  var0  ──held by──▶  eq0  ──probing──▶  var2
```

**N frames is N equation-nodes and 2N − 1 edges** — alternating unmatched, matched, unmatched.
Two frames, three edges. The nodes are the frames; the alternation is the point.

Continue three times.

**Expected:** `Assign` at depth 2 (`matching.rs:233` under the anchor, still two `augment_traced`
frames), then `DisplaceOk` at `matching.rs:213` with the stack back to **one** frame, then
`Assign` again.

**Expected:** the caption now reads **Matched 2 of 3**.

**Nothing walked back along a stored path to do that.** There is no path variable anywhere in
`matching.rs`. Each frame committed its own edge as it returned — **the unwind is the flip**, and
watching the stack shrink is watching the matching grow.

---

## Scene 3 — The same machinery, refusing

**Start a fresh debug session first: stop the debugger, then F5 again.** `cppvsdbg` will not
re-bind a breakpoint at a location whose breakpoint has left its active set during a session, so
a session you have been removing or disabling breakpoints in is not a clean instrument.

[TwiceDefined → Structural → Matching animation](hrw://load/TwiceDefined/Structural/MatchingAnim)

Two equations, `a = 1.0` and `a = time`, and two unknowns. **Both equations mention only `a`.**

**Expected:** the caption reads `Matched 1 of 2` — this model is structurally singular, and the
tour's job is to show you *how the algorithm finds that out*.

After it compiles, set **two** breakpoints: `matching.rs:189` as before, and **`matching.rs:243`**
— the bare `false` that ends `augment_traced`.

Press **Debug** and continue to `frame_index` `5`, then once more to `189`.

**Expected:** `eq` is `1`, `match_var[0]` is `Some` — the same collision as Scene 2 — but
**`vars` has length 1**, holding only `[0]`. In `ProportionalLoop` equation 1 had a second
candidate to fall back on. Here there is none.

Continue. **Expected:** `TryDisplace` at `matching.rs:202`, `frame_index` `6`.

**Continue. You land on `matching.rs:243`, not on the anchor.**

**Expected:** two `augment_traced` frames — inner at `243`, outer at `210` — with `eq` reading
`0`. Equation 0 was asked to move, found `visited[a]` already `true`, skipped its only candidate,
and fell out of the loop.

**Expected:** `frames` still has **7** entries, the last being `TryDisplace`. **This step emitted
nothing.** You are looking at a real decision the animation has no record of.

**Expected:** `var` and `iter` both read *"Variable is optimized away and not available."* That is
not noise — it is how you know the `for` loop **ended** rather than returning from inside it.

Continue. **Expected:** `DisplaceFail` at `matching.rs:213`, one `augment_traced` frame.

Continue. **Expected:** `matching.rs:243` again, now with **one** frame and `eq` reading `1` — the
outer search giving up in turn. Also no frame emitted.

Continue. **Expected:** the anchor, `frame_index` `8`, with `matching.rs:133` below it —
`EquationFailed`. The caption still reads `Matched 1 of 2`.

---

## Scene 4 — What the two runs say together

| | ProportionalLoop | TwiceDefined |
|---|---|---|
| stack at depth 2 | `181 → 210` | `243 → 210` |
| the path | eq1 → a → eq0 → **a free variable** | eq1 → a → eq0 → **nothing** |
| Berge's name for it | **augmenting** | merely **alternating** |
| the unwind | commits an edge per frame | commits nothing |
| `vars` at the collision | `[0, 1]` | `[0]` |

**Alternating paths are cheap — you can always find one.** A path becomes *augmenting* only if it
ends at a free variable, and that terminal condition is the entire content of the theorem that a
matching is maximum exactly when no augmenting path exists. You have now seen both sides of it,
and the difference on screen was one line number in a stack.

**And the failure was decided before the search began.** `TwiceDefined`'s incidence has two
entries, both in column `a`; **column `b` is empty**, so no permutation can place a nonzero on
`b`'s diagonal position. That is **Hall's condition** failing: the two equations together touch
only one unknown, so |N(S)| = 1 < 2 = |S|. The algorithm cannot know this in advance — it finds
out by exhausting the search, which is exactly what you stepped through.

**Notice what never happened: `b` was never visited.** No equation mentions it, so no frame ever
names it. The unmatched *unknown* is reported by absence at the end, never discovered.

**And the failed search left the matching untouched** — `match_eq` and `match_var` are identical
before and after equation 1's attempt. Only `visited` was written, and it is reset per equation.
That is why the outer loop never has to backtrack.

---

## Scene 5 — What this instrument can and cannot show you

**Two steps had no frame.** Both `matching.rs:243` stops are real algorithm decisions that the
frame stream structurally cannot contain, so no animation, no ledger and no log can show them.
**That is the reason to stand inside the algorithm rather than beside it.**

**What an anchor stop will not give you:** only `frame_index` is in scope there. The algorithm's
state — `eq`, `match_eq`, `visited` — lives four frames up, and the debugger shows one scope. So
**`live_trace.rs:173` tells you *which step*; `matching.rs:189` tells you *what the algorithm
knows*.** They are not interchangeable.

**And `Option` payloads are invisible one level down**: `match_var[0]` reads `Some` without
telling you the holder. On a 2×2 you can deduce it; on a large model you cannot.

> **To ask about where you are stopped, just say so.** The bridge publishes the stop —
> location, call stack and the innermost scope's locals — to `.hrw-bridge/debug-state.json`, and
> Claude reads it. Naming the file or selecting the line is unnecessary.
>
> *(This replaced the opposite advice on 2026-08-08. Until `docs/ideas.md` #72 shipped, Claude
> genuinely could not see a debugger stop and the tour told you to select the line first. Both
> statements were true when written, which is why this one carries its date.)*

---

## What this tour cannot check

**Whether the two-breakpoint rhythm reads as a rhythm.** The anchor fires on every frame while
`189` fires only after an `Explore`, so the pattern is uneven by construction. It is legible if
you name each stop from the calling frame — that is the theory Scene 1 is built on, and only
walking it decides whether naming is a small habit or a constant tax.

**Whether Scene 3's payoff lands.** "Two steps emitted nothing" is the strongest claim here, and
it is an argument about *absence* — the reader has to notice that `frames` did not grow. If that
reads as bookkeeping rather than as revelation, the scene needs the animation on screen beside
the debugger, and should say so explicitly.

**Whether `TwiceDefined` earns being a fourth specimen.** It was chosen because `CapacitorLoop` —
`matching.md` Act 3's failure — takes ~114 frames to reach its failure against nine here. The
trade is a synthetic model in exchange for a walkable one, and whether the synthetic feels like a
real lesson is a judgement only the walker can make.
