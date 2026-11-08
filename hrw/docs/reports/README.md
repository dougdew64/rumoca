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
| `msl-survey.csv` | `examples/survey_msl.rs` | **The corpus definition.** Rumoca's reach across every MSL model, plus the IR-shape metrics that stratify a sample. 2,791 models as of 2026-11-06, up from 2,626 when the root list was consolidated and `ModelicaReference` and `ObsoleteModelica4` entered it — the count is whatever the file holds, and nothing should spell it. **`n_equations` counts scalar equation rows since 2026-11-07** — each residual at its scalar size, each structured family at its row count, so 513 rows grew and none shrank; it read the residual arena before, which holds no family, and **`index_reduced` is measured again**, on singular raw systems only — `survey::tests::the_committed_survey_reduces_only_singular_systems` holds the artifact to that. |
| `msl-survey.meta.json` | same | provenance for the above |
| `msl-fidelity-report.csv` | `examples/fidelity_msl.rs`, promoted by `examples/promote_run.rs` | **The artifact.** Every check on the `fidelity::CHECKS` roster over the corpus — **2,780 of 2,791 models, all green (2026-11-07)**, the rest stopped by the memory ceiling and named in the sidecar. **The 11-07 run is the first with each model's reduced system as a subject** (`IndexReduction`, F2 against `prepare_for_solve` under `--max-reduce-eq`), and its CSV is **byte-identical** to 11-06's: an all-green row carries no subject count, so **nothing in this file can show how many reduced systems were checked** (`docs/tech-debt.md`, open item 8). The roster is the sidecar's `checks` field, never a range spelled here: this cell read `F1-F9` for three months, by which time F1 was retired, F8 was a measurement rather than a check, and F10 existed. |
| `msl-fidelity-profile.csv` | `scripts/measure-fidelity.ps1` | peak resident memory and wall time per model, plus the abort verdict |
| `msl-fidelity-report.meta.json` | `examples/promote_run.rs` | provenance, **including the `not_checked` bound** |
| `specimen-fidelity-report.csv` | the pre-commit test in `src/fidelity.rs` | the specimens named by `fidelity::MODELS` — a **subset** of `specimens/*.mo`, not all of them. **Churns on every full test run** — that is expected. |
| `specimen-oracle-report.csv` | `scripts/oracle.wls` | **System Modeler's verdict** on every specimen: `simulated`, `refused`, `import-failed`, `timeout`. The other side of every mismatch, and `every_specimen_has_an_oracle_verdict` fails by name when a new specimen has no row. **`secs` is wall-clock and unstable** — exclude it from a diff, exactly as `compile_cost` is excluded from the survey's; the first four columns were byte-identical across two consecutive runs. |

## Read at runtime — do not move or rename

Three paths are compiled in. Moving a file means editing code, not just documents:

| Path | Read by |
|---|---|
| `msl-survey.csv` | `examples/fidelity_msl.rs` (`corpus()`) — **the survey *is* the corpus definition**, so re-enumerating models anywhere else would be a second definition that drifts the moment MSL moves |
| `msl-survey.csv` | `examples/survey_msl.rs` — default `--out` |
| `specimen-fidelity-report.csv` | `src/fidelity.rs` — where the pre-commit test writes |

`examples/promote_run.rs` writes the three `msl-fidelity-*` files here.

## Rules

**Never write a corpus run directly into this directory.** Runs go to
`C:\Users\dougd\rumoca-runs\`, and `promote_run` copies them in — that is the step that
also writes the provenance sidecar and refuses to replace a larger report with a smaller one.
`fidelity_msl` **requires `--out`** for exactly this reason: a bare invocation used to default
into `docs/`, and on 2026-07-31 a profiling run overwrote a committed artifact that way.

**The oracle report is specimen-scale, which is why it may be written here directly.** The rule
above is about *corpus* runs — the thing that overwrote a committed artifact in 2026-07-31 was an
MSL-scale profiling run. `specimen-fidelity-report.csv` has always been written straight into this
directory by the pre-commit test, and `specimen-oracle-report.csv` follows it. **A corpus-scale
oracle run would not qualify** and would need the same `promote_run` treatment, including a
provenance sidecar. The run is still kept in `C:\Users\dougd\rumoca-runs\` as
`oracle-specimens-<stamp>.csv`, so the committed copy is never the only one.

**The bound travels with the data.** A table handed to a maintainer arrives without its
conversation, so whatever it does *not* cover has to be readable from the artifact itself.
`meta.json`'s `not_checked` field says which models were skipped and why — on 2026-11-06, **11
models** whose single process grew past what this machine can safely give one (`EngineV6` peaked
at 12.3 GB), with no timeouts and no free-RAM aborts. **Those are limits of the hardware and the
run configuration, not findings about HRW or Rumoca**, and the field says so in those words.

**The same 11 models were stopped by two independent runs that day**, byte-identical as a set — and by a third on 2026-11-07. A
deterministic stop is a property of those models, not a flaky machine — worth knowing before
anyone reads a changed `not_checked` list as noise.

**What these establish, and what they do not.** They establish that **HRW agrees with
Rumoca** — not that Rumoca is correct, and nothing at all about the rendered UI. That
distinction is the whole reason the oracle test (#43) is a separate report.
