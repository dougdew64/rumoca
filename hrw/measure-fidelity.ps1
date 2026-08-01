# Run the fidelity checks ONE MODEL PER PROCESS, with a live watchdog.
#
# WHY: on 2026-07-31 an unbounded run made the machine unusable, and the
# chunked replacement was still unsafe — chunking bounds accumulation ACROSS
# chunks but not within one, and its free-RAM check ran BETWEEN chunks, which
# is the only moment that cannot help.
#
# So: one model per process (worst case is bounded by one model), and a
# watchdog that samples DURING the run and kills on either guard.
#
# GUARD ON FREE RAM, NOT PROCESS SIZE. "The machine stays usable" is a
# free-RAM property. A 10 GB process is fine with 20 GB free; a 6 GB process
# is fatal with 1 GB free. The process ceiling is only a secondary sanity net.
#
# An abort is a MEASUREMENT, not a failure: "this model needs more than the
# ceiling" is the fact we are trying to learn, and it is recorded as such.
#
#   .\measure-fidelity.ps1 -Models (Get-Content C:\tmp\stage-c.txt)

param(
    # `-Models` is a comma-separated list. For the full corpus use `-ModelsFile`
    # instead: 2,626 qualified names is roughly 130,000 characters and Windows
    # caps a command line near 32,000, so the big run CANNOT be passed inline.
    [string]$Models      = "",
    [string]$ModelsFile  = "",
    [string]$Out         = "C:\tmp\fid-c.csv",         # the fidelity report (appended)
    [string]$Profile     = "C:\tmp\fid-memory.csv",    # the memory profile this produces
    [double]$MinFreeGB   = 3.0,
    # Calibrated 2026-07-31, not guessed. The original 5 GB / 300 s were set
    # before anything was measured, and both were MARGINALLY too tight:
    # LightningSegmentedTransmissionLine needs 529 s and 5,416 MB and passes
    # cleanly given them — it missed the old ceiling by 300 MB. The three
    # memory aborts peaked at 5,241 / 5,434 / 7,728 MB, so 10 GB clears all of
    # them on a machine with ~14 GB free once rust-analyzer is stopped.
    [double]$MaxProcGB   = 10.0,
    # 500ms, not 2000. Measured 2026-08-01: ONEBIT grew from under the 10 GB
    # ceiling to 11.4 GB inside a single 2 s interval — roughly 0.7 GB/s — so the
    # watchdog allowed a 1.4 GB overshoot and the machine was briefly left with
    # ~1.2 GB free, below the floor that exists to keep the desktop responsive.
    # At 500 ms the overshoot is ~0.35 GB. The cost is four cheap samples a
    # second against a process doing seconds of work.
    [int]$SampleMs       = 500,
    [int]$TimeoutSec     = 900,
    # Once a model passes this, the watchdog starts reporting which phase it is
    # in, every ~30 s. Normal models never reach it.
    [int]$SlowNarrateSec = 60,
    # Per-model phase breakdowns accumulate here. Defaults beside the profile.
    [string]$PhaseLog    = "",
    # Which verdicts to re-attempt on a resume. `aborted:free-ram` is always
    # worth retrying; `aborted:timeout` is worth it on a QUIETER machine, but
    # is not retried by default or a genuinely unfinishable model would burn
    # the full timeout on every re-run forever.
    [string[]]$RetryVerdicts = @('aborted:free-ram')
)

# **Normalise -RetryVerdicts before anything reads it.**
#
# `powershell -File script.ps1 -RetryVerdicts 'a','b'` passes ONE string "a,b",
# not a two-element array — only `-Command` binds arrays properly. On
# 2026-08-01 that silently made the retry pass a no-op: nothing matched, the
# script reported nothing to do, and passing the flag was WORSE than omitting
# it, because the one-element default would have matched. Splitting here makes
# the script behave identically however it is invoked.
#
# (This block existed TWICE until 2026-08-01 — two near-identical comments and
# two copies of the split. Harmless, since splitting an already-split list is
# idempotent, which is exactly why nothing caught it. The hazard was a later
# edit landing on one copy.)
$RetryVerdicts = @(
    $RetryVerdicts |
        ForEach-Object { $_ -split ',' } |
        ForEach-Object { $_.Trim() } |
        Where-Object { $_ }
)

$exe = Join-Path $PSScriptRoot "..\target\release\examples\fidelity_msl.exe"
if (-not (Test-Path $exe)) { throw "build first: cargo build -p hrw --release --example fidelity_msl" }

function Get-FreeGB { [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 2) }

# --- pre-flight: report the memory situation, and name the ONE action that helps ---
#
# Killing rust-analyzer here would be worse than useless: VS Code treats an
# unexpected exit as a CRASH and restarts the server within seconds, so the
# only effect is paying the re-indexing cost again. Measured 2026-07-31 — a
# kill was undone in under two minutes, back to 5.7 GB.
#
# The durable stop is Command Palette -> "rust-analyzer: Stop server", which is
# an INTENTIONAL stop the client does not resurrect. That is a VS Code command,
# not reachable from a shell, so this reports rather than acts.
$freeNow = Get-FreeGB
$ra = Get-Process rust-analyzer -ErrorAction SilentlyContinue
$raGB = if ($ra) { [math]::Round((($ra | Measure-Object WorkingSet64 -Sum).Sum)/1GB, 2) } else { 0 }
# Free RAM always; rust-analyzer only when it is actually running and in the
# way. Reporting "holding 0 GB resident" on every run is noise, and the reminder
# to stop it now lives in docs/long-runs.md where it is read before starting.
Write-Host "free RAM ${freeNow} GB"
if ($raGB -gt 1 -and $freeNow -lt 8) {
    Write-Host ""
    Write-Host "  rust-analyzer is running and free RAM is ${freeNow} GB." -ForegroundColor Yellow
    Write-Host "  For a clean sweep of the heaviest models, stop it FIRST via:" -ForegroundColor Yellow
    Write-Host "      Ctrl+Shift+P -> 'rust-analyzer: Stop server'" -ForegroundColor Yellow
    Write-Host "  Do NOT kill the process - VS Code restarts it and re-indexes." -ForegroundColor Yellow
    Write-Host "  Restart afterwards with 'rust-analyzer: Restart server'." -ForegroundColor Yellow
    Write-Host ""
}

if ($ModelsFile) {
    if (-not (Test-Path $ModelsFile)) { throw "no such -ModelsFile: $ModelsFile" }
    # One name per line, or comma-separated — accept either.
    $Models = ((Get-Content $ModelsFile) -join ',')
}
if (-not $Models) { throw "give -Models 'a,b,c' or -ModelsFile <path>" }
if (-not $PhaseLog) { $PhaseLog = [IO.Path]::ChangeExtension($Profile, ".phases.txt") }

$list = $Models -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
# **"corpus", not a count of work.** This is the whole model list; how many
# will actually run is not known until the profile has been read, and is
# announced below. Printing a bare "2626 models" first read as the plan.
Write-Host "corpus: $($list.Count) model(s); one process each"
Write-Host "guards: free RAM >= ${MinFreeGB}GB, process <= ${MaxProcGB}GB, sampled every ${SampleMs}ms"

# Models already in the report are skipped by the runner's own --resume.
if (-not (Test-Path $Profile)) {
    "name,peak_ws_mb,secs,verdict" | Out-File $Profile -Encoding utf8
}
# Only a SETTLED verdict counts as done.
#
# `aborted:free-ram` says the ENVIRONMENT was tight, not that the model is too
# big — three models aborted that way on 2026-07-31 while rust-analyzer held
# 5.7 GB, then had ample room minutes later. Treating that as done would bake a
# transient machine state into the profile permanently, and those are exactly
# the heavy models the stratified corpus exists to exercise.
#
# `aborted:timeout` was ALSO thought to be a property of the model. It is not,
# and the measurement says so: LightningSegmentedTransmissionLine took 529 s
# run in isolation and 901.7 s during the full sweep — 70% slower under
# contention. So it is retryable, but only when asked
# (`-RetryVerdicts 'aborted:free-ram','aborted:timeout'`), because retrying it
# by default would burn the whole timeout on a genuinely unfinishable model
# every single re-run.
#
# `aborted:proc-ceiling` stays done: a model that wants more than the ceiling
# wants it regardless of what else is running.
$alreadyProfiled = @{}
$retryable = @()
Get-Content $Profile | Select-Object -Skip 1 | ForEach-Object {
    $parts = $_ -split ','
    if ($parts.Count -ge 4 -and $RetryVerdicts -contains $parts[3]) {
        $retryable += $parts[0]
    } elseif ($parts[0]) {
        $alreadyProfiled[$parts[0]] = $true
    }
}
if ($retryable.Count -gt 0) {
    # **Says what it MEASURES: rows cleared, not models to be run.**
    #
    # This used to read "retrying N model(s)", where N counted only rows still
    # carrying a retryable VERDICT. Models left pending by a previous pass are
    # absent from the profile entirely, so they were never counted — and on
    # 2026-08-01 it announced "retrying 1 model(s)" while sixteen were queued.
    # The count that matters is the "N model(s) to process" line below, which
    # counts both kinds.
    Write-Host "cleared $($retryable.Count) stale row(s) with verdict(s): $($RetryVerdicts -join ', ')"
    # Drop their rows so the retry writes a fresh verdict rather than a duplicate.
    $kept = @(Get-Content $Profile | Select-Object -First 1)
    $kept += Get-Content $Profile | Select-Object -Skip 1 | Where-Object { $RetryVerdicts -notcontains ($_ -split ',')[3] }
    $kept | Out-File $Profile -Encoding utf8
}

# **Announce what is actually left to do.**
#
# The "retrying N model(s)" line above only fires when rows carrying a retryable
# VERDICT are found. After a previous retry pass those rows have already been
# REMOVED from the profile, so the models are pending by ABSENCE rather than by
# verdict — nothing matches, nothing is announced, and a correct run looks
# exactly like a run with nothing to do. Doug killed one on 2026-08-01 for
# precisely that reason, and was right to: silence is not an acceptable way to
# say "16 models to process".
#
# This reports the real number either way, and says so up front.
$todo = @($list | Where-Object { -not $alreadyProfiled.ContainsKey($_) })
if ($todo.Count -eq 0) {
    Write-Host "nothing to do: all $($list.Count) model(s) already have a settled verdict" -ForegroundColor Green
} else {
    Write-Host ("{0} model(s) to process, {1} already done:" -f $todo.Count, $alreadyProfiled.Count) -ForegroundColor Cyan
    foreach ($t in ($todo | Select-Object -First 20)) { Write-Host "    $t" -ForegroundColor Cyan }
    if ($todo.Count -gt 20) { Write-Host "    ... and $($todo.Count - 20) more" -ForegroundColor Cyan }
}

$i = 0
foreach ($m in $list) {
    $i++
    if ($alreadyProfiled.ContainsKey($m)) { continue }

    $free0 = Get-FreeGB
    if ($free0 -lt $MinFreeGB) {
        Write-Host "STOPPING before '$m': only ${free0}GB free" -ForegroundColor Yellow
        break
    }

    # **Announce the model BEFORE running it**, not after.
    #
    # The result used to be written only on completion, so a model that hung for
    # 900 s showed nothing at all — the one fact wanted during a stall (which
    # model?) was the one that could not be seen, and identifying a stuck process
    # on 2026-07-31 meant reconstructing it from the CSV afterwards.
    #
    # `-NoNewline` keeps it to ONE line per model: the name appears the instant
    # the model starts and the result completes the same line. A killed run then
    # leaves a visibly incomplete line marking exactly where it stopped.
    Write-Host -NoNewline ("{0,5}/{1}  {2,-72} " -f $i, $list.Count, $m)

    $errFile = Join-Path $env:TEMP "fid-one.err"
    # Clear the phase marker so a stale one from the previous model
    # cannot be reported as this model's current phase.
    Remove-Item (Join-Path $env:TEMP "fid-phase.txt") -Force -ErrorAction SilentlyContinue
    $t0 = Get-Date
    $proc = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardOutput "$env:TEMP\fid-one.out" `
        -RedirectStandardError $errFile `
        -ArgumentList @("--models", $m, "--out", $Out, "--resume", "--max-models", "1", "--rebuild-every", "1")

    $peakMB = 0
    $verdict = "ok"
    # Negative, not zero: the 30 s gap below is a rate limit BETWEEN narrations,
    # and starting at 0 made it apply to the FIRST one as well — so nothing could
    # print until 30 s no matter what -SlowNarrateSec said. That was the second
    # bug in this one feature; the first was a path eaten by an escape.
    $lastNarrate = -99999
    $lastPhaseText = $null
    $phaseSince = $null
    while (-not $proc.HasExited) {
        Start-Sleep -Milliseconds $SampleMs
        try { $proc.Refresh() } catch { break }
        if ($proc.HasExited) { break }
        $ws = [math]::Round($proc.WorkingSet64 / 1MB, 0)
        if ($ws -gt $peakMB) { $peakMB = $ws }
        $free = Get-FreeGB
        $elapsed = ((Get-Date) - $t0).TotalSeconds

        if ($free -lt $MinFreeGB) { $verdict = "aborted:free-ram"; }
        elseif ($ws / 1024 -gt $MaxProcGB) { $verdict = "aborted:proc-ceiling" }
        elseif ($elapsed -gt $TimeoutSec) { $verdict = "aborted:timeout" }

        # **Say which phase a slow model is sitting in, while it is happening.**
        #
        # The Rust runner writes each stage marker to stderr as it completes and
        # flushes, so the tail of its error file is the phase currently running.
        # Reported once a model passes the slow threshold, then every ~30 s, so
        # normal models stay quiet and a hung one narrates itself. Previously a
        # model could burn 900 s showing nothing at all.
        $elapsedNow = ((Get-Date) - $t0).TotalSeconds
        if ($elapsedNow -ge $SlowNarrateSec -and ($elapsedNow - $lastNarrate) -ge 30) {
            $lastNarrate = $elapsedNow
            # Join-Path, not string interpolation: an earlier edit turned the "\f"
            # of "$env:TEMP\fid-one.err" into a literal formfeed byte, so PowerShell
            # read $env:TEMP followed by junk. -ErrorAction SilentlyContinue then
            # produced NOTHING instead of an error, and the narration silently
            # never fired. The silencer hid my own bug.
            # Read the phase file the runner writes directly. NOT the child's
            # stderr: HRW's OutputCapture redirects stderr during a compile, so
            # anything the callback prints is swallowed before it gets there.
            $phaseFile = Join-Path $env:TEMP "fid-phase.txt"
            $phase = if (Test-Path $phaseFile) { (Get-Content $phaseFile -Raw).Trim() } else { $null }
            # **Measure time-in-phase here, rather than trusting the marker.**
            #
            # The runner writes the marker ONCE when a phase starts, stamped with
            # seconds-since-compile-start. Rendered as "14s in" it read as
            # time-spent-in-this-phase and never moved, which is worse than
            # useless — it looked like a frozen clock. The watchdog knows when the
            # marker last CHANGED, so it can report the real duration.
            if ($phase -ne $lastPhaseText) {
                $lastPhaseText = $phase
                $phaseSince = Get-Date
            }
            # Prefer the marker's own start stamp: the runner records
            # seconds-since-compile-start when the phase BEGAN, so
            # elapsed-minus-that is the true time in phase, exact from the very
            # first narration. The watchdog's own observation is the fallback,
            # and it necessarily reads 0 the first time it sees a phase.
            #
            # (Off by the ~1.3 s MSL load, since the marker is relative to the
            # compile and $t0 is relative to process start. Immaterial against
            # the minutes this exists to measure.)
            $inPhase = if ($phase -match 'running,\s*([\d.]+)s in') {
                [math]::Max(0, $elapsedNow - [double]$Matches[1])
            } elseif ($phaseSince) {
                ((Get-Date) - $phaseSince).TotalSeconds
            } else { 0 }
            # Drop the runner's own "(running, Ns in)" suffix; it means
            # "started at N s", not "N s elapsed", and we now show the real thing.
            $phaseName = ($phase -replace '\s*\(running,.*$', '')
            if ($phase) {
                Write-Host ""
                Write-Host ("       {0,5:N0}s  in: {1}  ({2:N0}s in this phase)" -f $elapsedNow, $phaseName, $inPhase) -ForegroundColor DarkGray
                Write-Host -NoNewline ("{0,5}/{1}  {2,-72} " -f $i, $list.Count, $m)
            }
        }

        if ($verdict -ne "ok") {
            # Close the -NoNewline line before writing a full line of our own.
            Write-Host ""
            Write-Host ("       KILL {0}: peak {1} MB, free {2} GB" -f $verdict, $peakMB, $free) -ForegroundColor Yellow
            try { $proc.Kill() } catch { }
            break
        }
    }
    # **Preserve the phase breakdown before it is overwritten.** The child's
    # stderr file is replaced on every model, so each `[phases]` line survived
    # only until the next model started — the measurement was being taken and
    # thrown away, which is the exact defect this instrumentation was added to
    # fix. Appended to a durable per-run log instead.
    if (Test-Path $errFile) {
        $phaseLines = Select-String -Path $errFile -Pattern ([regex]::Escape('[phases]')) |
            ForEach-Object { $_.Line.Trim() }
        foreach ($pl in $phaseLines) { "$m  $pl" | Out-File $PhaseLog -Append -Encoding utf8 }
    }

    $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    "$m,$peakMB,$secs,$verdict" | Out-File $Profile -Append -Encoding utf8

    # Completes the line opened above. The name is already printed.
    $colour = if ($verdict -eq "ok") { "Gray" } else { "Yellow" }
    Write-Host ("{0,6} MB {1,7}s  {2}" -f $peakMB, $secs, $verdict) -ForegroundColor $colour
}

if ($raGB -gt 1) {
    Write-Host "`nrust-analyzer was running during this sweep; if you stopped it, restart with" -ForegroundColor Cyan
    Write-Host "  Ctrl+Shift+P -> 'rust-analyzer: Restart server'" -ForegroundColor Cyan
}
Write-Host "`nmemory profile: $Profile"
Write-Host "fidelity report: $Out"
