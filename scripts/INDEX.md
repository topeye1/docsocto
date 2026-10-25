# 스크립트 색인

새 스크립트를 짜기 전에 여기부터 확인한다.

## M0 판정 (tools/m0)

- `tools/m0/run_hwp_cmp.ps1` — 한글 판정 한 번에 돌리기: 원본·수정본 HWPX를 한글로 PDF 내보내기 → 한글 PDF와 rhwp PDF 비교, 한글 원본과 수정본 비교. 결과는 `E:\OCTO\work\m0_out\cmp2_*.json`.
- `tools/m0/hwp_export.ps1 -Out <폴더> -Files <hwpx...>` — 한컴오피스(COM)로 열어 쪽 수 기록, PDF 내보내기. 모아 찍기 설정이 있으면 임시 사본에서 끄고 내보낸다.
- `tools/m0/one_up.py <원본> <사본>` — HWPX 사본의 settings.xml에서 모아 찍기(PrintMethod)를 0으로 바꾼다. 본문은 그대로.
- `tools/m0/cmp_pdf.py <폴더A> <접미A> <폴더B> <접미B> <out.json> ["이름|그림.png"]` — 두 PDF 묶음을 쪽마다 비교. 그대로 비교한 값과, 위치 어긋남을 맞춘 뒤 비교한 값, 어긋난 거리(mm)를 낸다.
- `tools/m0/xconv.ps1` — 변환 판정: HWPX→docx(한글 다른 이름 저장) 후 한글 PDF와 Word PDF 비교, docx→HWPX 후 Word PDF와 한글 PDF 비교. 결과는 `E:\OCTO\work\m0_out\xconv\cmp_h2d.json`·`cmp_d2h.json`, 1쪽 나란히 그림은 방 공유 `문서\M0`.

- tools/m0/xconv_word.ps1 — 변환 판정의 Word 단계(docx→PDF)와 PDF 비교. 파일마다 별도 프로세스로 띄우고 제한 시간(-Limit, 기본 55초)을 넘기면 끊고 timeout으로 기록한다. 한 파일 변환은 tools/m0/word_pdf.ps1.
- tools/m0/wc_roundtrip.ps1 — WordCraft docx 왕복 시험: 열기·PDF, 맨 앞에 한 줄 써 넣고 docx·PDF 저장, Word로 수정본 PDF(파일당 제한 시간·확인 창 자동 누름), Word 원본↔수정본·Word↔WordCraft 비교, 패키지 항목 비교.
- tools/m0/zip_diff.py <원본> <수정본> — docx·HWPX 패키지에서 바뀐·빠진·더해진 항목을 JSON으로 낸다.
- tools/m0/dialogs.ps1 — Close-Dialogs <프로세스명> <로그>: Word·한글의 확인 창을 찾아 안전한 단추(확인·허용·읽기 전용 등)를 누르고 로그에 남긴다.

## M1 (tools/m1, crates)

- crates/docsocto-preserve — 보존 저장 도구. docsocto-preserve <원본> <고치지 않은 저장본> <고친 저장본> <출력> [--report r.json]. 바뀐 문단만 끼우고 나머지는 원본 바이트 그대로. 빌드: cargo build -p docsocto-preserve --release (E:\OCTO\DOCS).
- tools/m1/preserve_check.ps1 — 보존 저장 검증: HWPX 10종(rhwp 셀 쓰기)·docx 6종(WordCraft 한 줄 쓰기)에 보존 저장을 적용해 한글·Word PDF를 원본과 비교.
