# Pass 5: the `/stop/` link segment and the `stop-N-...` heading anchors.
#
# Renaming the `## Stop N` headings changed every slug they generate, so
# cross-lab citations pointing at `stop-4-...` now resolve to nothing --
# caught by `lab_citations_name_a_real_lab_and_a_real_stop`, which is the
# checker earning its place.
#
# Doug ruled every hrw:// link is rewritten with no alias, so the URL segment
# moves too: `hrw://lab/<name>/stop/<slug>` becomes `.../station/<slug>`.
#
# Narrow by construction: only `stop-` followed by a digit (an anchor) and the
# exact `/stop/` path segment. Bare `stop` keeps its meaning everywhere else.

$ErrorActionPreference = 'Stop'

$pairs = @(
    @('/stop/',        '/station/'),      # the URL segment
    @('"stop"',        '"station"'),      # the parsed segment literal
    @('\bstop-(\d)',   'station-$1')      # heading anchors: stop-4-... -> station-4-...
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
