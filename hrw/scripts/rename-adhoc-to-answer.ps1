# "Ad hoc lab" becomes "Answer" -- Doug's ruling, 2026-09-02.
#
# Yesterday's tour -> lab rename produced "ad hoc lab" mechanically, and the
# phrase is wrong the way "tour guide" was wrong: it inherits a name whose
# properties the thing does not have. Everything that makes a lab a lab is
# absent by design -- no route, no stations, no predictions, nothing checks it,
# and it is discarded in seconds.
#
# The UI had already solved this. `LabSource::AdHoc` has rendered as
# "* Answer" in the picker since 2026-08-19; only the code and the prose
# lagged.
#
# Explicit list, never a blind substitution, because "ad hoc" is ordinary
# English and appears in prose that is not about this feature.

$ErrorActionPreference = 'Stop'

$pairs = @(
    # identifiers
    @('AdHocLab',                'Answer'),
    @('LabSource::AdHoc',        'LabSource::Answer'),
    @('Self::AdHoc',             'Self::Answer'),
    @('ad_hoc_selected',         'answer_selected'),
    # the live-state path
    @('\.hrw-bridge/lab\.md',    '.hrw-bridge/answer.md'),
    @('hrw-bridge\\lab\.md',     'hrw-bridge\answer.md'),
    # prose
    @('an ad hoc lab',           'an Answer'),
    @('An ad hoc lab',           'An Answer'),
    @('ad hoc labs',             'Answers'),
    @('Ad hoc labs',             'Answers'),
    @('ad hoc lab',              'Answer'),
    @('Ad hoc lab',              'Answer')
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

# `AdHoc` on its own, after the qualified forms above have been taken.
$bare = 0
foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = [regex]::Replace($orig, '\bAdHoc\b', 'Answer')
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $bare++
    }
}

Write-Output "files changed: $changed (+$bare with bare AdHoc)"
