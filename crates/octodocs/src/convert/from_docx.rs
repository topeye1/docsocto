//! docx → 블록 목록(M4 docx↔HWPX 변환). WordCraft 문서 모형을 스타일까지 풀어 읽는다.

use std::path::Path;

use wordcraft_doc::para::{InlineObject, OBJ, PAGE_BREAK};
use wordcraft_doc::props::{Border, Borders, LineSpacing, Underline, VAlign, VMerge};
use wordcraft_doc::table::Table as WTable;
use wordcraft_doc::{Align, Block as WBlock, Document, Paragraph, TextColor};

use super::emit::Page;
use super::flow::{Block, Cell, Image, Para, Run, RunStyle, Table};
use super::Report;

fn hex(c: wordcraft_doc::Rgb) -> String {
    format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)
}

struct Reader<'a> {
    doc: &'a Document,
    tmp: &'a Path,
    rep: &'a mut Report,
    pics: usize,
}

impl Reader<'_> {
    fn runs(&self, p: &Paragraph) -> Vec<Run> {
        let style = p.props.style.as_deref();
        let mut out: Vec<Run> = Vec::new();
        let mut off = 0usize;
        for r in &p.runs {
            let text = p.text.get(off..off + r.len).unwrap_or("");
            off += r.len;
            let rc = self.doc.styles.resolve_char(style, &r.props);
            if rc.hidden {
                continue;
            }
            let st = RunStyle {
                size: rc.size as f64,
                bold: rc.bold,
                italic: rc.italic,
                underline: !matches!(rc.underline, Underline::None),
                strike: rc.strike,
                color: match rc.color {
                    TextColor::Rgb(c) => hex(c),
                    TextColor::Auto => "000000".into(),
                },
                shade: rc.shading.or(rc.highlight).map(hex),
                font: Some(rc.font_ea.clone().unwrap_or_else(|| rc.font.clone())),
                font_latin: Some(rc.font.clone()),
                spacing: if rc.size > 0.0 { (rc.spacing / rc.size * 100.0) as f64 } else { 0.0 },
                ratio: rc.scale as f64,
                ..RunStyle::plain(10.0)
            };
            for ch in text.chars() {
                match ch {
                    '\n' => out.push(("\n".into(), None)),
                    OBJ | PAGE_BREAK => {}
                    '\u{000B}' | '\u{000E}' => out.push(("\n".into(), None)),
                    c => match out.last_mut() {
                        Some((t, Some(s))) if *s == st => t.push(c),
                        _ => out.push((c.to_string(), Some(st.clone()))),
                    },
                }
            }
        }
        out
    }

    fn para(&mut self, p: &Paragraph) -> Para {
        let rp = self.doc.styles.resolve_para(&p.props);
        let runs = self.runs(p);
        let mut q = Para::new(runs);
        q.size = q.runs.iter().find_map(|(_, s)| s.as_ref().map(|s| s.size)).unwrap_or_else(|| self.doc.styles.resolve_char(p.props.style.as_deref(), &p.mark).size as f64);
        q.align = match rp.align {
            Align::Center => "center",
            Align::Right => "right",
            Align::Justify | Align::Distribute => "justify",
            _ => "left",
        };
        q.pad_left = rp.indent_left as f64;
        q.indent_right = rp.indent_right as f64;
        q.first_line = rp.indent_first as f64;
        q.before = rp.space_before as f64;
        q.after = rp.space_after as f64;
        match rp.line_spacing {
            LineSpacing::Exactly(v) => q.line = Some(v as f64),
            LineSpacing::AtLeast(v) => {
                q.line = Some(v as f64);
                q.line_min = true;
            }
            // Word 의 "배수"는 글꼴 줄 높이(올림+내림+줄 틈) 기준이고 한글 비율은 글자 크기 기준이라 —
            // 글꼴 줄 높이를 재어 고정 간격으로 옮긴다(빈 문단 높이도 이것으로 맞는다)
            LineSpacing::Multiple(m) => {
                let mark = self.doc.styles.resolve_char(p.props.style.as_deref(), &p.mark);
                let (latin, ea) = q
                    .runs
                    .iter()
                    .find_map(|(_, s)| s.as_ref().map(|s| (s.font_latin.clone().unwrap_or_default(), s.font.clone().unwrap_or_default())))
                    .unwrap_or_else(|| (mark.font.clone(), mark.font_ea.clone().unwrap_or_else(|| mark.font.clone())));
                // 한글·한자가 든 줄(빈 문단 포함)만 동아시아 글꼴 높이를 따른다
                let has_ea = q.runs.iter().all(|(t, _)| t.is_empty()) || q.runs.iter().any(|(t, _)| t.chars().any(is_cjk_char));
                let k = line_ratio(&latin, false).max(if has_ea { line_ratio(&ea, true) } else { 0.0 });
                q.line = Some(m as f64 * k * q.size);
            }
        }
        if rp.numbering.is_some() {
            self.rep.note("numbering", "번호·글머리표 매기기 — 번호 글자 없이 옮김");
        }
        if rp.borders.is_some_and(|b| b.any_visible()) || rp.shading.is_some() {
            self.rep.note("para_border", "문단 테두리·배경 — 옮기지 않음");
        }
        q
    }

    fn images(&mut self, p: &Paragraph, align: &'static str) -> Vec<Image> {
        let mut out = Vec::new();
        for o in &p.objects {
            match o {
                InlineObject::Image { media, w, h, float, .. } => {
                    let Some(bytes) = self.doc.media.get(media) else { continue };
                    let ext = Path::new(media).extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
                    if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp") {
                        self.rep.skip("그림(읽지 못한 형식)");
                        continue;
                    }
                    self.pics += 1;
                    let path = self.tmp.join(format!("docx_pic{}.{ext}", self.pics));
                    if std::fs::write(&path, bytes.as_slice()).is_err() {
                        continue;
                    }
                    if float.wrap != wordcraft_doc::para::Wrap::Inline {
                        self.rep.note("float_picture", "떠 있는 그림 — 글자처럼 취급하는 그림으로 넣음");
                    }
                    out.push(Image { src: path.to_string_lossy().into_owned(), w: *w as f64, h: *h as f64, align, top: 0.0, bottom: 0.0, before: 0.0 });
                }
                InlineObject::Shape { .. } | InlineObject::Group { .. } => self.rep.skip("도형·글상자"),
                InlineObject::Graphic { .. } => self.rep.skip("차트·그래픽"),
                _ => self.rep.skip("그 밖의 개체"),
            }
        }
        out
    }

    fn table(&mut self, t: &WTable) -> Table {
        let ncols = t.cols().max(1);
        let style = t.props.style.as_deref().and_then(|id| self.doc.styles.table_style(id));
        let tb: Borders = t.props.borders.or(style.as_ref().and_then(|s| s.parts.borders)).unwrap_or_default();
        let def_margins = t.props.cell_margins.or(style.as_ref().and_then(|s| s.parts.cell_margins)).unwrap_or([0.0, 5.4, 0.0, 5.4]);
        let fill = t.props.shading.or(style.as_ref().and_then(|s| s.parts.fill));
        let widths: Vec<Option<f64>> = (0..ncols).map(|i| Some(t.grid.get(i).copied().unwrap_or(72.0) as f64)).collect();
        let nrows = t.rows.len();
        let mut cells = Vec::new();
        for (ri, row) in t.rows.iter().enumerate() {
            let mut g = row.first_grid();
            for (ci, cell) in row.cells.iter().enumerate() {
                let span = cell.span();
                if cell.props.vmerge == VMerge::Continue {
                    g += span;
                    continue;
                }
                let rows_spanned = t.vertical_extent(ri, ci).map(|(_, count)| count).unwrap_or(1).max(1);
                let side = |own: Option<Border>, outer: bool, tbl_outer: Option<Border>, tbl_inner: Option<Border>| -> (String, f64) {
                    match own.or(if outer { tbl_outer } else { tbl_inner }) {
                        Some(b) if b.is_visible() => (b.color.map(hex).unwrap_or_else(|| "000000".into()), b.width as f64),
                        _ => (String::new(), 0.0),
                    }
                };
                let cb = cell.props.borders.unwrap_or_default();
                let last_row = ri + rows_spanned >= nrows;
                let last_col = g + span >= ncols;
                let lines = vec![
                    ("top", side(cb.top, ri == 0, tb.top, tb.between)),
                    ("bottom", side(cb.bottom, last_row, tb.bottom, tb.between)),
                    ("left", side(cb.left, g == 0, tb.left, tb.inside_v)),
                    ("right", side(cb.right, last_col, tb.right, tb.inside_v)),
                ];
                let m = cell.props.resolved_margins(def_margins);
                let mut paras = Vec::new();
                for b in &cell.blocks {
                    match &**b {
                        WBlock::Para(p) => paras.push(self.para(p)),
                        WBlock::Table(_) => self.rep.note("nested_table", "칸 안의 표 — 옮기지 않음"),
                    }
                }
                // 칸 높이 어림: 줄 수 × 줄 높이 + 위아래 여백(한글은 내용이 더 길면 늘린다)
                let inner_w = (widths[g..(g + span).min(ncols)].iter().map(|w| w.unwrap_or(0.0)).sum::<f64>() - m[1] as f64 - m[3] as f64).max(10.0);
                let est: f64 = paras.iter().map(|p| estimate_height(p, inner_w)).sum::<f64>() + m[0] as f64 + m[2] as f64;
                let row_h = row.props.height.map(|h| h as f64).unwrap_or(0.0);
                cells.push(Cell {
                    paras,
                    bg: cell.props.shading.or(fill).map(hex),
                    valign: match cell.props.valign {
                        Some(VAlign::Center) => "center",
                        Some(VAlign::Bottom) => "bottom",
                        _ => "top",
                    },
                    pad: [m[0] as f64, m[3] as f64, m[2] as f64, m[1] as f64],
                    at: (ri, g),
                    span: (rows_spanned, span),
                    h: if rows_spanned == 1 { est.max(row_h) } else { 0.0 },
                    lines,
                    tables: Vec::new(),
                });
                g += span;
            }
        }
        Table { rows: nrows, cols: ncols, widths, cells, border: None, inner: true, top: 0.0, bottom: 0.0, before: 0.0 }
    }

    fn blocks(&mut self, bs: &[std::sync::Arc<WBlock>], out: &mut Vec<Block>) {
        for b in bs {
            match &**b {
                WBlock::Para(p) => {
                    let rp = self.doc.styles.resolve_para(&p.props);
                    if rp.page_break_before && !out.is_empty() {
                        out.push(Block::Break);
                    }
                    let q = self.para(p);
                    let has_text = q.runs.iter().any(|(t, s)| s.is_some() && !t.is_empty());
                    let imgs = self.images(p, q.align);
                    if has_text || imgs.is_empty() {
                        out.push(Block::Para(q.clone()));
                    }
                    for mut im in imgs {
                        im.before = if has_text { 0.0 } else { q.before };
                        out.push(Block::Image(im));
                    }
                    if p.text.contains(PAGE_BREAK) {
                        out.push(Block::Break);
                    }
                }
                WBlock::Table(t) => out.push(Block::Table(self.table(t))),
            }
        }
    }
}

fn is_cjk_char(c: char) -> bool {
    matches!(c as u32, 0x1100..=0x11FF | 0x2E80..=0x9FFF | 0xA960..=0xA97F | 0xAC00..=0xD7FF | 0xF900..=0xFAFF | 0xFF01..=0xFF60)
}

/// 글꼴 한 줄 높이 / 글자 크기 (Word 의 한 줄 간격). `ea` — 동아시아 글자에 쓰는 글꼴로 잴 때.
/// Word 는 한·중·일 글꼴(OS/2 코드 페이지)의 한 줄을 윈도 높이(올림+내림)의 1.3배로 잡는다 —
/// 맑은 고딕 1.33 → 1.73, HY신명조 1.0 → 1.30(03·02 학위논문 양식 Word PDF 실측).
fn line_ratio(family: &str, ea: bool) -> f64 {
    use skrifa::raw::TableProvider;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static CACHE: OnceLock<Mutex<HashMap<(String, bool), f64>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&(family.to_string(), ea)) {
        return *v;
    }
    // 없는 글꼴은 Word 가 대신 쓰는 글꼴로 잰다 — 한글 이름이면 맑은 고딕, 아니면 Times New Roman
    // (KoPub 바탕체가 없는 PC 에서 Word 는 맑은 고딕으로 그렸다 — 03 학위논문 양식 실측)
    let db = wordcraft_fonts::FontDb::global();
    let used = if db.has_family(family) {
        family
    } else if family.is_ascii() {
        "Times New Roman"
    } else {
        "Malgun Gothic"
    };
    let face = db.face(used, "Regular");
    let os2 = face.skrifa().and_then(|f| f.os2().ok().map(|o| (o.ul_code_page_range_1().unwrap_or(0), o.us_win_ascent() as f64 + o.us_win_descent() as f64)));
    let k = match os2 {
        // 코드 페이지 17~21: 일본어·중국어 간체·한국어 완성형·중국어 번체·한국어 조합형
        Some((cp, win)) if ea && cp & (0x1F << 17) != 0 && win > 0.0 && face.upem > 0.0 => (1.3 * win / face.upem).clamp(0.9, 2.6),
        _ => {
            let (a, d) = wordcraft_fonts::word::line_metrics(&face);
            if face.upem > 0.0 { ((a + d) / face.upem).clamp(0.9, 2.0) } else { 1.2 }
        }
    };
    cache.lock().unwrap().insert((family.to_string(), ea), k);
    k
}

/// 문단 높이 어림(pt) — 표 행 높이를 한글에 미리 알려 주는 데 쓴다
fn estimate_height(p: &Para, width: f64) -> f64 {
    let line = p.line.unwrap_or(p.size * 1.6 * p.line_mult.unwrap_or(1.0));
    let mut lines = 0.0;
    let mut cur = 0.0;
    for (t, s) in &p.runs {
        let Some(s) = s else {
            lines += 1.0;
            cur = 0.0;
            continue;
        };
        for c in t.chars() {
            cur += if (c as u32) < 0x2E80 { s.size * 0.55 } else { s.size };
        }
    }
    lines += (cur / (width - p.pad_left - p.indent_right).max(10.0)).ceil().max(1.0);
    lines * line + p.before + p.after
}

/// docx 바이트 → (쪽 설정, 블록 목록). 그림은 tmp 에 파일로 풀어 둔다.
pub fn read(path: &str, data: &[u8], tmp: &Path, rep: &mut Report) -> Result<(Page, Vec<Block>), String> {
    let doc = wordcraft_engine::io::open_bytes(path, data)?;
    let sp = &doc.last_section;
    let page = Page {
        w: sp.page_w as f64,
        h: sp.page_h as f64,
        margin: [sp.margin_top as f64, sp.margin_right as f64, sp.margin_bottom as f64, sp.margin_left as f64],
    };
    if !doc.parts.is_empty() {
        rep.skip("머리말·꼬리말·각주");
    }
    let mut r = Reader { doc: &doc, tmp, rep, pics: 0 };
    let mut out = Vec::new();
    r.blocks(&doc.body, &mut out);
    while matches!(out.last(), Some(Block::Break)) {
        out.pop();
    }
    Ok((page, out))
}
