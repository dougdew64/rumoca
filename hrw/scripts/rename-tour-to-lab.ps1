# Charter Decision 15: tours become labs, in one atomic pass.
#
# Written with the Write tool and run by path, per CLAUDE.md's rule against
# generating source text through a shell. Substitutions are an EXPLICIT LIST,
# never a blind `tour -> lab`, because three words must survive:
#   detour, tourist  (English)
#   SIGSTOP, backstop, stopwatch, stopped_at, and the playback verbs
#                    (a compile stops and a debugger stops -- rule 16 exists
#                     precisely because `stop` collided)
#
# Order matters: longest and most specific first, so `TourStop` becomes
# `LabStation` before any bare `Tour` rule can turn it into `LabStop`.

$ErrorActionPreference = 'Stop'

# --- ordered replacement pairs -------------------------------------------
$pairs = @(
    # 1. the compound that must not be split: stop -> STATION, not lab-stop
    @('TourStop',              'LabStation'),
    @('tour_stop',             'lab_station'),

    # 2. types and variants
    @('TourState',             'LabState'),
    @('TourLink',              'LabLink'),
    @('TourSource',            'LabSource'),
    @('TourPassage',           'LabPassage'),
    @('AdHocTour',             'AdHocLab'),
    @('OpenTour',              'OpenLab'),

    # 3. constants
    @('FIXTURE_TOURS_DIR',     'FIXTURE_LABS_DIR'),
    @('LIVE_TOUR_PATH',        'LIVE_LAB_PATH'),
    @('MAX_TOUR_CHROME',       'MAX_LAB_CHROME'),
    @('OVERVIEW_TOUR',         'OVERVIEW_LAB'),
    @('TOUR_CONTEXT_ABOVE',    'LAB_CONTEXT_ABOVE'),
    @('TOUR_POLL_INTERVAL',    'LAB_POLL_INTERVAL'),
    @('TOUR_PROGRESS_HEIGHT',  'LAB_PROGRESS_HEIGHT'),
    @('TOUR_PROGRESS_MARGIN',  'LAB_PROGRESS_MARGIN'),
    @('TOUR_KINDS',            'LAB_KINDS'),
    @('TOUR_FILE',             'LAB_FILE'),

    # 4. on-disk and link vocabulary (Doug: every link rewritten, no alias)
    @('hrw://tour/',           'hrw://lab/'),
    @('fixture-tours',         'fixture-labs'),
    @('fixture_tours',         'fixture_labs'),
    @('tour\.md',              'lab.md'),
    @('tour_panel',            'lab_panel'),
    @('gen_tour_catalogue',    'gen_lab_catalogue'),

    # 5. bare words LAST, word-bounded so detour/tourist survive
    @('\bTours\b',             'Labs'),
    @('\bTour\b',              'Lab'),
    @('\btours\b',             'labs'),
    @('\btour\b',              'lab')
)

# --- stop -> station, SURGICAL. Only identifiers that mean a lab station. --
# Everything else keeps `stop`: it is the right word for a compile halting
# and a debugger breaking, and rule 16 frees it for exactly that.
$stopPairs = @(
    @('autoplay_stop_heading', 'autoplay_station_heading'),
    @('\bstop_slug\b',         'station_slug'),
    @('\bstop_split\b',        'station_split'),
    @('\bstop_pending\b',      'station_pending'),
    @('\bconcept_stops\b',     'concept_stations'),
    @('\bcurrent_stop\b',      'current_station'),
    @('\bfound_stop\b',        'found_station'),
    @('\bnumbered_stops\b',    'numbered_stations'),
    @('\bother_stops\b',       'other_stations'),
    @('\bparse_stops\b',       'parse_stations'),
    @('\bvisible_stop_count\b','visible_station_count')
)

$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
$targets += Get-ChildItem -Path '.' -Include *.md -File
$targets += Get-ChildItem -Path 'specimens' -Recurse -Include *.md -File -ErrorAction SilentlyContinue

$changed = 0
foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig
    foreach ($p in $pairs)     { $text = [regex]::Replace($text, $p[0], $p[1]) }
    foreach ($p in $stopPairs) { $text = [regex]::Replace($text, $p[0], $p[1]) }
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files changed: $changed"
