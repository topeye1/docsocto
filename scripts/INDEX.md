# 스크립트 색인

새 스크립트를 짜기 전에 여기부터 확인한다.

## M0 판정 (tools/m0)

- `tools/m0/run_hwp_cmp.ps1` — 한글 판정 한 번에 돌리기: 원본·수정본 HWPX를 한글로 PDF 내보내기 → 한글 PDF와 rhwp PDF 비교, 한글 원본과 수정본 비교. 결과는 `E:\OCTO\work\m0_out\cmp2_*.json`.
- `tools/m0/hwp_export.ps1 -Out <폴더> -Files <hwpx...>` — 한컴오피스(COM)로 열어 쪽 수 기록, PDF 내보내기. 모아 찍기 설정이 있으면 임시 사본에서 끄고 내보낸다.
- `tools/m0/one_up.py <원본> <사본>` — HWPX 사본의 settings.xml에서 모아 찍기(PrintMethod)를 0으로 바꾼다. 본문은 그대로.
- `tools/m0/cmp_pdf.py <폴더A> <접미A> <폴더B> <접미B> <out.json> ["이름|그림.png"]` — 두 PDF 묶음을 쪽마다 비교. 그대로 비교한 값과, 위치 어긋남을 맞춘 뒤 비교한 값, 어긋난 거리(mm)를 낸다.
- `tools/m0/xconv.ps1` — 변환 판정: HWPX→docx(한글 다른 이름 저장) 후 한글 PDF와 Word PDF 비교, docx→HWPX 후 Word PDF와 한글 PDF 비교. 결과는 `E:\OCTO\work\m0_out\xconv\cmp_h2d.json`·`cmp_d2h.json`, 1쪽 나란히 그림은 방 공유 `문서\M0`.

- tools/m0/xconv_word.ps1 — 변환 판정의 Word 단계(docx→PDF)와 PDF 비교. 파일마다 별도 프로세스로 띄우고 제한 시간(-Limit, 기본 55초)을 넘기면 끊고 timeout으로 기록한다. 한 파일 변환은 tools/m0/word_pdf.ps1.
