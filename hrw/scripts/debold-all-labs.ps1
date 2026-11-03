# Apply the connect-expansion formatting to every other lab.
#
# Doug ruled the scheme good after seeing it on one lab: strip emphasis from
# running prose, keep the lab title and station headers (which are `#` and
# `##` headings and carry no `**` at all), and keep the two markers that are
# STRUCTURE rather than emphasis.
#
# Three things are preserved in every file, each for a different reason:
#
#   **Predict.**  /  **Expected:**
#       Machine-read by `a_lab_predicts_if_and_only_if_its_kind_says_so` and
#       `every_station_of_every_lab_owes_an_expected`. They are also the
#       checkpoint grammar -- charter Decision 14 makes the prediction the
#       pedagogical core, so it should be the most distinct thing on the page.
#
#   the FIRST bolded line
#       `LabSource::blurb_of` reads it to build the catalogue summary, and
#       `every_pinned_lab_claim_holds` pins the result. Unbolding it would
#       empty the catalogue entry and fail that pin.
#
# Whole-file and non-greedy, because bold spans cross line breaks -- the
# per-line pass on connect-expansion left 14 stray markers.
#
# ASCII-only: PowerShell 5.1 reads a BOM-less script as ANSI, which silently
# mangled em-dashes in an earlier pass today.

$ErrorActionPreference = 'Stop'

$dir  = Join-Path (Get-Location) 'docs/fixture-labs'
$skip = @('README.md', 'CATALOGUE.md', 'connect-expansion.md')

$totalBefore = 0
$totalAfter  = 0
$files       = 0

foreach ($f in (Get-ChildItem -Path $dir -Filter *.md -File | Sort-Object Name)) {
    if ($skip -contains $f.Name) { continue }

    $text   = [IO.File]::ReadAllText($f.FullName)
    $before = ([regex]::Matches($text, '\*\*[^*]+\*\*')).Count

    # 1. Protect the first bolded line -- whatever it says in this lab.
    $blurbMatch = [regex]::Match($text, '(?m)^\s*\*\*.{11,}$')
    $sentinel   = "`u{241E}BLURB`u{241E}"
    if (-not $blurbMatch.Success) {
        Write-Output ("  SKIPPED {0}: no bolded opening line to protect" -f $f.Name)
        continue
    }
    $blurbLine = $blurbMatch.Value
    $text = $text.Remove($blurbMatch.Index, $blurbLine.Length).Insert($blurbMatch.Index, $sentinel)

    # 2. Strip every remaining bold span, including ones crossing a newline.
    $text = [regex]::Replace($text, '(?s)\*\*(.+?)\*\*', '$1')

    # 3. Restore the blurb line and the two structural markers.
    $text = $text.Replace($sentinel, $blurbLine)
    $text = [regex]::Replace($text, '(?m)^(>\s*)Predict\.',  '$1**Predict.**')
    $text = [regex]::Replace($text, '(?m)^Expected:',        '**Expected:**')

    $after = ([regex]::Matches($text, '\*\*[^*]+\*\*')).Count
    [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))

    $totalBefore += $before
    $totalAfter  += $after
    $files++
    Write-Output ("  {0,-34} {1,4} -> {2,3}" -f $f.Name, $before, $after)
}

Write-Output ""
Write-Output "labs: $files    bold spans: $totalBefore -> $totalAfter"
