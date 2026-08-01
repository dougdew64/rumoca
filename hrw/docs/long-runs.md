# Runbook — the long runs

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

**3. Build the binaries.**

```powershell
cd C:\Users\dougd\source\repos\rumoca
cargo build -p hrw --release --example survey_msl
cargo build -p hrw --release --example fidelity_msl
```

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
    --out hrw\docs\msl-survey.csv
```

**Output is byte-identical whatever the shard count** — slicing indexes the sorted name list
and the merge sorts — so a maintainer can regenerate and diff it.

**Part 2, optional.** Reduction is capped at 800 equations, leaving ~71 models with
`index_reduced = skipped:too-large`. To close that gap (**hours** — it includes the Spice3
models that once consumed 97 minutes between them):

```powershell
.\target\release\examples\survey_msl.exe --only-skipped --out hrw\docs\msl-survey.csv
```

Part 1 stands alone as a complete report if part 2 never runs; the bound is stated in the
data.

### Watching it

```powershell
# rows so far, across shards
(Get-ChildItem C:\tmp\part-*.csv | ForEach-Object {
    (Get-Content $_ | Measure-Object -Line).Lines - 1 } | Measure-Object -Sum).Sum

# anything the run itself flagged as anomalous
Select-String -Path C:\tmp\part-*.log -Pattern 'ANOMALY'
```

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

Start-Transcript -Path C:\tmp\fid-full.log

.\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
    -Out C:\tmp\fid-full.csv -Profile C:\tmp\fid-full-memory.csv

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
python -c "import csv,io; rows=list(csv.DictReader(io.open('docs/msl-survey.csv',encoding='utf-8'))); io.open('C:/tmp/all-models.txt','w',encoding='utf-8',newline='').write('\n'.join(sorted(r['name'] for r in rows))+'\n')"
```

**Interrupted?** Run the identical command again. It resumes from the two CSVs, skips
everything settled, and retries free-RAM aborts. Nothing is lost — rows are flushed as
produced.

### Watching it, from a second window

```powershell
(Get-Content C:\tmp\fid-full.csv | Measure-Object -Line).Lines - 1          # models done
Import-Csv C:\tmp\fid-full.csv | Where-Object outcome -eq 'violations'      # findings
Import-Csv C:\tmp\fid-full-memory.csv | Where-Object verdict -ne 'ok'       # aborts
Import-Csv C:\tmp\fid-full-memory.csv | Sort-Object {[int]$_.peak_ws_mb} -Descending |
    Select-Object -First 10                                                # heaviest models
```

### What the verdicts mean

| Verdict | Means | Do |
|---|---|---|
| `ok` | completed | nothing |
| `aborted:free-ram` | **the machine was tight**, not the model | retried automatically on the next run; close things or stop rust-analyzer |
| `aborted:proc-ceiling` | the **model** exceeded 5 GB | a finding — investigate that model |
| `aborted:timeout` | the **checks were slow on that model** — cause not yet measured | a finding; see `architecture.md` §11 "The checks themselves are expensive on large systems" before raising `-TimeoutSec` |

The guards are `-MinFreeGB 3` and `-MaxProcGB 5`, sampled every 2 s **during** the run.
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

## Afterwards

- Restart rust-analyzer: `Ctrl+Shift+P` → **`rust-analyzer: Restart server`**.
- Commit the regenerated `docs/msl-survey.csv` and its `.meta.json` together; the sidecar is
  what lets the table say which Rumoca and which MSL it describes.
- If a run was interrupted and resumed, the CSV is still sorted and deterministic — the
  merge sorts, and the fidelity report is written in corpus order.
