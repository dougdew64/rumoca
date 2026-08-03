# Fixture tours — tests you walk

**Purpose:** what a fixture tour is, how to walk one, and the rules for writing another.
**Status:** 👤 reference, written for a human.
**Read when:** about to walk a tour, or about to write one. **These are tests, not
explanations** — that distinction governs everything below.

## What a fixture tour is

**A short sequence of clickable stops through HRW's own views, each with an expectation that
can fail.** You pick one from the row of tours at the top of Tour mode and click through it.

They exist because of a gap nothing else covers: **Claude cannot see the rendered UI.** The
test suite checks HRW's logic; a fixture tour checks that clicking the thing does the thing.
That is the half of verification only a human can do, and these make it cheap to do.

**They are versioned and kept**, unlike an *ad hoc* tour (`.hrw-bridge/tour.md`, gitignored,
regenerated per question). The difference is not permanence for its own sake — it is that a
fixture tour has **pass/fail criteria** and an ad hoc tour has prose, and prose rots. This
project retired 1,632 lines of explanation for that reason, and deleted a 1,071-line tour that
described a 7×7 matrix on a tab showing 48 equations.

**Only justified because something runs them.** `fixture_tour_links_all_resolve` parses every
link in this directory on every test run, so a vocabulary change breaks the build rather than
breaking a document quietly. **A saved tour nobody runs is stored prose with extra steps.**

## Walking one

1. Run HRW — `cargo run -p hrw` from the workspace root.
2. Open **Tour mode** and pick a tour from the row at the top.
3. Click each link in order and check the **Expected** line beneath it.

**Notices appear in the status bar**, along the bottom of the window. Several stops expect
one, and a reader who does not know where to look cannot check an expectation — which is a
real bug this suite has already produced.

**When something does not match, say so even if it looks minor.** Every off-stop finding so
far came from attention left spare by a short tour, which is why they stay short.

## The tours

**There are two kinds, and they are judged differently.** Both are run by
`fixture_tour_links_all_resolve` and both hold every `**Expected:**` line to being violable —
that discipline is common to all of them. What differs is what a walk is *for*.

### Capability tours — the subject is HRW

Each verifies one feature. A failed stop implicates exactly one thing.

| Tour | Verifies |
|---|---|
| [`node-pointing.md`](node-pointing.md) | pointing at a tree node, and following an identifier |
| [`frame-seeking.md`](frame-seeking.md) | stopping an animation on a given frame; addressing an equation |
| [`camera-aiming.md`](camera-aiming.md) | whether the canvas camera lands where a link says |
| [`structural-vs-numerical-rank.md`](structural-vs-numerical-rank.md) | **cross-platform** — two stops in HRW, then a notebook, because full structural rank with numerical singularity is a thing HRW cannot show |
| [`the-oracle.md`](the-oracle.md) | **cross-platform** — a model Rumoca accepts and System Modeler rejects |

### Curriculum tours — the subject is the compiler, and HRW is the instrument

Each teaches one step of
[`the-chain-of-problems.md`](../compiler-phases/the-chain-of-problems.md). **The prose is
load-bearing** (Doug, 2026-08-03): a stop is the explanation, and the pane is the evidence for
it. These are longer than a capability tour on purpose.

| Tour | Teaches |
|---|---|
| [`dae-construction.md`](dae-construction.md) | how the flat model becomes states/algebraics/parameters + residuals, why the equation count must equal the unknown count, and what an unbalanced model actually means — with excursions to Wolfram and System Modeler |

**Why the "keep it narrow" rule below does not bind these.** That rule protects *attention per
expectation*, because a capability tour spends your surplus attention on finding off-stop bugs
in HRW. A curriculum tour is spending it on the concept instead. The rule it does keep is the
one that matters for both: **claims stay austere and trace-sourced, however long the prose
gets.** Length is bought with explanation, never with hedging.

Cross-platform tours may route through Wolfram Desktop or System Modeler when the point cannot
be made in HRW. Their notebooks are versioned in [`notebooks/`](notebooks/) — a *fixture*
notebook is kept for the same reason a fixture tour is, while an ad hoc notebook is ephemeral.
Claude evaluates every cell through the kernel first, then ships them for **you** to evaluate:
the stop that lands is the one you check yourself.

## Rules for writing one

**One capability per tour, and keep it narrow.** The scarce resource is **attention per
expectation**, not the number of walks. A wide tour consumes the surplus that produces
off-stop findings rather than multiplying them, and a stop failure in a narrow tour implicates
exactly one feature.

**Every `**Expected:**` line must be violable.** Write what would be *different* if the
feature broke — a number, a named field, "nothing moves", "the counter goes down". *"Mostly
collapsed"* where the truth is **fully** collapsed tests nothing, and hedged expectations
teach the reader to skim, which defeats the point.

**An expectation must say WHERE to look**, not only what to look for. A stop was once
correctly refused with the reason on screen, and reported as "nothing happened", because the
tour never said notices live in the status bar.

**Write the tour while you still know what should happen.** Both the worst expectations ever
shipped here described behaviour Claude had *not* just built.

**Past ten or so fixtures this needs a selection principle** — walk whatever just changed,
plus one stale one — and **visible staleness**: nothing currently catches a tour whose
*expectations* rot, only its links. "Last walked" is derivable from the `tour-link` entries in
the action trail, and nobody has built it yet. <!-- unbuilt: last_walked -->

## Further reading

- 👤 [`../architecture.md`](../architecture.md) — how tour mode and the `hrw://` link
  vocabulary work
- 👤 [`../../README.md`](../../README.md) — what HRW is, and the capture plan that keeps
  screenshots honest by taking them at these stops
