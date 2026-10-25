//! 참고 자료 읽기(M3, 진행일지 A20): 자료 파일을 글자로 뽑고 위치를 붙인다.
//! AI가 숫자를 지어내지 않고 출처(문단·표 행·시트!셀·슬라이드·쪽)를 밝히게 하려는 것(9절).

use std::io::{Cursor, Read};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde_json::{json, Value};

fn part(at: String, text: String) -> Value {
    json!({"at": at, "text": text})
}

fn zip_text(z: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Option<String> {
    let mut f = z.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

/// 문단 단위로 글자를 모은다: `para_tag`(a:p, w:p 등) 끝마다 끊고 `text_tag`(a:t, t) 글자를 잇는다.
fn xml_paragraphs(xml: &str, para_tag: &str, text_tag: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_t = false;
    let mut r = Reader::from_str(xml);
    loop {
        match r.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == text_tag => in_t = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == text_tag => in_t = false,
            Ok(Event::End(e)) if e.local_name().as_ref() == para_tag => {
                let t = cur.trim().to_string();
                if !t.is_empty() {
                    out.push(t);
                }
                cur.clear();
            }
            Ok(Event::Text(t)) if in_t => cur.push_str(&t.xml10_content()),
            Ok(Event::GeneralRef(e)) if in_t => {
                if let Ok(Some(c)) = e.resolve_char_ref() {
                    cur.push(c);
                    continue;
                }
                cur.push_str(match &*e {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "quot" => "\"",
                    "apos" => "'",
                    _ => "",
                });
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

fn read_pptx(data: &[u8]) -> Result<Vec<Value>, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    let mut slides: Vec<(usize, String)> = (0..z.len())
        .filter_map(|i| z.by_index(i).ok().map(|f| f.name().to_string()))
        .filter_map(|n| n.strip_prefix("ppt/slides/slide").and_then(|r| r.strip_suffix(".xml")).and_then(|k| k.parse().ok()).map(|k| (k, n.clone())))
        .collect();
    slides.sort();
    let mut out = Vec::new();
    for (k, name) in slides {
        let xml = zip_text(&mut z, &name).unwrap_or_default();
        for (i, p) in xml_paragraphs(&xml, "p", "t").into_iter().enumerate() {
            out.push(part(format!("슬라이드 {k} 문단 {}", i + 1), p));
        }
    }
    Ok(out)
}

/// 열 번호(0부터) → A, B, …, AA
fn col_name(mut c: usize) -> String {
    let mut s = Vec::new();
    loop {
        s.push((b'A' + (c % 26) as u8) as char);
        if c < 26 {
            break;
        }
        c = c / 26 - 1;
    }
    s.iter().rev().collect()
}

fn read_xlsx(data: &[u8]) -> Result<Vec<Value>, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    // 공유 문자열
    let shared: Vec<String> = zip_text(&mut z, "xl/sharedStrings.xml").map(|x| xml_paragraphs(&x, "si", "t")).unwrap_or_default();
    // 시트 이름과 파일: workbook.xml 의 sheet 순서 = rels 의 rId → 파일
    let wb = zip_text(&mut z, "xl/workbook.xml").unwrap_or_default();
    let rels = zip_text(&mut z, "xl/_rels/workbook.xml.rels").unwrap_or_default();
    let mut rel_target = std::collections::HashMap::new();
    let mut r = Reader::from_str(&rels);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if e.local_name().as_ref() == "Relationship" => {
                let get = |k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k).map(|a| a.value.to_string()).unwrap_or_default();
                rel_target.insert(get("Id"), get("Target"));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut sheets = Vec::new();
    let mut r = Reader::from_str(&wb);
    while let Ok(ev) = r.read_event() {
        match ev {
            Event::Empty(e) | Event::Start(e) if e.local_name().as_ref() == "sheet" => {
                let get = |k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k).map(|a| a.value.to_string()).unwrap_or_default();
                let target = rel_target.get(&get("r:id")).cloned().unwrap_or_default();
                let path = if target.starts_with('/') { target.trim_start_matches('/').to_string() } else { format!("xl/{target}") };
                sheets.push((get("name"), path));
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let mut out = Vec::new();
    for (sname, path) in sheets {
        let xml = zip_text(&mut z, &path).unwrap_or_default();
        let mut r = Reader::from_str(&xml);
        let (mut cell_ref, mut cell_t, mut in_v, mut val) = (String::new(), String::new(), false, String::new());
        let mut row_cells: Vec<String> = Vec::new();
        let mut row_no = String::new();
        loop {
            match r.read_event() {
                Ok(Event::Start(e)) if e.local_name().as_ref() == "row" => {
                    row_cells.clear();
                    row_no = e.attributes().flatten().find(|a| a.key.as_ref() == "r").map(|a| a.value.to_string()).unwrap_or_default();
                }
                Ok(Event::Start(e)) if e.local_name().as_ref() == "c" => {
                    cell_ref = e.attributes().flatten().find(|a| a.key.as_ref() == "r").map(|a| a.value.to_string()).unwrap_or_default();
                    cell_t = e.attributes().flatten().find(|a| a.key.as_ref() == "t").map(|a| a.value.to_string()).unwrap_or_default();
                    val.clear();
                }
                Ok(Event::Start(e)) if matches!(e.local_name().as_ref(), "v" | "t") => in_v = true,
                Ok(Event::End(e)) if matches!(e.local_name().as_ref(), "v" | "t") => in_v = false,
                Ok(Event::Text(t)) if in_v => val.push_str(&t.xml10_content()),
                Ok(Event::End(e)) if e.local_name().as_ref() == "c" => {
                    let v = if cell_t == "s" { val.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default() } else { val.trim().to_string() };
                    if !v.is_empty() {
                        row_cells.push(format!("{cell_ref}={v}"));
                    }
                }
                Ok(Event::End(e)) if e.local_name().as_ref() == "row" => {
                    if !row_cells.is_empty() {
                        out.push(part(format!("{sname}!{row_no}행"), row_cells.join(" | ")));
                    }
                }
                Ok(Event::Eof) | Err(_) => break,
                _ => {}
            }
        }
    }
    let _ = col_name; // 열 이름은 셀 참조(A1)가 이미 담고 있다
    Ok(out)
}

fn read_pdf(data: &[u8]) -> Result<Vec<Value>, String> {
    let pages = pdf_extract::extract_text_from_mem_by_pages(data).map_err(|e| format!("PDF 읽기 실패: {e}"))?;
    let mut out = Vec::new();
    for (i, t) in pages.iter().enumerate() {
        for (j, line) in t.lines().map(str::trim).filter(|l| !l.is_empty()).enumerate() {
            out.push(part(format!("{}쪽 {}줄", i + 1, j + 1), line.to_string()));
        }
    }
    Ok(out)
}

fn read_doc(path: &str, data: Vec<u8>) -> Result<Vec<Value>, String> {
    let mut e = crate::open(path, data)?;
    let mut out = Vec::new();
    for p in e.run("doc.text", &json!({}))?.as_array().cloned().unwrap_or_default() {
        out.push(part(format!("문단 {}", p["para"]), p["text"].as_str().unwrap_or("").to_string()));
    }
    // 표는 행 단위로 " | " 로 잇는다
    for t in e.run("table.list", &json!({})).unwrap_or(json!([])).as_array().cloned().unwrap_or_default() {
        let ti = t["table"].as_u64().unwrap_or(0);
        let (rows, cols) = (t["rows"].as_u64().unwrap_or(0), t["cols"].as_u64().unwrap_or(0));
        if rows * cols > 4000 {
            continue;
        }
        for r in 0..rows {
            let mut cells = Vec::new();
            for c in 0..cols {
                if let Ok(v) = e.run("table.getCell", &json!({"table": ti, "row": r, "col": c})) {
                    let s = v["text"].as_str().unwrap_or("").split_whitespace().collect::<Vec<_>>().join(" ");
                    if !s.is_empty() {
                        cells.push(s);
                    }
                }
            }
            if !cells.is_empty() {
                out.push(part(format!("표 {ti} {}행", r + 1), cells.join(" | ")));
            }
        }
    }
    Ok(out)
}

/// 자료 파일 하나를 읽는다.
pub fn read(path: &str) -> Result<Value, String> {
    let data = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let parts = match ext.as_str() {
        "pptx" => read_pptx(&data)?,
        "xlsx" => read_xlsx(&data)?,
        "pdf" => read_pdf(&data)?,
        "hwpx" | "hwp" | "docx" | "md" | "txt" => read_doc(path, data)?,
        other => return Err(format!("읽을 수 없는 자료 형식: .{other} (hwpx·hwp·docx·md·txt·pptx·xlsx·pdf)")),
    };
    Ok(json!({"path": path, "format": ext, "count": parts.len(), "parts": parts}))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_from_drawingml() {
        let x = r#"<p:sld><a:p><a:r><a:t>매출 </a:t></a:r><a:r><a:t>12억 &amp; 이익</a:t></a:r></a:p><a:p><a:r><a:t>둘째</a:t></a:r></a:p></p:sld>"#;
        assert_eq!(xml_paragraphs(x, "p", "t"), vec!["매출 12억 & 이익", "둘째"]);
    }

    #[test]
    fn column_names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
    }
}
