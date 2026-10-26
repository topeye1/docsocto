# M0: open each HWPX in Hancom Office (COM) and export PDF, one page per sheet.
# Files whose settings.xml asks for N-up printing (PrintMethod != 0) are copied
# to a temp folder with PrintMethod=0 first; the document body is untouched.
# usage: hwp_export.ps1 -Out <dir> -Files <hwpx...>  또는  -ListFile <한 줄에 한 경로인 파일>
param([string]$Out, [string[]]$Files, [string]$ListFile)
# powershell -File 로 부르면 -Files 에 첫 경로만 들어간다 — 여러 파일은 -ListFile 로 넘긴다.
if ($ListFile) { $Files = @(Get-Content $ListFile -Encoding utf8 | Where-Object { $_ }) }
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $Out | Out-Null
$tmp = Join-Path $Out '_1up'
New-Item -ItemType Directory -Force $tmp | Out-Null
$log = Join-Path $Out 'hwp_export.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8

$fix = Join-Path $PSScriptRoot 'one_up.py'
# 확인 창 지킴이(결정 #29): 한글이 도는 동안 2초마다 확인 창을 눌러 dialogs.log에 남긴다 — 파일 접근 허용 창에 68분 멈춘 일(M3-5)
$guard = Start-Job -ArgumentList $PSScriptRoot, (Join-Path $Out 'dialogs.log') -ScriptBlock {
  param($root, $dlog)
  . (Join-Path $root 'dialogs.ps1')
  while ($true) { Close-Dialogs @('Hwp', 'HwpFrame') $dlog | Out-Null; Start-Sleep 2 }
}
$hwp = New-Object -ComObject HWPFrame.HwpObject
try { $hwp.XHwpWindows.Item(0).Visible = $false } catch {}
try { $null = $hwp.RegisterModule('FilePathCheckDLL', 'FilePathCheckerModule') } catch {}

$rows = @()
foreach ($src in $Files) {
  $base = [IO.Path]::GetFileNameWithoutExtension($src)
  $row = [ordered]@{ file = [IO.Path]::GetFileName($src) }
  $in = Join-Path $tmp ([IO.Path]::GetFileName($src))
  $row.nup = (python -I $fix $src $in)
  try {
    $row.open = $hwp.Open($in, 'HWPX', 'forceopen:true;versionwarning:false;suspendpassword:true')
    if ($row.open) {
      $row.pages = $hwp.PageCount
      $row.pdf = $hwp.SaveAs((Join-Path $Out "$base.pdf"), 'PDF', '')
      $hwp.Clear(1)
    }
  } catch { $row.err = $_.Exception.Message }
  $rows += [pscustomobject]$row
  "$($row.file) nup=$($row.nup) open=$($row.open) pages=$($row.pages) pdf=$($row.pdf) $($row.err)" | Out-File $log -Append -Encoding utf8
}
try { $hwp.Quit() } catch {}
Stop-Job $guard; Remove-Job $guard -Force
$rows | ConvertTo-Json | Out-File (Join-Path $Out 'hwp_results.json') -Encoding utf8
"== done $(Get-Date -Format s)" | Out-File $log -Append -Encoding utf8
