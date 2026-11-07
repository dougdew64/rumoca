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
    `context.following`, the open tour and the UI mode.

    THE TWO FILES ARE WRITTEN BY DIFFERENT MECHANISMS, and keeping them in step is
    the whole correctness of this hook. `focus.json` is written by `bridge::write`
    whenever the context changes; `session.json` only at the end of a frame in which
    an action was RECORDED. Since 2026-11-07 `bridge::write` records a
    `context-emitted` action itself, so every context change moves both files in the
    same frame. Before that, the Question Bar's x buttons recorded nothing, and this
    hook went on reporting a point Doug had cleared.

    So: the point's AGE comes from the last point gesture in the trail; whether a
    point EXISTS comes from the snapshot; and `focus.json`'s mtime is a CROSS-CHECK
    against the `context-emitted` action that wrote it. (Until 2026-11-06 the age
    came from that mtime, which any writer could move.)
    `doc_citations::the_hook_reports_cleared_points_and_never_cries_wolf_on_a_correct_file`
    runs this script against fixture states and fails by name if any of that breaks.

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

# Defined first because the question block below needs it, and that block runs before
# anything that can exit early.
function Format-Age([int]$seconds) {
    if ($seconds -lt 90) { return "${seconds}s ago" }
    if ($seconds -lt 5400) { return "$([int]($seconds / 60))m ago" }
    return "$([int]($seconds / 3600))h ago"
}

# ===========================================================================
# THE QUESTION DOUG ASKED IN HRW (2026-11-07)
# ===========================================================================
#
# Doug asks HRW questions in HRW's Question Bar. HRW writes the question TOGETHER WITH
# the context it is about to `question.json`, and cannot wake Claude -- so the
# question waits until Doug types `?` here. On `?` this block prints it; on any other
# prompt it says, in one line, that a question is waiting, so a question can never sit
# unanswered because the trigger was forgotten.
#
# **It runs before every early exit**, so an unreadable session.json cannot hide a
# waiting question.
#
# **THE QUESTION'S CONTEXT IS NOT THE LIVE CONTEXT, and the output says so.** The
# lines further down describe what HRW holds NOW; the question was asked about the
# context frozen in question.json. If Doug clicked after asking, they differ -- and
# reading the live `pointing at:` line as the question's referent would be the exact
# join-by-time defect this design removes. So the question's own `this` is printed,
# and the live block is labelled.
$questionPath = Join-Path $bridge 'question.json'
$seenPath = Join-Path $bridge 'question-seen.json'

# The prompt arrives on stdin as JSON from the UserPromptSubmit harness. Read it only
# when stdin is redirected, so running this script by hand does not block on the
# console.
$prompt = $null
if ([Console]::IsInputRedirected) {
    try {
        $raw = [Console]::In.ReadToEnd()
        if ($raw) { $prompt = ($raw | ConvertFrom-Json).prompt }
    } catch { $prompt = $null }
}
# `?` on its own is a PROTOCOL TOKEN, matched exactly. It is never a substring search
# over Doug's words, which docs/identity-and-provenance.md forbids.
#
# **But the VS Code extension puts its own notes in front of what Doug typed**, and they
# are stripped first. Found on the FIRST REAL USE, 2026-11-07: Doug opened a file, typed
# `?`, and the message arrived as `<ide_opened_file>The user opened the file …
# </ide_opened_file>` followed by `?` -- so the whole prompt was not `?`, and the hook
# announced "Doug has not typed ? yet" to the `?` itself. Every test had sent a bare `?`,
# which is not what the editor sends. The tags have a fixed, documented form
# (`<ide_opened_file>`, `<ide_selection>`, ...), so removing them is stripping the
# harness's markup, not interpreting Doug's words; what remains is still compared
# exactly.
$typed = $null
if ($null -ne $prompt) {
    $typed = ([regex]::Replace([string]$prompt, '(?s)<ide_[a-z_]+>.*?</ide_[a-z_]+>', '')).Trim()
}
$asksForQuestion = ($typed -eq '?')

if (Test-Path $questionPath) {
    # "Seen" is keyed to the question file's own mtime: question.json is written only
    # when Doug asks, so a new mtime is exactly a new question, and nothing needs
    # parsing on the prompts that are not `?`.
    $questionTicks = (Get-Item $questionPath).LastWriteTimeUtc.Ticks
    $seenTicks = $null
    if (Test-Path $seenPath) {
        try {
            $seenTicks = [int64](Get-Content $seenPath -Raw -Encoding utf8 | ConvertFrom-Json).question_mtime_ticks
        } catch { $seenTicks = $null }
    }
    $alreadyShown = ($null -ne $seenTicks) -and ($questionTicks -eq $seenTicks)
    $questionAge = Format-Age ([int]((Get-Date).ToUniversalTime() - (Get-Item $questionPath).LastWriteTimeUtc).TotalSeconds)

    if ($asksForQuestion) {
        try {
            $q = Get-Content $questionPath -Raw -Encoding utf8 | ConvertFrom-Json
            $again = if ($alreadyShown) { ' (shown before - this is the same question)' } else { '' }
            # Doug, 2026-11-07: "you are free to use this chat window, a VS code tab, System
            # Modeler or Wolfram Desktop in whichever way you believe would be most effective."
            # This line said "ANSWER IT HERE, in the chat" until then -- narrower than his
            # ruling, and the line Claude reads at the very moment of choosing.
            Write-Output "$tag HRW QUESTION #$($q.seq), asked $questionAge$again - answer from this conversation, NEVER in HRW, using the chat, a VS Code tab, System Modeler or Wolfram Desktop - whichever teaches best:"
            Write-Output "$tag   question: $($q.question)"
            Write-Output "$tag   this = $($q.this)   (the bar's rule: the point, then the follow, then the stage on screen - and Doug saw this beside the field)"
            Write-Output "$tag   the context it was asked about is question.json's `"context`" - the SAME document focus.json held at that moment; open it for node detail"
            Write-Output "$tag   the lines below are the LIVE context NOW; if Doug clicked after asking they differ, and the question is about the one above"
            # Remember it was shown, so the "waiting" reminder stops.
            @{ question_mtime_ticks = $questionTicks; seq = $q.seq } | ConvertTo-Json |
                Set-Content -Path $seenPath -Encoding utf8
        } catch {
            Write-Output "$tag question.json exists but is unreadable: $($_.Exception.Message)"
        }
    } elseif (-not $alreadyShown) {
        Write-Output "$tag AN HRW QUESTION IS WAITING (asked $questionAge) - Doug has not typed ? yet; if this message is not about it, say so"
    }
} elseif ($asksForQuestion) {
    Write-Output "$tag no HRW question has been asked - type one into HRW's Question Bar first"
}

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
# **AND IT IS A SNAPSHOT, WHICH IS WHY IT NO LONGER SAYS "IS" (2026-11-07).** This read
# "HRW IS RUNNING - close it before the gate", and the comment above it said the line
# "decides whether the gate may run". Both were wrong in the same way: the probe runs
# once, when Doug presses Enter, and a turn can last minutes. On 2026-11-07 Claude
# reported the line as a present fact three minutes later -- "HRW is running" -- and
# POSTPONED A COMMIT on it, after Doug had closed the app. That is the fourth instance;
# CLAUDE.md records the first three, 2026-11-05.
#
# So the line now carries the time it was taken, in the past tense, and says plainly
# that it is not a reason to skip the gate. **The gate is what decides whether the gate
# can run**: it fails fast on the relink and names the cause, so trying it costs seconds
# and a stale reading costs a postponed commit and a wrong statement to Doug.
$hrwUp = $null -ne (Get-Process -Name 'hrw' -ErrorAction SilentlyContinue)
$probedAt = (Get-Date).ToString('HH:mm:ss')
if ($hrwUp) {
    Write-Output "$tag HRW WAS RUNNING at $probedAt, when this message was sent - a snapshot, not a fact about later: never report it as current, and never skip or postpone the gate on it (run the gate; it checks for itself)"
} else {
    Write-Output "$tag HRW was NOT running at $probedAt, when this message was sent - every age below is a file's, not a live session's"
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

# **THE POINT LINE AGREES WITH THE SNAPSHOT, not merely with the last gesture**
# (2026-11-07). The previous version reported the age of the latest point-kind action
# whether or not that point still existed -- so after the Question Bar's x, which then
# recorded nothing, it went on saying "pointed at 26m ago" over a point Doug had cleared.
# `$ctx.pointing_at` is the current state; the trail only supplies its age.
if ($null -eq $ctx.pointing_at) {
    if ($captureTime -and $lastPoint.kind -eq 'point-dropped') {
        $capAge = Format-Age ([int]($now - $captureTime).TotalSeconds)
        Write-Output "$tag the point was CLEARED $capAge - nothing is pointed at, so 'this' is not an HRW point"
    } else {
        Write-Output "$tag nothing is pointed at - 'this' is not an HRW point"
    }
} elseif ($captureTime -and $lastPoint.kind -ne 'point-dropped') {
    $capAge = Format-Age ([int]($now - $captureTime).TotalSeconds)
    Write-Output "$tag pointed at $capAge via $($lastPoint.kind) - from the action trail, the same file as the context below"
} else {
    # **The trail is capped at 200 actions, so say that rather than claim an age.** A
    # long session can push the gesture out; the point itself is real (it is in the
    # snapshot), only its age is unknown.
    Write-Output "$tag a point is set but its gesture is not in the trail (capped at 200 actions) - its age is unknown"
}

# **focus.json is the DETAIL file, and its mtime is a CROSS-CHECK against the EMISSION
# that wrote it** (2026-11-07). Every write of focus.json now records a
# `context-emitted` action, in the same frame, at the one choke point (`bridge::write`).
# So the two must agree to within that frame; a disagreement means something OTHER
# than HRW's emitter touched the file, and reading it would describe that writer.
#
# The previous cross-check compared against the last POINT gesture -- and focus.json is
# also rewritten by a follow, by either x, and by a recompile that keeps the point. So
# it warned "do NOT read it as Doug's context" after every edit-and-recompile, about a
# file that was correct: an instruction to distrust the right file. That is worse than
# no check, which is why it was replaced rather than widened.
$emitKinds = @('context-emitted')
$lastEmit = $session.actions | Where-Object { $emitKinds -contains $_.kind } | Select-Object -Last 1
$emitTime = $null
if ($lastEmit -and $lastEmit.at) {
    try   { $emitTime = [datetime]::Parse($lastEmit.at.Replace(' UTC', '')) }
    catch { $emitTime = $null }
}

if (Test-Path $focusPath) {
    $focusTime = (Get-Item $focusPath).LastWriteTime.ToUniversalTime()
    $focusAge = Format-Age ([int]($now - $focusTime).TotalSeconds)
    if ($emitTime -and $lastEmit.detail -like '*NOT WRITTEN*') {
        Write-Output "$tag WARNING HRW's last emission FAILED ($($lastEmit.detail)) - focus.json holds an OLDER context than the one below"
    } elseif ($emitTime) {
        # Same frame, so seconds of slack is generous; the old 120 s tolerance was
        # sized for comparing against a gesture that might precede the write.
        $skew = [math]::Abs((New-TimeSpan -Start $emitTime -End $focusTime).TotalSeconds)
        if ($skew -gt 5) {
            Write-Output "$tag WARNING focus.json ($focusAge) was NOT written by HRW's last emission - something else touched it; do NOT read it as Doug's context"
        } else {
            Write-Output "$tag focus.json is HRW's latest emission ($focusAge) - safe to open for node detail"
        }
    } elseif ($started -and $focusTime -lt [datetime]::Parse($started.Replace(' UTC', ''))) {
        Write-Output "$tag focus.json ($focusAge) PREDATES this HRW session - leftover from a previous run, not context"
    } else {
        # No emission in the trail: either an HRW build from before 2026-11-07, which did
        # not record them, or a trail that scrolled. Say which cannot be told, rather than
        # guess -- and do not warn, since there is nothing to compare against.
        Write-Output "$tag focus.json written $focusAge - cannot cross-check: no context-emitted in the trail (an HRW build from before 2026-11-07 does not record them; rebuild)"
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

    # **The verb is the user's message unless a gesture DECLARED one** -- Doug's ruling,
    # 2026-11-07: "Unless I select something like a context menu item which explicitly
    # declares a verb, no verbs should be associated with my context nouns." This line
    # printed `$p.request` raw, and every point arrived tagged `Explain` -- a verb nobody
    # chose, read by Claude before Doug's actual sentence and biasing it toward
    # explaining. Now `request` is null unless the 🐞 debugger menu item declared it.
    #
    # Three cases, each SAID rather than left blank (the rule above):
    #   null              -> no verb declared; read it from the message
    #   'Explain'/'explain' -> written by an HRW build from BEFORE the fix, which
    #                        presumed it. Relaying it as a verb would repeat the defect
    #                        for as long as the old binary runs, so it is named as
    #                        presumed rather than passed through.
    #   anything else     -> a verb a gesture declared, e.g. debug-where-set
    $verb = if ($null -eq $p.request) {
        'no verb declared - the verb is the message'
    } elseif ($p.request -eq 'Explain' -or $p.request -eq 'explain') {
        'no verb declared - "Explain" was PRESUMED by a pre-2026-11-07 HRW build; rebuild'
    } else {
        "verb declared: $($p.request)"
    }
    Write-Output "$tag pointing at: $($p.target)   ($($p.kind), $stage, $verb)"
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
# not have, which is the one disagreement the Question Bar exists to make impossible.
if ($ctx.last_emission_error) {
    Write-Output "$tag NOT EMITTED - $($ctx.last_emission_error)"
}
