# Corrective pass 2 for walk -> session: CAPITALISED forms.
#
# [regex]::Replace is case-sensitive, so `\bwalking\b` left `Walking`,
# `WALKING` and `Walk` untouched -- including the very heading Doug flagged,
# "THE WALK *IS* THE LEARNING". Same class of error as the first rename's
# `\b[A-Za-z_][A-Za-z0-9_]*[Tt]our` regex, which required a character BEFORE
# "Tour" and so missed `TourState` entirely. Twice now a case or anchoring
# assumption has silently narrowed a substitution.
#
# Explicit strings, because the remaining sites are few and several NEARBY
# uses must survive: Doug's 2026-08-08 quotation, Decision 14's own wording,
# the collision analysis naming `walk_modules()` and `fn walk(`, the retired
# `last_walked` marker's real name, and this file's retitle note quoting the
# old heading.

$ErrorActionPreference = 'Stop'

$pairs = @(
    @('### ⟶ THE WALK *IS* THE LEARNING — read this before anything else here',
      '### ⟶ RUNNING A LAB *IS* THE LEARNING — read this before anything else here'),
    @('**WHILE WALKING, ENGAGE — DO NOT PATCH.**',
      '**WHILE RUNNING ONE, ENGAGE — DO NOT PATCH.**'),
    @('**Walking asks whether what is written lands.',
      '**Running a lab asks whether what is written lands.'),
    @('**WALKING AND EXPLORING STRESS DIFFERENT SURFACES.**',
      '**RUNNING AND EXPLORING STRESS DIFFERENT SURFACES.**'),
    @('walking labs when he can focus',   'running labs when he can focus'),
    @('## Open questions a walk may hit', '## Open questions a session may hit'),
    @('expectation, not his walks',       'expectation, not his sessions')
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
