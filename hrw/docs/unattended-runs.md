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

### Queued 2026-08-23 for night 3 — `app.rs`, and the goal is BUGS

**Same machine, same preconditions.** Run `cargo run -p hrw --example check_machine` first; it
costs about a second and answers what a `git pull` does not bring.

**Why these three.** [`app-split-plan.md`](app-split-plan.md) is **closed** — extraction landed
2026-08-21 — so this is bug discovery, not refactoring, and `CLAUDE.md`'s rule picks seams **by
where defects are likely**: code never closely read, code that cannot be tested, clusters of
siblings where one member may differ.

`app.rs` holds three **routers**, each a match over a variant set, each 200+ lines. **Neither of
the big two has a wildcard arm**, so coverage is already compiler-enforced — *counting arms is not
the exercise*. The exercise is the recorded one: **a router's seam is an asymmetry among its
arms**, the lens that found four of the eight defects in the `app.rs` arc.

**1 — Column-read `drain_worker`'s six arms.** `app.rs:1780`, 215 lines. Every worker message
becomes UI state here: `Libraries`, `Log`, `CompileProgress`, `Compiled`, `DefTree`, `Simulated`.
Per arm, ask what its siblings do that it does not — invalidate a cache, request a repaint, clear
the `compiling` flag, feed the stage-diff highlight. **This class is live here**: the 2026-08-20
cache-lifetime finding and the stranded sub-view both landed in this function's blast radius.

**2 — Column-read `dispatch_hrw_link`'s twelve arms.** `app.rs:2568`, 244 lines. **Night 1 read
`HrwLink`'s `parse` and `describe` and found them sound; `dispatch` is the third member of that
family and has never been read.** Two questions, each with a precedent: does every arm guard its
target's *availability* — a link once selected a view that had no tab, which
`structural_view_available` exists to prevent — and does every arm clear pending state uniformly?
The tour-link hooks were once never cleared, so the first click masked every later one.

**3 — The `has_*` availability family.** Three predicates gating tabs, reading three different
sources: `has_alias_eliminations` (`app.rs:3952`) reads **the current stage**, `has_ic_plan`
(`app.rs:3983`) reads the **named** initialization stage, `has_pre_lowering_trace` (`app.rs:3998`)
reads **frames**.

**The obvious risk is already guarded, and that is what makes this worth reading rather than
fixing:** `StructuralView::AliasAnim => is_index_reduction && self.has_alias_eliminations()`, so
the predicate is only correct *because a caller adds the stage test*. `structural.json` carries no
`reduction` key at all, so the predicate alone is false on the Structural stage. **The live
question is whether every caller adds that test.** `app.rs:4349` compares
`viewport.structural == AliasAnim` directly and is the candidate odd caller.

**Free parallel read, consuming none of the item budget:** `central_panel_ui` — 321 lines from
`app.rs:4127`, the largest router and never closely read.

**Precedence:** a defect found in items 1 or 2 wins over starting the next item. Three is a cap,
not a quota.

### The command

```text
/loop Unattended run under hrw/docs/unattended-runs.md — read it first, every iteration, including THE QUEUED RUN section which carries tonight's three items and their evidence. app-side only. Full gate green before every commit, commit but never push, never leave the tree dirty, revert-and-record on hitting a no-go, append who-caught-it ledger rows for anything found, then write the handoff and end the loop.
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
   It picks FAST or FULL from the working tree, runs all four generators, adds `fmt` **and**
   `clippy` for any `crates/rumoca-*` package touched, stops at the first failure naming what that
   step protects, and refuses to start while HRW holds `hrw.exe`.
2. **Never leave the tree dirty.** Every item ends committed or reverted. Nothing half-done.
3. **Commit, do not push.** Nothing outward-facing happens with nobody awake.
4. **A hard cap of three items, then stop.** Sprawl is the named failure mode, not idleness —
   *"three finished things with tests beat eight half-done ones, because Claude is bad at telling
   what already depends on a behaviour."*
5. **Every item ends in a test that fails by name.** If an item cannot, **it is not unattended
   work** — record it and move on.
6. **On hitting a no-go mid-item: revert and record. Do not improvise.** The temptation is to
   decide the boundary does not really apply here. That decision is exactly what needs Doug.
7. **Write the handoff before stopping** — what landed, what was found, what was declined and why.

## The no-go list

Absolute, regardless of how safe it looks at 3 a.m.:

- **`worker.rs`'s compile path.** Extract *around* it; never restructure it. `compile_target` is
  1,085 lines from line 2049 and is prohibited ground.
- **How the MSL session is loaded, cached or shared** — Doug's ruling, on fidelity grounds.
- **Anything that changes what a pane *claims*.** Accuracy outranks everything, and a pane's claim
  is the one thing no checker here verifies for meaning.
- **Anything trading fidelity for anything else.**
- **Tour prose.** Doug's tour walks are his primary learning exercise; rewriting an explanation
  unsupervised is not Claude's to do. Fixing a checker-caught number or a dead link is fine.
- **Raising any budget** — the mandatory-path ratchet, the field-count ratchet, the orphaned-doc
  budget. A budget raised with nobody reading the reasoning is a budget with no check at all.
- **`docs/upstream-issues.md` P1**, and anything else awaiting a Doug protocol.
- **Pushing.**

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
