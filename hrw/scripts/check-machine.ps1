<#
.SYNOPSIS
  Verify this machine is ready to work on HRW. Run it after switching machines.

.DESCRIPTION
  Doug works on two machines and switches at least twice a week. Several things do
  NOT travel with a `git pull`, and the failures are quiet:

    - `.claude/settings.json` is gitignored BY UPSTREAM, so the permission allowlist
      is per machine. Without it every Bash call prompts for approval -- which during
      an unattended run, with nobody awake, is indistinguishable from a hang.
    - The VS Code bridge extension needs a build and a junction per machine.
    - The parsed-artifact cache is per machine and keyed on a fingerprint of
      `crates/`, so a first compile after switching re-parses the whole MSL.

  This script exists because those were documented rather than executed, and
  documentation is discovered probabilistically. Two of them were missed in one day
  on 2026-08-23 -- both found by Doug asking whether something would be found, not by
  any checker.

  BLOCKING checks fail the script (exit 1). ADVISORY checks report and do not.

.EXAMPLE
  # At a PowerShell prompt, from hrw/ -- this is all Doug types:
  .\scripts\check-machine.ps1

.EXAMPLE
  # From outside PowerShell (Claude's tools spawn a subprocess), or if a machine's
  # execution policy blocks the script:
  powershell -NoProfile -ExecutionPolicy Bypass -File hrw\scripts\check-machine.ps1

.NOTES
  `powershell`, NOT `pwsh` -- PowerShell 7 is not installed on either of Doug's
  machines, so `pwsh -File ...` fails with "not recognized" before the script runs.
  Found by testing the documented invocation rather than assuming it, 2026-08-23.

  And the wrapper form was first written into a HUMAN-facing instruction, which is a
  category error: it is what Claude needs, not what someone already at a PowerShell
  prompt would type. Doug: *"I don't understand why I would run a `powershell` command
  if I am already in PowerShell."* Two callers, two invocations -- say which is which.

  Written for Windows PowerShell 5.1: no ternaries, no null-coalescing, no `pwsh`-only
  cmdlets.
#>

[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$blocking = 0
$advisory = 0

function Report {
    param(
        [string]$Name,
        [ValidateSet('PASS', 'FAIL', 'WARN')] [string]$Status,
        [string]$Detail = '',
        [string]$Fix = ''
    )
    $colour = @{ PASS = 'Green'; FAIL = 'Red'; WARN = 'Yellow' }[$Status]
    Write-Host ("  {0,-5} {1}" -f $Status, $Name) -ForegroundColor $colour
    if ($Detail) { Write-Host "        $Detail" -ForegroundColor DarkGray }
    if ($Fix -and $Status -ne 'PASS') { Write-Host "        fix: $Fix" -ForegroundColor DarkGray }
    if ($Status -eq 'FAIL') { $script:blocking++ }
    if ($Status -eq 'WARN') { $script:advisory++ }
}

# Anchored at the repo root deliberately. A bare relative path reports the wrong
# answer from `hrw/`, which once had a check ordering a session to recreate a file it
# already had.
$repo = (git rev-parse --show-toplevel 2>$null)
if (-not $repo) {
    Write-Host 'FAIL  not inside a git repository' -ForegroundColor Red
    exit 1
}
$repo = $repo -replace '/', '\'
$hrw = Join-Path $repo 'hrw'

Write-Host ''
Write-Host "HRW machine check  --  $repo" -ForegroundColor Cyan
Write-Host ''

# ---------------------------------------------------------------- BLOCKING ----

$settings = Join-Path $repo '.claude\settings.json'
if (Test-Path $settings) {
    Report 'permission allowlist' 'PASS' "$settings"
}
else {
    Report 'permission allowlist' 'FAIL' `
        'every Bash call will prompt; during an unattended run that looks like a hang' `
        'hrw/docs/setup-windows.md section 8 has the file to create'
}

$running = @(Get-Process -Name 'hrw' -ErrorAction SilentlyContinue)
if ($running.Count -eq 0) {
    Report 'HRW not running' 'PASS' 'the full gate can build the binary'
}
else {
    Report 'HRW not running' 'FAIL' `
        "$($running.Count) instance(s) hold target\debug\hrw.exe" `
        'close HRW, or run the two gate targets separately (see CLAUDE.md, Running things)'
}

# --------------------------------------------------------------- ADVISORY ----

$cache = Join-Path $env:LOCALAPPDATA 'Rumoca\source-roots\parsed-files'
if (Test-Path $cache) {
    Report 'parsed-artifact cache' 'PASS' 'MSL parses are cached for this compiler fingerprint'
}
else {
    Report 'parsed-artifact cache' 'WARN' `
        'absent, so the first compile re-parses the whole MSL' `
        'nothing to do -- expect a slow first gate, and do not diagnose it as a hang'
}

$ext = Join-Path $env:USERPROFILE '.vscode\extensions\dougdew64.hrw-debugger-bridge-0.1.0'
if (Test-Path $ext) {
    Report 'VS Code bridge extension' 'PASS' 'junction present'
}
else {
    Report 'VS Code bridge extension' 'WARN' `
        'only matching-live.md needs it; the other tours run from HRW alone' `
        'hrw/docs/setup-windows.md section 6 -- npm install, npm run build, then the junction'
}

$dirty = @(git -C $repo status --porcelain --untracked-files=all)
if ($dirty.Count -eq 0) {
    Report 'working tree clean' 'PASS' ''
}
else {
    Report 'working tree clean' 'WARN' "$($dirty.Count) uncommitted change(s)" 'commit or stash before an unattended run'
}

# ------------------------------------------------------------------ VERDICT ----

Write-Host ''
if ($blocking -gt 0) {
    Write-Host "$blocking blocking problem(s). Fix before working, and before any unattended run." -ForegroundColor Red
    exit 1
}
if ($advisory -gt 0) {
    Write-Host "Ready. $advisory advisory note(s) above." -ForegroundColor Yellow
    exit 0
}
Write-Host 'Ready.' -ForegroundColor Green
exit 0
