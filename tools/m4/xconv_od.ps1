# M4 docx↔HWPX 변환 판정(결정 #25) — 변환기를 한글 대신 Octo Docs(octodocs convert)로.
#   h2d: HWPX → docx(octodocs) ; 한글 PDF(원본 HWPX, M0 기준선) vs Word PDF(변환 docx)
#   d2h: docx → HWPX(octodocs) ; Word PDF(원본 docx, M0 기준선) vs 한글 PDF(변환 HWPX)
# usage: xconv_od.ps1 [-Out E:\OCTO\work\m4x] [-Max 30] [-Only h2d|d2h]
#   기준 PDF 는 M0 xconv(E:\OCTO\work\m0_out\xconv\h2d_src·d2h_word)를 그대로 쓴다.
#   결과: <Out>\cmp_h2d.json·cmp_d2h.json, 파일마다 변환 보고서 <Out>\report\<이름>.json
param([string]$Out = 'E:\OCTO\work\m4x', [int]$Max = 30, [string]$Only = '')
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$x = 'E:\OCTO\DOCS\target\release\octodocs.exe'
$base = 'E:\OCTO\work\m0_out\xconv'
foreach ($k in 'h2d_docx', 'h2d_word', 'd2h_hwpx', 'd2h_hwp', 'report') { New-Item -ItemType Directory -Force (Join-Path $Out $k) | Out-Null }
$log = Join-Path $Out 'xconv_od.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
. E:\OCTO\DOCS\tools\m0\dialogs.ps1

if ($Only -ne 'd2h') {
  $hwpx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_' | Select-Object -First $Max
  foreach ($f in $hwpx) {
    $b = $f.BaseName; $dx = Join-Path $Out "h2d_docx\$b.docx"
    Remove-Item $dx -ErrorAction SilentlyContinue
    & $x convert $f.FullName $dx --report (Join-Path $Out "report\$b.h2d.json") > $null 2> (Join-Path $Out "report\$b.h2d.err")
    L "h2d convert $b exit=$LASTEXITCODE"
  }
  # Word PDF — 파일마다 따로 띄우고 55초 넘으면 끊는다, 확인 창 지킴이
  $guard = Start-Job -ScriptBlock { . E:\OCTO\DOCS\tools\m0\dialogs.ps1; while ($true) { Close-Dialogs @('WINWORD') 'E:\OCTO\work\m4x_dialogs.log' | Out-Null; Start-Sleep 2 } }
  foreach ($f in Get-ChildItem (Join-Path $Out 'h2d_docx\*.docx')) {
    $pdf = Join-Path $Out "h2d_word\$($f.BaseName).pdf"
    Remove-Item $pdf -ErrorAction SilentlyContinue
    $p = Start-Process powershell -ArgumentList @('-NoProfile', '-File', 'E:\OCTO\DOCS\tools\m0\word_pdf.ps1', '-Src', $f.FullName, '-Pdf', $pdf) -PassThru -WindowStyle Hidden
    if (-not $p.WaitForExit(55000)) { $p.Kill(); Get-Process WINWORD -ErrorAction SilentlyContinue | Stop-Process -Force; L "h2d word TIMEOUT $($f.BaseName)" }
    L "h2d word $($f.BaseName) pdf=$(Test-Path $pdf)"
  }
  Stop-Job $guard; Remove-Job $guard -Force
  python -I E:\OCTO\DOCS\tools\m0\cmp_pdf.py "$base\h2d_src" - (Join-Path $Out 'h2d_word') - (Join-Path $Out 'cmp_h2d.json') 2> (Join-Path $Out 'cmp_h2d.err') | Out-Null
}
if ($Only -ne 'h2d') {
  $docx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_' | Select-Object -First $Max
  $list = Join-Path $Out 'd2h_list.txt'
  Remove-Item $list -ErrorAction SilentlyContinue
  foreach ($f in $docx) {
    $b = $f.BaseName; $hx = Join-Path $Out "d2h_hwpx\$b.hwpx"
    Remove-Item $hx -ErrorAction SilentlyContinue
    & $x convert $f.FullName $hx --report (Join-Path $Out "report\$b.d2h.json") > $null 2> (Join-Path $Out "report\$b.d2h.err")
    L "d2h convert $b exit=$LASTEXITCODE"
    if (Test-Path $hx) { $hx | Out-File $list -Append -Encoding utf8 }
  }
  if (Test-Path $list) { & E:\OCTO\DOCS\tools\m0\hwp_export.ps1 -Out (Join-Path $Out 'd2h_hwp') -ListFile $list | Out-Null }
  python -I E:\OCTO\DOCS\tools\m0\cmp_pdf.py "$base\d2h_word" - (Join-Path $Out 'd2h_hwp') - (Join-Path $Out 'cmp_d2h.json') 2> (Join-Path $Out 'cmp_d2h.err') | Out-Null
}
# 요약: 파일마다 쪽 수(원본/변환)·차이율, 그리고 M0(한글이 변환기)일 때 값
python -I E:\OCTO\DOCS\tools\m4\xconv_sum.py $Out $base
"== end $(Get-Date -Format s)" | Out-File $log -Append -Encoding utf8
