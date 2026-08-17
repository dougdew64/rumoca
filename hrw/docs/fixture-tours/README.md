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
| [`matching.md`](matching.md) | **animation-based** — bipartite matching by augmenting-path search: greedy success, the moment the algorithm backs up and re-homes an earlier assignment, and a system that is square but structurally singular |

**An animation-based tour pauses on algorithm *steps*, not panes.** Its links are
`hrw://stage/<Stage>/<View>/frame/<n>`, and the frame numbers come from
`cargo run -p hrw --example frame_index -- <Model>`, which prints the ready-made link under each
step. **Do not transcribe the frame number by hand** — links are 1-based and the internal step
list is 0-based, and that tool spent a day telling authors otherwise.

**Why the "keep it narrow" rule below does not bind these.** That rule protects *attention per
expectation*, because a capability tour spends your surplus attention on finding off-stop bugs
in HRW. A curriculum tour is spending it on the concept instead. The rule it does keep is the
one that matters for both: **claims stay austere and trace-sourced, however long the prose
gets.** Length is bought with explanation, never with hedging.

#### A tour's job is to make the reader able to ask the next question, not to answer it

**Doug, 2026-08-16, having re-walked `connect-expansion.md` and then asked three detailed
questions from the panes:**

> *"You created a first draft of the connections tour, with the assumption that I knew nothing
> about connections. Then, I began walking the tour and iterating with you to improve that tour.
> And during those iterations, I gained the basic understanding… Now, I'm going back through the
> tour, am using HRW's panes to think of more detailed (not-so-basic) questions."*

**Three phases, and the third is what makes short tours correct:**

| phase | what happens | where the learning is |
|---|---|---|
| 1 | Claude drafts, assuming no knowledge | nowhere yet — a draft is a hypothesis |
| 2 | Doug walks it and iterates with Claude | **here** — the repair loop is the teaching (`vision.md`) |
| 3 | Doug re-walks, reads the panes, asks detailed questions | **here** — and the tour deliberately does not answer these |

**Phase 2 tests the prose. Phase 3 tests the instrument** *(observed 2026-08-16, when
`connect-expansion.md` became the first tour to complete all three)*. In phase 2 he follows the
tour, so the tour is what fails. In phase 3 he *explores* — clicking links out of order, reading
panes the prose never mentions, hovering things — so the **panes and the navigation** are what
fail. That day's phase 3 produced three teaching answers and, alongside them: a missing UI
explanation, a bridge that had stopped publishing what a pane drew, three dead scroll areas, tour
links that worked once per session, link navigation broken for nine of eleven stages, and a
divider that misremembered its width.

**None of those are connection-specific.** They are shared surfaces, so the expectation for the
*next* tour's phase 3 is that it finds far fewer — and if it does not, the finding is that phase 3
exercises something the tests still cannot reach, which is worth more than the individual bugs.

**The test for "not too little, not too much" is therefore operational, not aesthetic:** *could
this question have been asked before the tour?* One of the three that morning — *why must an
unconnected flow variable get an equation when an unconnected potential need not?* — is only
**askable** by someone already holding the *n*−1 versus exactly-1 rule, because the asymmetry it
asks about **is** that rule. A tour that pre-emptively answered it would have spent his attention
before he had a reason to want it.

So **write to the point where the reader can generate the question, and stop.** The answer belongs
in the conversation, where it can be shaped by what he actually noticed —
charter Decision 8's split, arriving from the other direction: *the noun is assembled by mouse,
the verb is an unbounded utterance.*

The full account, including what made each of the three questions click, is in
[`question-ledger.md`](../question-ledger.md), 2026-08-16.

#### Two passes over every subject: the idea, then the code

**Agreed with Doug 2026-08-15**, generalising a split that had already happened by accident
between [`matching.md`](matching.md) and [`matching-live.md`](matching-live.md):

> *"The connections tour which we just completed is about the concepts, math and algorithms, but
> not about the rumoca code… We're going to make two walking passes through the tour subjects.
> During the first pass, we will focus on concepts, math and algorithms… During the second pass,
> we will focus on the rumoca code."*

| pass | subject | lab | example |
|---|---|---|---|
| **1** | concepts, mathematics, the algorithm as an idea | the HRW pane | `connect-expansion.md` |
| **2** | how Rumoca implements it | the source and the debugger | `matching-live.md` |

**Pass 2 is not defined by the debugger**, even though `matching-live.md` is a debugger tour.
Stepping is the sharpest instrument for reading an algorithm's *behaviour*; it is useless for
why a phase is organised as it is, why a type sits at one IR boundary and not another, or why an
origin is a `String` on one side of DAE construction and an enum on the other. Those are read,
not stepped. **So `-live` in a filename names the instrument, not the pass** — prefer a suffix
that names the pass for new ones, and leave `matching-live.md` alone rather than break its links.

**The template still applies, with the lab swapped.** *Predict → look → falsified if →
explanation after.* Predict what a function returns, which branch runs, what the union-find holds
at this step — then step, and check. A pass-2 tour that merely narrates source is prose competing
with the reader's own editor.

**What this fixes, and it is the reason to adopt it rather than a nicety.** The depth rule sends
premature detail to [`../compiler-phases/`](../compiler-phases/), which is raw, unsummarised, and
**read by nobody** — a graveyard dressed as a database. Under two passes that same material is
the *source* for a pass-2 tour, so exiling it is deferral rather than disposal. It also lets a
pass-1 tour be **ruthless** about excluding implementation detail, because there is somewhere
specific for it to go.

**Write pass 2 when he starts pass 2**, not alongside pass 1. The rule to write while you still
know what should happen is satisfied by capturing the material in `compiler-phases/` as it comes
up; composing it into a walk before it is wanted is building what nobody has asked to read.

Cross-platform tours may route through Wolfram Desktop or System Modeler when the point cannot
be made in HRW. Their notebooks are versioned in [`notebooks/`](notebooks/) — a *fixture*
notebook is kept for the same reason a fixture tour is, while an ad hoc notebook is ephemeral.
Claude evaluates every cell through the kernel first, then ships them for **you** to evaluate:
the stop that lands is the one you check yourself.

## Rules for writing one

### The rule the others now serve: prose to the first PREDICTION, then the pane

**Agreed with Doug 2026-08-12, and curriculum tours are to be written on this assumption.**
It came out of his own account of walking the first two: the tours felt like **books** —
gaps, and a struggle to read — while the conversation was unlike a **lecture**, because
questions get answered *and the tour gets fixed*. See
[`../vision.md`](../vision.md), "Why this beats books and lectures".

**The RHS is a laboratory, not an illustration.** Doug: *"the RHS will be partly helpful for
you to demonstrate what your tour prose attempts to explain, and the RHS will be mostly
helpful as a kind of lab for me to explore and test my expectations."*

**So the unit of a curriculum stop is a prediction the reader commits to before looking**, and
the prose before it exists only to make that prediction possible:

1. **Explain to the first point where a prediction is possible — then stop.** Not to
   comprehension. *"There should be three groups"* is crude, falsifiable and available after
   two sentences, and checking it is the fastest way to find out those two sentences were
   misread. **Prose that continues past the first prediction is prose competing with the
   lab.**
2. **State what would FALSIFY it, not only what to see.** A description invites agreement; a
   prediction invites a look. This is the existing violability rule with its purpose widened —
   it was written so a tour could **test HRW**, and it turns out to be how Doug learns, so it
   is now doing two jobs and gets stricter rather than looser.
3. **Explanation comes AFTER the look**, not before. Explaining first leaves nothing to be
   wrong about, which is comfortable and teaches less.
4. **A stop whose pane cannot falsify anything does not belong in a tour.** Move it to
   [`../compiler-phases/`](../compiler-phases/) as prose. A tour is for claims a pane can
   refute.

**And the reader audits the prose, which is the half Claude cannot do.** Every *count* in
these tours is read from a generated trace and is sound. Doug: *"if ever during that
learning process I find that the RHS does not agree with the prose, I will report that to
you."* So a stop should make disagreement easy to notice, which is the same demand as
rule 2.

**Rendering claims are no longer wholly unverified — as of 2026-08-13, for published
panes** *(this paragraph used to say every one of them was, and that is now false)*. HRW
publishes the pane on screen to `.hrw-bridge/view.json` as **the renderer's own input**,
and because that value is a pure function of a headless compile, a test can check a tour
against it. So:

- **A pane with a `to_bridge_json` gets a group table**, marked `<!-- pane-groups -->`, and
  `doc_citations::tour_group_tables_match_the_real_equation_sheet` verifies every label and
  count against a real compile. Today that is **Flatten → Equations** only.
- **Everything else is still unverified**, and so is everything about *pixels* on any pane —
  whether a `category` is drawn as a heading, whether rows are legible, whether something is
  scrolled out of view. The checker verifies **content, never rendering**.

**What this cost to learn:** Doug walked `connect-expansion.md` against the real pane on
2026-08-13 and found **six** disagreements in one sitting — wrong group headings, a layout
implied by a table that had no counterpart, and a claim that two renderings of an equation
lived in different panes when both are columns of the same row. Four of the six are now the
kind a test catches.

### The template — `connect-expansion.md`

**Doug, after walking it: *"That is the template for all other tours."*** It is the worked example;
read it before writing or converting one. The shape of every act:

```markdown
## Act N — <a question, not a topic>

<setup: the least that makes the prediction possible>

> **Predict.** <a question with a committed answer>

[▶ Look — <Specimen> → <Stage> → <SubView>](hrw://load/…)

**Expected:** <the answer, exact>

**Falsified if** <what would refute it>

### What just happened

<the explanation, only now>
```

**Five things make it work, and four of them are not the format:**

1. **Acts chain.** Each prediction is answerable from the previous act's *result* — nodes in Act 1
   become the input to Act 2's equation count, which becomes Act 3's row-pairing. A tour whose acts
   could be reordered is a list of observations, which is what this one was before.
2. **Every term is defined at first use, and one word never does two jobs.** This tour needs three
   levels — **connector**, **node**, **connection set** — and conflating any two of them broke it
   three separate times. Fixing the wording was never enough; the levels had to be named.
3. **Say where a claim is *not* visible.** A flow set of *n* prints as one row naming all *n*; a
   potential set prints as *n* − 1 pairs and its size appears nowhere. Stating that turned the
   tour's most persistent confusion into its spine. **If a number you assert cannot be found on the
   screen, say so in the act that asserts it.**
4. **Numbers are declared falsifiable up front.** The tour opens by saying its counts come from
   generated traces and asks to be told when one disagrees — which is what makes the reader an
   instrument rather than an audience.
5. **No historical asides.** See below.

**Applied as tours are touched, not as a campaign.** `connect-expansion.md` is the first one
converted, because it was being revised anyway.

### Keep the tour's history out of the tour

**A tour is written for Doug; a changelog is written for Claude.** No *"reworded after Doug
asked"*, no *"corrected 2026-08-13"*, no dated parentheticals. They accumulated to eight in one
file and made it read as a maintenance log.

That history is not lost, it is **filed where it belongs**: the decision and its reasoning in
[`../../DECISIONS.md`](../../DECISIONS.md), the question that prompted it in
[`../question-ledger.md`](../question-ledger.md), and the mechanism in a code comment or
[`../compiler-phases/`](../compiler-phases/). A tour states what is true now.

### The rules this rests on

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
