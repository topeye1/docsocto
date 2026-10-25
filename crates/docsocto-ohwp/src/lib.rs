//! ohwp — rhwp 위에 얹는 DocsOcto의 HWPX 엔진 층.
//!
//! 모든 편집은 "명령 id + JSON 매개변수"로 부른다(기획서 3절 기능 2). 명령 목록은
//! [`commands`]가 돌려주고, [`Session::run`]이 실행한다. 저장은 [`Session::save`]가
//! 보존 저장(docsocto-preserve)을 거쳐 손대지 않은 부분을 원본 바이트 그대로 둔다.

use rhwp::document_core::queries::table_extract::extract_tables;
use rhwp::model::control::Control;
use rhwp::wasm_api::HwpDocument;
use serde_json::{json, Value};

pub use rhwp;

/// 문서를 열어 아무것도 고치지 않고 HWPX로 다시 저장한다(보존 저장의 B0).
pub fn resave_hwpx(data: &[u8]) -> Result<Vec<u8>, String> {
    let doc = open(data)?;
    doc.export_hwpx_native().map_err(|e| format!("저장 실패: {e:?}"))
}

fn open(data: &[u8]) -> Result<HwpDocument, String> {
    HwpDocument::from_bytes(data).map_err(|e| format!("열기 실패: {e:?}"))
}

/// 명령 하나의 설명.
pub struct CommandInfo {
    pub id: &'static str,
    pub ko: &'static str,
    pub en: &'static str,
    /// 매개변수 이름과 설명(한 줄)
    pub params: &'static str,
}

const COMMANDS: &[CommandInfo] = &[
    CommandInfo { id: "doc.info", ko: "문서 정보", en: "Document info", params: "" },
    CommandInfo { id: "table.list", ko: "표 목록", en: "List tables", params: "" },
    CommandInfo { id: "table.getCell", ko: "셀 읽기", en: "Get cell text", params: "table,row,col" },
    CommandInfo { id: "table.setCell", ko: "셀 쓰기", en: "Set cell text", params: "table,row,col,text" },
    CommandInfo { id: "table.insertRow", ko: "행 넣기", en: "Insert row", params: "table,row,below?" },
    CommandInfo { id: "table.insertColumn", ko: "열 넣기", en: "Insert column", params: "table,col,right?" },
    CommandInfo { id: "table.deleteRow", ko: "행 지우기", en: "Delete row", params: "table,row" },
    CommandInfo { id: "table.deleteColumn", ko: "열 지우기", en: "Delete column", params: "table,col" },
    CommandInfo { id: "table.merge", ko: "셀 합치기", en: "Merge cells", params: "table,startRow,startCol,endRow,endCol" },
    CommandInfo { id: "table.split", ko: "셀 나누기", en: "Split cell", params: "table,row,col" },
    CommandInfo { id: "table.columnWidths", ko: "열 너비", en: "Column widths", params: "table,widths[] (HWPUNIT)" },
    CommandInfo { id: "table.delete", ko: "표 지우기", en: "Delete table", params: "table" },
    CommandInfo { id: "table.cellCharFormat", ko: "셀 글자 모양", en: "Cell character format", params: "table,row,col,para?,start?,end?,props{}" },
    CommandInfo { id: "table.cellParaFormat", ko: "셀 문단 모양", en: "Cell paragraph format", params: "table,row,col,para?,props{}" },
    CommandInfo { id: "para.text", ko: "문단 읽기", en: "Get paragraph text", params: "section,para" },
    CommandInfo { id: "para.insert", ko: "문단 넣기", en: "Insert paragraph", params: "section,para" },
    CommandInfo { id: "para.delete", ko: "문단 지우기", en: "Delete paragraph", params: "section,para" },
    CommandInfo { id: "para.merge", ko: "문단 합치기", en: "Merge with next paragraph", params: "section,para" },
    CommandInfo { id: "para.split", ko: "문단 나누기", en: "Split paragraph", params: "section,para,offset" },
    CommandInfo { id: "text.insert", ko: "글자 넣기", en: "Insert text", params: "section,para,offset,text" },
    CommandInfo { id: "text.delete", ko: "글자 지우기", en: "Delete text", params: "section,para,offset,count" },
    CommandInfo { id: "text.replace", ko: "바꾸기", en: "Replace text", params: "find,replace" },
    CommandInfo { id: "format.char", ko: "글자 모양", en: "Character format", params: "section,para,start,end,props{bold,italic,underline,fontSize,textColor,…}" },
    CommandInfo { id: "format.para", ko: "문단 모양", en: "Paragraph format", params: "section,para,props{alignment,lineSpacing,…}" },
    CommandInfo { id: "page.break", ko: "쪽 나눔", en: "Page break", params: "section,para,offset" },
];

pub fn commands() -> &'static [CommandInfo] {
    COMMANDS
}

/// 열린 문서 하나. 원본 바이트를 들고 있다가 저장 때 보존 저장에 쓴다.
pub struct Session {
    original: Vec<u8>,
    doc: HwpDocument,
}

fn us(p: &Value, k: &str) -> Result<usize, String> {
    p.get(k).and_then(Value::as_u64).map(|v| v as usize).ok_or_else(|| format!("매개변수 '{k}'(0 이상의 정수)가 필요합니다"))
}
fn us_or(p: &Value, k: &str, d: usize) -> usize {
    p.get(k).and_then(Value::as_u64).map(|v| v as usize).unwrap_or(d)
}
fn st<'a>(p: &'a Value, k: &str) -> Result<&'a str, String> {
    p.get(k).and_then(Value::as_str).ok_or_else(|| format!("매개변수 '{k}'(문자열)가 필요합니다"))
}
fn props(p: &Value) -> String {
    p.get("props").cloned().unwrap_or_else(|| json!({})).to_string()
}
fn engine<T: std::fmt::Debug>(e: T) -> String {
    format!("엔진 오류: {e:?}")
}
fn parsed(s: String) -> Value {
    serde_json::from_str(&s).unwrap_or(Value::String(s))
}

/// 본문 최상위 표 하나의 자리.
#[derive(Clone, Copy)]
struct TableAt {
    section: usize,
    para: usize,
    control: usize,
}

impl Session {
    pub fn open(data: Vec<u8>) -> Result<Self, String> {
        let doc = open(&data)?;
        Ok(Session { original: data, doc })
    }

    fn table(&self, no: usize) -> Result<TableAt, String> {
        let grids = extract_tables(self.doc.document());
        let top: Vec<_> = grids.iter().filter(|g| g.container_path.is_empty()).collect();
        top.iter()
            .find(|g| g.index == no)
            .map(|g| TableAt { section: g.section, para: g.paragraph, control: g.control })
            .ok_or_else(|| format!("본문 최상위 표 {no}번이 없습니다(최상위 표 {}개)", top.len()))
    }

    fn table_model(&self, at: TableAt) -> Result<&rhwp::model::table::Table, String> {
        match self.doc.document().sections.get(at.section).and_then(|s| s.paragraphs.get(at.para)).and_then(|p| p.controls.get(at.control)) {
            Some(Control::Table(t)) => Ok(t),
            _ => Err("표 자리를 찾지 못했습니다(내부 불일치)".into()),
        }
    }

    /// (표 자리, 셀 번호, 셀 문단별 글자 수)
    fn cell(&self, p: &Value) -> Result<(TableAt, usize, Vec<usize>), String> {
        let at = self.table(us(p, "table")?)?;
        let (row, col) = (us(p, "row")? as u16, us(p, "col")? as u16);
        let t = self.table_model(at)?;
        if row >= t.row_count || col >= t.col_count {
            return Err(format!("좌표가 표 밖입니다 — 이 표는 {}행 {}열입니다", t.row_count, t.col_count));
        }
        match t.cells.iter().enumerate().find(|(_, c)| c.row == row && c.col == col) {
            Some((i, c)) => Ok((at, i, c.paragraphs.iter().map(|p| p.text.chars().count()).collect())),
            None => Err(format!("({row},{col})은 합쳐진 셀 안쪽입니다 — 합쳐진 셀의 왼쪽 위 좌표를 쓰십시오")),
        }
    }

    pub fn run(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        Ok(match id {
            "doc.info" => {
                let doc = self.doc.document();
                let tables = extract_tables(doc).iter().filter(|g| g.container_path.is_empty()).count();
                json!({
                    "sections": doc.sections.len(),
                    "paragraphs": doc.sections.iter().map(|s| s.paragraphs.len()).collect::<Vec<_>>(),
                    "tables": tables,
                })
            }
            "table.list" => {
                let grids = extract_tables(self.doc.document());
                let mut out = Vec::new();
                for g in grids.iter().filter(|g| g.container_path.is_empty()) {
                    let at = TableAt { section: g.section, para: g.paragraph, control: g.control };
                    if let Ok(t) = self.table_model(at) {
                        out.push(json!({"table": g.index, "section": g.section, "para": g.paragraph, "rows": t.row_count, "cols": t.col_count}));
                    }
                }
                Value::Array(out)
            }
            "table.getCell" => {
                let (at, ci, _) = self.cell(p)?;
                let t = self.table_model(at)?;
                let text = t.cells[ci].paragraphs.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join("\n");
                json!({ "text": text })
            }
            "table.setCell" => {
                // 진행일지 A6·A8: 칸의 원래 서식을 쓴다. 모든 문단 글자를 지운 뒤 첫 문단 0자리에
                // 넣으면 그 자리의 첫 글자 모양을 그대로 이어받는다.
                let (at, ci, lens) = self.cell(p)?;
                let text = st(p, "text")?.to_string();
                let d = &mut self.doc;
                for (pi, len) in lens.iter().enumerate() {
                    if *len > 0 {
                        d.delete_text_in_cell_native(at.section, at.para, at.control, ci, pi, 0, *len).map_err(engine)?;
                    }
                }
                if !text.is_empty() {
                    d.insert_text_in_cell_native(at.section, at.para, at.control, ci, 0, 0, &text).map_err(engine)?;
                }
                json!({ "ok": true })
            }
            "table.insertRow" => {
                let at = self.table(us(p, "table")?)?;
                let below = p.get("below").and_then(Value::as_bool).unwrap_or(true);
                parsed(self.doc.insert_table_row_native(at.section, at.para, at.control, us(p, "row")? as u16, below).map_err(engine)?)
            }
            "table.insertColumn" => {
                let at = self.table(us(p, "table")?)?;
                let right = p.get("right").and_then(Value::as_bool).unwrap_or(true);
                parsed(self.doc.insert_table_column_native(at.section, at.para, at.control, us(p, "col")? as u16, right).map_err(engine)?)
            }
            "table.deleteRow" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.delete_table_row_native(at.section, at.para, at.control, us(p, "row")? as u16).map_err(engine)?)
            }
            "table.deleteColumn" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.delete_table_column_native(at.section, at.para, at.control, us(p, "col")? as u16).map_err(engine)?)
            }
            "table.merge" => {
                let at = self.table(us(p, "table")?)?;
                parsed(
                    self.doc
                        .merge_table_cells_native(
                            at.section,
                            at.para,
                            at.control,
                            us(p, "startRow")? as u16,
                            us(p, "startCol")? as u16,
                            us(p, "endRow")? as u16,
                            us(p, "endCol")? as u16,
                        )
                        .map_err(engine)?,
                )
            }
            "table.split" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.split_table_cell_native(at.section, at.para, at.control, us(p, "row")? as u16, us(p, "col")? as u16).map_err(engine)?)
            }
            "table.columnWidths" => {
                let at = self.table(us(p, "table")?)?;
                let widths: Vec<u32> = p
                    .get("widths")
                    .and_then(Value::as_array)
                    .ok_or("매개변수 'widths'(정수 배열)가 필요합니다")?
                    .iter()
                    .map(|v| v.as_u64().map(|x| x as u32).ok_or("widths 값은 정수여야 합니다"))
                    .collect::<Result<_, _>>()?;
                parsed(self.doc.set_table_column_widths_native(at.section, at.para, at.control, widths).map_err(engine)?)
            }
            "table.delete" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.delete_table_control_native(at.section, at.para, at.control).map_err(engine)?)
            }
            "table.cellCharFormat" => {
                let (at, ci, lens) = self.cell(p)?;
                let pi = us_or(p, "para", 0);
                let len = *lens.get(pi).ok_or("셀에 그 문단이 없습니다")?;
                let (s, e) = (us_or(p, "start", 0), us_or(p, "end", len));
                parsed(self.doc.apply_char_format_in_cell_native(at.section, at.para, at.control, ci, pi, s, e, &props(p)).map_err(engine)?)
            }
            "table.cellParaFormat" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(self.doc.apply_para_format_in_cell_native(at.section, at.para, at.control, ci, us_or(p, "para", 0), &props(p)).map_err(engine)?)
            }
            "para.text" => {
                let doc = self.doc.document();
                let para = doc.sections.get(us(p, "section")?).and_then(|s| s.paragraphs.get(us(p, "para").unwrap_or(usize::MAX))).ok_or("그 문단이 없습니다")?;
                json!({ "text": para.text })
            }
            "para.insert" => parsed(self.doc.insert_paragraph_native(us(p, "section")?, us(p, "para")?).map_err(engine)?),
            "para.delete" => parsed(self.doc.delete_paragraph_native(us(p, "section")?, us(p, "para")?).map_err(engine)?),
            "para.merge" => parsed(self.doc.merge_paragraph_native(us(p, "section")?, us(p, "para")?).map_err(engine)?),
            "para.split" => parsed(self.doc.split_paragraph_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?, None).map_err(engine)?),
            "text.insert" => parsed(self.doc.insert_text_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?, st(p, "text")?).map_err(engine)?),
            "text.delete" => parsed(self.doc.delete_text_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?, us(p, "count")?).map_err(engine)?),
            "text.replace" => {
                let (find, rep) = (st(p, "find")?.to_string(), st(p, "replace")?.to_string());
                if find.is_empty() {
                    return Err("find는 비어 있으면 안 됩니다".into());
                }
                self.replace_all(&find, &rep)?
            }
            "format.char" => parsed(self.doc.apply_char_format_native(us(p, "section")?, us(p, "para")?, us(p, "start")?, us(p, "end")?, &props(p)).map_err(engine)?),
            "format.para" => parsed(self.doc.apply_para_format_native(us(p, "section")?, us(p, "para")?, &props(p)).map_err(engine)?),
            "page.break" => parsed(self.doc.insert_page_break_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?).map_err(engine)?),
            other => return Err(format!("모르는 명령입니다: {other} (ohwp commands 로 목록 확인)")),
        })
    }

    /// 본문 문단과 최상위 표 셀 문단에서 모두 바꾼다. 뒤에서부터 바꿔 앞쪽 위치가 밀리지 않게 한다.
    fn replace_all(&mut self, find: &str, rep: &str) -> Result<Value, String> {
        let flen = find.chars().count();
        let mut body: Vec<(usize, usize, usize)> = Vec::new(); // (section, para, offset)
        let mut cells: Vec<(TableAt, usize, usize, usize)> = Vec::new(); // (table, cell, para, offset)
        let doc = self.doc.document();
        for (si, s) in doc.sections.iter().enumerate() {
            for (pi, para) in s.paragraphs.iter().enumerate() {
                for off in char_matches(&para.text, find) {
                    body.push((si, pi, off));
                }
                for (ci, c) in para.controls.iter().enumerate() {
                    if let Control::Table(t) = c {
                        for (celli, cell) in t.cells.iter().enumerate() {
                            for (cpi, cp) in cell.paragraphs.iter().enumerate() {
                                for off in char_matches(&cp.text, find) {
                                    cells.push((TableAt { section: si, para: pi, control: ci }, celli, cpi, off));
                                }
                            }
                        }
                    }
                }
            }
        }
        let n = body.len() + cells.len();
        for &(si, pi, off) in body.iter().rev() {
            self.doc.delete_text_native(si, pi, off, flen).map_err(engine)?;
            if !rep.is_empty() {
                self.doc.insert_text_native(si, pi, off, rep).map_err(engine)?;
            }
        }
        for &(at, ci, cpi, off) in cells.iter().rev() {
            self.doc.delete_text_in_cell_native(at.section, at.para, at.control, ci, cpi, off, flen).map_err(engine)?;
            if !rep.is_empty() {
                self.doc.insert_text_in_cell_native(at.section, at.para, at.control, ci, cpi, off, rep).map_err(engine)?;
            }
        }
        Ok(json!({ "replaced": n }))
    }

    /// 엔진이 쓴 HWPX를 그대로 돌려준다.
    pub fn engine_bytes(&self) -> Result<Vec<u8>, String> {
        self.doc.export_hwpx_native().map_err(engine)
    }

    /// 보존 저장: 원본·무변경 저장본·편집 저장본을 맞대어 바뀐 부분만 원본에 끼운다.
    pub fn save(&self) -> Result<(Vec<u8>, docsocto_preserve::Report), String> {
        let b1 = self.engine_bytes()?;
        let b0 = resave_hwpx(&self.original)?;
        if is_hwpx(&self.original) {
            docsocto_preserve::preserve(&self.original, &b0, &b1).map_err(|e| e.to_string())
        } else {
            // HWP 5.0 원본은 패키지가 달라 맞댈 수 없다 — 엔진 결과(HWPX)를 그대로 쓴다.
            Ok((b1, docsocto_preserve::Report { whole_engine: true, ..Default::default() }))
        }
    }
}

fn is_hwpx(data: &[u8]) -> bool {
    data.starts_with(b"PK")
}

/// 겹치지 않는 일치 위치(글자 단위).
fn char_matches(hay: &str, needle: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(i) = hay[from..].find(needle) {
        let b = from + i;
        out.push(hay[..b].chars().count());
        from = b + needle.len();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "../../samples/m0/05_산업부_NTB수요기술_등록동의서.hwpx";

    fn session() -> Session {
        Session::open(std::fs::read(SAMPLE).expect("시험 양식")).unwrap()
    }

    #[test]
    fn every_command_has_a_name_in_both_languages() {
        for c in commands() {
            assert!(!c.ko.is_empty() && !c.en.is_empty(), "{}", c.id);
        }
        let mut ids: Vec<_> = commands().iter().map(|c| c.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), commands().len(), "명령 id 중복");
    }

    #[test]
    fn char_matches_counts_chars_not_bytes() {
        assert_eq!(char_matches("가나다나", "나"), vec![1, 3]);
    }

    #[test]
    fn set_cell_then_get_cell() {
        let mut s = session();
        s.run("table.setCell", &json!({"table":0,"row":1,"col":1,"text":"DocsOcto 시험"})).unwrap();
        let v = s.run("table.getCell", &json!({"table":0,"row":1,"col":1})).unwrap();
        assert_eq!(v["text"], "DocsOcto 시험");
    }

    #[test]
    fn row_insert_and_delete_keep_shape() {
        let mut s = session();
        let before = s.run("table.list", &json!({})).unwrap()[0]["rows"].as_u64().unwrap();
        s.run("table.insertRow", &json!({"table":0,"row":0})).unwrap();
        assert_eq!(s.run("table.list", &json!({})).unwrap()[0]["rows"].as_u64().unwrap(), before + 1);
        s.run("table.deleteRow", &json!({"table":0,"row":1})).unwrap();
        assert_eq!(s.run("table.list", &json!({})).unwrap()[0]["rows"].as_u64().unwrap(), before);
    }

    #[test]
    fn replace_counts_matches() {
        let mut s = session();
        let v = s.run("text.replace", &json!({"find":"NTB","replace":"NTB"})).unwrap();
        assert!(v["replaced"].as_u64().unwrap() >= 2);
    }

    #[test]
    fn save_keeps_untouched_parts_byte_identical() {
        let mut s = session();
        s.run("table.setCell", &json!({"table":0,"row":1,"col":1,"text":"DocsOcto 시험"})).unwrap();
        let (out, report) = s.save().unwrap();
        assert!(!report.whole_engine);
        let o = docsocto_preserve::read_package(&std::fs::read(SAMPLE).unwrap()).unwrap();
        let p = docsocto_preserve::read_package(&out).unwrap();
        let changed: Vec<_> = o.iter().filter(|e| p.iter().find(|x| x.name == e.name).map(|x| x.data != e.data).unwrap_or(true)).map(|e| e.name.clone()).collect();
        assert_eq!(changed, vec!["Contents/section0.xml".to_string()]);
        // 다시 열려야 한다
        Session::open(out).unwrap();
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(session().run("no.such", &json!({})).is_err());
    }
}
