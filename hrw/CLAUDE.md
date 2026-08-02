# CLAUDE.md — HRW Observatory

**Purpose:** the rules that bind, what is being worked on now, and where everything else lives.
**Status:** authority. **The one file to read at session start.**
**Read when:** every session, first.

Rust/egui observatory for studying the Rumoca Modelica compiler.
**[`docs/README.md`](docs/README.md) is the document index** — every file, its purpose, and
whether it is live. Go there rather than guessing.

Purpose, scope and binding decisions are in [`docs/CHARTER.md`](docs/CHARTER.md) (v1.1) —
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

**Instrumentation of the Rumoca crates is intended, and must stay additive,
observation-only, and upstreamable.** Across a crate boundary a phase's `pub(crate)` internals
are unreachable, so "accessing internals" means **additively widening visibility / adding
observation hooks in `../crates/rumoca-*`**. Semantics-preserving, so HRW stays faithful to
real Rumoca and rebases stay clean.

- **After touching a `crates/rumoca-*` file, run `cargo clippy -p <that-crate> --all-targets`.**
  Those crates are clippy-clean and `[workspace.lints]` denies; a lint the instrumentation
  introduces fails upstream CI.
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
testing debt"), which also records what `egui_kittest` genuinely cannot reach — **only two
surfaces**, `incidence_view.rs` cells and `spyplot.rs`; the animations *are* testable.
**Not growing the debt is free.**

**INSERT A TEST AFTER A FUNCTION'S CLOSING BRACE, never before its `fn` line.** A doc comment
and its attributes sit *above* the item, so anything placed between them is adopted by the
wrong one — the new test gets two `#[test]`s and **the old function silently stops being a
test.** Nothing fails; the suite goes green. This has bitten three times: it broke Doug's
debugger launch on 2026-07-31 and twice disabled
`a_broken_specimen_does_not_poison_the_next_compile` on 2026-08-01.
`doc_citations::no_function_has_two_test_attributes` now catches it.

**TAG A CLAIM OF ABSENCE, or it rots unnoticed.** The must-fire rule pointed at silence; this
is the same principle pointed at **absence**. When a document says something is not built,
tag it so the claim is checkable:

```markdown
Query axes over the corpus do not exist yet. <!-- unbuilt: survey_filter -->
```

`doc_citations::claims_of_absence_are_still_true` fails if the target **does** resolve.
**A wrong negative is the one error nobody catches**: acting on a wrong *positive* means
going to use the thing and finding it missing, while acting on a wrong *negative* means **not
looking**. Four stale ones were found on 2026-08-01, and `ideas.md` #42 was two days from
having its link vocabulary re-implemented on top of itself. Coverage is expected to be low —
tag when you write the claim, the way provenance tags work.

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
4. **← NEXT (2026-08-02): the UI pause** — *a lot* of automated UI tests first, then
   refactor `app.rs` into small, comprehensible pieces. **Doug's call**, and the ordering is
   his: tests before any refactoring, so regressions have something to hit.

   **Tonight's measurements, so tomorrow starts from facts rather than impressions:**

   | | |
   |---|---|
   | `app.rs` | **9,562 lines, 188 fns, 105 fields on `App`, 53 `&mut self` methods** |
   | biggest two | `central_panel_ui` **771**, `frame_ui` **727** — 16% of the file |
   | `ui_tests.rs` | 561 lines, **14 tests** against 17 `*_ui` fns |

   **The 105 fields are the problem, not the 9,562 lines.** Splitting `central_panel_ui`
   into ten functions that each take `&mut self` moves lines without reducing coupling:
   every one still reaches any of 105 fields, so nothing becomes independently testable
   and the next defect hides just as well. **Extract state, not just functions** — the
   unit that makes a pane comprehensible is the set of fields it owns.

   **Doug: *"we have been incredibly lucky."* The sharper reading is that we were not
   lucky — breakage went undetected.** The Context Bar was half-built from its first
   commit; the source view refused MSL models outright; the specimen list hid the corpus.
   Weeks each, all in daily use. That strengthens the case rather than weakening it:
   the risk being bought down is **invisible** breakage, which is why the tests come first.

   **Two traps, both hit today, both worse at scale:**
   - **A test written against current code encodes current behaviour, defects included.**
     The corpus test asserted the very hiding Doug then reported as a bug.
   - **A UI test can pass while checking nothing.** `test_set_specimen_files` was undone
     by the scratch poll, so "no specimen row is rendered" was true of an empty list.
     **At a hundred tests, vacuous ones are worse than no tests** — they manufacture the
     confidence that licenses the refactor. Every test earns its place by being seen to
     **fail against a deliberately broken version**; that is the only evidence that
     distinguishes the two.

   **The plan is [`docs/ui-pause-plan.md`](docs/ui-pause-plan.md)** — evidence per target,
   the four steps, and what the pause will not do.

   **Claude decides the tests, the seams and the timing** — `DECISIONS.md`, *"Claude is the
   primary consumer of HRW's code"*. The mandate is delegated **judgement, not purpose**:
   Claude's effectiveness is instrumental, Doug's understanding is the goal, and every
   refactoring commit must name the friction it removes so the delegation stays auditable.

5. **Then: one corpus list with a filter** (`docs/ideas.md` **#52**). One list widget
   over **three visible sources** — curated `specimens/`, scratch `.hrw-bridge/specimens/`, and
   the 2,626 MSL rows — with the reports as **columns and filter predicates**, never as separate
   views. **NOT a Test mode**, decided 2026-08-01: that loop *is* Specimen mode's loop, and
   `DECISIONS.md` records why "mode" was the wrong unit.

   **The filter is a prerequisite, not an enhancement** — 18 files need none, 2,644 do.
   **Merge the widget, not the corpora**: curated specimens have properties MSL rows do not, and
   the sources stay visibly distinct.

**[`docs/reports.md`](docs/reports.md) is the design authority for step 4.** Its load-bearing
claim: **survey → eligible, fidelity → trustworthy, oracle → findings**, joined on `name`.

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
- **One constraint survives because it is free:** *if* an oracle report is ever produced, it must
  emit the same `name` join key. That binds the **oracle's** design, not the filter's, and
  retrofitting it later would cost the join.

**A dependency the sequence used to hide, now met:**

- ~~The list needs a compile-by-qualified-name path in the worker.~~ ✅
  **`WorkerState::compile_model_by_name` exists** — built for step 2, since checking HRW's
  representation of an MSL model means compiling it *through HRW's own path*, which is the thing
  under test. Note **why it cannot just call `compile` with the library file**: a library file
  may declare many classes — `Blocks/Continuous.mo` holds `CriticalDamping` among others — so
  "the first class in the file" is the wrong model. The document is **located, not added**.

**The signal that dropping the mode was wrong**, recorded so it stays recognisable rather than
being rationalised away: a question that genuinely **cannot be expressed as a filter** over the
joined rows. That would mean something Test-mode-shaped was right after all, and it should
reopen `docs/ideas.md` #52.

---

## Running things

```text
cargo test -p hrw --lib -- --test-threads=1                        # ~8s,   431 tests — between edits
cargo test -p hrw --lib --features slow-tests -- --test-threads=1  # ~2min, 491 tests — before committing
cargo clippy -p hrw --all-targets                                  # covers the BIN; check the exit code
```

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
./scripts/measure-fidelity.ps1 -ModelsFile C:/tmp/all-models.txt `
    -Out C:/tmp/fid-full.csv -Profile C:/tmp/fid-full-memory.csv
```

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

**A LARGE-SCALE SWEEP IS OWED** — Doug runs it **the evening of 2026-08-02**.
Trigger 3 fired twice on 2026-08-01: the Parse stage of a library model went from
`{"classes":{},"within":null}` to its declaring file's full AST, and every
`Location` in that AST now carries the document URI instead of a basename.

**The point is not compliance.** Four F-checks walk `StageKind::COMPILATION`,
which begins with Parse — so they ran on all 2,626 MSL models in the last sweep
**and found nothing, because there was nothing there**. Those greens were vacuous
for that stage. This run is the first time Parse IR is really checked across the
corpus. Expect it to cost more than the last one (12 models already exceeded this
machine's limits), and treat new F7 violations as *information*, not regression:
the shape checks have never seen a whole library AST.

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
