# Promote a finished run's output into the repository as a deliverable.
#
# WHY THIS EXISTS: the runbook originally told Doug to write long-run output to
# C:\tmp. Those files cost HOURS to produce and are exactly the zero-adoption-cost
# artifacts docs/upstream-strategy.md wants to hand to maintainers — a temp
# directory is the wrong home for both reasons, and it sits among the scratch
# files Claude generates constantly.
#
# This copies (never moves) the finished CSVs into hrw/docs/ under names that
# cannot be confused with the small pre-commit test's output, and writes a
# provenance sidecar so the table can say what it describes.
#
#   .\promote-run.ps1 -RunDir C:\Users\dougd\rumoca-runs\2026-08-01-full
#   .\promote-run.ps1 -Report C:\tmp\fid-full.csv -Profile C:\tmp\fid-full-memory.csv

param(
    [string]$RunDir  = "",
    [string]$Report  = "",
    [string]$Profile = "",
    [switch]$Force
)

if ($RunDir) {
    if (-not $Report)  { $Report  = Join-Path $RunDir "fid-full.csv" }
    if (-not $Profile) { $Profile = Join-Path $RunDir "fid-full-memory.csv" }
}
if (-not $Report)  { throw "give -RunDir, or -Report and -Profile" }
if (-not (Test-Path $Report))  { throw "no such report: $Report" }

$docs = Join-Path $PSScriptRoot "docs"
$destReport = Join-Path $docs "msl-fidelity-report.csv"
$destMeta   = Join-Path $docs "msl-fidelity-report.meta.json"

$rows = @(Import-Csv $Report)
$n = $rows.Count
if ($n -lt 100 -and -not $Force) {
    throw "only $n rows - that looks like a partial or specimen run. Use -Force if you mean it."
}

# Refuse to replace a larger existing report with a smaller one unless forced:
# the likeliest accident is promoting a partial re-run over a complete sweep.
if (Test-Path $destReport) {
    $existing = @(Import-Csv $destReport).Count
    if ($n -lt $existing -and -not $Force) {
        throw "refusing to replace $existing rows with $n. Use -Force if that is intended."
    }
}

Copy-Item $Report $destReport -Force
$byOutcome = $rows | Group-Object outcome | ForEach-Object { '"{0}": {1}' -f $_.Name, $_.Count }

$profileNote = "null"
$verdicts = "{}"
if ($Profile -and (Test-Path $Profile)) {
    Copy-Item $Profile (Join-Path $docs "msl-fidelity-profile.csv") -Force
    $pv = @(Import-Csv $Profile) | Group-Object verdict | ForEach-Object { '"{0}": {1}' -f $_.Name, $_.Count }
    $verdicts = "{" + ($pv -join ", ") + "}"
    $profileNote = '"msl-fidelity-profile.csv"'
}

$rumoca = (Select-String -Path (Join-Path $PSScriptRoot "..\Cargo.toml") -Pattern '^version\s*=' | Select-Object -First 1).Line -replace '.*"(.*)".*','$1'
$meta = @"
{
  "generated_unix": $([int][double]::Parse((Get-Date -UFormat %s))),
  "generated_utc": "$((Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ'))",
  "models": $n,
  "outcomes": {$($byOutcome -join ", ")},
  "profile": $profileNote,
  "run_verdicts": $verdicts,
  "source_report": "$($Report -replace '\\','/')",
  "note": "F1-F9 over the MSL corpus. Establishes that HRW agrees with Rumoca, NOT that Rumoca is correct, and does not test the rendered UI."
}
"@
$meta | Out-File $destMeta -Encoding utf8

"promoted $n rows"
"  -> docs/msl-fidelity-report.csv"
if ($profileNote -ne "null") { "  -> docs/msl-fidelity-profile.csv" }
"  -> docs/msl-fidelity-report.meta.json"
""
"now commit them:  git add hrw/docs/msl-fidelity-*  && git commit"
