# M4 (결정 #33): HTML을 Edge(headless)로 그려 요소별 최종 스타일·자리를 JSON으로 받는다.
# usage: html_dump.ps1 -Html <원본.html> -Out <결과.json> [-WidthPx 794]
# 원본은 건드리지 않는다 — 같은 폴더에 사본을 만들어(상대 경로 그림이 그대로 보이게) 스크립트를 붙인다.
param([Parameter(Mandatory)][string]$Html, [Parameter(Mandatory)][string]$Out, [int]$WidthPx = 794)
$ErrorActionPreference = 'Stop'
$edge = @('C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe', 'C:\Program Files\Microsoft\Edge\Application\msedge.exe') |
  Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw 'Edge(msedge.exe)를 찾지 못했습니다' }
$src = (Resolve-Path $Html).Path
$tmp = Join-Path (Split-Path $src) ('.octodocs_dump_' + [IO.Path]::GetFileName($src))
$js = Get-Content (Join-Path $PSScriptRoot 'html_dump.js') -Raw -Encoding UTF8
$body = [IO.File]::ReadAllText($src, [Text.Encoding]::UTF8)
[IO.File]::WriteAllText($tmp, $body + "`n<script>window.addEventListener('load',function(){$js});</script>", (New-Object Text.UTF8Encoding $false))
try {
  $prof = Join-Path $env:TEMP 'octodocs_edge_profile'
  $url = 'file:///' + ($tmp -replace '\\', '/')
  # msedge 는 창 프로그램이라 PowerShell 로는 표준 출력이 안 잡힌다 — cmd 로 파일에 받는다
  $domFile = "$tmp.dom"
  cmd /c "`"$edge`" --headless=new --disable-gpu --no-first-run --user-data-dir=`"$prof`" --window-size=$WidthPx,1123 --virtual-time-budget=5000 --dump-dom `"$url`" > `"$domFile`" 2> nul"
  # msedge 는 자식 프로세스에 일을 넘기고 먼저 끝난다 — 덤프가 다 써질 때까지(최대 30초) 기다린다
  $dom = ''
  for ($i = 0; $i -lt 60; $i++) {
    try { $dom = [IO.File]::ReadAllText($domFile, [Text.Encoding]::UTF8); if ($dom -match '</html>') { break } } catch { }
    Start-Sleep -Milliseconds 500
  }
  Remove-Item $domFile -ErrorAction SilentlyContinue
  # 스크립트 주석에도 같은 꼬리표가 있으니 마지막 것을 쓴다
  $m = [regex]::Matches($dom, '<pre id="octodocs-dump">([^<]*)</pre>') | Select-Object -Last 1
  if (-not $m) { $m = [regex]::Match('', 'x') }
  if (-not $m.Success) { throw 'Edge 결과에 덤프가 없습니다(스크립트가 돌지 않음)' }
  $json = [Net.WebUtility]::HtmlDecode($m.Groups[1].Value)
  [IO.File]::WriteAllText($Out, $json, (New-Object Text.UTF8Encoding $false))
  "dump: $Out ($((Get-Item $Out).Length) bytes)"
} finally {
  Remove-Item $tmp -ErrorAction SilentlyContinue
}
