# M3-5 정책자금 융자신청서 시나리오 완주: 답 목록(loan_answers.json)으로 form.fill → form.check(쪽 그림) → 보존 저장
# → 한글(COM)로 열어 쪽 수·PDF 확인. 사용: tools/m3/loan_scenario.ps1 [-Out E:\OCTO\work\m3_loan]
param([string]$Out = 'E:\OCTO\work\m3_loan', [switch]$NoHwp)
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$root = 'E:\OCTO\DOCS'; $x = "$root\target\release\octodocs.exe"
$utf8 = New-Object Text.UTF8Encoding $false
New-Item -ItemType Directory -Force $Out | Out-Null
$a = Get-Content "$root\tools\m3\loan_answers.json" -Raw -Encoding utf8 | ConvertFrom-Json
$form = Join-Path $root $a.form
$answers = @($a.answers | ForEach-Object { @{ id = $_.id; value = $_.value } })
$cmds = @(
  @{ command = 'form.fill'; params = @{ answers = $answers } },
  @{ command = 'form.check'; params = @{ out = "$Out\check" } }
)
[IO.File]::WriteAllText("$Out\cmds.json", (ConvertTo-Json @{ commands = $cmds } -Depth 8), $utf8)
$saved = "$Out\융자신청서_작성.hwpx"
$t0 = Get-Date
$lines = @(& $x run $form --script "$Out\cmds.json" --save $saved --overwrite 2> "$Out\run.err")
"run: $([int]((Get-Date) - $t0).TotalMilliseconds)ms, 답 $($answers.Count)개"
[IO.File]::WriteAllText("$Out\result.jsonl", ($lines -join "`n"), $utf8)
$fill = ($lines | Where-Object { $_ -like '*"form.fill"*' } | Select-Object -First 1 | ConvertFrom-Json).result
$chk = ($lines | Where-Object { $_ -like '*"form.check"*' } | Select-Object -First 1 | ConvertFrom-Json).result
"fill: 채움 $($fill.filled), 실패 $(@($fill.failed).Count) $(@($fill.failed) | ForEach-Object { "$($_.id):$($_.error)" })"
"check: 쪽 $($chk.pages.before)→$($chk.pages.after), 늘어난 행 $(@($chk.grown_rows).Count), 남은 빈칸 $(@($chk.unfilled).Count), 그림 $(@($chk.pictures).Count)"
$chk.grown_rows | ForEach-Object { "  늘어남: 표$($_.table) 행$($_.row) $($_.before)→$($_.after) ($($_.page)쪽)" }
$chk.page_spill | ForEach-Object { "  쪽 아래 넘침: $($_.page)쪽 $($_.over_px)px" }
$chk.unfilled | ForEach-Object { "  빈칸: $($_.id) $($_.label)" }
Get-Content "$Out\run.err" -Encoding utf8 | Select-Object -First 5
if (Test-Path $saved) { "saved: $((Get-Item $saved).Length) bytes" } else { "saved: 없음"; exit 1 }
if ($NoHwp) { exit 0 }
# 한글로 열어 쪽 수·PDF
& "$root\tools\m0\hwp_export.ps1" -Out "$Out\hwp" -Files $form, $saved
Get-ChildItem "$Out\hwp" | ForEach-Object { "$($_.Name) $($_.Length)" }
