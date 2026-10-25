# M0 xconv, Word stage + compare. Each file runs in its own process with a time limit,
# so one hung document is killed and recorded instead of stalling the whole run.
# usage: xconv_word.ps1 [-Out E:\OCTO\work\m0_out\xconv] [-Limit 55]
param([string]$Out = 'E:\OCTO\work\m0_out\xconv', [int]$Limit = 55,
      [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M0')
$ErrorActionPreference = 'Continue'
$docx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_'
$log = Join-Path $Out 'xconv_word.log'
"== start $(Get-Date -Format s) limit=${Limit}s" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
New-Item -ItemType Directory -Force (Join-Path $Out 'h2d_word'), (Join-Path $Out 'd2h_word') | Out-Null
$jobs = @()
Get-ChildItem (Join-Path $Out 'h2d_docx\*.docx') | % { $jobs += ,@('h2d', $_.FullName, (Join-Path $Out "h2d_word\$($_.BaseName).pdf")) }
$docx | % { $jobs += ,@('d2h', $_.FullName, (Join-Path $Out "d2h_word\$($_.BaseName).pdf")) }
$one = Join-Path $PSScriptRoot 'word_pdf.ps1'
. (Join-Path $PSScriptRoot 'dialogs.ps1')
$dlog = Join-Path $Out 'dialogs.log'
$rows = @()
foreach ($j in $jobs) {
  Get-Process WINWORD -ErrorAction SilentlyContinue | Stop-Process -Force
  $o = Join-Path $Out '_w.out'; $e = Join-Path $Out '_w.err'
  $t0 = Get-Date
  $p = Start-Process powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$one`"","`"$($j[1])`"","`"$($j[2])`"" -RedirectStandardOutput $o -RedirectStandardError $e -PassThru -WindowStyle Hidden
  $st = 'ok'
  $end = (Get-Date).AddSeconds($Limit)
  while (-not $p.WaitForExit(2000) -and (Get-Date) -lt $end) { $null = Close-Dialogs 'WINWORD' $dlog }
  if (-not $p.HasExited) { $st = 'timeout'; $p.Kill(); Get-Process WINWORD -ErrorAction SilentlyContinue | Stop-Process -Force }
  $pg = (Select-String -Path $o -Pattern '^PAGES=(\d+)' -ErrorAction SilentlyContinue).Matches.Groups[1].Value
  if ($st -eq 'ok' -and -not $pg) { $m = (Get-Content $e -ErrorAction SilentlyContinue) -join ' '; $st = 'error: ' + $m.Substring(0, [Math]::Min(200, $m.Length)) }
  $r = [pscustomobject]@{ dir = $j[0]; file = (Split-Path $j[1] -Leaf); word_pages = $pg; sec = [int]((Get-Date) - $t0).TotalSeconds; status = $st }
  $rows += $r; L "$($r.dir) $($r.file) pages=$pg $($r.sec)s $st"
}
$rows | ConvertTo-Json | Out-File (Join-Path $Out 'xconv_word.json') -Encoding utf8
New-Item -ItemType Directory -Force $Share | Out-Null
$day = Get-Date -Format yyMMdd
$h1 = (Get-ChildItem (Join-Path $Out 'h2d_src\05_*.pdf'))[0].BaseName
$d1 = ($docx | Select-Object -First 1).BaseName
L "== cmp h2d"
python -I "$PSScriptRoot\cmp_pdf.py" (Join-Path $Out 'h2d_src') - (Join-Path $Out 'h2d_word') - (Join-Path $Out 'cmp_h2d.json') "$h1|$Share\${day}_변환비교_HWPX_docx.png" 2>> (Join-Path $Out 'cmp.err')
L "== cmp d2h"
python -I "$PSScriptRoot\cmp_pdf.py" (Join-Path $Out 'd2h_word') - (Join-Path $Out 'd2h_hwp') - (Join-Path $Out 'cmp_d2h.json') "$d1|$Share\${day}_변환비교_docx_HWPX.png" 2>> (Join-Path $Out 'cmp.err')
L "== XCONV_WORD_DONE $(Get-Date -Format s)"
