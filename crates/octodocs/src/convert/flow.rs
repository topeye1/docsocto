//! 흐름 방식(M4-H5, 결정 #33): Edge 덤프 → 문단·표 블록. 글이 흘러 쪽을 넘긴다 — 여러 쪽 본문용.
//!   제목·문단 → 문단, `<table>` → 표(칸 색·테두리·합침), 가로로 늘어선 블록(flex·다단) → 테두리 표,
//!   바탕·테두리 있는 상자 → 1칸 표, 인라인 꼬리표(바탕 있는 inline-block) → 글자 음영.
//! 블록 목록([`Block`])은 docx·HWPX 출력부(emit)가 함께 쓰는 중간 형태다.

use serde_json::Value;

use super::css::*;
use super::Report;

/// 글자 조각 서식
#[derive(Clone, Debug, PartialEq)]
pub struct RunStyle {
    pub size: f64,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: String,
    pub shade: Option<String>,
    /// 꼬리표 — 음영 글자 앞뒤에 빈칸 하나씩
    pub pad: bool,
}

/// (글자, 서식) — 서식이 None 이면 줄 바꿈
pub type Run = (String, Option<RunStyle>);

#[derive(Clone, Debug)]
pub struct Para {
    pub runs: Vec<Run>,
    pub align: &'static str,
    /// 고정 줄 간격(pt)
    pub line: Option<f64>,
    pub size: f64,
    /// 왼쪽 막대(색, pt)
    pub bar: Option<(String, f64)>,
    pub pad_left: f64,
    pub top: f64,
    pub bottom: f64,
    /// 앞 블록과의 간격(pt)
    pub before: f64,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub paras: Vec<Para>,
    pub bg: Option<String>,
    pub valign: &'static str,
    /// 위·오른쪽·아래·왼쪽 안쪽 여백(pt)
    pub pad: [f64; 4],
    pub at: (usize, usize),
    pub span: (usize, usize),
    /// Edge 가 그린 칸 높이(pt)
    pub h: f64,
    /// 이 칸만의 테두리 면(면 이름, (색, pt))
    pub lines: Vec<(&'static str, (String, f64))>,
}

#[derive(Clone, Debug)]
pub struct Table {
    pub rows: usize,
    pub cols: usize,
    /// 열 너비(pt) — None 이면 쪽 폭에 맡긴다
    pub widths: Vec<Option<f64>>,
    pub cells: Vec<Cell>,
    pub border: Option<(String, f64)>,
    /// 안쪽 경계선도 그리는가
    pub inner: bool,
    pub top: f64,
    pub bottom: f64,
    pub before: f64,
}

#[derive(Clone, Debug)]
pub enum Block {
    Para(Para),
    Table(Table),
    Break,
}

// ---------------------------------------------------------------- 덤프 읽기

/// 옮기지 않는 요소(그림·그래픽·영상) — 보고서에 적는다
const MEDIA: &[&str] = &["img", "svg", "canvas", "video", "audio", "iframe", "object", "embed", "picture"];

fn elems(n: &Value) -> Vec<&Value> {
    kids(n).iter().filter(|k| tag(k) != "#text").collect()
}

fn is_inline(n: &Value) -> bool {
    matches!(sv(n, "display"), "inline" | "inline-block") && !matches!(tag(n), "table" | "tr" | "td" | "th")
}

fn has_text(n: &Value) -> bool {
    kids(n).iter().any(|k| tag(k) == "#text") || n.get("before").is_some()
}

fn round_half(v: f64) -> f64 {
    (v * 2.0).round() / 2.0
}

fn run_style(n: &Value, shade: Option<String>) -> RunStyle {
    let w: u32 = sv(n, "font-weight").parse().unwrap_or(400);
    RunStyle {
        size: round_half(spx(n, "font-size") * PX),
        bold: w >= 600,
        italic: sv(n, "font-style") == "italic",
        underline: sv(n, "text-decoration-line").contains("underline"),
        color: rgba(sv(n, "color")).map(|c| c.0).unwrap_or_else(|| "000000".into()),
        shade,
        pad: false,
    }
}

/// 바탕 있는 꼬리표(inline-block)에 좌우 안쪽 여백이 있으면 음영 글자 앞뒤에 빈칸 — 꼬리표 폭을 비슷하게
fn tag_pad(n: &Value) -> bool {
    bg(n).is_some() && spx(n, "padding-left") >= 3.0
}

fn text_of(k: &Value) -> String {
    k["lines"].as_array().map(|ls| ls.iter().filter_map(|l| l["text"].as_str()).collect()).unwrap_or_default()
}

fn padded(mut r: Vec<Run>, on: bool) -> Vec<Run> {
    if on {
        for (_, s) in r.iter_mut() {
            if let Some(s) = s {
                s.pad = true;
            }
        }
    }
    r
}

/// 인라인 내용 → [(글자, 서식)] — 줄은 Edge 가 나눈 것이므로 이어 붙여 다시 흘린다
fn runs_of(n: &Value, shade: Option<String>, rep: &mut Report) -> Vec<Run> {
    let mut out = Vec::new();
    if let Some(b) = n.get("before").and_then(Value::as_str) {
        out.push((b.to_string(), Some(run_style(n, shade.clone()))));
    }
    for k in kids(n) {
        match tag(k) {
            "#text" => {
                let t = text_of(k);
                if !t.is_empty() {
                    out.push((t, Some(run_style(n, shade.clone()))));
                }
            }
            "br" => out.push(("\n".into(), None)),
            t if MEDIA.contains(&t) => rep.skip(t),
            _ if is_inline(k) => {
                let r = runs_of(k, bg(k).or_else(|| shade.clone()), rep);
                out.extend(padded(r, tag_pad(k)));
            }
            t => rep.skip(t),
        }
    }
    out
}

/// HTML 공백 규칙: 연속 공백 하나로, 문단 앞뒤 공백 없앰
fn tidy(runs: Vec<Run>) -> Vec<Run> {
    let mut out: Vec<Run> = Vec::new();
    for (t, s) in runs {
        if s.is_none() {
            out.push((t, s));
            continue;
        }
        let mut t: String = collapse_ws(&t);
        if let Some((pt, Some(_))) = out.last() {
            if pt.ends_with(' ') && t.starts_with(' ') {
                t.remove(0);
            }
        }
        if !t.is_empty() {
            out.push((t, s));
        }
    }
    while let Some((t, Some(_))) = out.first() {
        if !t.starts_with(' ') {
            break;
        }
        let t2 = t.trim_start().to_string();
        if t2.is_empty() {
            out.remove(0);
        } else {
            out[0].0 = t2;
        }
    }
    while let Some((t, Some(_))) = out.last() {
        if !t.ends_with(' ') {
            break;
        }
        let t2 = t.trim_end().to_string();
        let i = out.len() - 1;
        if t2.is_empty() {
            out.pop();
        } else {
            out[i].0 = t2;
        }
    }
    out
}

fn collapse_ws(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    let mut sp = false;
    for c in t.chars() {
        if c.is_whitespace() {
            if !sp {
                s.push(' ');
            }
            sp = true;
        } else {
            s.push(c);
            sp = false;
        }
    }
    s
}

// ---------------------------------------------------------------- 블록 만들기

fn align_of(n: &Value) -> &'static str {
    match sv(n, "text-align") {
        "center" => "center",
        "right" | "end" => "right",
        "justify" => "justify",
        _ => "left",
    }
}

fn para(n: &Value, runs: Vec<Run>) -> Para {
    let b = bx(n);
    let lh = sv(n, "line-height");
    let bar = if box_border(n).is_none() && bg(n).is_none() { side(n, "left") } else { None };
    Para {
        runs,
        align: align_of(n),
        line: lh.ends_with("px").then(|| px(lh) * PX),
        size: round_half(spx(n, "font-size") * PX),
        pad_left: if bar.is_some() { spx(n, "padding-left") * PX } else { 0.0 },
        bar,
        top: b.y,
        bottom: b.y + b.h,
        before: 0.0,
    }
}

fn page_break_after(n: &Value) -> bool {
    matches!(sv(n, "break-after"), "page" | "always") || matches!(sv(n, "page-break-after"), "page" | "always")
}

/// 블록 n 의 내용 → [문단|표|쪽나눔]. nested 면(칸 안) 표·격자를 만들지 않고 문단으로만 편다.
pub fn flow(n: &Value, nested: bool, rep: &mut Report) -> Vec<Block> {
    if !nested {
        if tag(n) == "table" {
            return vec![Block::Table(html_table(n, rep))];
        }
        if let Some(g) = grid(n, rep) {
            return vec![Block::Table(g)];
        }
        if (bg(n).is_some() || box_border(n).is_some()) && (has_text(n) || !elems(n).is_empty()) && !matches!(tag(n), "body" | "section") {
            return vec![Block::Table(box_table(n, rep))];
        }
    }
    note_style(n, rep);
    let mut out = Vec::new();
    let mut cur: Vec<Run> = Vec::new();
    let flush = |cur: &mut Vec<Run>, out: &mut Vec<Block>| {
        let r = tidy(std::mem::take(cur));
        if r.iter().any(|(t, s)| s.is_some() && !t.trim().is_empty()) {
            out.push(Block::Para(para(n, r)));
        }
    };
    if let Some(b) = n.get("before").and_then(Value::as_str) {
        cur.push((b.to_string(), Some(run_style(n, None))));
    }
    for k in kids(n) {
        match tag(k) {
            "#text" => {
                let t = text_of(k);
                if !t.is_empty() {
                    cur.push((t, Some(run_style(n, None))));
                }
            }
            "br" => cur.push(("\n".into(), None)),
            t if MEDIA.contains(&t) => rep.skip(t),
            _ if is_inline(k) => {
                let r = runs_of(k, bg(k), rep);
                cur.extend(padded(r, tag_pad(k)));
            }
            _ => {
                flush(&mut cur, &mut out);
                out.extend(flow(k, nested, rep));
            }
        }
    }
    flush(&mut cur, &mut out);
    // 글자만 든 블록은 문단 자리를 블록 상자로(첫 줄 위 여백 포함)
    if out.len() == 1 && elems(n).is_empty() {
        if let Block::Para(p) = &mut out[0] {
            let b = bx(n);
            p.top = b.y;
            p.bottom = b.y + b.h;
        }
    }
    if !nested && page_break_after(n) && !out.is_empty() {
        out.push(Block::Break);
    }
    out
}

/// 옮기지 못하거나 바꿔 그린 스타일을 보고서에 적는다
fn note_style(n: &Value, rep: &mut Report) {
    if gradient(sv(n, "background-image")).is_some() {
        rep.note("gradient_dropped", "그러데이션 바탕(흐름 방식) — 옮기지 않음");
    }
    if spx(n, "border-top-left-radius") > 0.5 && (bg(n).is_some() || box_border(n).is_some()) {
        rep.note("radius_square", "둥근 모서리 — 네모로 그림");
    }
    let op: f64 = sv(n, "opacity").parse().unwrap_or(1.0);
    if op < 0.999 {
        rep.note("opacity_dropped", "요소 불투명도(opacity) — 옮기지 않음");
    }
}

fn cell_paras(n: &Value, rep: &mut Report) -> Vec<Para> {
    flow(n, true, rep).into_iter().filter_map(|b| if let Block::Para(p) = b { Some(p) } else { None }).collect()
}

fn cell_of(n: &Value, fill: Option<String>, rep: &mut Report) -> Cell {
    if spx(n, "border-top-left-radius") > 0.5 && (bg(n).is_some() || box_border(n).is_some()) {
        rep.note("radius_square", "둥근 모서리 — 네모로 그림");
    }
    let rs = n.get("rowspan").and_then(Value::as_u64).unwrap_or(1) as usize;
    let cs = n.get("colspan").and_then(Value::as_u64).unwrap_or(1) as usize;
    Cell {
        paras: cell_paras(n, rep),
        bg: bg(n).or(fill),
        valign: match sv(n, "vertical-align") {
            "middle" => "center",
            "bottom" => "bottom",
            _ => "top",
        },
        pad: pads(n),
        at: (0, 0),
        span: (rs, cs),
        h: bx(n).h * PX,
        lines: Vec::new(),
    }
}

fn bounds(mut xs: Vec<f64>, tol: f64) -> Vec<f64> {
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut out: Vec<f64> = Vec::new();
    for x in xs {
        if out.last().map_or(true, |l| x - l > tol) {
            out.push(x);
        }
    }
    out
}

fn col_at(bs: &[f64], x: f64) -> usize {
    (0..bs.len()).min_by(|&a, &b| (bs[a] - x).abs().partial_cmp(&(bs[b] - x).abs()).unwrap()).unwrap_or(0)
}

fn html_table(n: &Value, rep: &mut Report) -> Table {
    let mut rows: Vec<&Value> = Vec::new();
    for g in elems(n) {
        if tag(g) == "tr" {
            rows.push(g);
        } else {
            rows.extend(elems(g).into_iter().filter(|r| tag(r) == "tr"));
        }
    }
    let cells: Vec<(usize, &Value)> =
        rows.iter().enumerate().flat_map(|(ri, r)| elems(r).into_iter().filter(|c| matches!(tag(c), "td" | "th")).map(move |c| (ri, c))).collect();
    let bs = bounds(cells.iter().flat_map(|(_, c)| [bx(c).x, bx(c).x + bx(c).w]).collect(), 1.5);
    let mut out = Vec::new();
    for (ri, c) in &cells {
        let b = bx(c);
        let (c0, c1) = (col_at(&bs, b.x), col_at(&bs, b.x + b.w));
        let mut cell = cell_of(c, None, rep);
        cell.at = (*ri, c0);
        cell.span = (cell.span.0, (c1.saturating_sub(c0)).max(1));
        out.push(cell);
    }
    if out.iter().any(|c| c.span.0 > 1) {
        rep.note("rowspan", "세로 합친 칸 — HWPX만 합침, docx는 나눠 둠");
    }
    // 테두리는 본문 칸(td) 것을 표 전체에 — Word 쪽이 표 단위로만 받는다
    let td = cells.iter().find(|(_, c)| tag(c) == "td").or(cells.first()).map(|(_, c)| *c);
    let b = bx(n);
    Table {
        rows: rows.len(),
        cols: bs.len().saturating_sub(1).max(1),
        widths: bs.windows(2).map(|w| Some((w[1] - w[0]) * PX)).collect(),
        cells: out,
        border: td.and_then(box_border),
        inner: true,
        top: b.y,
        bottom: b.y + b.h,
        before: 0.0,
    }
}

/// 가로로 늘어선 블록(flex 한 줄, 다단 목록)을 표로. 아니면 None.
fn grid(n: &Value, rep: &mut Report) -> Option<Table> {
    let flex = sv(n, "display") == "flex";
    let mut ks: Vec<&Value> = elems(n).into_iter().filter(|k| !is_inline(k) || flex).collect();
    if ks.len() < 2 || matches!(tag(n), "table" | "tr") {
        return None;
    }
    let xs = bounds(ks.iter().map(|k| bx(k).x).collect(), 2.0);
    if xs.len() < 2 {
        return None;
    }
    let b = bx(n);
    let fill = bg(n);
    let k0b = bx(ks[0]);
    let one_row = ks.iter().all(|k| bx(k).y < k0b.y + k0b.h);
    let (rows, edges, mut cells, border);
    if one_row {
        ks.sort_by(|p, q| bx(p).x.partial_cmp(&bx(q).x).unwrap());
        // 칸 경계는 이웃 블록 사이 빈 자리의 가운데 — Word·한글 글자가 Edge 보다 조금 넓어도 접히지 않게
        let mut e = vec![b.x];
        for w in ks.windows(2) {
            let (p, q) = (bx(w[0]), bx(w[1]));
            e.push((p.x + p.w + q.x) / 2.0);
        }
        e.push(b.x + b.w);
        edges = e;
        let stacks: Vec<Vec<&Value>> = ks.iter().map(|k| elems(k)).collect();
        let k0 = stacks[0].len();
        // 카드처럼 테두리 상자마다 같은 수의 블록이 위아래로 쌓였으면 그 수만큼 행으로 나눈다
        if k0 >= 2 && stacks.iter().all(|s| s.len() == k0) && ks.iter().all(|k| box_border(k).is_some()) {
            if ks.iter().any(|k| spx(k, "border-top-left-radius") > 0.5) {
                rep.note("radius_square", "둥근 모서리 — 네모로 그림");
            }
            let mut cs = Vec::new();
            for (c, s) in stacks.iter().enumerate() {
                for (r, item) in s.iter().enumerate() {
                    let mut cell = cell_of(item, None, rep);
                    cell.at = (r, c);
                    cell.span = (1, 1);
                    cs.push(cell);
                }
            }
            cells = cs;
            rows = k0;
            border = box_border(ks[0]);
            if ks.len() > 1 && bx(ks[1]).x - (bx(ks[0]).x + bx(ks[0]).w) > 3.0 {
                rep.note("card_gap", "카드 사이 빈 틈 — 붙은 표 칸으로 그림");
            }
        } else {
            let mut cs = Vec::new();
            for (c, k) in ks.iter().enumerate() {
                let kb = bx(k);
                let mut cell = cell_of(k, fill.clone(), rep);
                // 바탕 있는 글자 조각(꼬리표)은 칸 색이 아니라 글자 음영으로
                if bg(k).is_some() && elems(k).is_empty() {
                    cell.bg = fill.clone();
                    let pad = tag_pad(k);
                    for p in cell.paras.iter_mut() {
                        for (_, s) in p.runs.iter_mut() {
                            if let Some(s) = s {
                                s.shade = bg(k);
                                s.pad = pad;
                            }
                        }
                    }
                }
                // flex 줄 가운데에 놓인 조각(align-items:center)은 칸 세로 가운데로
                if (kb.y + kb.h / 2.0 - (b.y + b.h / 2.0)).abs() < 2.0 && kb.h < b.h - 4.0 {
                    cell.valign = "center";
                }
                cell.at = (0, c);
                cell.span = (1, 1);
                cell.h = b.h * PX;
                if c == ks.len() - 1 && flex && kb.x + kb.w > b.x + b.w - 20.0 {
                    for p in cell.paras.iter_mut() {
                        p.align = "right"; // margin-left:auto 로 오른쪽 끝에 붙은 조각
                    }
                }
                cs.push(cell);
            }
            cells = cs;
            rows = 1;
            border = box_border(n);
        }
        let pad = pads(n);
        for cl in cells.iter_mut() {
            if fill.is_some() && cl.pad.iter().all(|v| *v == 0.0) {
                cl.pad = [pad[0], 2.0, pad[2], 2.0];
            }
        }
    } else {
        let cols: Vec<Vec<&Value>> = xs
            .iter()
            .map(|x| {
                let mut c: Vec<&Value> = ks.iter().copied().filter(|k| (bx(k).x - x).abs() <= 2.0).collect();
                c.sort_by(|p, q| bx(p).y.partial_cmp(&bx(q).y).unwrap());
                c
            })
            .collect();
        rows = cols.iter().map(Vec::len).max().unwrap_or(1);
        let mut e = vec![b.x];
        e.extend_from_slice(&xs[1..]);
        e.push(b.x + b.w);
        edges = e;
        let mut cs = Vec::new();
        for (c, col) in cols.iter().enumerate() {
            for (r, k) in col.iter().enumerate() {
                let mut cell = cell_of(k, None, rep);
                cell.at = (r, c);
                cell.span = (1, 1);
                cs.push(cell);
            }
        }
        cells = cs;
        border = None;
    }
    if spx(n, "border-top-left-radius") > 0.5 && (fill.is_some() || border.is_some()) {
        rep.note("radius_square", "둥근 모서리 — 네모로 그림");
    }
    let inner = border.is_some() && (!one_row || rows > 1);
    Some(Table {
        rows,
        cols: edges.len() - 1,
        widths: edges.windows(2).map(|w| Some((w[1] - w[0]) * PX)).collect(),
        cells,
        border,
        inner,
        top: b.y,
        bottom: b.y + b.h,
        before: 0.0,
    })
}

fn box_table(n: &Value, rep: &mut Report) -> Table {
    let b = bx(n);
    note_style(n, rep);
    let mut cell = cell_of(n, None, rep);
    cell.at = (0, 0);
    cell.span = (1, 1);
    Table { rows: 1, cols: 1, widths: vec![Some(b.w * PX)], cells: vec![cell], border: box_border(n), inner: false, top: b.y, bottom: b.y + b.h, before: 0.0 }
}

/// 블록 사이 빈 자리(Edge 상자 간격) → 앞 간격(pt). 쪽 나눔 뒤 첫 블록은 0.
pub fn spacing(blocks: &mut [Block]) {
    let mut prev: Option<f64> = None;
    for bl in blocks.iter_mut() {
        let (top, bottom, before) = match bl {
            Block::Break => {
                prev = None;
                continue;
            }
            Block::Para(p) => (p.top, p.bottom, &mut p.before),
            Block::Table(t) => (t.top, t.bottom, &mut t.before),
        };
        *before = prev.map_or(0.0, |pv| ((top - pv) * PX).max(0.0));
        prev = Some(bottom);
    }
}

/// 덤프 전체 → 블록 목록(앞 간격까지)
pub fn blocks(root: &Value, rep: &mut Report) -> Vec<Block> {
    let mut b = flow(root, false, rep);
    while matches!(b.last(), Some(Block::Break)) {
        b.pop(); // 마지막 쪽 뒤 나눔은 빈 쪽만 만든다
    }
    spacing(&mut b);
    b
}
