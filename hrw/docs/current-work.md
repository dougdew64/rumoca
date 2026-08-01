# Current work — the fidelity sweep, and working alongside it

**This is a live plan, not a record.** Update it as steps complete; delete it when the sweep
is done and its findings have landed in `fidelity-plan.md` and `architecture.md`. It exists
because the plan spans a couple of days and would otherwise live only in a chat transcript.

Started 2026-07-31.

---

## The plan

| # | Step | State |
|---|---|---|
| 1 | **Stage C** — 53 models stratified by IR shape | running |
| 2 | **Build + measure per-check timing** on a model that times out | code written, `cargo check` clean; **cannot build while stage C holds the exe** |
| 3 | **Decide**: fix a superlinear check, or accept the timeouts | needs step 2 |
| 4 | **The big run** — full corpus, overnight | needs step 3 |
| 5 | **Triage** the findings — three categories, see below | needs step 4 |
| 6 | **Fix**, re-run to green | |
| 7 | **Test mode** (`ideas.md` #52) — somewhere to *look* at these reports | the payoff |

Steps 1-4 are the machine's time; steps 5-7 are ours.

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
.\measure-fidelity.ps1 -ModelsFile C:\tmp\all-models.txt `
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
