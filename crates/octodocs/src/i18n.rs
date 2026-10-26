//! 화면·도움말·오류 문구: 한글 기본, 영어 함께(결정 #21·#22).

pub fn lang() -> String {
    std::env::var("OCTODOCS_LANG").ok().filter(|l| l == "ko" || l == "en").unwrap_or_else(crate::settings::language)
}

/// 문구 하나. 모르는 키는 키 그대로 돌려준다.
pub fn t(key: &str) -> String {
    let ko = lang() == "ko";
    let s = match (key, ko) {
        ("usage", true) => USAGE_KO,
        ("usage", false) => USAGE_EN,
        ("unknown_format", true) => "형식을 알 수 없는 파일입니다: {path} (hwpx·hwp·docx·md·txt만 엽니다)",
        ("unknown_format", false) => "Unknown file type: {path} (Octo Docs opens hwpx, hwp, docx, md, txt)",
        ("bad_default", true) => "기본 형식은 hwpx 또는 docx 중 하나입니다",
        ("bad_default", false) => "The default format must be hwpx or docx",
        ("init_choose", true) => "처음 실행입니다. 새 문서의 기본 형식을 고르십시오: octodocs init --default hwpx  또는  octodocs init --default docx",
        ("init_choose", false) => "First run. Choose the default format for new documents: octodocs init --default hwpx  or  octodocs init --default docx",
        ("init_done", true) => "설정을 저장했습니다",
        ("init_done", false) => "Settings saved",
        ("saved", true) => "저장했습니다",
        ("saved", false) => "Saved",
        ("whole_engine", true) => "원본과 맞대지 못한 부분이 있어 엔진 결과 전체로 저장했습니다",
        ("whole_engine", false) => "Some parts could not be matched to the original; saved the engine output as a whole",
        ("exists", true) => "이미 있는 파일입니다 — 덮어쓰려면 --overwrite 를 주십시오",
        ("exists", false) => "The file already exists — pass --overwrite to replace it",
        _ => return key.to_string(),
    };
    s.to_string()
}

const USAGE_KO: &str = "Octo Docs — HWPX·docx·md·txt 문서 편집기

사용법:
  octodocs init --default <hwpx|docx> [--lang <ko|en>]   처음 설정(기본 형식·언어)
  octodocs commands [<파일>] [--json]                   공통 명령 목록(파일을 주면 그 형식 고유 명령도)
  octodocs inspect <파일>                               문서 정보·표 목록·본문 글자(JSON)
  octodocs read <자료>                                  참고 자료 글자(hwpx·docx·md·txt·pptx·xlsx·pdf, 위치 포함)
  octodocs run <파일> [--script <명령.json>] [--cmd 'id={json}' …] [--save <출력>] [--overwrite]
                                                       명령을 차례로 실행하고, --save 면 보존 저장
                                                       (출력을 안 주면 <원본>_수정.<확장자>)
  octodocs new <파일>                                   새 문서(확장자 없으면 기본 형식)
  octodocs render <파일> --page <N> --out <그림.svg|pdf> HWPX 쪽 그림
  octodocs convert <원본.html> <결과.docx|hwpx> [--mode auto|flow|fixed] [--report <보고서.json>] [--overwrite]
                                                       요청형 변환(Edge 가 그린 결과로) — 변환 보고서를 냄
  octodocs mcp                                          AI 연결(MCP, 표준 입출력)

원본은 덮어쓰지 않습니다. 덮어쓰려면 --save 에 원본 경로와 --overwrite 를 함께 주십시오.";

const USAGE_EN: &str = "Octo Docs — editor for HWPX, docx, md and txt

Usage:
  octodocs init --default <hwpx|docx> [--lang <ko|en>]   first-run settings
  octodocs commands [<file>] [--json]                   common commands (plus the format's own with a file)
  octodocs inspect <file>                               info, tables and body text as JSON
  octodocs read <source>                                text of a reference file with locations (… pptx, xlsx, pdf)
  octodocs run <file> [--script <cmds.json>] [--cmd 'id={json}' …] [--save <out>] [--overwrite]
                                                       run commands; --save writes a preserving save
                                                       (default output <name>_수정.<ext>)
  octodocs new <file>                                   new document (no extension: default format)
  octodocs render <file> --page <N> --out <page.svg|pdf> render an HWPX page
  octodocs convert <src.html> <out.docx|hwpx> [--mode auto|flow|fixed] [--report <report.json>] [--overwrite]
                                                       convert on request (from what Edge draws), with a report
  octodocs mcp                                          AI connection (MCP over stdio)

The original is never overwritten unless you pass its path to --save together with --overwrite.";
