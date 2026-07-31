# Run the fidelity harness in bounded chunks.
#
# WHY THIS EXISTS: on 2026-07-31 an unbounded 53-model run made Doug's machine
# unusable and forced a hard power-cycle. Each chunk here is a separate process
# that exits, so the OS reclaims everything unconditionally — a session rebuild
# inside the process cannot make that guarantee.
#
# It also refuses to start a chunk when free RAM is low, so the failure mode is
# "stops early and says so" rather than "takes the desktop down".
#
#   .\run-fidelity.ps1 -Out C:\tmp\fid.csv -Models (Get-Content C:\tmp\stage-c.txt)
#   .\run-fidelity.ps1 -Out C:\tmp\fid.csv -Limit 200 -ChunkSize 25

param(
    [string]$Out       = "C:\tmp\fidelity-report.csv",
    [string]$Models    = "",          # comma-separated; empty = whole corpus
    [int]$Limit        = 0,           # 0 = no limit
    [int]$ChunkSize    = 25,
    [int]$MinFreeGB    = 6,           # refuse to start a chunk below this
    [int]$MaxChunks    = 200
)

$exe = Join-Path $PSScriptRoot "..\target\release\examples\fidelity_msl.exe"
if (-not (Test-Path $exe)) { throw "build it first: cargo build -p hrw --release --example fidelity_msl" }

function Get-FreeGB {
    [math]::Round((Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory / 1MB, 1)
}

if (Test-Path $Out) { Remove-Item $Out -Force }
Write-Host "chunk size $ChunkSize, refusing to start below ${MinFreeGB}GB free"

for ($chunk = 1; $chunk -le $MaxChunks; $chunk++) {
    $free = Get-FreeGB
    if ($free -lt $MinFreeGB) {
        Write-Host "STOPPING: only ${free}GB free, below the ${MinFreeGB}GB floor." -ForegroundColor Yellow
        Write-Host "  Progress is in $Out; re-run to continue from there."
        break
    }

    $a = @("--out", $Out, "--max-models", $ChunkSize, "--resume")
    if ($Models) { $a += @("--models", $Models) }
    if ($Limit -gt 0) { $a += @("--limit", $Limit) }

    $before = Get-FreeGB
    & $exe @a 2>&1 | ForEach-Object {
        # Surface only the lines worth reading during a long run.
        if ($_ -match '^\[done\]|^\s+\[slow\]|^\s+\[rebuild\]|violations|nothing left') { Write-Host $_ }
    }
    $rc = $LASTEXITCODE
    $after = Get-FreeGB

    $rows = if (Test-Path $Out) { (Get-Content $Out | Measure-Object -Line).Lines - 1 } else { 0 }
    Write-Host ("chunk {0}: {1} rows total, free {2} -> {3} GB" -f $chunk, $rows, $before, $after)

    if ($rc -ne 0) {
        Write-Host "chunk exited $rc — stopping" -ForegroundColor Red
        break
    }
    # The runner prints this and exits 0 when the corpus is exhausted.
    if ($rows -gt 0 -and $script:lastRows -eq $rows) {
        Write-Host "no new rows this chunk — done." ; break
    }
    $script:lastRows = $rows
}

Write-Host "`nfinal: $((Get-Content $Out | Measure-Object -Line).Lines - 1) rows in $Out"
