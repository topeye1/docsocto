# M3-4 form.check 실측: 양식마다 두 번 — (짧음) 입력 칸 2개에 짧은 답 → 늘어난 행 0이어야 함,
# (긺) 둘째 칸에 아주 긴 답 → 그 행이 늘어난 행으로 잡혀야 함. 결과는 <Out>\<이름>.<모드>.check.json
# 사용: tools/m3/form_check.ps1 -Files <양식...> [-Out 폴더]
param([string[]]$Files, [string]$Out = 'E:\OCTO\work\m3_check')
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$x = 'E:\OCTO\DOCS\target\release\octodocs.exe'
$utf8 = New-Object Text.UTF8Encoding $false
New-Item -ItemType Directory -Force $Out | Out-Null
foreach ($f in $Files) {
  $name = [IO.Path]::GetFileNameWithoutExtension($f)
  $j = ((& $x run $f --cmd 'form.slots={}' 2> "$Out\$name.slots.err") -join "`n") | ConvertFrom-Json
  $ids = @($j.result.slots | Where-Object { $_.kind -ne 'choice' -and $_.id -like 't*' } | Select-Object -First 2 | ForEach-Object { $_.id })
  if ($ids.Count -lt 2) { "$name : 칸이 2개 미만 — 건너뜀"; continue }
  $long = ('가나다라마바사아자차카타파하 ' * 12).Trim()
  foreach ($mode in 'short', 'long') {
    $v2 = if ($mode -eq 'long') { $long } else { '확인' }
    $cmds = @(
      @{ command = 'form.fill'; params = @{ answers = @(@{ id = $ids[0]; value = '시험' }, @{ id = $ids[1]; value = $v2 }) } },
      @{ command = 'form.check'; params = @{ out = "$Out\$name.$mode" } }
    )
    $sp = "$Out\$name.$mode.cmds.json"
    [IO.File]::WriteAllText($sp, (ConvertTo-Json @{ commands = $cmds } -Depth 8), $utf8)
    $lines = @(& $x run $f --script $sp 2> "$Out\$name.$mode.run.err")
    $chk = $lines | Where-Object { $_ -like '*"form.check"*' } | Select-Object -First 1
    [IO.File]::WriteAllText("$Out\$name.$mode.check.json", "$chk", $utf8)
    if (-not $chk) { "== $name [$mode] form.check 결과 없음"; Get-Content "$Out\$name.$mode.run.err" -Encoding utf8 | Select-Object -First 3; continue }
    $r = ($chk | ConvertFrom-Json).result
    $g = @($r.grown_rows | ForEach-Object { "표$($_.table) 행$($_.row) $($_.before)→$($_.after) ($($_.page)쪽)" }) -join '; '
    "== $name [$mode] 칸 $($ids -join ',') | 쪽 $($r.pages.before)→$($r.pages.after) | 늘어난 행 $(@($r.grown_rows).Count): $g | 남은 빈칸 $(@($r.unfilled).Count) | 그림 $(@($r.pictures).Count)"
  }
}
