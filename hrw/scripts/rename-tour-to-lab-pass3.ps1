# Pass 3 of the tour -> lab rename: the leftovers pass 1 and 2 could not see.
#
#   tours_          plural before an underscore; pass 2 only took `tour_`
#   TOUR / Tour     upper- and title-case bare words, including the GATE VERDICT
#                   name (FAST / TOUR / FULL), which is what the runner prints
#   toured          a test local in context_bar.rs meaning "has a lab open"
#
# `end_to_end_tour` stays protected. `detour` and `tourist` survive on word
# boundaries, as in pass 1.

$ErrorActionPreference = 'Stop'

$SENTINEL = "`u{241E}HISTORICAL_END_TO_END_TOUR`u{241E}"

$pairs = @(
    @('\btoured\b',  'with_lab'),
    @('tours_',      'labs_'),
    @('Tours_',      'Labs_'),
    @('\bTOURS\b',   'LABS'),
    @('\bTOUR\b',    'LAB')
)

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
$targets += Get-ChildItem -Path '.' -Include *.md -File

foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig.Replace('end_to_end_tour', $SENTINEL)
    foreach ($p in $pairs) { $text = [regex]::Replace($text, $p[0], $p[1]) }
    $text = $text.Replace($SENTINEL, 'end_to_end_tour')
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files changed: $changed"
