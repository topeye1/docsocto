# M3-5 시나리오 자료(가상 회사) 만들기 — octodocs new + para.append 로 docx 두 벌, 사업계획은 PDF로도 낸다.
# 실제 회사·개인 정보는 쓰지 않는다(진행일지 A21). 사용: tools/m3/make_scenario.ps1 [-Out samples\m3]
param([string]$Out = 'E:\OCTO\DOCS\samples\m3')
[Console]::OutputEncoding = [Text.Encoding]::UTF8
$x = 'E:\OCTO\DOCS\target\release\octodocs.exe'
$utf8 = New-Object Text.UTF8Encoding $false
function Make-Doc($path, [string[]]$lines) {
  if (Test-Path $path) { Remove-Item $path }
  & $x new $path --overwrite 2> $null | Out-Null
  $cmds = @(@{ command = 'text.append'; params = @{ section = 0; para = 0; text = $lines[0] } })
  for ($i = 1; $i -lt $lines.Count; $i++) { $cmds += @{ command = 'para.append'; params = @{ section = 0; para = $i - 1; text = $lines[$i] } } }
  $sp = "$path.cmds.json"
  [IO.File]::WriteAllText($sp, (ConvertTo-Json @{ commands = $cmds } -Depth 6), $utf8)
  $tmp = "$path.tmp.docx"
  & $x run $path --script $sp --save $tmp --overwrite 2> "$path.err" | Out-Null
  Move-Item -Force $tmp $path; Remove-Item $sp
}
Make-Doc "$Out\가온정밀_회사소개.docx" @(
  '가온정밀 주식회사 회사소개서 (가상 시험 자료)',
  '1. 기본 정보',
  '회사명: 가온정밀 주식회사 / 대표이사: 김가온',
  '사업자등록번호: 000-00-00000 / 법인등록번호: 000000-0000000',
  '본사·공장: 서울특별시 서초구 시험로 123, 가온빌딩 2층',
  '대표 전화: 02-000-0000 / 이메일: info@example.com',
  '업종: 제조업(정밀 금속가공) / 주요 품목: 반도체 장비용 정밀 부품',
  '상시 종업원: 23명(남 17명, 여 6명)',
  '2. 연혁',
  '2016. 3. 2. 가온정밀 설립(개인사업자)',
  '2019. 7. 1. 법인 전환(가온정밀 주식회사)',
  '2023. 5. 15. 기업부설연구소 인정(한국산업기술진흥협회)',
  '3. 대표자',
  '김가온 — 전공: 기계공학(정밀가공 20년)',
  '경력: 한빛기계(주) 정밀부품 생산 담당(과장) 2004~2010, 누리테크(주) 진공부품 생산기술(부장) 2010~2016',
  '포상: 서울특별시장 표창(우수 중소기업, 2022. 11. 20.), 중소벤처기업부 장관 표창(기술혁신, 2024. 6. 12.)',
  '4. 제품 개요',
  '반도체 식각·증착 장비에 들어가는 진공 챔버 부품과 정밀 가스 분배판을 5축 가공으로 만든다. 표면 거칠기 Ra 0.2㎛ 이하.',
  '5. 기술 인력',
  '연구소장 박누리 — 서울과학기술대 기계공학과 — 기계설계산업기사 — 한국산업인력공단',
  '생산팀장 이하늘 — 한국폴리텍대 기계과 — 컴퓨터응용가공산업기사 — 한국산업인력공단',
  '품질과장 최바다 — 인하공전 금형설계과 — 품질경영산업기사 — 한국산업인력공단',
  '6. 지식재산',
  '특허 — 진공 챔버용 가스 분배판 가공 방법 — 제10-0000001호 — 2021. 9. 3. — 김가온 — 가온정밀 주식회사',
  '실용신안 — 정밀 부품 세척 지그 — 제20-0000002호 — 2023. 2. 17. — 박누리 — 가온정밀 주식회사'
)
Make-Doc "$Out\가온정밀_사업계획.docx" @(
  '가온정밀 주식회사 2026년 시설·운전자금 사업계획 (가상 시험 자료)',
  '1. 목적',
  '반도체 장비 고객사 증설에 맞춰 5축 가공기 1대를 들이고 원자재를 미리 확보한다.',
  '2. 자금 소요 계획',
  '시설자금: 5축 가공기 1대 — 소요액 300,000,000원 — 산출근거: 장비 견적 1대 × 300,000,000원',
  '운전자금(원자재): 소요액 150,000,000원 — 산출근거: 알루미늄 6061 판재 월 25,000,000원 × 6개월',
  '운전자금(인건비): 소요액 50,000,000원 — 산출근거: 신규 가공 기능직 2명 × 월 4,166,667원 × 6개월',
  '총 소요액: 500,000,000원',
  '3. 융자 신청',
  '융자신청액: 300,000,000원(시설 200,000,000원, 운전 100,000,000원), 총 소요액의 60%. 나머지 200,000,000원은 자기자금.',
  '4. 신용 보증',
  '서울신용보증재단 신용평가액 200백만원(2026. 8. 평가).'
)
& $x render "$Out\가온정밀_사업계획.docx" --page 1 --out "$Out\가온정밀_사업계획.pdf" 2> "$Out\render.err" | Out-Null
Remove-Item "$Out\가온정밀_사업계획.docx"
Get-ChildItem $Out | % { "$($_.Name) $($_.Length)" }
Get-ChildItem $Out -Filter *.err | Remove-Item

