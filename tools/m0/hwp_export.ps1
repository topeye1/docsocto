# M0: open each HWPX in Hancom Office (COM) and export PDF, one page per sheet.
# Files whose settings.xml asks for N-up printing (PrintMethod != 0) are copied
# to a temp folder with PrintMethod=0 first; the document body is untouched.
# usage: hwp_export.ps1 -Out <dir> -Files <hwpx...>
param([string]$Out, [string[]]$Files)
$ErrorActionPreference = 'Continue'
New-Item -ItemType Directory -Force $Out | Out-Null
$tmp = Join-Path $Out '_1up'
New-Item -ItemType Directory -Force $tmp | Out-Null
$log = Join-Path $Out 'hwp_export.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8

$fix = Join-Path $PSScriptRoot 'one_up.py'
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
$rows | ConvertTo-Json | Out-File (Join-Path $Out 'hwp_results.json') -Encoding utf8
"== done $(Get-Date -Format s)" | Out-File $log -Append -Encoding utf8
