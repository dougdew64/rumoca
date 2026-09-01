# Corrective pass for the walk -> session rename.
#
# The predicted failure, and the third time this exact class has appeared:
# PROSE ABOUT A RENAME IS THE PROSE A RENAME MOST RELIABLY BREAKS, because
# it is the only text where the retired word appears DELIBERATELY.
#
# Two kinds here, both restored:
#
#   QUOTATIONS      Doug's words. A quotation is a claim about what someone
#                   said, so editing one is an accuracy defect, not a
#                   cosmetic one.
#   DECISION TITLES Decision 14 is literally "A walk is a lab session, not a
#                   reading" -- it is the decision that RETIRED the word, so
#                   its title must keep it or the decision states a tautology.

$ErrorActionPreference = 'Stop'

$pairs = @(
    # Decision 14's title, its amendment-log entry, and the labs README banner
    @('A run is a lab session, not a reading',  'A walk is a lab session, not a reading'),
    @('a run is a lab session, not a reading',  'a walk is a lab session, not a reading'),
    @('a run is a **lab session**, not a reading', 'a walk is a **lab session**, not a reading'),
    @('part of the run itself',                 'part of the walk itself'),
    # Doug's verbatim words
    @('the need to run labs',                   'the need to walk labs')
)

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md -File
$targets += Get-ChildItem -Path '.' -Include *.md -File

foreach ($f in ($targets | Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig
    foreach ($p in $pairs) { $text = $text.Replace($p[0], $p[1]) }
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $changed++
    }
}
Write-Output "files corrected: $changed"
