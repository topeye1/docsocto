//! Octo Docs — HWPX·docx·md·txt를 같은 명령 이름으로 고친다(기획서 3절 기능 2·3·4·4a, M2).
//!
//! 파일 형식에 따라 엔진을 고른다: HWPX(·HWP) → ohwp(rhwp), docx → WordCraft, md·txt → 텍스트 엔진.
//! 공통 명령([`COMMON`])은 세 엔진이 같은 이름·같은 매개변수로 받는다. 공통 목록에 없는 명령은
//! 그 형식의 엔진에 그대로 넘긴다(HWPX는 ohwp 명령, docx는 WordCraft 명령).
//! 저장은 원본을 덮어쓰지 않고(5절), docx·HWPX는 보존 저장을 거친다.

pub mod convert;
pub mod docx;
pub mod form;
pub mod hwpx;
pub mod i18n;
pub mod mcp;
pub mod read;
pub mod settings;
pub mod text;

use serde_json::Value;

/// 문서 형식.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Hwpx,
    Hwp,
    Docx,
    Markdown,
    Text,
}

impl Format {
    pub fn from_path(path: &str) -> Option<Format> {
        let ext = path.rsplit('.').next()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "hwpx" => Format::Hwpx,
            "hwp" => Format::Hwp,
            "docx" => Format::Docx,
            "md" | "markdown" => Format::Markdown,
            "txt" | "text" => Format::Text,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Hwpx => "hwpx",
            Format::Hwp => "hwp",
            Format::Docx => "docx",
            Format::Markdown => "md",
            Format::Text => "txt",
        }
    }

    /// 저장할 때의 확장자 — HWP 5.0은 편집 저장을 HWPX로 한다(3절 기능 11).
    pub fn save_ext(self) -> &'static str {
        match self {
            Format::Hwp => "hwpx",
            f => f.name(),
        }
    }
}

/// 엔진 하나가 지켜야 할 약속.
pub trait Engine {
    fn format(&self) -> Format;
    /// 명령 실행. 공통 명령과 그 엔진 고유 명령을 모두 받는다.
    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String>;
    /// 저장할 바이트(보존 저장 포함)와 보고서(JSON).
    fn save(&mut self) -> Result<(Vec<u8>, Value), String>;
    /// 엔진 고유 명령 목록 (id, 한글 이름, 영어 이름)
    fn native_commands(&self) -> Vec<(String, String, String)>;
    /// 열 때 받은 원본 바이트(form.check가 원본 배치와 견준다). 배치가 없는 형식은 None.
    fn original(&self) -> Option<&[u8]> {
        None
    }
}

/// 공통 명령: (id, 한글, 영어, 매개변수). 매개변수 뜻은 세 형식에서 같다.
///
/// 위치 규칙(진행일지 A13):
/// - `para` 는 본문 문단 번호(0부터). HWPX는 구역(section) 안 문단, docx는 본문 블록(표도 한 칸),
///   md·txt는 줄 번호다. docx·md·txt의 section 은 언제나 0.
/// - `offset`·`start`·`end`·`count` 는 글자 수(유니코드 문자) 단위.
/// - `table` 은 본문 최상위 표 번호(0부터), `row`·`col` 은 격자 좌표(합친 셀은 왼쪽 위).
pub const COMMON: &[(&str, &str, &str, &str)] = &[
    ("doc.info", "문서 정보", "Document info", ""),
    ("doc.text", "본문 글자", "Body text", ""),
    ("para.text", "문단 읽기", "Get paragraph text", "section,para"),
    ("para.insert", "문단 넣기", "Insert paragraph", "section,para"),
    ("para.delete", "문단 지우기", "Delete paragraph", "section,para"),
    ("para.append", "문단 덧붙이기", "Append paragraph", "section,para,text"),
    ("text.insert", "글자 넣기", "Insert text", "section,para,offset,text"),
    ("text.delete", "글자 지우기", "Delete text", "section,para,offset,count"),
    ("text.append", "문단 끝에 덧붙이기", "Append text", "section,para,text"),
    ("text.find", "찾기", "Find text", "find"),
    ("text.replace", "바꾸기", "Replace text", "find,replace"),
    ("table.list", "표 목록", "List tables", ""),
    ("table.getCell", "셀 읽기", "Get cell text", "table,row,col"),
    ("table.setCell", "셀 쓰기", "Set cell text", "table,row,col,text"),
    ("table.insertRow", "행 넣기", "Insert row", "table,row,below?"),
    ("table.deleteRow", "행 지우기", "Delete row", "table,row"),
    ("table.insertColumn", "열 넣기", "Insert column", "table,col,right?"),
    ("table.deleteColumn", "열 지우기", "Delete column", "table,col"),
    ("table.merge", "셀 합치기", "Merge cells", "table,startRow,startCol,endRow,endCol"),
    ("format.bold", "굵게", "Bold", "section,para,start,end,on?"),
    ("format.italic", "기울임", "Italic", "section,para,start,end,on?"),
    ("format.underline", "밑줄", "Underline", "section,para,start,end,on?"),
    ("para.align", "정렬", "Align", "section,para,align(left|center|right|justify)"),
    ("form.slots", "채울 칸 뽑기", "List fillable slots", ""),
    ("form.fill", "칸 채우기", "Fill slots", "answers[{id,value|choose}]"),
    ("form.check", "채운 양식 확인", "Check filled form", "out?(쪽 그림 폴더)"),
];

/// 명령 하나 실행: 양식 명령(form.*)은 공통 층에서, 나머지는 엔진에서.
pub fn run(e: &mut dyn Engine, id: &str, p: &Value) -> Result<Value, String> {
    match id {
        "form.slots" => form::slots(e),
        "form.fill" => form::fill(e, p),
        "form.check" => form::check(e, p),
        _ => e.run(id, p),
    }
}

pub fn is_common(id: &str) -> bool {
    COMMON.iter().any(|c| c.0 == id)
}

/// 파일 바이트를 형식에 맞는 엔진으로 연다.
pub fn open(path: &str, data: Vec<u8>) -> Result<Box<dyn Engine>, String> {
    let fmt = Format::from_path(path).ok_or_else(|| i18n::t("unknown_format").replace("{path}", path))?;
    Ok(match fmt {
        Format::Hwpx | Format::Hwp => Box::new(hwpx::HwpxEngine::open(fmt, data)?),
        Format::Docx => Box::new(docx::DocxEngine::open(path, data)?),
        Format::Markdown | Format::Text => Box::new(text::TextEngine::open(fmt, data)?),
    })
}

/// 원본을 덮어쓰지 않는 기본 저장 이름: `<원본이름>_수정.<확장자>` (5절).
pub fn default_save_path(path: &str, fmt: Format) -> String {
    let (stem, _) = path.rsplit_once('.').unwrap_or((path, ""));
    format!("{stem}_수정.{}", fmt.save_ext())
}

/// 파일을 열어 문서 정보·표 목록·본문 글자를 돌려준다(고치지 않음).
pub fn inspect(path: &str) -> Result<Value, String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let mut e = open(path, data)?;
    let empty = serde_json::json!({});
    Ok(serde_json::json!({
        "format": e.format().name(),
        "info": e.run("doc.info", &empty)?,
        "tables": e.run("table.list", &empty)?,
        "text": e.run("doc.text", &empty)?,
    }))
}

/// 새 문서를 만든다. 확장자가 없으면 설정의 기본 형식(결정 #15)을 붙인다. 만든 경로를 돌려준다.
pub fn new_document(path: &str, overwrite: bool) -> Result<String, String> {
    let path = match Format::from_path(path) {
        Some(_) => path.to_string(),
        None => {
            let def = settings::load().default_format.ok_or_else(|| i18n::t("init_choose"))?;
            format!("{path}.{def}")
        }
    };
    if std::path::Path::new(&path).exists() && !overwrite {
        return Err(i18n::t("exists"));
    }
    let bytes = match Format::from_path(&path).unwrap() {
        Format::Hwpx | Format::Hwp => octodocs_ohwp::blank_hwpx()?,
        Format::Docx => wordcraft_engine::io::save_bytes("new.docx", &wordcraft_doc::Document::new())?,
        Format::Markdown | Format::Text => Vec::new(),
    };
    let path = if path.to_ascii_lowercase().ends_with(".hwp") { format!("{path}x") } else { path };
    std::fs::write(&path, bytes).map_err(|e| format!("{path}: {e}"))?;
    Ok(path)
}

/// 매개변수 도우미.
pub(crate) mod p {
    use serde_json::Value;

    pub fn us(p: &Value, k: &str) -> Result<usize, String> {
        p.get(k).and_then(Value::as_u64).map(|v| v as usize).ok_or_else(|| format!("매개변수 '{k}'(0 이상의 정수)가 필요합니다"))
    }
    pub fn us_or(p: &Value, k: &str, d: usize) -> usize {
        p.get(k).and_then(Value::as_u64).map(|v| v as usize).unwrap_or(d)
    }
    pub fn st<'a>(p: &'a Value, k: &str) -> Result<&'a str, String> {
        p.get(k).and_then(Value::as_str).ok_or_else(|| format!("매개변수 '{k}'(문자열)가 필요합니다"))
    }
    pub fn b_or(p: &Value, k: &str, d: bool) -> bool {
        p.get(k).and_then(Value::as_bool).unwrap_or(d)
    }
    /// 글자 단위 위치 → 바이트 위치
    pub fn byte_at(s: &str, chars: usize) -> usize {
        s.char_indices().nth(chars).map(|(i, _)| i).unwrap_or(s.len())
    }
    /// 겹치지 않는 일치 위치(글자 단위)
    pub fn char_matches(hay: &str, needle: &str) -> Vec<usize> {
        let mut out = Vec::new();
        if needle.is_empty() {
            return out;
        }
        let mut from = 0;
        while let Some(i) = hay[from..].find(needle) {
            let b = from + i;
            out.push(hay[..b].chars().count());
            from = b + needle.len();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_from_path() {
        assert_eq!(Format::from_path("a/신청서.HWPX"), Some(Format::Hwpx));
        assert_eq!(Format::from_path("x.docx"), Some(Format::Docx));
        assert_eq!(Format::from_path("readme.md"), Some(Format::Markdown));
        assert_eq!(Format::from_path("x.pdf"), None);
    }

    #[test]
    fn save_path_never_overwrites() {
        assert_eq!(default_save_path("C:/a/신청서.hwpx", Format::Hwpx), "C:/a/신청서_수정.hwpx");
        assert_eq!(default_save_path("old.hwp", Format::Hwp), "old_수정.hwpx");
    }

    #[test]
    fn common_ids_are_unique() {
        let mut ids: Vec<_> = COMMON.iter().map(|c| c.0).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), COMMON.len());
    }
}
