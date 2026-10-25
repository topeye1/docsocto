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

/// styles.xml 의 cellXfs 순서대로 "날짜 서식인가". 내장 서식 14~22·45~47, 또는 y·d 가 든 사용자 서식.
fn xlsx_date_styles(styles: &str) -> Vec<bool> {
    let mut custom = std::collections::HashMap::new();
    let mut out = Vec::new();
    let (mut r, mut in_xfs) = (Reader::from_str(styles), false);
    loop {
        match r.read_event() {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if e.local_name().as_ref() == "numFmt" => {
                let get = |k: &str| e.attributes().flatten().find(|a| a.key.as_ref() == k).map(|a| a.value.to_string()).unwrap_or_default();
                custom.insert(get("numFmtId"), get("formatCode"));
            }
            Ok(Event::Start(e)) if e.local_name().as_ref() == "cellXfs" => in_xfs = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == "cellXfs" => in_xfs = false,
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) if in_xfs && e.local_name().as_ref() == "xf" => {
                let id = e.attributes().flatten().find(|a| a.key.as_ref() == "numFmtId").map(|a| a.value.to_string()).unwrap_or_default();
                let n: u32 = id.parse().unwrap_or(0);
                let date = (14..=22).contains(&n)
                    || (45..=47).contains(&n)
                    || custom.get(&id).is_some_and(|c: &String| {
                        // 따옴표·대괄호 안 글자는 서식 문자가 아니다
                        let c = c.replace("&quot;", "\"");
                        let mut plain = String::new();
                        let (mut q, mut b) = (false, false);
                        for ch in c.chars() {
                            match ch {
                                '"' => q = !q,
                                '[' => b = true,
                                ']' => b = false,
                                _ if !q && !b => plain.push(ch.to_ascii_lowercase()),
                                _ => {}
                            }
                        }
                        plain.contains('y') || plain.contains('d')
                    });
                out.push(date);
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

/// 엑셀 날짜 일련번호(1900 체계) → "YYYY-MM-DD". 시각이 있으면 " HH:MM"을 붙인다.
fn excel_date(v: f64) -> Option<String> {
    if !(1.0..2_958_466.0).contains(&v) {
        return None;
    }
    // 1900-03-01 이후는 1899-12-30 기준(엑셀의 1900년 윤일 오류 보정)
    let days = v.floor() as i64 + if v < 61.0 { 1 } else { 0 };
    let z = days - 25569 + 719468; // 1970-01-01 기준 일수 → 달력
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    let secs = ((v - v.floor()) * 86400.0).round() as i64;
    Some(if secs > 0 { format!("{y:04}-{m:02}-{d:02} {:02}:{:02}", secs / 3600, secs % 3600 / 60) } else { format!("{y:04}-{m:02}-{d:02}") })
}

fn read_xlsx(data: &[u8]) -> Result<Vec<Value>, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(data)).map_err(|e| e.to_string())?;
    let date_styles = zip_text(&mut z, "xl/styles.xml").map(|x| xlsx_date_styles(&x)).unwrap_or_default();
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
        let mut cell_date = false;
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
                    let st: usize = e.attributes().flatten().find(|a| a.key.as_ref() == "s").and_then(|a| a.value.to_string().parse().ok()).unwrap_or(0);
                    cell_date = date_styles.get(st).copied().unwrap_or(false);
                    val.clear();
                }
                Ok(Event::Start(e)) if matches!(e.local_name().as_ref(), "v" | "t") => in_v = true,
                Ok(Event::End(e)) if matches!(e.local_name().as_ref(), "v" | "t") => in_v = false,
                Ok(Event::Text(t)) if in_v => val.push_str(&t.xml10_content()),
                Ok(Event::End(e)) if e.local_name().as_ref() == "c" => {
                    let v = if cell_t == "s" {
                        val.trim().parse::<usize>().ok().and_then(|i| shared.get(i).cloned()).unwrap_or_default()
                    } else if cell_date && cell_t.is_empty() {
                        val.trim().parse::<f64>().ok().and_then(excel_date).unwrap_or_else(|| val.trim().to_string())
                    } else {
                        val.trim().to_string()
                    };
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

/// 빈칸 묶음 길이들
fn space_runs(t: &str) -> Vec<usize> {
    let mut runs = Vec::new();
    let mut n = 0;
    for ch in t.chars() {
        if ch == ' ' {
            n += 1;
        } else {
            if n > 0 {
                runs.push(n);
            }
            n = 0;
        }
    }
    runs
}

/// 글자 조각마다 빈칸 하나를 끼우고 진짜 띄어쓰기는 빈칸 2개 이상으로 낸 쪽인가
/// (pdf-extract가 글자 묶음 사이마다 빈칸을 넣는 PDF).
fn pdf_padded(t: &str) -> bool {
    let runs: Vec<usize> = t.lines().flat_map(|l| space_runs(l.trim())).collect();
    let wide = runs.iter().filter(|&&n| n >= 2).count();
    runs.len() >= 10 && wide * 10 >= runs.len() * 3
}

/// 빈칸 1개는 지우고 2개 이상은 하나로.
fn pdf_unpad(line: &str) -> String {
    let mut out = String::new();
    let mut n = 0;
    for ch in line.chars() {
        if ch == ' ' {
            n += 1;
            continue;
        }
        if n >= 2 {
            out.push(' ');
        }
        n = 0;
        out.push(ch);
    }
    out
}

fn read_pdf(data: &[u8]) -> Result<Vec<Value>, String> {
    let pages = pdf_extract::extract_text_from_mem_by_pages(data).map_err(|e| format!("PDF 읽기 실패: {e}"))?;
    let mut out = Vec::new();
    for (i, t) in pages.iter().enumerate() {
        let padded = pdf_padded(t);
        for (j, line) in t.lines().map(str::trim).filter(|l| !l.is_empty()).enumerate() {
            let line = if padded { pdf_unpad(line) } else { line.to_string() };
            out.push(part(format!("{}쪽 {}줄", i + 1, j + 1), line));
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
    fn excel_dates() {
        assert_eq!(excel_date(44666.0).as_deref(), Some("2022-04-15"));
        assert_eq!(excel_date(45371.0).as_deref(), Some("2024-03-20"));
        assert_eq!(excel_date(45371.5).as_deref(), Some("2024-03-20 12:00"));
        let styles = r#"<styleSheet><numFmts><numFmt numFmtId="164" formatCode="yyyy-mm-dd"/><numFmt numFmtId="165" formatCode="&quot;d&quot;#,##0"/></numFmts><cellXfs><xf numFmtId="0"/><xf numFmtId="164"/><xf numFmtId="165"/><xf numFmtId="14"/></cellXfs></styleSheet>"#;
        assert_eq!(xlsx_date_styles(styles), vec![false, true, false, true]);
    }

    #[test]
    fn pdf_spacing() {
        let t = "가온정밀   주식회사  2026 년   시설 · 운전자금   사업계획  ( 가상   시험   자료 )\n1.  목적\n반도체   장비   고객사   증설에   맞춰  5 축   가공기  1 대를   들이고";
        assert!(pdf_padded(t));
        assert_eq!(pdf_unpad("가온정밀   주식회사  2026 년   시설 · 운전자금"), "가온정밀 주식회사 2026년 시설·운전자금");
        assert!(!pdf_padded("plain text with single spaces only here and there and more words"));
    }

    #[test]
    fn column_names() {
        assert_eq!(col_name(0), "A");
        assert_eq!(col_name(25), "Z");
        assert_eq!(col_name(26), "AA");
    }
}
