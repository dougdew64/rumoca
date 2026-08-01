# Run drivers

**Purpose:** the PowerShell drivers for HRW's long runs — what each does and how it is
invoked.
**Status:** reference. The scripts themselves carry the *why* in their header comments.
**Read when:** about to run a sweep, or about to change one of these. The step-by-step
procedure is [`../docs/long-runs.md`](../docs/long-runs.md); this page is only the inventory.

*Grouped here 2026-08-01. Both lived loose in `hrw/` beside `Cargo.toml`.*

| Script | Does |
|---|---|
| [`measure-fidelity.ps1`](measure-fidelity.ps1) | Runs F1-F9 **one model per process**, with a watchdog sampling free RAM and process size every 500 ms. Writes the fidelity report and a memory profile. |
| [`promote-run.ps1`](promote-run.ps1) | Copies a finished run's CSVs into `../docs/reports/` and writes the provenance sidecar, including the `not_checked` bound. |

**Invoke from `hrw/`, not from here:**

```powershell
cd C:\Users\dougd\source\repos\rumoca\hrw
.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt -Out ... -Profile ...
.\scripts\promote-run.ps1 -RunDir C:\Users\dougd\rumoca-runs\<run>
```

Both resolve their own paths from `$PSScriptRoot`, so they work from any working directory —
but the documented form is the one above.

## Things that bite

**`measure-fidelity.ps1` runs the RELEASE binary**, `../../target/release/examples/fidelity_msl.exe`,
which `cargo test` and a debug build never touch. It holds **compiled-in paths** — notably
`docs/reports/msl-survey.csv`. A release binary can therefore sit stale for days while the
whole suite passes. Rebuild before a sweep.

**Never run a sweep unbounded.** An unbounded 53-model run made this machine unusable and
forced a hard power-cycle (2026-07-31). One model per process is the bound, because **only
process exit releases memory** — a session rebuild releases what the session holds, not what
the allocator fragmented.

**The guard is on FREE RAM, not process size.** "The machine stays usable" is a free-RAM
property: a 10 GB process is fine with 20 GB free and fatal with 1 GB free. A ceiling that
cannot fire is indistinguishable from no guard.

**`powershell -File` collapses array arguments.** `-RetryVerdicts 'a','b'` arrives as the
single string `"a,b"`; only `-Command` binds arrays. `measure-fidelity.ps1` splits on commas
before reading the value, so it behaves identically either way — **this is load-bearing, not
tidying.** Without it the retry pass silently matches nothing, which is worse than omitting
the flag, since the one-element default would at least have matched.

## Their standing disposition

Both meet all three conditions of `../docs/tech-debt.md`'s second trigger — re-run
repeatedly, can fail silently, and have already produced defects only Doug caught. They are
**candidates for a rewrite in Rust** (`../docs/verification-plan.md` item 3), with one real
counter-argument recorded there: **a script is editable without a rebuild**, which mattered on
2026-08-01 when the fidelity binary was locked by a running sweep and the watchdog needed
fixing mid-run.

That item has a checkpoint rather than a commitment: re-ask whether it earns the time once the
earlier items land.
