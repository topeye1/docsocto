# M4 HTML 변환 판정(결정 #33): Edge 덤프 → docx·HWPX → Word·한글 PDF → Edge 원본 PDF와 비교.
#   표지(자리 고정, dump2docx/dump2hwpx)와 본문(흐름 방식, flow.py)을 한 번에 돌린다.
# usage: m4_check.ps1 [-Build] [-Only cover|body] [-Share] [-Rust]
#   -Rust: Python 시험 도구 대신 제품 명령(octodocs convert)으로 원본 HTML 에서 바로 변환
#   작업 폴더: E:\OCTO\work\m4(표지), E:\OCTO\work\m4f(본문). 각 폴더에 blank.docx·blank.hwpx·<이름>.dump.json·edge\<이름>.pdf 가 있어야 한다.
#   -Share: 결과 docx·HWPX·비교 그림을 방 공유 문서\M4 에 261026_ 이름으로 복사
param([switch]$Build, [string]$Only = '', [switch]$Share, [switch]$Rust)
$ErrorActionPreference = 'Continue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8
Set-Location E:\OCTO\DOCS
if ($Build) {
  # ~\.cargo\bin\cargo.exe(rustup 대리)가 실행되지 않는 일이 있어 툴체인 cargo 를 직접 부른다
  $tc = 'C:\Users\RGC\.rustup\toolchains\stable-x86_64-pc-windows-msvc\bin'; $env:PATH = "$tc;$env:PATH"
  & "$tc\cargo.exe" build --release -p octodocs 2> E:\OCTO\work\m4_check.build.err
  $code = $LASTEXITCODE
  "build exit=$code"
  if ($code -ne 0) { Get-Content E:\OCTO\work\m4_check.build.err -Tail 30; exit 1 }
}
$x = 'E:\OCTO\DOCS\target\release\octodocs.exe'
$shareDir = 'E:\OCTO\DOCS\Docs\Docs-share\문서\M4'
$t = @(
  @{ key = 'cover'; w = 'E:\OCTO\work\m4'; name = 'yg_cover'; html = "$shareDir\261026_표지_원본.html"; docx = 'tools\m4\dump2docx.py'; hwpx = 'tools\m4\dump2hwpx.py'; title = '표지' },
  @{ key = 'body'; w = 'E:\OCTO\work\m4f'; name = 'yg_body'; html = "$shareDir\261026_본문2쪽_원본.html"; docx = 'tools\m4\flow.py'; hwpx = 'tools\m4\flow.py'; title = '본문2쪽' }
) | Where-Object { -not $Only -or $_.key -eq $Only }
. E:\OCTO\DOCS\tools\m0\dialogs.ps1
$fail = 0
foreach ($c in $t) {
  $w = $c.w; $n = $c.name
  New-Item -ItemType Directory -Force "$w\word", "$w\hwp", "$w\hwpcmp" | Out-Null
  if ($Rust) {
    Remove-Item "$w\$n.conv.docx", "$w\$n.conv.hwpx" -ErrorAction SilentlyContinue
    & $x convert $c.html "$w\$n.conv.docx" --report "$w\$n.docx.report.json" > "$w\$n.d.out" 2> "$w\$n.d.err"
    "$($c.key) convert docx exit=$LASTEXITCODE"
    & $x convert $c.html "$w\$n.conv.hwpx" --report "$w\$n.hwpx.report.json" > "$w\$n.h.out" 2> "$w\$n.h.err"
    "$($c.key) convert hwpx exit=$LASTEXITCODE"
    Get-Content "$w\$n.d.err", "$w\$n.h.err" -Encoding utf8 -TotalCount 5
    Get-Content "$w\$n.hwpx.report.json" -Encoding utf8 -ErrorAction SilentlyContinue
  } elseif ($c.key -eq 'body') {
    python -I $c.docx "$w\$n.dump.json" docx "$w\$n.docx.json"
    python -I $c.hwpx "$w\$n.dump.json" hwpx "$w\$n.hwpx.json"
  } else {
    python -I $c.docx "$w\$n.dump.json" "$w\$n.docx.json"
    python -I $c.hwpx "$w\$n.dump.json" "$w\$n.hwpx.json"
  }
  if (-not $Rust) {
  Remove-Item "$w\$n.conv.docx", "$w\$n.conv.hwpx" -ErrorAction SilentlyContinue
  & $x run "$w\blank.docx" --script "$w\$n.docx.json" --save "$w\$n.conv.docx" > "$w\$n.d.out" 2> "$w\$n.d.err"
  "$($c.key) docx exit=$LASTEXITCODE"
  & $x run "$w\blank.hwpx" --script "$w\$n.hwpx.json" --save "$w\$n.conv.hwpx" > "$w\$n.h.out" 2> "$w\$n.h.err"
  "$($c.key) hwpx exit=$LASTEXITCODE"
  }
  if (-not (Test-Path "$w\$n.conv.docx") -or -not (Test-Path "$w\$n.conv.hwpx")) {
    "FAIL $($c.key): 변환 파일이 없습니다"; Get-Content "$w\$n.d.err", "$w\$n.h.err" -Encoding utf8 -TotalCount 10; $fail++; continue
  }
  $guard = Start-Job -ScriptBlock { . E:\OCTO\DOCS\tools\m0\dialogs.ps1; while ($true) { Close-Dialogs @('WINWORD') 'E:\OCTO\work\m4_check.dialogs.log' | Out-Null; Start-Sleep 2 } }
  Remove-Item "$w\word\$n.pdf", "$w\hwpcmp\$n.pdf" -ErrorAction SilentlyContinue
  powershell -NoProfile -File E:\OCTO\DOCS\tools\m0\word_pdf.ps1 -Src "$w\$n.conv.docx" -Pdf "$w\word\$n.pdf"
  Stop-Job $guard; Remove-Job $guard -Force
  & E:\OCTO\DOCS\tools\m0\hwp_export.ps1 -Out "$w\hwp" -Files "$w\$n.conv.hwpx"
  if (Test-Path "$w\hwp\$n.conv.pdf") { Copy-Item "$w\hwp\$n.conv.pdf" "$w\hwpcmp\$n.pdf" -Force }
  foreach ($k in 'word', 'hwpcmp') {
    if (-not (Test-Path "$w\$k\$n.pdf")) { "FAIL $($c.key) ${k}: PDF 없음"; $fail++; continue }
    python -I E:\OCTO\DOCS\tools\m0\cmp_pdf.py "$w\edge" - "$w\$k" - "$w\cmp_$k.json" "$n|$w\cmp_$k.png" 2> $null
    $r = Get-Content "$w\cmp_$k.json" -Raw -Encoding utf8 | ConvertFrom-Json
    "RESULT $($c.key) $k pages=$($r[0].pages_a)/$($r[0].pages_b) raw=$($r[0].raw_mean) aligned=$($r[0].aligned_mean) shift_mm=($($r[0].shift_dx_mm),$($r[0].shift_dy_mm))"
  }
  if ($Share) {
    Copy-Item "$w\$n.conv.docx" "$shareDir\261026_$($c.title)_변환.docx" -Force
    Copy-Item "$w\$n.conv.hwpx" "$shareDir\261026_$($c.title)_변환.hwpx" -Force
  }
}
if ($fail) { "FAILED $fail"; exit 1 }
"OK"
