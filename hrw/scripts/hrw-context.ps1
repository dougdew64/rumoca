<#
.SYNOPSIS
    Report HRW's assembled context, for the UserPromptSubmit hook.

.DESCRIPTION
    Prints a few lines describing what HRW currently has assembled, so a question
    like "what is this?" arrives with its referent already attached.

    THE PROBLEM IT SOLVES. HRW writes the capture to `.hrw-bridge/focus.json`, but
    nothing pushes that at Claude -- he has to think to read it. That makes an
    unprefaced question a coin-flip: usually he looks, sometimes he answers about
    the wrong subject, and the failure is silent. This turns "Claude must remember"
    into "Claude is always told".

.NOTES
    IT ALWAYS PRINTS SOMETHING, and that is deliberate rather than tidy. Every line
    carries the `[hrw-context]` tag, so its ABSENCE is evidence the hook is not
    running -- on a machine where it was never installed, say. Silence would be
    indistinguishable from "nothing is captured", which is the wrong-negative shape
    this repository treats as the error nobody catches. Same reasoning as
    `debug-state.json`'s `variables: null` meaning NOT FETCHED rather than none.

    IT READS `session.json`, NOT `focus.json`. The capture file is ~740 KB and
    parsing it on every prompt would tax every message Doug sends. `session.json`
    is ~7 KB and already carries the same summary -- `context.pointing_at`,
    `context.following`, the open tour and the UI mode. The capture's AGE comes
    from `focus.json`'s mtime, which needs no parse at all.

    `powershell`, not `pwsh`: pwsh is installed on neither of Doug's machines, and
    `CLAUDE.md` records the machine-switch that cost.
#>

$ErrorActionPreference = 'Stop'

# **UTF-8 out, or the tour name lies.** Windows PowerShell defaults its output encoding
# to the ANSI code page, so the ad hoc tour's label came through as "tour: ? Answer" on
# the hook's first live firing -- the sparkle replaced by a question mark. A channel
# built to carry the exact subject of a question may not mangle the subject's name.
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

$tag = '[hrw-context]'

# $CLAUDE_PROJECT_DIR is set by the harness; fall back to this script's grandparent
# so the script is runnable by hand for debugging.
$root = $env:CLAUDE_PROJECT_DIR
if (-not $root) { $root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot) }
$bridge = Join-Path $root 'hrw\.hrw-bridge'
$sessionPath = Join-Path $bridge 'diagnostics\session.json'
$focusPath = Join-Path $bridge 'focus.json'

if (-not (Test-Path $sessionPath)) {
    Write-Output "$tag HRW has not run in this clone (no session.json), so there is no assembled context."
    exit 0
}

try {
    $session = Get-Content $sessionPath -Raw -Encoding utf8 | ConvertFrom-Json
} catch {
    Write-Output "$tag session.json is unreadable: $($_.Exception.Message)"
    exit 0
}

$app = $session.app
$ctx = $app.context

# **Age comes from focus.json, which is written when a capture happens.**
# session.json refreshes on HRW's own cadence, so its mtime says how live the app
# is, not how old the point is -- and reporting the wrong one would make a stale
# capture look current, which is the whole failure this exists to prevent.
$age = 'never captured'
if (Test-Path $focusPath) {
    $seconds = [int]((Get-Date) - (Get-Item $focusPath).LastWriteTime).TotalSeconds
    $age = if ($seconds -lt 90) { "${seconds}s ago" }
           elseif ($seconds -lt 5400) { "$([int]($seconds / 60))m ago" }
           else { "$([int]($seconds / 3600))h ago" }
}

$where = @("mode: $($app.ui_mode)")
if ($app.tour) { $where += "tour: $($app.tour)" }
if ($app.model) { $where += "model: $($app.model)" }
if ($app.stage_tab) { $where += "stage: $($app.stage_tab)" }
Write-Output "$tag captured $age - context #$($ctx.seq) - $($where -join ' - ')"

if ($ctx.pointing_at) {
    $p = $ctx.pointing_at
    Write-Output "$tag pointing at: $($p.target)   ($($p.kind), $($p.stage), $($p.request))"
} else {
    Write-Output "$tag pointing at: nothing"
}

if ($ctx.following) {
    $f = $ctx.following
    Write-Output "$tag following: $($f.identifier)   ($($f.mentions) mentions)"
} else {
    Write-Output "$tag following: nothing"
}

# A refusal to emit is context too: it means the bar is showing a point Claude does
# not have, which is the one disagreement the Context Bar exists to make impossible.
if ($ctx.last_emission_error) {
    Write-Output "$tag NOT EMITTED - $($ctx.last_emission_error)"
}
