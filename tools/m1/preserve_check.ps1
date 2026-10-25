# M1-1 보존 저장 검증 (진행일지 A3).
#   HWPX: O=양식, B0=ohwp resave(진짜 무변경 저장), B1=rhwp 셀 쓰기(M0 산출물) → P=보존 저장
#         한글로 P를 PDF로 내보내 원본 한글 PDF와 비교, 패키지 항목 비교.
#   docx: O=시험 docx, B0=WordCraft 그냥 저장, B1=WordCraft 문서 끝에 한 줄 쓰기 → P=보존 저장
#         Word로 P를 PDF로 내보내 원본 Word PDF와 비교, 패키지 항목 비교.
# Word·한글은 파일마다 별도 프로세스, 제한 시간, 확인 창 자동 누름(tools/m0/dialogs.ps1).
# usage: preserve_check.ps1 [-Out E:\OCTO\work\m1_out\preserve] [-Limit 55]
param([string]$Out = 'E:\OCTO\work\m1_out\preserve', [int]$Limit = 55,
      [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M1')
$ErrorActionPreference = 'Continue'
$m0 = Join-Path (Split-Path $PSScriptRoot) 'm0'
$pres = 'E:\OCTO\DOCS\target\release\octodocs-preserve.exe'
$ohwp = 'E:\OCTO\DOCS\target\release\ohwp.exe'
$w0 = 'E:\OCTO\work\m0_out'
foreach ($k in 'hwpx','hwp_pdf','docx','word_pdf','b0') { New-Item -ItemType Directory -Force (Join-Path $Out $k) | Out-Null }
New-Item -ItemType Directory -Force $Share | Out-Null
$log = Join-Path $Out 'preserve_check.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
. (Join-Path $m0 'dialogs.ps1'); $dlog = Join-Path $Out 'dialogs.log'

function Run-Limited($procName, $script, [string[]]$argv, $tag) {
  Get-Process $procName -ErrorAction SilentlyContinue | Stop-Process -Force
  $o = Join-Path $Out "_$tag.out"; $e = Join-Path $Out "_$tag.err"
  $al = @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$script`"") + ($argv | % { "`"$_`"" })
  $p = Start-Process powershell -ArgumentList $al -RedirectStandardOutput $o -RedirectStandardError $e -PassThru -WindowStyle Hidden
  $end = (Get-Date).AddSeconds($Limit)
  while (-not $p.WaitForExit(2000) -and (Get-Date) -lt $end) { $null = Close-Dialogs $procName $dlog }
  if (-not $p.HasExited) { $p.Kill(); Get-Process $procName -ErrorAction SilentlyContinue | Stop-Process -Force; return 'timeout' }
  return ((Get-Content $o -Encoding utf8) -join ' ')
}

# ---- HWPX
$hwpOut = @()
foreach ($f in Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_') {
  $b = $f.BaseName
  $b0 = Join-Path $Out "b0\$b.hwpx"; $b1 = Join-Path $w0 "${b}_edit.hwpx"; $p = Join-Path $Out "hwpx\$b.hwpx"
  & $ohwp resave $f.FullName $b0 2> (Join-Path $Out "b0\$b.err")
  if (-not (Test-Path $b0)) { L "hwpx $b B0 failed"; continue }
  & $pres $f.FullName $b0 $b1 $p --report (Join-Path $Out "hwpx\$b.json") 2> (Join-Path $Out "hwpx\$b.err")
  $diff = python -I (Join-Path $m0 'zip_diff.py') $f.FullName $p | ConvertFrom-Json
  L "hwpx $b preserve=$LASTEXITCODE changed=[$($diff.changed -join ',')] removed=$(@($diff.removed).Count)"
  $hwpOut += $p
}
# 한글 내보내기: 한 프로세스에서 여러 파일 — 전체 제한은 파일 수 × 제한 시간
$Limit0 = $Limit; $Limit = $Limit0 * [Math]::Max(1, $hwpOut.Count)
$lst = Join-Path $Out 'hwp_files.txt'; [IO.File]::WriteAllLines($lst, [string[]]$hwpOut, (New-Object Text.UTF8Encoding $false))
$r = Run-Limited 'Hwp' (Join-Path $m0 'hwp_export.ps1') @('-Out', (Join-Path $Out 'hwp_pdf'), '-ListFile', $lst) 'hwp'
$Limit = $Limit0
L "hangul export: $(if ($r -eq 'timeout') { 'timeout' } else { 'done' })"
Get-Content (Join-Path $Out 'hwp_pdf\hwp_export.log') -Encoding utf8 -ErrorAction SilentlyContinue | % { L "  $_" }
L "== cmp hangul: original vs preserved"
python -I (Join-Path $m0 'cmp_pdf.py') "$w0\hwp1" - (Join-Path $Out 'hwp_pdf') - (Join-Path $Out 'cmp_hwpx.json') "05_산업부_NTB수요기술_등록동의서|$Share\$(Get-Date -Format yyMMdd)_보존저장_HWPX_한글비교.png" 2>> (Join-Path $Out 'cmp.err')

# ---- docx
foreach ($f in Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_') {
  $b = $f.BaseName
  $b0 = "$w0\wc_noop\edit\$b.docx"; $b1 = "$w0\wc\edit\$b.docx"; $p = Join-Path $Out "docx\$b.docx"
  & $pres $f.FullName $b0 $b1 $p --report (Join-Path $Out "docx\$b.json") 2> (Join-Path $Out "docx\$b.err")
  $pe = $LASTEXITCODE
  $diff = python -I (Join-Path $m0 'zip_diff.py') $f.FullName $p | ConvertFrom-Json
  $r = Run-Limited 'WINWORD' (Join-Path $m0 'word_pdf.ps1') @($p, (Join-Path $Out "word_pdf\$b.pdf")) 'word'
  L "docx $b preserve=$pe changed=[$($diff.changed -join ',')] removed=$(@($diff.removed).Count) word=$r"
}
L "== cmp word: original vs preserved"
python -I (Join-Path $m0 'cmp_pdf.py') "$w0\xconv\d2h_word" - (Join-Path $Out 'word_pdf') - (Join-Path $Out 'cmp_docx.json') "01_경북IT융합산업기술원_기업지원신청서|$Share\$(Get-Date -Format yyMMdd)_보존저장_docx_Word비교.png" 2>> (Join-Path $Out 'cmp.err')
L "== PRESERVE_CHECK_DONE $(Get-Date -Format s)"