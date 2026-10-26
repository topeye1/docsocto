# M4 변환 판정(아무 HTML 하나): octodocs convert 로 docx·HWPX → Word·한글 PDF → Edge 인쇄 PDF 와 비교.
# usage: convert_check.ps1 -Html <원본.html> -Work <작업 폴더> [-Name <이름>] [-KeepEdge(이미 있는 Edge PDF 를 그대로 씀)]
#   결과: <Work>\<이름>.docx·.hwpx·.report.json, RESULT 줄(쪽 수·차이율), 비교 그림 <Work>\cmp_word.png·cmp_hwpcmp.png
param([Parameter(Mandatory)][string]$Html, [Parameter(Mandatory)][string]$Work, [string]$Name = '', [switch]$KeepEdge)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$x = 'E:\OCTO\DOCS\target\release\octodocs.exe'
if (-not $Name) { $Name = [IO.Path]::GetFileNameWithoutExtension($Html) }
$w = $Work; $n = $Name
New-Item -ItemType Directory -Force "$w\edge", "$w\word", "$w\hwp", "$w\hwpcmp" | Out-Null
# Edge 인쇄 PDF(기준) — 머리말·꼬리말 없이
$edge = @('C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe', 'C:\Program Files\Microsoft\Edge\Application\msedge.exe') | Where-Object { Test-Path $_ } | Select-Object -First 1
$url = 'file:///' + ((Resolve-Path $Html).Path -replace '\', '/')
if (-not ($KeepEdge -and (Test-Path "$w\edge\$n.pdf"))) {
Remove-Item "$w\edge\$n.pdf" -ErrorAction SilentlyContinue
cmd /c "`"$edge`" --headless=new --disable-gpu --no-first-run --user-data-dir=`"$env:TEMP\octodocs_edge_pdf`" --no-pdf-header-footer --print-to-pdf=`"$w\edge\$n.pdf`" `"$url`" 2> nul"
for ($i = 0; $i -lt 40 -and -not (Test-Path "$w\edge\$n.pdf"); $i++) { Start-Sleep -Milliseconds 500 }
}
"edge pdf: $(Test-Path "$w\edge\$n.pdf")"
$fail = 0
foreach ($fmt in 'docx', 'hwpx') {
  Remove-Item "$w\$n.$fmt" -ErrorAction SilentlyContinue
  & $x convert $Html "$w\$n.$fmt" --report "$w\$n.$fmt.report.json" > "$w\$n.$fmt.out" 2> "$w\$n.$fmt.err"
  "convert $fmt exit=$LASTEXITCODE"
  if (-not (Test-Path "$w\$n.$fmt")) { "FAIL convert ${fmt}:"; Get-Content "$w\$n.$fmt.err" -Encoding utf8 -TotalCount 5; $fail++ }
}
if (Test-Path "$w\$n.hwpx.report.json") { Get-Content "$w\$n.hwpx.report.json" -Encoding utf8 }
. E:\OCTO\DOCS\tools\m0\dialogs.ps1
if (Test-Path "$w\$n.docx") {
  $guard = Start-Job -ScriptBlock { . E:\OCTO\DOCS\tools\m0\dialogs.ps1; while ($true) { Close-Dialogs @('WINWORD') 'E:\OCTO\work\convert_check.dialogs.log' | Out-Null; Start-Sleep 2 } }
  Remove-Item "$w\word\$n.pdf" -ErrorAction SilentlyContinue
  powershell -NoProfile -File E:\OCTO\DOCS\tools\m0\word_pdf.ps1 -Src "$w\$n.docx" -Pdf "$w\word\$n.pdf"
  Stop-Job $guard; Remove-Job $guard -Force
}
if (Test-Path "$w\$n.hwpx") {
  Remove-Item "$w\hwpcmp\$n.pdf" -ErrorAction SilentlyContinue
  & E:\OCTO\DOCS\tools\m0\hwp_export.ps1 -Out "$w\hwp" -Files "$w\$n.hwpx"
  if (Test-Path "$w\hwp\$n.pdf") { Copy-Item "$w\hwp\$n.pdf" "$w\hwpcmp\$n.pdf" -Force }
}
foreach ($k in 'word', 'hwpcmp') {
  if (-not (Test-Path "$w\$k\$n.pdf")) { "FAIL ${k}: PDF 없음"; $fail++; continue }
  python -I E:\OCTO\DOCS\tools\m0\cmp_pdf.py "$w\edge" - "$w\$k" - "$w\cmp_$k.json" "$n|$w\cmp_$k.png" 2> $null
  $r = Get-Content "$w\cmp_$k.json" -Raw -Encoding utf8 | ConvertFrom-Json
  "RESULT $n $k pages=$($r[0].pages_a)/$($r[0].pages_b) raw=$($r[0].raw_mean) aligned=$($r[0].aligned_mean) shift_mm=($($r[0].shift_dx_mm),$($r[0].shift_dy_mm))"
}
if ($fail) { "FAILED $fail"; exit 1 }
"OK"
