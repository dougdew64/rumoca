# CLAUDE.md — HRW Observatory

**Purpose:** the rules that bind, what is being worked on now, and where everything else lives.
**Status:** authority. **The one file to read at session start.**
**Read when:** every session, first.

Rust/egui observatory for studying the Rumoca Modelica compiler.
**[`docs/README.md`](docs/README.md) is the document index** — every file, its purpose, and
whether it is live. Go there rather than guessing.

**The hierarchy everything else derives from** (charter v1.4, Doug 2026-08-05): **his education is
the purpose**; **accuracy is the first corollary** (Decision 7) because an inaccurate instrument
teaches something false; **low friction is the second** (Decision 9) because an accurate
instrument that costs attention to operate spends the attention meant for learning. **Accuracy
outranks friction where they conflict**, and they rarely do.

Purpose, scope and binding decisions are in [`docs/CHARTER.md`](docs/CHARTER.md) (v1.4 —
**Decision 7, Accuracy, ranks above everything else in this repository**; **Decision 8, the
instrument assumes the reasoner**, governs what UI gets built at all: *the noun is assembled by
mouse, the verb is an unbounded utterance*. **The test is whether the answer is known in
advance** — fixed answers belong on screen because a tooltip beats a question for latency and
focus; answers that depend on what is being asked belong to Claude) —
consult it for any design question; **do not re-litigate settled decisions in-session.**
Append any nontrivial implementation choice to [`DECISIONS.md`](DECISIONS.md) with a one-line
rationale.

**[`docs/working-with-doug.md`](docs/working-with-doug.md) — read it too.** Who Doug is and
how he learns, which nothing else in this repository carries. The short form:

- **Decades of C/C++/Java, new to Rust and egui.** The gap is *idiom*, not concepts —
  translate (`trait` ≈ interface, ownership ≈ RAII + move), and frame Modelica-compiler
  concepts as **introductions, not reminders**.
- **Top-down, and problem before solution.** State the problem a step solves before the
  mechanism; he learns by understanding *why*.
- **The conversation is the instrument.** Sessions are teaching dialogues; **code changes are
  a byproduct of understanding, not the deliverable.** Explain the math alongside the code and
  name the textbook algorithm.
- **Propose features unprompted** that would deepen understanding — he asked for this, and the
  success metric is his understanding, not feature count.
- **Every change ships with tests, comments and doc updates**, unasked. **The source is itself
  a learning artifact**, so clean commented code ranks with the explanations, not below them.
- **Learning over polish**; **prefer Rust/egui over VS Code work**; **token cost is not a
  constraint** — never trade richness for economy.

---

## The rules

**EDUCATION IS THE PURPOSE, AND ACCURACY IS ITS PRECONDITION** *(Doug, 2026-08-04, the day
this rule was found missing)*. HRW exists so Doug can learn Rumoca. **A tool that
misrepresents Rumoca does not teach him less — it teaches him something false, and he has no
way to tell which parts are which.** Accuracy therefore outranks every other consideration in
this repository: features, polish, performance, a tidy log, a complete-looking pane, and **the
cost of a change to the Rumoca crates**.

**STOP AND FIX, AS OFTEN AS NECESSARY** — Doug's standing authorisation, in his words: *"We
will pause and fix code as often as necessary in order to deliver accuracy."* A day spent
removing fictions is not a detour from the curriculum. 2026-08-04 was spent entirely on it and
was the correct use of the day.

**AND THE PRIMARY REASON IS NOT DOUG'S COMFORT — IT IS WHETHER CLAUDE CAN STILL FIX IT AT ALL**
*(Doug, 2026-08-19, stating the principle that has governed the project all along)*: *"the
primary reason is so that we can increase the probability of you being able to fix bugs at all.
If we wait too long to fix bugs, it will become too difficult for you to fix those bugs without
also breaking other code which has been built atop those bugs."*

**One day supplied three demonstrations and one counter-example that sharpens the rule.**

- **The tour transport bar.** Nobody had noticed its wrap behaviour, but `MIN_LEFT_POINTS`, two
  divider-test thresholds, `MAX_TOUR_CHROME` and the picker's adaptive width had all been tuned
  *around* it. By the time it was touched, **five perturbations each failed differently**,
  because the surrounding code encoded the equilibrium. Two theories died before the third
  worked.
- **`differentiated_rows`.** A tour was written on the misreading, so correcting the field meant
  rewriting the tour.
- **The mirrored funnel.** It existed because Rumoca had no observation API, so deleting it
  required building one first.
- **The eighteen browser-opening links** — same age, and they cost **minutes**. Mechanical, all
  at once, no theories. **Because nothing depended on them:** nobody clicked them, so nothing had
  grown on top.

**So the cost scales with how much has come to DEPEND on the broken behaviour, not with how long
it has been broken. The danger sign is load, not age.** A three-day-old bug that things are
already leaning on is more urgent than a three-week-old one nothing touches.

**And the corollary that binds Claude specifically: he is bad at telling what depends on a
behaviour.** The divider proved it — four changes made on the assumption of independence, each
revealing another coupling. **So "I cannot tell what is built on this yet" is itself an argument
for fixing now**, while the answer is still small enough to discover.

**NOTHING HRW SHOWS MAY BE INVENTED.** Every number, structure, tree, animation frame and log
line must be traceable to something Rumoca **actually did on this run**. Three corollaries,
each bought with a fiction removed on 2026-08-04:

- **Absence is stated, never filled.** A pane with nothing to show says the compiler produced
  nothing and why. It does not derive a plausible substitute. The BLT tabs of a structurally
  singular model rendered blocks HRW had computed itself, and a learner reading them would have
  concluded the compiler decomposed a system it had refused to touch.
- **A derived view declares that it is derived.** Re-running a phase to observe it is
  sometimes the only way to see inside it, and that is **legitimate when labelled**. What is
  forbidden is presenting the re-run as the compilation. Every replay in HRW is now gone,
  replaced by capture scopes recording the real run — but the rule is about the label, not the
  mechanism, because the label was never the part that was blocked.
- **A log line describes what happened, not what reads well.** The "DAE pipeline" bracket named
  a phase that does not exist, in order to give five phases a tidy parent. Ordering, nesting and
  attribution are claims, and a claim that reads nicely is still a claim.

**"REPLAY" MEANS TWO THINGS, AND ONLY ONE IS FORBIDDEN** *(recorded 2026-08-04, after the
sweep found the collision)*. Both senses are live in this repository and in the UI:

- **Playback** — stepping through frames that were *recorded during the real compile*. This is
  the animation feature and it is **correct**. The UI says it (*"no frame 3 in this replay"*),
  the tours say it (*"the reduction replay opens"*), and `CompileFrames` holds them.
- **Re-execution presented as the compile** — HRW running an algorithm again when a tab opens,
  and drawing the result as if it were what the compiler did. This is the **fiction**, and it
  is gone.

**A third case is neither**: a **live debug session** genuinely re-runs a phase under the
debugger, because that is what the user asked for. `PendingLiveDebug::PreLowering` re-running
from the flat model is correct and must not be "fixed".

Without this distinction written down, a later session reads *"the reduction replay"* in a tour
and either **removes a working feature** or, worse, concludes the fictions were already dealt
with and stops looking. Judge by **where the frames came from**, never by the word.

**WHY THIS RULE WAS MISSING — and what its absence predicts.** The rules below protect against
*missing* reports (must-fire), *unchecked* claims of absence, and *misidentified* things (no
heuristic name-matching). **None of them forbade invented content**, so none of the fictions
ever felt like a violation: each was written as *"here is a way to show him this"*, and HRW's
tests check data while the falsehood lived in what the pane **claimed**.

The trap that hid it is worth naming, because it will recur in a new dress:
**[`fidelity-plan.md`](docs/fidelity-plan.md)'s programme verifies the NOUN, and its success
felt like it verified everything.** 2,614 green rows answers *"is HRW's IR faithful?"* Every
fiction removed on 2026-08-04 was about a **verb** — what the compiler did, in what order, what
it declined to do, whether it ran at all — and **not one of F1-F9 could have caught a single
one of them.** A fabricated BLT block is well-formed and round-trips; a good replay is
*by construction* indistinguishable in its output. **A large green result covers the territory
it measured and no more, and the confidence it produces does not know that.**

**Instrumentation of the Rumoca crates is intended, and must stay additive,
observation-only, and upstreamable.** **The checklist below is a quality bar, not a
discouragement** — added 2026-08-04, because its cumulative weight had become one. Every
Rumoca edit carried a checklist and every HRW edit carried none, so when two paths led to the
same pane, the ungated one won and the fiction accumulated. The capture scopes that replaced
every replay landed in **two days** once Doug priced it explicitly: *"it is much better to
defend a rumoca api change to the repo maintainers than to defend replays."* They were
**unpriced, not difficult.** When accuracy needs a Rumoca change, the change is the cheap
option. Across a crate boundary a phase's `pub(crate)` internals
are unreachable, so "accessing internals" means **additively widening visibility / adding
observation hooks in `../crates/rumoca-*`**. Semantics-preserving, so HRW stays faithful to
real Rumoca and rebases stay clean.

- **After touching a `crates/rumoca-*` file, run BOTH:**

  ```powershell
  cargo clippy -p <that-crate> --all-targets
  cargo fmt -p <that-crate> -- --check
  ```

  Those crates are clippy-clean **and rustfmt-clean**, and `[workspace.lints]` denies. **Upstream
  CI runs both** (`cargo fmt --all -- --check` is a gating job), so either one failing sinks a PR
  before a maintainer reads it.

  **`fmt` was missing from this rule until 2026-08-05**, and it cost 82 unformatted hunks across
  four crates — accumulated over a week in which clippy was run every single time, exactly as
  written. **A rule that names one of two gates reads as complete.** Found only when the fmt work
  was planned, not when the instrumentation landed.

  **They interact, which is the reason to run them together rather than in sequence at the end.**
  Formatting rewraps lines, and rewrapping pushed `reduce_constrained_dummy_derivatives_with_trace`
  from 99 lines to **102**, over `too_many_lines`'s threshold of 100 — so a formatting pass turned
  into a build failure. Run `fmt` first, then `clippy` on the formatted code; the reverse tells
  you the code was clean in a shape it will not ship in.
- **Commit Rumoca crate changes separately from HRW code**, so an upstream PR is a clean
  cherry-pick.
- **Ask before adding a dependency.** Record accepted ones in `DECISIONS.md`.
- **After changing traced algorithm code, update the guided tours.**
  `docs/compiler-phases/*/guided-tour.md` quote **line numbers, code excerpts, locals and enum
  variants** from `crates/rumoca-phase-structural/` (`matching.rs`, `tarjan.rs`,
  `live_trace.rs`). **Nothing compiles them, so they go stale silently** and a learner
  following one with wrong line numbers is simply confused. Grep the tours for any name that
  moved. This applies to *any* such change, not only a rebase — `docs/updating-rumoca.md`
  step 6 is the rebase instance of the same rule.

**When you write a memory, name where it belongs in the repo** (`DECISIONS.md`, "the
repository is the system of record"). **The memory store does not survive a clone** — it lives
outside the repo, keyed to the project's filesystem *path*, so a different machine *or a
different clone path* loses all of it. If a fact has no home in the repository, that is the
finding.

**THE MUST-FIRE RULE.** Any code whose job is to *report* something gets a test proving it
reports; **silence must be a failure, never a pass.** Its absence makes a change incomplete.
All seven silent bugs of 2026-08-01 were observers that looked like they worked: a dead column,
an array argument collapsed by `powershell -File`, an `eprintln!` swallowed by HRW's own
fd-level `OutputCapture`, a rate limiter gating its own first fire, an announcement silent when
work was pending by absence. `fidelity.rs` had this discipline
(`each_invariant_catches_its_own_violation`); the tooling around it did not.

**A PANE IS A REPORTER TOO, so a new pane that reports something ships with a headless
test** (added 2026-08-01). The Context Bar showed three true things and silently omitted a
fourth — the background — for weeks, because **a partial report leaves no gap where the
missing part was**: everything on screen was correct. Doug caught it; nothing could have.
Retrofitting the existing panes is logged in [`docs/tech-debt.md`](docs/tech-debt.md) ("UI
testing debt"), which also records what `egui_kittest` genuinely cannot reach — **two
surfaces**: `incidence_view.rs` cells and `spyplot.rs`. The animations *are* testable.
**Not growing the debt is free.**

**NARROWED 2026-08-20: that is a claim about the PIXELS, and it had been read as a claim about
the panes.** `matrix_panes.rs` — the pane that decides which cache is filled from which half of
the report, which camera looks at it and what is said when there is nothing to show — now has
**six tests running in 0.02 s**, because captions, split headings and absence notices are
ordinary labels and a cache is a field a test reads after the frame. Nothing in either painter
changed to allow it. **Two surfaces cannot be reached; the panes around them always could**, and
one of those tests catches a Before/After swap that is invisible to every check about what
reached the screen. Same shape as the scroll-area correction below: *a null result measured at
one level was generalised into a property of the whole thing.*

**SCROLL-AREA CONFIGURATION WAS THE THIRD, AND THAT CLAIM WAS FALSE** *(corrected
2026-08-12, after a defect hid behind it for eight days)*. `both()` vs `vertical()` was
recorded as *"config, not behaviour — nothing observable differs"*, on three real
measurements that were all correct and all taken **inside** the scroll area. What differs
is **the size of the enclosing panel**: a vertical-only area reports its content's full
width as the width it wants, so the tour panel opened at 899pt of a 1280pt window instead
of 512pt and the divider froze. **The question nobody asked was "does the container
change?"** — and a scroll axis is precisely a claim about how a widget negotiates size
with its parent. `ui_tests::the_left_panel_content_never_detaches_from_the_divider` now
fails by name when the axis is reverted.

**The transferable rule: when a null result is about to become "this cannot be tested",
check whether every probe was aimed at the same level.** Three null results inside one
widget were generalised into a property of the widget, and that sentence stopped anyone
looking for eight days. A wrong *negative* is the error nobody catches, because acting on
it means **not looking** — the same asymmetry the claims-of-absence rule below is built
on.

**AND THE SECOND SCROLL-AREA BUG, 2026-08-16: NEVER NEST A VERTICAL SCROLL AREA INSIDE
ONE.** Doug: *"the connection sets lists are not using all available vertical space…
showing only three connection sets per list."* The rule: **the parent owns the scrolling
and the height, and a child view just renders.** A tall model then makes a tall pane,
which is the honest result. **The nesting is the defect; the height cap only set how
obvious it was** — the full account is on the test's own doc comment.

`playback::tests_layout` fails if a view `app.rs` wraps in a scroll area constructs one or
caps its own height, and **derives that roster from `app.rs` instead of naming a file.**
The per-file version guarded one view for a week while the identical defect sat in
`alias_anim` and `ic_plan_anim`; generalising it on 2026-08-23 found both on its first run.

**Both scroll-area bugs were reported by Doug, not by a test**, and neither is visible to
`egui_kittest` — a clipped child is still in the accessibility tree. Layout remains the
surface where his report *is* the verification.

**A RULE IS ALSO A CLAIM ABOUT ITS SCOPE, AND THAT IS THIS REPOSITORY'S MOST FREQUENT
FAILURE** *(named 2026-08-21, from four instances that had each been filed as a separate
correction)*. Every one is the same shape — **a statement true in one domain, applied in a
wider one**:

- **`fmt` missing from a two-gate rule**, which "read as complete" and cost 82 unformatted
  hunks across a week in which clippy was run every single time.
- **A claim about the PIXELS read as a claim about the PANES** — `matrix_panes` had six
  tests available the whole time it was filed as untestable.
- **Three null results inside one widget generalised into a property of the widget**, which
  stopped anyone looking at the scroll axis for eight days.
- **A rule about extracting a function applied to relocating a test module**, which deferred
  the largest and safest step of the `app.rs` arc — 71 % of everything that file shed.

**So the mechanism, and it costs one clause: state what a rule does NOT forbid, beside what
it does.** The template already exists here and is proven — *"REPLAY means two things, and
only one is forbidden"* was written precisely so a later session would not delete a working
feature. **Every rule that has bitten us this way would have been safe with one such
sentence.**

**AND THE INTERPRETIVE HALF, which is what a reader can act on when the rule is already
written badly: a policy blocking something obviously valuable and zero-risk is evidence about
the POLICY, not about the action.** When a rule forbids the highest-value, lowest-risk move
available, the odds favour a misreading over the move being wrong. **Stop and re-read the rule
before abandoning the move.**

**A QUALITY BAR CAN BECOME A DISCOURAGEMENT, AND IT HAS NOW DONE SO TWICE.** The first was
the Rumoca instrumentation checklist: every Rumoca edit carried one and every HRW edit carried
none, so the ungated path won and fictions accumulated for weeks. The second ran the other
direction — **the test-block move was deferred *because* it appeared to need justification**,
while ordinary extractions that fit the rule's template proceeded unexamined. **A rule shapes
behaviour by making one option feel illegitimate, whatever it says.**

**The fix that worked the first time is the fix: price it explicitly, out loud.** The capture
scopes landed in two days once Doug said *"it is much better to defend a rumoca api change to
the repo maintainers than to defend replays."* They were **unpriced, not difficult.** When a
rule seems to forbid the obvious move, say what each option actually costs before concluding
the rule wins.

**A PLAN ORGANISED AROUND ONE KIND OF ACT HIDES EVERY OTHER KIND** — the contributing cause
of the same episode, and it is not a policy failure at all. `app-split-plan.md` was structured
around **seams**, so an act that is not a seam had no row in it and stayed invisible until
someone stepped outside the frame. **When a plan has produced no progress on something
obvious, check whether its own structure has a place to put it.**

**INSERT A TEST AFTER A FUNCTION'S CLOSING BRACE, never before its `fn` line** — anything placed
between a doc comment and its item is adopted by the wrong one, and the old function silently
stops being a test. Bitten three times; the history and the mechanism are on
`doc_citations::no_function_has_two_test_attributes`, which catches it.

**A CHECKER RETIRES THE PROSE IT REPLACES** *(2026-08-22)*. When a rule becomes a test, the prose
here shrinks to **one sentence and a pointer at the test** — the reasoning belongs on that test's
doc comment, beside the code enforcing it, where it cannot drift. This paragraph paid for itself
that way: `no_function_has_two_test_attributes` and `claims_of_absence_are_still_true` already
carried their histories, so the copies here became pointers.

**The budget is the forcing function, not goodwill**
(`doc_citations::the_mandatory_reading_path_stays_small`), **and a pointer must resolve**
(`doc_citations::qualified_citations_resolve`) — which is what makes this safe against a rename.

**EDIT FILES WITH THE EDIT/WRITE TOOLS. Do not generate source text through a shell.** Three
separate corruptions on 2026-08-01 share this one root, and they were *silent* — the tool
reported success every time:

- **Shell quoting ate content.** `python -c "…"` in Bash: backticks became command
  substitution and swallowed two file paths, including the one pointer that made a memory
  useful. Never route content containing backticks, `$` or backslashes through `-c` or a
  heredoc.
- **Escapes leaked one language into another.** Generating Rust from Python string literals
  put a literal `\u{2014}` into comment text. Content must be **literal in the tool call**, not
  a string inside a string.
- **Line arithmetic stole an attribute.** Inserting by index instead of by seen context put a
  test between a doc comment and its `fn`, silently un-testing a regression guard.

**A generator script is the exception, not the tool of choice.** When one is genuinely
warranted, write it with the Write tool and run it by path — that pattern never produced shell
corruption. And **read back anything a shell wrote.**

**Do not sell the `app.rs` refactor on these.** A large file *pressures* Claude toward
generators, but the corruption is a habit that operates on small files too — the memory case
proves it. Recorded 2026-08-02 after Doug pushed back on exactly that over-claim: the
refactor's justification is blast radius and testability, measured in
[`docs/ui-pause-plan.md`](docs/ui-pause-plan.md), and it did not need the help.

**DO NOT COMMIT TROUBLESHOOTING INSTRUMENTATION, AND DO NOT CALL AN UNCONFIRMED FIX A FIX**
*(Doug, 2026-08-03, after the LHS-width episode)*. Ten commits went into one 40 %-width bug:
**two were pure instrumentation and four were fixes that did not work.**

- **A probe lives in the working tree until it earns permanence.** It reaches Doug's running
  app by being *edited* — he builds from the tree — so committing adds nothing and pushing adds
  less. Commit it only once it is something the repository should keep, and say why.
- **A fix that cannot be reproduced locally is a hypothesis.** Say so in the subject line, not
  three paragraphs down. Four of those commits read as fixes and were not.

**The cost is not tidiness.** This repository is **public**, and `docs/upstream-strategy.md`
stakes Doug's credibility with Rumoca maintainers on work that is *reproducible and honestly
bounded*. A log showing six superseded commits for one bug reads as thrashing, whatever the
messages say. The failure was applying a work-product ritual — green suite, commit, push — to a
probe, using *"is this verified?"* as the test instead of *"should the repository keep this?"*

**TAG A CLAIM OF ABSENCE, or it rots unnoticed.** The must-fire rule pointed at silence; this
is the same principle pointed at **absence**. When a document says something is not built,
tag it so the claim is checkable:

```markdown
Sorting the corpus is not built. <!-- unbuilt: survey::sort_rows -->
```

**Name the symbol as it will actually be spelled**, because the mechanism only fires when the
target *does* resolve — **a misspelled target is silently permanent**, and a tag that resolves
nothing is indistinguishable from a claim that is still true. `doc_citations::claims_of_absence_are_still_true`
fails if the target resolves; its doc comment carries the four stale cases and why a wrong
*negative* is the error nobody catches. Coverage is expected to be low — tag when you write the
claim, the way provenance tags do.

**REFACTOR FOR CLAUDE'S COMPREHENSION, NOT FOR A HUMAN'S** *(Doug, 2026-08-05 — standing
policy)*. His words: *"no human being has yet needed to comprehend or maintain any functions
[in HRW]. Instead, at least so far, you have been doing the comprehending and maintaining. …
We will refactor HRW functions when doing so improves your ability to comprehend or maintain
those functions, or will improve your ability to test those functions and keep them correct."*

**So the trigger is one of three, and none of them is a line count:**

1. Claude's **comprehension** of the code degrades.
2. Claude's **ability to maintain** it degrades.
3. A refactor would **improve testability** — the same rule
   [`docs/format-and-app-plan.md`](docs/format-and-app-plan.md) already states: *no extraction
   lands without a test that could not have been written before it.* **That rule governs
   extracting a FUNCTION OR A TYPE and nothing else** — see its scope note, added after it was
   read as governing a file-level move of a `#[cfg(test)]` module and deferred the largest,
   safest step of the whole arc.

**This is why the three complexity lints are declined** (`hrw/Cargo.toml` carries the full
reasoning). They encode a human-comprehension heuristic, and enforcing it would reward splitting
a function *to satisfy the lint* — extraction with no new seam and no new test.

### Trigger 2 FIRED for `app.rs` on 2026-08-19 — the split ran, and the FRAMING is what generalises

**The arc is closed** (2026-08-21; the record is [`docs/app-split-plan.md`](docs/app-split-plan.md),
the closure box is under *Current work*). What is kept here is **how the trigger was stated**,
because that is what the next candidate will be judged by.

**The case was never that the file was large.** That is the heuristic this policy exists to refuse,
and it would equally have licensed splitting `worker.rs` — which was larger for most of the arc,
is larger again now, and **has caused none of the trouble below.** It was deliberately left alone
as the control.

**The case was that it exceeded what Claude could hold, with defects to show for it.** In one
session: line-number arithmetic used to locate an edit (one of the three silent-corruption causes
this file names), Rust generated through a shell three times with doc references silently swallowed
twice, and repeated edits made against stale assumptions about surrounding code. Each was cheaper
than reading the region first — which is the definition of a file too big to maintain.

**So state the trigger as "exceeds what Claude can hold, with defects to show for it", never as
"large".** That distinction is what stops the next session splitting files by line count — and it
is the reason `worker.rs` is still one file.

### And a second observable signal: HANDOFF FREQUENCY

**Doug, 2026-08-19:** *"it has seemed that you are needing to perform context maintenance more
frequently lately. Perhaps the increasingly large files such as `app.rs` are contributing to that.
I hope that you will consider context maintenance as a trigger for code refactoring."*

**This is worth having because it is external and countable.** This file already warns that
**Claude is a poor sensor for his own comprehension failures** — both August examples were caught
by the compiler, not by noticing confusion — and the only reliable signal recorded so far is
*"defects only a human caught"*. Handoff frequency is a second one that does not depend on
Claude's self-report at all.

**The mechanism is plausible:** a large file costs context per edit, context spent is session
length lost, and shorter sessions mean more handoffs. So rising handoff frequency is a candidate
proxy for maintainability decay.

**It is a trigger to MEASURE, not to refactor on.** Handoffs also rise with prose volume, long
gates and simply doing more in a session — 2026-08-19 involved a great deal of writing — so the
correlation is unestablished. When it fires, the response is to find out *which files the session
was reading*, not to start extracting.

**AND THERE IS A CONFOUND THAT LARGELY DISARMS IT, NAMED THE SAME DAY IT WAS ADOPTED.** Doug,
2026-08-19: *"ever since I switched to Opus 5, you've been context-limited. You seemed not to
experience context maintenance problems when we used Opus 4.6."* **A model change and a file
growing cannot be told apart by counting handoffs**, so the signal cannot currently attribute
anything to `app.rs`. Use it only when the model has been stable across the compared period, and
say which model when recording a count. **Claude has no reliable introspective access to his own
context size and must not estimate one** — that number belongs to the tooling, not to a guess.

**Three costs measured that day, which do not depend on the model:**

- **`app.rs` at 14,437 lines.** Editing it repeatedly caused the whole file to be re-injected —
  hundreds of lines per occurrence. The largest single lever, and the reason
  `format-and-app-plan.md` Step 3 reopened.
- **Claude's own verbosity.** Commit messages ran 30–40 lines each that day. Thoroughness had
  been treated as free and is not.
- **Measure-revert cycles.** The divider investigation took nine or ten build-test rounds. Worth
  paying, and worth *noticing* — a session doing that has less room for everything else.

**What the evidence says length does to Claude, measured 2026-08-05.** It bit twice this week,
and both times the cause was **local context at the edit point**, not total length: the
`Provenance` enum inserted between `#[derive]` and its struct, and an `events_stage` borrow
error from restructuring a match whose arms moved a value. It did **not** bite across roughly
eight edits to `compile_target` (**1,085 lines**), which is linear and heavily commented.
`compile_target` is hard to *test* because it takes `&mut self` and emits through a closure —
**not because it is long.**

**Claude is a poor sensor for his own comprehension failures**, and this policy should not rest
on his self-report. Both failures above were caught by the **compiler**, not by noticing
confusion. The reliable signal is already written down: `tech-debt.md`'s **trigger 2 — code that
has produced defects only a human caught.** If a function starts producing defects Doug finds
and Claude does not, the criterion has fired regardless of what Claude reports.

**The human reader is Doug, and he named two scenarios** *(2026-08-05)*. They pull in opposite
directions, so the policy is **two-tier**, not one rule:

**Scenario 1 — Doug reads to understand, and asks Claude.** *"I will need to gain a rough
understanding of all of this HRW code which you have written… so long as you can answer my
questions about the code which you wrote, then all will be well."*

> **The rule this creates is on CLAUDE, not on the code: always be able to answer Doug's
> questions about code you wrote.** Its practical consequence is that **the *why* must live in
> the repository, not in a conversation that scrolls away** — a comment, `DECISIONS.md`, or a
> doc. Code whose rationale exists only in chat violates this rule the moment the session ends.
> It also rules out writing constructs whose behaviour Claude would have to guess at.

**Scenario 2 — Doug edits the visualizations himself.** *"Eventually it will become impractical
for me to describe to you the details of visualizations which I want… I will likely make changes
to the code by myself and then request that you comprehend, improve and test the code which I've
written."*

> **The visualization files are held to HUMAN comprehension**, and Doug specifically: decades of
> C/C++/Java, **new to Rust and egui** (`docs/working-with-doug.md`). Measured 2026-08-05, that
> surface is **`canvas.rs`, `incidence_view.rs`, `matching_anim.rs`, `tarjan_anim.rs`,
> `spyplot.rs`**, plus any new custom-painted view.
>
> **The barrier there is idiom, not length.** A 203-line `draw_matrix` that is a linear sequence
> of paint calls is the *easy* kind of code for a C++ programmer. What is hard is closures
> capturing state, iterator chains standing in for loops, `impl Trait`, and borrow-checker
> dances around `&mut Ui`. **Prefer the plain form in these files even when the idiomatic Rust
> is terser**, and comment the egui idiom where it appears, because he is learning it here.
>
> **Keep geometry named and single-sourced.** `tarjan_anim::equation_world_pos` is the pattern:
> one function owning where a thing sits, so changing the layout is changing one place, and
> drawing and camera-aiming cannot disagree.
>
> **AND THE SHARP PROBLEM: these are the least-testable files in the project.** The
> surfaces `egui_kittest` cannot reach are `incidence_view.rs` cells and `spyplot.rs`
> — **exactly the code Doug will edit.** *(Scroll configuration was listed here as a third
> and is not one; see the correction above.)* So the response is to
> **push logic out of the paint path into checkable data**, as `Plot::problems()` and
> `IncidenceMatrix::problems()` now do: a thin renderer over verified data means his edits land
> on a small surface whose correctness he can see, rather than on parsing whose errors are
> invisible. **When touching these files, move a computation out before adding one in.**
>
> **This applies to new visualization code and to files as they are touched — not as a refactor
> campaign.** Doug said *eventually*; building for it now would be speculation.

**The `crates/rumoca-*` instrumentation is not covered by any of this** and stays under
`[workspace.lints]`, complexity lints included, because it is offered to human maintainers
**now**. `docs/upstream-strategy.md` puts HRW itself last among deliverables, being the only one
asking for maintenance burden.

**DO NOT optimise HRW to widen test scope** (Doug, 2026-07-31 — standing boundary,
[`docs/fidelity-plan.md`](docs/fidelity-plan.md)). Measurement showed HRW's *compile path*, not
the checks, costs 30 s and 3.5 GB on a 4,193-equation model. Doug: *"we should not redesign
worker.rs's compile path. Perhaps ever… If some models cannot be fidelity-tested within our
limits, so be it."* The stage JSON trees, equation sheet, identifier index and animation frames
**are the product**. Raising `-TimeoutSec` / `-MaxProcGB` when measurement justifies it is
calibration, not optimisation, and is fine. **HRW is an education project, not a production
tool.**

**THE PROHIBITION IS REVISABLE ON EVIDENCE, AND "PERHAPS EVER" WAS HIDING THAT** *(Doug,
2026-08-21)*: *"until we have an evidence-based reason to change our policy, let's maintain our
prohibition against a redesign of worker.rs's compile path."* Unchanged in force — **and now
carrying the condition under which it could change**, which the old phrasing gave a reader no way
to find. That is the *"state what the rule does not forbid"* mechanism applied to this rule.

**AND THE OPERATIONAL HALF, because this is where it will be got wrong: "evidence-based reason"
means bringing the evidence to DOUG, never concluding in-session that the evidence authorises
proceeding.** The live temptation is concrete — `#48` may measure MSL loading as the dominant test
cost, and a session could read that finding as permission. **It is not.** The measurement is a
finding; the policy change is Doug's call. **Splitting `worker.rs` into modules is not a redesign
of the compile path** and needs no such permission; changing how the MSL session is loaded, cached
or shared does.

**Refactoring `worker.rs` therefore has a boundary `app.rs`'s never had: extract AROUND the
compile path, do not restructure it.** `compile_target` (1,085 lines) will invite exactly that,
and it is on record as hard to *test* because it takes `&mut self` and emits through a closure —
**not because it is long.** That is a testability seam, which is the licensed kind.

**The composition primitives are frozen** — one point-at + one follow + background, unchanged
until a practical scenario demonstrates a need. Multiple `follow` items and a third "compare"
primitive were considered and deliberately not built; **do not re-propose them from first
principles.**

**No heuristic name-matching** — [`docs/identity-and-provenance.md`](docs/identity-and-provenance.md).
No substring search ever decides identity. Cited by six source files.

**Documents divide by audience, and so does who judges them** ([`DECISIONS.md`](DECISIONS.md),
2026-08-01). Everything except READMEs is **Claude's, maintained without asking** —
reorganise, condense, delete a rotted file, correct a stale claim, all on Claude's own
judgement. **READMEs and their further reading are for Doug and Rumoca maintainers**, and the
test is whether a reader acts without asking Claude; an index link is not an endorsement, and
a README states facts it does not own **by reference, never transcription**. **`hrw/README.md`'s
value case is a joint rewrite** — keep that file accurate and current, but **do not invest in
persuasion alone**: whether prose lands is the one signal Claude cannot generate, and the
solo attempt at explanation (`end_to_end_tour.md`) is the project's clearest failure.

**Tech-debt sweeps have TWO triggers** ([`docs/tech-debt.md`](docs/tech-debt.md)). Forward:
each phase boundary, scoped to what the next phase touches. Backward (added 2026-08-01): **code
that has produced defects only a human caught.** Ask *"who caught it?"* — toolchain, nothing to
sweep; Doug, the code lives somewhere nothing checks. The property is **verifiability, not
Rust**; adding a test, a non-vacuity guard, or a loud failure is often cheaper than converting.

---

## Current work

> ### ⟶ OPEN THE NEXT SESSION WITH THIS
>
> **NIGHT 3 RAN — 3 items, 3 commits, nothing pushed.** Its record, one finding owed to Doug,
> and the lens it changes: [`docs/unattended-runs.md`](docs/unattended-runs.md). No run is queued.
>
> **This section holds ONLY what is in flight. Everything closed lives in
> [`DECISIONS.md`](DECISIONS.md)** — *"closed arcs move out of `CLAUDE.md`"*, 2026-08-22, which
> names each arc and the file that holds its record. **Do not restate a closed arc here; link it.**
>
> **The rule, and it is now enforced rather than remembered:** what is in flight goes at the top,
> standing context underneath, and **a ✅ box is history the moment its arc closes.**
> `doc_citations::the_mandatory_reading_path_stays_small` fails by name if this section or the
> reading path outgrows its budget; **its doc comment carries why a budget rather than another
> prune.**
>
> ### ⟶ TWO MODES RUN IN PARALLEL, SPLIT BY DOUG'S AVAILABLE ATTENTION — 2026-08-21
>
> *"During my mornings and evenings I can focus on walking tours. But during my workdays I cannot
> focus on this project as much. So, during my workdays, I will task you with performing refactoring
> and fixing bugs."*
>
> | when | mode | cost to Doug | cost to Claude |
> |---|---|---:|---:|
> | mornings / evenings | **walking tours** — teaching dialogue | ~6 s per iteration | conversational |
> | workdays | **refactoring + bug hunting** — low supervision | ~0 | the FULL ~220 s gate |
>
> **TOUR PROSE IS NOT WORKDAY WORK, and the reason is not scheduling** *(Doug, 2026-08-22)*: *"most
> of my conceptual learning happens when iterating with you during [tour] walks… making the tour
> prose correct and personally effective during those walks is my primary learning exercise
> right now."* **Improving an explanation alone consumes the material his learning runs on.** Fixing
> a checker-caught number, a dead link or a stale citation is fine; **rewriting an explanation
> unsupervised is not Claude's to do.** See [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md).
>
> **The gate lands on Claude in the workday mode, not on Doug — which is why `#48` closed.** The
> friction Doug named was *his* waiting, and tour work does not pay it.
>
> **THE DECISION BOUNDARY, and it matters more when nobody is watching.** Claude decides seams,
> extractions, tests, and bug fixes that arrive with a test failing by name. Claude **brings back**:
> anything trading fidelity for anything else, `worker.rs`'s compile path, any step toward
> `upstream-issues.md` P1, and anything that changes what a pane *claims*. **2026-08-21 is the worked
> example** — lever B was Claude's to measure and Doug's to rule on, and Doug declined it on fidelity.
>
> **BIAS TO CHECKABLE OUTPUT, because the one reliable signal for Claude's comprehension failures is
> *"defects only a human caught"* — and that signal weakens exactly when Doug is less available.**
> Prefer work whose success is verifiable without him: a guard that fails by name, a prose claim
> converted into a test. Both defects found on 2026-08-21 were that shape — a test that never tested
> what it was named for, and a doc comment false since `last_specimen_uri` landed.
>
> **THE FAILURE MODE IS SPRAWL, NOT IDLENESS.** One item per session still binds; column-read audits
> are the cheap parallel activity and consume none of that budget. Three finished things with tests
> beat eight half-done ones, because Claude is bad at telling what already depends on a behaviour.
>
> **AND TASKING WORKS BEST AS A GOAL, NOT A FILE** — *"find bugs in the artifact pane"* beats
> *"refactor `app.rs`"*, because the seam-selection heuristic changes with the goal and the
> cheapness-driven seams are spent.
>
> ### ⟶ THE TOUR MODE: **TOURS**, NOT FEATURE CODE — Doug, 2026-08-21
>
> *"My hunch is that we are shifting project modes from changing HRW rust code to changing tours.
> In other words, my hope going forward is to focus on improving tours, not feature code."*
>
> **This is the mode, and it also closed `#48`.** The gate is keyed on `src/`, `crates/`,
> `examples/` and `Cargo.toml`; **tour work touches none of them.** Measured 2026-08-21: editing
> `index-reduction.md` costs **6.1 s** to check and **~36 s** to commit. So the test-time friction
> Doug called a failure mode is ~6 s in the mode he is entering — which is why further optimisation
> was declined rather than missed.
>
> **THE TOUR ITERATION LOOP, and it is not the gate:**
>
> ```text
> cargo test -p hrw --lib -- --test-threads=1 doc_citations tour   # 6.1s -- while editing
> cargo run -q -p hrw --example gen_tour_catalogue                 # 9.9s -- ONLY if a ## heading changed
> cargo test -p hrw --lib -- --test-threads=1                      # 29.9s -- before the commit
> ```
>
> **TWO TRAPS, both of which have cost the full 220 s gate before:**
>
> - **`connect-expansion.md` is the one expensive tour — but only its five guarded tables are.**
>   It is the only tour carrying `<!-- pane-groups -->` / `pane-origins` / `pane-frames` tables,
>   which slow-gated tests verify against a real compile. **Editing one of those tables means
>   FULL**, whatever the diff-grep says; **editing its prose does not, and never did.**
>
>   **YOU NO LONGER HAVE TO REMEMBER THAT** *(built 2026-08-22)*.
>   `doc_citations::editing_a_guarded_tour_table_needs_the_full_gate` compares every guarded region
>   against `HEAD` and **fails by name in the FAST suite** if one changed, naming the marker and
>   printing the FULL command. It is gated *off* under `slow-tests` — in a FULL run the real
>   checkers are executing, so **the cheap gate is the only place the warning is useful.**
>
>   **The gain is assurance, not permission.** Prose edits were always FAST; what was missing was
>   any check that an edit believed to be prose actually was one. The filtered iteration line
>   catches it, so a green `doc_citations tour` run now means FAST was genuinely the right gate.
> - **Any `##` heading edit changes `CATALOGUE.md`.** Forget `gen_tour_catalogue` and
>   `tour_catalogue_is_current` fails. The order is `cargo fmt` → generators → checks, and getting
>   it backwards has cost the whole gate four times.
>
>   **AND THAT IS NOT THE ONLY TRIGGER — the blurb is the tour's FIRST BOLDED LINE** *(found
>   2026-08-22)*. `tour::catalogue` takes each tour's summary from the first line starting with
>   `**`, so **inserting any bolded paragraph above the existing one silently replaces the
>   catalogue's summary** — in this case with a mid-sentence fragment, *"MLS §9.3 requires connected
>   connectors to be type-compatible, and Rumoca does check. Four"*. Nothing about headings was
>   involved. **A new intro section goes BELOW the tour's opening bold line**, or the catalogue's
>   description of that tour changes with it.
>
> **⟶ ✅ CONNECTIONS IS WALKED AND PROTECTED — 2026-08-22. THE NEXT TOUR IS `dae-construction`.**
>
> The restart Doug announced on 2026-08-21 is complete: `connect-expansion.md` is walked, and its prose is held by **ten walked regions**. **Doug, 2026-08-22:** *"I will walk tours
> in the same sequence as the compiler phases."* That sequence is `the-concepts.md`'s own numbering,
> so the order is **dae-construction → matching (→ matching-live) → blt-ordering → tearing →
> index-reduction → initialization → solve-lowering → events.**
>
> **`index-reduction.md` is mid-walk and waits its turn at #6** — asked and ruled the same day.
> Doug was shown that it sits five tours ahead of where the order reaches it and answered *"I will
> walk the tours in compiler phase order."* **The sequence governs; there is no exception for the
> tour already in progress.** Its harder bar — index reduction explained to someone with only basic
> calculus — is unchanged and waiting, not abandoned.
>
> **`connect-expansion.md` is still the one tour carrying `pane-groups` tables**, so editing those
> tables still means the FULL gate — that warning outlives the walk.
>
> **He also graded the last walk, and the grade is the useful part:** the opening of
> `index-reduction.md` is *"good. Not yet very good, but nevertheless good. You are definitely
> starting to figure this out."* **Good is not the target for that tour** — see below.
>
> **`index-reduction.md` CARRIES A HARDER BAR THAN EVERY OTHER TOUR**, and Doug has staked it in
> public with a PhD Modelica friend: **an explanation of index reduction that anybody with only
> basic calculus can understand.** He intends to prove it can be done. The full constraint, what it
> forbids assuming, and the three corrections that are worked examples of it are in
> [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md) — read that before touching that
> tour. **The bar is PREDICTION, not comprehension: a correct tour can still fail it, and only the
> walk can measure that.**
>
> **Convert and improve ONE TOUR AT A TIME — the one Doug is about to walk.** Seven of nine have
> never been walked; `docs/fixture-tours/README.md` carries the template and its five rules.
>
> ### ⟶ AFTER `#48`: RESUME REFACTORING, AND THE GOAL IS BUGS — Doug, 2026-08-21
>
> **The `app.rs` arc's real return was defects found, not lines moved, and the numbers separate
> cleanly.** `app.rs` shed **7,961 lines — and 5,613 of them, 71 %, came from one move that
> refactored nothing** (the `cfg(test)` blocks). Extraction accounted for ~2,348. Meanwhile the
> defect yield held steady across the whole arc: **eight found by extracting**, with the last three
> iterations each still turning something up. **Two diverging curves, and the bugs are the one
> worth buying.**
>
> **So: resume `app.rs` with bug discovery as the stated goal, then `worker.rs`.** The order is
> Doug's. This is trigger 3 (testability), not a line-count target — which the policy above
> refuses, and which the 71 % figure shows was never the thing delivering value anyway.
>
> **THE SEAM-SELECTION HEURISTIC CHANGES WITH THE GOAL, and this is the part a session will
> otherwise get wrong.** Most of the arc chose seams by **cheapness** — the coupling table, the
> zero-`self` sweep. **If the goal is defects, choose by where defects are likely**: code never
> closely read, code that cannot currently be tested, and clusters of siblings where one member
> may differ. The plan's cheap seams are spent; that is expected and is not a reason to stop.
>
> **AND RUN COLUMN-READ AUDITS AS A CHEAP PARALLEL ACTIVITY, because four of the eight defects
> came from that ONE tool** — reading a list of siblings as a column and finding the odd member.
> It found the stranded `Animate` arm, the alias defect, the Flatten stranding and the artifact
> pane's missing gate. **It needs no extraction at all**, so it does not consume the
> one-item-per-session budget. Extraction was the forcing function that made someone look, not the
> mechanism that found them — so schedule the looking directly.
>
> # THE WALK — standing context
>
> ## DEFAULT TO TEACHING, NOT TO BUILDING — a standing instruction, not a mood
>
> **Doug, 2026-08-08:** *"I will finally begin a serious walk through the tours and try to shift our
> conversation to be about my education rather than about HRW features."* And the reason, which
> should not need saying twice: *"We've been working on this project for three weeks, and I have not
> yet been rewarded with a learning experience."*
>
> **When Doug reports something during a walk, the first question is *"what does this teach, and is
> it true?"*** — not *"what should we build?"* A feature is warranted when it unblocks the learning;
> `docs/ideas.md` is where the rest goes. **Treat an hour of HRW polish during a walk as a cost.**
>
> **The template, and the five things that make it work, are in
> [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md)** — read it before writing or
> converting a tour. **Improve ONE TOUR AT A TIME**, the one he is about to walk, never a campaign:
> *"working through each conversion with you is educational for me."*
>
> ### The three agreements that govern every answer now
>
> - **The RHS is a lab, not an illustration** (`docs/vision.md`). Prose runs only to the first
>   **prediction**, then he tests it on screen. **The threshold is a prediction, not
>   understanding** — a crude falsifiable guess beats a complete explanation.
> - **Beginner depth: concepts now, details later** (`docs/working-with-doug.md`). Detail costs
>   Claude nothing and costs Doug attention, so the pressure always runs one way. Premature
>   detail goes to `docs/compiler-phases/`, not into the answer.
> - **Neither the tours nor the UI are fundamental — the mathematics as Rumoca implements it
>   is.** So a mismatch may be fixed by changing the *pane*, and on 2026-08-13 one was. But
>   **labels must expose Rumoca's structure, not a pedagogically convenient one**; when prose
>   and pane disagree, **Rumoca is the arbiter**.
>
> ### CLAUDE CAN NOW READ THE PANE — use it before asking Doug to describe anything
>
> **`hrw/.hrw-bridge/view.json` holds the view on screen**, as the renderer's own input.
> Published for **Flatten → EquationSheet** and **Structural → Incidence**; a view with no
> publisher removes the file, so absence is honest. `diagnostics/session.json` names the current
> `sub_view`. Every row carries `id` (`f_x[N]`) so *"this equation"* resolves across panes.
>
> **Adding a view is: give its data type a `to_bridge_json`, then add one arm in
> `App::publish_current_view`.** The unpublished pane that matters most is **Flatten →
> Connections**, which is the only one that shows connection sets and therefore the only
> unverifiable claim left in `connect-expansion.md` Act 1.
>
> **A tour's group table is machine-checked** by
> `doc_citations::tour_group_tables_match_the_real_equation_sheet` (slow-gated), against a real
> compile. Mark the table `<!-- pane-groups -->`.
>
> **A MARKER'S REGION IS THE TABLE THAT FOLLOWS IT, AND NOTHING ELSE** *(bounded 2026-08-22)*. The
> scan used to run forward unbounded to the next backticked row, so **deleting a table while
> leaving its marker made that marker silently adopt the next table in the file** — `pane-groups`
> comparing against `pane-origins` rows. A missing *marker* always failed loudly; a missing *table*
> did not. `a_marker_whose_table_is_gone_does_not_adopt_a_later_one` is the guard.
>
> ### What a walk still cannot be replaced by
>
> Claude verifies **content, never pixels**. Whether a layout is legible or an animation reads as
> a search is Doug's report and nothing else. Four of the six mismatches he found on 2026-08-13
> are now the kind a test catches; **the other two were conceptual** — a tour whose central idea
> had no counterpart on screen — and no test closes that.
>
> ## WHERE INDEX REDUCTION STANDS (2026-08-18) — read before touching that tour or tab
>
> **Doug reported the tour was "way, way too short", and following that found a defect at each of
> four levels — all now fixed.** A false headline claim (`Drivetrain` differentiates **six** times,
> not zero); no cross-check of a stage summary against its own frames (now
> `a_reduction_summary_never_claims_more_than_its_frames_recorded`); a tab that **re-executed rather
> than observed**, since replaced by Rumoca's `prepare_dae_for_structural_analysis_fully_observed`;
> and **Rumoca not reducing the canonical index-3 DAE — adjudicated 2026-08-22**
> (`docs/upstream-issues.md`, `docs/ideas.md` #83).
>
> **The corpus spans the phase**: `BouncingBall` (nothing needed), `BenchActuator` (1
> differentiation), `Drivetrain` (6, at 97 equations), `CartesianPendulum` (cannot be reduced).
> Smallest-first is the tour's spine and the pendulum is its ending. The pane publishes
> `n_differentiations`, so a funnel that did nothing now says so.
>
> ## Open questions a walk may hit
>
> - **A reproduced state-count inconsistency**, in `docs/upstream-issues.md`: `Drivetrain`'s index
>   reduction demotes nine states to three while solve lowering reports **9**, and
>   `GearWithBrake` shows the same gap. **Not diagnosed.** `solve-lowering.md` omits its natural
>   example rather than write around it. Needs a System Modeler adjudication (`#43`).
> - **`RcCircuit` reports one `zero_crossing_condition`** with no `when` clause at all.
>   `events.md` Act 1 quotes only the four counts that are explicable.
> - **`#77`** — a live tour needs three panes and the layout has two. **Largely resolved 2026-08-12**
>   (`docs/ideas.md` #77, `DECISIONS.md`): three defects, each one number or one call — `DEFAULT_ZOOM`
>   2.0 → 1.0, an absolute `MIN_LEFT_POINTS` floor, and the tour pane's scroll axis. Doug: *"Finally,
>   HRW is usable on my 13" screen."* Expect tours to be **taller** now, which is the correct trade.
>   **What survives is only the genuine three-pane case** — HRW at half width beside VS Code — so
>   `matching-live.md` alone may still want a layout change; do not build one for the other eight.
>
> ## The debugger facts that were expensive to learn
>
> - **`cppvsdbg` will not re-bind a breakpoint at a location whose breakpoint left the adapter's
>   active set during a session** — by removal *or* by being disabled. Only a new debug session
>   recovers it. (`#74`)
> - **VS Code exposes no `verified` field to extensions**, so `breakpointPresent` means *"an
>   enabled breakpoint exists"* and can never mean *"execution will stop"*. (`#75`)
> - **To read a debugger stop, read `.hrw-bridge/debug-state.json`** — check `writtenAtMs` and
>   `seq` first, and skip `[len]`, `[capacity]`, `[Raw View]`. (`#72`)
>

---

## Running things

**FIRST THING ON A MACHINE YOU HAVE NOT RUN ON BEFORE — RUN THE MACHINE CHECK.** Claude runs it
unprompted: Doug switches machines twice a week, feels every cost, and cannot see any of these
coming, while Claude can.

```text
cargo run -p hrw --example check_machine
```

It verifies what does **not** travel with a `git pull`: the **permission allowlist** (gitignored by
upstream — and *fatal to an unattended run*, since a prompt with nobody awake is indistinguishable
from a hang), whether HRW is holding `hrw.exe`, the parsed-artifact cache, and the bridge
extension. Blocking problems exit non-zero and name their fix.

**ITERATING AND GATING ARE DIFFERENT ACTS, AND CONFLATING THEM COST DOUG TWO HOURS**
*(measured 2026-08-15, from the session transcript)*. Of 274 minutes of compute that day,
**172 went to `--features slow-tests` across 61 invocations** — 63 % of all waiting, for
**six** commits. The gate itself is not the problem; running it thirty-odd times is.

```text
# ITERATE — while editing, and for every must-fire revert-and-check. ~10s.
cargo test -p hrw --lib --features slow-tests -- --test-threads=1 <name-filter>

# GATE — ONCE, immediately before the commit. ~225s.
cargo test -p hrw --lib --test msl_resolve --features slow-tests -- --test-threads=1
cargo clippy -p hrw --all-targets                  # covers the BIN; check the exit code

# The fast suite, when nothing slow-gated is in play. ~15s.
cargo test -p hrw --lib -- --test-threads=1
```

**`--lib` ALONE SILENTLY SKIPS `tests/`, AND IT DID SO FOR AT LEAST ELEVEN DAYS**
*(found 2026-08-16, when Doug asked whether every checker runs in the gate)*.
`tests/msl_resolve.rs` — two tests proving the MSL dependency-loading path resolves
`Modelica.*` references end to end — had not run in any pre-commit gate since at least
2026-08-05. It passes, and costs **6.3 s**. Nothing was broken; nothing would have said so
either.

**Every target the gate names must be spelled out, because `--lib` is a filter and a filter
is silent about what it removed.** `doc_citations::every_test_target_runs_in_the_documented_gate`
now fails if a file appears in `tests/` that this command does not name.

**Deliberately still outside the gate**, and these are choices rather than oversights:

| what | why |
|---|---|
| `--features notebook-check` | reloads the MSL 21 times (157 s) and is order-dependent; see the third-gate note above |
| `examples/fidelity_msl`, `examples/survey_msl` | hours-long corpus sweeps with their own runbook and watchdog |
| doc-tests | `cargo test -p hrw --doc` runs **0** tests; there is nothing there to lose |

**The filtered line was missing from this file until 2026-08-15, and its absence is the whole
story.** The gate table below answers *"which gate before I commit?"* and returns **FULL** for
any `src/` change — correctly. But it was the **only** decision procedure written down, so it
got applied after every edit as well, and there was no sanctioned cheap option to reach for
instead. A rule that is right for its own question becomes wrong when it is the only rule
present. **Switching feature sets is not why**: measured at 1–2 s, cargo keeps both variants.

**ANNOUNCE THE COST BEFORE PAYING IT.** Before any command expected to exceed ~60 s, say what
it is and roughly what it costs, so Doug can redirect *before* the wait rather than discover it.
**This is the only item here that addresses cause rather than symptom**, because nothing else in
Claude's loop has a clock: correctness in this repository has must-fire, non-vacuity guards,
ratchets and a dozen checkers, and elapsed time has no mechanism whatever. Doug feels the cost
and cannot see what is about to run; Claude can see it and does not feel it — the same asymmetry
this file already records for the permission allowlist.

**AND THE LAST STEP BEFORE PUSHING IS THE HANDOFF — Claude does this unprompted** *(added
2026-08-19, after Doug had to ask three times)*.

**Ask one question before every push: does a fresh session need to know something it would not
learn from the diff?** A finding, a decision, a correction, or work left owed. If yes, update the
handoff box in *this* commit. If no — most commits — do nothing and move on.

**It is standing authorisation, not a request to be granted.** This file already says *"Claude
never needs permission for context maintenance"*, and Doug still had to prompt for it on
2026-08-18 and twice on 2026-08-19: *"It seems that you should be performing context maintenance
automatically. Are you not able to do that?"*

**The cause, so the fix targets it.** Maintenance was being treated as a **task** — something
done when asked — rather than a **step** in a sequence that already runs every time. And it
competes for context budget against the work, which is exactly backwards: **the work is what gets
lost without it.** A session that runs out of context having shipped code and no handoff has
spent its budget on the half that a `git log` could partly reconstruct, and skipped the half that
nothing can.

**This is the same asymmetry the permission allowlist and the cost-announcement rules record:**
Claude can see the need and does not feel the cost; Doug feels it and cannot see the need.
Whenever that shape appears, the mechanism belongs on Claude's side.

**The pre-commit order is FMT, then GENERATE, then GATE — and it is an order, not a set.**
`docs/architecture.md` carries module **line counts** derived from the source, so:

```text
cargo run -p hrw --example gate        # fmt -> generate -> lint -> test, in that order
```

**RUN THE TOOL, NOT THE SEQUENCE — added 2026-08-23, after this order was got wrong for the
TENTH time.** It cost the whole gate four times before that session and **six times during it**,
always the same way: `clippy` and the gate run straight after `cargo fmt`, so the rewrapped source
no longer matches the line counts `architecture.md` carries, and `architecture_regions_are_current`
fails at the end of a 230-second run. **Ten instances is not a memory problem**, and this
repository's own answer to a rule that keeps being got wrong is to give it a mechanism.

`gate` picks FAST or FULL from the working tree by the rule below, runs **all four** generators
(the matching reference included, which the hand-written list omitted), adds `fmt` **and** `clippy`
for any `crates/rumoca-*` package the change touches, stops at the first failure naming what that
step protects, and refuses to start while HRW holds `hrw.exe`. `--fast` / `--full` override the
verdict. The decision itself is `gate_policy`, in the library so it is reachable by a test —
choosing FULL needlessly costs four minutes and is obvious, while choosing FAST wrongly is silent.

`gen_field_help` is still outside it, deliberately: `build.rs` runs it.

**The gate fails while HRW is running** — `Access is denied` on `hrw.exe`. Doug builds from the
tree and keeps the app open, so this is normal, and **not always transient**: once a preceding
`clippy --all-targets` has invalidated the binary's fingerprint, it retries forever. **Split it and
both halves run with the app open** — selecting one target at a time does not pull in the bin:

```text
cargo test -p hrw --lib --features slow-tests -- --test-threads=1
cargo test -p hrw --test msl_resolve --features slow-tests -- --test-threads=1
```

**With HRW closed the combined line works** (measured 2026-08-22: 823 + 2, 128 s), which is why an
unattended run requires it closed — see [`docs/unattended-runs.md`](docs/unattended-runs.md), **read
that before doing any work while Doug is asleep.**

**`.hrw-bridge/tour.md` IS LIVE STATE, AND TESTS THAT PAINT MUST HOLD IT** *(2026-08-16, three
defects in one hour)*. It is Claude's answer to Doug's last question, and `tour::poll`
**auto-selects it** when nothing else is chosen — which resets the stage side. So its mere
presence changes what a painted frame does, and the suite had never run while one existed.

All three failure modes appeared the first time one did: a test that *asserted* the file was
absent (failing whenever the feature had been used), a test that wrote its own and **deleted**
Doug's afterwards, and a test that painted against whatever was on disk. Use
`ui_tests::AdHocTour::absent()` or `::with(text)`; both restore what was there, including on a
panic.

**A THIRD GATE EXISTS AND IS NOT IN EITHER OF THOSE — the notebook content check** *(added
2026-08-15)*. The committed specimen traces had been stale for **25 days** and nothing could
notice; `manifest_stage_rosters_match_the_pipeline` now catches a stage appearing or
disappearing for free, but only this catches *contents* drifting:

```text
cargo test -p hrw --lib --features notebook-check -- --test-threads=1 the_committed_notebook
cargo run -p hrw --example gen_trace -- --all      # 3m45s, the fix when it fails
```

**Run it after touching a `*_to_json` writer and after rebasing on upstream** — the same two
triggers the large fidelity sweep carries. It costs **109 s** (was 157 s until the parsed-source-root
memo landed 2026-08-21) and has its own feature because it must give each specimen a **fresh**
`WorkerState`: against the shared worker it is *order-dependent*, passing alone and failing in
company.

**IT IS ALSO THE BENCHMARK FOR ANY MSL-LOADING CHANGE, and that is worth knowing before reaching
for the gate.** It performs **21 MSL loads** by construction, so a per-load saving shows up 21
times against very little else — the memo's 48 s was predicted from counters at 49 s and measured
here at 48 s. The gate cannot do this: it ran **240 s, 287 s and 10,780 s** on 2026-08-21 with no
source change between them.

**AND IT IS A FIDELITY CHECK, not only a staleness check.** It compares every specimen's committed
per-stage IR against a fresh compile, so it is the instrument for *"did this change what Rumoca
produces?"* — the question a green gate cannot answer, since the gate was green before the change
too. Run it for any change to the compile or library-loading path, not just to a `*_to_json` writer.

**That order-dependence is a fact about the notebook, not about the test, and it belongs
here rather than in a test comment.** A committed trace is one sample of a function whose
hidden argument is the session — `gen_trace` runs one process per specimen, so what is
committed is the *virgin-session* value. Two specimens (`GearWithBrake`,
`MissingComponentClass`) demonstrably emit different JSON depending on what the session already
holds. So **"the trace is correct by construction" is true only of a virgin session**, and any
future code that reproduces a trace must reproduce that condition — including how the specimen
path is *spelled*, since `parse_to_ast` stamps it into every `Location` and a `\` for a `/` made
109 of 109 files look like total drift.

**AND THE DRIVE LETTER'S CASE IS PART OF THAT SPELLING** *(2026-08-17)*. `CARGO_MANIFEST_DIR` is
not stable in it: the committed traces carry `C:\Users\…`, and the same command in the same
directory later produced `c:\Users\…` — from both git-bash and PowerShell, so it is cargo's
resolution and not the shell's. A newly added specimen's four AST stages therefore differed from a
fresh compile and failed the notebook gate, on a difference that carries no information.

`gen_trace` now uppercases the drive letter before handing the path in
(`uppercase_drive_letter`). **That is canonicalisation, not editing what the compiler said** —
`c:\` and `C:\` name the same file, and choosing the spelling we hand in is the same deliberate act
as `worker.rs` handing in the full document URI. Rewriting the path to be *repo-relative* is a
different thing, changes which file the string names, and was rejected on 2026-08-16.

**MATCH THE GATE TO THE CHANGE — and let the diff decide, not judgement** *(measured
2026-08-15, after Doug reported test latency as genuine friction)*. Most commits in a walking
session touch only documents, and a docs-only change **cannot regress compile-heavy behaviour**,
so paying 225 s for it is ritual rather than evidence. On 2026-08-15 that ritual cost about ten
minutes across one session.

**This decides what to run BEFORE A COMMIT. It is not the answer to "what do I run after this
edit"** — that is the filtered line above. The distinction is written down because its absence
turned a latency fix into a latency cost on the day it landed: keyed to `--cached`, the table
speaks about staged changes, but being the only procedure in the file it got applied to every
iteration, returning FULL for the `src/` work that filled the day.

```bash
git diff --cached --name-only | grep -qE '(^|/)(src|crates|examples)/|Cargo\.toml' && echo FULL || echo FAST
```

| verdict | run |
|---|---|
| **FAST** — docs, tours, notebooks only | `cargo test -p hrw --lib -- --test-threads=1` **and** the doc checks below |
| **FULL** — any `src/`, `crates/`, `examples/` or `Cargo.toml` | the slow-tests line, plus clippy |

**FAST is not "skip the tests"** — the doc and tour checkers are exactly what a docs-only change
*can* break, and they are the cheap ones:

```text
cargo test -p hrw --lib doc_citations -- --test-threads=1   # ~1.4s
cargo test -p hrw --lib tour          -- --test-threads=1   # ~0.3s
```

**The one exception, and it is not optional:** a tour's `<!-- pane-groups -->` /
`pane-origins` / `pane-frames` tables are checked by *slow* tests, because verifying them needs a
real compile. **Editing one of those tables means FULL**, whatever the grep says — the diff
touches only `docs/`, and the check that guards it does not run in the fast suite.

**Where the ~225 s actually goes**, so nobody re-derives it: about twenty tests carry ~129 s of
it, led by `all_healthy_specimens_simulate` (16 s), `every_stage_serializes_without_panicking`
(15 s) and `a_rumoca_failure_is_represented_faithfully` (14 s). A rebuild after touching one file
is **8 s**; a no-op build is **1 s**. It was ~190 s until 2026-08-15; the gate grows as tests are
added, which is another reason to filter while iterating rather than treat it as a fixed price.

**WHERE IT REALLY GOES, measured 2026-08-21** (`docs/ideas.md` #48, which supersedes the
per-test figures above): **72 compiles at ~3.4 s and 10 MSL loads at ~4.4 s are 92 % of the run.**
Each compile re-resolves the whole MSL, so a two-equation specimen costs 3.5 s and the same file
with no MSL loaded costs 0.03 s. **The per-test table above ranks symptoms; this ranks causes.**

**FIVE levers already ruled out by measurement — do not re-propose them.** Cutting `t_end` is the
fifth (**0.4 s** — integration is free; `simulate` averages *less* than `compile_target`).
Parallelism buys about
two seconds, because the worker tests serialise on a global `Mutex<WorkerState>` regardless
(`docs/ideas.md` #48). **Memoising simulations buys about two seconds**: a simulation's key
must include `t_end`, and the sites are almost all distinct pairs —
`all_healthy_specimens_simulate` is nine *different* specimens at one `t_end`, so there is
nothing to reuse. Claude proposed that on 2026-08-15 from a sum of slow-looking tests and
withdrew it on measuring, which is the same mistake #48 already records once.
**Memoising specimen *compiles* is already built** — `compile_specimen_shared` caches, so 47 of
the 59 call sites are already free; #48's remaining title is misleading. And **feature-set
thrashing is not a cost**: alternating `--features slow-tests` with the plain suite was measured
at **1–2 s**, because cargo keeps both variants. Claude was one sentence from proposing a
practice change on that theory before measuring it.

**The pattern in all four: a sum of slow-looking names is not a measurement.** Three of these
were proposed from arithmetic over test names and died on contact with a clock. Measure the
thing, then decide.

**`--test-threads=1` is required, and since 2026-08-20 it is the DEFAULT** — two pre-existing
tests race on process-global stdout and on `focus.json`, and the suite **deadlocks** under the
default harness on a clean tree. `.cargo/config.toml` at the **workspace root** now sets
`RUST_TEST_THREADS = "1"`, so a bare `cargo test` is correct too. The commands below keep the
explicit flag: it is free, it survives someone running from a directory whose config differs, and
it documents the requirement where the reader is.

**Recognising it costs fifteen seconds, and output will not tell you.** `OutputCapture` owns
fd 1, so a hung run **stops printing which test it is on** — the last line is whichever test
flushed, not the culprit, and it will accuse an innocent one. Check the process instead:
**frozen CPU time with every thread in `Wait`** is hung; accumulating CPU is merely slow. It was
misread as slow for ninety minutes on 2026-08-20.

**`cargo test` does not build the binary, and that gap is not theoretical.** On 2026-07-31 a
`#[cfg(test)]` was placed above the first of three lifted helpers — **the attribute applies to
one item** — so two compiled into `--bin hrw` referencing test-only imports. Every test passed;
**Doug's debugger launch failed.** `cargo clippy --all-targets` covers the bin, **but check its
exit code, not its output**: the same breakage survived a clippy run piped to
`grep -c "^warning: "`, which counts warnings and silently ignores a compile error.

**Use the fast suite between edits and the full one before every commit.** Measured 2026-07-29:
49 of 402 tests took 180 of the suite's 183 seconds, nearly all compiling a specimen against the
MSL. They are gated by `slow-tests` and reported as ignored *with a reason*. **Parallelism is
not the fix** — they serialize on a global `Mutex<WorkerState>` regardless, saving about two
seconds; `docs/ideas.md` **#48** (memoize compiled specimens) is.

**Long runs → [`docs/long-runs.md`](docs/long-runs.md)**, the runbook for the MSL survey and the
fidelity sweep: copy-paste commands, what to watch, how to resume, what each abort verdict
means. *Why* each precaution exists is `docs/architecture.md` §11.

**NEVER run the fidelity sweep unbounded, and never in a bare loop.** An unbounded 53-model run
made Doug's machine unusable and forced a hard power-cycle (2026-07-31). Use the watchdog:

```powershell
# stop rust-analyzer FIRST via Ctrl+Shift+P -> "rust-analyzer: Stop server"
cargo build -p hrw --release --example fidelity_msl
.\scripts\measure-fidelity.ps1 -ModelsFile "C:\tmp\all-models.txt" -Out "C:\Users\dougd\rumoca-runs\fid-full.csv" -Profile "C:\Users\dougd\rumoca-runs\fid-full-memory.csv"
```

**One line per command — no backtick continuations** *(2026-08-04)*. A trailing backtick is
PowerShell's line continuation and **does not survive a paste** out of a chat window or most
editors. When it is lost, the first line runs alone and the script starts with **every argument
at its default**, silently — no error, no warning. Observed: `-Out`/`-Profile` fell back to a
previous run's files and the script announced **3 models to process** instead of 2,626.
**`--release` is likewise not optional**: the script runs `target/release/...` and a dev build
leaves a stale release binary in place. Both are in
[`docs/long-runs.md`](docs/long-runs.md) with the full account.

**One model per process**, so the worst case is bounded by a single model. **A session rebuild
is not a memory bound** — it releases what the session holds, not what the allocator
fragmented. **Only process exit is.** The watchdog guards on **free RAM** (3 GB floor), not
process size, sampled during the run: Doug proposed a 30 GB ceiling on a 31.7 GB machine, and
*a guard that cannot fire is indistinguishable from no guard*.

- Long runs go in a **standalone terminal**, not VS Code's.
- Output goes to `C:\Users\dougd\rumoca-runs\`, **never `C:\tmp`**, and is promoted into `docs/`
  by `cargo run -p hrw --example promote_run`, which writes the provenance sidecar.
- **Do not rebuild an example while a run holds its binary.**
- **Stop rust-analyzer first** — it holds ~5.7 GB here. **Do not kill the process**; VS Code
  treats that as a crash and restarts it within seconds.

**THE OWED SWEEP IS DONE** — run 2026-08-02, promoted 2026-08-03. Trigger 3 had fired twice:
the Parse stage of a library model went from `{"classes":{},"within":null}` to its declaring
file's full AST, and every `Location` in it gained the document URI.

**Result: 2,614 `ok`, 0 violations, 0 failed checks, 12 not checked** (3 free-RAM, 2
proc-ceiling, 7 timeout — all limits of this machine, not findings).

**And this zero counts, where the previous one did not.** Four F-checks walk
`StageKind::COMPILATION`, which begins with Parse, so they ran on every model before too — and
found nothing because there was nothing there. F7 samples up to 400 paths per stage and was
getting about *two* from an empty AST. It now walks a real one. Mean peak memory rose
1,228 → 1,353 MB, which is the ASTs being built.

**What it establishes**: HRW's path grammar round-trips over real Modelica ASTs at corpus
scale. **What it does not**: that HRW's AST equals Rumoca's — nothing in the sweep compares
them. That equivalence is `worker::tests::hrw_reparse_of_a_library_file_matches_the_sessions_own_ast`,
over 120 documents. **Representation is verified at corpus scale; equivalence at sample scale.**

**AND IT SAYS NOTHING ABOUT WHAT THE COMPILER DID** *(added 2026-08-04, after the fictions)*.
Every F-check asks about a **noun**: is this structure what Rumoca produced? The claims HRW
makes about **verbs** — which phase ran, in what order, nested inside what, how long it took,
what it declined to do, and whether a view came from the compile or from HRW re-running the
algorithm — **are outside the programme entirely.** That is not a flaw in the checks; it is
their scope. The flaw was reading a corpus-scale green as an answer to "is HRW faithful?"
**Whenever this file, a commit message or a report cites the sweep as evidence of fidelity,
the noun/verb split must be stated with it.**

**When the fidelity checks run** (policy 2026-07-31; reasoning in
[`docs/fidelity-plan.md`](docs/fidelity-plan.md)). Small scale — the 16 curated specimens —
**stays in the pre-commit run** (~90 s), answering *"did HRW drift from itself?"*, which is not
a rare event: both bugs found on 2026-07-31 were HRW-internal drift, weeks old. Large scale gets
its own feature gate. Run the large suite: **(1)** after rebasing on upstream (a step in
`docs/updating-rumoca.md`); **(2)** before submitting a PR to CogniPilot/rumoca; **(3)** when
HRW changes how it **emits or reads stage JSON** — the `*_to_json` functions in `worker.rs`, the
path grammar in `bridge.rs`, `IncidenceMatrix::from_report`, or any animation's re-derivation.
**Trigger 3 is code-shaped, not judgement-shaped, deliberately** — "when a change gives reason
to doubt fidelity" is exactly the judgement that already failed twice.

**BEFORE THE NEXT LARGE RUN, BUILD `docs/ideas.md` #46** *(Doug, 2026-08-05)* — a failure
specimen per compiler phase. **The 2026-08-04 run measured why**: 0 of 2,614 rows carried a
failure message and no MSL model produces an empty stage, so **F10's absence clause had nothing
to act on** and its zero covers only the two near-tautological clauses. Absence is a property of
*failing* compiles and the corpus has none. Another 8.5-hour sweep of the same corpus would
re-confirm the same narrow zero; #46 is what turns it into coverage. See
[`docs/fidelity-plan.md`](docs/fidelity-plan.md), "F10's first corpus run".

### A `crates/` CHANGE COSTS DOUG ONE FULL MSL RE-PARSE ON HIS NEXT LAUNCH — 2026-08-22

**This is about the running app, not the test gate, and it is the answer to "HRW felt broken
today".** Doug rebuilt HRW after a two-week gap on the other machine, and **his first in-app
compile took minutes while every later one was quick.** Diagnosed, then confirmed by his restart:
first compile fast again.

**Three costs, three lifetimes — and the MSL is never *compiled*.** It is parsed and resolved;
only the specimen goes through the phase pipeline. Conflating those three is what makes the
behaviour look mysterious:

| work | lifetime |
|---|---|
| **parse** MSL text → ASTs | **on disk, indefinitely** — `%LOCALAPPDATA%\Rumoca\source-roots\parsed-files` |
| **load** ASTs into the `Session` | until the HRW process exits |
| **resolve** names → DefIds (38,855) | until the next compile — `compile_target` invalidates it every call |

**So HRW reloads the MSL every launch and re-resolves it every compile; what it does not do is
re-parse it** — that result is cached on disk (218,558 files on Doug's machine, since 2026-07-29).
The cost lands on the *first compile* rather than at launch because `App::new` sends
`SetLibraries` and **the worker is one thread processing messages in order**, so an early compile
queues behind the library load.

**THE INVALIDATION RULE, which is the part worth remembering.** The artifact cache key is
`blake3(schema ‖ compiler ‖ file_name ‖ source_hash)`, and `compiler_source_fingerprint()`
(`rumoca-compile/src/source_root_cache.rs`) hashes **the entire `crates/` tree** plus the
workspace `Cargo.toml`, `Cargo.lock` and `rust-toolchain.toml`. **Any change anywhere under
`crates/` invalidates every cached parsed file at once.**

- **A `crates/rumoca-*` edit therefore has a cost this file did not previously price**: beside
  clippy, fmt and upstreamability, it buys Doug **one full MSL re-parse on his next launch.**
  Still cheap against what instrumentation buys — but say so when proposing one, per
  *ANNOUNCE THE COST BEFORE PAYING IT*, since he pays this one and cannot see it coming.
- **`hrw/` edits are FREE.** `hrw/` is outside `crates/`, so ordinary HRW work never invalidates
  it. **Adding a dependency does**, by moving `Cargo.lock`.
- **The tell, when Doug reports a slow launch:** check the mtime of
  `%LOCALAPPDATA%\Rumoca\source-roots\parsed-files`. Recent means it was being *written*, which
  only happens on a miss. `semantic-summaries` is a separate layer with its own lifetime.

**Do not run `du` or `ls -lt` on that directory** — 218k entries, and both timed out at two
minutes while diagnosing this. `ls -ld` on the directory answers the question in milliseconds.

---

## Where things live

- **Rumoca source** — HRW lives **inside** the fork; read the sibling `../crates/...` directly.
  It is the exact tree HRW builds against, no Cargo-cache indirection. "Updating Rumoca" means
  **rebasing the `hrw` branch on upstream**, per
  [`docs/updating-rumoca.md`](docs/updating-rumoca.md).
- **[`docs/compiler-phases/`](docs/compiler-phases/) — Claude's teaching database.** **Audience
  is Claude, not Doug**, who reads it only indirectly through answers; Claude maintains and
  commits it. Start at
  [`the-chain-of-problems.md`](docs/compiler-phases/the-chain-of-problems.md). **What goes in:**
  Doug's *questions*, the confusion behind them, and what made a thing click — **not** Claude's
  explanations, which are regenerable and build an echo chamber a later session mistakes for
  fact. **Every claim carries provenance** ([`docs/provenance.md`](docs/provenance.md));
  untagged prose is a **lead, not a fact**, and upgrades lazily when a real question sends
  Claude into the source. Two tests in `src/doc_citations.rs` check that cited paths exist.
- **[`docs/question-ledger.md`](docs/question-ledger.md)** — Doug's questions verbatim and what
  made each click. **Scan it before answering in a familiar area.** A repeat branches two ways
  demanding opposite responses: the concept is hard (try a different angle), or the thing is not
  visible in HRW (a feature request, better than any Claude invents).
- **[`docs/upstream-strategy.md`](docs/upstream-strategy.md)** — **consult when planning work**,
  not only when preparing something to send. Engagement is a **means** to Doug's education, so
  questions must be worth answering: *"here are 380 MSL models failing at flatten with the same
  error shape — expected?"* beats *"why does X?"*. **Order deliverables by their cost to accept,
  not our effort** — bug reports with System Modeler adjudication, an MSL capability map and
  differential testing are zero-cost gifts; **HRW goes last**, being the only item asking for
  maintenance burden. Anything published must be **reproducible** and **honestly bounded**.
- **[`docs/upstream-issues.md`](docs/upstream-issues.md)** — **Claude adds entries and never
  files them.** Only *reproduced* bugs, with suspect code marked unverified: a confident wrong
  diagnosis wastes a maintainer's time and costs the credibility this project is building.
- **[`docs/fixture-tours/`](docs/fixture-tours/) — tours that are *tests*, not explanations.**
  Versioned, unlike an ad hoc tour (`.hrw-bridge/tour.md`, gitignored). **Only justified because
  something runs them:** `fixture_tour_links_all_resolve` parses every link on every test run.
  Three rules, each bought with a defect:
  - **One tour per capability, narrow.** The scarce resource is **Doug's attention per
    expectation**, not his walks; a wide tour consumes the surplus that produced the off-stop
    findings (`docs/ideas.md` #49).
  - **An expectation must say WHERE to look.** Doug reported "nothing happened" at a stop that
    was correctly refused with the reason on screen — the tour never said notices live in the
    status bar.
  - **Every `**Expected:**` line must be violable.** "Mostly collapsed" where the truth is
    *fully* collapsed tests nothing, and hedged expectations teach Doug to read them loosely.
- **[`docs/specimen-notebook/`](docs/specimen-notebook/)** — per specimen: `trace/` (durable
  per-stage IR + manifest, from `cargo run --example gen_trace -- <Model>`, **generated and
  therefore correct by construction — any number about a specimen is read from here**) and
  `purpose.md` (why it exists; rendered as the Purpose tab).
- Architectural invariants are in Rumoca's numbered SPEC files; comments cite Modelica Language
  Specification sections. **Respect phase boundaries** — IR crates are pure data.

## Architecture rules (charter §4.4, Decision 6)

- Rumoca is linked **as a library**, via path deps on `../crates/rumoca-*`. **Never shell out to
  the Rumoca CLI.** A load-IR-from-JSON import path is retained as a secondary mode only.
- Compilation and simulation run on a **worker thread**, results returned over a channel. The
  egui `update()` loop never blocks and never calls the compiler or solver directly.
- Native builds only. No WASM, no web deployment (charter Decision 5).
- **One generic serde-value tree inspector** pointed at every stage's IR — not per-stage bespoke
  tree widgets. Graph and custom-painter views arrive in their own arcs.
- **New pipeline stages must be wired into ALL per-stage systems** — stage-diff highlight,
  stage-file publishing, and the notebook trace.

## Debugging conventions

The VS Code debugger is a first-class learning instrument: structure code so a breakpoint can be
set inside a Rumoca phase while it processes a specimen.

- Breakpoints belong in **actions** (button handlers, worker tasks), **never in the per-frame
  paint path.** Keep compile/simulate logic out of rendering code.
- `[profile.dev.package]`: keep full debug info on all Rumoca crates.
- Setup, launch config and failure signatures: [`docs/setup-windows.md`](docs/setup-windows.md).

**WHEN DOUG IS AT A BREAKPOINT, READ `hrw/.hrw-bridge/debug-state.json`** *(built 2026-08-08,
`docs/ideas.md` #72)*. **Claude cannot see a debug session** — #70 measured it: a stop yields no
location, no stack and no values, and no tool exposes them. Stopping does surface the *file* via
an `ide_opened_file` event, and a **selected** line arrives, but the running program's state does
not. So the bridge extension publishes it: stack frames, the innermost location, and the locals of
its most local scope.

**Three rules for reading it, and the first is not optional:**

- **CHECK `writtenAtMs` AND `seq` BEFORE BELIEVING ANY OF IT.** A payload from the *previous* step
  is indistinguishable from a current one by content alone, and describing the wrong state
  confidently is the exact failure this repository spends most of its rules on. Nothing deletes
  the file at shutdown, deliberately — so a stale file is the expected case, not an anomaly.
- **`variables: null` means NOT FETCHED**, with `variablesError` saying why. `[]` means fetched
  and empty. Never report the first as "no locals".
- **`frameCount` is the truth; `frames` may be capped** (`framesTruncated`). Depth matters:
  for `augment_traced` the stack **is** the augmenting path — N nested frames is an N-edge
  alternating path with each frame's `eq` a node on it.

**And do not substitute `breakpoint-request.json` for it.** That file holds the line HRW *asked*
to arm, which at the live-trace anchor coincides with where Doug is stopped — so answering from it
looks like working debugger vision until the day he stops somewhere else. #70 records this trap in
full; **right often enough to be trusted is the failure mode.**

## Specimen rules (charter §4.3, §4.1)

- Specimens live in `specimens/`, authored in Wolfram System Modeler, in the **portable Modelica
  subset** — no Wolfram extensions. Done = compiles and runs equivalently in both.
- **Every specimen carries a `// purpose:` comment** (one line, phenomenon-focused), plus a
  `docs/specimen-notebook/<Model>/` trace and `purpose.md`.
- **Scratch specimens live in `.hrw-bridge/specimens/`** — Claude writes them mid-conversation to
  answer a question; HRW lists them within a second, no restart. **Not** held to the rules above,
  and **ephemeral by construction**. **A scratch name may not shadow a curated one** — the
  collision is reported and the file skipped, because silently loading a different model than the
  name says would have Claude reason confidently about source Doug is not looking at.
- **No MSL MultiBody.** Mechanical components come from our own small planar library.
- Comparison protocol: identical solver tolerances and initial conditions, explicit `experiment`
  annotations; agreement metric = relative error on state trajectories and event-time deltas.
- **Prefer standards** — MSL and portable Modelica over custom implementations.

## Arc close-out ritual

An arc is done when: (1) the specimen passes the differential test in both toolchains; (2) the
arc's observatory pane renders the relevant IR; (3) Doug has single-stepped the phase in the
debugger on that specimen; (4) the trace log (IR before/after) is captured; (5) this file's
Current work section is advanced. *(Gates 1 and 3 are under review — treat as
satisfiable-by-acceptance; `docs/ideas.md` #4.)*
