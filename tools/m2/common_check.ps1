# M2 공통 명령 검증(7절 "같은 명령이 docx와 HWPX에서 모두 동작"):
# HWPX·docx 시료 전부에 같은 명령 묶음(셀 쓰기 + 마지막 문단에 덧붙이기 + 굵게)을 docsocto로 돌려 보존 저장하고,
# 한글·Word로 원본과 결과를 PDF로 내보내 비교한다. Word·한글은 제한 시간·확인 창 자동 누름.
# usage: common_check.ps1 [-Out E:\OCTO\work\m2_out\common] [-Limit 55]
param([string]$Out = 'E:\OCTO\work\m2_out\common', [int]$Limit = 55, [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M2')
$ErrorActionPreference = 'Continue'
$tools = Split-Path $PSScriptRoot; $m0 = Join-Path $tools 'm0'
$x = 'E:\OCTO\DOCS\target\release\docsocto.exe'
foreach ($k in 'out','script','hwp_o','hwp_e','word_o','word_e') { New-Item -ItemType Directory -Force (Join-Path $Out $k) | Out-Null }
New-Item -ItemType Directory -Force $Share | Out-Null
$log = Join-Path $Out 'common_check.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
. (Join-Path $m0 'dialogs.ps1'); $dlog = Join-Path $Out 'dialogs.log'
$utf8 = New-Object Text.UTF8Encoding $false
function Run-Limited($procName, $script, [string[]]$argv, $limit) {
  Get-Process $procName -ErrorAction SilentlyContinue | Stop-Process -Force
  $o = Join-Path $Out "_$procName.out"
  $al = @('-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$script`"") + ($argv | % { "`"$_`"" })
  $p = Start-Process powershell -ArgumentList $al -RedirectStandardOutput $o -RedirectStandardError (Join-Path $Out "_$procName.err") -PassThru -WindowStyle Hidden
  $end = (Get-Date).AddSeconds($limit)
  while (-not $p.WaitForExit(2000) -and (Get-Date) -lt $end) { $null = Close-Dialogs $procName $dlog }
  if (-not $p.HasExited) { $p.Kill(); Get-Process $procName -ErrorAction SilentlyContinue | Stop-Process -Force; return 'timeout' }
  return ((Get-Content $o -Encoding utf8) -join ' ')
}
$samples = @(Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_') + @(Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_')
$hwpO = @(); $hwpE = @()
foreach ($f in $samples) {
  $b = $f.BaseName; $ext = $f.Extension; $outp = Join-Path $Out "out\$b$ext"
  # Out-String 은 창 너비로 긴 줄을 자를 수 있다 — 줄을 그대로 잇는다. 읽기 실패 때 앞 파일 값이 남지 않게 매번 비운다.
  $j = $null; $last = $null
  $info = (& $x inspect $f.FullName 2> $null) -join "`n"
  try { $j = $info | ConvertFrom-Json } catch { $j = $null }
  if (-not $j) { L "$($f.Name) OPEN FAILED"; continue }
  $last = @($j.text) | Select-Object -Last 1
  $ok = $false
  foreach ($rc in @(@(1,1), @(0,1), @(1,0), @(0,0))) {
    $cmds = @()
    if (@($j.tables).Count -gt 0) { $cmds += @{ command = 'table.setCell'; params = @{ table = 0; row = $rc[0]; col = $rc[1]; text = 'DocsOcto 공통 명령 시험' } } }
    if ($last) {
      $n = ([string]$last.text).Length
      $cmds += @{ command = 'text.append'; params = @{ section = $last.section; para = $last.para; text = ' [DocsOcto]' } }
      $cmds += @{ command = 'format.bold'; params = @{ section = $last.section; para = $last.para; start = $n + 1; end = $n + 11 } }
    }
    $sj = Join-Path $Out "script\$b.json"
    [IO.File]::WriteAllText($sj, (@{ commands = $cmds } | ConvertTo-Json -Depth 6), $utf8)
    & $x run $f.FullName --script $sj --save $outp --overwrite > (Join-Path $Out "out\$b.log") 2> (Join-Path $Out "out\$b.err")
    if ($LASTEXITCODE -eq 0) { $ok = $true; break }
  }
  if (-not $ok) { L "$($f.Name) RUN FAILED: $((Get-Content (Join-Path $Out "out\$b.err") -Encoding utf8) -join ' ')"; continue }
  if ($ext -eq '.hwpx') { $hwpO += $f.FullName; $hwpE += $outp; L "$($f.Name) saved" }
  else {
    $ro = Run-Limited 'WINWORD' (Join-Path $m0 'word_pdf.ps1') @($f.FullName, (Join-Path $Out "word_o\$b.pdf")) $Limit
    $re = Run-Limited 'WINWORD' (Join-Path $m0 'word_pdf.ps1') @($outp, (Join-Path $Out "word_e\$b.pdf")) $Limit
    L "$($f.Name) saved word_orig=[$ro] word_edit=[$re]"
  }
}
foreach ($pair in @(@('hwp_o', $hwpO), @('hwp_e', $hwpE))) {
  $lst = Join-Path $Out "$($pair[0]).txt"; [IO.File]::WriteAllLines($lst, [string[]]$pair[1], $utf8)
  $r = Run-Limited 'Hwp' (Join-Path $m0 'hwp_export.ps1') @('-Out', (Join-Path $Out $pair[0]), '-ListFile', $lst) ($Limit * [Math]::Max(1, $pair[1].Count))
  L "hangul $($pair[0]): $(if ($r -eq 'timeout') { 'timeout' } else { 'done' })"
  Get-Content (Join-Path $Out "$($pair[0])\hwp_export.log") -Encoding utf8 -ErrorAction SilentlyContinue | Where-Object { $_ -match 'open=' } | % { L "  $_" }
}
$day = Get-Date -Format yyMMdd
L '== cmp hwpx'
python -I (Join-Path $m0 'cmp_pdf.py') (Join-Path $Out 'hwp_o') - (Join-Path $Out 'hwp_e') - (Join-Path $Out 'cmp_hwpx.json') "05_산업부_NTB수요기술_등록동의서|$Share\${day}_공통명령_HWPX_한글비교.png" 2> (Join-Path $Out 'cmp.err')
L '== cmp docx'
python -I (Join-Path $m0 'cmp_pdf.py') (Join-Path $Out 'word_o') - (Join-Path $Out 'word_e') - (Join-Path $Out 'cmp_docx.json') "01_경북IT융합산업기술원_기업지원신청서|$Share\${day}_공통명령_docx_Word비교.png" 2>> (Join-Path $Out 'cmp.err')
L "== COMMON_CHECK_DONE $(Get-Date -Format s)"