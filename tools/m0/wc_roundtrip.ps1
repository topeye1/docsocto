# M0 WordCraft docx round trip (decision #3/#5):
#   1) WordCraft: original docx -> PDF ; edit (type a line at the start) -> save docx + PDF
#   2) Word: edited docx -> PDF (per-file process, time limit, dialog auto-click)
#   3) compare  a) Word(original) vs Word(edited)   : nothing but the edit changed
#               b) Word(original) vs WordCraft(original) : same rendering
#      plus a part-by-part diff of original vs edited package.
#   -AtEnd: type the line at the document end (only the last page should change).
#   -NoEdit: open and save only (no typing) — isolates what saving alone changes.
# usage: wc_roundtrip.ps1 [-Out E:\OCTO\work\m0_out\wc] [-Limit 55] [-NoEdit]
param([string]$Out = 'E:\OCTO\work\m0_out\wc', [int]$Limit = 55,
      [string]$WordSrc = 'E:\OCTO\work\m0_out\xconv\d2h_word',
      [string]$Share = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M0', [switch]$NoEdit, [switch]$AtEnd)
$ErrorActionPreference = 'Continue'
$wc = 'E:\OCTO\work\wordcraft\target\release\wordcraft-cli.exe'
$docx = Get-ChildItem 'E:\OCTO\DOCS\samples\m0_docx\*.docx' | Where-Object Name -match '^\d\d_'
$d = @{}; foreach ($k in 'wc_src','wc_edit','edit','word_edit','word_src') { $d[$k] = Join-Path $Out $k; New-Item -ItemType Directory -Force $d[$k] | Out-Null }
$log = Join-Path $Out 'wc_roundtrip.log'
"== start $(Get-Date -Format s)" | Out-File $log -Encoding utf8
function L($s) { $s | Out-File $log -Append -Encoding utf8; $s }
. (Join-Path $PSScriptRoot 'dialogs.ps1'); $dlog = Join-Path $Out 'dialogs.log'
$utf8 = New-Object Text.UTF8Encoding $false
$rows = @()
foreach ($f in $docx) {
  $b = $f.BaseName; $r = [ordered]@{ file = $f.Name }
  & $wc convert $f.FullName (Join-Path $d.wc_src "$b.pdf") > $null 2> (Join-Path $Out "$b.conv.err"); $r.wc_pdf = $LASTEXITCODE
  & $wc info $f.FullName > (Join-Path $Out "$b.info.json") 2> $null; $r.wc_open = $LASTEXITCODE
  $ed = Join-Path $d.edit "$b.docx"
  $cmds = New-Object Collections.ArrayList; $null = $cmds.Add($(if ($AtEnd) { 'caret.docEnd' } else { 'caret.docStart' }))
  if (-not $NoEdit) { $null = $cmds.Add(@{ command = 'text.insert'; params = @{ text = 'Octo Docs M0 시험' } }) }
  $js = @{ file = $f.FullName; commands = $cmds;
           save = @($ed, (Join-Path $d.wc_edit "$b.pdf")) } | ConvertTo-Json -Depth 5
  $sj = Join-Path $Out "$b.run.json"; [IO.File]::WriteAllText($sj, $js, $utf8)
  & $wc run --script $sj > $null 2> (Join-Path $Out "$b.run.err"); $r.wc_edit = $LASTEXITCODE
  if (Test-Path $ed) {
    $r.size_src = $f.Length; $r.size_edit = (Get-Item $ed).Length
    $r.parts = (python -I (Join-Path $PSScriptRoot 'zip_diff.py') $f.FullName $ed) | ConvertFrom-Json
    # Word: open the edited docx and export PDF
    Get-Process WINWORD -ErrorAction SilentlyContinue | Stop-Process -Force
    $o = Join-Path $Out '_w.out'; $e = Join-Path $Out '_w.err'
    $p = Start-Process powershell -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File',"`"$(Join-Path $PSScriptRoot 'word_pdf.ps1')`"","`"$ed`"","`"$(Join-Path $d.word_edit "$b.pdf")`"" -RedirectStandardOutput $o -RedirectStandardError $e -PassThru -WindowStyle Hidden
    $end = (Get-Date).AddSeconds($Limit)
    while (-not $p.WaitForExit(2000) -and (Get-Date) -lt $end) { $null = Close-Dialogs 'WINWORD' $dlog }
    if (-not $p.HasExited) { $r.word = 'timeout'; $p.Kill(); Get-Process WINWORD -ErrorAction SilentlyContinue | Stop-Process -Force }
    else { $pg = (Select-String -Path $o -Pattern '^PAGES=(\d+)').Matches.Groups[1].Value; $r.word = if ($pg) { "ok pages=$pg" } else { 'error: ' + ((Get-Content $e) -join ' ') } }
  } else { $r.word = 'no edited file' }
  Copy-Item (Join-Path $WordSrc "$b.pdf") (Join-Path $d.word_src "$b.pdf") -ErrorAction SilentlyContinue
  $rows += [pscustomobject]$r
  L "$b open=$($r.wc_open) pdf=$($r.wc_pdf) edit=$($r.wc_edit) word=$($r.word) size=$($r.size_src)->$($r.size_edit) changed=[$($r.parts.changed -join ',')] removed=$(@($r.parts.removed).Count) added=$(@($r.parts.added).Count)"
}
$rows | ConvertTo-Json -Depth 5 | Out-File (Join-Path $Out 'wc_roundtrip.json') -Encoding utf8
$day = Get-Date -Format yyMMdd; $d1 = ($docx | Select-Object -First 1).BaseName
L "== cmp word src vs word edit"
python -I "$PSScriptRoot\cmp_pdf.py" $d.word_src - $d.word_edit - (Join-Path $Out 'cmp_word_edit.json') "$d1|$Share\${day}_WordCraft_$(if ($NoEdit) { '그냥저장' } else { '수정본' })_Word비교.png" 2>> (Join-Path $Out 'cmp.err')
L "== cmp word vs wordcraft"
python -I "$PSScriptRoot\cmp_pdf.py" $d.word_src - $d.wc_src - (Join-Path $Out 'cmp_word_wc.json') "$d1|$Share\${day}_Word_WordCraft_비교.png" 2>> (Join-Path $Out 'cmp.err')
L "== WC_ROUNDTRIP_DONE $(Get-Date -Format s)"