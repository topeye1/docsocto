//! docx 엔진: WordCraft(wordcraft-engine)에 공통 명령을 번역해 넘긴다(3절 기능 4).
//! WordCraft는 "커서·선택 + 명령" 방식이라 공통 명령의 위치(문단·표·셀)를 WordCraft 위치
//! `{story:"body", path:[블록,(행,셀,블록)…], off:바이트}` 로 바꾼 뒤 명령을 부른다.
//! 공통 목록에 없는 명령은 WordCraft 명령으로 그대로 넘긴다.

use serde_json::{json, Value};
use wordcraft_doc::{Block, Document};
use wordcraft_engine::Session;

use crate::p::{b_or, byte_at, char_matches, st, us, us_or};
use crate::{Engine, Format};

pub struct DocxEngine {
    name: String,
    original: Vec<u8>,
    s: Session,
}

fn pos(path: Vec<usize>, off: usize) -> Value {
    json!({"story": "body", "path": path, "off": off})
}

fn para_text(b: &Block) -> Option<String> {
    b.as_para().map(|p| p.plain_text().to_string())
}

fn cell_text(cell: &wordcraft_doc::table::Cell) -> String {
    cell.blocks.iter().filter_map(|b| para_text(b)).collect::<Vec<_>>().join("\n")
}

impl DocxEngine {
    pub fn open(path: &str, data: Vec<u8>) -> Result<Self, String> {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "문서.docx".into());
        let doc = wordcraft_engine::io::open_bytes(&name, &data)?;
        Ok(DocxEngine { name, original: data, s: Session::new(doc) })
    }

    fn doc(&self) -> &Document {
        &self.s.doc
    }

    fn wc(&mut self, id: &str, p: Value) -> Result<Value, String> {
        self.s.run(id, &p).map_err(|e| format!("{id}: {e}"))
    }

    fn check_section(p: &Value) -> Result<(), String> {
        if us_or(p, "section", 0) != 0 {
            return Err("docx는 구역 0만 씁니다(문단 번호는 본문 전체 기준)".into());
        }
        Ok(())
    }

    /// 본문 문단(블록) i 의 글자. 표 블록이면 오류.
    fn para(&self, i: usize) -> Result<String, String> {
        let b = self.doc().body.get(i).ok_or_else(|| format!("{i}번 문단이 없습니다(본문 블록 {}개)", self.doc().body.len()))?;
        para_text(b).ok_or_else(|| format!("{i}번 블록은 표입니다"))
    }

    fn tables(&self) -> Vec<usize> {
        self.doc().body.iter().enumerate().filter(|(_, b)| b.as_table().is_some()).map(|(i, _)| i).collect()
    }

    /// (표 블록 번호, 행, 셀 번호) — col 은 격자 열. 합친 셀 안쪽이면 오류.
    fn cell_at(&self, p: &Value) -> Result<(usize, usize, usize), String> {
        let t = us(p, "table")?;
        let tb = *self.tables().get(t).ok_or_else(|| format!("본문 최상위 표 {t}번이 없습니다(표 {}개)", self.tables().len()))?;
        let table = self.doc().body[tb].as_table().unwrap();
        let (row, col) = (us(p, "row")?, us(p, "col")?);
        let r = table.rows.get(row).ok_or_else(|| format!("이 표는 {}행입니다", table.rows.len()))?;
        let mut g = 0;
        for (ci, c) in r.cells.iter().enumerate() {
            if g == col {
                // 세로로 합친 셀의 아래쪽(vMerge continue)은 화면에 없는 칸이다 — HWPX와 같게 거른다
                if c.props.vmerge == wordcraft_doc::props::VMerge::Continue {
                    return Err(format!("({row},{col})은 세로로 합쳐진 셀 안쪽입니다 — 합쳐진 셀의 왼쪽 위 좌표를 쓰십시오"));
                }
                return Ok((tb, row, ci));
            }
            g += c.span().max(1);
            if col < g {
                return Err(format!("({row},{col})은 합쳐진 셀 안쪽입니다 — 합쳐진 셀의 왼쪽 위 좌표를 쓰십시오"));
            }
        }
        Err(format!("이 행은 격자 {g}열입니다"))
    }

    fn select(&mut self, a: Value, f: Value) -> Result<(), String> {
        self.wc("select.range", json!({"anchor": a, "focus": f})).map(|_| ())
    }

    fn caret(&mut self, at: Value) -> Result<(), String> {
        self.wc("caret.set", json!({"pos": at})).map(|_| ())
    }

    /// 문단 범위(글자 단위)를 고른다.
    fn select_chars(&mut self, p: &Value) -> Result<(), String> {
        Self::check_section(p)?;
        let i = us(p, "para")?;
        let t = self.para(i)?;
        let (s, e) = (byte_at(&t, us(p, "start")?), byte_at(&t, us(p, "end")?));
        self.select(pos(vec![i], s), pos(vec![i], e))
    }
}

impl Engine for DocxEngine {
    fn format(&self) -> Format {
        Format::Docx
    }

    fn run(&mut self, id: &str, p: &Value) -> Result<Value, String> {
        // 문단 번호 없이 오면 WordCraft 그대로(커서 자리에 쓰기·지금 문단 정렬) — 글상자 안 쓰기 등(M4)
        if matches!(id, "text.insert" | "para.align" | "format.bold" | "format.italic" | "format.underline") && p.get("para").is_none() {
            return self.wc(id, p.clone());
        }
        Ok(match id {
            "doc.info" => json!({
                "format": "docx",
                "blocks": self.doc().body.len(),
                "paragraphs": self.doc().body.iter().filter(|b| b.as_para().is_some()).count(),
                "tables": self.tables().len(),
            }),
            "doc.text" => Value::Array(
                self.doc()
                    .body
                    .iter()
                    .enumerate()
                    .filter_map(|(i, b)| para_text(b).filter(|t| !t.is_empty()).map(|t| json!({"section":0,"para":i,"text":t})))
                    .collect(),
            ),
            "para.text" => {
                Self::check_section(p)?;
                json!({"text": self.para(us(p, "para")?)?})
            }
            "text.insert" | "text.append" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                let t = self.para(i)?;
                let off = if id == "text.append" { t.len() } else { byte_at(&t, us(p, "offset")?) };
                self.caret(pos(vec![i], off))?;
                self.wc("text.insert", json!({"text": st(p, "text")?}))?;
                json!({"ok": true})
            }
            "text.delete" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                let t = self.para(i)?;
                let off = us(p, "offset")?;
                let (s, e) = (byte_at(&t, off), byte_at(&t, off + us(p, "count")?));
                self.select(pos(vec![i], s), pos(vec![i], e))?;
                self.wc("text.delete", json!({}))?;
                json!({"ok": true})
            }
            "para.insert" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                self.para(i)?;
                self.caret(pos(vec![i], 0))?;
                self.wc("text.newParagraph", json!({}))?;
                json!({"para": i})
            }
            "para.append" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                let t = self.para(i)?;
                self.caret(pos(vec![i], t.len()))?;
                self.wc("text.newParagraph", json!({}))?;
                if let Some(text) = p.get("text").and_then(Value::as_str).filter(|s| !s.is_empty()) {
                    self.wc("text.insert", json!({"text": text}))?;
                }
                json!({"para": i + 1})
            }
            "para.delete" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                let t = self.para(i)?;
                let n = self.doc().body.len();
                if i + 1 < n && self.doc().body[i + 1].as_para().is_some() {
                    self.select(pos(vec![i], 0), pos(vec![i + 1], 0))?;
                } else if i > 0 && self.doc().body[i - 1].as_para().is_some() {
                    let prev = self.para(i - 1)?;
                    self.select(pos(vec![i - 1], prev.len()), pos(vec![i], t.len()))?;
                } else {
                    self.select(pos(vec![i], 0), pos(vec![i], t.len()))?;
                }
                self.wc("text.delete", json!({}))?;
                json!({"ok": true})
            }
            "text.find" => {
                let find = st(p, "find")?;
                let mut out = Vec::new();
                for (i, b) in self.doc().body.iter().enumerate() {
                    if let Some(t) = para_text(b) {
                        for off in char_matches(&t, find) {
                            out.push(json!({"section":0,"para":i,"offset":off}));
                        }
                    }
                }
                Value::Array(out)
            }
            "text.replace" => {
                let (find, rep) = (st(p, "find")?.to_string(), st(p, "replace")?.to_string());
                if find.is_empty() {
                    return Err("find는 비어 있으면 안 됩니다".into());
                }
                self.wc("edit.replaceAll", json!({"text": find, "with": rep}))?
            }
            "table.list" => {
                let mut out = Vec::new();
                for (n, tb) in self.tables().into_iter().enumerate() {
                    let t = self.doc().body[tb].as_table().unwrap();
                    out.push(json!({"table": n, "section": 0, "para": tb, "rows": t.rows.len(), "cols": t.grid.len()}));
                }
                Value::Array(out)
            }
            "doc.pageCount" => json!({"pages": self.s.layout().pages.len()}),
            "layout.cells" => {
                // WordCraft 배치의 칸 영역(pt). 쪽에 걸친 칸은 높이를 더하고 처음 놓인 쪽을 낸다. 칸 번호는 격자 열로 바꾼다.
                let t = us(p, "table")?;
                let tb = *self.tables().get(t).ok_or_else(|| format!("본문 최상위 표 {t}번이 없습니다"))?;
                let spans: Vec<Vec<usize>> =
                    self.doc().body[tb].as_table().unwrap().rows.iter().map(|r| r.cells.iter().map(|c| c.span().max(1)).collect()).collect();
                let lay = self.s.layout();
                let mut acc: std::collections::BTreeMap<(usize, usize), (usize, f32, f32, bool)> = Default::default();
                for (pi, page) in lay.pages.iter().enumerate() {
                    for it in &page.items {
                        if let wordcraft_engine::layout::Placed::Cell { rect, covered: false, table, row, cell, story: wordcraft_doc::StoryRef::Body, clip } = it {
                            if table.0.len() == 1 && table.0[0] as usize == tb {
                                let e = acc.entry((*row, *cell)).or_insert((pi, 0.0, rect.w, false));
                                e.1 += rect.h;
                                e.3 |= *clip;
                            }
                        }
                    }
                }
                let mut cells = Vec::new();
                for ((r, ci), (page, h, w, clip)) in acc {
                    let col: usize = spans.get(r).map(|sp| sp.iter().take(ci).sum()).unwrap_or(ci);
                    cells.push(json!({"row": r, "col": col, "page": page, "w": (w * 10.0).round() / 10.0, "h": (h * 10.0).round() / 10.0, "clip": clip}));
                }
                json!({"cells": cells})
            }
            "table.getCell" => {
                let (tb, r, c) = self.cell_at(p)?;
                let t = self.doc().body[tb].as_table().unwrap();
                json!({"text": cell_text(&t.rows[r].cells[c])})
            }
            "table.setCell" => {
                let (tb, r, c) = self.cell_at(p)?;
                let text = st(p, "text")?.to_string();
                let cell = &self.doc().body[tb].as_table().unwrap().rows[r].cells[c];
                let last = cell.blocks.len().saturating_sub(1);
                let last_len = cell.blocks.get(last).and_then(|b| para_text(b)).map(|t| t.len());
                match last_len {
                    // 셀 안 첫 문단 처음부터 마지막 문단 끝까지 골라 새 글자로 바꾼다 — 첫 글자 모양을 이어받는다
                    Some(len) => self.select(pos(vec![tb, r, c, 0], 0), pos(vec![tb, r, c, last], len))?,
                    None => return Err("셀 마지막 블록이 표라서 셀 쓰기를 할 수 없습니다(중첩 표)".into()),
                }
                if text.is_empty() {
                    if cell_text(&self.doc().body[tb].as_table().unwrap().rows[r].cells[c]).is_empty() {
                        return Ok(json!({"ok": true}));
                    }
                    self.wc("text.delete", json!({}))?;
                } else {
                    self.wc("text.insert", json!({"text": text}))?;
                }
                json!({"ok": true})
            }
            "table.insertRow" | "table.deleteRow" | "table.insertColumn" | "table.deleteColumn" => {
                let mut q = p.clone();
                if id.ends_with("Row") {
                    q["col"] = json!(0);
                } else {
                    q["row"] = json!(0);
                }
                let (tb, r, c) = self.cell_at(&q)?;
                self.caret(pos(vec![tb, r, c, 0], 0))?;
                let cmd = match id {
                    "table.insertRow" if b_or(p, "below", true) => "table.insertRowBelow",
                    "table.insertRow" => "table.insertRowAbove",
                    "table.insertColumn" if b_or(p, "right", true) => "table.insertColumnRight",
                    "table.insertColumn" => "table.insertColumnLeft",
                    other => other,
                };
                self.wc(cmd, json!({}))?
            }
            "table.merge" => {
                let t = us(p, "table")?;
                let a = self.cell_at(&json!({"table": t, "row": us(p, "startRow")?, "col": us(p, "startCol")?}))?;
                let b = self.cell_at(&json!({"table": t, "row": us(p, "endRow")?, "col": us(p, "endCol")?}))?;
                self.select(pos(vec![a.0, a.1, a.2, 0], 0), pos(vec![b.0, b.1, b.2, 0], 0))?;
                self.wc("table.merge", json!({}))?
            }
            "format.bold" | "format.italic" | "format.underline" => {
                self.select_chars(p)?;
                self.wc(id, json!({"value": b_or(p, "on", true)}))?
            }
            "para.align" => {
                Self::check_section(p)?;
                let i = us(p, "para")?;
                self.para(i)?;
                self.caret(pos(vec![i], 0))?;
                self.wc("para.align", json!({"value": st(p, "align")?}))?
            }
            // 공통 목록에 없는 명령은 WordCraft 명령 그대로
            other => self.wc(other, p.clone())?,
        })
    }

    fn save(&mut self) -> Result<(Vec<u8>, Value), String> {
        let b1 = wordcraft_engine::io::save_bytes(&self.name, &self.s.doc)?;
        let orig_doc = wordcraft_engine::io::open_bytes(&self.name, &self.original)?;
        let b0 = wordcraft_engine::io::save_bytes(&self.name, &orig_doc)?;
        let (bytes, report) = octodocs_preserve::preserve(&self.original, &b0, &b1).map_err(|e| e.to_string())?;
        Ok((bytes, serde_json::to_value(&report).unwrap_or_else(|_| json!({}))))
    }

    fn original(&self) -> Option<&[u8]> {
        Some(&self.original)
    }

    fn native_commands(&self) -> Vec<(String, String, String)> {
        // WordCraft 명령 목록은 `wordcraft-cli commands` 와 같다 — 이름만 영어로 둔다
        Vec::new()
    }
}
