# Pass 4: stop -> station, the half that cannot be done blind.
#
# Rule 16 renamed the unit to `station` PRECISELY BECAUSE `stop` collided: a
# compile stops, a debugger stops, and the UI says "Stop following" and "Stop
# pointing at this". Those must all survive untouched -- so this matches only
# `Stop` followed by a NUMBER or a template placeholder, which is a lab
# station and nothing else.
#
# Also flips the one kind Decision 15 renamed: adjudication -> calibration.
# The other three keep their names (concept, feature, failure); `experiment`,
# `orientation` and `diagnosis` were rejected on domain collisions recorded
# under Decision 15.

$ErrorActionPreference = 'Stop'

$pairs = @(
    # headings and prose references: Stop 1, Stop 3b, Stop 0
    @('\bStop (\d)',        'Station $1'),
    # template placeholders: `## Stop N —`, `## ⚙ Stop N+1 —`
    @('\bStop N\b',         'Station N'),
    # the format! literal `Stop {n} `
    @('\bStop \{n\}',       'Station {n}'),
    # the bare search prefix `"Stop "` -- quotes included, so "Stop following"
    # and "Stop pointing" (which have no closing quote there) are untouched
    @('"Stop "',            '"Station "'),
    # the kind Decision 15 renamed
    @('kind: adjudication',  'kind: calibration'),
    @('"adjudication"',      '"calibration"')
)

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
$targets += Get-ChildItem -Path '.' -Include *.md -File

foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig
    foreach ($p in $pairs) { $text = [regex]::Replace($text, $p[0], $p[1]) }
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files changed: $changed"
