# Current work — the fidelity sweep, and working alongside it

**Purpose:** the step-by-step state of the fidelity sweep and what can be built alongside it.
**Status:** **live plan.** Delete it when the sweep's findings have landed in
`fidelity-plan.md` and `architecture.md`.
**Read when:** picking up mid-sequence and needing to know which step is next.

**This is a live plan, not a record.** Update it as steps complete; delete it when the sweep
is done and its findings have landed in `fidelity-plan.md` and `architecture.md`. It exists
because the plan spans a couple of days and would otherwise live only in a chat transcript.

Started 2026-07-31.

---

## The plan

| # | Step | State |
|---|---|---|
| 1 | **Stage C** — 53 models stratified by IR shape | ✅ **done** — 43 completed, **0 violations**, 10 aborts now explained |
| 2 | **Measure where the cost is** | ✅ **done** — it is HRW's compile path, not the checks; `--only-checks` proved it |
| 3 | **Decide** | ✅ **done** — CALIBRATE, do not optimise. Guards raised to 900 s / 10 GB; the worst model needs 529 s and 5,416 MB and passes cleanly |
| 4 | **The big run** — full corpus | ✅ **done** — 2,610 models, **0 violations**; 16 aborts under retry (checklist E) |
| 5 | **Triage** the findings — three categories, see below | **nothing to triage: zero violations.** Confirm after the retry |
| 6 | **Fix**, re-run to green | likely a no-op — see step 5 |
| 6b | **The verification pause** — [`verification-plan.md`](verification-plan.md), four items | **after 6, before 7** (agreed 2026-08-01) |
| 7 | **The oracle test** — design, run (`ideas.md` #43) | after 6b |
| 8 | **Test mode** (`ideas.md` #52) — somewhere to *look* at these reports | the payoff |
| 9 | **A reading path for HRW**, then a **structural pass on `app.rs`** | after 8, deliberately |

Steps 1-4 are the machine's time; steps 5-9 are ours.

### Why 9 comes last, rather than sooner (Doug, 2026-07-31)

HRW is now **33,964 lines across 33 modules** against Rumoca's 138,987 across 53 crates —
about a quarter, and no longer trivial. Doug: *"I'm definitely going to have to consider HRW
to be a subject of focused study, just like rumoca."*

But the complexity is **concentrated, not diffuse**: `app.rs` is 9,039 lines and `worker.rs`
5,668 — **43% of all HRW code in two files**. And unlike Rumoca's, much of it is *accidental*
rather than essential: a 9,000-line UI module is not inherent to what HRW does. So part of
the answer is not "study it harder" but "make it smaller".

**Both are deferred until after Test mode on purpose**, and the reason is a rule this project
already holds: `feedback-tech-debt-sweeps-serve-future-phases` — skip debt a later phase will
rewrite. Test mode touches `app.rs` and adds a fourth `UiMode`, so a structural pass or a
reading path written before it would be partly obsolete on arrival. CLAUDE.md already defers
splitting `central_panel_ui` for exactly this reason.

**The gap the reading path fills**: HRW has 19,117 lines of documentation across 64 files —
generous against 34k of code — but `architecture.md` is a 1,500-line *reference*. It answers
"how does X work", never "where do I start". Rumoca has `compiler-phases/` for that; HRW has
no equivalent.

### Why step 2 is worth doing before step 4

44 corpus models are ≥1,200 equations. At the 300 s timeout that is **3.7 hours of a
ten-hour run producing no data** — on exactly the models that stress the representation
hardest. Whether one check is superlinear decides if those hours are recoverable.

**Half an hour of measurement against a ten-hour run.** Stage C is already showing the
pattern: 5 timeouts in the first 21 models, all large systems.

### Step 5's three categories, which is the part that is not "fixing bugs"

Nine of the twelve violations F6-F9 produced on their first run were the **check's** fault,
not HRW's. So triage precedes fixing, and a violation means *something disagrees*, not
*HRW is wrong*:

| Category | Response |
|---|---|
| HRW misrepresents Rumoca | **a real bug** — fix HRW |
| The check is wrong | fix the check; the instrument is lying, not the subject |
| **Rumoca is odd, HRW rendered it faithfully** | **not a bug** — possibly an upstream finding |

Misclassifying either way is expensive. The third row is the one that needs care.

---

## When the big run finishes — the order matters

**Lettered, not numbered.** The plan table above uses numbers, and "step 6" was
genuinely ambiguous between the two on 2026-08-01. A number now always means the
plan; a letter always means this checklist.

Doug and Claude walk this together (agreed 2026-08-01). **Snapshot before anything else**: the
sweep cost hours, and every later step touches the same files.

**A. Confirm it actually finished** — not merely that the worker is gone between models.

```powershell
Get-Process fidelity_msl -ErrorAction SilentlyContinue      # expect nothing
(Get-Content C:/tmp/fid-full.csv | Measure-Object -Line).Lines - 1
Import-Csv C:/tmp/fid-full-memory.csv | Group-Object verdict | Select-Object Name, Count
```

**B. Snapshot the complete sweep, BEFORE any re-run.** A resume only appends, so it should not
be able to lose anything — but "should not" is a poor reason to skip a copy that costs seconds.

```powershell
$stamp = Get-Date -Format "yyyyMMdd-HHmm"
$dir = "C:/Users/dougd/rumoca-runs/complete-$stamp"
New-Item -ItemType Directory $dir -Force | Out-Null
Copy-Item C:/tmp/fid-full.csv, C:/tmp/fid-full-memory.csv, C:/tmp/fid-full.log $dir
```

**C. Promote to the repository and commit**, so git holds the artifact before anything else
happens to it.

```powershell
cd C:/Users/dougd/source/repos/rumoca/hrw
./scripts/promote-run.ps1 -Report C:/tmp/fid-full.csv -Profile C:/tmp/fid-full-memory.csv
git add hrw/docs/reports/msl-fidelity-* ; git commit -m "hrw: MSL fidelity report"
```

**D. Free memory.** Close Chrome; confirm rust-analyzer is still stopped. `ServiceShell` held
3.4 GB during the sweep — between them that is roughly 6 GB.

**E. Re-run for the aborted items.** Free-RAM aborts are retried with no flag; add the
timeouts explicitly, since those turned out to be partly environmental (529 s in isolation
against 901 s under load).

```powershell
./scripts/measure-fidelity.ps1 -ModelsFile C:/tmp/all-models.txt -Out C:/tmp/fid-full.csv -Profile C:/tmp/fid-full-memory.csv -RetryVerdicts 'aborted:free-ram','aborted:timeout'
```

**F. Re-promote and commit.** `scripts/promote-run.ps1` refuses to replace a larger report with a
smaller one, so a mistake here fails loudly rather than quietly.

**Then** triage — step 5 of the plan above, with its three categories.

## Pending — after the retry pass lands

**Log the model at the START of its test, not only on completion** (Doug, 2026-08-01).

Today the result line is written after a model finishes, so a model that hangs for 900 s
shows **nothing** — the one fact you want during a stall is the one you cannot see. Claude hit
this directly on 2026-07-31 trying to identify a stuck process, and had to reconstruct it
afterwards from the CSV.

Keep it to one line per model:

```powershell
Write-Host -NoNewline ("{0,4}/{1}  {2,-70} " -f $i, $count, $m)   # before starting
# ... run, watch ...
Write-Host ("{0,6} MB {1,7}s  {2}" -f $peakMB, $secs, $verdict)   # completes the line
```

The name appears the instant the model starts, and the result lands on the same line. A killed
run then leaves a visibly **incomplete** line marking exactly where it stopped — which the
two-line version would not give.

## The zero-contention work: the feature backlog

Doug has a long list of feature ideas to discuss and record. **That is the ideal work while a
sweep runs** — it needs no build, no binary, and no CPU, so it competes with nothing. Add to
`docs/ideas.md`.

## Working on HRW while the sweep runs

**The sweep does not need to be continuous.** It resumes from its two CSVs, skips everything
settled, and retries free-RAM aborts. So the intended rhythm is:

> **Run the sweep overnight. Develop during the day, with it stopped.**

That costs nothing, because resuming is free — and it avoids every problem below.

### What is actually blocked, and what is not

Only one thing is genuinely blocked: **rebuilding `target/release/examples/fidelity_msl.exe`**.
Windows will not overwrite a running executable — and more importantly, rebuilding mid-sweep
would mean **later models are checked by a different binary than earlier ones**, which
quietly corrupts the artifact. That is the real reason not to, not the file lock.

Everything else works:

| Task | While the sweep runs |
|---|---|
| Edit any source | **yes** |
| `cargo check`, `cargo clippy` | **yes** |
| `cargo test -p hrw --lib` | **yes** |
| `cargo build -p hrw --bin hrw`, run the app | **yes** — different artifact |
| Build `survey_msl` | yes |
| Documentation, design, planning | yes |
| **Rebuild `fidelity_msl`** | **NO** — stop the sweep first |

### The real cost is contention, not locking

A `cargo build` uses several GB and every core. Running one during the sweep can push free
RAM below the 3 GB floor and trip the watchdog — recoverable, since free-RAM aborts are
retried, but it wastes the model in flight.

So if you do build during a sweep, expect the odd `aborted:free-ram` and know it costs
minutes, not correctness.

### If you want genuinely parallel work

A separate git worktree with its own `target/` isolates the build entirely:

```powershell
cd C:\Users\dougd\source\repos\rumoca
git worktree add ..\rumoca-dev hrw
```

Costs disk and still competes for CPU and RAM. **Probably not worth it** given that stopping
and resuming the sweep is free — reach for it only if a sweep must run uninterrupted.

---

## Stopping and resuming the sweep

```powershell
# stop: Ctrl+C in its terminal, then make sure no worker survived
Get-Process fidelity_msl -ErrorAction SilentlyContinue | Stop-Process -Force

# resume: the identical command. It skips settled rows and retries free-RAM aborts.
cd C:\Users\dougd\source\repos\rumoca\hrw
.\scripts\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
    -Out C:\tmp\fid-full.csv -Profile C:\tmp\fid-full-memory.csv
```

Full procedure, including the `Start-Transcript` and `-ModelsFile` gotchas:
[`long-runs.md`](long-runs.md).

---

## Checking progress at any time

```powershell
(Get-Content C:\tmp\fid-full.csv | Measure-Object -Line).Lines - 1              # models done
Import-Csv C:\tmp\fid-full.csv | Where-Object outcome -eq 'violations'          # findings
Import-Csv C:\tmp\fid-full-memory.csv | Group-Object verdict | Select-Object Name, Count
```
