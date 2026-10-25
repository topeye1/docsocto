# M1-5 ohwp 검증: 양식마다 ohwp 명령(셀 쓰기 + 05는 행 넣기·셀 합치기)으로 고쳐 보존 저장하고,
# 한글에서 열어 PDF로 내보낸 뒤 원본 한글 PDF와 비교한다.
# usage: ohwp_check.ps1 [-Out E:\OCTO\work\m1_out\ohwp] [-Limit 55]
param([string]$Out = 'E:\OCTO\work\m1_out\ohwp', [int]$Limit = 55, [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M1')
$ErrorActionPreference = 'Continue'
$m0 = Join-Path (Split-Path $PSScriptRoot) 'm0'
$x = 'E:\OCTO\DOCS\target\release\ohwp.exe'
foreach ($k in 'hwpx','pdf','script') { New-Item -ItemType Directory -Force (Join-Path $Out $k) | Out-Null }
$log = Join-Path $Out 'ohwp_check.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
. (Join-Path $m0 'dialogs.ps1'); $dlog = Join-Path $Out 'dialogs.log'
$utf8 = New-Object Text.UTF8Encoding $false
$files = @()
foreach ($f in Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_') {
  $b = $f.BaseName; $p = Join-Path $Out "hwpx\$b.hwpx"
  $done = $false
  foreach ($rc in @(@(1,1), @(0,1), @(1,0), @(0,0))) {
    $cmds = @(@{ command = 'table.setCell'; params = @{ table = 0; row = $rc[0]; col = $rc[1]; text = 'DocsOcto M1 시험' } })
    if ($b -like '05_*') {
      $cmds += @{ command = 'table.insertRow'; params = @{ table = 0; row = 4; below = $true } }
      $cmds += @{ command = 'table.setCell'; params = @{ table = 0; row = 5; col = 0; text = '추가 행' } }
      $cmds += @{ command = 'table.merge'; params = @{ table = 0; startRow = 5; startCol = 1; endRow = 5; endCol = 3 } }
    }
    $sj = Join-Path $Out "script\$b.json"
    [IO.File]::WriteAllText($sj, (@{ commands = $cmds } | ConvertTo-Json -Depth 6), $utf8)
    & $x run $f.FullName --script $sj --save $p --report (Join-Path $Out "hwpx\$b.report.json") > (Join-Path $Out "hwpx\$b.out") 2> (Join-Path $Out "hwpx\$b.err")
    if ($LASTEXITCODE -eq 0 -and (Test-Path $p)) { $done = $true; L "$b cell=($($rc -join ',')) saved"; break }
  }
  if (-not $done) { L "$b FAILED: $((Get-Content (Join-Path $Out "hwpx\$b.err") -Encoding utf8) -join ' ')"; continue }
  $files += $p
}
$lst = Join-Path $Out 'files.txt'; [IO.File]::WriteAllLines($lst, [string[]]$files, $utf8)
Get-Process Hwp -ErrorAction SilentlyContinue | Stop-Process -Force
$pr = Start-Process powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$(Join-Path $m0 'hwp_export.ps1')`"",'-Out',"`"$(Join-Path $Out 'pdf')`"",'-ListFile',"`"$lst`"" -PassThru -WindowStyle Hidden
$end = (Get-Date).AddSeconds($Limit * [Math]::Max(1, $files.Count))
while (-not $pr.WaitForExit(2000) -and (Get-Date) -lt $end) { $null = Close-Dialogs 'Hwp' $dlog }
if (-not $pr.HasExited) { $pr.Kill(); Get-Process Hwp -ErrorAction SilentlyContinue | Stop-Process -Force; L 'hangul export: timeout' }
Get-Content (Join-Path $Out 'pdf\hwp_export.log') -Encoding utf8 -ErrorAction SilentlyContinue | % { L "  $_" }
L '== cmp hangul: original vs ohwp'
python -I (Join-Path $m0 'cmp_pdf.py') 'E:\OCTO\work\m0_out\hwp1' - (Join-Path $Out 'pdf') - (Join-Path $Out 'cmp_ohwp.json') "05_산업부_NTB수요기술_등록동의서|$Share\$(Get-Date -Format yyMMdd)_ohwp_명령_한글비교.png" 2> (Join-Path $Out 'cmp.err')
L "== OHWP_CHECK_DONE $(Get-Date -Format s)"