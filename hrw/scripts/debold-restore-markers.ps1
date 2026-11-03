# Iteration 1, pass 3: restore the two markers that are STRUCTURE, not emphasis.
#
# The gate caught what the experiment could not see from the panel: `**Predict.**`
# and `**Expected:**` are machine-read. `a_lab_predicts_if_and_only_if_its_kind_says_so`
# and `every_station_of_every_lab_owes_an_expected` both match on the bolded form, so
# stripping them broke the checkers -- and, more to the point, erased the two markers a
# reader needs most.
#
# That is the finding, not a setback: BOLD HAS TWO JOBS IN A LAB. One is emphasis, which
# at 98 spans per 288 lines had stopped working. The other is the checkpoint grammar --
# where a prediction is owed and where the answer is given. Charter Decision 14 makes the
# prediction the pedagogical core, so it SHOULD be the most visually distinct thing on
# the page. Removing it was removing the wrong bold.

$ErrorActionPreference = 'Stop'

$path = Join-Path (Get-Location) 'docs/fixture-labs/connect-expansion.md'
$text = [IO.File]::ReadAllText($path)

$before = ([regex]::Matches($text, '\*\*')).Count

# Line-anchored so only the markers move, never the words in running prose.
$text = [regex]::Replace($text, '(?m)^(>\s*)Predict\.', '$1**Predict.**')
$text = [regex]::Replace($text, '(?m)^Expected:', '**Expected:**')

$after = ([regex]::Matches($text, '\*\*')).Count
[IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false))

Write-Output "markers restored; ** count $before -> $after"
