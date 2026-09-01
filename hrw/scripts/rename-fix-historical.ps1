# Corrective pass for the tour -> lab rename.
#
# `end_to_end_tour.md` is the REAL NAME of a file deleted on 2026-08-01 and
# recoverable from git history under that name. The bulk rename rewrote its
# 18 mentions to `end_to_end_lab.md`, which falsifies a historical record --
# the thing this repository forbids above all else. A proper noun that names
# something in the past does not move when the vocabulary does.

$ErrorActionPreference = 'Stop'

$fixed = 0
foreach ($f in (Get-ChildItem -Path 'docs','.' -Recurse -Include *.md -File |
                Where-Object { $_.FullName -notmatch '\\target\\' } |
                Sort-Object FullName -Unique)) {
    $orig = [IO.File]::ReadAllText($f.FullName)
    $text = $orig.Replace('end_to_end_lab.md', 'end_to_end_tour.md')
    if ($text -ne $orig) {
        [IO.File]::WriteAllText($f.FullName, $text, (New-Object Text.UTF8Encoding $false))
        $fixed++
    }
}
Write-Output "files corrected: $fixed"
