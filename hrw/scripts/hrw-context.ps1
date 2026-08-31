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

function Format-Age([int]$seconds) {
    if ($seconds -lt 90) { return "${seconds}s ago" }
    if ($seconds -lt 5400) { return "$([int]($seconds / 60))m ago" }
    return "$([int]($seconds / 3600))h ago"
}

# **BOTH freshnesses, because they are different and the first version conflated
# them.** It took the age from focus.json (written on capture) and the CONTENT from
# session.json -- and session.json is written only when HRW records an *action*, so
# after a launch with no clicking it still describes the app at startup. The line
# read "captured 8m ago" over state that was minutes stale, which is precisely the
# stale-capture-looking-current failure this was built to prevent, wearing the
# disguise of the fix. Found 2026-08-30 while diagnosing why the 🎯 button did
# nothing: session.json's mtime never moved, which was the evidence.
$sessionAge = Format-Age ([int]((Get-Date) - (Get-Item $sessionPath).LastWriteTime).TotalSeconds)
Write-Output "$tag state as of $sessionAge - session.json is rewritten only when HRW records an action"

$where = @("mode: $($app.ui_mode)")
if ($app.tour) { $where += "tour: $($app.tour)" }
if ($app.model) { $where += "model: $($app.model)" }
if ($app.stage_tab) { $where += "stage: $($app.stage_tab)" }
Write-Output "$tag context #$($ctx.seq) - $($where -join ' - ')"

# **A capture older than the session is not this session's.** `seq` restarts at 0 on
# launch, so a leftover focus.json from a previous run would otherwise read as a
# recent capture. The session's own start is the first thing in the action trail.
$started = ($session.actions | Where-Object { $_.kind -eq 'session' } | Select-Object -First 1).at
if (Test-Path $focusPath) {
    $focusTime = (Get-Item $focusPath).LastWriteTime
    $focusAge = Format-Age ([int]((Get-Date) - $focusTime).TotalSeconds)
    if ($started -and $focusTime.ToUniversalTime() -lt [datetime]::Parse($started.Replace(' UTC', ''))) {
        Write-Output "$tag focus.json ($focusAge) PREDATES this HRW session - nothing has been captured since launch"
    } else {
        Write-Output "$tag focus.json written $focusAge"
    }
} else {
    Write-Output "$tag no focus.json - nothing has ever been captured in this clone"
}

if ($ctx.pointing_at) {
    $p = $ctx.pointing_at
    # **A null field is SAID, not left blank.** A tour passage has no stage, and the
    # first version interpolated the null straight into the line -- "(tour passage in
    # connect-expansion, , Explain)". Two commas with nothing between them read as a
    # value that failed to render, not as a deliberate absence, which is the one
    # distinction this whole channel exists to keep. Same rule as `kind: "none"` in
    # focus.json and `variables: null` in debug-state.json.
    $stage = if ($null -eq $p.stage) { 'no stage - prose, not a phase' } else { $p.stage }
    Write-Output "$tag pointing at: $($p.target)   ($($p.kind), $stage, $($p.request))"
} else {
    Write-Output "$tag pointing at: nothing"
}

if ($ctx.following) {
    $f = $ctx.following
    # Same shape, and it would have bitten identically: `mentions` is null until the
    # tracking summary has been computed, which would have printed "( mentions)".
    $mentions = if ($null -eq $f.mentions) { 'not yet counted' } else { "$($f.mentions) mentions" }
    Write-Output "$tag following: $($f.identifier)   ($mentions)"
} else {
    Write-Output "$tag following: nothing"
}

# A refusal to emit is context too: it means the bar is showing a point Claude does
# not have, which is the one disagreement the Context Bar exists to make impossible.
if ($ctx.last_emission_error) {
    Write-Output "$tag NOT EMITTED - $($ctx.last_emission_error)"
}
