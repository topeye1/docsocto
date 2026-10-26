//! 블록 목록([`Block`]) → 엔진 명령 목록. docx 는 WordCraft 명령(커서 방식), HWPX 는 ohwp 명령(문단·표 번호 방식).

use serde_json::{json, Value};

use super::css::MM;
use super::flow::{Block, Cell, Para, Run, RunStyle, Table};

pub type Cmds = Vec<(String, Value)>;

pub const FONT: &str = "맑은 고딕";
pub const A4: (f64, f64) = (595.3, 841.9);

/// 쪽 여백(pt): 위·오른쪽·아래·왼쪽
#[derive(Clone, Copy, Debug)]
pub struct Page {
    pub w: f64,
    pub h: f64,
    pub margin: [f64; 4],
}

impl Default for Page {
    fn default() -> Self {
        Page { w: A4.0, h: A4.1, margin: [14.0 * MM, 15.0 * MM, 16.0 * MM, 15.0 * MM] }
    }
}

fn text_of(t: &str, s: &RunStyle) -> String {
    if s.pad {
        format!(" {t} ")
    } else {
        t.to_string()
    }
}

/// 줄 바꿈(None)마다 나눈 [(글자, 서식)] 묶음
fn pieces(runs: &[Run]) -> Vec<Vec<(&str, &RunStyle)>> {
    let mut out = vec![Vec::new()];
    for (t, s) in runs {
        match s {
            None => out.push(Vec::new()),
            Some(s) => out.last_mut().unwrap().push((t.as_str(), s)),
        }
    }
    out
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

// ---------------------------------------------------------------- docx(WordCraft)

pub struct Docx {
    pub cmds: Cmds,
    blk: usize,
    tables: usize,
    prev_table: bool,
}

fn pos(path: &[usize]) -> Value {
    json!({"story": "body", "path": path, "off": 0})
}

impl Docx {
    pub fn new(page: Page) -> Self {
        let mut d = Docx { cmds: Vec::new(), blk: 0, tables: 0, prev_table: false };
        let [top, right, bottom, left] = page.margin;
        if (page.w - A4.0).abs() < 2.0 && (page.h - A4.1).abs() < 2.0 {
            d.c("layout.size", json!({"name": "A4"}));
        } else {
            d.c("layout.size", json!({"width": r2(page.w), "height": r2(page.h)}));
        }
        d.c("layout.margins", json!({"top": top, "bottom": bottom, "left": left, "right": right}));
        d.c("caret.set", json!({"pos": pos(&[0])}));
        d
    }

    fn c(&mut self, cmd: &str, p: Value) {
        self.cmds.push((cmd.to_string(), p));
    }

    fn para_fmt(&mut self, p: &Para, before: f64, in_cell: bool) {
        self.c("para.align", json!({"value": p.align}));
        if let Some(l) = p.line {
            self.c("para.lineSpacing", json!({"exactly": l}));
        }
        self.c("para.spacing", json!({"before": r1(before), "after": 0}));
        self.c("para.indents", json!({"left": p.pad_left, "right": 0, "firstLine": 0}));
        // 한글·숫자/영문 사이 자동 띄우기 끄기 — Edge 에는 없는 빈칸이 생겼다("8 가지")
        self.c("para.asianTypography", json!({"autoSpaceDE": false, "autoSpaceDN": false}));
        if let Some((col, w)) = &p.bar {
            self.c("para.borders", json!({"sides": {"left": {"style": "single", "width": w, "color": col}}}));
        } else if !in_cell {
            self.c("para.borders", json!({"kind": "none"}));
        }
    }

    fn runs(&mut self, runs: &[Run]) {
        for (t, s) in runs {
            let Some(s) = s else {
                self.c("text.lineBreak", json!({}));
                continue;
            };
            self.c("format.font", json!({"name": FONT, "eastAsia": FONT}));
            self.c("format.size", json!({"size": s.size}));
            self.c("format.color", json!({"color": s.color}));
            self.c("format.bold", json!({"value": s.bold}));
            self.c("format.italic", json!({"value": s.italic}));
            self.c("format.underline", json!({"value": s.underline}));
            self.c("format.shading", json!({"color": s.shade}));
            self.c("text.insert", json!({"text": text_of(t, s)}));
        }
    }

    /// 표 둘이 붙으면 Word 가 한 표로 합친다 — 그 사이 간격을 빈 문단 하나로
    fn gap(&mut self, pt: f64) {
        self.c("para.lineSpacing", json!({"exactly": r1(pt).max(1.0)}));
        self.c("para.spacing", json!({"before": 0, "after": 0}));
        self.c("para.borders", json!({"kind": "none"}));
        self.c("format.size", json!({"size": 1}));
        self.c("text.newParagraph", json!({}));
        self.blk += 1;
    }

    pub fn block(&mut self, bl: &Block, page_break: bool) {
        match bl {
            Block::Para(p) => {
                if page_break {
                    self.c("para.pageBreakBefore", json!({"value": true}));
                }
                self.para_fmt(p, p.before, false);
                self.runs(&p.runs);
                self.c("text.newParagraph", json!({}));
                self.c("para.pageBreakBefore", json!({"value": false}));
                self.blk += 1;
                self.prev_table = false;
            }
            Block::Table(t) => {
                if page_break {
                    self.c("para.pageBreakBefore", json!({"value": true}));
                    self.gap(t.before.max(1.0));
                    self.c("para.pageBreakBefore", json!({"value": false}));
                } else if self.prev_table || t.before > 1.0 {
                    self.gap(t.before);
                }
                self.table(t);
                self.prev_table = true;
            }
            Block::Break => {}
        }
    }

    fn table(&mut self, t: &Table) {
        let b = self.blk;
        let cell0 = |r: usize, c: usize| pos(&[b, r, c, 0]);
        self.c("insert.table", json!({"rows": t.rows, "cols": t.cols}));
        self.c("caret.set", json!({"pos": cell0(0, 0)}));
        match &t.border {
            Some((col, w)) => {
                if !t.inner {
                    self.c("table.borders", json!({"kind": "none"}));
                }
                self.c("table.borders", json!({"kind": if t.inner { "all" } else { "outside" }, "width": w, "color": col}));
            }
            None => self.c("table.borders", json!({"kind": "none"})),
        }
        let pad = t.cells.first().map(|c| c.pad).unwrap_or_default();
        self.c("table.cellMargins", json!({"top": pad[0], "right": pad[1], "bottom": pad[2], "left": pad[3]}));
        for (c, w) in t.widths.iter().enumerate() {
            if let Some(w) = w {
                self.c("caret.set", json!({"pos": cell0(0, c)}));
                self.c("table.columnWidth", json!({"width": r2(*w)}));
            }
        }
        // 가로 합침 — 행마다 오른쪽부터 합쳐야 앞 칸 번호가 그대로다(세로 합침은 아직 없음)
        let mut idx = vec![(0usize, 0usize); t.cells.len()];
        for r in 0..t.rows {
            let mut spans: Vec<usize> = (0..t.cells.len()).filter(|&i| t.cells[i].at.0 == r).collect();
            spans.sort_by_key(|&i| t.cells[i].at.1);
            for &i in spans.iter().rev() {
                let (c0, n) = (t.cells[i].at.1, t.cells[i].span.1);
                if n > 1 {
                    // 공통 명령(표 번호·칸 좌표) — octodocs 가 WordCraft 합치기로 옮긴다
                    self.c("table.merge", json!({"table": self.tables, "startRow": r, "startCol": c0, "endRow": r, "endCol": c0 + n - 1}));
                }
            }
            for (k, &i) in spans.iter().enumerate() {
                idx[i] = (r, k);
            }
        }
        for (i, cl) in t.cells.iter().enumerate() {
            let (r, k) = idx[i];
            self.c("caret.set", json!({"pos": cell0(r, k)}));
            self.c("table.shading", json!({"color": cl.bg}));
            self.c("table.properties", json!({"valign": cl.valign}));
            self.cell_paras(cl);
        }
        self.blk = b + 1;
        self.tables += 1;
        self.c("caret.set", json!({"pos": pos(&[self.blk])}));
    }

    fn cell_paras(&mut self, cl: &Cell) {
        for (k, p) in cl.paras.iter().enumerate() {
            if k > 0 {
                self.c("text.newParagraph", json!({}));
            }
            let gap = if k == 0 { 0.0 } else { ((p.top - cl.paras[k - 1].bottom) * super::css::PX).max(0.0) };
            self.para_fmt(p, gap, true);
            self.runs(&p.runs);
        }
    }
}

// ---------------------------------------------------------------- HWPX(ohwp)

/// 한글 선 굵기 표(mm)
const HWP_W: [f64; 16] = [0.1, 0.12, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0];
/// pt → HWPUNIT
const HU: f64 = 100.0;
/// 한글이 HWPX 문단 위 간격을 엔진 값의 두 배로 그렸다(본문 2쪽 실측) — rhwp 단위 확인은 할 일
const SPACE_X: f64 = 0.5;
/// 한글이 HWPX 고정 줄 간격을 엔진 값의 절반으로 읽는다(표지 실측) — rhwp 단위 확인은 할 일
pub const FIXED_X: f64 = 2.0;

pub fn hwp_line(col: &str, w_pt: f64) -> Value {
    let mm = w_pt / MM;
    let i = (0..HWP_W.len()).min_by(|&a, &b| (HWP_W[a] - mm).abs().partial_cmp(&(HWP_W[b] - mm).abs()).unwrap()).unwrap();
    json!({"type": 1, "width": i, "color": format!("#{col}")})
}

fn no_line() -> Value {
    json!({"type": 0, "width": 0, "color": "#000000"})
}

fn hu(v: f64) -> i64 {
    (v * HU).round() as i64
}

pub fn char_props(s: &RunStyle) -> Value {
    // useFontSpace: 꺼져 있으면 한글은 빈칸을 글자 크기의 절반으로 그려 Edge·Word 보다 줄이 넓어졌다
    let mut p = json!({"fontName": FONT, "fontSize": (s.size * 100.0).round() as i64, "textColor": format!("#{}", s.color),
        "bold": s.bold, "italic": s.italic, "underline": s.underline, "useFontSpace": true});
    if let Some(sh) = &s.shade {
        p["shadeColor"] = json!(format!("#{sh}"));
    }
    p
}

pub struct Hwpx {
    pub cmds: Cmds,
    /// 다음 블록을 쓸 빈 문단
    cur: usize,
    tables: usize,
}

impl Hwpx {
    pub fn new(page: Page) -> Self {
        let mut h = Hwpx { cmds: Vec::new(), cur: 0, tables: 0 };
        let [top, right, bottom, left] = page.margin;
        h.c("page.setup", json!({"width": page.w, "height": page.h, "top": top, "bottom": bottom, "left": left, "right": right}));
        h
    }

    fn c(&mut self, cmd: &str, p: Value) {
        self.cmds.push((cmd.to_string(), p));
    }

    /// 다 채운 뒤 쪽 설정을 다시 걸어 엔진이 전체를 다시 조판하게 한다(표 높이·줄 자리 갱신)
    pub fn finish(&mut self) {
        let first = self.cmds[0].clone();
        self.cmds.push(first);
    }

    fn para_props(p: &Para, before: f64) -> Value {
        let mut q = json!({"alignment": p.align, "spacingBefore": (before * HU * SPACE_X).round() as i64, "spacingAfter": 0,
            "marginLeft": hu(p.pad_left), "indent": 0, "autoSpaceKrEn": false, "autoSpaceKrNum": false});
        if let Some(l) = p.line {
            q["lineSpacingType"] = json!("Fixed");
            q["lineSpacing"] = json!((l * HU * FIXED_X).round() as i64);
        }
        q
    }

    pub fn block(&mut self, bl: &Block, page_break: bool) {
        match bl {
            Block::Para(p) if p.bar.is_some() => {
                let t = Self::bar_table(p);
                self.block(&Block::Table(t), page_break)
            }
            Block::Para(p) => {
                for (i, runs) in pieces(&p.runs).iter().enumerate() {
                    if i > 0 {
                        self.c("para.append", json!({"section": 0, "para": self.cur, "text": ""}));
                        self.cur += 1;
                    }
                    let mut off = 0;
                    for (t, s) in runs {
                        let t = text_of(t, s);
                        let n = t.chars().count();
                        self.c("text.insert", json!({"section": 0, "para": self.cur, "offset": off, "text": t}));
                        self.c("format.char", json!({"section": 0, "para": self.cur, "start": off, "end": off + n, "props": char_props(s)}));
                        off += n;
                    }
                    let mut q = Self::para_props(p, if i == 0 { p.before } else { 0.0 });
                    q["pageBreakBefore"] = json!(page_break && i == 0);
                    self.c("format.para", json!({"section": 0, "para": self.cur, "props": q}));
                }
                self.c("para.append", json!({"section": 0, "para": self.cur, "text": ""}));
                self.cur += 1;
            }
            Block::Table(t) => {
                self.c("format.para", json!({"section": 0, "para": self.cur, "props": {
                    "spacingBefore": (t.before * HU * SPACE_X).round() as i64, "spacingAfter": 0, "pageBreakBefore": page_break,
                    "marginLeft": 0, "indent": 0, "alignment": "left", "lineSpacingType": "Percent", "lineSpacing": 100}}));
                self.table(t);
            }
            Block::Break => {}
        }
    }

    /// 왼쪽 막대 제목은 왼쪽 테두리만 있는 1칸 표로
    fn bar_table(p: &Para) -> Table {
        let (col, w) = p.bar.clone().unwrap();
        let mut inner = p.clone();
        inner.bar = None;
        inner.pad_left = 0.0;
        let cell = Cell {
            paras: vec![inner],
            bg: None,
            valign: "center",
            pad: [0.0, 0.0, 0.0, p.pad_left - w],
            at: (0, 0),
            span: (1, 1),
            h: (p.bottom - p.top) * super::css::PX,
            lines: vec![("left", (col, w))],
        };
        Table { rows: 1, cols: 1, widths: vec![None], cells: vec![cell], border: None, inner: false, top: p.top, bottom: p.bottom, before: p.before }
    }

    fn table(&mut self, t: &Table) {
        let ti = self.tables;
        self.tables += 1;
        let cur = self.cur;
        self.c("table.create", json!({"section": 0, "para": cur, "offset": 0, "rows": t.rows, "cols": t.cols}));
        if t.widths.iter().all(Option::is_some) {
            let w: Vec<i64> = t.widths.iter().map(|w| hu(w.unwrap())).collect();
            self.c("table.columnWidths", json!({"table": ti, "widths": w}));
        }
        // 글자처럼 취급 — 한글은 떠 있는 표의 자리를 저장된 표 높이로 잡아, 내용이 늘면 아래 글과 겹쳤다
        self.c("table.props", json!({"table": ti, "props": {"paddingLeft": 0, "paddingRight": 0, "paddingTop": 0, "paddingBottom": 0, "cellSpacing": 0, "treatAsChar": true}}));
        for cl in &t.cells {
            let ((r, c), (rs, cs)) = (cl.at, cl.span);
            if rs > 1 || cs > 1 {
                self.c("table.merge", json!({"table": ti, "startRow": r, "startCol": c, "endRow": r + rs - 1, "endCol": c + cs - 1}));
            }
        }
        // 행 높이 — 엔진이 칸 높이를 여백만큼(약 1mm)으로 저장해 한글이 표 아래 글을 표 위에 겹쳐 놓았다.
        // Edge 가 그린 행 높이를 칸 높이로 넣는다(한글은 내용이 더 길면 늘린다).
        let mut row_h = vec![0.0f64; t.rows];
        for cl in &t.cells {
            if cl.span.0 == 1 && cl.at.0 < t.rows {
                row_h[cl.at.0] = row_h[cl.at.0].max(cl.h);
            }
        }
        for cl in &t.cells {
            let (r, c) = cl.at;
            let height: f64 = row_h[r..(r + cl.span.0).min(t.rows)].iter().sum();
            let line = t.border.as_ref().map(|(col, w)| hwp_line(col, *w)).unwrap_or_else(no_line);
            let mut sides: Vec<(&str, Value)> = ["Left", "Right", "Top", "Bottom"].iter().map(|k| (*k, line.clone())).collect();
            if t.border.is_some() && !t.inner {
                // 바깥 테두리만 — 안쪽 경계는 없앤다
                for (k, v) in sides.iter_mut() {
                    let inner = match *k {
                        "Top" => r > 0,
                        "Bottom" => r + cl.span.0 < t.rows,
                        "Left" => c > 0,
                        _ => c + cl.span.1 < t.cols,
                    };
                    if inner {
                        *v = no_line();
                    }
                }
            }
            for (k, (col, w)) in &cl.lines {
                let name = match *k {
                    "left" => "Left",
                    "right" => "Right",
                    "top" => "Top",
                    _ => "Bottom",
                };
                for (kk, v) in sides.iter_mut() {
                    if *kk == name {
                        *v = hwp_line(col, *w);
                    }
                }
            }
            let mut props = serde_json::Map::new();
            for (k, v) in sides {
                props.insert(format!("border{k}"), v);
            }
            props.insert("fillType".into(), json!(if cl.bg.is_some() { "solid" } else { "none" }));
            props.insert("fillColor".into(), json!(format!("#{}", cl.bg.as_deref().unwrap_or("FFFFFF"))));
            let [pt, pr, pb, pl] = cl.pad;
            props.insert("paddingTop".into(), json!(hu(pt)));
            props.insert("paddingRight".into(), json!(hu(pr)));
            props.insert("paddingBottom".into(), json!(hu(pb)));
            props.insert("paddingLeft".into(), json!(hu(pl)));
            props.insert("applyInnerMargin".into(), json!(true));
            props.insert("verticalAlign".into(), json!(match cl.valign {
                "center" => 1,
                "bottom" => 2,
                _ => 0,
            }));
            if height > 0.0 {
                props.insert("height".into(), json!(hu(height)));
            }
            self.c("table.cellProps", json!({"table": ti, "row": r, "col": c, "props": props}));
            let mut k = 0usize;
            let mut last_len = 0usize;
            for (pi, p) in cl.paras.iter().enumerate() {
                for (j, runs) in pieces(&p.runs).iter().enumerate() {
                    if k > 0 {
                        self.c("table.cellSplitPara", json!({"table": ti, "row": r, "col": c, "para": k - 1, "offset": last_len}));
                    }
                    let mut off = 0;
                    for (txt, s) in runs {
                        let txt = text_of(txt, s);
                        let n = txt.chars().count();
                        self.c("table.cellInsertText", json!({"table": ti, "row": r, "col": c, "para": k, "offset": off, "text": txt}));
                        self.c("table.cellCharFormat", json!({"table": ti, "row": r, "col": c, "para": k, "start": off, "end": off + n, "props": char_props(s)}));
                        off += n;
                    }
                    last_len = off;
                    let gap = if pi == 0 || j > 0 { 0.0 } else { ((p.top - cl.paras[pi - 1].bottom) * super::css::PX).max(0.0) };
                    self.c("table.cellParaFormat", json!({"table": ti, "row": r, "col": c, "para": k, "props": Self::para_props(p, gap)}));
                    k += 1;
                }
            }
        }
        self.cur += 1;
    }
}

/// 블록 목록 → 명령 목록
pub fn emit(blocks: &[Block], hwpx: bool, page: Page) -> Cmds {
    let mut brk = false;
    if hwpx {
        let mut g = Hwpx::new(page);
        for bl in blocks {
            if matches!(bl, Block::Break) {
                brk = true;
                continue;
            }
            g.block(bl, brk);
            brk = false;
        }
        g.finish();
        g.cmds
    } else {
        let mut g = Docx::new(page);
        for bl in blocks {
            if matches!(bl, Block::Break) {
                brk = true;
                continue;
            }
            g.block(bl, brk);
            brk = false;
        }
        g.cmds
    }
}
