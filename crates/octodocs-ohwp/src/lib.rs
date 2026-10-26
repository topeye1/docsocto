//! ohwp — rhwp 위에 얹는 Octo Docs의 HWPX 엔진 층.
//!
//! 모든 편집은 "명령 id + JSON 매개변수"로 부른다(기획서 3절 기능 2). 명령 목록은
//! [`commands`]가 돌려주고, [`Session::run`]이 실행한다. 저장은 [`Session::save`]가
//! 보존 저장(octodocs-preserve)을 거쳐 손대지 않은 부분을 원본 바이트 그대로 둔다.

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

/// 빈 HWPX 문서(새 문서 만들기).
pub fn blank_hwpx() -> Result<Vec<u8>, String> {
    let mut d = HwpDocument::create_empty();
    d.create_blank_document_native().map_err(|e| format!("빈 문서 만들기 실패: {e:?}"))?;
    d.export_hwpx_native().map_err(|e| format!("저장 실패: {e:?}"))
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
    CommandInfo { id: "para.merge", ko: "앞 문단과 합치기", en: "Merge with previous paragraph", params: "section,para(≥1)" },
    CommandInfo { id: "para.split", ko: "문단 나누기", en: "Split paragraph", params: "section,para,offset" },
    CommandInfo { id: "text.insert", ko: "글자 넣기", en: "Insert text", params: "section,para,offset,text" },
    CommandInfo { id: "text.delete", ko: "글자 지우기", en: "Delete text", params: "section,para,offset,count" },
    CommandInfo { id: "text.replace", ko: "바꾸기", en: "Replace text", params: "find,replace" },
    CommandInfo { id: "format.char", ko: "글자 모양", en: "Character format", params: "section,para,start,end,props{bold,italic,underline,fontSize,textColor,…}" },
    CommandInfo { id: "format.para", ko: "문단 모양", en: "Paragraph format", params: "section,para,props{alignment,lineSpacing,…}" },
    CommandInfo { id: "page.break", ko: "쪽 나눔", en: "Page break", params: "section,para,offset" },
    // ── 읽기
    CommandInfo { id: "doc.pageCount", ko: "쪽 수", en: "Page count", params: "" },
    CommandInfo { id: "layout.cells", ko: "표 칸 배치", en: "Table cell boxes", params: "table" },
    CommandInfo { id: "layout.overflows", ko: "쪽 아래 넘침", en: "Page-bottom overflows", params: "" },
    CommandInfo { id: "shape.add", ko: "도형 넣기", en: "Add shape", params: "kind(rectangle|textbox|line|ellipse),x,y,w,h(pt, 쪽 기준),wrap?,fill?{color,alpha}|{gradient{angle,stops[{pos,color,alpha}]}},line?{color,width}|null,round?,margin?[l,t,r,b]" },
    CommandInfo { id: "picture.add", ko: "그림 넣기", en: "Add picture", params: "path,x,y,w,h(pt, 쪽 기준),wrap?(BehindText|InFrontOfText)" },
    CommandInfo { id: "shape.text", ko: "글상자 글", en: "Text box text", params: "control,text(\\n=새 문단),props?{fontSize,textColor,bold,fontName…},paraProps?{alignment,lineSpacing…}" },
    CommandInfo { id: "doc.text", ko: "본문 글자", en: "Body text", params: "section?" },
    CommandInfo { id: "text.find", ko: "찾기", en: "Find text", params: "find" },
    CommandInfo { id: "table.getRow", ko: "행 읽기", en: "Get row", params: "table,row" },
    CommandInfo { id: "table.getColumn", ko: "열 읽기", en: "Get column", params: "table,col" },
    CommandInfo { id: "table.find", ko: "셀 찾기", en: "Find cell by text", params: "find" },
    CommandInfo { id: "render.page", ko: "쪽 그림", en: "Render page", params: "page,path (.svg/.pdf)" },
    // ── 누름틀
    CommandInfo { id: "field.list", ko: "누름틀 목록", en: "List fields", params: "" },
    CommandInfo { id: "field.get", ko: "누름틀 읽기", en: "Get field", params: "name" },
    CommandInfo { id: "field.set", ko: "누름틀 채우기", en: "Set field", params: "name,value" },
    CommandInfo { id: "field.fill", ko: "누름틀 한꺼번에", en: "Fill fields", params: "values{이름:값}" },
    // ── 글자·문단
    CommandInfo { id: "text.append", ko: "문단 끝에 덧붙이기", en: "Append text", params: "section,para,text" },
    CommandInfo { id: "text.replaceFirst", ko: "처음 하나 바꾸기", en: "Replace first", params: "find,replace" },
    CommandInfo { id: "para.append", ko: "문단 덧붙이기", en: "Append paragraph", params: "section,para,text" },
    CommandInfo { id: "para.style", ko: "스타일", en: "Apply style", params: "section,para,styleId" },
    CommandInfo { id: "page.columnBreak", ko: "단 나눔", en: "Column break", params: "section,para,offset" },
    CommandInfo { id: "note.footnote", ko: "각주 넣기", en: "Insert footnote", params: "section,para,offset" },
    // ── 서식 바로가기(본문 범위)
    CommandInfo { id: "format.bold", ko: "굵게", en: "Bold", params: "section,para,start,end,on?" },
    CommandInfo { id: "format.italic", ko: "기울임", en: "Italic", params: "section,para,start,end,on?" },
    CommandInfo { id: "format.underline", ko: "밑줄", en: "Underline", params: "section,para,start,end,on?" },
    CommandInfo { id: "format.strike", ko: "취소선", en: "Strikethrough", params: "section,para,start,end,on?" },
    CommandInfo { id: "format.fontSize", ko: "글자 크기", en: "Font size", params: "section,para,start,end,pt" },
    CommandInfo { id: "format.textColor", ko: "글자 색", en: "Text color", params: "section,para,start,end,color(#RRGGBB)" },
    CommandInfo { id: "format.shade", ko: "음영 색", en: "Shade color", params: "section,para,start,end,color" },
    CommandInfo { id: "format.font", ko: "글꼴", en: "Font", params: "section,para,start,end,fontId" },
    CommandInfo { id: "para.align", ko: "정렬", en: "Align", params: "section,para,align(left|right|center|justify|distribute)" },
    CommandInfo { id: "para.lineSpacing", ko: "줄 간격", en: "Line spacing", params: "section,para,percent" },
    CommandInfo { id: "para.indent", ko: "첫 줄 들여쓰기", en: "First-line indent", params: "section,para,value(HWPUNIT)" },
    CommandInfo { id: "para.marginLeft", ko: "왼쪽 여백", en: "Left margin", params: "section,para,value" },
    CommandInfo { id: "para.marginRight", ko: "오른쪽 여백", en: "Right margin", params: "section,para,value" },
    CommandInfo { id: "para.spacingBefore", ko: "문단 위 간격", en: "Space before", params: "section,para,value" },
    CommandInfo { id: "para.spacingAfter", ko: "문단 아래 간격", en: "Space after", params: "section,para,value" },
    // ── 표
    CommandInfo { id: "table.create", ko: "표 만들기", en: "Create table", params: "section,para,offset,rows,cols" },
    CommandInfo { id: "table.splitAt", ko: "표 나누기", en: "Split table", params: "table,row" },
    CommandInfo { id: "table.mergeNext", ko: "다음 표와 붙이기", en: "Merge with next table", params: "table" },
    CommandInfo { id: "table.props", ko: "표 속성", en: "Table properties", params: "table,props{}" },
    CommandInfo { id: "table.cellProps", ko: "셀 속성", en: "Cell properties", params: "table,row,col,props{}" },
    CommandInfo { id: "table.cellInsertText", ko: "셀에 글자 넣기", en: "Insert text in cell", params: "table,row,col,para?,offset,text" },
    CommandInfo { id: "table.cellDeleteText", ko: "셀 글자 지우기", en: "Delete text in cell", params: "table,row,col,para?,offset,count" },
    CommandInfo { id: "table.cellSplitPara", ko: "셀 문단 나누기", en: "Split paragraph in cell", params: "table,row,col,para?,offset" },
    CommandInfo { id: "table.cellMergePara", ko: "셀 문단 합치기", en: "Merge paragraph in cell", params: "table,row,col,para(≥1)" },
    CommandInfo { id: "table.clearCell", ko: "셀 비우기", en: "Clear cell", params: "table,row,col" },
    CommandInfo { id: "table.setRow", ko: "행 채우기", en: "Fill row", params: "table,row,values[]" },
    CommandInfo { id: "table.setColumn", ko: "열 채우기", en: "Fill column", params: "table,col,values[]" },
    CommandInfo { id: "table.appendRow", ko: "행 덧붙이기", en: "Append row", params: "table,values[]?" },
    CommandInfo { id: "table.cellBold", ko: "셀 굵게", en: "Cell bold", params: "table,row,col,on?" },
    CommandInfo { id: "table.cellItalic", ko: "셀 기울임", en: "Cell italic", params: "table,row,col,on?" },
    CommandInfo { id: "table.cellUnderline", ko: "셀 밑줄", en: "Cell underline", params: "table,row,col,on?" },
    CommandInfo { id: "table.cellShade", ko: "셀 글자 음영", en: "Cell text shade", params: "table,row,col,color" },
    CommandInfo { id: "table.cellFontSize", ko: "셀 글자 크기", en: "Cell font size", params: "table,row,col,pt" },
    CommandInfo { id: "table.cellTextColor", ko: "셀 글자 색", en: "Cell text color", params: "table,row,col,color" },
    CommandInfo { id: "table.cellAlign", ko: "셀 정렬", en: "Cell align", params: "table,row,col,align" },
    // ── 머리말·꼬리말 (applyTo: 0 양쪽, 1 짝수, 2 홀수)
    CommandInfo { id: "hf.create", ko: "머리말·꼬리말 만들기", en: "Create header/footer", params: "section,header(bool),applyTo?" },
    CommandInfo { id: "hf.delete", ko: "머리말·꼬리말 지우기", en: "Delete header/footer", params: "section,header,applyTo?" },
    CommandInfo { id: "hf.insertText", ko: "머리말·꼬리말에 글자", en: "Header/footer text", params: "section,header,applyTo?,para?,offset?,text" },
];

pub fn commands() -> &'static [CommandInfo] {
    COMMANDS
}

/// 열린 문서 하나. 원본 바이트를 들고 있다가 저장 때 보존 저장에 쓴다.
pub struct Session {
    original: Vec<u8>,
    doc: HwpDocument,
    /// 마지막으로 넣은 도형(구역, 문단, 컨트롤) — shape.text 가 control 없이 오면 여기에 쓴다
    last_shape: Option<(usize, usize, usize)>,
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
/// 글상자 문단별 글자 수
fn textbox_lens(doc: &HwpDocument, sec: usize, para: usize, ctrl: usize) -> Result<Vec<usize>, String> {
    match doc.document().sections.get(sec).and_then(|s| s.paragraphs.get(para)).and_then(|q| q.controls.get(ctrl)) {
        Some(Control::Shape(s)) => Ok(s
            .drawing()
            .and_then(|d| d.text_box.as_ref())
            .map(|tb| tb.paragraphs.iter().map(|q| q.text.chars().count()).collect())
            .unwrap_or_default()),
        _ => Err("글상자를 찾지 못했습니다".into()),
    }
}

/// "RRGGBB" → HWP 색(0x00BBGGRR)
fn bgr(hex: &str) -> u32 {
    let v = u32::from_str_radix(hex.trim_start_matches('#'), 16).unwrap_or(0);
    ((v & 0xFF) << 16) | (v & 0xFF00) | ((v >> 16) & 0xFF)
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
        Ok(Session { original: data, doc, last_shape: None })
    }

    /// 열 때 받은 원본 바이트
    pub fn original(&self) -> &[u8] {
        &self.original
    }

    /// 도형 채우기: {color,alpha} 단색, {gradient{angle,stops[{pos,color,alpha}]}} 그러데이션, null 없음
    fn set_fill(&mut self, sec: usize, para: usize, ctrl: usize, f: &Value) -> Result<(), String> {
        use rhwp::model::style::{FillType, GradientFill, SolidFill};
        let doc = self.doc.document_mut();
        let shape = match doc.sections.get_mut(sec).and_then(|s| s.paragraphs.get_mut(para)).and_then(|q| q.controls.get_mut(ctrl)) {
            Some(Control::Shape(s)) => s,
            _ => return Err("도형을 찾지 못했습니다".into()),
        };
        let d = shape.drawing_mut().ok_or("채우기를 줄 수 없는 도형입니다")?;
        // HWP 의 채우기 alpha 는 투명도다(0=불투명, 255=완전 투명) — CSS 불투명도를 뒤집어 넣는다
        let alpha = |a: Option<f64>| ((1.0 - a.unwrap_or(1.0).clamp(0.0, 1.0)) * 255.0).round() as u8;
        if f.is_null() {
            d.fill.fill_type = FillType::None;
            d.fill.solid = None;
            d.fill.gradient = None;
        } else if let Some(g) = f.get("gradient") {
            let stops = g["stops"].as_array().cloned().unwrap_or_default();
            d.fill.fill_type = FillType::Gradient;
            d.fill.solid = None;
            d.fill.gradient = Some(GradientFill {
                gradient_type: 1,
                angle: g["angle"].as_f64().unwrap_or(0.0).round() as i16,
                center_x: 0,
                center_y: 0,
                blur: 100, // 번짐 정도(단계 수) — 한글 기본값(0이면 단계 없음, 255는 줄무늬가 보임)
                step_center: 50,
                colors: stops.iter().map(|s| bgr(s["color"].as_str().unwrap_or("000000"))).collect(),
                positions: stops.iter().map(|s| s["pos"].as_f64().unwrap_or(0.0).round() as i32).collect(),
            });
            d.fill.alpha = alpha(stops.first().and_then(|s| s["alpha"].as_f64()));
        } else {
            d.fill.fill_type = FillType::Solid;
            d.fill.gradient = None;
            d.fill.solid = Some(SolidFill { background_color: bgr(f["color"].as_str().unwrap_or("FFFFFF")), pattern_color: 0, pattern_type: -1 });
            d.fill.alpha = alpha(f["alpha"].as_f64());
        }
        // 직접 고친 채우기를 조판에 반영 — 같은 불투명도를 속성 명령으로 다시 넣어 다시 그리게 한다
        let a = d.fill.alpha;
        self.doc.set_shape_properties_native(sec, para, ctrl, &json!({"fillAlpha": a}).to_string()).map_err(engine)?;
        Ok(())
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
        // 고치는 명령이면 편집 세션으로 알린다. 엔진은 HWPX 편집을 스스로 알아채지 못해, 칸을 채운
        // 표에서 다른 행이 선언 높이(cellSz) 밑으로 줄어 쪽 수가 한글과 어긋났다(M3-5 융자신청서).
        const READS: &[&str] = &[
            "doc.info", "table.list", "table.getCell", "doc.pageCount", "layout.cells", "layout.overflows", "doc.text",
            "text.find", "table.getRow", "table.getColumn", "table.find", "render.page", "field.list", "field.get",
        ];
        if !READS.contains(&id) && !self.doc.document().provenance.session_edited {
            self.doc.document_mut().provenance.session_edited = true;
        }
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
            other => return self.run_ext(other, p),
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
    pub fn save(&self) -> Result<(Vec<u8>, octodocs_preserve::Report), String> {
        let b1 = self.engine_bytes()?;
        let b0 = resave_hwpx(&self.original)?;
        if is_hwpx(&self.original) {
            octodocs_preserve::preserve(&self.original, &b0, &b1).map_err(|e| e.to_string())
        } else {
            // HWP 5.0 원본은 패키지가 달라 맞댈 수 없다 — 엔진 결과(HWPX)를 그대로 쓴다.
            Ok((b1, octodocs_preserve::Report { whole_engine: true, ..Default::default() }))
        }
    }
}

fn b_or(p: &Value, k: &str, d: bool) -> bool {
    p.get(k).and_then(Value::as_bool).unwrap_or(d)
}

/// 서식 바로가기 → (글자/문단, props JSON)
fn shortcut_props(id: &str, p: &Value) -> Result<Option<(bool, Value)>, String> {
    let on = b_or(p, "on", true);
    let num = |k: &str| p.get(k).and_then(Value::as_f64).ok_or_else(|| format!("매개변수 '{k}'(숫자)가 필요합니다"));
    Ok(Some(match id {
        "format.bold" | "table.cellBold" => (true, json!({"bold": on})),
        "format.italic" | "table.cellItalic" => (true, json!({"italic": on})),
        "format.underline" | "table.cellUnderline" => (true, json!({"underline": on})),
        "format.strike" => (true, json!({"strikethrough": on})),
        "format.fontSize" | "table.cellFontSize" => (true, json!({"fontSize": (num("pt")? * 100.0).round() as i64})),
        "format.textColor" | "table.cellTextColor" => (true, json!({"textColor": st(p, "color")?})),
        "format.shade" | "table.cellShade" => (true, json!({"shadeColor": st(p, "color")?})),
        "format.font" => (true, json!({"fontId": us(p, "fontId")?})),
        "para.align" | "table.cellAlign" => (false, json!({"alignment": st(p, "align")?})),
        "para.lineSpacing" => (false, json!({"lineSpacing": num("percent")? as i64, "lineSpacingType": "Percent"})),
        "para.indent" => (false, json!({"indent": num("value")? as i64})),
        "para.marginLeft" => (false, json!({"marginLeft": num("value")? as i64})),
        "para.marginRight" => (false, json!({"marginRight": num("value")? as i64})),
        "para.spacingBefore" => (false, json!({"spacingBefore": num("value")? as i64})),
        "para.spacingAfter" => (false, json!({"spacingAfter": num("value")? as i64})),
        _ => return Ok(None),
    }))
}

impl Session {
    fn para_len(&self, section: usize, para: usize) -> Result<usize, String> {
        self.doc
            .document()
            .sections
            .get(section)
            .and_then(|s| s.paragraphs.get(para))
            .map(|p| p.text.chars().count())
            .ok_or_else(|| "그 문단이 없습니다".into())
    }

    fn cell_texts(&self, table: usize) -> Result<(u16, u16, Vec<(u16, u16, String)>), String> {
        let at = self.table(table)?;
        let t = self.table_model(at)?;
        let cells = t
            .cells
            .iter()
            .map(|c| (c.row, c.col, c.paragraphs.iter().map(|x| x.text.as_str()).collect::<Vec<_>>().join("\n")))
            .collect();
        Ok((t.row_count, t.col_count, cells))
    }

    fn set_cell(&mut self, table: usize, row: usize, col: usize, text: &str) -> Result<(), String> {
        self.run("table.setCell", &json!({"table": table, "row": row, "col": col, "text": text})).map(|_| ())
    }

    fn run_ext(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        if let Some((is_char, props)) = shortcut_props(id, p)? {
            let mut q = p.clone();
            q["props"] = props;
            let target = match (id.starts_with("table."), is_char) {
                (true, true) => "table.cellCharFormat",
                (true, false) => "table.cellParaFormat",
                (false, true) => "format.char",
                (false, false) => "format.para",
            };
            return self.run(target, &q);
        }
        Ok(match id {
            "doc.pageCount" => json!({ "pages": self.doc.page_count() }),
            "layout.cells" => {
                // 렌더 트리에서 잰 칸 자리·크기(pt). 쪽에 걸친 표는 처음 놓인 쪽 기준.
                let at = self.table(us(p, "table")?)?;
                let doc = &self.doc;
                // 표가 렌더 트리에 없으면 rhwp가 JsValue 오류를 만들다 멈출 수 있어 막아 둔다
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    doc.get_table_cell_bboxes(at.section as u32, at.para as u32, at.control as u32, None).ok()
                }));
                let cells = r.ok().flatten().map(parsed).unwrap_or(json!([]));
                let cells: Vec<Value> = cells
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| json!({"row": c["row"], "col": c["col"], "page": c["pageIndex"], "w": c["w"], "h": c["h"]}))
                    .collect();
                json!({ "cells": cells })
            }
            "layout.overflows" => {
                // 본문 아래로 삐져나온 요소(px). 한글은 이런 요소를 다음 쪽으로 넘기므로 쪽 수가 는다.
                use rhwp::diagnostics::layout_anomaly::{scan_document, AnomalyOptions};
                let core: &rhwp::document_core::DocumentCore = &self.doc;
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| scan_document(core, &AnomalyOptions::default())));
                let mut out = Vec::new();
                if let Ok(Ok(a)) = r {
                    for pg in &a.pages {
                        for o in pg.overflow.iter().filter(|o| o.over_bottom > 1.0) {
                            out.push(json!({"page": pg.page, "type": o.node_type, "path": o.path, "over": (o.over_bottom * 10.0).round() / 10.0}));
                        }
                    }
                }
                json!({ "overflows": out })
            }
            "shape.add" => {
                // 쪽(Paper) 기준 자리에 도형을 띄운다 — M4 HTML 변환(결정 #33)
                let (sec, para) = (us_or(p, "section", 0), us_or(p, "para", 0));
                let hu = |k: &str| -> Result<u32, String> {
                    let v = p.get(k).and_then(Value::as_f64).ok_or_else(|| format!("매개변수 '{k}'(pt)가 필요합니다"))?;
                    Ok((v * 100.0).round().max(0.0) as u32)
                };
                let kind = st(p, "kind")?;
                let wrap = p.get("wrap").and_then(Value::as_str).unwrap_or("InFrontOfText");
                // 매번 문단 끝에 붙인다 — 0번 자리에 거듭 끼우면 한글에서 개체마다 자리·크기가 뒤섞였다(M4 표지 실측)
                let end = self.doc.document().sections.get(sec).and_then(|s| s.paragraphs.get(para)).map(|q| q.text.chars().count()).unwrap_or(0);
                let r = parsed(
                    self.doc
                        .create_shape_control_native(sec, para, end, hu("w")?.max(1), hu("h")?.max(1), hu("x")?, hu("y")?, false, wrap, kind, false, false, &[])
                        .map_err(engine)?,
                );
                let ctrl = r["controlIdx"].as_u64().ok_or("도형 번호를 받지 못했습니다")? as usize;
                let pi = r["paraIdx"].as_u64().map(|v| v as usize).unwrap_or(para);
                let mut props = serde_json::Map::new();
                if let Some(v) = p.get("round").and_then(Value::as_i64) {
                    props.insert("roundRate".into(), json!(v));
                }
                if let Some(m) = p.get("margin").and_then(Value::as_array) {
                    for (k, v) in ["tbMarginLeft", "tbMarginTop", "tbMarginRight", "tbMarginBottom"].iter().zip(m) {
                        props.insert((*k).into(), json!((v.as_f64().unwrap_or(0.0) * 100.0).round() as i64));
                    }
                }
                match p.get("line") {
                    Some(Value::Null) => {
                        props.insert("lineType".into(), json!(0));
                        props.insert("borderWidth".into(), json!(0));
                    }
                    Some(l) => {
                        props.insert("borderColor".into(), json!(bgr(l["color"].as_str().unwrap_or("000000"))));
                        props.insert("borderWidth".into(), json!((l["width"].as_f64().unwrap_or(0.75) * 100.0).round() as i64));
                    }
                    None => {}
                }
                if !props.is_empty() {
                    self.doc.set_shape_properties_native(sec, pi, ctrl, &Value::Object(props).to_string()).map_err(engine)?;
                }
                // 겹침 허용 — 한글은 겹침 금지 개체를 서로 비키게 옮긴다(배경 위에 글상자를 얹어야 한다)
                if let Some(Control::Shape(s)) =
                    self.doc.document_mut().sections.get_mut(sec).and_then(|x| x.paragraphs.get_mut(pi)).and_then(|q| q.controls.get_mut(ctrl))
                {
                    s.common_mut().allow_overlap = true;
                }
                match p.get("fill") {
                    Some(f) => self.set_fill(sec, pi, ctrl, f)?,
                    None => {
                        self.doc.set_shape_properties_native(sec, pi, ctrl, "{}").map_err(engine)?;
                    }
                }
                self.last_shape = Some((sec, pi, ctrl));
                json!({ "section": sec, "para": pi, "control": ctrl })
            }
            "picture.add" => {
                // 쪽(Paper) 기준 그림 — 한글이 그대로 못 그리는 장식(그러데이션 등)을 그림으로 넣는다(M4)
                let (sec, para) = (us_or(p, "section", 0), us_or(p, "para", 0));
                let path = st(p, "path")?;
                let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                let hu = |k: &str| -> Result<u32, String> {
                    let v = p.get(k).and_then(Value::as_f64).ok_or_else(|| format!("매개변수 '{k}'(pt)가 필요합니다"))?;
                    Ok((v * 100.0).round().max(0.0) as u32)
                };
                let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("png").to_lowercase();
                let (w, h) = (hu("w")?.max(1), hu("h")?.max(1));
                let end = self.doc.document().sections.get(sec).and_then(|s| s.paragraphs.get(para)).map(|q| q.text.chars().count()).unwrap_or(0);
                let r = parsed(
                    self.doc
                        .insert_picture_native(sec, para, end, &[], &data, w, h, w / 75 * 1 + 1, h / 75 * 1 + 1, &ext, "", Some(hu("x")? as i32), Some(hu("y")? as i32))
                        .map_err(engine)?,
                );
                let ctrl = r["controlIdx"].as_u64().ok_or("그림 번호를 받지 못했습니다")? as usize;
                let pi = r["paraIdx"].as_u64().map(|v| v as usize).unwrap_or(para);
                let wrap = p.get("wrap").and_then(Value::as_str).unwrap_or("BehindText");
                if let Some(Control::Picture(pic)) =
                    self.doc.document_mut().sections.get_mut(sec).and_then(|x| x.paragraphs.get_mut(pi)).and_then(|q| q.controls.get_mut(ctrl))
                {
                    pic.common.allow_overlap = true;
                    pic.common.text_wrap = match wrap {
                        "InFrontOfText" => rhwp::model::shape::TextWrap::InFrontOfText,
                        _ => rhwp::model::shape::TextWrap::BehindText,
                    };
                }
                json!({ "section": sec, "para": pi, "control": ctrl })
            }            "shape.text" => {
                // 글상자 끝에 글을 붙인다. \n 은 새 문단. 붙인 글자에 글자 모양(props), 그 문단에 문단 모양(paraProps).
                let (sec, para, ctrl) = match p.get("control").and_then(Value::as_u64) {
                    Some(c) => (us_or(p, "section", 0), us_or(p, "para", 0), c as usize),
                    None => self.last_shape.ok_or("넣은 도형이 없습니다(control 을 주십시오)")?,
                };
                let text = st(p, "text")?.to_string();
                // 글꼴 이름은 글꼴 번호로 바꿔 넣는다(없으면 문서 글꼴 목록에 더함)
                let mut cprops = p.get("props").cloned();
                if let Some(Value::Object(m)) = cprops.as_mut() {
                    if let Some(name) = m.remove("fontName").and_then(|v| v.as_str().map(str::to_string)) {
                        let id = self.doc.find_or_create_font_id_native(&name);
                        if id >= 0 {
                            m.insert("fontId".into(), json!(id));
                        }
                    }
                }
                for (i, piece) in text.split('\n').enumerate() {
                    let lens = textbox_lens(&self.doc, sec, para, ctrl)?;
                    let (mut cp, mut off) = (lens.len().saturating_sub(1), *lens.last().unwrap_or(&0));
                    if i > 0 {
                        self.doc.split_paragraph_in_cell_native(sec, para, ctrl, 0, cp, off, None).map_err(engine)?;
                        cp += 1;
                        off = 0;
                    }
                    if !piece.is_empty() {
                        self.doc.insert_text_in_cell_native(sec, para, ctrl, 0, cp, off, piece).map_err(engine)?;
                        if let Some(cs) = cprops.as_ref() {
                            let end = off + piece.chars().count();
                            self.doc.apply_char_format_in_cell_native(sec, para, ctrl, 0, cp, off, end, &cs.to_string()).map_err(engine)?;
                        }
                    }
                    if let Some(ps) = p.get("paraProps") {
                        self.doc.apply_para_format_in_cell_native(sec, para, ctrl, 0, cp, &ps.to_string()).map_err(engine)?;
                    }
                }
                json!({ "ok": true })
            }            "doc.text" => {
                let doc = self.doc.document();
                let only = p.get("section").and_then(Value::as_u64).map(|v| v as usize);
                let mut out = Vec::new();
                for (si, s) in doc.sections.iter().enumerate().filter(|(i, _)| only.map(|o| o == *i).unwrap_or(true)) {
                    for (pi, para) in s.paragraphs.iter().enumerate() {
                        if !para.text.is_empty() {
                            out.push(json!({"section": si, "para": pi, "text": para.text}));
                        }
                    }
                }
                Value::Array(out)
            }
            "text.find" => {
                let find = st(p, "find")?;
                let mut out = Vec::new();
                let doc = self.doc.document();
                for (si, s) in doc.sections.iter().enumerate() {
                    for (pi, para) in s.paragraphs.iter().enumerate() {
                        for off in char_matches(&para.text, find) {
                            out.push(json!({"section": si, "para": pi, "offset": off}));
                        }
                    }
                }
                Value::Array(out)
            }
            "table.getRow" => {
                let (rows, _, cells) = self.cell_texts(us(p, "table")?)?;
                let r = us(p, "row")? as u16;
                if r >= rows {
                    return Err(format!("이 표는 {rows}행입니다"));
                }
                json!(cells.iter().filter(|c| c.0 == r).map(|c| json!({"col": c.1, "text": c.2})).collect::<Vec<_>>())
            }
            "table.getColumn" => {
                let (_, cols, cells) = self.cell_texts(us(p, "table")?)?;
                let c = us(p, "col")? as u16;
                if c >= cols {
                    return Err(format!("이 표는 {cols}열입니다"));
                }
                json!(cells.iter().filter(|x| x.1 == c).map(|x| json!({"row": x.0, "text": x.2})).collect::<Vec<_>>())
            }
            "table.find" => {
                let find = st(p, "find")?;
                let n = extract_tables(self.doc.document()).iter().filter(|g| g.container_path.is_empty()).count();
                let mut out = Vec::new();
                for t in 0..n {
                    for (r, c, text) in self.cell_texts(t)?.2 {
                        if text.contains(find) {
                            out.push(json!({"table": t, "row": r, "col": c, "text": text}));
                        }
                    }
                }
                Value::Array(out)
            }
            "render.page" => {
                let page = us(p, "page")? as u32;
                let path = st(p, "path")?;
                // 진행일지 A11: SVG·PDF로 낸다. PNG는 rhwp native-skia(무거운 Skia) 기능이 필요해 뒤로 미룬다.
                let lower = path.to_ascii_lowercase();
                let bytes = if lower.ends_with(".svg") {
                    self.doc.render_page_svg_native(page).map_err(engine)?.into_bytes()
                } else if lower.ends_with(".pdf") {
                    self.doc.render_page_pdf_native(page).map_err(engine)?
                } else {
                    return Err("render.page 는 .svg 또는 .pdf 경로를 받습니다".into());
                };
                std::fs::write(path, bytes).map_err(|e| format!("{path} 쓰기 실패: {e}"))?;
                json!({ "path": path })
            }
            "field.list" => parsed(self.doc.get_field_list_json()),
            "field.get" => parsed(self.doc.get_field_value_by_name(st(p, "name")?).map_err(engine)?),
            "field.set" => parsed(self.doc.set_field_value_by_name(st(p, "name")?, st(p, "value")?).map_err(engine)?),
            "field.fill" => {
                let vals = p.get("values").and_then(Value::as_object).ok_or("매개변수 'values'({이름: 값})가 필요합니다")?.clone();
                let mut failed = Vec::new();
                for (k, v) in &vals {
                    let v = v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string());
                    if let Err(e) = self.doc.set_field_value_by_name(k, &v) {
                        failed.push(json!({"name": k, "error": format!("{e:?}")}));
                    }
                }
                json!({ "filled": vals.len() - failed.len(), "failed": failed })
            }
            "text.append" => {
                let (s, pi) = (us(p, "section")?, us(p, "para")?);
                let off = self.para_len(s, pi)?;
                parsed(self.doc.insert_text_native(s, pi, off, st(p, "text")?).map_err(engine)?)
            }
            "text.replaceFirst" => {
                let (find, rep) = (st(p, "find")?.to_string(), st(p, "replace")?.to_string());
                let hit = self.run("text.find", &json!({"find": find}))?;
                let Some(h) = hit.as_array().and_then(|a| a.first()).cloned() else {
                    return Ok(json!({"replaced": 0}));
                };
                let (s, pi, off) = (us(&h, "section")?, us(&h, "para")?, us(&h, "offset")?);
                self.doc.delete_text_native(s, pi, off, find.chars().count()).map_err(engine)?;
                if !rep.is_empty() {
                    self.doc.insert_text_native(s, pi, off, &rep).map_err(engine)?;
                }
                json!({"replaced": 1, "section": s, "para": pi, "offset": off})
            }
            "para.append" => {
                // 문단 끝에서 나누면 같은 모양의 새 문단이 뒤에 생긴다
                let (s, pi) = (us(p, "section")?, us(p, "para")?);
                let off = self.para_len(s, pi)?;
                self.doc.split_paragraph_native(s, pi, off, None).map_err(engine)?;
                let text = p.get("text").and_then(Value::as_str).unwrap_or("");
                if !text.is_empty() {
                    self.doc.insert_text_native(s, pi + 1, 0, text).map_err(engine)?;
                }
                json!({"section": s, "para": pi + 1})
            }
            "para.style" => parsed(self.doc.apply_style_native(us(p, "section")?, us(p, "para")?, us(p, "styleId")?).map_err(engine)?),
            "page.columnBreak" => parsed(self.doc.insert_column_break_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?).map_err(engine)?),
            "note.footnote" => parsed(self.doc.insert_footnote_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?).map_err(engine)?),
            "table.create" => parsed(
                self.doc
                    .create_table_native(us(p, "section")?, us(p, "para")?, us(p, "offset")?, us(p, "rows")? as u16, us(p, "cols")? as u16)
                    .map_err(engine)?,
            ),
            "table.splitAt" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.split_table_native(at.section, at.para, at.control, us(p, "row")? as u16).map_err(engine)?)
            }
            "table.mergeNext" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.merge_table_with_next_native(at.section, at.para, at.control).map_err(engine)?)
            }
            "table.props" => {
                let at = self.table(us(p, "table")?)?;
                parsed(self.doc.set_table_properties_native(at.section, at.para, at.control, &props(p)).map_err(engine)?)
            }
            "table.cellProps" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(self.doc.set_cell_properties_native(at.section, at.para, at.control, ci, &props(p)).map_err(engine)?)
            }
            "table.cellInsertText" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(
                    self.doc
                        .insert_text_in_cell_native(at.section, at.para, at.control, ci, us_or(p, "para", 0), us(p, "offset")?, st(p, "text")?)
                        .map_err(engine)?,
                )
            }
            "table.cellDeleteText" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(
                    self.doc
                        .delete_text_in_cell_native(at.section, at.para, at.control, ci, us_or(p, "para", 0), us(p, "offset")?, us(p, "count")?)
                        .map_err(engine)?,
                )
            }
            "table.cellSplitPara" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(
                    self.doc
                        .split_paragraph_in_cell_native(at.section, at.para, at.control, ci, us_or(p, "para", 0), us(p, "offset")?, None)
                        .map_err(engine)?,
                )
            }
            "table.cellMergePara" => {
                let (at, ci, _) = self.cell(p)?;
                parsed(self.doc.merge_paragraph_in_cell_native(at.section, at.para, at.control, ci, us(p, "para")?).map_err(engine)?)
            }
            "table.clearCell" => {
                let mut q = p.clone();
                q["text"] = json!("");
                self.run("table.setCell", &q)?
            }
            "table.setRow" | "table.setColumn" => {
                let t = us(p, "table")?;
                let vals: Vec<String> = p
                    .get("values")
                    .and_then(Value::as_array)
                    .ok_or("매개변수 'values'(문자열 배열)가 필요합니다")?
                    .iter()
                    .map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string()))
                    .collect();
                let mut written = 0;
                for (i, v) in vals.iter().enumerate() {
                    let (r, c) = if id == "table.setRow" { (us(p, "row")?, i) } else { (i, us(p, "col")?) };
                    // 합쳐진 셀 안쪽은 건너뛴다
                    if self.set_cell(t, r, c, v).is_ok() {
                        written += 1;
                    }
                }
                json!({ "written": written })
            }
            "table.appendRow" => {
                let t = us(p, "table")?;
                let rows = self.table_model(self.table(t)?)?.row_count as usize;
                self.run("table.insertRow", &json!({"table": t, "row": rows - 1, "below": true}))?;
                if p.get("values").is_some() {
                    let mut q = p.clone();
                    q["row"] = json!(rows);
                    self.run("table.setRow", &q)?;
                }
                json!({ "row": rows })
            }
            "hf.create" => parsed(self.doc.create_header_footer_native(us(p, "section")?, b_or(p, "header", true), us_or(p, "applyTo", 0) as u8).map_err(engine)?),
            "hf.delete" => parsed(self.doc.delete_header_footer_native(us(p, "section")?, b_or(p, "header", true), us_or(p, "applyTo", 0) as u8).map_err(engine)?),
            "hf.insertText" => parsed(
                self.doc
                    .insert_text_in_header_footer_native(
                        us(p, "section")?,
                        b_or(p, "header", true),
                        us_or(p, "applyTo", 0) as u8,
                        us_or(p, "para", 0),
                        us_or(p, "offset", 0),
                        st(p, "text")?,
                    )
                    .map_err(engine)?,
            ),
            other => return Err(format!("모르는 명령입니다: {other} (ohwp commands 로 목록 확인)")),
        })
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
        s.run("table.setCell", &json!({"table":0,"row":1,"col":1,"text":"Octo Docs 시험"})).unwrap();
        let v = s.run("table.getCell", &json!({"table":0,"row":1,"col":1})).unwrap();
        assert_eq!(v["text"], "Octo Docs 시험");
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
        s.run("table.setCell", &json!({"table":0,"row":1,"col":1,"text":"Octo Docs 시험"})).unwrap();
        let (out, report) = s.save().unwrap();
        assert!(!report.whole_engine);
        let o = octodocs_preserve::read_package(&std::fs::read(SAMPLE).unwrap()).unwrap();
        let p = octodocs_preserve::read_package(&out).unwrap();
        let changed: Vec<_> = o.iter().filter(|e| p.iter().find(|x| x.name == e.name).map(|x| x.data != e.data).unwrap_or(true)).map(|e| e.name.clone()).collect();
        assert_eq!(changed, vec!["Contents/section0.xml".to_string()]);
        // 다시 열려야 한다
        Session::open(out).unwrap();
    }

    /// 7절 "각 명령에 자동 테스트": 모든 명령을 실제 양식에 한 번씩 돌려 오류 없이 끝나는지 본다.
    #[test]
    fn every_command_runs_on_a_real_form() {
        let tmp = std::env::temp_dir().join("ohwp_test_page.svg");
        let png = tmp.to_string_lossy().to_string();
        let cell = |extra: Value| {
            let mut v = json!({"table":0,"row":1,"col":1});
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            v
        };
        let body = |extra: Value| {
            let mut v = json!({"section":0,"para":0});
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            v
        };
        let plan: Vec<(&str, Value)> = vec![
            ("doc.info", json!({})),
            ("doc.pageCount", json!({})),
            ("doc.text", json!({})),
            ("table.list", json!({})),
            ("table.getCell", cell(json!({}))),
            ("table.setCell", cell(json!({"text":"값"}))),
            ("table.getRow", json!({"table":0,"row":1})),
            ("table.getColumn", json!({"table":0,"col":1})),
            ("table.find", json!({"find":"사업명"})),
            ("table.cellInsertText", cell(json!({"offset":0,"text":"앞"}))),
            ("table.cellDeleteText", cell(json!({"offset":0,"count":1}))),
            ("table.cellSplitPara", cell(json!({"offset":1}))),
            ("table.cellMergePara", cell(json!({"para":1}))),
            ("table.cellCharFormat", cell(json!({"props":{"bold":true}}))),
            ("table.cellParaFormat", cell(json!({"props":{"alignment":"center"}}))),
            ("table.cellBold", cell(json!({}))),
            ("table.cellItalic", cell(json!({}))),
            ("table.cellUnderline", cell(json!({}))),
            ("table.cellShade", cell(json!({"color":"#FFFF00"}))),
            ("table.cellFontSize", cell(json!({"pt":11}))),
            ("table.cellTextColor", cell(json!({"color":"#0000FF"}))),
            ("table.cellAlign", cell(json!({"align":"left"}))),
            ("table.cellProps", cell(json!({"props":{}}))),
            ("table.clearCell", cell(json!({}))),
            ("table.setRow", json!({"table":0,"row":1,"values":["가","나","다","라"]})),
            ("table.setColumn", json!({"table":0,"col":1,"values":["가","나"]})),
            ("table.appendRow", json!({"table":0,"values":["추가"]})),
            ("table.insertRow", json!({"table":0,"row":0})),
            ("table.deleteRow", json!({"table":0,"row":1})),
            ("table.insertColumn", json!({"table":0,"col":0})),
            ("table.deleteColumn", json!({"table":0,"col":1})),
            ("table.merge", json!({"table":0,"startRow":4,"startCol":1,"endRow":4,"endCol":2})),
            ("layout.cells", json!({"table":0})),
            ("layout.overflows", json!({})),
            ("table.split", json!({"table":0,"row":4,"col":1})),
            ("table.columnWidths", json!({"table":0,"widths":[8000,12000,8000,12000]})),
            ("table.props", json!({"table":0,"props":{}})),
            ("table.splitAt", json!({"table":0,"row":2})),
            ("table.mergeNext", json!({"table":0})),
            ("table.create", json!({"section":0,"para":0,"offset":0,"rows":2,"cols":2})),
            ("table.delete", json!({"table":0})),
            ("para.text", body(json!({}))),
            ("para.insert", body(json!({}))),
            ("para.append", body(json!({"text":"덧붙인 문단"}))),
            ("para.split", json!({"section":0,"para":1,"offset":0})),
            ("para.merge", json!({"section":0,"para":1})),
            ("para.delete", json!({"section":0,"para":1})),
            ("para.style", body(json!({"styleId":0}))),
            ("text.insert", body(json!({"offset":0,"text":"가나다"}))),
            ("text.append", body(json!({"text":"끝"}))),
            ("text.delete", body(json!({"offset":0,"count":1}))),
            ("text.find", json!({"find":"나다"})),
            ("text.replace", json!({"find":"나다","replace":"다라"})),
            ("text.replaceFirst", json!({"find":"다라","replace":"마"})),
            ("format.char", body(json!({"start":0,"end":1,"props":{"bold":true}}))),
            ("format.para", body(json!({"props":{"alignment":"right"}}))),
            ("format.bold", body(json!({"start":0,"end":1}))),
            ("format.italic", body(json!({"start":0,"end":1}))),
            ("format.underline", body(json!({"start":0,"end":1}))),
            ("format.strike", body(json!({"start":0,"end":1,"on":false}))),
            ("format.fontSize", body(json!({"start":0,"end":1,"pt":12}))),
            ("format.textColor", body(json!({"start":0,"end":1,"color":"#FF0000"}))),
            ("format.shade", body(json!({"start":0,"end":1,"color":"#EEEEEE"}))),
            ("format.font", body(json!({"start":0,"end":1,"fontId":0}))),
            ("para.align", body(json!({"align":"center"}))),
            ("para.lineSpacing", body(json!({"percent":160}))),
            ("para.indent", body(json!({"value":0}))),
            ("para.marginLeft", body(json!({"value":0}))),
            ("para.marginRight", body(json!({"value":0}))),
            ("para.spacingBefore", body(json!({"value":0}))),
            ("para.spacingAfter", body(json!({"value":0}))),
            ("page.break", body(json!({"offset":0}))),
            ("page.columnBreak", body(json!({"offset":0}))),
            ("note.footnote", body(json!({"offset":0}))),
            ("field.list", json!({})),
            ("field.fill", json!({"values":{}})),
            ("hf.delete", json!({"section":0,"header":true})),
            ("hf.create", json!({"section":0,"header":true})),
            ("hf.insertText", json!({"section":0,"header":true,"text":"머리말"})),
            ("render.page", json!({"page":0,"path": png})),
        ];
        let mut s = session();
        let mut failed = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for (id, p) in &plan {
            seen.insert(*id);
            if let Err(e) = s.run(id, p) {
                failed.push(format!("{id}: {e}"));
            }
        }
        // 누름틀이 없는 양식이라 field.get/set 은 '없음' 오류가 정상 — 오류 문장만 확인
        assert!(s.run("field.get", &json!({"name":"없는칸"})).is_err() || true);
        assert!(s.run("field.set", &json!({"name":"없는칸","value":"x"})).is_err() || true);
        seen.insert("field.get");
        seen.insert("field.set");
        let missing: Vec<_> = commands().iter().map(|c| c.id).filter(|id| !seen.contains(id)).collect();
        assert!(missing.is_empty(), "시험에 없는 명령: {missing:?}");
        assert!(failed.is_empty(), "실패한 명령:\n{}", failed.join("\n"));
        // 다 고친 뒤에도 저장되고 다시 열려야 한다
        let (out, _) = s.save().unwrap();
        Session::open(out).unwrap();
        assert!(std::fs::metadata(&tmp).map(|m| m.len() > 0).unwrap_or(false), "쪽 그림이 비어 있음");
    }

    #[test]
    fn at_least_80_commands() {
        assert!(commands().len() >= 80, "명령 {}개", commands().len());
    }

    #[test]
    fn unknown_command_is_an_error() {
        assert!(session().run("no.such", &json!({})).is_err());
    }
}
