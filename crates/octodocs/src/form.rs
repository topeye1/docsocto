//! 양식 채우기(M3, 기획서 3절 기능 7): 칸 뽑기(form.slots) · 채우기(form.fill).
//! 공통 명령(table.list·table.getCell·table.setCell·field.*) 위에 만들어 HWPX·docx에서 똑같이 돈다.
//! 답을 쓰는 것은 MCP로 붙은 AI다(진행일지 A17) — 여기서는 칸과 그 뜻(라벨·표 제목)만 뽑는다.

use serde_json::{json, Value};

use crate::Engine;

/// 칸 하나가 "입력 칸"인지: 비었거나, 안내문(※·(예시)·(공란))만 있거나, 단위·괄호·빈칸만 있는 틀이다.
pub fn is_input(text: &str) -> Option<&'static str> {
    let t = text.trim();
    if t.is_empty() {
        return Some("empty");
    }
    if t.starts_with('※') || t.starts_with("(예시") || t.starts_with("예시)") || t == "(공란)" || t.starts_with("* 작성") {
        return Some("guide");
    }
    // 단위·괄호·구두점·빈칸만 남는 틀(예: "     명 (남   ,여    )", "             원")
    const UNITS: &[&str] = &["백만원", "천원", "만원", "원", "명", "년", "월", "일", "남", "여", "개", "건", "회", "%", "㎡", "평", "세"];
    let mut rest = t.to_string();
    for u in UNITS {
        rest = rest.replace(u, "");
    }
    let rest: String = rest.chars().filter(|c| !c.is_whitespace() && !"()[]{},.:;~/-·~（）".contains(*c)).collect();
    // 빈칸이 넓게 있어야 틀이다(라벨 "(원)" 같은 것과 가르려고)
    if rest.is_empty() && text.contains("   ") {
        return Some("template");
    }
    None
}

/// 라벨이 될 수 있는 칸: 입력 칸이 아니고 글자(한글·영문)가 있다(～·: 같은 기호 칸은 아님).
fn is_label(s: &str) -> bool {
    is_input(s).is_none() && s.chars().any(|c| c.is_alphabetic())
}

/// 라벨 다듬기: 원문자 번호·줄바꿈·겹빈칸을 걷는다.
fn clean_label(s: &str) -> String {
    let s: String = s.chars().filter(|c| !('\u{2460}'..='\u{2473}').contains(c)).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 칸 글자에서 '□ 보기' 묶음을 뽑는다.
fn choices(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split(['□', '■']).skip(1) {
        let opt: String = part.split(['\n']).next().unwrap_or("").trim().to_string();
        let opt = opt.trim_end_matches([',', '/', '·']).trim().to_string();
        if !opt.is_empty() {
            out.push(opt);
        }
    }
    out
}

struct Grid {
    rows: usize,
    cols: usize,
    /// (row, col) → 그 좌표에서 시작하는 셀의 글자(합쳐진 셀 안쪽은 None)
    cells: Vec<Option<String>>,
}

impl Grid {
    fn at(&self, r: usize, c: usize) -> Option<&String> {
        self.cells.get(r * self.cols + c).and_then(|x| x.as_ref())
    }
}

fn grid(e: &mut dyn Engine, table: usize, rows: usize, cols: usize) -> Grid {
    let mut cells = vec![None; rows * cols];
    for r in 0..rows {
        for c in 0..cols {
            if let Ok(v) = e.run("table.getCell", &json!({"table": table, "row": r, "col": c})) {
                cells[r * cols + c] = Some(v["text"].as_str().unwrap_or("").to_string());
            }
        }
    }
    Grid { rows, cols, cells }
}

fn caption(e: &mut dyn Engine, section: u64, para: u64) -> String {
    // 표가 든 문단 앞쪽으로 올라가며 글자가 있는 첫 문단
    let mut p = para as i64 - 1;
    while p >= 0 && para as i64 - p <= 3 {
        if let Ok(v) = e.run("para.text", &json!({"section": section, "para": p})) {
            let t = clean_label(v["text"].as_str().unwrap_or(""));
            if !t.is_empty() {
                return t;
            }
        }
        p -= 1;
    }
    String::new()
}

/// form.slots: 채울 칸 목록.
pub fn slots(e: &mut dyn Engine) -> Result<Value, String> {
    let mut out = Vec::new();
    let tables = e.run("table.list", &json!({}))?;
    for t in tables.as_array().cloned().unwrap_or_default() {
        let ti = t["table"].as_u64().unwrap_or(0) as usize;
        let (rows, cols) = (t["rows"].as_u64().unwrap_or(0) as usize, t["cols"].as_u64().unwrap_or(0) as usize);
        if rows * cols > 4000 {
            continue; // 지나치게 큰 표(본문 전체가 표인 문서 등)는 건너뛴다
        }
        let g = grid(e, ti, rows, cols);
        let cap = caption(e, t["section"].as_u64().unwrap_or(0), t["para"].as_u64().unwrap_or(0));
        let title = g.at(0, 0).filter(|_| (1..cols).all(|c| g.at(0, c).is_none())).map(|s| clean_label(s)).unwrap_or_default();
        for r in 0..g.rows {
            for c in 0..g.cols {
                let Some(text) = g.at(r, c) else { continue };
                // 고르기 칸
                let opts = choices(text);
                if opts.len() >= 2 {
                    let label = (0..c).rev().find_map(|cc| g.at(r, cc).filter(|s| is_input(s).is_none() && choices(s).len() < 2)).map(|s| clean_label(s)).unwrap_or_default();
                    out.push(json!({
                        "id": format!("t{ti}r{r}c{c}"), "kind": "choice", "table": ti, "row": r, "col": c,
                        "label": label, "options": opts, "current": text, "table_title": if title.is_empty() { &cap } else { &title },
                    }));
                    continue;
                }
                let Some(kind) = is_input(text) else { continue };
                // 라벨: 같은 행 왼쪽의 글자 칸 → 없으면 같은 열 위쪽
                let left = (0..c).rev().find_map(|cc| g.at(r, cc).filter(|s| is_label(s)).map(|s| clean_label(s)));
                let up = (0..r).rev().find_map(|rr| g.at(rr, c).filter(|s| is_label(s)).map(|s| clean_label(s)));
                let (label, header) = match (&left, &up) {
                    (Some(l), Some(u)) if r > 0 && *u != title => (l.clone(), u.clone()),
                    (Some(l), _) => (l.clone(), String::new()),
                    (None, Some(u)) => (u.clone(), String::new()),
                    (None, None) => continue, // 라벨 없는 빈칸은 장식 칸으로 본다
                };
                // 표 제목 칸 옆·아래의 빈칸은 여백 칸이다
                if kind == "empty" && (label == title || label == cap) {
                    continue;
                }
                let mut s = json!({
                    "id": format!("t{ti}r{r}c{c}"), "kind": kind, "table": ti, "row": r, "col": c,
                    "label": label, "table_title": if title.is_empty() { &cap } else { &title },
                });
                if !header.is_empty() {
                    s["column"] = json!(header);
                }
                if kind != "empty" {
                    s["template"] = json!(text);
                }
                out.push(s);
            }
        }
    }
    // 누름틀
    if let Ok(fl) = e.run("field.list", &json!({})) {
        if let Some(arr) = fl.as_array().or_else(|| fl.get("fields").and_then(Value::as_array)) {
            for f in arr {
                let name = f.get("name").and_then(Value::as_str).unwrap_or("");
                if !name.is_empty() {
                    out.push(json!({
                        "id": format!("f:{name}"), "kind": "field", "label": name,
                        "guide": f.get("guide").or_else(|| f.get("instruction")).cloned().unwrap_or(Value::Null),
                        "current": f.get("value").cloned().unwrap_or(Value::Null),
                    }));
                }
            }
        }
    }
    Ok(json!({"count": out.len(), "slots": out}))
}

/// "t3r2c5" → (3, 2, 5)
fn parse_cell_id(id: &str) -> Option<(usize, usize, usize)> {
    let id = id.split(':').next()?;
    let rest = id.strip_prefix('t')?;
    let (t, rest) = rest.split_once('r')?;
    let (r, c) = rest.split_once('c')?;
    Some((t.parse().ok()?, r.parse().ok()?, c.parse().ok()?))
}

/// form.fill: {answers:[{id, value}]}. 고르기 칸은 value(보기 글자)의 □를 ■로 바꾼다.
pub fn fill(e: &mut dyn Engine, p: &Value) -> Result<Value, String> {
    let answers = p.get("answers").and_then(Value::as_array).ok_or("매개변수 'answers'([{id, value}])가 필요합니다")?;
    let (mut ok, mut failed) = (0, Vec::new());
    for a in answers {
        let id = a.get("id").and_then(Value::as_str).unwrap_or("");
        let value = a.get("value").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())).unwrap_or_default();
        let r = if let Some(name) = id.strip_prefix("f:") {
            e.run("field.set", &json!({"name": name, "value": value})).map(|_| ())
        } else if let Some((t, r, c)) = parse_cell_id(id) {
            let kind = a.get("kind").and_then(Value::as_str).unwrap_or("");
            if kind == "choice" || a.get("choose").is_some() {
                let pick = a.get("choose").and_then(Value::as_str).unwrap_or(&value).to_string();
                let cur = e.run("table.getCell", &json!({"table": t, "row": r, "col": c}))?;
                let cur = cur["text"].as_str().unwrap_or("").to_string();
                match cur.find(&format!("□{pick}")).or_else(|| cur.find(&format!("□ {pick}"))) {
                    Some(i) => {
                        let mut new = cur.clone();
                        new.replace_range(i..i + '□'.len_utf8(), "■");
                        e.run("table.setCell", &json!({"table": t, "row": r, "col": c, "text": new})).map(|_| ())
                    }
                    None => Err(format!("보기 '{pick}'을 찾지 못했습니다")),
                }
            } else {
                e.run("table.setCell", &json!({"table": t, "row": r, "col": c, "text": value})).map(|_| ())
            }
        } else {
            Err(format!("모르는 칸 id: {id}"))
        };
        match r {
            Ok(()) => ok += 1,
            Err(err) => failed.push(json!({"id": id, "error": err})),
        }
    }
    Ok(json!({"filled": ok, "failed": failed}))
}

/// 표마다 행 높이(행 안 칸 높이 중 가장 큰 것)와 그 행이 놓인 쪽.
fn row_heights(e: &mut dyn Engine) -> Vec<Vec<(f64, u64, bool)>> {
    let tables = e.run("table.list", &json!({})).ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    let mut out = Vec::new();
    for t in tables {
        let ti = t["table"].as_u64().unwrap_or(0);
        let rows = t["rows"].as_u64().unwrap_or(0) as usize;
        let mut h = vec![(0.0f64, u64::MAX, false); rows];
        if let Ok(v) = e.run("layout.cells", &json!({"table": ti})) {
            for c in v["cells"].as_array().cloned().unwrap_or_default() {
                let r = c["row"].as_u64().unwrap_or(u64::MAX) as usize;
                if let Some(slot) = h.get_mut(r) {
                    slot.0 = slot.0.max(c["h"].as_f64().unwrap_or(0.0));
                    slot.1 = slot.1.min(c["page"].as_u64().unwrap_or(u64::MAX));
                    slot.2 |= c["clip"].as_bool().unwrap_or(false);
                }
            }
        }
        out.push(h);
    }
    out
}

/// 쪽마다 본문 아래로 넘친 최대 길이(px). 이 명령이 없는 엔진(docx)은 비어 있다.
fn overflows(e: &mut dyn Engine) -> std::collections::BTreeMap<u64, f64> {
    let mut m = std::collections::BTreeMap::new();
    if let Ok(v) = e.run("layout.overflows", &json!({})) {
        for o in v["overflows"].as_array().cloned().unwrap_or_default() {
            let (pg, over) = (o["page"].as_u64().unwrap_or(0), o["over"].as_f64().unwrap_or(0.0));
            let s = m.entry(pg).or_insert(0.0f64);
            *s = s.max(over);
        }
    }
    m
}

fn pages(e: &mut dyn Engine) -> u64 {
    e.run("doc.pageCount", &json!({})).ok().and_then(|v| v["pages"].as_u64()).unwrap_or(0)
}

/// form.check: 채운 뒤 배치를 원본 배치와 견준다(진행일지 A24 — 넘침은 렌더한 쪽에서 잰다).
/// - 늘어난 행: 원본보다 높아진 표 행(글자가 칸 너비를 넘어 줄이 늘어난 것)
/// - 쪽 수 변화, 쪽 아래 넘침(page_spill — 한글에서 쪽이 늘 자리), 남은 빈칸(form.slots에 아직 나오는 칸)
/// - out 을 주면 HWPX는 문제가 난 쪽(없으면 1쪽)을 SVG로 낸다
pub fn check(e: &mut dyn Engine, p: &Value) -> Result<Value, String> {
    let fmt = e.format();
    let orig = e.original().ok_or("이 형식은 배치 확인을 하지 않습니다(HWPX·docx만)")?.to_vec();
    let mut before = crate::open(&format!("원본.{}", fmt.name()), orig)?;
    let (rows0, pages0, over0) = (row_heights(before.as_mut()), pages(before.as_mut()), overflows(before.as_mut()));
    drop(before);
    let (rows1, pages1, over1) = (row_heights(e), pages(e), overflows(e));
    let mut grown = Vec::new();
    let mut bad_pages = std::collections::BTreeSet::new();
    for (ti, (a, b)) in rows0.iter().zip(&rows1).enumerate() {
        if a.len() != b.len() {
            continue; // 행을 넣거나 지운 표는 견주지 않는다
        }
        for (r, (x, y)) in a.iter().zip(b).enumerate() {
            // 높아졌거나, 높이가 고정된 행이라 글자가 잘리게 됐으면 넘침
            if (y.0 > x.0 + 0.5 && x.0 > 0.0) || (y.2 && !x.2) {
                let cells: Vec<String> = (0..64)
                    .filter_map(|c| e.run("table.getCell", &json!({"table": ti, "row": r, "col": c})).ok().map(|v| v["text"].as_str().unwrap_or("").to_string()))
                    .filter(|s| !s.trim().is_empty())
                    .collect();
                grown.push(json!({"table": ti, "row": r, "before": x.0, "after": y.0, "clipped": y.2 && !x.2, "page": y.1 + 1, "texts": cells}));
                bad_pages.insert(y.1);
            }
        }
    }
    // 본문 아래로 새로 넘친 쪽: 엔진은 쪽 수를 그대로 두지만 한글은 넘친 것을 다음 쪽으로 보낸다(M3-5 실측 7→8쪽)
    let mut spill = Vec::new();
    for (pg, over) in &over1 {
        if *over > over0.get(pg).copied().unwrap_or(0.0) + 1.0 {
            spill.push(json!({"page": pg + 1, "over_px": over}));
            bad_pages.insert(*pg);
        }
    }
    let left: Vec<Value> = slots(e)?["slots"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|s| matches!(s["kind"].as_str(), Some("empty" | "guide" | "template")))
        .map(|s| json!({"id": s["id"], "label": s["label"], "kind": s["kind"]}))
        .collect();
    let mut pictures = Vec::new();
    if let Some(dir) = p.get("out").and_then(Value::as_str) {
        if matches!(fmt, crate::Format::Hwpx | crate::Format::Hwp) {
            std::fs::create_dir_all(dir).map_err(|e| format!("{dir}: {e}"))?;
            if bad_pages.is_empty() {
                bad_pages.insert(0);
            }
            for pg in bad_pages.iter().filter(|p| **p != u64::MAX).take(10) {
                let path = std::path::Path::new(dir).join(format!("쪽{}.svg", pg + 1)).to_string_lossy().to_string();
                e.run("render.page", &json!({"page": pg, "path": path}))?;
                pictures.push(json!(path));
            }
        }
    }
    Ok(json!({
        "pages": {"before": pages0, "after": pages1},
        "grown_rows": grown,
        "page_spill": spill,
        "unfilled": left,
        "pictures": pictures,
        "ok": grown.is_empty() && spill.is_empty() && left.is_empty() && pages0 == pages1,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_cells() {
        assert_eq!(is_input(""), Some("empty"));
        assert_eq!(is_input("  \n "), Some("empty"));
        assert_eq!(is_input("     명 (남   ,여    )"), Some("template"));
        assert_eq!(is_input("             원"), Some("template"));
        assert_eq!(is_input("\n(             )"), Some("template"));
        assert_eq!(is_input("※ 최근 3년 실적 기재"), Some("guide"));
        assert_eq!(is_input("③ 기업체명"), None);
        assert_eq!(is_input("(원)"), None);
    }

    #[test]
    fn choice_options() {
        assert_eq!(choices("□ 제조업 □ 서비스업\n□ 기타"), vec!["제조업", "서비스업", "기타"]);
    }

    #[test]
    fn cell_ids() {
        assert_eq!(parse_cell_id("t3r12c5"), Some((3, 12, 5)));
        assert_eq!(parse_cell_id("t0r1c2:□예"), Some((0, 1, 2)));
        assert_eq!(parse_cell_id("f:성명"), None);
    }

    #[test]
    fn labels_are_cleaned() {
        assert_eq!(clean_label("① 대표자성명\n(공동사업자)"), "대표자성명 (공동사업자)");
    }
}
