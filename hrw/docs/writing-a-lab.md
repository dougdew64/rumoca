# Writing and editing a lab — the mechanics

**Purpose:** the keyboard-level procedure for producing a lab — the per-kind templates, the
`hrw://` wiring a hub lab requires, and the edit/gate loop with its two traps.
**Status:** procedure. Follow it; do not re-derive it.
**Read when:** you are about to create or edit a lab. **What a lab may CLAIM, and the pedagogy
that decides its content, stays in** [`fixture-labs/README.md`](fixture-labs/README.md) — read
that first; this file is what you consult once you are writing.

*Split out of `fixture-labs/README.md` on 2026-09-22, when that file reached its 80,000-byte
reading ceiling with 66 bytes to spare and could not accept a new rule. The ceiling's own
instruction is "it wants splitting, not a bigger number", and Decision 11 says where the pieces
go: procedure lives in a procedure document. Nothing here was rewritten in the move.*

---

## A lab the overview links into must link back — with `hrw://`, twice

**Doug, 2026-08-17:** *"There's a top-level lab which links to subordinate labs. I really want
to be able to navigate backward from a subordinate lab to the top-level lab so that I can then
navigate downward to another subordinate lab."*

[`the-concepts.md`](fixture-labs/the-concepts.md) is a **hub**: ten rows, each an `hrw://lab/<name>`
link into a phase lab. Those links ran one way only, so running the chain meant reopening the
picker between every pair — with the hub sitting alphabetically among its own children, at
position 21 of 23.

**The convention is two back-links per lab**, and each placement answers a different moment:

```markdown
# Fixture lab — <phase>: <the idea>

[The chain overview](hrw://lab/the-concepts)
```

- **After the H1** — for *"wrong lab, take me back"*, before any reading has happened.
- **In the closing section** — `Or go back up: [The chain overview](hrw://lab/the-concepts)`
  — for the reader who finished and wants the next phase.

**AND THAT GOVERNS EVERY LINK IN A LAB, NOT ONLY LINKS TO LABS** — a rule stated three times
about whichever file type was in front of us, and evaded three times by the next one:

**This table defines the `hrw://` verbs, so it is where the rename lands.** `hrw://lab/<name>`
becomes `hrw://lab/<name>` in the atomic pass — **every occurrence rewritten, no alias** (Doug,
2026-09-01: *"we are replacing the concept of tours with the concept of labs"*).
`fixture_lab_links_all_resolve` makes that safe: a missed link fails by name rather than rotting.
The other four verbs are unaffected.

| to reach | write |
|---|---|
| another lab | `hrw://lab/the-concepts` → **`hrw://lab/the-concepts`** at the rename |
| a doc under `hrw/docs/` | `hrw://doc/upstream-issues.md` — nesting allowed |
| **a source file, at a symbol** | `hrw://src/hrw/src/bridge.rs#resolve_source` |
| a Wolfram notebook | `hrw://notebook/structural-vs-numerical-rank.nb` |
| a web page | an ordinary `https://` link — **the browser is right here** |

**Every code name a grounded lab states should be an `hrw://src` link** *(2026-08-31)* — Doug,
pointing at *"`connections/mod.rs` uses union-find"*: *"This reference and others like it would be
much more helpful as links to the code files in VS Code."* A name he cannot reach is a citation;
one he can click is the source. **Name the symbol, never a line** — the line is computed at click
time, so a link that resolves is right by construction, while `tech-debt.md`'s `worker.rs:3434`
rotted inside a day. Functions, types, enum variants and struct fields all resolve.

**This is the half that pays for grounding:** `fixture_labs_reference_files_that_exist` resolves
every one in the FAST suite, so a renamed symbol fails a test rather than a run.

**AND IT IS WHAT MAKES THE CONVERSATION SAFE, NOT ONLY THE PROSE** *(2026-09-01)*. Rule 9 records
that grounding is the only defence that transfers to the bench, because no checker can see what
Claude says there. **An `hrw://src` link is that defence made operational:** Doug can open the file
and refute the answer while it is being given. A cited symbol he cannot reach is a claim he must
take on trust.

**SYMBOL-NOT-LINE IS THE BEST DESIGN IN THIS DOCUMENT, and it is worth copying rather than merely
obeying.** Charter Decision 13 says perishable specifics do not belong in durable text. This rule
does something stronger: **it makes the perishable specific unrepresentable.** There is no way to
write a rotting line number into an `hrw://src` link, because the line is computed at click time
and a checker resolves the symbol. `tech-debt.md`'s hand-written `worker.rs:3434` rotted inside a
day; this cannot. **When building the loop target from the tier table, aim here — the best loop
does not catch the error, it removes the way to make it.**

`doc_citations::no_lab_links_to_a_bare_file_path` fails on anything else, in the fast suite, on
the day it is written; **the three defects and the reasoning live on that test**, not here.

`doc_citations::every_lab_the_overview_links_to_links_back` derives the list from the overview's
own links and reports the two failures separately, because *no way back* and *a way back that
goes nowhere* look identical in a diff.

**And the hub sorts first in the picker**, with a separator beneath it
(`LabState::picker_order`). That was chosen over a dedicated "up" button because the transport
bar already sets the panel's width floor and another control raises it — and because a capability
lab has no parent to go up to, so the button would be dead most of the time.

---

## The templates — one per kind

**Every kind's template ends the same way: a claim that can fail.** What differs is how the station
earns it. Each template below is **derived from labs that already work**, not designed — read the
named exemplar before writing a new lab of that kind.

> **THE SKELETONS BELOW STILL SAY `Station N`, AND THAT IS DELIBERATE UNTIL THE ATOMIC RENAME.** Rule
> 16 settled the vocabulary — the unit is a **station** — but these skeletons are *copied* when a
> new lab is written. Updating them now would produce new labs saying `Station` while all 23
> existing labs say `Stop`, which charter Decision 15 calls **worse than no rename**: two words for
> one thing, and no way to tell which is current. **Do not "fix" them individually.** They change
> in the one atomic pass, with every lab and every `hrw://lab/<name>` link.

### Concept — `connect-expansion.md`

**Doug, after running it: *"That is the template for all other labs."*** — and after the drafting
sweep: *"all of your phase 1 concept labs have been great. You have completely nailed that
format."* *(His words, 2026-08-17, before the numbering was retired; "phase 1" there is drafting.)*

**This template is frozen.** It is validated by Doug's runs, which is the one signal Claude cannot
generate, so it changes only on his report. The shape of every station:

```markdown
## Station N — <a question, not a topic>

<setup: the least that makes the prediction possible>

> **Predict.** <a question with a committed answer>

`[Look — <Specimen> → <Stage> → <SubView>](hrw://load/…)`

**Expected:** <the answer, exact>

**Falsified if** <what would refute it>

### What just happened

<the explanation, only now>
```

**Five things make it work, and four of them are not the format:**

1. **Stations chain.** Each prediction is answerable from the previous station's *result* — nodes in
   Station 1 become the input to Station 2's equation count, which becomes Station 3's row-pairing. **A lab
   whose stations could be reordered is a list of observations**, which is what this one was before.
   *(The chaining is a property of the content, not of the word: it survived the rename from "act"
   and must not be lost with it.)*
2. **Every term is defined at first use, and one word never does two jobs.** This lab needs three
   levels — **connector**, **node**, **connection set** — and conflating any two of them broke it
   three separate times. Fixing the wording was never enough; the levels had to be named.
3. **Say where a claim is *not* visible.** A flow set of *n* prints as one row naming all *n*; a
   potential set prints as *n* − 1 pairs and its size appears nowhere. Stating that turned the
   lab's most persistent confusion into its spine. **If a number you assert cannot be found on the
   screen, say so in the station that asserts it.**
4. **Numbers are declared falsifiable up front.** The lab opens by saying its counts come from
   generated traces and asks to be told when one disagrees — which is what makes the reader an
   instrument rather than an audience.
5. **No historical asides.** See below.

**A `Station 0` is legitimate and carries no prediction** — it is setup, with an expectation to check
and nothing to predict. `matching-live.md` and `frame-seeking.md` both have one.

### Feature — `node-pointing.md`

**No prediction, and that is correct.** You are being asked to *do* the action; there is nothing to
guess, because the point is whether clicking the thing does the thing.

```markdown
## Station N — <the action, imperatively>

`[<the link that performs it>](hrw://stage/Structural/Tree/node/…)`

**Expected:** <what changes on screen, precisely enough to be wrong>
```

**Keep it narrow — one capability per lab.** The scarce resource is attention per expectation, and
a failed station in a narrow lab implicates exactly one feature. And **say where to look**: several
stations expect a status-bar notice, and a reader who does not know that reports "nothing happened".

### Failure — `failure-parse.md`

**You read a diagnosis rather than predicting one.** The specimen is stated up front with the line
that breaks it, because the interest is in what the compiler *says* and how far it gets.

```markdown
**Specimen:** `<Model>` — <the one thing wrong with it>

**The question to hold:** <what the reader should be wondering>

## Station N — <what this pane reveals>

`[<load link>](hrw://load/…)`

**Expected:** <the diagnosis, or "not reached", exactly>
```

**Close with `## What to bring back`** — open questions for Doug, since a failure lab's real
output is a design opinion about whether the diagnosis is actionable.

### Calibration *(was: adjudication)* — `the-oracle.md`

**The station's activity is asking a reference implementation**, so every heading carries the
instrument as an emoji: 📐 HRW, ⚙ System Modeler, 🧮 Wolfram.

```markdown
## 📐 Station N — <what HRW claims>

**Expected:** <HRW's answer>

## ⚙ Station N+1 — Ask the other implementation

**Expected:** <what the other tool says, and whether they agree>
```

**Claude evaluates every notebook cell through the kernel first**, then ships it for Doug to
evaluate — the station that lands is the one he checks himself. Fixture notebooks are versioned in
[`notebooks/`](fixture-labs/notebooks/); ad hoc ones are ephemeral.

### Bug report — not built <!-- unbuilt: bug_report_lab -->

A lab that narrates the steps of a failure for a screen recording, to hand a Rumoca maintainer a
reproduction. **No template until an instance exists** — writing one from imagination is how a
convention becomes load-bearing before it is known to work.

**And it is the first kind whose reader is not Doug.** Its audience is maintainers, which flips who
judges it under the two-audience rule ([`../../DECISIONS.md`](../DECISIONS.md)): the test becomes
*"did a maintainer act on it without asking Claude?"* — so **Doug's run cannot validate one.**

### Keep the lab's history out of the lab

**A lab is written for Doug; a changelog is written for Claude.** No *"reworded after Doug
asked"*, no *"corrected 2026-08-13"*, no dated parentheticals. They accumulated to eight in one
file and made it read as a maintenance log.

That history is not lost, it is **filed where it belongs**: the decision and its reasoning in
[`../../DECISIONS.md`](../DECISIONS.md), the question that prompted it in
[`../question-ledger.md`](question-ledger.md), and the mechanism in **a code comment or on the
test that enforces it**. A lab states what is true now.

*(That last route said `../compiler-phases/` until 2026-09-01 — the sixth site sending HRW material
into what turned out to be **Rumoca reference documentation**. An HRW mechanism belongs beside the
HRW code that implements it.)*

**THE SCOPE IS A LAB, AND RULES FILES DELIBERATELY DO THE OPPOSITE — including this one.** A lab is
read to **learn**, so history sits between the reader and the idea. A rules file is read to
**decide**, and the failure it must prevent is a session re-deriving a rule that was retired.

**That is not hypothetical here.** On 2026-09-01 alone, retired rules came back five separate ways:
running discipline returned as a queued `last_walked` feature, `compiler-phases/` was described as
a deferral store in six places, and the `Predict` counts outlived the checker that superseded them.
**Each would have been prevented by one dated line saying what changed.** So this file carries its
corrections and the labs carry none — the same test decides both: *if this note were gone, would
the rule become easier to get wrong?*

**A dated note earns its place by preventing a re-derivation, and stops earning it when nobody is
tempting.** Those added during this sweep are load-bearing now and will not be forever; the
discriminator for removing one is whether anyone has tried to re-derive that rule since.

## Running a lab edit — the loop, and the two gate traps

*(Moved here from `CLAUDE.md`'s Current work on 2026-09-01. These are needed **while editing a
lab**, which is exactly when this file is read; they were filed under "what is in flight", which
they never were.)*

**THE LAB ITERATION LOOP, and it is not the gate:**

```text
cargo test -p hrw --lib -- --test-threads=1 doc_citations lab   # 6.1s -- while editing
cargo run -q -p hrw --example gen_lab_catalogue                 # 9.9s -- see the trigger below
cargo run -p hrw --example gate                                  # before the commit
```

**The third line is the RUNNER, not the plain fast suite** *(corrected 2026-08-31)*. It said
`cargo test -p hrw --lib` — which for a lab edit is the one gate that **cannot see the
change**, because the tests verifying guarded tables against a real compile are slow-gated off.
The runner picks LAB for a lab edit (11.1 s) and FAST otherwise.

**And the generator's trigger is not only a `##` heading** — that comment said `ONLY`, which the
blurb trap twenty lines below has contradicted since 2026-08-22. Regenerate if a `##` heading
**or the lab's first bolded line** moved; when in doubt run the checkers first and let
`lab_catalogue_is_current` tell you.

**TWO GATE TRAPS, both of which have cost the full gate before:**

- **`connect-expansion.md` is the one expensive lab — but only its five guarded tables are.**
  It is the only lab carrying `<!-- pane-groups -->` / `pane-origins` / `pane-frames` tables,
  which slow-gated tests verify against a real compile. **Editing one of those tables means the
  LAB gate** — 11.1 s, not FULL's ~101 — whatever the diff-grep says; **editing its prose does
  not, and never did.**

  **YOU NO LONGER HAVE TO REMEMBER THAT** *(built 2026-08-22)*.
  `doc_citations::editing_a_guarded_lab_table_needs_the_full_gate` compares every guarded region
  against `HEAD` and **fails by name in the FAST suite** if one changed, naming the marker and
  printing the FULL command. It is gated *off* under `slow-tests` — in a FULL run the real
  checkers are executing, so **the cheap gate is the only place the warning is useful.**

  **The gain is assurance, not permission.** Prose edits were always FAST; what was missing was
  any check that an edit believed to be prose actually was one. The filtered iteration line
  catches it, so a green `doc_citations lab` run now means FAST was genuinely the right gate.
- **Any `##` heading edit changes `CATALOGUE.md`.** Forget `gen_lab_catalogue` and
  `lab_catalogue_is_current` fails. The order is `cargo fmt` → generators → checks, and getting
  it backwards has cost the whole gate four times.

  **AND THAT IS NOT THE ONLY TRIGGER — the blurb is the lab's FIRST BOLDED LINE** *(found
  2026-08-22)*. `lab::catalogue` takes each lab's summary from the first line starting with
  `**`, so **inserting any bolded paragraph above the existing one silently replaces the
  catalogue's summary** — in this case with a mid-sentence fragment, *"MLS §9.3 requires connected
  connectors to be type-compatible, and Rumoca does check. Four"*. Nothing about headings was
  involved. **A new intro section goes BELOW the lab's opening bold line**, or the catalogue's
  description of that lab changes with it.
