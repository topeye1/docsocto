# Octo Docs

docx와 HWPX를 한 앱에서 같은 명령으로 다루는 오픈소스 문서 편집기입니다. md·txt 같은 일반 텍스트 파일도 함께 편집합니다.

- 문서는 원래 형식 그대로 고칩니다. 손대지 않은 부분은 원본과 같게 남깁니다.
- Octo Docs에서 저장한 docx는 MS Word에서, HWPX는 한컴오피스에서 깨지지 않는 것을 목표로 합니다. 반대 방향도 마찬가지입니다.
- docx와 HWPX 사이를 서로 바꿔 저장할 수 있습니다. 보이는 모양이 같은 것을 기준으로 합니다.
- AI 도구(Claude Code, Codex CLI 등)가 MCP로 문서를 읽고 고칠 수 있습니다.

"깨지지 않는다"는 Word·한글에서 내보낸 PDF와 Octo Docs에서 내보낸 PDF를 쪽마다 비교해 판정합니다.

## 상태

M0(검증) 단계입니다. 아직 쓸 수 있는 코드는 없습니다. 계획은 [plan/기획서.md](plan/기획서.md)에 있습니다.

## 바탕 기술

- [rhwp](https://github.com/edwardkim/rhwp): HWP·HWPX 파싱과 저장 (MIT)
- [WordCraft](https://github.com/storytold/wordcraft): docx 편집 엔진 (MIT 또는 Apache-2.0)

## 글꼴

표준 글꼴은 Noto Sans KR(SIL OFL 1.1)이며 설치본에 함께 들어갑니다. 문서 안의 글꼴 이름은 바꾸지 않습니다. PC에 없는 무료 글꼴은 내려받아 쓰고, 받을 수 없는 글꼴은 표준 글꼴로 그립니다. 다른 글꼴을 쓰려면 OS에 평소처럼 설치하면 됩니다.

## 라이선스

MIT. [LICENSE](LICENSE)를 보세요.
