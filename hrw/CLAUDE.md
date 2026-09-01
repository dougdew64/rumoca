# CLAUDE.md — HRW Observatory

**Purpose:** the rules that bind, what is being worked on now, and where everything else lives.
**Status:** authority. **The one file to read at session start.**
**Read when:** every session, first.

Rust/egui observatory for studying the Rumoca Modelica compiler.
**[`docs/README.md`](docs/README.md) is the document index** — every file, its purpose, and
whether it is live. Go there rather than guessing.

**The hierarchy everything else derives from** (charter v1.5): **Doug's education is the purpose**;
**accuracy is the first corollary** (Decision 7), because an inaccurate instrument teaches something
false; **low friction is the second** (Decision 9). Accuracy outranks friction where they conflict.

**SINCE v1.5, ACCURACY SERVES A SECOND END — CLAUDE'S OWN REASONING.** Doug, 2026-08-24: *"basing
decisions upon HRW accuracy and consistency benefits not only my learning experience, but also your
ability to reason about this project."* **So consistency is a test Claude applies too: would this
change improve or worsen my ability to reason about and maintain this code?** Uniformity of
*meaning*, never symmetry of shape, and subordinate to what it serves. The hazard — Claude is a poor
sensor for his own comprehension — and the worked examples are in `DECISIONS.md`, 2026-08-24.

**[`docs/CHARTER.md`](docs/CHARTER.md) holds purpose, scope and the binding decisions** — including
Decision 8, the instrument assumes the reasoner, which governs what UI gets built at all. **Consult
it for any design question; do not re-litigate settled decisions in-session.** Append any nontrivial
implementation choice to [`DECISIONS.md`](DECISIONS.md) with a one-line rationale.

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
  constraint on what HRW CAPTURES for Claude to reason from** — Doug's words are about *"context
  captures which will best enable you to provide high quality answers"*, and no file-size cap
  survives without re-justifying on what Claude needs to answer well. **It is not a licence for
  long prose aimed at Doug**, which the opposite rule governs: *answer the question asked, at the
  depth asked, and stop*. Detail costs Claude nothing and costs Doug attention, so the pressure
  always runs one way.

---

## The rules

**ACCURACY IS THE PRECONDITION OF EDUCATION, AND IT OUTRANKS EVERYTHING** — including features,
polish, performance and **the cost of a change to the Rumoca crates**. **This is charter Decision 7,
and the charter states it**: everything displayed must be traceable to something the compiler
actually did on the run observed; absence is stated rather than filled; a derived view declares
itself; a log's ordering and attribution are claims. **Doug's standing authorisation comes with it**
— *"we will pause and fix code as often as necessary"* — so a day spent removing fictions is not a
detour.

**Read Decision 7 rather than a summary of it.** What follows here is only what the charter does
*not* say, because it is operational rather than constitutional.

**A STAGE'S OUTCOME IS A CLAIM TOO: `Outcome::Failed` means "the pipeline stopped here", so at most
ONE stage per compile may carry it.** Four sites once painted whole runs of stages `Failed` for a
single stop; `the_corpus_outcome_matrix_is_unchanged` pins it now.

**"REPLAY" MEANS TWO THINGS, AND ONLY ONE IS FORBIDDEN.** Both senses are live in the code and the
UI, and confusing them has twice threatened a working feature:

- **Playback** — stepping frames *recorded during the real compile*. This is the animation feature
  and it is **correct**. `CompileFrames` holds them.
- **Re-execution presented as the compile** — running an algorithm again when a tab opens and
  drawing the result as if it were what the compiler did. This is the fiction, and it is gone.
- **A live debug session is neither.** `PendingLiveDebug::PreLowering` genuinely re-runs a phase
  because that is what the user asked for. It is correct and must not be "fixed".

**Judge by where the frames came from, never by the word.** Without this written down a later
session reads *"the reduction replay"* in a tour and either removes a working feature or concludes
the fictions were dealt with and stops looking.

**AND A LARGE GREEN RESULT COVERS THE TERRITORY IT MEASURED AND NO MORE, WHILE THE CONFIDENCE IT
PRODUCES DOES NOT KNOW THAT.** 2,614 green fidelity rows could not have caught one of the fictions,
because every check asked about a **noun** and every fiction was a **verb**.


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

**WHEN A NULL RESULT IS ABOUT TO BECOME "THIS CANNOT BE TESTED", CHECK WHETHER EVERY PROBE WAS
AIMED AT THE SAME LEVEL.** Three null results inside one widget became a property of the widget and
stopped anyone looking for eight days; a claim about the **pixels** was read as a claim about the
**panes**, and `matrix_panes.rs` had six tests available the whole time it was filed as untestable.
**A wrong *negative* is the error nobody catches, because acting on it means not looking.**

**Two scroll-area rules, each with a test that carries its own account:** a scroll axis is a claim
about how a widget negotiates size with its **parent**
(`ui_tests::the_left_panel_content_never_detaches_from_the_divider`), and **never nest a vertical
scroll area inside one** — the parent owns the scrolling and the height (`playback::tests_layout`).

**Both were reported by Doug, not by a test**, and neither is visible to `egui_kittest` — a clipped
child is still in the accessibility tree. **Layout is the surface where his report *is* the
verification.**


**A RULE IS ALSO A CLAIM ABOUT ITS SCOPE, and stating what it does NOT forbid costs one clause.**
This is the repository's most frequent failure — a statement true in one domain, applied in a wider
one — and every instance would have been safe with one such sentence. The template is proven:
*"REPLAY means two things, and only one is forbidden"* was written precisely so a later session
would not delete a working feature.

**And the interpretive half, for when a rule is already written badly: a policy blocking something
obviously valuable and zero-risk is evidence about the POLICY, not about the action.** Stop and
re-read the rule before abandoning the move.

**A QUALITY BAR CAN BECOME A DISCOURAGEMENT, and the fix is to price it out loud.** A rule shapes
behaviour by making one option feel illegitimate, whatever it says — the Rumoca instrumentation
checklist did it for weeks, until Doug priced it: *"it is much better to defend a rumoca api change
to the repo maintainers than to defend replays."* The work landed in two days. **It was unpriced,
not difficult.**


**INSERT A TEST AFTER A FUNCTION'S CLOSING BRACE, never before its `fn` line** — anything placed
between a doc comment and its item is adopted by the wrong one, and the old function silently
stops being a test. Bitten three times; the history and the mechanism are on
`doc_citations::no_function_has_two_test_attributes`, which catches it.

**A CHECKER RETIRES THE PROSE IT REPLACES** *(2026-08-22)*. When a rule becomes a test, the prose
here shrinks to **one sentence and a pointer at the test** — the reasoning belongs on that test's
doc comment, beside the code enforcing it, where it cannot drift. This paragraph paid for itself
that way: `no_function_has_two_test_attributes` and `claims_of_absence_are_still_true` already
carried their histories, so the copies here became pointers.

**GENERALISED 2026-08-31 — ONE HOME PER FACT, whether or not a checker exists.** Doug: *"when
attempting to make sense of all of these rules, you seem to struggle with conflicts."* Of seven
contradictions found that day, **none was a disagreement about what he wants** — five were stale
copies, two were dropped scopes. **The rules do not conflict; copies of them fall out of date.**
And spread predicts drift: *gate* was discussed in 7 of 7 governing documents and caused three of
the findings, while *never push* lives in one and has never drifted.

- **Compress by DE-DUPLICATION, never truncation.** A rule's single statement keeps its scope and
  its non-forbiddings — terse rules have failed here repeatedly. What goes is its *second* copy.
- **An ACTOR may state what it must do; it may not restate WHY the rule is what it is.**
  `unattended-runs.md` keeps what a night does at a ceiling and lost the derivation of the ceiling.
- **Counting mentions overstates duplication.** Three of four "ceiling" hits were unrelated senses.
  Only reading finds it, which is a limit on the nightly sweep, not a task for it.

**The forcing function is a mechanism, not goodwill** — a limit
(`doc_citations::the_mandatory_reading_path_stays_small`, stated in
[`docs/reading-budgets.txt`](docs/reading-budgets.txt)) plus a nightly sweep (`examples/doc_report`)
— **and a pointer must resolve** (`doc_citations::qualified_citations_resolve`), which is what makes
this safe against a rename.

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

**Do not sell a refactor on these** *(Doug pushed back on exactly that over-claim, 2026-08-02;
`worker.rs` is the next candidate it could be made about)*. A large file *pressures* Claude toward
generators, but the corruption is a habit that operates on small files too. A refactor's
justification is blast radius and testability, and it does not need the help.

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

**WHEN TO REFACTOR is charter Decision 12(b)** — Claude's comprehension, his ability to
maintain, or testability, and **never a line count.** The three complexity lints are declined
for that reason (`hrw/Cargo.toml` carries it).

### DEFAULT TO TEACHING, NOT TO BUILDING — a standing instruction, not a mood

**Doug, 2026-08-08:** *"I will finally begin a serious walk through the tours and try to shift our
conversation to be about my education rather than about HRW features."* And the reason, which
should not need saying twice: *"We've been working on this project for three weeks, and I have not
yet been rewarded with a learning experience."*

**When Doug reports something during a walk, the first question is *"what does this teach, and is
it true?"*** — not *"what should we build?"* A feature is warranted when it unblocks the learning;
[`docs/ideas.md`](docs/ideas.md) is where the rest goes. **Treat an hour of HRW polish during a
walk as a cost.**

**Neither the tours nor the UI are fundamental — the mathematics as Rumoca implements it is.** So
a mismatch may be fixed by changing the *pane*, and on 2026-08-13 one was. But **labels must
expose Rumoca's structure, not a pedagogically convenient one**; when prose and pane disagree,
**Rumoca is the arbiter.**

**When a 🎯 capture arrives, locate the passage in the file the capture names.** The emitted text
is what the pane *rendered*, so it will not match the markdown byte-for-byte.

**The rest is owned elsewhere and must not be restated here** — prose runs only to the first
prediction, and one tour at a time ([`docs/fixture-tours/README.md`](docs/fixture-tours/README.md));
concepts now, details later, and the two things Claude cannot judge
([`docs/working-with-doug.md`](docs/working-with-doug.md)).

### Two signals that a file has outgrown what Claude can hold

**State the trigger as "exceeds what Claude can hold, WITH DEFECTS TO SHOW FOR IT", never as
"large".** Size is the heuristic this policy exists to refuse: it would equally have licensed
splitting `worker.rs`, which was larger throughout the `app.rs` arc, caused none of the trouble,
and whose production code is still one file. The arc's record is
[`docs/app-split-plan.md`](docs/app-split-plan.md).

**Handoff frequency is the second signal, and it MEASURES rather than triggers.** A model change
and a file growing cannot be told apart by counting handoffs, so use it only across a stable model
and **say which model when recording a count.** **Claude has no reliable introspective access to
his own context size and must not estimate one** — that number belongs to the tooling.


**The human reader is Doug, and he named two scenarios** *(2026-08-05)* that pull in opposite
directions, so the policy is **two-tier**.

**Scenario 1 — he reads to understand and asks.** *"so long as you can answer my questions about
the code which you wrote, then all will be well."* **The rule this creates is on CLAUDE, not on the
code: always be able to answer.** Its consequence is that **the *why* must live in the repository,
not in a conversation that scrolls away** — a comment, `DECISIONS.md`, or a doc. Code whose
rationale exists only in chat violates this the moment the session ends, and it rules out
constructs whose behaviour Claude would have to guess at.

**Scenario 2 — he edits the visualizations himself.** *"I will likely make changes to the code by
myself and then request that you comprehend, improve and test the code which I've written."* Those
files are therefore held to **human** comprehension, Doug specifically — decades of C/C++/Java, new
to Rust and egui. The surface: **`canvas.rs`, `incidence_view.rs`, `matching_anim.rs`,
`tarjan_anim.rs`, `spyplot.rs`**, plus any new custom-painted view. Four rules follow:

- **The barrier is idiom, not length.** A linear sequence of paint calls is the *easy* kind of code
  for a C++ programmer; closures capturing state, iterator chains, `impl Trait` and borrow-checker
  dances around `&mut Ui` are not. **Prefer the plain form even when idiomatic Rust is terser**, and
  comment the egui idiom where it appears.
- **Keep geometry named and single-sourced** — `tarjan_anim::equation_world_pos` is the pattern, so
  drawing and camera-aiming cannot disagree about where a thing sits.
- **These are the least-testable files in the project**, and `egui_kittest` cannot reach
  `incidence_view.rs` cells or `spyplot.rs` — **exactly the code he will edit.** So **push logic out
  of the paint path into checkable data**, as `Plot::problems()` and `IncidenceMatrix::problems()`
  do. **When touching these files, move a computation out before adding one in.**
- **This binds as files are touched, never as a campaign.** He said *eventually*; building for it
  now would be speculation.

**The `crates/rumoca-*` instrumentation is not covered by any of this** and stays under
`[workspace.lints]`, complexity lints included, because it is offered to human maintainers
**now**. `docs/upstream-strategy.md` puts HRW itself last among deliverables, being the only one
asking for maintenance burden.

**THREE STANDING PROHIBITIONS ARE CHARTER DECISION 12**, not craft, and only Doug amends them:
**(a)** do not optimise HRW to widen test scope — and `worker.rs`'s compile path is extracted
*around*, never restructured; **(b)** refactor for Claude's comprehension, never a line count;
**(c)** the composition primitives are frozen. **Read the charter, not a summary.**

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

> ### ⟶ THE DOCUMENT REVIEW IS RUNNING — began 2026-09-01, **this file** first
>
> **Doug is questioning every paragraph**, beginning with whether it is still needed: *"it seems to
> be as much a historical log as a policy statement."* **He was right** — this file was 1,420 lines,
> of which `The rules` / `Current work` / `Running things` were 88 %.
>
> **The test is charter Decision 10's**, not *"is this old"*: does this paragraph **bind**, or is it
> a conclusion drawn from one case? A conclusion is *evidence for* a decision, never itself a rule.
> Decision 11 then routes whatever survives. **The finding that has recurred at every step: almost
> nothing was wrong or old — it was filed in the wrong place**, and *one home per fact* is violated
> *within* a single file as often as between two.
>
> **Claude is the wrong judge of what to cut** and should supply evidence, not verdicts: this
> history is the record of his own failures.
>
> **NEXT: tour rules that create friction**, then `docs/fixture-tours/README.md`. Nights 1-7 and
> what each found are in [`docs/unattended-run-log.md`](docs/unattended-run-log.md); the nightly
> document step is in [`docs/unattended-runs.md`](docs/unattended-runs.md), which owns both the
> *restore, never choose* boundary and the finding that document drift is **same-day** — so the
> risk window is hours, and a day of heavy mechanism change should end with a sweep.
>
> **OWED, and both wait for the next `src/` errand rather than buying a gate of their own** —
> [`docs/tech-debt.md`](docs/tech-debt.md), *"Owed sweeps"*. Doug ruled that on 2026-08-31.
>
> **This section holds ONLY what is in flight. Everything closed lives in
> [`DECISIONS.md`](DECISIONS.md)** — *"closed arcs move out of `CLAUDE.md`"*, 2026-08-22, which
> names each arc and the file that holds its record. **Do not restate a closed arc here; link it.**
>
> **The rule, and it is now enforced rather than remembered:** what is in flight goes at the top,
> standing context underneath, and **a ✅ box is history the moment its arc closes.**
> `doc_citations::the_mandatory_reading_path_stays_small` fails by name if this section or the
> reading path outgrows its limit. **The limits, the unit and what crossing one means live in
> [`docs/reading-budgets.txt`](docs/reading-budgets.txt) and nowhere else.**
>
> **THE TWO MODES DOUG WORKS IN — walking tours when he can focus, low-supervision work
> when he cannot — and the decision boundary that comes with them are in
> [`docs/working-with-doug.md`](docs/working-with-doug.md), under *Standing rules*.**
>
> ### ⟶ THE WALK IS THE MODE — Doug, 2026-08-21
>
> *"my hope going forward is to focus on improving tours, not feature code."* **The iteration
> loop, the two gate traps and the one-tour-at-a-time rule are in
> [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md)** — read before touching a tour.
>
> **⟶ WHERE THE WALK IS — `connect-expansion`, RE-WALKED, THEN `dae-construction`**
>
> **Doug, 2026-08-22:** *"I will walk tours in the same sequence as the compiler phases"* —
> `the-concepts.md`'s own numbering: **dae-construction → matching (→ matching-live) → blt-ordering
> → tearing → index-reduction → initialization → solve-lowering → events.**
>
> **`connect-expansion` IS BEING RE-WALKED FIRST**, ahead of `dae-construction`, rewritten
> 2026-08-30 under the provoke-questions rules and **again 2026-08-31 under code-grounding** —
> its opening walks `connections/mod.rs` and every code name is an `hrw://src` link. **Stops 1
> and 2 have not been re-walked against that opening**; expect the seams to show there first.
> **Stop 6 and `ScopedConnect` are BRAND NEW and have never been walked** — the specimen was
> authored 2026-08-31 to falsify Stop 1's "twice the node count" rule, and its numbers are
> machine-checked but its *prose* has had no reader.
> **NOTHING TRACKS WHICH TOURS HAVE BEEN WALKED**
> since 2026-08-31 — *"that bookkeeping doesn't yield enough value."* The `walked:` and
> `authored:` markers and both checkers are gone, with the you-do goal the second served. **Do not
> reintroduce either**; judge from the conversation, per `working-with-doug.md`'s table.
>
> **`index-reduction.md` is mid-walk and waits its turn at #6** — asked and ruled: *"I will walk
> the tours in compiler phase order."* **No exception for the tour already in progress.**
>
> **AND IT CARRIES A HARDER BAR**, staked in public: **index reduction explained to anybody with
> only basic calculus** — the bar is PREDICTION, not comprehension. That constraint, the
> provoke-questions rules and the one-tour-at-a-time rule are all in
> [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md); read it before touching a tour.
>
> **REFACTORING IS QUEUED, NOT IN FLIGHT: `app.rs` then `worker.rs`, with bug discovery as the
> stated goal** — Doug's standing order, 2026-08-21, unworked since. The seam heuristic, why the
> goal changes it, and the column-read audit are in
> [`docs/app-split-plan.md`](docs/app-split-plan.md) §3b.
>
> **THE ONE UNVERIFIABLE CLAIM LEFT IN `connect-expansion.md` Act 1** is there because **Flatten →
> Connections has no `view.json` publisher**, and it is the only pane that shows connection sets.
> Publishing it is: give its data type a `to_bridge_json`, then one arm in
> `App::publish_current_view`.
>
> ## WHERE INDEX REDUCTION STANDS — read before touching that tour or tab
>
> **The corpus spans the phase**: `BouncingBall` (nothing needed), `BenchActuator` (1
> differentiation), `Drivetrain` (6, at 97 equations), `CartesianPendulum` (not reduced by Rumoca).
> Smallest-first is the tour's spine and the pendulum is its ending. The pane publishes
> `n_differentiations`, so a funnel that did nothing now says so. **Rumoca not reducing the
> canonical index-3 DAE was adjudicated 2026-08-22** — System Modeler reduces it to two states by
> dynamic state selection, so the gap is evidenced rather than inferred, and Stop 5 and the
> upstream entry are unblocked. The run and the pre-committed outcomes are in
> [`docs/upstream-issues.md`](docs/upstream-issues.md); the follow-on work is `docs/ideas.md`
> **#83** (general Pantelides) and **#5** (the four-bar linkage).
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
>   (`docs/ideas.md` #77, `DECISIONS.md`); tours are **taller** now, which is the correct trade.
>   **What survives is only the genuine three-pane case** — HRW at half width beside VS Code — so
>   `matching-live.md` alone may still want a layout change; do not build one for the other eight.


---

## Running things

**The procedures are [`docs/running-things.md`](docs/running-things.md)** — gate commands, the
three suites and what each protects, the notebook content check, long-run safety, and the
diagnostic tells for a hung or slept run. **Follow it step by step rather than from memory.**

**What stays here, because it binds rather than instructs:**

- **The gate is green before every commit**, via the runner: `cargo run -p hrw --example gate`.
  It decides FAST, TOUR or FULL from the working tree; `gate_policy` is the rule and has tests.
- **ITERATING AND GATING ARE DIFFERENT ACTS.** Filter while editing; gate once, before the commit.
  Conflating them cost 172 of one day's 274 compute-minutes for six commits.
- **ANNOUNCE THE COST BEFORE PAYING IT** — before any command expected to exceed ~60 s, say what
  it is and roughly what it costs, so Doug can redirect before the wait rather than discover it.
  **Never quote a timing `examples/measure` did not produce, and never subtract two that it did.**
- **THE HANDOFF IS THE LAST STEP BEFORE A PUSH, and Claude does it unprompted.** One question:
  does a fresh session need something it would not learn from the diff? If yes, update the handoff
  box in *this* commit. **This is the same asymmetry as the cost rule** — Claude can see the need
  and does not feel the cost; Doug feels the cost and cannot see the need. **Whenever that shape
  appears, the mechanism belongs on Claude's side.**
- **A `crates/` change costs Doug one full MSL re-parse on his next launch**, because the artifact
  cache key hashes the whole tree. `hrw/` edits are free. Say so when proposing one.

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
  Three rules, each bought with a defect, and the README carries what each cost:
  - **One tour per capability, narrow** — the scarce resource is Doug's attention per
    expectation, not his walks (`docs/ideas.md` #49).
  - **An expectation must say WHERE to look** — he reported "nothing happened" at a stop
    correctly refused with the reason on screen, in the status bar the tour never named.
  - **Every `**Expected:**` line must be violable.** Hedged expectations teach him to read
    them loosely, and terser prose does not license looser claims.
- **[`docs/specimen-notebook/`](docs/specimen-notebook/)** — per specimen: `trace/` (durable
  per-stage IR + manifest, from `cargo run --example gen_trace -- <Model>`, **generated and
  therefore correct by construction — any number about a specimen is read from here**) and
  `purpose.md` (why it exists; rendered as the Purpose tab).
- Architectural invariants are in Rumoca's numbered SPEC files; comments cite Modelica Language
  Specification sections. **Respect phase boundaries** — IR crates are pure data.

## Architecture craft

*(What the charter settles — Rumoca linked as a library and never shelled out to, native builds
only, no WASM — is Decisions 4, 5 and 6. **Read them there.** What follows is craft.)*

- **The egui `update()` loop never blocks and never calls the compiler or solver directly.**
  Compilation and simulation run on a worker thread, results returned over a channel.
- **One generic serde-value tree inspector** pointed at every stage's IR — not per-stage bespoke
  tree widgets.
- **A new pipeline stage must be wired into ALL per-stage systems** — stage-diff highlight,
  stage-file publishing, and the notebook trace. Miss one and the stage is silently half-present.


## Debugging conventions

The VS Code debugger is a first-class learning instrument: structure code so a breakpoint can be
set inside a Rumoca phase while it processes a specimen.

- Breakpoints belong in **actions** (button handlers, worker tasks), **never in the per-frame
  paint path.** Keep compile/simulate logic out of rendering code.
- `[profile.dev.package]`: keep full debug info on all Rumoca crates.
- Setup, launch config and failure signatures: [`docs/setup-windows.md`](docs/setup-windows.md).

**CLAUDE CANNOT SEE A DEBUG SESSION.** A stop yields no location, no stack and no values to him,
so when Doug is at a breakpoint the state comes from `.hrw-bridge/debug-state.json`, which the
bridge extension publishes. **Check `writtenAtMs` and `seq` before believing any of it** — nothing
deletes the file at shutdown, so a stale payload is the expected case and is indistinguishable from
a current one by content alone. **How to read its fields, and the trap of substituting
`breakpoint-request.json`, are in [`docs/running-things.md`](docs/running-things.md).**


## Specimen craft

*(The charter settles what a specimen IS — authored in System Modeler, portable Modelica subset,
no MSL MultiBody, and the differential-comparison protocol: §4.1 and §4.3, Decisions 2 and 3.
**Read them there.** What follows is craft.)*

- **Every specimen carries a one-line `// purpose:` comment**, phenomenon-focused, plus a
  `docs/specimen-notebook/<Model>/` trace and `purpose.md`. **A new specimen must also be wired
  into the corpus outcome baseline**, or `the_corpus_outcome_matrix_is_unchanged` fails by name.
- **Scratch specimens live in `.hrw-bridge/specimens/`** — written mid-conversation to answer a
  question, listed within a second, and **ephemeral by construction**, so they are not held to the
  rules above.
- **A scratch name may not shadow a curated one.** The collision is reported and the file skipped,
  because silently loading a different model than the name says would have Claude reasoning
  confidently about source Doug is not looking at.
