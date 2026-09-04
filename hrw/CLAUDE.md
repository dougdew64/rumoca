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

**ACCURACY IS THE PRECONDITION OF EDUCATION AND OUTRANKS EVERYTHING** — features, polish,
performance, and the cost of a change to the Rumoca crates. **This is charter Decision 7 and only
the charter states it**; read it there rather than any summary. What follows here is operational:
what the charter does *not* say.

**A STAGE'S OUTCOME IS A CLAIM TOO: `Outcome::Failed` means "the pipeline stopped here", so at most
ONE stage per compile may carry it.** Four sites once painted whole runs of stages `Failed` for a
single stop; `the_corpus_outcome_matrix_is_unchanged` pins it now.

**"REPLAY" MEANS THREE THINGS AND ONLY ONE IS FORBIDDEN — judge by where the frames came from,
never by the word.** **Playback** of frames recorded during the real compile is the animation
feature and is **correct** (`CompileFrames`). **A live debug session** genuinely re-runs a phase
because that is what was asked for (`PendingLiveDebug::PreLowering`) and must not be "fixed".
**Re-execution presented as the compile** is the fiction, and it is gone. *(Twice this has
threatened a working feature: a session reads "the reduction replay" in a lab and either deletes
something correct or — the silent case — concludes the fictions were dealt with and stops looking.)*

**A LARGE GREEN RESULT COVERS THE TERRITORY IT MEASURED AND NO MORE, WHILE THE CONFIDENCE IT
PRODUCES DOES NOT KNOW THAT.** 2,614 green fidelity rows could not have caught one of the fictions,
because every check asked about a **noun** and every fiction was a **verb**. **And 2026-09-01 added
a second instance: the FULL gate passed — 915 tests — while `CLAUDE.md` carried five broken links
to a directory the rename had moved.** Nothing resolves this file's markdown links. **Before citing
a green run as evidence, say what it measured.**

**AND A THIRD KIND, 2026-09-02, WHICH IS NOT "THE CHECK DID NOT RUN" BUT "NO CHECK COULD HAVE
EXISTED": NOTHING COMPARES A LABEL AGAINST ITS MEANING.** `events_to_json` published Rumoca's
`synthetic_root_conditions` as `zero_crossing_conditions`, so `BouncingBall` showed **0** — a
bouncing ball apparently detecting no contact. **Every value was correct for three weeks.** The
fidelity sweep compares structures, the notebook check compares values, and both were right; the
defect lived entirely in a name. It reached three labs and an `upstream-issues.md` entry written to
be filed before Doug said *"accuracy is a requirement, verify."*

**So when HRW renames a field it read from Rumoca, that rename is a CLAIM** — and the cheapest
defence is not to make one. **Publish Rumoca's own names**; where a friendlier label is genuinely
wanted, put it in the prose beside the value, never in place of it.


**When accuracy needs a Rumoca change, the change is the cheap option** — see *a quality bar can
become a discouragement*, below, which carries the account.

**The gates, commit split, dependency rule and rebase triggers are procedure** —
[`docs/running-things.md`](docs/running-things.md), *Touching a Rumoca crate*. **That instrumentation
must stay additive, observation-only and upstreamable is settled** by the workspace-root `CLAUDE.md`
and [`docs/upstream-strategy.md`](docs/upstream-strategy.md); do not restate it here.

**WHEN YOU WRITE A MEMORY, NAME WHERE IT BELONGS IN THE REPO — and charter Decision 11 answers
that**: Doug's decisions to the charter, Claude's craft here, procedure to `running-things.md`,
history to `DECISIONS.md`. **If a fact has no home in the repository, that is the finding.**

**The memory store does not survive a clone.** It lives outside the repo, keyed to the project's
filesystem *path*, so a different machine **or a different clone path** loses all of it — and Doug
switches machines twice a week. **Writing a memory feels like recording something**; the write
succeeds, and the loss happens later, on a machine where nobody is looking.

**THE MUST-FIRE RULE.** Any code whose job is to *report* something gets a test proving it
reports; **silence must be a failure, never a pass.** Its absence makes a change incomplete.
All seven silent bugs of 2026-08-01 were observers that looked like they worked: a dead column,
an array argument collapsed by `powershell -File`, an `eprintln!` swallowed by HRW's own
fd-level `OutputCapture`, a rate limiter gating its own first fire, an announcement silent when
work was pending by absence. `fidelity.rs` had this discipline
(`each_invariant_catches_its_own_violation`); the tooling around it did not.

**"REPORTER" HAS WIDENED TWICE, AND THE SECOND TIME WAS 2026-09-01.** It began as *code*, then the
Context Bar showed that **a pane is a reporter**. Then a **document** proved to be one: `CLAUDE.md`
carried five broken links through a FULL gate because nothing resolves this file's markdown links,
while `fixture_lab_links_all_resolve` covers the labs. **A document that cites is reporting, and an
unresolved citation is silence passing.** Expect the noun to widen again rather than treating the
current list as the boundary.

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

**AND THE CHEAPEST DRESS IT WEARS IS A FILTERED GREP** *(2026-09-04)*. Claude reported that the
simulation plot did not honour the follow, from `grep "tracked" | grep -iE "plot|sim"` — which
demands both terms on **one line**. The feature was there and had been: the binding is `let
tracked = self.tracked_identifier;`, carrying neither word, and the empty result read as absence.
Doug had nothing followed at the time, so the screen agreed. **A second filter is a claim that
the thing you seek and the word you chose share a line**, which for a binding is usually false —
so grep the symbol alone and read the hits, or grep the file.

**Two scroll-area rules, each with a test that carries its own account:** a scroll axis is a claim
about how a widget negotiates size with its **parent**
(`ui_tests::the_left_panel_content_never_detaches_from_the_divider`), and **never nest a vertical
scroll area inside one** — the parent owns the scrolling and the height (`playback::tests_layout`).

**AND A STANDING LIMIT ON WHAT CLAUDE CAN VERIFY AT ALL, which is not a fact about scroll areas.**
Both of those were **reported by Doug, not caught by a test**, and neither is visible to
`egui_kittest` — **a clipped child is still in the accessibility tree**, so a widget can be
correct in the tree and wrong on the screen. **Where that is true, his report *is* the
verification**, and there is no test to write instead.

**It is the same boundary as *effectiveness is Doug's*, reached from the tooling side rather than
the pedagogical one.** Expect it wherever the question is *what did it look like* rather than *what
did it contain* — layout, legibility, whether an animation reads as a search. **Say which parts of
a report are test-verified and which are only reasoned**, rather than letting a green run imply
more than it measured.


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
stops being a test. The history and the mechanism are on
`doc_citations::no_function_has_two_test_attributes` and
`tests_orphaned_docs::no_doc_block_gains_a_second_summary`, which catch it.

**BITTEN EIGHT TIMES, FIVE OF THEM ON 2026-09-04 ALONE — so the rule is not the gap, the ANCHOR
is.** Every instance came from anchoring an edit on the text being inserted *before*: `#[test]`,
or the `fn` line. Both sit **below** the neighbour's doc comment, so matching them puts the new
item inside it. **Anchor on the preceding item's closing brace instead** — `    }` plus the blank
line — which is the one position that cannot be inside anything. The checkers catch it every
time, so the cost is a red gate rather than a silent defect; it is still five gate runs.

**A CHECKER RETIRES THE PROSE IT REPLACES** *(2026-08-22)*. When a rule becomes a test, the prose
here shrinks to **one sentence and a pointer at the test** — the reasoning belongs on that test's
doc comment, beside the code enforcing it, where it cannot drift. This paragraph paid for itself
that way: `no_function_has_two_test_attributes` and `claims_of_absence_are_still_true` already
carried their histories, so the copies here became pointers.

**OPEN THE TEST AND CHECK THAT IT ACTUALLY CARRIES THE ACCOUNT BEFORE SHRINKING THE PROSE**
*(2026-09-01)*. A named test is not evidence that the reasoning lives there. Must-fire's account was
nearly deleted on that assumption, and
`each_invariant_catches_its_own_violation`'s doc comment turns out to be three lines about its own
case, saying nothing about the seven silent bugs. **Unverified, this rule licenses deleting the only
copy** — which is the one failure it exists to prevent.

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
- **A DUPLICATE IS MOST OFTEN BORN WHILE REMOVING ONE** *(2026-09-01, twice in one session)*. Cutting
  a rule from one place, Claude restates it in the place he is writing rather than pointing at where
  it already lives — the quality-bar rule and the violability rule both went that way, each within an
  hour of the sweep that was hunting exactly this. **The move is always a pointer, never a
  restatement**, and the moment to check is when prose is being *moved*, not when it is being added.

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

**A generator script is the exception, not the tool of choice.** When one is genuinely warranted,
write it with the Write tool and run it by path — that pattern never produced shell corruption. And
**read back anything a shell wrote.** *(Validated 2026-09-01 by the tour → lab rename, which
genuinely warranted five: reading back is what caught a falsified historical filename and three
mangled quotations.)*

**The corruption is a habit that operates on small files too.** A large file *pressures* Claude
toward generators, but the memory case was a single short file. **Do not treat file size as the
risk factor**, and do not offer this rule as an argument for a refactor — that justification is
charter Decision 12(b), and it does not need the help.

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

**THE QUESTION APPLIES TO SCRIPTS AND ONE-OFF TOOLING TOO, and it went unasked on 2026-09-01.**
Five generator scripts were written for the tour → lab rename and all five committed without the
test ever being applied. **They probably do earn permanence** — a rename of that shape recurs, and
they are the record of exactly what was substituted and what was deliberately protected. **The
defect is that the question was never asked**, which is the rule firing and Claude not hearing it.
**Ask it out loud for anything single-use before committing it.**

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

**AND A TAG ON WORK THAT MUST NOT BE DONE IS DELETED, NOT KEPT** *(2026-09-01)*. `unbuilt:` exists
to **invite** a later session to build the thing, so an absence tag on something ruled out is a
recruitment notice. `last_walked` marked the absence of walk-tracking derived from the action
trail — work Doug had by then retired twice — and it went with the paragraph carrying it.
**When a claim of absence becomes a claim of prohibition, the tag is the wrong instrument.**

**WHEN TO REFACTOR is charter Decision 12(b)** — Claude's comprehension, his ability to
maintain, or testability, and **never a line count.** The three complexity lints are declined
for that reason (`hrw/Cargo.toml` carries it).

### DEFAULT TO TEACHING, NOT TO BUILDING — a standing instruction, not a mood

**Doug, 2026-08-08:** *"I will finally begin a serious walk through the labs and try to shift our
conversation to be about my education rather than about HRW features."* And the reason, which
should not need saying twice: *"We've been working on this project for three weeks, and I have not
yet been rewarded with a learning experience."*

**When Doug reports something during a lab session, the first question is *"what does this teach,
and is it true?"*** — not *"what should we build?"* A feature is warranted when it unblocks the
learning; [`docs/ideas.md`](docs/ideas.md) is where the rest goes. **Treat an hour of HRW polish
during a session as a cost** — and charter Decision 14 raised that cost rather than lowering it,
because the hour now competes with an exchange that teaches directly.

**Neither the labs nor the UI are fundamental — the mathematics as Rumoca implements it is.** So
a mismatch may be fixed by changing the *pane*, and on 2026-08-13 one was. But **labels must
expose Rumoca's structure, not a pedagogically convenient one**; when prose and pane disagree,
**Rumoca is the arbiter.**

**ANSWER THROUGH HRW WHEN HRW CAN SHOW IT — a standing expectation, and its failure is silent**
*(Doug, 2026-09-02)*: *"Enabling you to provide richer answers is a big reason for this HRW project.
This HRW project provides less advantage over this text conversation if you don't make use of HRW to
provide rich answers when possible."*

**Before answering, ask what HRW could show** — write an **Answer** to `.hrw-bridge/answer.md` when
the reply wants a route through panes. There are **eighteen `hrw://` link forms**
(`app.rs`'s `parse_hrw_link` is the roster): not only `load` and `stage`, but `node` to land on one
field, `frame` on one animation step, `equation` on one row, `follow` to trace an identifier across
every stage, `src` to open Rumoca in VS Code, `source` for a specimen line, `breakpoint` to arm a
live anchor, `systemmodeler` and `notebook` for the oracle.

**Why this needs writing down rather than remembering: a text-only answer looks finished.** Nothing
signals that the platform went unused — Doug gets a good answer and neither of us notices. **The
recorded instance is 2026-09-02**: an Answer about the bouncing-ball event quoted six counts with no
route to any of them, and said *"worth asking System Modeler"* when `hrw://systemmodeler/` exists.
**Describing an action HRW can perform is the tell.**

**WHEN DOUG CORRECTS AN ANSWER, EDIT THE ANSWER — do not reply in chat and leave it standing.**
*(Doug named this 2026-09-02: "it is great that you are iteratively improving your answer rather
than creating a sort of conversational thread which follows your original answer.")* **This is a
third reason an Answer earns its place**, beside *richer than text* and *a route through panes*:
a chat thread accretes, so the wrong original stays at the top and the reader reconstructs the
truth from four messages, while **the Answer simply becomes right.**

**The failure is silent in this project's usual way**: replying *"you're right, it's actually X"*
leaves a document asserting something false, with the correction in a conversation that scrolls
away — which is the same reason the *why* must live in the repository. Today's Answer took four
corrections and stayed one document.

**ORDER THE LINKS: A LOAD COMES FIRST.** `source`, `follow`, `stage/node`, `stage/frame`,
`stage/equation` and `SwitchStage` all **require a loaded specimen** — with none, HRW refuses rather
than half-applying, because a pending state would fire later and send the reader somewhere no link
pointed. **The same Answer put two specimen-requiring links above its first `load`**, so the first
two things Doug could click were guaranteed to be refused, and he reported them as broken.

**That report is the third of its kind, and every time the reason was on screen.** A refused link
notifies in the **status bar**, which reads as *nothing happened* to anyone not looking there — the
observation the labs' *an expectation must say WHERE to look* rule is built from. **Say it in the
Answer**, once, near the first link.

**When a 🎯 capture arrives, locate the passage in the file the capture names.** The emitted text
is what the pane *rendered*, so it will not match the markdown byte-for-byte.

**AND WHEN NO CAPTURE ARRIVES, SAY SO RATHER THAN GUESSING WHICH PASSAGE WAS MEANT** *(2026-09-01)*.
The hook reports the state on every prompt, including *"`focus.json` predates this HRW session"* —
**that line is information, not noise.** Asked to explain "this statement" with nothing captured,
reporting the absence and naming the likely candidates costs one exchange; guessing wrong costs an
explanation of the wrong sentence, which Doug has no reason to suspect.

**The rest is owned elsewhere and must not be restated here** — prose runs only to the first
prediction, and one lab at a time ([`docs/fixture-labs/README.md`](docs/fixture-labs/README.md));
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
- **This binds as files are touched, never as a campaign.** *(Confirmed live 2026-09-01 — Doug: "I
  do expect to make edits to some of the visualization logic." It is no longer the hypothetical
  "eventually" of 2026-08-05. **Some**, though: the five files are the measured surface, not a list
  he has committed to, so a campaign would still be Claude rewriting files nobody asked about.)*

**AND SCENARIO 2 IS THE ONE COUNTERWEIGHT TO CHARTER DECISION 12(b), which is worth stating so a
later session does not apply the wrong one here.** Everywhere else, code is refactored for
**Claude's** comprehension and never for a human's. **In these five files a human reader binds** —
Doug specifically, new to Rust and egui. Applying 12(b) here produces terser idiomatic Rust that
serves nobody, because the person who has to read it is the one who cannot.

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

**THE TECH-DEBT TRIGGER WORTH REMEMBERING IS "WHO CAUGHT IT?"** — **toolchain, nothing to sweep;
a human, the code lives somewhere nothing checks.** The property is **verifiability, not Rust**, so
adding a test, a non-vacuity guard or a loud failure is often cheaper than converting anything.
*(2026-09-01 supplied two: Doug caught the `experiment` collision, and grep caught five broken
links no test resolves.)* **Both triggers and the debt itself are in
[`docs/tech-debt.md`](docs/tech-debt.md)**; the forward one — each phase boundary, scoped to what
the next phase touches — is procedure and lives there.

---

## Current work

> ### ⟶ THE MACHINE SWITCH DID NOT HAPPEN — Doug stayed here, 2026-09-03
>
> The handoff written for it is gone rather than left standing; a stale instruction to rebuild
> things "on that machine" is worse than none. **Its one owed item is DONE** — Doug rebuilt the
> bridge extension and reloaded on 2026-09-03, verified by `out/*.js` carrying the new fields.
>
> **The standing fact it leaves behind: `hrw/vscode-extension/out/` is gitignored, so a commit
> touching the extension does not reach the running VS Code until `npm run build` plus
> *Developer: Reload Window*.** Doug always launches HRW under the debugger — his words — so the
> bridge is his primary instrument, and a stale build reports the old schema while looking
> current. **Say so in the same message as any extension change**, the way a `crates/` edit's MSL
> re-parse is priced.
>
> **The OOM arc is closed.** egui's debug-only `Id` map is O(2^depth) in nesting; HRW aborted at a
> 94 GB commit. Account in [`DECISIONS.md`](DECISIONS.md) and [`docs/upstream-issues.md`](docs/upstream-issues.md)
> **E1**; the guard is `doc_citations::egui_debug_id_diagnostics_stay_off_in_dev_builds`.
>
> **Two things are owed.** **E1 is written and NOT FILED** — Doug's call, and its measurements are
> all against `0.35.0`, so re-run the reproducer on egui `master` before filing rather than trusting
> the source-read. And `every_documented_source_path_exists` reads *another* project's
> `crates/<name>/src/*.rs` as a workspace path, which cost E1 its GitHub links; teaching it about
> URLs is a `src/` change nobody has made.
>
> **AND A THIRD DEFECT CLASS IN THE SAME ANSWER, 2026-09-03 — Doug: *"I don't see this in Solve
> Lowering."*** He was right three times over: `discrete_real_updates` is an **Events** summary
> field that I attributed to Solve lowering; there is no `Minus` node; and **Solve lowering has no
> expression tree at all** — it holds a straight-line instruction program (`LoadY`, `LoadP`,
> `Unary`, `Binary`) over numbered registers, which is what *lowering* names. `Y[1]` was my
> shorthand for `problem.layout.bindings.v.Y.index`, borrowed from `solve-lowering.md` **without
> the table that grounds it**.
>
> **The pattern, now three for three: every wrong claim in that Answer was a NOTATION or a
> LOCATION, never a number.** Each number checked out against the trace — seven `p_scalars`, index
> 1, one Events update. What failed was *where I said to look* and *what I said it would look
> like*. **So verify an Answer's pointers against the trace, not only its counts** — a count is
> checked by `docs/specimen-notebook/`, and nothing checks a pointer, because
> `.hrw-bridge/answer.md` is gitignored and no checker reads it.
>
> ### ⟶ THE DOCUMENT REVIEW IS RUNNING — began 2026-09-01, **this file** first
>
> **Doug is questioning every paragraph**, beginning with whether it is still needed: *"it seems to
> be as much a historical log as a policy statement."* **He was right** — this file was 1,420 lines,
> of which `The rules` / `Current work` / `Running things` were 88 %.
>
> **THE TEST IS CHARTER DECISION 16'S THREE QUESTIONS, IN ORDER** — not Decision 10 alone, which is
> what this box said until the pass reached it. **(1)** Can it fail by name? → build the mechanism,
> delete the prose. **(2)** Is it a conclusion about a situation? → it rots; ask Doug for a ruling.
> **(3)** Is it about how Claude works, with a silent failure mode and an instance on record? →
> keep it, and say that is why. **Decision 13** adds the second axis — a rule that binds can still
> name something perishable — and **Decision 11** routes whatever survives.
>
> **Claude is the wrong judge of what to cut** and should supply evidence, not verdicts: this
> history is the record of his own failures. **Vindicated twice today** — he proposed deleting
> 8,650 lines of Rumoca documentation on a premise only Doug could correct, and created two
> duplicates while conducting the sweep that hunts duplicates.
>
> **THREE PASSES ARE COMPLETE.** The lab rules — 25 items, Doug ruling on each: 19 kept, 2 cut, 1
> named the standing loop target, the vocabulary settled, 2 renamed. Then `The rules` here under
> Decision 16 — 8 mechanisms, 8 category 3, 27 judgements: **2 cut, 3 routed to procedure, 1
> duplicate removed, 21 kept.** Then `docs/fixture-labs/README.md` under Decision 16 — 40 sections,
> **6 mechanisms (all ten cited tests verified to resolve), 9 category 3, 17 judgements**, of which
> three needed work and two of those carried defects Claude had introduced the same day.
>
> **CLOSED 2026-09-01: `doc_citations::every_markdown_link_in_a_governing_document_resolves`.**
> Nothing checked `[text](path)` in the governing documents — only backtick citations — so the
> rename left five dead links in this file behind a green FULL gate. **The checker found eleven on
> its first run**, six of them in `running-things.md`, which was written that morning with
> `CLAUDE.md`-relative links and gated green four times.
>
> **⟶ THE LABS' PROSE WAS CHANGED BY SUBSTITUTION, AND NO CHECKER COVERS WHETHER IT STILL READS.**
> `Stop N` → `Station N`, `walk` → `run`. The gate proves links resolve, tables match a real compile
> and kinds are consistent; **it cannot tell whether a sentence still reads.** Four mangled
> sentences were found in the governing documents by grepping one predicted pattern.
>
> **THE TWO HIGHEST-RISK CASES WERE CHECKED BY DOUG ON 2026-09-01 AND BOTH PASSED** — do not redo
> them. **`connect-expansion`'s opening**, the densest prose in the corpus, rewritten twice under
> the provoke-questions and code-grounding rules. And **Station 6 with `ScopedConnect`**, which
> carried two risks at once: authored 2026-08-31 to falsify Station 1's rule, machine-checked
> numbers, and **prose that had never had a reader.**
>
> **What remains is ordinary risk across the other labs**, no longer a known exposure — read them as
> they come up rather than as a campaign. **Only Doug can detect this**, which is why it is recorded
> here rather than queued as work.
>
> **AND THE RESULT CONTRADICTS WHAT BOTH OF US EXPECTED, so do not read it as failure next time:
> Decision 16 is a CLASSIFICATION test, not a deletion test.** `The rules` grew from 622 to 640
> lines. Most rules were category 3 doing real work **with pieces missing**, and the pieces got
> added because today is the day those rules fired. **The dominant defect was still the one the lab
> sweep found six times** — a mechanism exists, prose beside it does the same job by hand, and the
> hand copy rots. The mechanism was never the wrong one.
>
> **⟶ THE RENAME IS DONE — charter Decision 15, executed 2026-09-01 in TWO atomic passes.**
>
> **The vocabulary is `lab` / `station` / `observation` / `instructor`, and you RUN a lab in a
> SESSION.** The first pass took the nouns — 127 files, ~3,750 occurrences — and **missed the verb
> entirely**; Doug caught it. The second took `walk` → `run`/`session` across 85 more files. Both
> gated green, and **Doug verified the capture button and live debugging by hand** — the half no
> gate can check.
>
> **`walk` and `stop` both collided, and traversal survives untouched**: `walk_modules()`,
> `fn walk(dir: &Path)`, `walk_blocks`, `walker`, `SIGSTOP`, `backstop`, *"Stop following"*. **Ten
> `walk`s remain in the governing files on purpose** — two of Doug's quotations, `last_walked`'s
> real name, Decision 14's own wording, and the collision analysis itself. `observation`, `Predict` and `Expected`
> were already lab-native and never moved. Among the kinds only `adjudication` → `calibration`;
> `concept`, `feature` and `failure` keep their names, and `experiment` / `orientation` /
> `diagnosis` are **rejected on domain collisions** recorded under Decision 15.
>
> **Two things were deliberately NOT renamed, and both must stay that way.**
> **`DECISIONS.md`** — 191 mentions — is history and does not bind, so an entry dated 2026-07-22
> saying *"guided tours drive backlog prioritization"* stays true about that day; it carries a
> vocabulary note instead. And **`end_to_end_tour.md`** is the real name of a file deleted
> 2026-08-01 and recoverable from git under it. The bulk pass rewrote that one and a corrective
> pass put it back; every later pass sentinel-protected it. **A proper noun naming something in the
> past does not move when the vocabulary does.**
>
> **A CHECKER GAP FOUND BY GREP, NOT BY A TEST, AND STILL OPEN.** `CLAUDE.md` carried **five broken
> links** to `docs/fixture-tours/README.md` after the directory moved, and the **full gate passed**.
> Nothing resolves this file's markdown links, though `fixture_lab_links_all_resolve` covers the
> labs. That is a hole in the most-read file in the reading path — **worth closing on the next
> `src/` errand.**
>
> **The rename stopped cleanly at the VS Code extension**, which needed no functional change: it
> knows about debugging, and the bridge contract it reads never carried the word. One doc comment
> was the entire surface. That is evidence the separation is real rather than aspirational.
>
> Nights 1-7 and what each found are in [`docs/unattended-run-log.md`](docs/unattended-run-log.md);
> the nightly document step is in [`docs/unattended-runs.md`](docs/unattended-runs.md), which owns
> both the *restore, never choose* boundary and the finding that document drift is **same-day** — so
> the risk window is hours, and a day of heavy mechanism change should end with a sweep.
>
> **NOTHING IS OWED.** All four sweeps landed 2026-09-01 as one errand — four items sharing a
> single FULL gate is what made a deliberate errand cheaper than four ride-alongs.
> [`docs/tech-debt.md`](docs/tech-debt.md), *"Owed sweeps"*, keeps the record.
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
> **THE TWO MODES DOUG WORKS IN — running labs when he can focus, low-supervision work
> when he cannot — and the decision boundary that comes with them are in
> [`docs/working-with-doug.md`](docs/working-with-doug.md), under *Standing rules*.**
>
> ### ⟶ THE LABS — order, standing constraints, and nothing about progress
>
> **Doug runs labs in compiler-phase order** *(2026-08-22)*: **dae-construction → matching (→
> matching-live) → blt-ordering → tearing → index-reduction → initialization → solve-lowering →
> events**, which is `the-concepts.md`'s own numbering. **That is a rule about sequence, not a
> report on position.**
>
> **The iteration loop, the gate traps and the one-lab-at-a-time rule are in
> [`docs/fixture-labs/README.md`](docs/fixture-labs/README.md)** — read before touching a lab.
>
> **`connect-expansion` was rewritten 2026-08-30 under the provoke-questions rules and again
> 2026-08-31 under code-grounding** — its opening walks `connections/mod.rs` and every code name is
> an `hrw://src` link. *(A fact about the artifact. Whether anyone has read it is not recorded, by
> the rule below.)*
>
> *(Retitled 2026-09-01. It read "THE WALK IS THE MODE", asserting a mode from 2026-08-21 that this
> session was not in — a rename, a charter and two sweeps — and it carried a "WHERE THE WALK IS"
> pointer naming the current and next lab. **That was walk-tracking in prose, which the rule below
> forbids in exactly those words.**)*
>
> **NOTHING TRACKS OR REPORTS WHAT HAS BEEN RUN, IN A MARKER OR IN PROSE** *(Doug, 2026-09-01)*:
> *"that discipline was turning education into a chore, including frequent pesters from you about
> the need to walk labs."* The `walked:`/`authored:` markers went on 2026-08-31 — *"that
> bookkeeping doesn't yield enough value"* — **and this section went on doing it by hand anyway**,
> keeping a backlog of which stops had no reader yet and surfacing it every session. That is the
> pestering, and it is what a session reads first. **Do not reintroduce it in either form**; judge
> from the conversation. **Doug runs a lab when he wants to, in whatever order he likes.**
>
> **`index-reduction.md` carries a harder bar**, staked in public: **index reduction explained to
> anybody with only basic calculus** — the bar is PREDICTION, not comprehension. That constraint,
> the provoke-questions rules and the one-lab-at-a-time rule are all in
> [`docs/fixture-labs/README.md`](docs/fixture-labs/README.md); read it before touching a lab.
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
> ## Open questions a lab session may hit
>
> - **A reproduced state-count inconsistency**, in `docs/upstream-issues.md`: `Drivetrain`'s index
>   reduction demotes nine states to three while solve lowering reports **9**, and
>   `GearWithBrake` shows the same gap. **Not diagnosed.** `solve-lowering.md` omits its natural
>   example rather than write around it. Needs a System Modeler adjudication (`#43`).
> - **RESOLVED 2026-09-02, and it was HRW's defect rather than Rumoca's.** `RcCircuit`'s one
>   `synthetic_root_conditions` with no `when` clause is correct: the field counts roots Rumoca
>   had to *synthesise* because no relation supplied one — here, over the MSL Resistor's
>   `R.T_heatPort`. **HRW published it as `zero_crossing_conditions`**, which reads as a claim
>   about all zero crossings; `BouncingBall` then showed **0**, a bouncing ball apparently
>   detecting no contact. **A renamed field is a claim, and no fidelity check could catch this
>   one because every value was right — the defect was in the label.** The upstream entry built
>   on it is retracted.
> - **`#77`** — a live lab needs three panes and the layout has two. **Largely resolved 2026-08-12**
>   (`docs/ideas.md` #77, `DECISIONS.md`); labs are **taller** now, which is the correct trade.
>   **What survives is only the genuine three-pane case** — HRW at half width beside VS Code — so
>   `matching-live.md` alone may still want a layout change; do not build one for the other eight.


---

## Running things

**The procedures are [`docs/running-things.md`](docs/running-things.md)** — gate commands, the
three suites and what each protects, the notebook content check, long-run safety, and the
diagnostic tells for a hung or slept run. **Follow it step by step rather than from memory.**

**What stays here, because it binds rather than instructs:**

- **The gate is green before every commit**, via the runner: `cargo run -p hrw --example gate`.
  It decides FAST, LAB or FULL from the working tree; `gate_policy` is the rule and has tests.
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
- **[`docs/compiler-phases/`](docs/compiler-phases/) — the closest thing that exists to Rumoca
  documentation.** *(Corrected 2026-09-01. It was described here as "Claude's teaching database"
  whose contents should be Doug's questions rather than Claude's explanations — which is not what
  it is, is not how it came to exist, and caused a session to propose deleting 8,650 lines of it.)*
  **Claude wrote these files for Doug BEFORE HRW existed**, and Doug copied them in; upstream
  Rumoca has no equivalent. Start at
  [`the-chain-of-problems.md`](docs/compiler-phases/the-chain-of-problems.md).
  **They are reference, and their maintenance trigger is the Rumoca version bump** — steps 6 and 7
  of [`docs/updating-rumoca.md`](docs/updating-rumoca.md), which own it. **Keep them.** Two tests
  in `src/doc_citations.rs` check that their 41 `crates/` citations still resolve, which is what
  makes a rebase surface what moved.
  **They describe Rumoca as of the last refresh and carry no provenance tags**, so between
  refreshes read them as a map rather than as a verified claim — the source is the arbiter.
  **`the-chain-of-problems.md` is cited from six places in `src/`**; do not move it.
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
- **[`docs/fixture-labs/`](docs/fixture-labs/) — labs that are *tests*, not explanations.**
  Versioned, unlike an **Answer** (`.hrw-bridge/answer.md`, gitignored). **Only justified because
  something runs them:** `fixture_lab_links_all_resolve` parses every link on every test run.
  Three rules, each bought with a defect, and the README carries what each cost:
  - **One lab per capability, narrow** — the scarce resource is Doug's attention per
    expectation, not his sessions (`docs/ideas.md` #49).
  - **An expectation must say WHERE to look** — he reported "nothing happened" at a stop
    correctly refused with the reason on screen, in the status bar the lab never named.
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
- **A new pipeline stage must be wired into ALL per-stage systems, and the roster is
  `StageKind::ALL` rather than any list written here.** Miss one and the stage is **silently
  half-present**. *(This named three systems until 2026-09-01 — stage-diff highlight, stage-file
  publishing, the notebook trace — and there were more. That is the exhaustive-list-written-when-
  there-were-fewer-things failure, in the one rule whose own text says the failure is silent.)*

  **Four are now checked, and each fails by name**: `arch_doc::the_stage_roster_matches_stagekind`
  (`architecture.md`'s pipeline table — written after `Dae` joined `StageKind::ALL` on 2026-08-03
  and the document went on describing a ten-stage pipeline), `bridge::focus_json_stage_files_match_constant`,
  `stage_tabs::no_compilation_stage_is_missing_from_the_tab_roster`, and
  `worker::manifest_stage_rosters_match_the_pipeline` for the notebook trace. **Add a stage, run the
  gate, and let the failures enumerate the work** — that is more reliable than any list in this
  file, including this one.


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
