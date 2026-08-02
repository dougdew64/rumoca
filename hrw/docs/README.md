# Document index

**Purpose:** the map of every HRW document — what it is for, whether it is live, and the
moment that should send you to it.
**Status:** index. Update it whenever a document is added, retired, or changes status.
**Read when:** you are looking for something and do not know where it lives; or you are about
to add a document and should check whether one already covers it.

Every document states its own purpose, status and reading moment in a three-line header, and
this index is the sorted view of those.

## Two audiences — check this before writing

**Most documents here are written for Claude.** The exception is READMEs and their *further
reading*, which are written for **Doug and for Rumoca maintainers**, and whose test is
whether a reader can finish the job **without asking Claude** (`../DECISIONS.md`,
2026-08-01).

| Marked | Audience | Standard |
|---|---|---|
| 👤 | Doug and Rumoca maintainers | self-sufficient — a reader must never need Claude |
| — | Claude | raw, detailed, nothing summarised away |

**Being listed in this index does not make a document human-facing.** This page links to
nearly everything so Doug can audit what exists; the 👤 mark is what carries the promise. A
README must let its reader finish **without following any link into an unmarked document**.

**And a 👤 document states facts it does not own by *reference*, never by transcription** —
counts and outcomes point at the generated artifact rather than repeating its numbers, because
human-facing prose is exactly what rots here. `end_to_end_tour.md` died asserting a 7×7 matrix
on a tab showing 48 equations.

## Status vocabulary

| Status | Means |
|---|---|
| **authority** | a decision or constraint that binds future work. Consult before designing; do not re-litigate. |
| **procedure** | how to perform a specific operation. Follow it; do not re-derive it. |
| **reference** | how something works. Look up; do not read end to end. |
| **live plan** | current work in flight. **Delete when its work lands.** |
| **record** | append-only history. Rarely rewritten, occasionally consulted. |
| **historical** | superseded. Kept for reasoning only — **do not follow its plan.** |

---

## Read first

| Document | Purpose |
|---|---|
| [`../CLAUDE.md`](../CLAUDE.md) | The rules, the current sequence, and pointers. **The one file to read at session start.** |
| 👤 [`../README.md`](../README.md) | The project README — **the only document written for a human arriving cold**, including Doug on a new machine. |

## Authorities — consult before designing

| Document | Binds |
|---|---|
| 👤 [`CHARTER.md`](CHARTER.md) | Purpose, scope, method, and the binding decisions. The most binding document here. |
| 👤 [`vision.md`](vision.md) | The north star, and the platform HRW is becoming. |
| [`fidelity-plan.md`](fidelity-plan.md) | What F1-F9 check, when they run, and the standing boundary against optimising HRW for test scope. |
| 👤 [`reports.md`](reports.md) | How the survey, fidelity and oracle reports compose. **Design authority for Test mode (#52) and the oracle test (#43).** |
| [`upstream-strategy.md`](upstream-strategy.md) | How engaging Rumoca's maintainers serves Doug's education, and the planning rules that follow. |
| [`working-with-doug.md`](working-with-doug.md) | Who Doug is, how he learns, and the standing working agreements. **All of it lived only in Claude's memory until 2026-08-01**, and memory does not survive a clone. |
| [`identity-and-provenance.md`](identity-and-provenance.md) | No heuristic name-matching; identity vs membership; what provenance Rumoca preserves. **Cited by six source files.** |
| [`tech-debt.md`](tech-debt.md) | The two sweep triggers, the tour-holes table, and the outstanding debt. |
| [`provenance.md`](provenance.md) | How Claude marks what it verified from what it inferred. |

## Procedures — how to do a specific thing

| Document | For |
|---|---|
| 👤 [`setup-windows.md`](setup-windows.md) | A fresh Windows machine → running HRW → live-trace debugging. |
| 👤 [`long-runs.md`](long-runs.md) | The MSL survey and the fidelity sweep, including the retry pass. **Never run the sweep unbounded.** |
| 👤 [`updating-rumoca.md`](updating-rumoca.md) | Rebasing the `hrw` branch on upstream. |

## Live plans — delete when their work lands

| Document | Covers |
|---|---|
| [`current-work.md`](current-work.md) | The fidelity sweep and the sequence around it. |
| [`verification-plan.md`](verification-plan.md) | The six-item pause: must-fire tests, the stale-negative test, clearing clippy, a faster suite, headless UI testing, Rust drivers. |
| [`source-tooling-plan.md`](source-tooling-plan.md) | **Part live.** Phases 1-5 delivered; **Phases 6 (tree rework) and 7 (canvas views) are unbuilt design work.** Read before touching the IR tree or a canvas view. |

## Reference — look things up

| Document | Describes |
|---|---|
| 👤 [`architecture.md`](architecture.md) | How HRW works, including §11 the testing architecture and the scale/safety rules. |
| [`context-assembly.md`](context-assembly.md) | The capture design — how a question carries its context to Claude. **Delivered**; kept for the reasoning. |
| [`debug-set-sites.md`](debug-set-sites.md) | IR field → the Rumoca line that assigns it, for arming a breakpoint. |
| [`compiler-phases/`](compiler-phases/) | **Mixed, deliberately.** Its 👤 [`README.md`](compiler-phases/README.md) and 👤 [`the-chain-of-problems.md`](compiler-phases/the-chain-of-problems.md) are for a human — why the pipeline has this shape. Everything below them is **Claude's teaching database**, raw and unsummarised, with untagged prose treated as a lead rather than a fact. |

## Records — append-only

| Document | Holds |
|---|---|
| [`ideas.md`](ideas.md) | The numbered backlog, #1-#57. Candidates, not commitments; numbers are permanent. |
| [`../DECISIONS.md`](../DECISIONS.md) | Every nontrivial implementation choice, plus the closed-arc record. |
| [`question-ledger.md`](question-ledger.md) | Doug's questions verbatim, and what made each click. **The only artifact whose value grows with time.** |
| [`upstream-issues.md`](upstream-issues.md) | Rumoca bugs, written ready to file. **Claude never files them.** |
| 👤 [`fixture-tours/`](fixture-tours/) | Tours that are *tests*. One per capability, narrow, with violable expectations. |
| 👤 [`specimen-notebook/`](specimen-notebook/) | Per specimen: a generated `trace/` and a hand-written `purpose.md`. |
| 👤 [`reports/`](reports/) | **Generated data, not prose** — the survey, the corpus fidelity artifact and its profile, the specimen report. Explained by [`reports.md`](reports.md); inventoried by [`reports/README.md`](reports/README.md). |

## Data HRW reads at runtime — do not move

Load-bearing paths, not documents. Moving any of them breaks the app or its tests.

| Path | Used by |
|---|---|
| `fixture-tours/*.md` | the tour picker, and `fixture_tour_links_all_resolve` |
| `fixture-tours/notebooks/` | cross-platform tour stops |
| `specimen-notebook/<Model>/purpose.md` | the Purpose tab |
| `specimen-notebook/<Model>/trace/` | the durable per-stage IR |
| **`reports/msl-survey.csv`** | `fidelity_msl::corpus()` and `survey_msl` — **the corpus definition** |
| **`reports/specimen-fidelity-report.csv`** | written by the pre-commit test in `src/fidelity.rs` |
| `reports/msl-fidelity-*` | written by `examples/promote_run.rs`; the committed artifact |

**Three of those are compiled-in paths**, so moving a report means editing code. See
[`reports/README.md`](reports/README.md).

`src/doc_citations.rs` scans `../CLAUDE.md`, `../README.md`, `../DECISIONS.md` and **all of
`docs/` recursively**, so a document moved into a subdirectory is still checked.

## Historical — reasoning only, do not follow

| Document | Status |
|---|---|
| [`history/answer-platform-plan.md`](history/answer-platform-plan.md) | Retired 2026-08-01. Its live items were moved out first — the file lists where each went. Kept for the "features are experimentable, stored prose is not" correction. |

**Deleted 2026-08-01: `compiler-phases/end_to_end_tour.md`** (1,071 lines). HRW stopped showing
it on 2026-07-29, but it stayed inside the teaching database where a later session would read
it as authoritative — and its twelve stop-by-stop walkthroughs had rotted (Stop 8 described a
7×7 incidence matrix on a tab that shows 48 equations). Its conceptual sections, which make no
claim about what is on screen, are now
[`compiler-phases/the-chain-of-problems.md`](compiler-phases/the-chain-of-problems.md).
Recoverable from git history.
