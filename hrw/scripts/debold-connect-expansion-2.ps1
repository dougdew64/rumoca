# Iteration 1, pass 2: bold spans that cross a line break.
#
# Pass 1 worked line by line, so `**strongest where you need it least, absent
# where\nyou need it most.**` survived untouched -- 14 stray `**` markers, 7
# unclosed pairs. Same class of error as the case-sensitive regexes earlier
# today: an assumption inside a substitution silently narrowed it.
#
# Whole-file this time, non-greedy, with the blurb line cut out and restored
# around the replacement so `LabSource::blurb_of` still finds it.

$ErrorActionPreference = 'Stop'

$path = Join-Path (Get-Location) 'docs/fixture-labs/connect-expansion.md'
$text = [IO.File]::ReadAllText($path)

$blurb    = '**This lab counts.**'
$sentinel = "`u{241E}BLURB`u{241E}"
if (-not $text.Contains($blurb)) { throw "blurb line not found; refusing to strip" }
$text = $text.Replace($blurb, $sentinel)

$before = ([regex]::Matches($text, '\*\*')).Count
$text   = [regex]::Replace($text, '(?s)\*\*(.+?)\*\*', '$1')
$after  = ([regex]::Matches($text, '\*\*')).Count

$text = $text.Replace($sentinel, $blurb)
[IO.File]::WriteAllText($path, $text, (New-Object Text.UTF8Encoding $false))

Write-Output "markers before: $before  after: $after (2 expected, the blurb pair)"
