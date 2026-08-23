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
- **Tour prose.** Doug's phase-2 walks are his primary learning exercise; rewriting an explanation
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

### ⟶ ONE BOUNDARY CALL FOR DOUG TO RULE ON

**The plan said "app.rs only". Items 2 and 3 landed in `stage_tabs.rs` and `ui_tests.rs`.**

Claude judged these in scope — both are app-side UI modules split out of `app.rs`, and item 2's
agreed text (*"every per-stage system"*) spans twelve files by construction, so a literal reading
would have made the item impossible. **Neither is on the no-go list.** But it is a widening of a
stated scope, decided with nobody to ask, and that is exactly the class this protocol exists to
surface rather than let pass unnoticed.

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
