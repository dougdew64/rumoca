# Unattended runs — working while Doug is asleep

**Purpose:** the rules that bind a session running with nobody to ask.
**Status:** authority for unattended work. **Read this before starting any.**
**Read when:** a session begins while Doug is asleep, or `/loop` is running overnight.

Doug, 2026-08-22: *"could you do that work while I am asleep?"* Yes — under these rules, and
**only for the classes of work named below.**

---

## THE CONTRACT — what Claude must SAY when proposing a night

**Doug does not read this file, and was never meant to.** *(Corrected 2026-08-31, an hour after a
section was added inviting him to: **"I hadn't even known that I was supposed to be reading a
document before unattended runs were begun. Instead, I had been depending upon our conversation
here."**)*

**So this section is not a page for him — it is Claude's script.** When a night is proposed, say
this in the conversation. A document he does not read cannot inform consent, and pointing at one
is not the same as telling him.

### DOUG'S SAFETY MODEL, IN HIS WORDS — and it is simpler than this file

> *"For me, the primary safety mechanism for unattended runs is that you don't push unattended."*

**That reorders everything below.** The rules in this file are mostly about **quality** — is the
work good, is it finished, is it verifiable. Only a few are about **safety**, and safety here means
one thing: **nothing that happens overnight is irreversible.** Commit-never-push delivers that
almost entirely, because anything committed and unpushed can be undone with one command and has
reached nobody.

**So the load-bearing rule is "do not push", and it must never bend for any reason.** A bad commit
is a bad morning; a push is a fact about the world. **And the same weight applies to anything else
that leaves the machine or cannot be undone** — filing an issue, posting anywhere, deleting
something git does not hold.

**THE ONE HOLE IN THAT MODEL, named the day it was stated: GITIGNORED STATE IS NOT PROTECTED.**
`.hrw-bridge/` — `lab.md`, `focus.json`, `view.json`, scratch specimens and notebooks — is
outside git, so commit-never-push does **not** make an overnight change to it reversible.
`lab.md` in particular is Claude's answer to Doug's last question, and deleting it would destroy
something no `git reset` restores. **Treat `.hrw-bridge/` as read-only overnight**, except where a
test already restores what it touched (`ui_tests::AdHocLab`).

### What Claude will and will not do

**When Doug says "run tonight", this is what he is agreeing to.**

**Claude will:**

- run the **document sweep first**, then at most **four items**
- finish or revert each one — **never leave anything half-done, never leave the tree dirty**
- get the **gate green before every commit**
- **commit, and never push.** Nothing leaves the machine while you are asleep.

**Claude will not** — regardless of how safe it looks at 3 a.m.:

- touch `worker.rs`'s **compile path**, or how the **MSL** is loaded, cached or shared
- change **what any pane claims**, or trade fidelity for anything
- **rewrite lab prose.** Fixing a checker-caught number or a dead link is fine.
- **raise any ceiling or budget**
- **push**

**What you get in the morning:** unpushed commits, a run-log entry, and a handoff in `CLAUDE.md`.
**Anything Claude would not decide alone is stated as a pair, not a recommendation** — the morning
is for you to rule, not to ratify.

**Before you start one**, three preconditions, all checked by one command:

```text
cargo run -p hrw --example check_machine
```

HRW closed, sleep disabled, tree clean and pushed. It exits non-zero and names the fix.

**This section is the authority; the detail below derives from it.** A rule further down that
contradicts it is a bug in the detail, not a competing rule.

**And the obligation it creates is on Claude, not on Doug: SAY IT, do not link it.** He consents in
the conversation, so the conversation is where the terms have to appear. If a night would do
something this section does not cover, that is not a gap to fill by editing the file — **it is
something to say out loud before he agrees.**

---

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
goes to [`unattended-run-log.md`](unattended-run-log.md) and this section is overwritten by
the next plan — otherwise it becomes
the accumulating history `CLAUDE.md`'s *Current work* had to be rescued from.

### QUEUED — nothing. The slot is empty.

**Night 7 ran and closed 2026-08-31** — the document-contradiction lens, four findings, all
resolved. Its record is in [`unattended-run-log.md`](unattended-run-log.md).

**The lens is NOT spent**, so the rotation rule does not retire it: it returned four on its first
run. **But do not queue it back-to-back.** Everything it found was *same-day* drift — a mechanism
changed that afternoon and its description did not — so the yield came from a day of heavy
mechanism change, not from accumulated rot. Point it at the documents again after the next such
day, not tomorrow.

**The obvious next work needs Doug awake:** `connect-expansion` is rewritten and unwalked, Station 6
and `ScopedConnect` have never been read by anyone, and night 7 left him one ruling — hard rule 5
against the document item — **ruled the next morning**, and recorded in the log.

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
   **It reads the tree and decides.** What it does is described in `CLAUDE.md` and enforced by
   `gate_policy`; restating the verdict rule here is what made three of night 7's findings, so it
   is deliberately not restated.
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
5. **Every CODE item ends in a test that fails by name.** If a code item cannot, **it is not
   unattended work** — record it and move on.

   **Doug ruled 2026-08-31 that this does not govern document maintenance**, after night 7 hit the
   conflict: a prose contradiction fix cannot end in a failing test, and the queued item required
   exactly that work. **Document maintenance is not exempt from verification, only from this
   instrument** — its standard is the restore-never-choose boundary in the standing document step
   below, where a *source of truth* plays the part a failing test plays here. Both answer rule 0:
   **do only work whose success is verifiable without Doug.**
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
- **Lab prose.** Doug's lab runs are his primary learning exercise; rewriting an explanation
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
than by a check: `fixture-labs/README.md` said `node` earned its place while the lab it governs
had dropped the abstraction; a checker was named for vocabulary its subject no longer used; and a
pinned assertion quoted a sentence the lab had deleted.

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

It prints every reading path's size against its **ceiling** — the ceilings, their unit and their
derivation are stated in [`reading-budgets.txt`](reading-budgets.txt) and nowhere else — and lists
**passages of 400+ characters appearing in more than one document**. Exit **0** means nothing is
needed; **1** means the morning starts here.

**Why duplication and not just size.** The growth that caused the whole budget problem was never a
document getting long — it was **the same prose in two files**, four rulings written into both
`DECISIONS.md` and `fixture-labs/README.md` on one day. A size check sees that as "a document
grew" and bills for it; this sees the thing itself.

### What the night may do, and what waits for Doug

`CLAUDE.md` already authorises Claude to reorganise and condense documents without asking. **The
line is the same one that governs lab prose: an explanation is Doug's learning material, so
trimming one is his call.**

| do it unattended | leave it for Doug |
|---|---|
| retire closed history to `DECISIONS.md` | trim or reword an explanation |
| delete a duplicated passage, keeping one | restructure a README |
| fix a stale claim or a dead link | anything that crosses a ceiling |

**`doc_report` never edits**, deliberately. A sweep that both decides and acts, unwatched, on
documents whose value is judgement is exactly what Doug reserved to himself. It reports; the night
acts within the left column; a ceiling crossing goes in the run log and waits.

### RESTORE, NEVER CHOOSE — this is document maintenance's answer to hard rule 5

**Hard rule 5 does not govern this work** (Doug, 2026-08-31), because a prose fix cannot end in a
failing test. **It is not thereby unverified** — the instrument is different. **One question
decides every fix: is there a source of truth to restore to, or must I pick?**

| resolve it alone | bring it to Doug |
|---|---|
| a digest broadened or narrowed its source — **restore the source's scope** | two deliberate statements that genuinely differ |
| Doug's verbatim words are recorded and a gloss overreaches them | resolving it needs a ruling on what he meant |
| a document claims another says X; that one no longer does | the fix changes what a rule *permits* |
| a citation names a renamed or deleted symbol, file or test | anything requiring an explanation to be trimmed |

**Restoring a scope from Doug's own quoted sentence is a correction, not a judgement.** Choosing
between two things he said is a judgement and is his, **however obvious the answer looks at 3 a.m.**

**This lived in night 7's queued plan and was nearly lost with it** — the plan was cleared from the
single slot on completion, as that slot's rule requires, which would have taken the boundary with
it. Document maintenance is a **standing** step, so its boundary belongs here rather than in any
one night's plan.

**The four passes, likewise standing:** digest audit (a reading-path document against the source it
summarises — `CLAUDE.md` digests three others throughout, and both of the first two findings came
from this), quote provenance, cross-reference claims (*"X says Y"* — check that X says Y), then
rule pairs grouped by subject.

**A ceiling crossing reports and waits.** It is never a licence to raise the ceiling — that rule,
and why, is in [`reading-budgets.txt`](reading-budgets.txt).

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

**Moved to [`unattended-run-log.md`](unattended-run-log.md)** on 2026-08-31. Consult it when
choosing a lens — the rotation rule above turns on what previous nights already established.
