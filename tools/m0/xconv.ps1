# M0 cross-format judgement: convert, then compare the PDFs of the two formats.
#   h2d: HWPX --Hangul SaveAs--> docx ; PDF(HWPX by Hangul) vs PDF(docx by Word)
#   d2h: docx --Hangul SaveAs--> HWPX ; PDF(docx by Word)   vs PDF(HWPX by Hangul)
# The converter is Hangul for now (Octo Docs's own converter is M4); the pipeline
# and comparison stay the same when the converter is swapped.
# usage: xconv.ps1 [-Out E:\OCTO\work\m0_out\xconv]
param([string]$Out = 'E:\OCTO\work\m0_out\xconv',
      [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M0')
$ErrorActionPreference = 'Continue'
$hwpx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0\*.hwpx' | Where-Object Name -match '^\d\d_'
$docx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_'
$d = @{}
foreach ($k in 'h2d_src','h2d_docx','h2d_word','d2h_word','d2h_hwpx','d2h_hwp','_1up') {
  $d[$k] = Join-Path $Out $k; New-Item -ItemType Directory -Force $d[$k] | Out-Null
}
$log = Join-Path $Out 'xconv.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }

# 1) Hangul: HWPX -> PDF (1-up) and -> docx ; docx -> HWPX -> PDF
$fix = Join-Path $PSScriptRoot 'one_up.py'
$hwp = New-Object -ComObject HWPFrame.HwpObject
try { $hwp.XHwpWindows.Item(0).Visible = $false } catch {}
try { $null = $hwp.RegisterModule('FilePathCheckDLL', 'FilePathCheckerModule') } catch {}
$opt = 'forceopen:true;versionwarning:false;suspendpassword:true'
$rows = @()
foreach ($f in $hwpx) {
  $b = $f.BaseName; $r = [ordered]@{ dir = 'h2d'; file = $f.Name }
  $in = Join-Path $d._1up $f.Name
  $null = python -I $fix $f.FullName $in
  try {
    if ($hwp.Open($in, 'HWPX', $opt)) {
      $r.src_pages = $hwp.PageCount
      $r.src_pdf = $hwp.SaveAs((Join-Path $d.h2d_src "$b.pdf"), 'PDF', '')
      $r.conv = $hwp.SaveAs((Join-Path $d.h2d_docx "$b.docx"), 'OOXML', '')
      $hwp.Clear(1)
    } else { $r.err = 'open failed' }
  } catch { $r.err = $_.Exception.Message }
  $rows += [pscustomobject]$r; L "$($r.dir) $($r.file) pages=$($r.src_pages) pdf=$($r.src_pdf) conv=$($r.conv) $($r.err)"
}
foreach ($f in $docx) {
  $b = $f.BaseName; $r = [ordered]@{ dir = 'd2h'; file = $f.Name }
  $hx = Join-Path $d.d2h_hwpx "$b.hwpx"
  try {
    if ($hwp.Open($f.FullName, '', $opt)) {
      $r.conv = $hwp.SaveAs($hx, 'HWPX', ''); $hwp.Clear(1)
      if ($r.conv -and $hwp.Open($hx, 'HWPX', $opt)) {
        $r.dst_pages = $hwp.PageCount
        $r.dst_pdf = $hwp.SaveAs((Join-Path $d.d2h_hwp "$b.pdf"), 'PDF', '')
        $hwp.Clear(1)
      }
    } else { $r.err = 'open failed' }
  } catch { $r.err = $_.Exception.Message }
  $rows += [pscustomobject]$r; L "$($r.dir) $($r.file) conv=$($r.conv) pages=$($r.dst_pages) pdf=$($r.dst_pdf) $($r.err)"
}
try { $hwp.Quit() } catch {}

# 2) Word: docx -> PDF (converted docx for h2d, original docx for d2h)
$word = New-Object -ComObject Word.Application
$word.Visible = $false; $word.DisplayAlerts = 0
function WordPdf($src, $pdf) {
  $doc = $word.Documents.Open($src, $false, $true, $false)
  $n = $doc.ComputeStatistics(2)
  $doc.ExportAsFixedFormat($pdf, 17); $doc.Close(0); $n
}
foreach ($r in $rows) {
  $b = [IO.Path]::GetFileNameWithoutExtension($r.file)
  try {
    if ($r.dir -eq 'h2d' -and $r.conv) {
      $r | Add-Member dst_pages (WordPdf (Join-Path $d.h2d_docx "$b.docx") (Join-Path $d.h2d_word "$b.pdf"))
    } elseif ($r.dir -eq 'd2h') {
      $r | Add-Member src_pages (WordPdf $docx.Where({ $_.BaseName -eq $b })[0].FullName (Join-Path $d.d2h_word "$b.pdf"))
    }
  } catch { $r | Add-Member -Force werr $_.Exception.Message }
  L "word $($r.dir) $($r.file) src=$($r.src_pages) dst=$($r.dst_pages) $($r.werr)"
}
try { $word.Quit() } catch {}
$rows | ConvertTo-Json | Out-File (Join-Path $Out 'xconv_results.json') -Encoding utf8

# 3) compare: source-format PDF vs converted-format PDF
New-Item -ItemType Directory -Force $Share | Out-Null
$day = Get-Date -Format yyMMdd
$h1 = ($hwpx | Where-Object Name -match '^05_')[0].BaseName
$d1 = ($docx | Select-Object -First 1).BaseName
L "== cmp h2d"
python -I "$PSScriptRoot\cmp_pdf.py" $d.h2d_src '' $d.h2d_word '' (Join-Path $Out 'cmp_h2d.json') "$h1|$Share\${day}_변환비교_HWPX_docx.png" 2>> (Join-Path $Out 'cmp.err')
L "== cmp d2h"
python -I "$PSScriptRoot\cmp_pdf.py" $d.d2h_word '' $d.d2h_hwp '' (Join-Path $Out 'cmp_d2h.json') "$d1|$Share\${day}_변환비교_docx_HWPX.png" 2>> (Join-Path $Out 'cmp.err')
L "== XCONV_DONE $(Get-Date -Format s)"
