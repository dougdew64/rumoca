# Fixture lab — Matching, live: the call stack is the augmenting path

<!-- kind: concept -->

[The chain overview](hrw://lab/the-concepts)

**A concept lab, pass two.** [matching](hrw://lab/matching) taught the idea; this one is about
Rumoca's code, stepped in a debugger while it runs. Run the pass-one lab first — the stations
below assume you know what a matching is and what a rank deficiency means.

This is the only lab that needs setup. Station 0 is not optional, and it is setup rather than
teaching: it has an expectation to check, but nothing to predict.

A vocabulary note, because this lab needs three words other labs do not. A station is a
place in *this document*. A break is where the *debugger* halts execution. An anchor is
the named location a break is armed at — `decision`, `recurse`, `give_up`, `push`, `gate`. Keeping
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

**Expected:** a red dot appears at `matching.rs:224`, the `match match_var[var]` expression inside
`augment_traced`. HRW's status bar names the line it asked for.

Falsified if: nothing arms, or the dot lands outside `augment_traced`.

*What just happened.* The link names the anchor; the anchor finds the line. `decision` is
declared in `matching_ledger.rs` and resolved by *searching the source* at click time, so editing
anything above it moves the breakpoint with it. A link that hard-coded the line would break silently
— you would stop somewhere plausible and reason about the wrong code.

So why does this lab print `189` at all? Because something checks it.
`matching_ledger::every_line_the_live_lab_cites_is_a_real_anchor` scans this document for every
`matching.rs:<n>` and fails unless `<n>` is a live anchor or emit site. A citation is safe exactly
when a test resolves it — which is the same rule as the equation ids in the pass-one labs, and the
reason this document may name a line while the *link* may not.

The anchors that exist are `decision`, `recurse`, `give_up`, `push`, `gate` and `anchor` — each
naming a decision the algorithm makes, not a place in a file.

So the anchors are declared in `matching_ledger.rs` and resolved at click time. `decision`,
`give_up`, `recurse`, `push` and `gate` are the ones that exist, and each names a *decision the
algorithm makes* rather than a place in a file.

---

## Station 2 — The call stack is the augmenting path

Continue (F5) until you stop at the decision anchor a few times, then look at the call stack
rather than at the variables.

> **Predict.** `augment` is recursive. What does the depth of the stack correspond to, in the
> graph?

**Expected:** the stack shows two `augment_traced` frames — the inner one at `matching.rs:216`,
the outer at `matching.rs:245`, which is the recursive call site. Each frame's `eq` local is a
different equation. N nested frames is an N-edge alternating path.

Falsified if: the stack is flat at every break, or two frames report the same `eq`.

*What just happened.* This is the station the lab exists for, and it is not visible from the
animation. The augmenting-path search runs alternately along unmatched and matched edges, looking
for an unmatched unknown. That run is implemented as recursion — so the *path* the algorithm is
currently exploring is literally the sequence of frames on the stack, and its length is the depth.

Read the stack bottom-up and you are reading the path from its start. Step until it deepens and you
are watching the path extend; step until it returns and you are watching a dead end abandoned.

Claude cannot see any of this, which is worth knowing while you work: a break yields no
location, no stack and no values to a tool. If you want to ask about a break, the extension publishes
it to `.hrw-bridge/debug-state.json` — stack frames, the innermost location, and the locals of the
most local scope.

---

## Station 3 — The same machinery, refusing

`TwiceDefined` is two equations in two unknowns, and `matching.md` established that only one pairs.

[Look — TwiceDefined → Structural → Matching animation](hrw://load/TwiceDefined/Structural/MatchingAnim)

[⬤ Break at the free-versus-displace decision](hrw://breakpoint/decision)
[⬤ Break where the search gives up](hrw://breakpoint/give_up)

> **Predict.** With two anchors armed and a system that cannot be matched, which one stops first,
> and how many times does each fire?

**Expected:** red dots at `matching.rs:224` and `matching.rs:278`, the latter on the bare `false`
that ends `augment_traced`. The decision anchor fires while the search explores; `give_up` fires
when it exhausts the alternatives for the unmatchable equation — reporting `f_x[1]` unmatched and
`b` as the unmatched unknown.

Falsified if: `give_up` never fires, or the search reports a different unmatched pair.

*What just happened.* Failure is a code path, not an absence of one. The algorithm does not
fail by crashing or by running out of time: it searches every alternating path from `f_x[1]`,
finds no unmatched unknown at the end of any of them, and returns. `give_up` is that return.

That is why `matching.md` could state a rank deficiency of exactly 1 — the deficiency is the count
of equations for which this path exists and terminates without success.

---

## Station 4 — What this instrument can and cannot show you

> **Predict.** You have now seen the same algorithm succeed and fail. What could a debugger show
> you here that the animation could not, and what can it not show?

**Expected:** the recursion depth and the locals at each frame are visible only in the debugger; the
*overall* progress of the matching is visible only in the animation.

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

Whether the stack reads as a path. Station 2 is the whole point and it depends on the call stack
being legible in the VS Code UI, which no test reaches. If the frames collapse or the `eq` local is
optimised away, the station says nothing.

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

Or go back up: [The chain overview](hrw://lab/the-concepts)
