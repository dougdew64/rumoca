# Generated reports

**Purpose:** the committed output of HRW's test runs — what is in each file, who writes it,
and which are safe to overwrite.
**Status:** reference. **The files themselves are load-bearing at runtime** — code reads three
of them by path.
**Read when:** before touching anything in this directory, or when a run has produced output
that needs promoting.

**How the three reports compose is [`../reports.md`](../reports.md)** — the design authority,
and the thing to read before building Test mode (#52) or the oracle test (#43). Its
load-bearing claim: **survey → eligible, fidelity → trustworthy, oracle → findings.** This
page is only the inventory.

*Grouped here 2026-08-01. These are generated data, not prose, and they were sitting loose
among the documents.*

## What is here

| File | Written by | Covers |
|---|---|---|
| `msl-survey.csv` | `examples/survey_msl.rs` | **The corpus definition.** Rumoca's reach across all 2,626 MSL models, plus the IR-shape metrics that stratify a sample. |
| `msl-survey.meta.json` | same | provenance for the above |
| `msl-fidelity-report.csv` | `examples/fidelity_msl.rs`, promoted by `promote-run.ps1` | **The artifact.** F1-F9 over the corpus — 2,614 of 2,626 models, all green (2026-08-01). |
| `msl-fidelity-profile.csv` | `measure-fidelity.ps1` | peak resident memory and wall time per model, plus the abort verdict |
| `msl-fidelity-report.meta.json` | `promote-run.ps1` | provenance, **including the `not_checked` bound** |
| `specimen-fidelity-report.csv` | the pre-commit test in `src/fidelity.rs` | the 16 curated specimens. **Churns on every full test run** — that is expected. |

## Read at runtime — do not move or rename

Three paths are compiled in. Moving a file means editing code, not just documents:

| Path | Read by |
|---|---|
| `msl-survey.csv` | `examples/fidelity_msl.rs` (`corpus()`) — **the survey *is* the corpus definition**, so re-enumerating models anywhere else would be a second definition that drifts the moment MSL moves |
| `msl-survey.csv` | `examples/survey_msl.rs` — default `--out` |
| `specimen-fidelity-report.csv` | `src/fidelity.rs` — where the pre-commit test writes |

`promote-run.ps1` writes the three `msl-fidelity-*` files here.

## Rules

**Never write a corpus run directly into this directory.** Runs go to
`C:\Users\dougd\rumoca-runs\`, and `promote-run.ps1` copies them in — that is the step that
also writes the provenance sidecar and refuses to replace a larger report with a smaller one.
`fidelity_msl` **requires `--out`** for exactly this reason: a bare invocation used to default
into `docs/`, and on 2026-07-31 a profiling run overwrote a committed artifact that way.

**The bound travels with the data.** A table handed to a maintainer arrives without its
conversation, so whatever it does *not* cover has to be readable from the artifact itself.
`meta.json`'s `not_checked` field says which models were skipped and why — currently three
model families that exceed this machine's memory or the run's time limit. **Those are limits
of the hardware and the run configuration, not findings about HRW or Rumoca**, and the field
says so in those words.

**What these establish, and what they do not.** They establish that **HRW agrees with
Rumoca** — not that Rumoca is correct, and nothing at all about the rendered UI. That
distinction is the whole reason the oracle test (#43) is a separate report.
