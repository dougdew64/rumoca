# Run drivers

**Purpose:** the PowerShell drivers for HRW's long runs — what each does and how it is
invoked.
**Status:** reference. The scripts themselves carry the *why* in their header comments.
**Read when:** about to run a sweep, or about to change one of these. The step-by-step
procedure is [`../docs/long-runs.md`](../docs/long-runs.md); this page is only the inventory.

*Grouped here 2026-08-01. Both lived loose in `hrw/` beside `Cargo.toml`.*

| Script | Does |
|---|---|
| [`check-machine.ps1`](check-machine.ps1) | **Run after switching machines.** Verifies what a `git pull` does not bring: the permission allowlist, whether HRW holds `hrw.exe`, the parsed-artifact cache, the bridge extension. Blocking problems exit 1 and name their fix. Invoke with `powershell -NoProfile -ExecutionPolicy Bypass -File …` — **`pwsh` is not installed.** |
| [`measure-fidelity.ps1`](measure-fidelity.ps1) | Runs F1-F9 **one model per process**, with a watchdog sampling free RAM and process size every 500 ms. Writes the fidelity report and a memory profile. |
| ~~`promote-run.ps1`~~ | **Moved to Rust 2026-08-01** — `cargo run -p hrw --example promote_run`. See *The split* below. |

**Invoke from `hrw/`, not from here:**

```powershell
cd C:\Users\dougd\source\repos\rumoca\hrw
.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt -Out ... -Profile ...
cargo run -p hrw --example promote_run -- --run-dir C:\Users\dougd\rumoca-runs\<run>
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

## The split — why one moved to Rust and one did not

`verification-plan.md` item 3 asked whether both drivers should become Rust binaries, and its
checkpoint said to re-ask on newer evidence. Answered 2026-08-01: **they are not the same
case.**

| | `promote-run` → **Rust** | `measure-fidelity.ps1` → **stays** |
|---|---|---|
| Needs a memory-sampling crate | no | **yes** — a dependency needing Doug's approval |
| Mid-run editability matters | no — runs for seconds, after the sweep | **yes** — the watchdog was fixed mid-run on 2026-08-01 while the binary was locked |
| Writes a **published claim** | **yes** — the sidecar's `not_checked` sentence | no |

**The published claim is what decided it.** That sentence travels to a maintainer *with* the
table and is read as fact, and this project's rule is *speed on actions, **care on records***.
Its logic — the two guards and the bound — now lives in `src/promote.rs` with tests; the
driver is `examples/promote_run.rs`.

**The watchdog keeps its recorded reason.** It is verified only by being run, which is a known
gap rather than an exemption — see the must-fire audit in `../docs/verification-plan.md`.

**One condition of the trigger genuinely weakened.** "Re-run repeatedly" was true while sweeps
were daily; the corpus is now green and the suite runs after a rebase, before a PR, or when
stage-JSON emission changes — a few times a year. That is the argument against converting the
larger script, and it is honest to say so rather than convert on principle.
