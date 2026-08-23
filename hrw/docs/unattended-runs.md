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

### Queued 2026-08-23 for that night, on THIS machine — retargeted the same day

**Doug ruled on 2026-08-23 that the run executes on the machine this plan was written on**, not
on the other one it was originally queued for. **That removes the machine handoff, which carried
the run's only silent-stall risk** — a missing permission allowlist prompts on the first Bash
call, and asleep, a prompt is indistinguishable from a hang.

**The machine check was run the same day and passed all five checks**, so the three things a
handoff loses are all discharged: the allowlist is present, `hrw.exe` is unlocked, and **the
parsed-artifact cache is warm — do NOT expect the slow first gate a fresh machine would pay.**

**Run it once more before starting anyway.** It costs about a second, it is what the handoff
table used to be, and the tree may have moved since:

```text
cargo run -p hrw --example check_machine
```

Plus the standing preconditions below: **HRW closed, sleep disabled, tree clean and pushed.**
Doug disabled sleep on 2026-08-23.

### The three items

**1 — Generalise the no-nested-scroll rule, then fix the two defects it exposes.**

The rule (`CLAUDE.md`, 2026-08-16) is *"never nest a vertical scroll area inside one; the parent
owns the scrolling and the height, and a child view just renders."* It is enforced for **one** file
by `connection_anim::tests_layout` and was never generalised. Verified 2026-08-23:

| file | parent wrapper | the view itself |
|---|---|---|
| `alias_anim` | `app.rs:3940` `ScrollArea::vertical()` | **`alias_anim.rs:182`** `ScrollArea::vertical()` … `.max_height(320.0)` |
| `ic_plan_anim` | `app.rs:3973` `ScrollArea::vertical()` | **`ic_plan_anim.rs:303`** `ScrollArea::vertical()` … `.max_height(300.0)` |

**It manifests.** The 320pt cap holds ~16–18 grid rows; alias eliminations per specimen are
**`Drivetrain` 77**, `MotorWithBrake` and `BenchActuator` 41, `GearWithBrake` 33, `RcCircuit` and
`OverInitRc` 20. `Drivetrain` — the index-reduction tour's centrepiece — shows under a quarter of
its list inside a small box while the pane around it has room. `ic_plan_anim`'s 300pt cap is
overflowed by `RcCircuit` and `OverInitRc` at 21 blocks.

**The fix is subtractive**: delete the inner `ScrollArea` and its `max_height`, exactly as
`connection_anim` was fixed. **The must-fire proof is free** — write the checker first and it fails
on two real defects; no synthetic break is needed.

**2 — Column-read the `Animated` trait: 8 implementors × 5 methods.**

`playback.rs:68`. Implementors: `alias`, `connection`, `ic_plan`, `matching`, `pre_lowering`,
`reduction`, `tarjan`, `tearing`. Look for the member shaped differently — the lens that found the
stranded stage tab and the Flatten sub-view stranding. **Worth one check regardless of what turns
up: `which()` values must be unique**, since they are matched against and a collision misroutes
silently.

**3 — Write the Pantelides acceptance test against the committed oracle data.**

**Needs no System Modeler** — the perishable half was captured 2026-08-23 on the machine that has
it, and lives in
[`specimen-notebook/CartesianPendulum/oracle/`](specimen-notebook/CartesianPendulum/oracle/).
**Read that directory's README first**, particularly the tolerance trap: do not pin System
Modeler's numerical choices as truth.

Write it against **today's API** — compile `CartesianPendulum` through the existing path and assert
what a correct reduction produces — so it compiles now and the compiler keeps it honest for months.
It **fails today by design**: `#[ignore]` it with the reason naming `docs/ideas.md` #83, so the day
Pantelides lands it is a red test turning green rather than a project needing a plan.

Strongest assertions first, because they cannot be wrong: **state count 2**, **`lambda` peaks at
*m*(*g* + *v*²/*L*) = 29.43 at the bottom**, **the constraint holds**. The 101 trajectory samples
are the differential half, at a stated relative tolerance per charter §4.3.

**⟶ WRITE IT AS A LADDER, NOT ONE RED TEST** *(Doug, 2026-08-23, from the textbook comparison)*.
A textbook's end-of-chapter project is **scoped and graded** — each step completable in a sitting,
each giving its own sense of progress. A single binary test says when you are *done*; it never says
whether you are *on track*, and that is the one thing textbook projects do better than this one.

So aim for roughly five tests, each able to turn green on its own:

1. the system is **detected as high-index** at all
2. **one constraint differentiates** correctly
3. the iteration **reaches a fixed point**
4. the pendulum **reduces to two states**
5. its **trajectory matches the oracle** within tolerance

Write whichever of them are expressible against today's API; where a rung needs an entry point
that does not exist yet, say so in the test's `ignore` reason rather than inventing one. **Same
destination, but the climber knows where they are.**

**Precedence:** if items 1 or 2 surface a defect that needs fixing, that wins and this moves to the
next run. The three-item cap is not a quota to fill.

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

1. **The full gate is green before every commit**, in one command (HRW is closed, so this works):
   ```text
   cargo test -p hrw --lib --test msl_resolve --features slow-tests -- --test-threads=1
   ```
   preceded by the usual `cargo fmt` → generators → `cargo clippy -p hrw --all-targets`.
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
