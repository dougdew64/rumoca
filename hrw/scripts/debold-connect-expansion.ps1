# Experiment, iteration 1: strip bold from connect-expansion.md.
#
# Doug, 2026-09-01: "the labs are difficult to read ... there's a lot in the
# labs which is competing for my visual attention." Measured in that one lab:
# 98 bold spans and 166 code spans across 288 lines of prose -- roughly one
# styled fragment per line, sustained. Emphasis is DIFFERENTIAL, so at that
# density nothing reads as emphasised and the texture must be parsed before
# the sentence can be read.
#
# Instruction: remove all bolding except the lab title and the station
# headers. Those are `#` and `##` headings and carry no `**` at all, so this
# strips every `**...**` in the file.
#
# ONE DELIBERATE EXCEPTION, flagged rather than silently applied: line ~51,
# "**This lab counts.**" is not decoration -- `LabSource::blurb_of` reads a
# lab's FIRST BOLDED LINE to build the catalogue summary, and the pin added
# the same day asserts it. Unbolding it would empty the catalogue entry and
# fail `every_pinned_lab_claim_holds`. If the scheme survives the experiment,
# the right fix is to change how the blurb is derived, not to re-bold a line.
#
# ASCII-only by design: PowerShell 5.1 reads a BOM-less script as ANSI, which
# silently mangled em-dashes in an earlier corrective pass today.

$ErrorActionPreference = 'Stop'

$path  = Join-Path (Get-Location) 'docs/fixture-labs/connect-expansion.md'
$lines = [IO.File]::ReadAllLines($path)

$blurbSeen = $false
$stripped  = 0

for ($i = 0; $i -lt $lines.Length; $i++) {
    $line = $lines[$i]

    # Preserve the first bolded line: it is the catalogue's data source.
    if (-not $blurbSeen -and $line -match '^\s*\*\*.{11,}') {
        $blurbSeen = $true
        continue
    }

    $new = [regex]::Replace($line, '\*\*([^*]+)\*\*', '$1')
    if ($new -ne $line) {
        $stripped += ([regex]::Matches($line, '\*\*([^*]+)\*\*')).Count
        $lines[$i] = $new
    }
}

[IO.File]::WriteAllLines($path, $lines, (New-Object Text.UTF8Encoding $false))
Write-Output "bold spans removed: $stripped (the blurb line kept)"
