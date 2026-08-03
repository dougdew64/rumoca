# Runbook — the long runs

**Purpose:** copy-paste commands for the MSL survey and the fidelity sweep, what to watch,
how to resume, and what each abort verdict means.
**Status:** procedure. Follow it; do not re-derive it.
**Read when:** about to start either long run. **Never run the fidelity sweep unbounded** —
an unbounded run took Doug's machine down on 2026-07-31.

**Copy-paste procedures for the two runs that take minutes to hours.** Written to be used
without re-deriving anything. *Why* each precaution exists is in
[`architecture.md`](architecture.md) §11 "Running the checks at scale"; this page is the
*how*.

| Run | Cost | When |
|---|---|---|
| [MSL survey](#the-msl-survey) | ~11 min (6 shards) | after a Rumoca rebase; before an upstream PR |
| [Fidelity sweep](#the-fidelity-sweep) | 2-4 h (serial) | after a rebase; before a PR; when HRW changes how it emits or reads stage JSON |

Full trigger policy: [`fidelity-plan.md`](fidelity-plan.md) § "When these run".

---

## Before either run

**1. Use a standalone terminal.** Press `Win`, type **Windows Terminal** (or **PowerShell**),
open it.

> **Not VS Code's integrated terminal.** It dies with VS Code, and so does anything started
> from it — including a run you meant to leave overnight.

**2. Stop rust-analyzer.** In VS Code: `Ctrl+Shift+P` → **`rust-analyzer: Stop server`**.

> Frees ~5.7 GB here — more than two parallel workers need. **Do not kill the process
> instead**: VS Code treats an unexpected exit as a *crash* and restarts it within seconds,
> so you pay the re-index cost for nothing. The Command Palette stop is *intentional* and is
> not resurrected. Restart afterwards with **`rust-analyzer: Restart server`**.

**3. Build the binaries — `--release`, and do not skip this even if you "just built".**

```powershell
cd C:\Users\dougd\source\repos\rumoca
cargo build -p hrw --release --example survey_msl
cargo build -p hrw --release --example fidelity_msl
```

> **`scripts/measure-fidelity.ps1` runs `target/release/examples/fidelity_msl.exe`**, which `cargo
> test` and a debug build never touch. So a **release** binary can sit stale for days while
> everything else looks green — and it holds compiled-in paths, notably
> `docs/reports/msl-survey.csv`.
>
> This bit on 2026-08-01: moving the reports into `docs/reports/` updated the source and the
> debug build, the whole suite passed, and the *release* binary still carried the old path. The
> next sweep would have died with "run the survey first". **After any change to a compiled-in
> path, rebuild release before running a sweep** — `cargo build` is seconds against a run that
> is hours.

**4. Nothing heavy runs alongside it — including Claude.**

> The watchdog aborts a model when **free RAM drops below 3 GB**, and it samples *during* the
> run. So anything that takes memory while the sweep is going does not merely slow it: it
> **turns green models into `aborted:free-ram` rows**, and the run looks like it found
> something when it found contention.
>
> **`cargo test -p hrw --lib --features slow-tests` is the specific hazard**, because 49 of
> its tests compile specimens against the MSL. `cargo build` is the other — and rebuilding an
> example the run is executing fails outright with `LNK1104`, which at least announces itself.
>
> Added 2026-08-03. **Claude must not run the suite, a build, or a survey while a sweep is in
> progress**, and should say so rather than assume the constraint is understood.

---

## The MSL survey

Compiles every MSL model and records outcome plus IR shape. Sharded across processes;
**`Session` is not thread-safe, so parallelism is by process, never by thread.**

```powershell
cd C:\Users\dougd\source\repos\rumoca

# 6 shards in parallel (~11 min). Adjust 6 to taste; each worker peaks ~1.3 GB.
0..5 | ForEach-Object {
    Start-Process -NoNewWindow -FilePath .\target\release\examples\survey_msl.exe `
        -ArgumentList @("--slice", "$_/6", "--rebuild-every", "200",
                        "--out", "C:\tmp\part-$_.csv")
}

# Wait for all six to finish, then merge.
while (Get-Process survey_msl -ErrorAction SilentlyContinue) { Start-Sleep -Seconds 15 }

.\target\release\examples\survey_msl.exe --merge `
    C:\tmp\part-0.csv,C:\tmp\part-1.csv,C:\tmp\part-2.csv,C:\tmp\part-3.csv,C:\tmp\part-4.csv,C:\tmp\part-5.csv `
    --out hrw\docs\reports\msl-survey.csv
```

**The model set and every structural column are deterministic** — slicing indexes the sorted
name list and the merge sorts — so a maintainer can regenerate and diff it. **Two columns are
not**, and a diff must exclude them or it reports noise:

| Column | Why it varies | Verified |
|---|---|---|
| `compile_cost` | **wall-clock derived.** Fewer parallel shards means less contention means a model crosses the slow/fast threshold. | 2026-08-01: re-running at **4** shards against the committed **6**-shard artifact moved 16 models, **all `slow` → `fast`** — one direction, which is what load rather than randomness predicts. |
| `message` | **a known Rumoca non-determinism** — a cyclic-dependency chain is reported from a hash-ordered entry point, so the same cycle prints starting at a different member. [`upstream-issues.md`](upstream-issues.md) issue 3. | 6 models, all `StateGraph`-related. |

**So the honest claim is: same models, same structure, same outcomes — not the same bytes.**
*(Corrected 2026-08-01. The previous text promised byte-identity, which is the stronger claim
a maintainer would actually test, and it fails on 22 of 2,626 rows.)*

To diff two surveys meaningfully, drop the two unstable columns:

```powershell
function Get-Stable($p) {
    Import-Csv $p | Select-Object * -ExcludeProperty compile_cost, message |
        ConvertTo-Csv -NoTypeInformation
}
Compare-Object (Get-Stable a.csv) (Get-Stable b.csv)   # no output = agreement
```

**Part 2, optional.** Reduction is capped at 800 equations, leaving ~71 models with
`index_reduced = skipped:too-large`. To close that gap (**hours** — it includes the Spice3
models that once consumed 97 minutes between them):

```powershell
.\target\release\examples\survey_msl.exe --only-skipped --out hrw\docs\reports\msl-survey.csv
```

Part 1 stands alone as a complete report if part 2 never runs; the bound is stated in the
data.

**Part 2 is also the control group for the performance question** (`docs/ideas.md` #54). It
runs exactly the capped models **uncapped, through `Session` directly with no HRW extraction**,
so the difference against HRW's time on the same models is HRW's overhead, cleanly attributed.
Without it, comparing a capped survey against an uncapped HRW path attributes a Rumoca phase
to HRW — which is the error that produced, and required retracting, a "50-170x" figure.

### Watching it

```powershell
# rows so far, across shards
(Get-ChildItem C:\tmp\part-*.csv | ForEach-Object {
    (Get-Content $_ | Measure-Object -Line).Lines - 1 } | Measure-Object -Sum).Sum

# anything the run itself flagged as anomalous
# Anomalies THIS run only. The -Newer guard is not optional — see below.
$since = (Get-Date).AddHours(-6)
Get-ChildItem C:\tmp\part-*.csv.health.log | Where-Object LastWriteTime -gt $since |
    Select-String -Pattern 'ANOMALY'
```

**The time guard on the anomaly check is not decoration.** *(Corrected 2026-08-01 after the
old form was run and every hit it returned was from the previous day.)* Two traps compound:

- **Nothing cleans `C:\tmp\part-*` between runs.** Run 4 shards after a 6-shard run and
  `part-4.*` and `part-5.*` are still yesterday's, with no marker saying so.
- **The old pattern was `part-*.log`, which glob-matches `part-0.csv.health.log` as well as
  orphaned `part-0.log` files that nothing writes any more.** It appeared to work because the
  glob happened to catch the right files — alongside the wrong ones.

The survey writes its health log as **`<out>.health.log`**, i.e. `part-0.csv.health.log`. Match
that name explicitly and filter by time, or delete `C:\tmp\part-*` before starting.

**Never pipe a run through `tail`** — it buffers to EOF, so you see nothing until the end.
Read the CSVs; they are written and flushed per row.

---

## The fidelity sweep

Runs F1–F9 over the corpus, **one model per process**.

> **Never run this unbounded.** An unbounded 53-model run made this machine unusable and
> forced a hard power-cycle (2026-07-31). A session rebuild is *not* a memory bound — only
> process exit is.

```powershell
cd C:\Users\dougd\source\repos\rumoca\hrw

Start-Transcript -Path C:\Users\dougd\rumoca-runs\fid-full.log

.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
    -Out C:\Users\dougd\rumoca-runs\fid-full.csv -Profile C:\Users\dougd\rumoca-runs\fid-full-memory.csv

Stop-Transcript
```

Two things that are easy to get wrong:

- **`Start-Transcript`, not `>` or `Tee-Object`.** The script uses `Write-Host`, which does
  not go through the pipeline in PowerShell 5.1, so a redirect captures *nothing*.
- **`-ModelsFile`, not `-Models`.** 2,626 qualified names is ~130,000 characters against a
  Windows command-line cap near 32,000.

**If the corpus list is missing**, regenerate it from the survey:

```powershell
cd C:\Users\dougd\source\repos\rumoca\hrw
python -c "import csv,io; rows=list(csv.DictReader(io.open('docs/reports/msl-survey.csv',encoding='utf-8'))); io.open('C:/tmp/all-models.txt','w',encoding='utf-8',newline='').write('\n'.join(sorted(r['name'] for r in rows))+'\n')"
```

**Interrupted?** Run the identical command again. It resumes from the two CSVs, skips
everything settled, and retries free-RAM aborts. Nothing is lost — rows are flushed as
produced. **No flag is needed for that.**

**Recovering the aborts, on a quieter machine.** Close what is holding memory (Chrome,
rust-analyzer), then:

```powershell
# free-RAM aborts only — the default, no flag required
.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
    -Out C:\tmp\fid-full.csv -Profile C:\tmp\fid-full-memory.csv

# ALSO retry the timeouts, which are partly environmental
.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
    -Out C:\tmp\fid-full.csv -Profile C:\tmp\fid-full-memory.csv `
    -RetryVerdicts 'aborted:free-ram','aborted:timeout'
```

**Timeouts are worth retrying and are not retried by default**, which looks inconsistent
until you see both reasons. They *are* partly environmental —
`LightningSegmentedTransmissionLine` took **529 s in isolation and 901.7 s during the full
sweep**, 70% slower under contention. But retrying them automatically would burn the entire
900 s timeout on a genuinely unfinishable model on *every* re-run, forever. So: retryable,
by request.

`aborted:proc-ceiling` is never retried — a model that wants more than the ceiling wants it
regardless of what else is running.

### Watching it, from a second window

```powershell
(Get-Content C:\Users\dougd\rumoca-runs\fid-full.csv | Measure-Object -Line).Lines - 1          # models done
Import-Csv C:\Users\dougd\rumoca-runs\fid-full.csv | Where-Object outcome -eq 'violations'      # findings
Import-Csv C:\Users\dougd\rumoca-runs\fid-full-memory.csv | Where-Object verdict -ne 'ok'       # aborts
Import-Csv C:\Users\dougd\rumoca-runs\fid-full-memory.csv | Sort-Object {[int]$_.peak_ws_mb} -Descending |
    Select-Object -First 10                                                # heaviest models
```

### Recovering the aborted models — a second pass

**Run this in the same standalone PowerShell window as the sweep**, not from the editor.
Anything launched from VS Code is a child of the extension host and dies with it.

**1. Close the sweep's transcript**, if it is still open. It finalises the file — a transcript
copied while still open can be missing its tail.

```powershell
Stop-Transcript
```

**2. Snapshot the finalised log** alongside the CSVs already snapshotted.

```powershell
Copy-Item C:/tmp/fid-full.log C:/Users/dougd/rumoca-runs/complete-<stamp>/ -Force
```

**3. Free memory.** Close Chrome; leave rust-analyzer stopped. Both together are worth ~9 GB
here, which is what decides whether the memory-aborted models fit.

**4. Start a transcript for the retry**, kept separate from the sweep's so each run has its
own record.

```powershell
Start-Transcript -Path C:/Users/dougd/rumoca-runs/fid-retry.log
```

**5. Run the retry.**

```powershell
cd C:/Users/dougd/source/repos/rumoca/hrw
./scripts/measure-fidelity.ps1 -ModelsFile C:/tmp/all-models.txt -Out C:/tmp/fid-full.csv -Profile C:/tmp/fid-full-memory.csv -RetryVerdicts 'aborted:free-ram','aborted:timeout'
Stop-Transcript
```

It should print **`cleared N stale row(s) with verdict(s): …`**, then
**`N model(s) to process, M already done`**.

**Read the second line, not the first.** *(Corrected 2026-08-01 — this said to expect
`retrying N model(s)`, wording the script stopped using on 2026-08-01 precisely because it
undercounted, and then told you that its absence meant nothing matched. Following that
literally concludes the corpus is complete when it is not.)*

The `cleared` line counts **rows it deleted**, so it stays silent when models are pending by
*absence* — which is the normal state after an earlier retry pass already removed their rows.
**`N model(s) to process` counts both kinds and is the number that matters.** If *that* line
says `nothing to do`, the corpus really is complete.

**6. Promote and commit**, as in "Afterwards" below.

#### The `-File` array trap, which cost a wasted pass

`powershell -File script.ps1 -RetryVerdicts 'a','b'` passes **one string `"a,b"`**, not a
two-element array — only `-Command` binds arrays properly. On 2026-08-01 this silently made a
retry pass a no-op, and **passing the flag was worse than omitting it**, because the
one-element default `@('aborted:free-ram')` *would* have matched.

`scripts/measure-fidelity.ps1` now splits on commas before reading the value, so it behaves
identically however it is invoked. The trap is recorded because it applies to **any** array
parameter passed through `-File`, not just this one.

#### What each verdict means for a retry

| Verdict | Retried by default? | Why |
|---|---|---|
| `aborted:free-ram` | **yes** | a fact about the machine at that moment, not the model |
| `aborted:timeout` | only on request | partly environmental — 529 s isolated against 901 s under load — but retrying by default would burn the full timeout on an unfinishable model every run |
| `aborted:proc-ceiling` | **never** | a model wanting more than the ceiling wants it regardless of what else is running |

### What the verdicts mean

| Verdict | Means | Do |
|---|---|---|
| `ok` | completed | nothing |
| `aborted:free-ram` | **the machine was tight**, not the model | retried automatically on the next run; close things or stop rust-analyzer |
| `aborted:proc-ceiling` | the model wants more memory than this machine can safely give | **a stated bound, not a defect** — see below. Never retried. |
| `aborted:timeout` | **HRW's compile path** is slow on that model — measured, not the checks | expected on very large systems; see `architecture.md` §11 "Where the cost on large systems actually is" |

#### Some models cannot pass on this machine, and that is a stated bound

Measured 2026-08-01. The Spice3 family was observed above **11.4 GB and still climbing** when
the watchdog stopped it. With Chrome and rust-analyzer both closed the machine offers **12.9
GB free**, and the 3 GB floor that keeps the desktop responsive leaves a practical ceiling of
**~9.9 GB** — *less than the 10 GB already configured*.

So the headroom is exhausted: **the models want more than the hardware can safely provide.**
That is a limit of the machine, not a defect in HRW or Rumoca, and the promote step writes it
into the report's sidecar as `not_checked` so the bound travels with the data rather than
beside it.

It is also mildly unsafe to keep attempting them: at ~0.7 GB/s growth those runs briefly left
~1.2 GB free, which is the territory that hung the machine on 2026-07-31. `proc-ceiling` is
never retried, so this does not recur.

The guards are `-MinFreeGB 3`, `-MaxProcGB 10` and `-TimeoutSec 900`, sampled every **500 ms**
**during** the run — 500 ms rather than 2 s because `ONEBIT` grew 1.4 GB inside a single
2 s interval. **The last two were calibrated on 2026-07-31 rather than guessed**: the
original 5 GB / 300 s were set before anything was measured, and both were marginally too
tight — the worst timeout model needs 529 s and 5,416 MB, missing the old ceiling by 300 MB,
and passes cleanly with zero violations once given room. **Stop rust-analyzer first**, or 10
GB will not be available.
Guard on **free RAM, not process size**: "the machine stays usable" is a free-RAM property,
and a ceiling above what the machine has free can never fire.

---

## Staging — do not jump straight to the full run

Of the twelve violations F6–F9 produced on their first real run, **nine were the check's
fault, not HRW's.** Running thousands of models against unfixed checks yields a flood
dominated by the instrument's own bugs.

| Stage | Corpus | For |
|---|---|---|
| A | 10 curated specimens (the `slow-tests` suite) | baseline |
| B | ~20 MSL, spread | shapes the specimens lack |
| C | ~53 stratified by IR shape | the extremes, where bugs live |
| **D** | full corpus | the artifact |

**Fix what each stage finds before starting the next.** Stage C found the F8 materialisation
bug that stages A and B could not, because the triggering shape — 48 equations but 110
functions — existed in neither.

---

## Afterwards — promote the output, do not leave it lying around

**A finished sweep costs hours. Do not leave it in a working directory.**

```powershell
cd C:/Users/dougd/source/repos/rumoca/hrw
cargo run -p hrw --example promote_run -- --run-dir C:/Users/dougd/rumoca-runs
git add hrw/docs/reports/msl-fidelity-* ; git commit -m "hrw: MSL fidelity report"
```

`examples/promote_run.rs` **copies** (never moves) into `docs/reports/` as `msl-fidelity-report.csv` plus a
provenance sidecar, and **refuses to replace a larger report with a smaller one** unless
forced — the likeliest accident is promoting a partial re-run over a complete sweep.

**Three files, three different jobs — do not confuse them:**

| File | Scope | Committed? |
|---|---|---|
| `docs/reports/specimen-fidelity-report.csv` | 10 curated specimens, written by the pre-commit **test** | yes, and it churns |
| `docs/reports/msl-fidelity-report.csv` | the **full MSL corpus** — the artifact | yes, deliberately |
| `C:/Users/dougd/rumoca-runs/*` | in-progress and historical run output | no — durable working area |

**Run output goes to `C:/Users/dougd/rumoca-runs/`, never `C:/tmp`.** Temp directories get
cleaned by Windows, and Claude generates scratch files there constantly. The original version
of this runbook said `C:/tmp`, which put hours of work one cleanup away from gone.

## Afterwards

- Restart rust-analyzer: `Ctrl+Shift+P` → **`rust-analyzer: Restart server`**.
- Commit the regenerated `docs/reports/msl-survey.csv` and its `.meta.json` together; the sidecar is
  what lets the table say which Rumoca and which MSL it describes.
- If a run was interrupted and resumed, the CSV is still sorted and deterministic — the
  merge sorts, and the fidelity report is written in corpus order.
