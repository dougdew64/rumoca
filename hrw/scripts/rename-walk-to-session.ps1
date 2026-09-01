# Charter Decision 15, second atomic pass: the VERB.
#
# The first rename settled the nouns (lab / station / observation /
# instructor) and missed `walk`, leaving ~350 occurrences in docs/ and ~230
# in src/. Doug ruled 2026-09-01: the verb is `run`, the session is a
# `session` -- which Decision 14 had already named ("a walk is a lab
# session, not a reading").
#
# `walk` COLLIDES exactly as `stop` did. Two senses live here:
#
#   LAB SESSION   the thing Doug does with a lab            -> rename
#   TRAVERSAL     walking a tree, blocks, modules, stages   -> MUST SURVIVE
#
# So traversal is protected by sentinel BEFORE anything is substituted, and
# restored after. Only five src/ identifiers are renamed, each individually
# confirmed to mean a lab session; every other walk-identifier is left alone
# because an unverified rename of traversal code is worse than a stale word.

$ErrorActionPreference = 'Stop'

# --- traversal, protected verbatim ---------------------------------------
$protect = @(
    'walk_modules', 'walk_for_labels', 'walk_blocks', 'walkable', 'walker',
    'following_an_identifier_walks_every_stage_without_panicking',
    'the_generated_ledger_reproduces_the_twicedefined_walk',
    'the_walk_opens_before_any_block_has_been_solved',
    'a_real_specimen_produces_a_walkable_plan',
    'walked_regions', 'unterminated_walked', 'walked_prose_never_changes_silently',
    'fn walk(', 'walks the alias', 'walking into library',
    'walks `', 'walks every', 'walk the tree', 'walking the tree'
)

# --- lab-session identifiers, each confirmed by reading its call site -----
$idents = @(
    @('a_finished_walk_returns_to_the_mode_it_started_in', 'a_finished_session_returns_to_the_mode_it_started_in'),
    @('a_link_still_dispatches_after_a_walk_has_been_stopped', 'a_link_still_dispatches_after_a_session_has_been_stopped'),
    @('starting_a_walk_forgets_where_the_last_one_stopped',   'starting_a_session_forgets_where_the_last_one_stopped'),
    @('the_play_button_starts_a_walk_and_the_readout_reports_it', 'the_play_button_starts_a_session_and_the_readout_reports_it'),
    @('test_set_walked_state', 'test_set_session_state')
)

# --- prose, applied after protection --------------------------------------
$prose = @(
    @('\bre-walked\b',  're-run'),      @('\bre-walk\b',   're-run'),
    @('\bwalkthrough\b','run-through'),
    @('\bwalking\b',    'running'),     @('\bwalked\b',    'run'),
    @('\bwalks\b',      'runs'),        @('\bwalk\b',      'run')
)

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
$targets += Get-ChildItem -Path '.' -Include *.md -File

foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig

    # 1. rename the confirmed lab-session identifiers first
    foreach ($p in $idents) { $text = $text.Replace($p[0], $p[1]) }

    # 2. sentinel every traversal form so prose rules cannot reach it
    $map = @{}
    $i = 0
    foreach ($p in $protect) {
        if ($text.Contains($p)) {
            $k = "`u{241E}P$i`u{241E}"
            $map[$k] = $p
            $text = $text.Replace($p, $k)
            $i++
        }
    }

    # 3. prose
    foreach ($p in $prose) { $text = [regex]::Replace($text, $p[0], $p[1]) }

    # 4. restore traversal
    foreach ($k in $map.Keys) { $text = $text.Replace($k, $map[$k]) }

    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files changed: $changed"
