# Second pass: prose phrasings the explicit list missed.
#
# Pass 1 replaced "ad hoc lab" and the identifiers. What survived is looser
# English -- "the ad hoc one", "ad hoc sorts first", "ad hoc goes first" --
# plus a test name.
#
# NOT touched, deliberately: "ad hoc notebook". A notebook is a Wolfram file,
# not an Answer, and `bridge.rs` distinguishes a scratch notebook from a
# committed one. Renaming that would conflate two different things, which is
# the error this whole rename exists to undo.

$ErrorActionPreference = 'Stop'

$pairs = @(
    @('the_ad_hoc_lab_is_a_button_and_not_a_picker_entry',
      'the_answer_is_a_button_and_not_a_picker_entry'),
    @('the ad hoc one first when it exists', 'the Answer first when one exists'),
    @('ad hoc first when one exists',        'the Answer first when one exists'),
    @('Ad hoc goes first because it answers','The Answer goes first because it answers'),
    @('"ad hoc sorts first"',                '"the Answer sorts first"'),
    @('while the ad hoc',                    'while the Answer'),
    @('#42 says ad hoc',                     '#42 says an Answer')
)

$changed = 0
$targets = @()
$targets += Get-ChildItem -Path 'src','examples' -Recurse -Include *.rs -File
$targets += Get-ChildItem -Path 'docs' -Recurse -Include *.md,*.txt -File
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
Write-Output "files changed: $changed"
