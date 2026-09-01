# Unattended runs — working while Doug is asleep

**Purpose:** the rules that bind a session running with nobody to ask.
**Status:** authority for unattended work. **Read this before starting any.**
**Read when:** a session begins while Doug is asleep, or `/loop` is running overnight.

Doug, 2026-08-22: *"could you do that work while I am asleep?"* Yes — under these rules, and
**only for the classes of work named below.**

## Why the rules are tighter than a normal session's

`CLAUDE.md` records that **the one reliable signal for Claude's comprehension failures is
"defects only a human caught"** — and it *"weakens exactly when Doug is less available."*
Overnight is the extreme case: that signal is **absent**, not weak.

So the governing principle is not "be careful." It is: **do only work whose success is verifiable
without him**, and treat everything else as out of scope no matter how valuable it looks. The
failure mode to engineer for is *"less got done"*, never *"something bad landed."*

## THE QUEUED RUN — single slot, REPLACE it, never append

**This section holds one run's plan and nothing else.** It exists because the conversation that
planned the run does not survive to the machine that executes it. When a run finishes, its record
goes to the run log below and this section is overwritten by the next plan — otherwise it becomes
the accumulating history `CLAUDE.md`'s *Current work* had to be rescued from.

### QUEUED — night 7: eliminate contradictions between the governing documents

**Doug, 2026-08-31:** *"I want you to thoroughly check the documents for inconsistencies and do
your best to eliminate inconsistencies. If you are unable to resolve inconsistencies, then bring
those to my attention in the morning."*

**A NEW LENS, and the old one is formally retired.** Every previous night read *code*. This reads
*documents* — the wall the ceilings cannot see, named the same day: two documents stating one rule
differently, so a session cannot tell which binds and silently picks. Test-suite timing is spent
(`ideas.md` #48 killed six levers, night 6 the seventh) and is not to be queued again.

**It is not speculative.** The lens found one before it was queued: *"token cost is not a
constraint — never trade richness for economy"* in `CLAUDE.md`, against *"answer the question
asked, at the depth asked, and stop"* in `working-with-doug.md` and *"thoroughness had been treated
as free and is not"* **400 lines away in `CLAUDE.md` itself.** All three on the mandatory reading
path. Doug's own words were correctly scoped to *context captures*; two rounds of summarising
dropped the scope.

#### THE BOUNDARY — restore, never choose

**This is the whole safety of the night, and it is one question: is there a source of truth to
restore to, or must I pick?**

| resolve it alone | bring it to Doug |
|---|---|
| a digest broadened or narrowed its source — **restore the source's scope** | two deliberate statements that genuinely differ |
| Doug's verbatim words are recorded and a gloss overreaches them | resolving it needs a ruling on what he meant |
| a document claims another says X; that one no longer does | the fix changes what a rule *permits* |
| a citation names a renamed or deleted symbol, file or test | anything requiring an explanation to be trimmed |

**Restoring a scope from Doug's own quoted sentence is a correction, not a judgement** — that is
what made the token-cost fix safe to make unattended. **Choosing between two things he said is a
judgement and is his**, however obvious the answer looks at 3 a.m.

#### METHOD — four passes, highest yield first

1. **Digest audit.** Every place a reading-path document summarises another, compared against the
   source. This is where the known instance came from, and `CLAUDE.md` is a digest of
   `working-with-doug.md`, `CHARTER.md` and `docs/README.md` throughout.
2. **Quote provenance.** Every quoted Doug sentence: does the gloss around it extend the claim?
   Does the same quote appear elsewhere with a different gloss?
3. **Cross-reference claims.** Every *"X says Y"* — check that X says Y. Cheap and mechanical.
4. **Rule pairs by subject.** Group the bolded imperatives by topic and compare within each group.
   Slowest, and the one most likely to surface a genuine conflict for Doug.

**Scope: the documents that GOVERN behaviour** — `CLAUDE.md`, `docs/working-with-doug.md`,
`docs/CHARTER.md`, `docs/README.md`, `docs/fixture-tours/README.md`, `docs/vision.md`,
`docs/unattended-runs.md`, `DECISIONS.md`. **Not `ideas.md`** (6,349 lines of speculation) and
**not `compiler-phases/`**: a stale idea misleads nobody the way a contradictory rule does.

**Output.** One commit per resolved contradiction, each naming both sides and which source settled
it. Everything unresolved goes in the run log below **and** the handoff, stated as a pair rather
than a recommendation — the morning is for Doug to rule, not to ratify.

**Run `cargo run -p hrw --example doc_report` first**, as every night now does.

### ⟶ RULED, 2026-08-25 — THE NIGHTS CONTINUE, AND ROTATION IS THE CONDITION

**Doug:** *"The unattended runs are very much worth their keep, so long as we remain willing to
rotate the lens."*

**That answers the question this slot had been holding open**, and it answers it against the
arithmetic. Two nights of careful reading produced one minor defect between them and night 4
produced none, so the case *"they are buying insurance rather than fixes"* was correct as far as
it went — **and the ruling is that insurance is worth its price.** A night that finds nothing has
established that a region is clean, which is a result; the failure mode is not a quiet night but
**a night pointed at a region already established as clean.**

**So the condition is operative, not decorative: a lens that has come up empty twice is spent, and
the next night must aim somewhere else.** Nights 3 and 4 already demonstrate the rotation working
— `app.rs`'s routers came back with four nulls out of seven threads, which is what moved the lens
to the panes. **Do not queue a fifth night on a lens that has already returned nothing**, and say
in the queued plan which lens is being retired and why.

**`CLAUDE.md`'s order still says `worker.rs` is next, and that wants Doug awake** — so a night is
the right vehicle for app-side reading, not for the compile path.


### The command

```text
/loop Unattended run under hrw/docs/unattended-runs.md — read it first, every iteration, including THE QUEUED RUN section which carries tonight's items and their evidence. Full gate green before every commit, commit but never push, never leave the tree dirty, revert-and-record on hitting a no-go, append who-caught-it ledger rows for anything found, then write the handoff and end the loop.
```

## Preconditions — Doug's, before starting

| | why |
|---|---|
| **HRW is closed** | with it running, `cargo test … --test msl_resolve` fails `Access is denied` on `hrw.exe`, and after a `clippy --all-targets` that is **permanent, not transient**. Half the gate cannot run, so every commit would rest on partial evidence. |
| **Sleep is disabled** | a gate run once took **10,780 s** because the machine slept mid-run. Attended, that is a puzzle; unattended, it is indistinguishable from a hang. |
| **The tree is clean and pushed** | so recovery is `git reset --hard origin/hrw` and nothing unpushed can be lost. |

## Hard rules — Claude's, during

1. **The gate is green before every commit**, in one command — **use the runner, not the
   sequence** (added 2026-08-23, after the fmt → generate → lint → test order was got wrong for
   the tenth time):
   ```text
   cargo run -p hrw --example gate
   ```
   It picks FAST, TOUR or FULL from the working tree, runs all four generators, adds `fmt` **and**
   `clippy` for any `crates/rumoca-*` package touched, stops at the first failure naming what that
   step protects, and refuses to start while HRW holds `hrw.exe`.
2. **Never leave the tree dirty.** Every item ends committed or reverted. Nothing half-done.
3. **Commit, do not push.** Nothing outward-facing happens with nobody awake.
4. **A hard cap of FOUR items, then stop** — raised from three by Doug on 2026-08-26. Sprawl is
   still the named failure mode, not idleness: *"three finished things with tests beat eight
   half-done ones, because Claude is bad at telling what already depends on a behaviour."* **The
   quoted sentence is about FINISHED versus HALF-DONE, not about the number three**, so raising the
   cap does not contradict it — and nothing here licenses a fifth item, a half-done fourth, or
   skipping a gate to fit one in. **If four will not fit, stop at three.**

   **What prompted it:** night 5 was queued with four items and ran three, leaving the fourth
   unstarted because the cap bound. That was the right call under the rule as written — Doug had
   approved the item, not a lifted cap — and he lifted the cap rather than have items carried
   forward. **The precedent worth keeping is the refusal, not the raise.**
5. **Every item ends in a test that fails by name.** If an item cannot, **it is not unattended
   work** — record it and move on.
6. **On hitting a no-go mid-item: revert and record. Do not improvise.** The temptation is to
   decide the boundary does not really apply here. That decision is exactly what needs Doug.
7. **Write the handoff before stopping** — what landed, what was found, what was declined and why.

## The no-go list

Absolute, regardless of how safe it looks at 3 a.m.:

- **`worker.rs`'s compile path.** Extract *around* it; never restructure it. `compile_target` is
  1,085 lines from line 2049 and is prohibited ground.
- **How the MSL session is loaded, cached or shared.** **The ruling was made and is standing**, not
  pending: Doug declined lever B — compiling MSL-free specimens in a bare session — on 2026-08-21,
  because the compiles are identical *except for DefId numbering*, and DefIds are observable in the
  pane and in every committed trace. So ~49 s of a ~290 s gate would have been bought by changing
  what HRW displays. Any **further** change in this class comes to him first; it is not an open
  question awaiting an answer.
- **Anything that changes what a pane *claims*.** Accuracy outranks everything, and a pane's claim
  is the one thing no checker here verifies for meaning.
- **Anything trading fidelity for anything else.**
- **Tour prose.** Doug's tour walks are his primary learning exercise; rewriting an explanation
  unsupervised is not Claude's to do. Fixing a checker-caught number or a dead link is fine.
- **Raising any ceiling or budget** — the reading-path ceilings (`docs/reading-budgets.txt`), the
  field-count ratchet, the orphaned-doc budget. Raised with nobody reading the reasoning, it is a
  limit with no check at all. **A ceiling crossing reports and waits**; it is never a licence to
  raise the ceiling, which is the rule the standing document step below states in full.
  *(Reworded 2026-08-31: this said "the mandatory-path ratchet", a mechanism retired that same
  day. The rule was always right and only its example had rotted.)*
- **`docs/upstream-issues.md` P1**, and anything else awaiting a Doug protocol.
- **Pushing.**

## DOCUMENT MAINTENANCE RUNS EVERY NIGHT — it is not an item, it is a standing step

### Read this before the sweep, because it decides every judgement call in it

**Doug, 2026-08-31**, naming the category: *"I believe that performing document maintenance every
night is our first example of regular project health work which has nothing to do with code
health. … maintaining those documents in good working order is essential to keeping this HRW
project healthy. When you are performing nightly document maintenance you should always consider
that you are doing so to prevent hitting a document wall so that you can keep this project moving
forward."*

**So the sweep is not tidiness, and its goal is not smaller numbers.** It exists so that a future
session can still read, trust and act on these documents. Every decision in it answers one
question: *does the project still move if this stays?*

**That distinction is load-bearing, because the two goals disagree.** A sweep optimising the
metric deletes a hard-won rule — the number falls and the project gets weaker, since the rule was
bought with a defect. A sweep optimising for movement deletes a **restated** rule, a closed arc, a
stale claim, a dead link, a duplicated passage — the number falls and nothing is lost. **Never ask
"is this document too big". Ask "is anything here no longer earning its place".**

### What a document wall actually is — and size measures only half of it

Two ways the project stops moving, and the nightly report sees one of them:

- **Volume.** A session spends its context reading before it can act. This is what the ceilings
  watch, and today it sits at **75 %** of a derived limit.
- **Contradiction.** Two documents state the same rule differently, so a session cannot tell which
  binds — and picks one, silently. **Nothing measures this**, and it is the nearer wall.

**2026-08-31 produced three contradictions in one day**, every one caught by conversation rather
than by a check: `fixture-tours/README.md` said `node` earned its place while the tour it governs
had dropped the abstraction; a checker was named for vocabulary its subject no longer used; and a
pinned assertion quoted a sentence the tour had deleted.

**`doc_report`'s duplication check finds COPIES, not contradictions** — an exact repeated passage,
which is the benign case. **A passage that was copied and then edited in one place only is the
dangerous case and is invisible to it.** Say so in the run log rather than reporting a clean sweep
as if it covered both; closing that gap is unsolved and wants Doug.

**Doug, 2026-08-31**, retiring the per-commit budget ratchet: *"rather than fighting budget
battles several times during a workday, perhaps we can do document clean-up every night during
that night's unattended run. And, if during a document cleanup you determine that we're about to
hit a document wall, then we can pause and work together to trim documents."*

**So it runs on every unattended night, before the night's actual item, and it is one command:**

```text
cargo run -p hrw --example doc_report
```

It prints every reading path's size against its **ceiling** (`docs/reading-budgets.txt`, derived —
the mandatory path is capped at a quarter of a 200k context) and lists **passages of 400+
characters appearing in more than one document**. Exit **0** means nothing is needed; **1** means
the morning starts here.

**Why duplication and not just size.** The growth that caused the whole budget problem was never a
document getting long — it was **the same prose in two files**, four rulings written into both
`DECISIONS.md` and `fixture-tours/README.md` on one day. A size check sees that as "a document
grew" and bills for it; this sees the thing itself.

### What the night may do, and what waits for Doug

`CLAUDE.md` already authorises Claude to reorganise and condense documents without asking. **The
line is the same one that governs tour prose: an explanation is Doug's learning material, so
trimming one is his call.**

| do it unattended | leave it for Doug |
|---|---|
| retire closed history to `DECISIONS.md` | trim or reword an explanation |
| delete a duplicated passage, keeping one | restructure a README |
| fix a stale claim or a dead link | anything that crosses a ceiling |

**`doc_report` never edits**, deliberately. A sweep that both decides and acts, unwatched, on
documents whose value is judgement is exactly what Doug reserved to himself. It reports; the night
acts within the left column; a ceiling crossing goes in the run log and waits.

**A ceiling crossing is not a licence to raise the ceiling.** That was the ratchet, and it charged
fifteen tolls in one day while rejecting nothing.

## What IS good unattended work

Chosen because the `app.rs` arc measured it, not because it sounds safe:

- **Column-read audits.** Reading a list of siblings as a column and finding the odd member found
  **four of the eight** defects in that arc, and **needs no extraction at all** — zero blast
  radius, proven yield.
- **Extractions that buy a test which could not have been written before.** The licensed kind, with
  a mechanical success criterion rather than a judgement call.
- **Bug fixes that arrive with a test failing by name** — explicitly Claude's to decide.

**`app.rs` before `worker.rs`.** `worker.rs` is 11,088 lines — larger than `app.rs` was when its
trigger fired — and most of it is compile path or MSL-session handling, so unattended work there
would keep arriving at the boundary where Doug must rule. **`worker.rs` wants him awake.**

## The first run is an experiment, and it has a measurable outcome

**One night, `app.rs` only, three items.** Then Doug judges one question: **did anything land that
he would have rejected?**

- **Nothing rejected →** extend the scope.
- **Something rejected →** we have learned exactly which class of work Claude cannot be trusted
  with unsupervised, **which is worth more than the refactoring.**

**There is no prior data.** Claude's unattended failure rate is unmeasured, so the cap and the
no-go list are deliberately tighter than they may need to be. Loosen them on evidence, the way
every other limit in this repository has been.

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
| 3 — same pattern elsewhere | `1358e89d` | the tour-picker test named **9 of 22** fixtures while its name claimed every one. Now derived from `fixture_tours()`. |

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
tour passage inherited it, so `focus.json` emitted `kind: "tour_passage"` beside
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
finding things the nights walked past, the lens is aimed wrong** — change the lens, not the
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
