//! HWPX(·HWP) → 블록 목록(M4 docx↔HWPX 변환). rhwp 문서 모형을 읽어 문단·표·그림·쪽 나눔으로 편다.
//! 문단 여백·간격·고정 줄 간격은 모형에 실제 값의 두 배로 들어 있다(rhwp style_resolver 와 같은 규칙).

use std::path::Path;

use octodocs_ohwp::rhwp::model::control::Control;
use octodocs_ohwp::rhwp::model::document::Document;
use octodocs_ohwp::rhwp::model::paragraph::{ColumnBreakType, NumberingMarker, Paragraph};
use octodocs_ohwp::rhwp::model::style::{Alignment, BorderFill, BorderLineType, CharShape, FillType, HeadType, LineSpacingType, UnderlineType};
use octodocs_ohwp::rhwp::model::table::{Table as HTable, VerticalAlign};

use super::emit::Page;
use super::flow::{Block, Cell, Image, Para, Run, RunStyle, Table};
use super::Report;

/// HWPUNIT → pt
fn pt(v: i64) -> f64 {
    v as f64 / 100.0
}

/// ColorRef(0x00BBGGRR) → "RRGGBB". 위 바이트가 0 이 아니면 "없음"
fn color(c: u32) -> Option<String> {
    if c >> 24 != 0 {
        return None;
    }
    Some(format!("{:02X}{:02X}{:02X}", c & 0xFF, (c >> 8) & 0xFF, (c >> 16) & 0xFF))
}

/// 한글 선 굵기 표(mm) — 굵기 번호 → pt
const HWP_W: [f64; 16] = [0.1, 0.12, 0.15, 0.2, 0.25, 0.3, 0.4, 0.5, 0.6, 0.7, 1.0, 1.5, 2.0, 3.0, 4.0, 5.0];

struct Reader<'a> {
    doc: &'a Document,
    tmp: &'a Path,
    rep: &'a mut Report,
    pics: usize,
}

impl Reader<'_> {
    fn font(&self, lang: usize, id: u16) -> Option<String> {
        self.doc.doc_info.font_faces.get(lang).and_then(|f| f.get(id as usize)).map(|f| f.name.clone()).filter(|n| !n.is_empty())
    }

    fn run_style(&self, cs: &CharShape) -> RunStyle {
        let shade = color(cs.shade_color).filter(|c| c != "FFFFFF");
        RunStyle {
            size: pt(cs.base_size as i64),
            bold: cs.bold,
            italic: cs.italic,
            underline: !matches!(cs.underline_type, UnderlineType::None),
            color: color(cs.text_color).unwrap_or_else(|| "000000".into()),
            shade,
            strike: cs.strikethrough,
            font: self.font(0, cs.font_ids[0]),
            font_latin: self.font(1, cs.font_ids[1]),
            spacing: cs.spacings[0] as f64,
            ratio: cs.ratios[0] as f64,
            ..RunStyle::plain(10.0)
        }
    }

    /// 문단 글자 → 조각(글자 모양이 바뀌는 자리마다)
    fn runs(&self, p: &Paragraph) -> Vec<Run> {
        let shapes = &self.doc.doc_info.char_shapes;
        let style_at = |pos: u32| -> RunStyle {
            let id = p.char_shapes.iter().filter(|r| r.start_pos <= pos).last().or(p.char_shapes.first()).map(|r| r.char_shape_id).unwrap_or(0);
            shapes.get(id as usize).map(|cs| self.run_style(cs)).unwrap_or_else(|| RunStyle::plain(10.0))
        };
        let mut out: Vec<Run> = Vec::new();
        // 문단 머리(번호·글머리표) — 한글이 그리는 글자를 본문 앞에 글자로 붙인다(Word 번호 목록으로는 옮기지 않음)
        if let Some(m) = self.marker(p) {
            out.push((format!("{m} "), Some(style_at(p.char_offsets.first().copied().unwrap_or(0)))));
        }
        // 양식 개체(선택 상자·라디오 단추·입력 상자) — 글자로 문단 앞에(02 사업계획서 "기관형태" 선택지가 빠졌다)
        for c in &p.controls {
            if let Control::Form(f) = c {
                use octodocs_ohwp::rhwp::model::control::FormType;
                let t = match f.form_type {
                    FormType::CheckBox => format!("{}{} ", if f.value == 1 { '☑' } else { '☐' }, f.caption),
                    FormType::RadioButton => format!("{}{} ", if f.value == 1 { '◉' } else { '○' }, f.caption),
                    FormType::PushButton => format!("{} ", f.caption),
                    _ => format!("{} ", f.text),
                };
                if !t.trim().is_empty() {
                    out.push((t, Some(style_at(p.char_offsets.first().copied().unwrap_or(0)))));
                }
            }
        }
        for (i, ch) in p.text.chars().enumerate() {
            if ch == '\n' || ch == '\r' {
                out.push(("\n".into(), None));
                continue;
            }
            if (ch as u32) < 0x20 && ch != '\t' {
                continue; // 제어 문자 자리
            }
            let pos = p.char_offsets.get(i).copied().unwrap_or(i as u32);
            let st = style_at(pos);
            match out.last_mut() {
                Some((t, Some(s))) if *s == st => t.push(ch),
                _ => out.push((ch.to_string(), Some(st))),
            }
        }
        out
    }

    fn marker(&self, p: &Paragraph) -> Option<String> {
        if let NumberingMarker::Text(t, _) = &p.numbering_marker {
            return Some(t.clone());
        }
        let ps = self.doc.doc_info.para_shapes.get(p.para_shape_id as usize)?;
        if ps.head_type != HeadType::Bullet || ps.numbering_id == 0 {
            return None;
        }
        let c = self.doc.doc_info.bullets.get(ps.numbering_id as usize - 1)?.bullet_char;
        if c == '\u{FFFF}' {
            return None;
        }
        // 기호 글꼴 자리(PUA F0xx)는 흔한 것만 표준 글자로
        let c = match c as u32 {
            0xF06C | 0xF09F => '●',
            0xF06E | 0xF0A7 => '■',
            0xF0D8 => '▶',
            0xF0FC => '✓',
            0xF000..=0xF0FF => '•',
            _ => c,
        };
        Some(c.to_string())
    }

    /// 문단 모양 → 문단(글자 크기는 첫 조각)
    fn para(&self, p: &Paragraph, runs: Vec<Run>) -> Para {
        let mut q = Para::new(runs);
        let size = q.runs.iter().find_map(|(_, s)| s.as_ref().map(|s| s.size)).unwrap_or_else(|| {
            // 빈 문단은 첫 글자 모양 크기
            let id = p.char_shapes.first().map(|r| r.char_shape_id).unwrap_or(0);
            self.doc.doc_info.char_shapes.get(id as usize).map(|c| pt(c.base_size as i64)).unwrap_or(10.0)
        });
        q.size = size;
        let Some(ps) = self.doc.doc_info.para_shapes.get(p.para_shape_id as usize) else { return q };
        q.align = match ps.alignment {
            Alignment::Left => "left",
            Alignment::Right => "right",
            Alignment::Center => "center",
            _ => "justify",
        };
        let half = |v: i32| v as f64 / 200.0;
        // 한글 내어쓰기(들여쓰기 < 0)는 첫 줄은 왼쪽 여백에, 이어지는 줄을 그만큼 안으로 —
        // Word 는 왼쪽 들여쓰기가 이어지는 줄 기준이라 그만큼 더하고 첫 줄을 음수로
        q.first_line = half(ps.indent);
        q.pad_left = half(ps.margin_left) + (-q.first_line).max(0.0);
        q.indent_right = half(ps.margin_right);
        q.before = half(ps.spacing_before);
        q.after = half(ps.spacing_after);
        if let Some(td) = self.doc.doc_info.tab_defs.get(ps.tab_def_id as usize) {
            // 탭 자리도 두 배로 들어 있다
            q.tabs = td.tabs.iter().map(|t| (t.position as f64 / 200.0, t.tab_type, t.fill_type != 0)).collect();
        }
        match ps.line_spacing_type {
            // 한글 비율 줄 간격은 글자 크기 기준 — Word 의 "배수"(글꼴 줄 높이 기준)와 달라 고정 값으로 옮긴다
            LineSpacingType::Percent => q.line = Some(size * ps.line_spacing as f64 / 100.0),
            LineSpacingType::Fixed => q.line = Some(half(ps.line_spacing)),
            LineSpacingType::Minimum => {
                q.line = Some(half(ps.line_spacing));
                q.line_min = true;
            }
            LineSpacingType::SpaceOnly => q.line_mult = Some(1.0),
        }
        q
    }

    fn border_fill(&self, id: u16) -> Option<&BorderFill> {
        // 1부터 센다
        id.checked_sub(1).and_then(|i| self.doc.doc_info.border_fills.get(i as usize))
    }

    fn table(&mut self, t: &HTable) -> Table {
        let (rows, cols) = (t.row_count.max(1) as usize, t.col_count.max(1) as usize);
        // 열 너비: 한 칸짜리 칸들에서, 없으면 합친 칸을 나눠서
        let mut widths: Vec<Option<f64>> = vec![None; cols];
        for c in &t.cells {
            if c.col_span == 1 && (c.col as usize) < cols {
                let w = pt(c.width as i64);
                widths[c.col as usize] = Some(widths[c.col as usize].map_or(w, |x: f64| x.min(w)));
            }
        }
        for c in &t.cells {
            let (c0, n) = (c.col as usize, (c.col_span.max(1) as usize).min(cols.saturating_sub(c.col as usize)));
            let known: f64 = (c0..c0 + n).filter_map(|i| widths.get(i).copied().flatten()).sum();
            let missing: Vec<usize> = (c0..c0 + n).filter(|&i| widths.get(i).is_some_and(|w| w.is_none())).collect();
            if !missing.is_empty() {
                let each = ((pt(c.width as i64) - known) / missing.len() as f64).max(5.0);
                for i in missing {
                    widths[i] = Some(each);
                }
            }
        }
        let widths: Vec<Option<f64>> = widths.into_iter().map(|w| Some(w.unwrap_or(30.0))).collect();
        let mut cells = Vec::new();
        for c in &t.cells {
            let bf = self.border_fill(c.border_fill_id);
            let mut lines = Vec::new();
            let mut bg = None;
            if let Some(bf) = bf {
                // borders: 왼쪽·오른쪽·위·아래
                for (k, side) in ["left", "right", "top", "bottom"].iter().enumerate() {
                    let b = &bf.borders[k];
                    if !matches!(b.line_type, BorderLineType::None) {
                        let w = HWP_W.get(b.width as usize).copied().unwrap_or(0.12) * super::css::MM;
                        lines.push((*side, (color(b.color).unwrap_or_else(|| "000000".into()), (w * 100.0).round() / 100.0)));
                    } else {
                        lines.push((*side, (String::new(), 0.0)));
                    }
                }
                if matches!(bf.fill.fill_type, FillType::Solid) {
                    bg = bf.fill.solid.as_ref().filter(|s| s.pattern_type <= 0).and_then(|s| color(s.background_color));
                }
                if matches!(bf.fill.fill_type, FillType::Gradient | FillType::Image) {
                    self.rep.note("cell_fill", "칸 그러데이션·그림 채우기 — 옮기지 않음");
                }
            }
            let pad = if c.apply_inner_margin { &c.padding } else { &t.padding };
            let (mut paras, mut tables) = (Vec::new(), Vec::new());
            for b in self.paras(&c.paragraphs, true) {
                match b {
                    Block::Para(p) => paras.push(p),
                    Block::Table(t) => tables.push((paras.len(), t)),
                    _ => {}
                }
            }
            // 한글은 칸의 마지막 줄 아래에 줄 간격 여유를 두지 않는데 Word 고정 줄 간격은 둔다 — 글이 행 높이를 정하는 칸이
            // 줄마다 길어져 표가 쪽을 넘었다(02 사업계획서 실무자 칸, 34 → 30쪽). 마지막 문단이 한 줄이면 글자 크기의 1.25배로.
            let inner = pt(c.width as i64) - pt(pad.left as i64) - pt(pad.right as i64);
            if let Some(last) = paras.last_mut() {
                let w: f64 = last.runs.iter().filter_map(|(t, s)| s.as_ref().map(|s| t.chars().map(|ch| if (ch as u32) < 0x2E80 { s.size * 0.55 } else { s.size }).sum::<f64>())).sum();
                let one_line = !last.runs.iter().any(|(_, s)| s.is_none()) && w <= inner - last.pad_left - last.indent_right - last.first_line.max(0.0);
                if one_line && !last.line_min && last.line.is_some_and(|l| l > last.size * 1.25) {
                    last.line = Some(last.size * 1.25);
                }
            }
            cells.push(Cell {
                paras,
                bg,
                valign: match c.vertical_align {
                    VerticalAlign::Center => "center",
                    VerticalAlign::Bottom => "bottom",
                    _ => "top",
                },
                pad: [pt(pad.top as i64), pt(pad.right as i64), pt(pad.bottom as i64), pt(pad.left as i64)],
                at: (c.row as usize, c.col as usize),
                span: (c.row_span.max(1) as usize, c.col_span.max(1) as usize),
                h: pt(c.height as i64),
                lines,
                tables,
            });
        }
        // 한글 표는 행마다 칸 경계가 달라도 된다 — 열 번호가 같아도 폭이 달랐고, 가장 좁은 폭을 열 너비로 쓰면
        // Word 에서 칸이 좁아져 글이 접혔다(01 사업계획서 "사업자등록번호" 칸 99.6pt → 96.8pt).
        // 모든 행의 실제 경계를 모은 촘촘한 격자로 옮긴다.
        let spec: Vec<_> = t.cells.iter().map(|c| (c.row as usize, c.col as usize, c.row_span.max(1) as usize, c.col_span.max(1) as usize, pt(c.width as i64))).collect();
        let (cols, widths) = match fine_grid(rows, &spec) {
            Some((w, place)) => {
                for (cl, (c0, n)) in cells.iter_mut().zip(place) {
                    cl.at.1 = c0;
                    cl.span.1 = n;
                }
                (w.len(), w.into_iter().map(Some).collect())
            }
            None => (cols, widths),
        };
        Table { rows, cols, widths, cells, border: None, inner: true, top: 0.0, bottom: 0.0, before: 0.0 }
    }

    fn picture(&mut self, pic: &octodocs_ohwp::rhwp::model::image::Picture, align: &'static str) -> Option<Image> {
        let id = pic.image_attr.bin_data_id;
        let list = &self.doc.bin_data_content;
        let c = if id == 0 { None } else { list.get(id as usize - 1).or_else(|| list.iter().find(|c| c.id == id)) }?;
        let bytes = c.data.load();
        let ext = c.extension.to_ascii_lowercase();
        if !matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp") {
            return None;
        }
        self.pics += 1;
        let path = self.tmp.join(format!("hwpx_pic{}.{ext}", self.pics));
        std::fs::write(&path, bytes).ok()?;
        if !pic.common.treat_as_char {
            self.rep.note("float_picture", "떠 있는 그림 — 글자처럼 취급하는 그림으로 넣음");
        }
        Some(Image {
            src: path.to_string_lossy().into_owned(),
            w: pt(pic.common.width as i64),
            h: pt(pic.common.height as i64),
            align,
            top: 0.0,
            bottom: 0.0,
            before: 0.0,
        })
    }

    /// 문단 목록 → 블록. in_cell 이면 표·그림은 넣지 않고 보고서에 적는다.
    fn paras(&mut self, ps: &[Paragraph], in_cell: bool) -> Vec<Block> {
        let mut out = Vec::new();
        for p in ps {
            if !in_cell && matches!(p.column_type, ColumnBreakType::Page | ColumnBreakType::Section) && !out.is_empty() {
                out.push(Block::Break);
            }
            let runs = self.runs(p);
            let has_text = runs.iter().any(|(t, s)| s.is_some() && !t.is_empty());
            let objects: Vec<&Control> = p.controls.iter().filter(|c| matches!(c, Control::Table(_) | Control::Picture(_))).collect();
            let para = self.para(p, runs);
            if has_text || objects.is_empty() {
                out.push(Block::Para(para.clone()));
            }
            for c in p.controls.iter() {
                match c {
                    // 칸 안의 표도 표로(docx 는 칸 안에 표를 둔다) — 큰 테두리 상자 안의 제목 띠·흐름표가 빠졌다(08 사업계획서)
                    Control::Table(t) => out.push(Block::Table(self.table(t))),
                    Control::Picture(pic) if !in_cell => match self.picture(pic, para.align) {
                        Some(im) => out.push(Block::Image(im)),
                        None => self.rep.skip("그림(읽지 못한 형식)"),
                    },
                    Control::Picture(_) => self.rep.note("cell_picture", "칸 안의 그림 — 옮기지 않음"),
                    Control::Shape(_) => self.rep.skip("도형·글상자"),
                    Control::Header(_) => self.rep.skip("머리말"),
                    Control::Footer(_) => self.rep.skip("꼬리말"),
                    Control::Footnote(_) => self.rep.skip("각주"),
                    Control::Endnote(_) => self.rep.skip("미주"),
                    Control::PageNumberPos(_) => self.rep.skip("쪽 번호"),
                    _ => {}
                }
            }
        }
        out
    }
}

/// HWPX 바이트 → (쪽 설정, 블록 목록). 그림은 tmp 에 파일로 풀어 둔다.
pub fn read(data: Vec<u8>, tmp: &Path, rep: &mut Report) -> Result<(Page, Vec<Block>), String> {
    let s = octodocs_ohwp::Session::open(data)?;
    let doc = s.document();
    let sec = doc.sections.first().ok_or("구역이 없습니다")?;
    let pd = &sec.section_def.page_def;
    let (mut w, mut h) = (pt(pd.width as i64), pt(pd.height as i64));
    if pd.landscape && w < h {
        std::mem::swap(&mut w, &mut h);
    }
    // 본문 위·아래는 여백 + 머리말·꼬리말 자리
    let page = Page {
        w,
        h,
        margin: [pt((pd.margin_top + pd.margin_header) as i64), pt(pd.margin_right as i64), pt((pd.margin_bottom + pd.margin_footer) as i64), pt(pd.margin_left as i64)],
    };
    if doc.sections.len() > 1 {
        rep.note("sections", "구역이 여럿 — 첫 구역의 쪽 설정을 문서 전체에 씀");
    }
    let mut r = Reader { doc, tmp, rep, pics: 0 };
    let mut blocks = Vec::new();
    for (i, sec) in doc.sections.iter().enumerate() {
        if i > 0 {
            blocks.push(Block::Break);
        }
        blocks.extend(r.paras(&sec.paragraphs, false));
    }
    Ok((page, blocks))
}

/// 칸들(행, 열, 행 수, 열 수, 폭 pt)의 실제 가로 경계를 모아 촘촘한 열 격자를 만든다.
/// (열 너비들, 칸마다 새 (시작 열, 열 수)). 행 안에서 열 번호 순서로 폭을 쌓아 경계를 잰다(위에서 내려온 세로 합침 칸 포함).
/// 0.75pt 안의 경계는 하나로 본다. 열 번호가 겹치거나 비면 None — 옛 방식으로.
fn fine_grid(rows: usize, cells: &[(usize, usize, usize, usize, f64)]) -> Option<(Vec<f64>, Vec<(usize, usize)>)> {
    let mut start = vec![f64::NAN; cells.len()];
    let mut end = vec![f64::NAN; cells.len()];
    let mut bounds: Vec<f64> = vec![0.0];
    for r in 0..rows {
        let mut row: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].0 <= r && r < cells[i].0 + cells[i].2).collect();
        row.sort_by_key(|&i| cells[i].1);
        let (mut x, mut next) = (0.0f64, 0usize);
        for &i in &row {
            if cells[i].1 != next {
                return None;
            }
            if r == cells[i].0 {
                start[i] = x;
                end[i] = x + cells[i].4;
            } else if (start[i] - x).abs() > 0.75 {
                // 위에서 내려온 칸이 이 행에서는 다른 자리 — 첫 행 자리를 따른다
                x = start[i];
            }
            x += cells[i].4;
            next += cells[i].3;
            bounds.push(x);
        }
    }
    if start.iter().any(|v| v.is_nan()) {
        return None;
    }
    bounds.extend(start.iter().copied());
    bounds.sort_by(f64::total_cmp);
    let mut grid: Vec<f64> = Vec::new();
    let mut group: Vec<f64> = Vec::new();
    for b in bounds {
        if group.last().is_some_and(|&l| b - l > 0.75) {
            grid.push(group.iter().sum::<f64>() / group.len() as f64);
            group.clear();
        }
        group.push(b);
    }
    if !group.is_empty() {
        grid.push(group.iter().sum::<f64>() / group.len() as f64);
    }
    if grid.len() < 2 {
        return None;
    }
    let near = |x: f64| (0..grid.len()).min_by(|&a, &b| (grid[a] - x).abs().total_cmp(&(grid[b] - x).abs())).unwrap();
    let place = (0..cells.len())
        .map(|i| {
            let (a, b) = (near(start[i]), near(end[i]));
            (a.min(grid.len() - 2), b.max(a + 1) - a.min(grid.len() - 2))
        })
        .collect();
    let widths = grid.windows(2).map(|w| w[1] - w[0]).collect();
    Some((widths, place))
}
