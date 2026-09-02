# Lab kinds, vocabulary, and the Act→Stop rename

**Purpose:** the plan for naming the lab system correctly, and the record of why each name was
chosen. **Status:** live plan, 2026-08-17. **Read when:** writing or converting a lab, or when
something in `docs/fixture-labs/README.md` looks arbitrary and you want the reasoning.

**Doug, 2026-08-17:** *"The labs are fundamentally important for this HRW project. We have to get
the concepts and names right."*

---

## 1. How this started, and why the first answer was wrong

Doug, running the labs: *"the whole 'Act' thing keeps distracting me while I'm running labs.
'Act' is something that I associate with a theater or movie script."*

**The association is not a quirk — it is the word's origin, and it fights the design.** A script's
acts are things you *watch*. The README's own rule 4 says the falsifiable numbers are what makes
the reader *"an instrument rather than an audience"*. So the label cast Doug as the audience for a
performance, inside a document whose entire premise is that he is running the experiment.

**And nobody ever chose it.** The first concept lab, `dae-construction.md`, shipped on 2026-08-03
with `## Station 1` … `## Station 5`. `matching.md` landed *later the same day* with Acts, and its commit
message uses both words in adjacent sentences: *"its **stops** pause on algorithm STEPS"* and
*"**Acts** chosen by counting displacement steps."* `dae-construction` kept its Stops for thirteen
more days and only became Acts on 2026-08-16, as a side effect of the template conversion. Nothing
in `DECISIONS.md` argues for the word.

**The intermediate proposal, and why it was dropped.** Doug considered calling the units
*observations*. Two things killed it: `README.md` already uses "a list of observations" for the
**failure mode** (a lab whose units could be reordered), and — the same argument that killed
"Act" — an observation is something you make *after* looking, so naming the unit for its second
half drops the prediction, which is the half that makes the reader an instrument.

**What survived from that proposal, because it was right:** *observation* is the correct word for
**what Doug produces** — what he found, and whether it matched. That had never been named.

| | what it is | whose it is |
|---|---|---|
| **stop** | a question, a prediction, a look | the document's |
| **observation** | what was found, and whether it matched | **Doug's** |

---

## 2. The taxonomy

**Doug's model, 2026-08-17:** *"'lab' is the top-level noun… Your role is lab guide. There is
more than one kind of lab, with each kind of lab having its own goal. While all kinds of labs
have stops, each kind of lab might have different activities at its stops."*

**The corpus agrees, with one refinement Doug did not claim.** Counted across all 22 fixture labs:

| kind | labs | stops | Predict | Expected |
|---|---|---|---|---|
| **Concept** | 10 | 43 | **1/stop** | 1/stop |
| **Feature** | 3 | 20 | 0 | 1/stop |
| **Failure** | 6 | 24 | 0 | 1/stop |
| **Adjudication** | 2 | 8 | 0 | 1/stop |
| **Hub** | 1 (`the-concepts`) | 0 | 0 | 0 |
| **Ad hoc** | `.hrw-bridge/answer.md` | any | any | any |
| **Bug report** | none yet | — | — | — |

**The refinement: the invariant is `Expected`, not the activity.** Predict is exactly **zero** in
every non-concept lab and almost exactly **one per stop** in every concept lab. No gradient, no
partial cases — that is a design, not a backlog.

**So `Expected` is what makes a lab a *test* rather than an explanation**, which is the repo's
stated reason labs exist at all. `Predict` is merely *how a concept lab earns* its Expected. A
feature lab earns the same claim by having the reader **do** the action; a failure lab by having
them **read** the diagnosis; an adjudication lab by **asking another implementation**.

**This corrects a framing that was steering future work.** The README presented the
Predict/Look/Expected template as *"the shape of every act"*, applied *"as labs are touched, not
as a campaign"* — which reads as *"the other twelve are unconverted and will get predictions
eventually."* Claude asserted exactly that on 2026-08-17 (*"unconverted, not differently
designed"*) and the count says the opposite. **Conversions stop at the concept labs.**

### Why "Concept" and not "Math"

Doug's first word was *math lab*. It is wrong for at least two of the ten: `solve-lowering` is
memory layout and `connect-expansion` is language semantics (MLS §9.2). Neither is mathematics.

**And "curriculum" — the previous name — was already taken.** `CHARTER.md` §4.2 uses *curriculum*
for the seven-arc learning programme. So the lab kind had been borrowing the charter's word for
something broader. **Concept removes an overload rather than swapping a label.**

### Two kinds are Claude's inference, not Doug's taxonomy

**Adjudication** (`the-oracle`, `structural-vs-numerical-rank`) — the goal is settling a question
HRW *cannot* settle, using System Modeler or Wolfram. These already mark every stop with the
instrument it uses (📐 HRW, ⚙ System Modeler, 🧮 Wolfram), a convention invented ad hoc and never
written down. **They demonstrate that activity varies per *stop*, not only per kind.**

**Hub** (`the-concepts`) — no stops at all; a table of links into the concept labs. Whether
this is a kind or simply "not a lab" is Doug's ruling to make.

---

## 3. The name collision, and the rule that settles it

**There is a real collision, and it is exactly four lines.** Three of them the rename would
*create*; one already ships.

| sense | where it lives | sites |
|---|---|---|
| **lab stop** — the unit | headings, `hrw://…/station/<slug>`, `parse_stations`, CATALOGUE | — |
| **a compile halting** | failure labs, whose *subject* is stopping early | **1** |
| **a debugger stop** | `matching-live.md` only | **3** |
| the ⏹ Stop button | the transport bar | **0** — no lab mentions it |

**A claim made and withdrawn:** that the capability labs coexisting with the ⏹ Stop button proved
the collision tolerable. **There was never any contact** — no lab references the transport bar —
so there was no coexistence to learn from. The button is a non-issue for the opposite reason.

### The rule

> **"Stop" is a noun only for a lab stop.**

The **verb is free**: *"the compile stops at Parse"* cannot be misread as a unit. Of the ~17 noun
uses in the corpus, 13 are already the unit sense and correct.

**Both colliding senses already have better words in this repo**, so the fixes are corrections
rather than dodges:

| site | now | becomes |
|---|---|---|
| `matching-live:31` | *"learn what a stop is named"* | *"learn what an **anchor** is named"* |
| `matching-live:77` | *"flat at every stop"* | *"flat at every **break**"* |
| `matching-live:88` | *"ask about a stop"* | *"ask about a **break**"* |
| `failure-typecheck:93` | `Station 4 — Compare with a stop` | `Station 4 — Compare where they halt` |

The first is the clearest gain: the things being named **are** the anchors (`decision`, `recurse`,
`give_up`, `push`, `gate`), so the old wording was less accurate as well as ambiguous.
`matching-live` already writes `⬤ Break at the free-versus-displace decision` and the scheme is
`hrw://breakpoint/`, so **break** is the lab's own vocabulary, not an invention.

**`failure-typecheck:93` ships broken today** — its own table's column header is `stops at`.

---

## 4. The governing principle for execution

**Nothing that currently works changes shape.**

Doug, approving this plan: *"all of your phase 1 concept labs have been great. You have completely
nailed that format."* That format is validated by **his runs**, which is the one signal Claude
cannot generate. Therefore:

- **The concept template is frozen.** Act→Stop is a relabel. Not one word of
  setup → **Predict** → ▶ Look → **Expected** → **Falsified if** → *What just happened* moves.
- **Templates for other kinds are derived from labs that already work**, by reading what
  `node-pointing`, `failure-parse` and `the-oracle` actually do. **No template is invented.**
- **The bug-report kind gets no template**, because no instance exists. It gets a stated goal and
  an `unbuilt:` tag. Enshrining a convention before it is known to work is how a bad one becomes
  load-bearing.

**And the danger this plan carries, stated so it can be checked later:** a documented convention
can become a checklist that Claude writes *to*, and checklist-driven writing gets worse in a way
that is invisible from inside. Two guards — the checkers verify **structure only** (kind declared,
predictions present or absent, an Expected per stop) and **never** prose, judgement, or what makes
a stop worth stopping at; and the concept template is frozen at the shape Doug validated.

**The signal that this went wrong:** Doug reports that a lab reads as going through the motions.
That is not something a test can find.

---

## 5. Phases

**All phases executed 2026-08-17.** The decision record is `DECISIONS.md`, same date.

| # | phase | scope | |
|---|---|---|---|
| 0 | **this document** | committed before any edit lands | ✅ |
| 1 | **vocabulary rules** into `fixture-labs/README.md` | §2 and §3 above | ✅ |
| 2 | **the rename + the 4 collisions** | 110 occurrences in 10 labs, 36 refs outside | ✅ |
| 3 | **kinds machine-readable** | `<!-- kind: … -->` on 22 labs, one template per kind | ✅ |
| 4 | **checkers** | **5** checks, each reverted and confirmed to fire | ✅ |
| 5 | **regenerate + gate** | 689 lib + 2 `msl_resolve`, clippy clean | ✅ |

**Two things came out different from the plan, both found while executing:**

- **`matching-live.md` also had a `## Scene 0`** — more theatrical than "Act", and unnoticed when
  the plan was written. It became `Station 0`, following `frame-seeking.md`'s existing one, and the
  lab gained the stop/break/anchor vocabulary note.
- **Seven checks became five.** *"Prose agrees with the tag"* was dropped: `connect-expansion.md`
  opens with its own crafted lead and declares no kind in prose, and requiring one would have
  edited the template Doug validated. The tag is required; a prose sentence is optional. And
  *"`matching-live.md` uses no noun `stop`"* was wrong as stated — that lab legitimately calls
  **its own units** stops. It became: the vocabulary note must be present, and the three phrasings
  that were actually wrong must not return.

### Phase 2 detail

- `## Act N` → `## Station N`: 43 headings across `connect-expansion`, `dae-construction`, `matching`,
  `matching-live`, `blt-ordering`, `tearing`, `index-reduction`, `initialization`,
  `solve-lowering`, `events`.
- In-lab cross-references (`Act 2`, `Acts 1–3`, *"the previous act"*).
- `**A curriculum lab.**` → `**A concept lab.**` (8 labs); `matching-live`'s *"pass-two lab"*
  line gains its kind.
- The four collision sites in §3.
- **19 dangling `Act N` references in `src/`** → Stop. **Six more are verbatim Doug quotes and stay
  exactly as written** — editing a quote to match a later rename falsifies the record.
- `docs/ideas.md`, `docs/question-ledger.md`, `docs/upstream-issues.md`,
  `docs/specimen-notebook/OverDeterminedShaft/purpose.md` updated.
- **`DECISIONS.md` and `CHARTER.md` are historical record and are not touched.**

**Link risk: none.** The corpus contains exactly **one** `hrw://…/station/<slug>` link, targeting a
failure lab, so no slug in flight changes. **Nothing in `src/` parses the word "Act"** — the 25
occurrences are all prose.

### Phase 4 checks

| check | catches |
|---|---|
| every lab declares a known kind, prose agrees with the tag | drift between the two |
| a **concept** lab has ≥1 Predict per numbered stop | a concept lab shipped without its engine |
| **feature / failure / adjudication** labs have **zero** Predict | "converting" a lab that was already right |
| every numbered stop has an `Expected` | the universal invariant of §2 |
| no lab heading uses `Act` | rename regression |
| `matching-live.md` uses no noun `stop` | the §3 collision, enforced **exactly** rather than heuristically |
| a `<lab>.md Station N` reference in `src/` resolves to a real heading | the 19 dangling refs recurring |

---

## 6. Deliberately out of scope

- **A bug-report lab template.** No instance exists. Its audience is *Rumoca maintainers, not
  Doug* — the first lab kind whose reader is not him, which flips who judges it under the
  two-audience rule and means his run cannot validate it.
- **Changing `parse_stations`** so Play runs only numbered stops. Real imprecision — `CATALOGUE.md`
  lists "What this lab cannot check" as a stop — but it changes autoplay behaviour and deserves
  its own decision rather than riding along. <!-- unbuilt: parse_numbered_stops -->
- **Renaming the ⏹ Stop button.** No contact surface; see §3.
- **`CHARTER.md`.** Its "curriculum" means the seven-arc programme, and §4.2 is settled.

---

## Further reading

- 🤖 [`fixture-labs/README.md`](fixture-labs/README.md) — the conventions this plan installs
- 🤖 [`../DECISIONS.md`](../DECISIONS.md) — the decision record, including the 2026-08-17 entry
- 👤 [`CHARTER.md`](CHARTER.md) — Decision 8, the instrument assumes the reasoner
