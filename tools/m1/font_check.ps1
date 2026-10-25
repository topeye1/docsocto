# M1-4 글꼴 확인: rhwp에 한컴오피스 글꼴 폴더를 더해 렌더한 PDF를 한글 PDF와 비교한다(진행일지 A7).
# usage: font_check.ps1 [-Out E:\OCTO\work\m1_out\font]
param([string]$Out = 'E:\OCTO\work\m1_out\font', [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M1')
$ErrorActionPreference = 'Continue'
$m0 = Join-Path (Split-Path $PSScriptRoot) 'm0'
$rh = 'E:\OCTO\work\rhwp\target\release\rhwp.exe'
$hnc = Get-ChildItem 'C:\Program Files (x86)\Hnc','C:\Program Files\Hnc' -Recurse -Directory -Filter TTF -ErrorAction SilentlyContinue | Select-Object -First 1
$fp = @()
if ($hnc) { Get-ChildItem $hnc.FullName -Directory | % { $fp += '--font-path'; $fp += $_.FullName } }
New-Item -ItemType Directory -Force $Out | Out-Null
$log = Join-Path $Out 'font_check.log'
"== start $(Get-Date -Format s) fontpaths=$($fp -join ' ')" | Out-File $log -Encoding utf8
foreach ($f in Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_') {
  $pdf = Join-Path $Out "$($f.BaseName).pdf"
  & $rh export-pdf $f.FullName -o $pdf @fp > $null 2> (Join-Path $Out "$($f.BaseName).err")
  "$($f.BaseName) exit=$LASTEXITCODE size=$((Get-Item $pdf -ErrorAction SilentlyContinue).Length)" | Out-File $log -Append -Encoding utf8
}
"== cmp hangul vs rhwp(+hancom fonts)" | Out-File $log -Append -Encoding utf8
python -I (Join-Path $m0 'cmp_pdf.py') 'E:\OCTO\work\m0_out\hwp1' - $Out - (Join-Path $Out 'cmp_font.json') "05_산업부_NTB수요기술_등록동의서|$Share\$(Get-Date -Format yyMMdd)_글꼴폴더_한글_rhwp비교.png" 2> (Join-Path $Out 'cmp.err')
"== FONT_CHECK_DONE $(Get-Date -Format s)" | Out-File $log -Append -Encoding utf8