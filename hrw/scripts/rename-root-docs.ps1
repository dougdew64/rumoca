# Pass 6: CLAUDE.md and README.md, which passes 1-5 missed.
#
# `Get-ChildItem -Path '.' -Include *.md -File` without -Recurse matches
# nothing, so the three root documents were never touched. Two of them are
# LIVE and had to move; one is HISTORY and must not.
#
#   CLAUDE.md    live rules -- and it carried FIVE BROKEN LINKS to
#                docs/fixture-tours/README.md that the full gate did not
#                catch, because no checker resolves CLAUDE.md's markdown
#                links. Recorded as a gap.
#   README.md    live and user-facing; it also cited the old test name
#                `fixture_tour_links_all_resolve`.
#   DECISIONS.md **deliberately excluded.** It is history and does not bind
#                (charter Decision 11). An entry dated 2026-07-22 saying
#                "guided tours drive backlog prioritization" is a true
#                statement about what was decided then; rewriting it to
#                "labs" would have Doug deciding something he did not. Same
#                rule that protects `end_to_end_tour.md`.

$ErrorActionPreference = 'Stop'

$SENTINEL = "`u{241E}HISTORICAL_END_TO_END_TOUR`u{241E}"

$pairs = @(
    @('TourStop', 'LabStation'), @('tour_stop', 'lab_station'),
    @('TourState', 'LabState'), @('TourLink', 'LabLink'),
    @('TourSource', 'LabSource'), @('TourPassage', 'LabPassage'),
    @('AdHocTour', 'AdHocLab'), @('OpenTour', 'OpenLab'),
    @('FIXTURE_TOURS_DIR', 'FIXTURE_LABS_DIR'), @('LIVE_TOUR_PATH', 'LIVE_LAB_PATH'),
    @('MAX_TOUR_CHROME', 'MAX_LAB_CHROME'), @('OVERVIEW_TOUR', 'OVERVIEW_LAB'),
    @('TOUR_CONTEXT_ABOVE', 'LAB_CONTEXT_ABOVE'), @('TOUR_POLL_INTERVAL', 'LAB_POLL_INTERVAL'),
    @('TOUR_PROGRESS_HEIGHT', 'LAB_PROGRESS_HEIGHT'), @('TOUR_PROGRESS_MARGIN', 'LAB_PROGRESS_MARGIN'),
    @('TOUR_KINDS', 'LAB_KINDS'), @('TOUR_FILE', 'LAB_FILE'),
    @('hrw://tour/', 'hrw://lab/'), @('fixture-tours', 'fixture-labs'),
    @('fixture_tours', 'fixture_labs'), @('tour_panel', 'lab_panel'),
    @('gen_tour_catalogue', 'gen_lab_catalogue'), @('tour-kinds-plan', 'lab-kinds-plan'),
    @('guided-tour', 'guided-lab'), @('tour-design', 'lab-design'),
    @('tours_', 'labs_'), @('tour_', 'lab_'), @('_tours', '_labs'), @('_tour', '_lab'),
    @('\bTOURS\b', 'LABS'), @('\bTOUR\b', 'LAB'),
    @('\bTours\b', 'Labs'), @('\bTour\b', 'Lab'),
    @('\btours\b', 'labs'), @('\btour\b', 'lab'),
    @('\bStop (\d)', 'Station $1'), @('/stop/', '/station/'), @('\bstop-(\d)', 'station-$1')
)

$changed = 0
foreach ($name in @('CLAUDE.md', 'README.md')) {
    $path = Join-Path (Get-Location) $name
    $orig = [IO.File]::ReadAllText($path)
    $text = $orig.Replace('end_to_end_tour', $SENTINEL)
    foreach ($p in $pairs) { $text = [regex]::Replace($text, $p[0], $p[1]) }
    $text = $text.Replace($SENTINEL, 'end_to_end_tour')
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "root files changed: $changed (DECISIONS.md deliberately untouched)"
