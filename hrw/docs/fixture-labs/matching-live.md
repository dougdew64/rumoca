# Fixture lab — Matching, live: the `stack` local is the augmenting path

<!-- kind: concept -->

**This is the only lab that stops Rumoca mid-algorithm and reads its state in a debugger.**
[matching](hrw://lab/matching) taught the idea; this one is about the code that implements it.
Run the pass-one lab first — the stations below assume you know what a matching is and what a
rank deficiency means.

It is also the only lab that needs setup. Station 0 is not optional, and it is setup rather than
teaching: it has an expectation to check, but nothing to predict.

A vocabulary note, because this lab needs three words other labs do not. A station is a
place in *this document*. A break is where the *debugger* halts execution. An anchor is
the named location a break is armed at — `decision`, `descend`, `give_up`, `push`, `gate`,
`anchor`. Keeping
them apart matters here more than anywhere else in the corpus, because all three are in play at
once.

---

## Station 0 — Two things must be true before any of this works

A debugger must be attached, and the bridge extension must be alive. They are independent, and
one machine had the first without the second for twelve days: the Debug button looked completely
normal and nothing ever stopped.

1. Launch HRW under Debug HRW Observatory (cppvsdbg) (F5), not from a terminal.
2. Open the HRW Bridge output channel in VS Code.

**Expected:** the channel's first lines read `HRW Debugger Bridge activated` and
`Watching …\hrw\.hrw-bridge for breakpoint requests`. If it says `No .hrw-bridge directory found`,
the extension is running but pointed elsewhere.

If the extension is not installed at all, HRW says so when you press Debug rather than running
silently. That notice is the feature; a silent successful-looking run is what it replaced.

---

## Station 1 — Arm an anchor, and learn what an anchor is named

[Look — ProportionalLoop → Structural → Matching animation](hrw://load/ProportionalLoop/Structural/MatchingAnim)

[⬤ Break at the free-versus-displace decision](hrw://breakpoint/decision)

> **Predict.** The link above names `decision`, not a file and a line. Why would a lab refuse to
> name the line?

**Expected:** a red dot appears at `matching.rs:864`, the
`let Some(owner) = matching.match_var[variable]` expression inside `next_augment_step`. HRW's
status bar names the line it asked for.

Falsified if: nothing arms, or the dot lands outside `next_augment_step`.

*What just happened.* The link names the anchor; the anchor finds the line. `decision` is
declared in `matching_ledger.rs` and resolved by *searching the source* at click time, so editing
anything above it moves the breakpoint with it. A link that hard-coded the line would break silently
— you would stop somewhere plausible and reason about the wrong code.

So why does this lab print `864` at all? Because something checks it.
`matching_ledger::every_line_the_live_lab_cites_is_a_real_anchor` scans this document for every
`matching.rs:<n>` and fails unless `<n>` is a live anchor or emit site. A citation is safe exactly
when a test resolves it — which is the same rule as the equation ids in the pass-one labs, and the
reason this document may name a line while the *link* may not.

The anchors that exist are `decision`, `descend`, `give_up`, `push`, `gate` and `anchor` — each
naming a decision the algorithm makes, not a place in a file.

---

## Station 2 — The augmenting path is a vector you can read at once

[⬤ Break where a displacement is pushed](hrw://breakpoint/descend)

Continue (F5) until you stop at the decision anchor a few times, then look at the
`stack` local in `run_augment_search` rather than at the call stack.

Then arm `descend` as well. It sits at `matching.rs:817`, the
`stack.push(AugmentFrame::new(owner))` that extends the path — **the line that used
to be the recursive call.** Every break there is one more equation asked to move, and
`stack.len()` immediately afterwards is the path's new length.

> **Predict.** The search explores one alternating path at a time. Where is that path
> held, and how long is it?

**Expected:** `stack` holds one `AugmentFrame` per equation on the current path, each
with its own `eq`. The top is the equation being explored now; everything below it is an
equation that was asked to move. N entries is an N-edge alternating path. The matching's own
*call* frames are flat — `augment`, `run_augment_search`, `next_augment_step`, the same ones
every time — beneath the frames of the preparation that called it.

Falsified if: `stack` holds one entry at every break, or two entries report the same `eq`.

*What just happened.* This is the station the lab exists for, and it is not visible from
the animation. The augmenting-path search runs alternately along unmatched and matched
edges, looking for an unmatched unknown. That run is implemented as an **explicit stack**
— so the path the algorithm is currently exploring is literally the contents of `stack`,
and its length is the depth.

Read `stack` from index 0 upward and you are reading the path from its start. Step until it
grows and you are watching the path extend; step until an entry is popped and you are
watching a dead end abandoned.

> **This station changed shape in Rumoca 0.10.0, and the change is worth a minute.**
>
> The search used to be recursive, and this station used to say *"the call stack is the
> augmenting path"* — you climbed the debugger's frames, one `augment_traced` per
> displaced equation, and the depth was the path length.
>
> It is iterative now, and `matching.rs` says why — at the top of the file and again on
> `augment`: an array-shaped model can produce alternating chains hundreds of thousands of
> equations long, and the recursive form overflows the thread stack on those.
>
> **So the algorithm did not change — its representation of "where am I" did**, from the
> machine's stack to a `Vec` the code owns. That is a common move when a recursive
> algorithm meets real input sizes, and it is worth recognising: the recursion is still
> there in the *structure* of the search, just not in the call stack. Watching one local
> instead of scrolling frames is arguably the better view, since you see the whole path
> at once.

---

## Station 3 — The same machinery, refusing

`CapacitorLoop` is 14 equations in 14 unknowns, and `matching.md` established that only 13 pair.
It loads the Modelica Standard Library, so the compile takes longer than the ones above.

[Look — CapacitorLoop → Structural → Matching animation](hrw://load/CapacitorLoop/Structural/MatchingAnim)

Before arming anything, watch the refusal in the animation. It is eight frames:

[Frame 42 — try `0 = C.v - (src.p.v - 0)`](hrw://stage/Structural/MatchingAnim/frame/42) ·
[44 — displace `src.p.v`](hrw://stage/Structural/MatchingAnim/frame/44) ·
[46 — displace `src.v`](hrw://stage/Structural/MatchingAnim/frame/46) ·
[49 — the equation fails](hrw://stage/Structural/MatchingAnim/frame/49)

The search tries the capacitor's equation, explores `src.p.v`, asks the source's
`0 = src.v - (src.p.v - 0)` to give it up, explores `src.v` from there, and asks
`0 = src.v - src.V` to give *that* up. `src.V`'s equation has no other unknown to move to, so
both displacements fail (frames 47 and 48) and the equation is left unmatched (frame 49).

[⬤ Break at the free-versus-displace decision](hrw://breakpoint/decision)
[⬤ Break where a displacement is pushed](hrw://breakpoint/descend)
[⬤ Break where the search gives up](hrw://breakpoint/give_up)

> **Predict.** In the search for that one equation, how many times does each anchor fire, and
> what is in `stack` at the deepest break?

**Expected:** red dots at `matching.rs:864`, `matching.rs:817` and `matching.rs:841`, the last on
the bare `false` that ends `run_augment_search`. In the search for the capacitor's equation,
`decision` fires twice (exploring `src.p.v`, then `src.v`), `descend` twice (pushing the source's
two equations), and `give_up` once — after which the stage reports `f_x[6]` unmatched and
`gnd.p.i` as the unmatched unknown. At the second `descend` break `stack` holds two entries —
the capacitor's equation and the source's first — and three once the push executes.

Falsified if: `give_up` never fires, or the search reports a different unmatched pair.

**Count only that equation's search.** The anchors are code breakpoints, so they also fire for
every other equation — `decision` 24 times in this analysis, once per explore frame — and again
in the analyses the preparation runs after it: the reducer's demoted system goes through the same
`augment`. That one matches completely, so `give_up` fires only in the first. *(The animation
shows the first analysis only — 95 frames — so a break you hit after its last frame belongs to a
later one.)*

*What just happened.* Failure is a code path, not an absence of one. The algorithm does not
fail by crashing or by running out of time: it follows every alternating path it can from the
capacitor's equation, finds no unmatched unknown at the end of any of them, and returns.
`give_up` is that return.

That is why `matching.md` could state a rank deficiency of exactly 1 — the deficiency is the count
of equations whose search ends without finding a free unknown.

*(This station ran on `TwiceDefined` until 2026-11-27. In the run that simulates, Simplify folds
its second equation to `0 = 1 - time`, which mentions no unknown, so its search fails without
exploring anything — no displacement, nothing for `descend` to show.)*

---

## Station 4 — What this instrument can and cannot show you

> **Predict.** You have now seen the same algorithm succeed and fail. What could a debugger show
> you here that the animation could not, and what can it not show?

**Expected:** the path held in `stack`, its length, and the locals at each break are visible only
in the debugger; the *overall* progress of the matching is visible only in the animation.

Falsified if: the animation displays a call depth, or the debugger displays the finished
matching.

*What just happened.* The two instruments answer different questions and neither replaces the
other. The animation is the algorithm's output over time — which pairs exist after each step.
The debugger is its control flow — why it is trying this edge and not that one.

Two facts about the debugger that cost real time to learn, and which you will meet if you keep
going:

- `cppvsdbg` will not re-bind a breakpoint at a location whose breakpoint left the adapter's
  active set during a session — by removal *or* by being disabled. Only a *new* debug session
  recovers it. So if a second Debug press seems to run straight through, restart the session rather
  than re-arming.
- VS Code exposes no `verified` field to extensions, so "a breakpoint is present" can never mean
  "execution will stop". The bridge reports what it armed and cannot promise more.

---

## What this lab cannot check

Whether the stack reads as a path. Station 2 is the whole point and it depends on the `stack`
local being legible in VS Code's Variables pane, which no test reaches. If its entries collapse or
their `eq` fields are optimised away, the station says nothing.

Whether the anchors are still where the algorithm decides. They are resolved by name, so they
cannot point at a stale line — but nothing checks that `decision` still sits at a *decision*. A
refactor could move it somewhere valid and uninteresting.

Everything in Station 0. Whether the extension is alive, the junction exists, and the launch
configuration is right are all environment facts, and the only signal is what the output channel
says.

---

## What comes next

This is the first pass-two lab. The rest of the pipeline has pass-one labs only, and the same
treatment — read the phase's code while it runs — is available for every one of them.
