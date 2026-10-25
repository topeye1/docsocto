# M0 Hangul judgement, round 2: 1-up export + aligned comparison.
$ErrorActionPreference = 'Continue'
$w = 'E:\OCTO\work\m0_out'
$out = "$w\hwp1"
$files = @(Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | ForEach-Object FullName) +
         @(Get-ChildItem "$w\*_edit.hwpx" | ForEach-Object FullName)
& "$PSScriptRoot\hwp_export.ps1" -Out $out -Files $files
"== export"; Get-Content "$out\hwp_export.log"
"== cross (Hangul edit vs rhwp edit)"
python -I "$PSScriptRoot\cmp_pdf.py" $out '_edit' $w '_edit' "$w\cmp2_cross.json" 2> "$w\cmp2.err"
"== self (Hangul orig vs Hangul edit)"
python -I "$PSScriptRoot\cmp_pdf.py" $out '' $out '_edit' "$w\cmp2_self.json" 2>> "$w\cmp2.err"
"== CMP2_DONE"
