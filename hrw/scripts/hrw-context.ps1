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

# **Whether HRW is RUNNING, which no age can tell you and which Claude kept inferring
# from one.** Every line in this block reports a file's age, and a bridge file outlives
# the process that wrote it -- so a focus.json written 54s ago reads as a live session
# and is equally consistent with an app that exited 53s ago. On 2026-11-05 Claude
# asserted "HRW is open" twice from exactly that, with `tasklist` available and already
# used earlier in the same session; Doug corrected it. The hook's own comment above
# records the *previous* version of this mistake -- two file ages conflated with each
# other -- so this is the same defect one step out: a file age conflated with a
# process.
#
# **It also decides whether the gate may run**, which is the other reason a session
# wants it without asking: the gate relinks hrw.exe and fails while the app is up.
$hrwUp = $null -ne (Get-Process -Name 'hrw' -ErrorAction SilentlyContinue)
if ($hrwUp) {
    Write-Output "$tag HRW IS RUNNING - close it before the gate, which relinks hrw.exe"
} else {
    Write-Output "$tag HRW is NOT running - every age below is a file's, not a live session's"
}

$where = @("mode: $($app.ui_mode)")
if ($app.tour) { $where += "tour: $($app.tour)" }
if ($app.model) { $where += "model: $($app.model)" }
if ($app.stage_tab) { $where += "stage: $($app.stage_tab)" }
Write-Output "$tag context #$($ctx.seq) - $($where -join ' - ')"

# **A capture older than the session is not this session's.** `seq` restarts at 0 on
# launch, so a leftover focus.json from a previous run would otherwise read as a
# recent capture. The session's own start is the first thing in the action trail.
$started = ($session.actions | Where-Object { $_.kind -eq 'session' } | Select-Object -First 1).at

# **ONE SOURCE PER CLAIM, and this line is the third version of that lesson.** The
# capture's age now comes from the action trail -- the SAME file as the context printed
# below -- because the previous version took the age from focus.json's mtime and the
# content from session.json, so the two lines were never about the same event.
#
# On 2026-11-06 they openly disagreed and neither of us noticed until Doug asked how
# Claude would know what "this" referred to: session.json described a prose selection
# with no stage, focus.json held `RcCircuit` at `Flatten`, and that file's mtime had
# been moved 28 minutes earlier by `cargo test` writing the live bridge. The writer is
# gone (see `bridge::BRIDGE_DIR`, redirected under `cfg(test)` the same day), but the
# conflation was a defect in its own right: a corruptible mtime was the only freshness
# signal beside the referent, so a stale capture and a fresh one read identically.
# **THESE ARE THE ACTUAL RECORDED KINDS, and the first draft of this block invented
# one.** It matched `kind -eq 'capture'`, which exists nowhere in the trail -- the word
# appears only inside a doc comment at `app.rs:2569`, and a grep hit in prose was read
# as a code fact. The bug was silent in the worst way: the line would have reported
# "NO capture" forever, a false absence replacing a merely-unreliable age.
# `the_hook_matches_real_action_kinds` now fails by name if any name here stops being a
# `record_action` literal in `src/`.
$pointKinds = @('point-at', 'point-at-selection', 'point-dropped')
$lastPoint = $session.actions | Where-Object { $pointKinds -contains $_.kind } | Select-Object -Last 1
$captureTime = $null
if ($lastPoint -and $lastPoint.at) {
    try   { $captureTime = [datetime]::Parse($lastPoint.at.Replace(' UTC', '')) }
    catch { $captureTime = $null }
}
$now = (Get-Date).ToUniversalTime()

if ($captureTime -and $lastPoint.kind -eq 'point-dropped') {
    # **A cleared point is not a stale point, and the difference decides the answer.**
    # `pointing at` below is whatever the snapshot held; if the last thing Doug did was
    # drop the point, there is no referent and a question's "this" is not in HRW.
    $capAge = Format-Age ([int]($now - $captureTime).TotalSeconds)
    Write-Output "$tag the point was CLEARED $capAge - nothing is being pointed at, so do not resolve 'this' from the context below"
} elseif ($captureTime) {
    $capAge = Format-Age ([int]($now - $captureTime).TotalSeconds)
    Write-Output "$tag pointed at $capAge via $($lastPoint.kind) - from the action trail, the same file as the context below"
} else {
    # **The trail is capped at 200 actions, so say that rather than claim nothing was
    # pointed at.** A long session can push an early point-at out, and "nothing was
    # captured" would be a claim of absence the evidence does not support.
    Write-Output "$tag NO point-at in the action trail (capped at 200 actions) - the context below may be startup state rather than something Doug pointed at"
}

# **focus.json is the DETAIL file, and its mtime is a CROSS-CHECK rather than a
# freshness claim.** It carries the node value and tracking paths the summary below
# does not, so it is what Claude opens for detail -- and if its mtime disagrees with
# the recorded capture, something other than a capture wrote it and reading it would
# describe that writer instead. That check is the one thing that would have caught
# 2026-11-06 on the day rather than two months later.
if (Test-Path $focusPath) {
    $focusTime = (Get-Item $focusPath).LastWriteTime.ToUniversalTime()
    $focusAge = Format-Age ([int]($now - $focusTime).TotalSeconds)
    if ($captureTime) {
        $skew = [math]::Abs((New-TimeSpan -Start $captureTime -End $focusTime).TotalSeconds)
        if ($skew -gt 120) {
            Write-Output "$tag WARNING focus.json ($focusAge) does NOT match the recorded capture - something else wrote it; do NOT read it as Doug's context"
        } else {
            Write-Output "$tag focus.json agrees with that capture ($focusAge) - safe to open for node detail"
        }
    } elseif ($started -and $focusTime -lt [datetime]::Parse($started.Replace(' UTC', ''))) {
        Write-Output "$tag focus.json ($focusAge) PREDATES this HRW session - leftover from a previous run, not context"
    } else {
        Write-Output "$tag focus.json written $focusAge but no capture is recorded - treat it as leftover, not as context"
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
