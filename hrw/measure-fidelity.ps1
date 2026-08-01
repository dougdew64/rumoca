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
    [Parameter(Mandatory = $true)][string]$Models,
    [string]$Out         = "C:\tmp\fid-c.csv",         # the fidelity report (appended)
    [string]$Profile     = "C:\tmp\fid-memory.csv",    # the memory profile this produces
    [double]$MinFreeGB   = 3.0,
    [double]$MaxProcGB   = 5.0,
    [int]$SampleMs       = 2000,
    [int]$TimeoutSec     = 300
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
Write-Host "free RAM ${freeNow} GB; rust-analyzer holding ${raGB} GB resident"
if ($raGB -gt 1 -and $freeNow -lt 8) {
    Write-Host ""
    Write-Host "  rust-analyzer is holding ${raGB} GB and free RAM is ${freeNow} GB." -ForegroundColor Yellow
    Write-Host "  For a clean sweep of the heaviest models, stop it FIRST via:" -ForegroundColor Yellow
    Write-Host "      Ctrl+Shift+P -> 'rust-analyzer: Stop server'" -ForegroundColor Yellow
    Write-Host "  Do NOT kill the process - VS Code restarts it and re-indexes." -ForegroundColor Yellow
    Write-Host "  Restart afterwards with 'rust-analyzer: Restart server'." -ForegroundColor Yellow
    Write-Host ""
}

$list = $Models -split ',' | ForEach-Object { $_.Trim() } | Where-Object { $_ }
Write-Host "$($list.Count) models, one process each"
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
# `aborted:proc-ceiling` and `aborted:timeout` are properties of the MODEL, so
# they stay done: retrying them would just reproduce the same result.
$alreadyProfiled = @{}
$retryable = @()
Get-Content $Profile | Select-Object -Skip 1 | ForEach-Object {
    $parts = $_ -split ','
    if ($parts.Count -ge 4 -and $parts[3] -eq 'aborted:free-ram') {
        $retryable += $parts[0]
    } elseif ($parts[0]) {
        $alreadyProfiled[$parts[0]] = $true
    }
}
if ($retryable.Count -gt 0) {
    Write-Host "retrying $($retryable.Count) model(s) that aborted on free RAM, not on their own size"
    # Drop their rows so the retry writes a fresh verdict rather than a duplicate.
    $kept = @(Get-Content $Profile | Select-Object -First 1)
    $kept += Get-Content $Profile | Select-Object -Skip 1 | Where-Object { ($_ -split ',')[3] -ne 'aborted:free-ram' }
    $kept | Out-File $Profile -Encoding utf8
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

    $t0 = Get-Date
    $proc = Start-Process -FilePath $exe -PassThru -NoNewWindow -RedirectStandardOutput "$env:TEMP\fid-one.out" `
        -RedirectStandardError "$env:TEMP\fid-one.err" `
        -ArgumentList @("--models", $m, "--out", $Out, "--resume", "--max-models", "1", "--rebuild-every", "1")

    $peakMB = 0
    $verdict = "ok"
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

        if ($verdict -ne "ok") {
            Write-Host ("  KILL {0}: {1} (peak {2} MB, free {3} GB)" -f $m, $verdict, $peakMB, $free) -ForegroundColor Yellow
            try { $proc.Kill() } catch { }
            break
        }
    }
    $secs = [math]::Round(((Get-Date) - $t0).TotalSeconds, 1)
    "$m,$peakMB,$secs,$verdict" | Out-File $Profile -Append -Encoding utf8

    $flag = if ($verdict -eq "ok") { "" } else { "  <- $verdict" }
    Write-Host ("{0,3}/{1} {2,6} MB {3,6}s  {4}{5}" -f $i, $list.Count, $peakMB, $secs, $m, $flag)
}

if ($raGB -gt 1) {
    Write-Host "`nrust-analyzer was running during this sweep; if you stopped it, restart with" -ForegroundColor Cyan
    Write-Host "  Ctrl+Shift+P -> 'rust-analyzer: Restart server'" -ForegroundColor Cyan
}
Write-Host "`nmemory profile: $Profile"
Write-Host "fidelity report: $Out"
