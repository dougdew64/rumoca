# Unattended run log — what each night did

**Purpose:** the record of every unattended run, kept so a lens is not pointed twice at a region
already established as clean.
**Status:** record. **Not** the rules — those are
[`unattended-runs.md`](unattended-runs.md), and reading them is what binds a night.
**Read when:** choosing what to queue, or asking whether a lens is spent.

*(Split out of `unattended-runs.md` on 2026-08-31. It had grown to 643 lines of which **369 were
this log** — so the document a session must read before acting was 57 % history, and Doug reported
he could no longer understand the rules. A record and an authority have different readers and
different lifetimes; keeping them in one file made the authority unreadable to protect the
record.)*

## Run log

### Night 6 — 2026-08-31. Item 1: the queued conversion is NOT WORTH DOING, and the plan's premise was wrong

**Zero conversions, deliberately.** The queued item said *"71 call sites use
`compile_specimen_shared` and are free after the first; **17 call `w.compile(` directly** and pay
~3.4 s each, because every specimen compile re-resolves the whole MSL."*

**That last clause is false for most of them.** Fourteen direct sites survive (twelve in
`worker/tests.rs`, two in `worker/test_msl.rs`, one of which *is* the helper's own body). **Seven
build a bare `WorkerState::new()`** — no libraries set, so there is no MSL to re-resolve and the
compile is nearly free. Measured, three runs each:

| | measured |
|---|---|
| two bare-session compiles, together | **555 / 571 / 548 ms** — mostly cargo's own start-up |
| one MSL-loaded compile | **3476 / 3498 / 3408 ms** |

**Converting those seven would make the suite SLOWER**, because `compile_specimen_shared` compiles
against the MSL. The saving the item was queued to collect does not exist there.

**And every site that DOES pay the 3.4 s cannot convert**, each for its own reason — which is the
work the item actually asked for:

| site | why it stays |
|---|---|
| `tests.rs:65` | the compile is **setup**: it registers the specimen document on `w` so the following `open_def` resolves. The helper returns a cached value and mutates no `WorkerState`. |
| `tests.rs:1420` | a **scratch** specimen in `.hrw-bridge/specimens/`; the helper takes a curated specimen *name* and cannot reach it. |
| `tests.rs:1653` | a **sequencing** test — a library compile must happen first *in the same session*. A cached result has no session. |
| `tests.rs:3653`, `3793` | assert on **messages observed through a sink**. The helper replays nothing. |
| `tests.rs:2357` | `a_broken_specimen_does_not_poison_the_next_compile` — **about** fresh state, as the queued item already predicted. |

**No code changed, and hard rule 5 is why that is the right ending:** *"if an item cannot end in a
test that fails by name, it is not unattended work — record it and move on."* There is no test for
a conversion that should not happen.

### THE TRANSFERABLE PART: the estimate was built from a COUNT, not a measurement

*"17 sites × 3.4 s"* is arithmetic over names — the same shape `docs/ideas.md` #48 already records
**three times**, each proposal dying on contact with a clock. It survived here because the count
was true and only the per-item cost was assumed. **A number that is right about the numerator is
still not a measurement.**

### Night 1 — 2026-08-22. Three items, three commits, gate green on each, nothing pushed

| item | commit | what it found |
|---|---|---|
| 1 — `HrwLink` column read | `4f8239e8` | **no defect.** Parse arms are arity-disjoint, `describe` covers all twelve variants. Added a corpus-wide `parse→describe→parse` guard where only ten hand-picked literals were checked before. |
| 2 — per-stage wiring audit | `61407cee` | **a false claim.** `every_compilation_stage_has_a_tab_and_the_log_does_not` promised that a hand-written roster made a forgotten tab *"fail by name"*. It cannot: a stage absent from the tab array is absent from that list too, so nothing queries it. It caught **removal, never omission**. |
| 3 — same pattern elsewhere | `1358e89d` | the lab-picker test named **9 of 22** fixtures while its name claimed every one. Now derived from `fixture_labs()`. |

**Every claim was demonstrated by breaking it, not by passing.** Item 2's break is the one worth
keeping: with `Events` deleted from **both** the tab array and the old test's list — which is what
*"added and forgotten"* looks like — the old test passes and the new one fails naming `Events`.

### ✅ THE BOUNDARY CALL WAS RATIFIED — Doug, 2026-08-23

**The plan said "app.rs only"; items 2 and 3 landed in `stage_tabs.rs` and `ui_tests.rs`.** Claude
judged them in scope — both are app-side modules split out of `app.rs`, and item 2's agreed text
(*"every per-stage system"*) spans twelve files by construction, so a literal reading made the item
impossible. Doug: **"Your scope call was correct."**

**So the standing rule: "app.rs only" means the app side, not the file.** `worker.rs` and the
compile path remain the boundary that matters, and they are on the no-go list where they belong.

### Two things declined, and why

- **Extracting the tab roster into testable data.** It would have moved four long tooltip literals,
  and retyping them unattended risks a silent transcription error for no gain the source scan does
  not already give.
- **Testing the roster through the UI harness.** `ui_tests.rs` documents three traps there, one of
  which *"does not fail, it makes the test pass while checking nothing"*. Silent vacuity is the
  failure mode this protocol is built to avoid, so the compiler-checked route won.

**One near-miss worth recording.** Item 1 produced a synthetic companion test that was written,
run green, and then **deleted** — its four cases were already covered by literals in an older test
Claude had not known about. It would have shipped as a duplicate.

### Night 2 — 2026-08-23. Three items, three commits, gate green on each, nothing pushed

**On Doug's own machine, not the other one** — he retargeted it that evening and disabled sleep.
The machine check passed all five before starting, so the handoff table the plan opened with was
discharged rather than followed.

| item | commit | what it found |
|---|---|---|
| 1 — generalise the no-nested-scroll rule | `972e2c08` | **two real defects.** `alias_anim` (320pt) and `ic_plan_anim` (300pt) each nested a scroll area inside the one `app.rs` already wraps them in. Must-fire came free: the new checker failed on its first run, 4 violations across 2 files, no synthetic break. |
| 2 — column-read the `Animated` trait | `cabb4561` | **no defect, and that is the finding.** `which()` is unique, both dispatchers cover all eight, and the two views whose `live_state` diverges diverge *correctly*. Nothing held either fact; two guards now do. |
| 3 — the Pantelides acceptance ladder | `d67d04c3` | **a defect in a checker.** Five rungs written, rung 1 green and rungs 2–5 verified red. Naming the module `pantelides_ladder` silently retired #83's own claim of absence, because `symbol_is_defined` matched substrings. |

**Every claim was demonstrated by breaking it.** Making `alias_anim` delegate `live_state` fails
naming it; giving `ic_plan_anim` `which() == "matching"` fails on the collision; the four red rungs
were **run** rather than assumed red.

#### The lens worked, and the yield is entirely toolchain-or-Claude

Four ledger rows, **none of them Doug's** — the scroll pair, the prefix-matching resolver, and the
`mem::take` citation that had only ever resolved through that loose match. Night 1 asked for the
ratio to move toward the toolchain; on night 2 it is the toolchain the whole way.

**The lens the run log named — *a claim that outruns its evidence* — found three of the four.**
A per-file check whose rule was general, a resolver whose "definition-shaped occurrence" was a
substring, and a citation that resolved for the wrong reason.

#### A source check matched its own text FOUR times in one night

`connection_anim`'s retired test documented this trap once. It recurred in the ladder's
self-count, in the resolver's regression guard, and it is why both new checkers assemble their
needles with `format!` instead of spelling them. **The new instance is the one worth carrying: a
string literal in a *test fixture* is not a comment**, so writing `mod pantelides` as test data
defined the symbol the same test proves absent. The scanner skips `//` lines and nothing else.

#### Declined, and why

- **Rewriting the eight `seek` doc comments** that say "all eight views agree" — a hardcoded count
  in prose, true today. Eight files touched unattended to buy a reader nothing.
- **Renaming `pantelides_ladder`** to dodge the resolver collision. It would have hidden a live
  wrong-negative bug behind a module name, and the collision was the only reason anyone found it.

#### The three follow-ups this night queued were closed the same day, attended — `86e0b951`

`current_frame_context` is guarded, every animated view is now accounted for by the scroll rule as
either wrapped or a canvas, and the tab-roster column read found **a claim rather than a bug**:
`stage_view.rs` promised that forgetting to add a variant to an `ALL` roster was loud, and it was
not. Proven by dropping `AliasAnim` — `every_sub_view_slug_round_trips` **passed** while checking
eight of nine.

#### Owed to Doug

- **Both scroll fixes need his eyes.** Claude verifies content, never pixels, and neither
  scroll-area bug in this project's history was visible to `egui_kittest` — a clipped child is
  still in the accessibility tree. **Open Index Reduction → Alias on `Drivetrain`** (77
  eliminations, so the old cap showed under a quarter) **and Initialization → IC Plan on
  `RcCircuit`** (21 blocks against a 300pt cap), and say whether the lists now use the pane.
- **Nothing is pushed.** Three commits sit on `hrw` ahead of `origin/hrw`.

### Night 3 — 2026-08-23. Three items, three commits, gate green on each, nothing pushed

| item | commit | what it found |
|---|---|---|
| 1 — `drain_worker`'s six arms | `0b5a747f` | **a defect.** `DefTree` was the one arm with no staleness guard: it cleared the loading indicator for a request still in flight. Also pinned `CompileProgress`'s documented contract, which nothing checked. |
| 2 — `dispatch_hrw_link`'s twelve arms | `3a18ef0b` | **a stale claim.** `requires_specimen`'s doc said *"the three that do not need one"* while its list held **six**. Three other asymmetries checked and found correct. |
| 3 — the `has_*` availability family | `c71ba6e1` | **null on the hypothesis, and a missing must-fire.** The stranded-view clamp exists and is correct — but three tests all call it directly, so deleting its production call site left every one of them passing. |

#### THE RESULT WORTH CARRYING: `app.rs`'s ROUTERS ARE SOUND, AND THEIR CHECKS ARE NOT

**Four threads were chased and four were nulls**, each of which looked like a defect on
first read: `ShowSource` being the only arm to set `ui_mode`; no arm guarding sub-view
availability; `AimAtEquation` leaving a highlight set across stages; a caller using the alias
predicate without its stage test. **Every one is correct, and correct for a documented
reason** — because each was repaired once already, and the repair is commented in place.

**So the defects left in this file are in the checks, not in the code.** All three items
ended by guarding a mechanism that existed and was unverified, and item 3 is the sharpest
case: the guard was written *because* of a real 2026-08-19 defect, and nothing ensured it
still ran.

**That is the lens for the next `app.rs` night** — not *"find the broken arm"*, which four
nulls suggest is largely spent, but *"find the guard nothing invokes"*. Ask of each
protective mechanism: would deleting its call site fail anything?

#### Owed to Doug

- ✅ **The recorded finding was ruled and fixed, 2026-08-24.** `CompileProgress` replaced
  `self.stages` and invalidated nothing, so a recompile drew the previous compile's matrix
  over the current compile's report.
- **Nothing is pushed.** Three commits sit on `hrw` ahead of `origin/hrw`.

#### ⟶ AND THE RULING IS THE PART TO KEEP — Doug, 2026-08-24

Claude presented it as a **trade**: hold the last complete result, or rebuild per progress
message, with a cost column. Doug did not weigh the columns; he asked what the project's own
principles said. *"This project is for my education, and accuracy is required for that
education. Also, inconsistency causes learning friction."*

**Under those, it is not a trade.** The pane drew real, correctly computed data **attributed to
the wrong run** — the fiction class `CLAUDE.md` already names, and the same shape as the replays
removed on 2026-08-04, which were also real output of a real algorithm presented as the
compilation. And the tab colours advanced while the pane held still, so two things on screen
described one instant differently, during the single gesture whose purpose is to see a change.

**The lesson for a future session is about the framing, not the cache.** Offering a balanced
cost table for a question the charter already answers **invites a ruling that contradicts the
project's own rules** — and the cost side of that table was performance, which this file says
repeatedly is not what HRW optimises for. *When a decision looks like a trade, check first
whether one option is a documented fiction; if it is, there is no trade to present.*

### Night 5 — 2026-08-25. `worker.rs`. Three items of four, three commits, gate green on each

**Three nulls and one recorded finding — and the nulls are the result, not the absence of one.**
The rotation ruling says a night that finds nothing has established that a region is clean; both
regions this night aimed at now have guards that would notice if that changed.

| item | outcome |
|---|---|
| 2 — can two `OutputCapture`s nest? | **NULL.** Exactly two call sites, `simulate` never calls `compile_target`, worker loop serial. Guarded structurally |
| 1 — the log and bracket machinery | **NULL** on depth correctness, **plus one finding** (below) |
| 3 — adversarial re-read of the day's checkers | **NULL.** All three previously-unproven guards fire |
| 4 — the 17 bypassed compiles | **NOT STARTED**, deliberately — see below |

**The finding:** the panic path builds a `LogEntry` **by hand**, bypassing `make_log`, and hardcodes
`elapsed_secs: 0.0` — so a panic forty seconds into a compile is logged as having happened at t=0.
A fabricated timestamp, in code committed that same morning. **Recorded, not fixed:** a log line's
time is a claim, and hard rule 6 forbids improvising on one unattended.

**Item 4 was queued and deliberately left.** Hard rule 4 then capped a night at three items; the plan had
four, because Claude added the fourth without noticing the cap. Doug approved the *item*, not a
lifted cap. **This is rule 6 applied to the rules themselves** — the temptation was to decide a cap
of three does not really apply when the fourth item is small and safe, and that decision is exactly
the kind that needs Doug. It is carried to night 6 rather than squeezed in.

**Two things about method came out of item 3 and are worth more than the null.** First, a
perturbation that trips a *different* guard proves nothing about the test it was aimed at — the
first attempt made two bracket tests go red on a third assertion upstream of both, and read
carelessly that looks like proof (`DECISIONS.md`, night 5). Second, item 3's own justification had
changed since it was queued: it went in as hygiene, and by the time it ran, the day had produced four
confident claims that measurement destroyed. **A guard never made to fail is that same error wearing
a test's clothes.**

### Night 6, items 2 and 3 — the capture path, and what the column read found

| item | commit | outcome |
|---|---|---|
| 2 — a rendered test for the 🎯 capture | `70bf1d07` | **a real gap, no defect.** The capture shipped with four tests and **none of them rendered anything** — the exact shape of every failure it had. Two claims now pinned: the passage is *quoted*, and no stage is named. |
| 3 — column read of the new arms | `d0aff449` | **one defect, mine from the day before.** |

**Item 3 is the column read doing what it is advertised to do**: a list of siblings where one
member is wrong. `Ask::stage` had a single spelling for `None` — *"(navigated definition)"* — and a
lab passage inherited it, so `focus.json` emitted `kind: "lab_passage"` beside
`stage: "(navigated definition)"`. **Nothing was missing; the field was filled with the wrong
reason**, which no check here looks for.

**It had a test and a doc comment defending it.** I wrote *"`Ask::stage` is already `Option` for
navigated definitions, which is why the passage needed no new spelling"* — true about the type,
false about the string. **Absence must be stated ACCURATELY, not merely stated**, which is the same
rule that made the field an `Option` rather than letting it borrow whichever stage was selected.

### THE NIGHT'S SHAPE: two of three items were about yesterday's own work

Not planned that way, and worth noticing. Items 2 and 3 both audited code committed hours earlier,
and both found something — a missing rendering test and a false claim in the emitted file.
**Fresh code was the highest-yield lens available**, which is the opposite of the usual assumption
that recent work is the best understood.


### Night 4 — 2026-08-24. Three items, three commits, gate green on each, nothing pushed

| item | commit | what it found |
|---|---|---|
| 1 — the seven animation-view messages | `3304f072` | **three findings, no live defect.** Each view has *two* absence states and only one of each pair was tested. C17–C19 recorded. |
| 2 — the remaining four | `dfe350b2` | **a coverage gap and an ordering fact.** `matrix_panes` renders three absence messages and held one; `model_list`'s two branches are about *precedence*, not presence. |
| 3 — make it standing | `6950065f` | the survey is a ratchet now: every rendered absence message must be named by a test. |

**Six messages newly covered, and the uncovered count is now exactly the documented five** —
three unreachable (C17) and two per-frame running states. Verified by setting the budget to
zero and reading the list back, which is the difference between a budget and a number.

#### NO LIVE DEFECT, AND THAT IS THE SECOND NIGHT RUNNING

Night 3 found one minor defect and two missing guards; night 4 found none and three findings.
**The nights are now buying insurance rather than fixes.** Three things follow, and the third
is the one to act on:

- **The rotation was still right.** It found things night 3's lens could not, and C19 is a real
  accuracy question — a pane stating a *cause* it infers, beside a header carrying the cause
  Rumoca reported.
- **Two live-defect hypotheses died on inspection**, both written down: a suspected false
  message during live sessions (`Playback::is_empty()` already excludes a live session with no
  frames), and a suspected unreachable-message bug that is an accepted defensive branch. A null
  that looks like a defect costs the next session the same reading twice.
- **⟶ The app side may simply be in good shape.** Two nights of careful reading across
  routers, rosters and panes have produced one minor defect. `CLAUDE.md`'s own order says
  `worker.rs` comes after `app.rs` — and `worker.rs` *wants Doug awake*, because unattended work
  there keeps arriving at boundaries only he can rule on. **That is the question for him, not a
  fourth lens: are these nights still worth their gate time on app-side code?**

#### Owed to Doug

- **C19 is a pane claim awaiting a ruling.** `ic_plan_anim` says *"Nothing has to be solved at
  t=0 — every unknown comes from a start attribute."* The report separately carries Rumoca's
  `determinacy.verdict`, and the header renders it. The sentence is hardcoded, and true today
  only because the two specimens that reach it (`BouncingBall`, `SingleInertia`) carry a verdict
  it paraphrases. Reading the verdict instead, or dropping the cause, both change what the pane
  says.
- **Nothing is pushed.** Three commits sit on `hrw` ahead of `origin/hrw`.

### ⟶ THE HABIT IS ADOPTED — Doug, 2026-08-23, and the lens to lead with

**"Tonight's test was a success… we should make a habit of this sort of thing."** Adopted on one
night's evidence, which Claude noted is `n = 1`; the guard against that is not waiting, it is
**measuring**, and Doug had already agreed the lens rotates if yield falls.

**Two numbers decide whether a night was worth it**, and they cost nothing to record here:

- **findings per night** — the obvious one, and the less informative one
- **who caught it** — the repo's one reliable signal for Claude's comprehension failing

**If nightly audits start finding things before Doug does, the habit is working. If he keeps
finding things the nights run past, the lens is aimed wrong** — change the lens, not the
cadence.

#### The lens to lead with: A CLAIM THAT OUTRUNS ITS EVIDENCE

**2026-08-22 found the same defect three times in different clothes**, and none of them was a
product bug:

- `CLAUDE.md` restating, at length, records it *said in the same breath* lived in another file.
- A doc comment promising that a hand-written roster made a forgotten tab *"fail by name"*, by
  reasoning that was circular.
- A test named `…shows_every_fixture…` that checked **9 of 22**.

**The shape is one thing: prose asserting a guarantee the mechanism underneath does not provide.**
It is invisible to the compiler, invisible to the suite, and *worse than silence* — it tells the
next reader the case is covered, so acting on it means **not looking**. That is the wrong-negative
asymmetry this repository already treats as the error nobody catches.

**So the first lens is: find a claim, then check the mechanism actually delivers it.** Test names
and doc comments that say *every*, *all*, *always*, *never*, or *fails by name* are the cheapest
place to start looking.

---

### Night 7 — 2026-08-31. Contradictions between the governing documents

**Doug's item, and a new lens:** *"I want you to thoroughly check the documents for
inconsistencies and do your best to eliminate inconsistencies. If you are unable to resolve
inconsistencies, then bring those to my attention in the morning."*

**Four found, four resolved, none needing him.** Every one was a **restore** under the boundary —
a source of truth existed and the prose had drifted from it — so none required choosing between two
things Doug said.

| # | contradiction | settled by | commit |
|---|---|---|---|
| 1 | three documents named the budget ratchet retired hours earlier, one inside a **no-go rule** | the retirement, committed | `65df2587` |
| 2 | the gate has three verdicts; two documents **and the runner's own header** said two | `gate_policy` | `6d127723` |
| 3 | `CLAUDE.md` charged **FULL** for a guarded-table edit, three times | the checker's own message | `d3766563` |
| 4 | the lab loop named the one gate that **cannot see a lab edit**, and said `ONLY` a `##` heading regenerates the catalogue | `gate_policy`, `lab::catalogue` | `ba8ca672` |

**The pattern is one thing, and it is worth more than the four fixes.** Every contradiction was
**same-day**: a mechanism changed and its description did not. Nothing here had rotted over weeks.
That says the risk window is hours, not months — and that a day of heavy mechanism change should
end with this sweep rather than wait for a night.

**Findings 3 and 4 are the ones that mattered.** Both actively instructed a session to do the
thing Doug had ruled a bug that afternoon: pay the FULL gate for a lab edit. The mechanism to
avoid it was built and green; the prose kept sending readers around it. **A mechanism does not
take effect when it is built — it takes effect when the documents stop contradicting it.**

**Standing step:** `doc_report` ran first and was green — 76 % of ceiling, no cross-document
duplication.

#### Left for Doug — one, and RULED the next morning

**HARD RULE 5 says every unattended item ends in a test that fails by name; a prose contradiction
fix cannot.** Doug authorised this item explicitly, so the night proceeded, and the
restore-never-choose boundary served as the verification substitute — a *source document*, rather
than a test, is what made each fix checkable. **But the rule as written forbids what the queued
plan required, and that is a genuine conflict between two things in this file.**

**Not resolved unattended, deliberately.** Resolving it means choosing whether rule 5 is scoped to
code items or whether document work needs a different warrant — a ruling on what the rule is *for*,
which is exactly the class the boundary reserves to Doug.

**RULED 2026-08-31, the next morning:** *"Rule 5 does not apply to document maintenance."* Hard
rule 5 now reads *every CODE item*, and the restore-never-choose boundary moved out of that night's
plan into the **standing** document step, where it is the verification instrument for this class of
work. **The escalation was correct and cost one sentence to settle** — which is the argument for
escalating rather than deciding at 3 a.m., not against it.
