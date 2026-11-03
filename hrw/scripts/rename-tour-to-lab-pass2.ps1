# Pass 2 of the tour -> lab rename: COMPOUND identifiers.
#
# Pass 1 used `\btour\b`, which correctly skipped `tour_link_y` and
# `every_tour_the_overview_links_to_links_back` -- `_` is a word character, so
# there is no word boundary between `tour` and `_`. This pass takes those.
#
# `end_to_end_tour` is protected by a sentinel: it is the real name of a file
# deleted 2026-08-01 and recoverable from git under that name, so it must not
# move with the vocabulary. Pass 1 rewrote it and a corrective pass put it
# back; doing that twice would be a defect, not a rhyme.

$ErrorActionPreference = 'Stop'

$SENTINEL = "`u{241E}HISTORICAL_END_TO_END_TOUR`u{241E}"

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
$targets += Get-ChildItem -Path '.' -Include *.md -File

foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)

    # protect the historical proper noun
    $text = $orig.Replace('end_to_end_tour', $SENTINEL)

    $text = [regex]::Replace($text, 'tour_',  'lab_')
    $text = [regex]::Replace($text, '_tour',  '_lab')
    $text = [regex]::Replace($text, 'Tour_',  'Lab_')
    $text = [regex]::Replace($text, '_Tour',  '_Lab')

    $text = $text.Replace($SENTINEL, 'end_to_end_tour')

    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files changed: $changed"
