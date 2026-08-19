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
showing only three connection sets per list."* `connection_anim_ui` wraps the view in a
vertical scroll area; the view then created **three more** inside it, each with a magic
height — 240pt for the lanes, 200pt for the frame's lists. A connection set costs a header
plus a line per variable plus a line per equation, so 240pt is about three sets: content
overflowed a small box while the pane around it stayed empty, and the wheel scrolled the
box instead of the page.

**The nesting is the defect; the height cap only set how obvious it was.** The rule:
**the parent owns the scrolling and the height, and a child view just renders.** A tall
model then makes a tall pane, which is the honest result.
`connection_anim::tests_layout` fails if that file constructs a scroll area or sets a
fixed height at all.

**Both scroll-area bugs were reported by Doug, not by a test**, and neither is visible to
`egui_kittest` — a clipped child is still in the accessibility tree. Layout remains the
surface where his report *is* the verification.

**INSERT A TEST AFTER A FUNCTION'S CLOSING BRACE, never before its `fn` line.** A doc comment
and its attributes sit *above* the item, so anything placed between them is adopted by the
wrong one — the new test gets two `#[test]`s and **the old function silently stops being a
test.** Nothing fails; the suite goes green. This has bitten three times: it broke Doug's
debugger launch on 2026-07-31 and twice disabled
`a_broken_specimen_does_not_poison_the_next_compile` on 2026-08-01.
`doc_citations::no_function_has_two_test_attributes` now catches it.

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

**Name the symbol as it will actually be spelled.** The example above used to read
`survey_filter`, and `ideas.md` carried that tag for a day after the filter shipped as
`matches_filter` — **the tag passed the whole time, because it resolved nothing.** A tag that
resolves nothing is indistinguishable from a claim that is still true, so the checker was green
on a claim that was false. The mechanism only fails when the target *does* resolve; a
misspelled target is silently permanent.

`doc_citations::claims_of_absence_are_still_true` fails if the target **does** resolve.
**A wrong negative is the one error nobody catches**: acting on a wrong *positive* means
going to use the thing and finding it missing, while acting on a wrong *negative* means **not
looking**. Four stale ones were found on 2026-08-01, and `ideas.md` #42 was two days from
having its link vocabulary re-implemented on top of itself. Coverage is expected to be low —
tag when you write the claim, the way provenance tags work.

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
   lands without a test that could not have been written before it.*

**This is why the three complexity lints are declined** (`hrw/Cargo.toml` carries the full
reasoning). They encode a human-comprehension heuristic, and enforcing it would reward splitting
a function *to satisfy the lint* — extraction with no new seam and no new test.

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
> ## THE CONVERSATION HAS CHANGED MODE
>
> **Doug, 2026-08-08:** *"I will finally begin a serious walk through the tours and try to shift
> our conversation to be about my education rather than about HRW features."*
>
> **That is a standing instruction, not a mood.** For three weeks this project built the
> instrument; the instrument is now good enough. **Default to teaching, not to building.** When
> Doug reports something during a walk, the first question is *"what does this teach, and is it
> true?"* — not *"what should we build?"* A feature is warranted when it unblocks the learning,
> and `docs/ideas.md` is where the rest goes.
>
> **He said the reason plainly, and it should not need saying twice:** *"We've been working on
> this project for three weeks, and I have not yet been rewarded with a learning experience."*
> Treat an hour spent on HRW polish during the walk as a cost, not a contribution.
>
> ## WHERE THE WALK ACTUALLY IS (2026-08-13) — read this before the older notes below
>
> **`connect-expansion.md` is walked, rewritten, and validated.** Doug: *"That is the template
> for all other tours."* The template and the five things that make it work are in
> [`docs/fixture-tours/README.md`](docs/fixture-tours/README.md) — **read that before writing or
> converting a tour.** Its shape: short setup → **Predict** → ▶ Look → **Expected** →
> **Falsified if** → *What just happened*, with the explanation only ever **after** the look.
>
> **Convert ONE TOUR AT A TIME — the one he is about to walk**, never a campaign. Doug:
> *"working through each conversion with you is educational for me"*, so the conversion is
> itself teaching, not preparation for teaching. The other eight are unconverted.
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
> ### What a walk still cannot be replaced by
>
> Claude verifies **content, never pixels**. Whether a layout is legible or an animation reads as
> a search is Doug's report and nothing else. Four of the six mismatches he found on 2026-08-13
> are now the kind a test catches; **the other two were conceptual** — a tour whose central idea
> had no counterpart on screen — and no test closes that.
>
> ## He is on VACATION, walking the tours (week of 2026-08-09)
>
> **Entry point: [`docs/fixture-tours/the-concepts.md`](docs/fixture-tours/the-concepts.md)**
> — the overview, whose rows are `hrw://tour/…` links that open each tour.
>
> **Nine tours cover the pipeline, and SEVEN OF THEM HAVE NEVER BEEN WALKED.** Written in one
> pass on 2026-08-08, at Doug's explicit direction to drop `#66`'s write-one-walk-one rule so a
> week of vacation could start immediately.
>
> | new 2026-08-08 | phase |
> |---|---|
> | `connect-expansion.md` | Flatten — 4 `connect`s become 3 sets become 7 equations |
> | `blt-ordering.md` | Tarjan/BLT — an order, no order, a system that splits |
> | `tearing.md` | 3×3 → 1×1, and the phase's only heuristic |
> | `index-reduction.md` | nine states, three degrees of freedom |
> | `initialization.md` | one state, two conditions |
> | `solve-lowering.md` | names become indices |
> | `events.md` | the equations that are not always true |
> | `the-concepts.md` | the overview |
>
> **Every COUNT in them is read from the specimens' generated notebook traces. Every RENDERING
> claim is unverified.** Each tour's closing *"What this tour cannot check"* section names its own
> two or three weakest claims — **read that section before defending anything in the tour.** If
> Doug reports a count is wrong, that is the serious case: a trace changed or Claude misread one.
>
> ## PER-MACHINE SETUP — do this before answering any debugger question
>
> **The VS Code extension is not installed by `git pull`.** It brings `src/*.ts`; it runs no
> `tsc` and creates no junction. This exact gap cost a day on 2026-08-08 (`docs/ideas.md` #72,
> operating notes):
>
> ```powershell
> cd hrw\vscode-extension ; npm install ; npm run build ; npm test
> New-Item -ItemType Junction -Path "$env:USERPROFILE\.vscode\extensions\dougdew64.hrw-debugger-bridge-0.1.0" -Target "$PWD"
> ```
>
> Then **reload the VS Code window**, and confirm with `code --list-extensions`. **Rebuild HRW
> too** — the `hrw://breakpoint/` links and the two-tier frame delay are compiled in.
>
> **Only `matching-live.md` needs any of this.** The other eight run from HRW alone.
>
> **AND CHECK THE PERMISSION ALLOWLIST EXISTS — first thing, on any machine.** `.claude/` is
> gitignored **by upstream Rumoca** (`e658c776`), so `.claude/settings.json` does not travel and a
> fresh clone prompts Doug for approval on **every** Bash call. Doug reported that as real friction
> on 2026-08-13; the contents and the reasoning are in
> [`docs/setup-windows.md`](docs/setup-windows.md) §8.
>
> ```powershell
> Test-Path (Join-Path (git rev-parse --show-toplevel) ".claude/settings.json")
> ```
>
> **Anchored at the repo root on purpose, and the bare relative form is a trap.** The first version
> of this line was `Test-Path .claude\settings.json`, which reported **False on a machine where the
> file exists** because the shell happened to be in `hrw/` — so the check would have ordered a
> future session to recreate a file it already had. Caught within a minute of writing it, by
> running it. **A check that reports absence from the wrong directory is worse than no check**, and
> this is the third time in one day that a stale working directory produced a confidently wrong
> result (see `DECISIONS.md`, 2026-08-12).
>
> **Claude must check this unprompted, because Claude cannot see permission prompts.** Doug feels
> the cost and has no reason to suspect a file he has never read; Claude can read the file's
> *existence* and cannot read the prompts. **Only one of us can detect this, and it is not the one
> paying for it.**
>
> ## WHERE INDEX REDUCTION STANDS (2026-08-18) — read before touching that tour or tab
>
> **Doug walked the phase-1 tours and reported that `index-reduction.md` was "way, way too
> short".** Following that took four steps down and found a defect at each:
>
> 1. **The tour's headline claim was false.** It said `Drivetrain` performs *zero*
>    differentiations. It performs **six**. `differentiated_rows` counts rows *surviving in the
>    final DAE*, and `eliminate_trivial` removes them at the end. Corrected in place; the wrong
>    version is kept visible in the stop, because the mechanism is the lesson.
> 2. **Nothing cross-checked a stage summary against the frames from the same run.** Every other
>    checker here compares a document to a trace, and the trace said zero.
>    `a_reduction_summary_never_claims_more_than_its_frames_recorded` now does, and its
>    *"some specimen must differentiate"* clause encodes the exact mistake.
> 3. **The Index Reduction tab was a re-execution, not an observation**, with HRW maintaining
>    Rumoca's step order by hand — and it had **already drifted**, missing `scalarize_equations`.
>    Rumoca gained a funnel observer (`prepare_dae_for_structural_analysis_fully_observed`), HRW
>    switched onto it, and the mirror is deleted. `updating-rumoca.md` step 3 is now moot.
> 4. **Rumoca does not reduce the canonical index-3 DAE.** `CartesianPendulum` compiles, every
>    funnel step reports zero, and it is left structurally singular — unmatched equation
>    `f_x[4]` (the constraint) against unmatched unknown `lambda` (its force). **Rumoca's index
>    reduction is pattern-based, not general Pantelides**, and every other constraint in the
>    corpus is an alias, which is why nobody noticed. Filed in `docs/upstream-issues.md` as a
>    question, not a defect claim; **not yet adjudicated against System Modeler**.
>
> **The corpus now spans the phase** — `BouncingBall` (no reduction needed), `BenchActuator`
> (1 differentiation, 48 equations), `Drivetrain` (6, at 97), `CartesianPendulum` (cannot be
> reduced). Smallest-first is the tour's spine, and the pendulum is its ending.
>
> **DONE 2026-08-18:** the pane now publishes `n_differentiations` beside the survivor list, so
> it can no longer be silent about six differentiations because none survived — the defect that
> started all of this. A funnel that did nothing also says so, which is `CartesianPendulum`'s
> whole lesson.
>
> **⟶ IF THIS SESSION IS ON THE MACHINE WITH SYSTEM MODELER**, the one owed experiment is
> simulating `hrw/specimens/CartesianPendulum.mo`. The protocol, and what each of the three
> possible outcomes establishes, are written down in `docs/upstream-issues.md` under the
> index-reduction entry — decided in advance so the result is not read to taste. It turns
> `index-reduction.md` Stop 5 from Claude's inference into an adjudicated fact.
>
> **The tour is rewritten** (2026-08-18): five stops, smallest-first —
> `BouncingBall` (nothing needed) → `BenchActuator` (**1** differentiation, 4→3 states, the
> mechanism at a size you can hold) → `Drivetrain` (6, at 97 equations) → `CartesianPendulum`
> (0, and cannot be reduced). It builds *why differentiating helps* from what an integrator can
> be asked for, and defines **index as a distance** rather than a score. **Not yet walked.**
>
> **THE TOUR IS BEING WALKED NOW, and three corrections have already come from it** — each a
> prose failure, none a wrong number, and none findable by any checker here:
>
> 1. *"Why a solver cannot simply be told about the constraint"* said **solver** and described
>    an **integrator**. A DAE solver handles constraints; that is its job. Wrong difficulty named.
> 2. The replacement assumed Newton iteration, Jacobians and singularity. Rebuilt on **matching**,
>    which he has walked — the same fact found by counting rather than arithmetic.
> 3. Backward references **retold** earlier results as prose, including a hand-written table that
>    duplicated the Incidence view. Doug: *"HRW is your platform. Use it."* Now
>    `hrw://tour/<name>/stop/<slug>` links and a pane link.
>
> **The standing offer that came with it:** *"if ever HRW does not meet your needs but could meet
> your needs with improvements, then let's pause and make those improvements."* Take it.
>
> ### DEIXIS — Doug cannot point at a tour statement, and asked for it 2026-08-19
>
> *"I'd like to enjoy the convenience of deixis when asking questions about statements which
> you've made in tours. Currently, it seems that I have to copy / paste those tour statements."*
>
> **HRW publishes `ui_mode: "Tour"` and nothing else about the tour** — not which one, not where
> he is in it. So "this statement" cannot be resolved.
>
> **Tell him he does not need to paste**, which is the immediate relief and cost nothing: the
> tours are on disk, so *"the Newton paragraph in the intro"* or *"Stop 2's table"* is enough to
> find the exact text. He had been pasting because nobody said so.
>
> Two improvements, agreed as a plan and **not yet built**:
>
> 1. **Publish which tour is selected** — ✅ **BUILT 2026-08-19.** `diagnostic_snapshot` now
>    carries `"tour"`, so a question about *"the Newton paragraph"* resolves: the name identifies
>    the document and the tours are on disk. **Read the capture before asking Doug which tour he
>    means.**
> 2. **Publish which stop is on screen** — **RECOMMENDED AGAINST, 2026-08-19.** Doug asked
>    whether the recommendation was for or against; the honest answer is against, and one reason
>    originally given for it was wrong.
>
> **Why against**, in the order that decides it:
>
> - **It does not deliver the request.** Doug wants to point at *statements*; this publishes
>   *stops*, and a stop is often a page. He would still say "the Newton paragraph in this stop",
>   which is barely shorter than "the Newton paragraph in the intro" — already unambiguous under
>   #1.
> - **It fails in the normal reading position.** With two or three stops visible, "this" is
>   ambiguous again. It works only when one stop fills the pane.
> - **The side benefit claimed for it does not exist.** It was said to make `stop/<slug>` links
>   land precisely. **That shipped on 2026-08-17**: the pane splits at the byte offset and calls
>   `scroll_to_cursor`, so egui computes the position from a cursor it knows exactly. That was
>   the strongest argument for #2 and it is already done.
> - **The cost is real and falls on a pane in constant use** — N markdown documents per frame,
>   where the code warns that two is "not free of consequence", and `connect-expansion` has
>   eleven headings.
>
> **If statement-level deixis still matters after living with #1**, the answer is not #2 — it is
> capturing *what was selected* rather than *where the pane is scrolled*. Different mechanism;
> do not design it until #1 has been used and found wanting.
>
> **STILL OWED:** the reduction passes expandable into their frames. Doug: *"my education is
> more important than strict
> adherence to the template"* — though the template constrains shape, not length, so a long
> tour needs no exemption. `docs/tour-kinds-plan.md` §4 is the governing principle.
>
> ## Open questions a walk may hit
>
> - **A reproduced state-count inconsistency**, in `docs/upstream-issues.md`: `Drivetrain`'s index
>   reduction demotes nine states to three while solve lowering reports **9**, and
>   `GearWithBrake` shows the same gap. **Not diagnosed.** `solve-lowering.md` omits its natural
>   example rather than write around it. Needs a System Modeler adjudication (`#43`).
> - **`RcCircuit` reports one `zero_crossing_condition`** with no `when` clause at all.
>   `events.md` Act 1 quotes only the four counts that are explicable.
> - **`#77`** — a live tour needs three panes and the layout has two. **Largely resolved
>   2026-08-12 and no longer blocking eight of the nine tours** (`docs/ideas.md` #77,
>   `DECISIONS.md`). Doug walked the tours on a 13" laptop with no external monitor and could not
>   fit the tour and the stage view at once; three defects were behind it, and **all three were one
>   number or one call:**
>   - **`DEFAULT_ZOOM` was 2.0**, which *multiplies* the display's own scaling, so a 13" screen gave
>     HRW ~640 layout points instead of ~1280. A pre-port WSLg compensation that had been
>     double-counting since 2026-07-27. Now 1.0.
>   - **The left panel's minimum was a fraction of the window**, which fell below the content's own
>     minimum on a small screen — the divider stopped while the content kept shrinking. Now
>     `MIN_LEFT_POINTS`, an absolute floor.
>   - **The tour pane was a vertical-only `ScrollArea`**, so it sized itself to the widest table in
>     the document: it opened at **70 % of the window while reporting a 40 % default**, and froze the
>     divider. Now `both()`, so wide content scrolls instead of pushing.
>
>   **Doug, 2026-08-12: *"Finally, HRW is usable on my 13" screen."*** Expect tours to be **taller**
>   now — prose wraps to 40 % rather than 70 % — which is the correct trade, not a regression. What
>   survives of #77 is only the genuine three-pane case: HRW at half width beside VS Code is back in
>   the ~640-point regime, so **`matching-live.md` alone may still want a layout change.** The stop
>   strip, drawer and alternating-mode options are recorded there; do not build one for the other
>   eight tours.
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
> ## THE MEMORY STORE DID NOT TRAVEL WITH THIS PULL
>
> It lives outside the repo, keyed to the filesystem path, so a different machine or clone path
> has none of it. **This box is the handoff.** If something here contradicts a recalled memory,
> this box is newer.
>
> **Standing:** Claude never needs permission for context maintenance, **and accuracy is never
> traded for it.**

**Pass two: re-implement Arcs 1-7 with internal Rumoca access, delivering richer stage views
than the public API allowed.** Per arc: scout what state the phase holds (read the crate under
`../crates/`), expose it additively, render it. Remaining per-arc opportunities are
`docs/ideas.md` #19-#22. The log view is delivered. Pass-one closure record and the arc history
are in [`DECISIONS.md`](DECISIONS.md).

**The sequence — each step's output is the next step's input.** Restructured 2026-08-01: the
oracle test is **no longer a step** (see below).

1. **The MSL survey** ✅ — `examples/survey_msl.rs`. Rumoca's reach across all 2,626 MSL models,
   plus the IR-shape metrics that stratify the sample.
2. **Fidelity testing at scale** ✅ — F1-F9 over that corpus. **2,614 of 2,626 models green**
   (2026-08-01); 12 exceeded this machine's memory or the time limit.
3. **The verification pause** ✅ — [`docs/verification-plan.md`](docs/verification-plan.md), all
   six items landed 2026-08-01: the must-fire convention and its audit, the stale-negative test,
   clippy cleared and denied, the pre-commit suite memoised (375s → 113s), **headless UI testing
   with `egui_kittest`**, and the run drivers resolved by splitting.
4. **The UI pause** ✅ — [`docs/ui-pause-plan.md`](docs/ui-pause-plan.md), landed 2026-08-02.
   Tests first, then refactoring, at Doug's direction. **`App` 105 → 57 fields**, `frame_ui`
   727 → 419, `central_panel_ui` 771 → 430, 504 → 524 tests, and `model_list.rs` /
   `tour.rs` split out of `app.rs`. Six state groupings now own what was scattered across
   `App`. **The field-count ratchet** (`doc_citations::app_does_not_regrow_its_field_count`)
   keeps it there: raising it requires the reasoning in the same commit.

   **What the pause did *not* settle**, recorded so it is not assumed: `app.rs` still ends the
   day at 9,434 lines, and the claim that its size causes editing defects is unproven either
   way — the honest test is whether `ui-findings.md`'s R-series stops recurring, which only
   the next substantial edit can show.

5. **The corpus list** ✅ **CLOSED 2026-08-03** — `docs/ideas.md` **#52**. Three sources behind
   one filter, built 2026-08-01. **The join it argued for was deleted rather than built**, on
   the sweep's evidence: 2,614 rows, `outcome=ok`, `n_violations=0`, no failed checks. A
   fidelity column would read `ok` on every row and a fidelity predicate would match everything
   or nothing.

   **The zero counts this time, and did not before.** Earlier sweeps ran those checks against
   `{"classes":{},"within":null}` and found nothing because there was nothing there. The
   2026-08-02 run walked a real Modelica AST — mean peak memory 1,228 → 1,353 MB, and F7 went
   from sampling ~2 nodes of the Parse stage to its 400-path cap.

   **What reopens it:** a report with a *non-constant* column. The oracle (#43) is the live
   candidate, since findings vary per model by construction.

6. **The draggable divider** ✅ — `docs/ideas.md` **#59**, built and confirmed working
   2026-08-03. `SplitState`: both panels resizable, clamped to 15–75 %, opening at 40/60.

   **The split is a fraction of the window, not a stored pixel width**, and that distinction
   was the bug: the first frame reports a window size that does not exist (5000 px observed),
   so 40 % of it was stored as an absolute 2000 px and clamped to the 75 % maximum on the real
   window. **Five attempts; the sixth came from instrumenting rather than theorising** —
   `ui-findings.md` C15, and the rule it produced is in the rules section above.

7. **← LIVE: Doug's education, along the chain from DAE onward** *(Doug, 2026-08-03:
   "we really haven't invested much time or effort into my education… now, I want to spend a
   while investing in my education")*. The subject is
   `docs/compiler-phases/the-chain-of-problems.md`, starting at its leftmost item, and the
   instruments are **HRW, System Modeler and Wolfram Desktop together** — explicitly *not*
   text answers in conversation.

   **"Understand" is defined by trial and error**, and the completion signal is Doug's:
   *"we'll know I've accomplished my goal when I stop requesting improvements."* So HRW is
   changed — **even substantially** — whenever a change would teach better. Doug's standing
   authorisation covers the Rumoca instrumentation too: *"if you determine that we need to
   change how we instrument Rumoca in order to enable you to create effective, high-value
   tours, then we will stop and change how we instrument Rumoca."*

   **The delivery vehicle is a fixture tour, not a conversational plan** — Doug's own
   instruction, because a plan scrolls out of the conversation and a versioned tour does not.
   Tours here are **live documents, extensions of the conversation**: regenerate one *while*
   Doug is walking it, and use it to motivate the questions he brings back.

   **Doug is walking the tours now (2026-08-03), and that is the live signal.** His grading
   criterion is recorded in `docs/question-ledger.md`: *"the real measure of whether the tours
   are good enough will be the nature of the questions which I ask you while and after I work
   through the tours."* Log each question **against the stop that prompted it**, and read that
   section before answering — it records the four question shapes and the opposite responses
   they call for. **No questions at all is ambiguous and must not be read as success.**

   **Delivered so far:** `docs/fixture-tours/dae-construction.md`, the first *curriculum*
   tour — `SingleInertia` (2 equations, 2 unknowns) against `UnbalancedShaft` (2 and 3,
   balance −1), one line apart. Composing it exposed two gaps that were then fixed rather
   than written around: **the DAE had no tab**, and **the five tree-only stages could not be
   pointed into** (`DECISIONS.md`, 2026-08-03).

   **Next in the chain:** index reduction, on `Drivetrain` — where a square system is no
   longer enough because ideal gears make a state non-independent.

   The standing menu below is **not** this work, and is picked from only when this is idle.

   In rough order of value: #46 (a failure specimen and tour per compiler phase —
   the largest item serving the learning mission, since phases that only ever succeed cannot
   be diagnosed), #49 **re-scoped** (fixture tours were sized for "everything a test cannot
   reach", which the pause measured at *two surfaces*, so that entry now drives work sized for
   a world that no longer exists), and #43 as a track.

**[`docs/reports.md`](docs/reports.md) held the design authority for the corpus list.** Its
load-bearing claim — **survey → eligible, fidelity → trustworthy, oracle → findings**, joined
on `name` — **is now half retired.** The list shipped; the join did not, because two of the
three reports turned out to be constants (see step 5). The claim stands for the *oracle*, whose
findings vary per model by construction, and that is the case which would reopen it.

**The oracle (#43) is a TRACK, not a step** *(2026-08-01, Doug)*. It was step 4 and gated the
work; it does not belong there.

- **It never blocked the list**, and the list does not need it to be *tested* either: the survey
  (2,626 rows) and the fidelity report (2,614) are two real sources with genuinely different
  shapes — *browse* versus *exceptions* — which is what exercises a filter. A third report would
  be **new columns, not a new shape**.
- **Its value is elsewhere**: Doug's education (an independent adjudicator, which is why *oracle
  first* is a standing practice — it corrects Claude's bias toward blaming its own specimen) and
  **upstream**, where `upstream-strategy.md` calls differential testing the rarest thing Doug
  brings.
- **One constraint survives because it is free, and it is now the only live piece of the join:**
  *if* an oracle report is ever produced, it must emit the same `name` join key. That binds the
  **oracle's** design, and retrofitting it later would cost the join that #52 deleted for want
  of a non-constant column.

**A dependency the sequence used to hide, now met:**

- ~~The list needs a compile-by-qualified-name path in the worker.~~ ✅
  **`WorkerState::compile_model_by_name` exists** — built for step 2, since checking HRW's
  representation of an MSL model means compiling it *through HRW's own path*, which is the thing
  under test. Note **why it cannot just call `compile` with the library file**: a library file
  may declare many classes — `Blocks/Continuous.mo` holds `CriticalDamping` among others — so
  "the first class in the file" is the wrong model. The document is **located, not added**.

**The signal that dropping the mode was wrong**, recorded so it stays recognisable rather than
being rationalised away: a question that genuinely **cannot be expressed as a filter** over the
list's rows. That would mean something Test-mode-shaped was right after all, and it should
reopen `docs/ideas.md` #52 — which is now closed, so reopening it is a deliberate act rather
than a drift.

---

## Running things

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
cargo fmt -p hrw                                   # rewraps lines -> changes the counts
cargo run -q -p hrw --example gen_architecture     # module sizes, App field groups
cargo run -q -p hrw --example gen_tour_catalogue   # tour stops -- ANY heading edit changes these
cargo clippy -p hrw --all-targets                  # lint the code in the shape it ships in
cargo test -p hrw --lib --test msl_resolve --features slow-tests -- --test-threads=1
```

**Getting this wrong costs the whole 225 s, and it has now cost it four times.** Twice on
2026-08-15 by regenerating *after* the gate; once on 2026-08-16 by regenerating *before*
`cargo fmt`, which reflowed the source it measures; and once the same day by regenerating
`architecture.md` while forgetting **`CATALOGUE.md` exists at all** — adding one `##` heading to
a tour changes its stop list.

**There are two generated documents, not one**, and the pattern each time has been *"I ran the
generator"* rather than *"I ran the generators"*. The failures announce themselves as
`architecture_regions_are_current` and `tour_catalogue_is_current`.

**A third and fourth generator exist and are NOT in this list**, deliberately:
`gen_field_help` (build-time doc comments, run by `build.rs`) and `gen_matching_reference`. If a
gate ever fails on their output, they belong here too.

**And the gate can fail while HRW is running** — `error: failed to remove file … hrw.exe,
Access is denied`. Doug builds from the tree and keeps the app open, so this is the normal case,
not an anomaly, and it is **not** always transient: once a preceding `clippy --all-targets` has
invalidated the binary's fingerprint, the combined form retries forever.

**Split it, and both halves run with the app open** *(measured 2026-08-16)*:

```text
cargo test -p hrw --lib --features slow-tests -- --test-threads=1
cargo test -p hrw --test msl_resolve --features slow-tests -- --test-threads=1
```

Selecting one target at a time does not pull in the bin; selecting two does. Prefer the combined
line, fall back to these two rather than asking Doug to close the app he is testing in.

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
triggers the large fidelity sweep carries. It costs **157 s** and has its own feature because it
must give each specimen a **fresh** `WorkerState`: against the shared worker it is
*order-dependent*, passing alone and failing in company.

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

**FOUR levers already ruled out by measurement — do not re-propose them.** Parallelism buys about
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

**`--test-threads=1` is required** — two pre-existing tests race on process-global stdout and
on `focus.json`, and the suite can **hang** under the default harness on a clean tree.

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
