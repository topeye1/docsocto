//! 블록 목록 → docx (WordCraft 문서 모형을 바로 짠다). 칸별 테두리·세로 합침·행 높이까지 담는다 — 명령 방식(emit::Docx)으로는
//! 칸마다 다른 테두리와 세로 합침을 넣을 수 없어서 docx↔HWPX 변환은 이 길을 쓴다.

use std::sync::Arc;

use wordcraft_doc::para::{Float, InlineObject, OBJ};
use wordcraft_doc::props::{Border, BorderStyle, Borders, CellProps, HeightRule, LineSpacing, RowProps, TabAlign, TabLeader, TabStop, TableProps, Underline, VAlign, VMerge};
use wordcraft_doc::table::{Cell as WCell, Row, Table as WTable};
use wordcraft_doc::{Align, Block as WBlock, CharProps, Document, ParaProps, Paragraph, Rgb, Run as WRun, TextColor};

use super::emit::{Page, FONT};
use super::flow::{Block, Cell, Image, Para, RunStyle, Table};

fn rgb(h: &str) -> Option<Rgb> {
    Rgb::parse(h)
}

fn char_props(s: &RunStyle) -> CharProps {
    let font = s.font.clone().unwrap_or_else(|| FONT.into());
    CharProps {
        font: Some(s.font_latin.clone().unwrap_or_else(|| font.clone())),
        font_ea: Some(font),
        size: Some(s.size as f32),
        bold: Some(s.bold),
        italic: Some(s.italic),
        underline: Some(if s.underline { Underline::Single } else { Underline::None }),
        strike: Some(s.strike),
        color: rgb(&s.color).map(TextColor::Rgb),
        shading: s.shade.as_deref().and_then(rgb),
        // 자간(%) → 글자 사이(pt), 장평(%) → 가로 비율
        spacing: (s.spacing != 0.0).then(|| (s.size * s.spacing / 100.0) as f32),
        scale: ((s.ratio - 100.0).abs() > 0.5).then_some(s.ratio as f32),
        ..Default::default()
    }
}

fn para_props(p: &Para, page_break: bool) -> ParaProps {
    ParaProps {
        align: Some(match p.align {
            "center" => Align::Center,
            "right" => Align::Right,
            "justify" => Align::Justify,
            _ => Align::Left,
        }),
        indent_left: Some(p.pad_left as f32),
        indent_right: Some(p.indent_right as f32),
        indent_first: Some(p.first_line as f32),
        space_before: Some(p.before as f32),
        space_after: Some(p.after as f32),
        line_spacing: Some(match p.line {
            Some(l) if p.line_min => LineSpacing::AtLeast(l as f32),
            Some(l) => LineSpacing::Exactly(l as f32),
            None => LineSpacing::Multiple(p.line_mult.unwrap_or(1.0) as f32),
        }),
        // 한글·숫자/영문 사이 자동 띄우기는 한글에 없다
        auto_space_de: Some(false),
        auto_space_dn: Some(false),
        page_break_before: page_break.then_some(true),
        tabs: (!p.tabs.is_empty()).then(|| {
            p.tabs
                .iter()
                .map(|&(pos, kind, dots)| TabStop {
                    pos: pos as f32,
                    align: match kind {
                        1 => TabAlign::Right,
                        2 => TabAlign::Center,
                        3 => TabAlign::Decimal,
                        _ => TabAlign::Left,
                    },
                    leader: if dots { TabLeader::Dot } else { TabLeader::None },
                })
                .collect()
        }),
        ..Default::default()
    }
}

fn paragraph(p: &Para, page_break: bool) -> Paragraph {
    let mut q = Paragraph::new();
    let mut first: Option<CharProps> = None;
    for (t, s) in &p.runs {
        let (text, props) = match s {
            None => ("\n".to_string(), first.clone().unwrap_or_default()),
            Some(s) => (if s.pad { format!(" {t} ") } else { t.clone() }, char_props(s)),
        };
        if first.is_none() && s.is_some() {
            first = Some(props.clone());
        }
        q.text.push_str(&text);
        q.runs.push(WRun { len: text.len(), props });
    }
    q.props = para_props(p, page_break);
    // 빈 문단의 높이는 문단 표시 서식이 정한다
    q.mark = first.unwrap_or_else(|| CharProps { size: Some(p.size as f32), font_ea: Some(FONT.into()), ..Default::default() });
    q
}

fn image_para(doc: &mut Document, im: &Image, page_break: bool) -> Option<Paragraph> {
    let bytes = std::fs::read(&im.src).ok()?;
    let ext = std::path::Path::new(&im.src).extension().and_then(|e| e.to_str()).unwrap_or("png").to_ascii_lowercase();
    let media = doc.add_media(bytes, &ext);
    let mut q = Paragraph::new();
    q.text.push(OBJ);
    q.runs.push(WRun { len: OBJ.len_utf8(), props: CharProps::default() });
    q.objects.push(InlineObject::Image { media, w: im.w as f32, h: im.h as f32, alt: String::new(), float: Float::default(), crop: [0.0; 4], ole: None });
    let mut p = Para::new(Vec::new());
    p.align = im.align;
    p.before = im.before;
    q.props = para_props(&p, page_break);
    Some(q)
}

fn border(side: Option<&(String, f64)>) -> Border {
    match side {
        Some((col, w)) if *w > 0.0 && !col.is_empty() => Border { style: BorderStyle::Single, width: (*w as f32).max(0.25), color: rgb(col), space: 0.0 },
        _ => Border { style: BorderStyle::None, width: 0.0, color: None, space: 0.0 },
    }
}

fn cell_borders(t: &Table, cl: &Cell) -> Borders {
    let find = |k: &str| -> Option<(String, f64)> {
        if let Some((_, v)) = cl.lines.iter().find(|(s, _)| *s == k) {
            return Some(v.clone());
        }
        // 표 테두리: 안쪽 경계는 inner 일 때만
        let (r, c) = cl.at;
        let outer = match k {
            "top" => r == 0,
            "bottom" => r + cl.span.0 >= t.rows,
            "left" => c == 0,
            _ => c + cl.span.1 >= t.cols,
        };
        t.border.clone().filter(|_| outer || t.inner)
    };
    Borders {
        top: Some(border(find("top").as_ref())),
        left: Some(border(find("left").as_ref())),
        bottom: Some(border(find("bottom").as_ref())),
        right: Some(border(find("right").as_ref())),
        between: None,
        inside_v: None,
    }
}

fn cell_blocks(cl: &Cell) -> Vec<Arc<WBlock>> {
    let mut v: Vec<Arc<WBlock>> = cl.paras.iter().map(|p| Arc::new(WBlock::Para(paragraph(p, false)))).collect();
    if v.is_empty() {
        v.push(Arc::new(WBlock::Para(Paragraph::new())));
    }
    v
}

fn table(t: &Table) -> WTable {
    let mut t = t.clone();
    t.normalize();
    let t = &t;
    let widths: Vec<f32> = t.widths.iter().map(|w| w.unwrap_or(400.0 / t.cols as f64) as f32).collect();
    // Word 는 행 높이에 그 행에서 시작하는 칸들의 가장 큰 위아래 여백을 더해 그렸다(01 사업신청서 22.3 → 28.4pt,
    // 여백을 빼니 22.7pt; 세로로 합친 칸이 그 행에서 시작하면 그 칸의 여백) — 빼서 넣는다
    let mut pad_v = vec![0.0f64; t.rows];
    for cl in t.cells.iter().filter(|cl| cl.at.0 < t.rows) {
        pad_v[cl.at.0] = pad_v[cl.at.0].max(cl.pad[0] + cl.pad[2]);
    }
    let mut row_h = vec![0.0f64; t.rows];
    for cl in &t.cells {
        if cl.span.0 == 1 && cl.at.0 < t.rows {
            row_h[cl.at.0] = row_h[cl.at.0].max(cl.h - pad_v[cl.at.0]);
        }
    }
    // 세로로 합친 칸의 높이는 합친 행들의 합이다(한글은 행 높이를 따로 두지 않는다) — 모자라면 높이가 정해지지 않은
    // 행에 나눠 주고, 없으면 마지막 행에 더한다. 합친 칸만 있는 행이 글 높이로 늘어 표가 길었다(01 사업신청서 참여 인력).
    let mut multi: Vec<_> = t.cells.iter().filter(|cl| cl.span.0 > 1 && cl.h > 0.0 && cl.at.0 < t.rows).collect();
    multi.sort_by_key(|cl| cl.span.0);
    for cl in multi {
        let rows = cl.at.0..(cl.at.0 + cl.span.0).min(t.rows);
        let need = cl.h - pad_v[rows.clone()].iter().sum::<f64>();
        let have: f64 = row_h[rows.clone()].iter().sum();
        if need > have + 0.5 {
            let free: Vec<usize> = rows.clone().filter(|&r| row_h[r] <= 0.0).collect();
            if free.is_empty() {
                row_h[rows.end - 1] += need - have;
            } else {
                for &r in &free {
                    row_h[r] = (need - have) / free.len() as f64;
                }
            }
        }
    }
    let at = |r: usize, c: usize| t.cells.iter().find(|cl| cl.at == (r, c));
    // 세로로 합쳐 아래 행을 덮는 칸
    let covering = |r: usize, c: usize| t.cells.iter().find(|cl| cl.at.1 == c && cl.at.0 < r && r < cl.at.0 + cl.span.0);
    let mut rows = Vec::new();
    for r in 0..t.rows {
        let mut cells = Vec::new();
        let mut c = 0;
        while c < t.cols {
            let (cl, vm) = match (at(r, c), covering(r, c)) {
                (Some(cl), _) => (Some(cl), if cl.span.0 > 1 { VMerge::Restart } else { VMerge::None }),
                (None, Some(cl)) => (Some(cl), VMerge::Continue),
                _ => (None, VMerge::None),
            };
            let span = cl.map(|x| x.span.1).unwrap_or(1).clamp(1, t.cols - c);
            let w: f32 = widths[c..c + span].iter().sum();
            let mut wc = match (cl, vm) {
                (Some(cl), VMerge::Continue) => WCell { props: CellProps { borders: Some(cell_borders(t, cl)), shading: cl.bg.as_deref().and_then(rgb), ..Default::default() }, ..WCell::empty() },
                (Some(cl), _) => WCell {
                    props: CellProps {
                        shading: cl.bg.as_deref().and_then(rgb),
                        borders: Some(cell_borders(t, cl)),
                        valign: Some(match cl.valign {
                            "center" => VAlign::Center,
                            "bottom" => VAlign::Bottom,
                            _ => VAlign::Top,
                        }),
                        margins: Some([cl.pad[0] as f32, cl.pad[3] as f32, cl.pad[2] as f32, cl.pad[1] as f32]),
                        ..Default::default()
                    },
                    blocks: cell_blocks(cl),
                    ..WCell::empty()
                },
                _ => WCell::empty(),
            };
            wc.props.span = span as u32;
            wc.props.vmerge = vm;
            wc.props.width = Some(w);
            cells.push(wc);
            c += span;
        }
        let props = RowProps { height: (row_h[r] > 0.0).then_some(row_h[r] as f32), height_rule: HeightRule::AtLeast, cant_split: false, ..Default::default() };
        rows.push(Row { props, cells, ..Default::default() });
    }
    let none = Border { style: BorderStyle::None, width: 0.0, color: None, space: 0.0 };
    WTable {
        props: TableProps {
            width: Some(widths.iter().sum()),
            borders: Some(Borders::all(none)),
            cell_margins: Some([0.0; 4]),
            fixed: true,
            ..Default::default()
        },
        grid: widths,
        rows,
        ..Default::default()
    }
}

/// 양쪽 정렬 문단의 줄 바꿈은 문단으로 나눈다 — Word 는 줄 바꿈(Shift+Enter)으로 끝나는 줄도 양쪽으로
/// 벌려 글자 사이가 크게 벌어졌다(한글은 벌리지 않는다). 이어지는 문단은 첫 줄 들여쓰기 없이, 앞뒤 간격은 처음·끝에만.
fn split_justified(p: &Para) -> Vec<Para> {
    if p.align != "justify" || !p.runs.iter().any(|(_, s)| s.is_none()) {
        return vec![p.clone()];
    }
    let mut parts: Vec<Vec<super::flow::Run>> = vec![Vec::new()];
    for r in &p.runs {
        if r.1.is_none() {
            parts.push(Vec::new());
        } else {
            parts.last_mut().unwrap().push(r.clone());
        }
    }
    let n = parts.len();
    parts
        .into_iter()
        .enumerate()
        .map(|(i, runs)| {
            let mut q = p.clone();
            q.runs = runs;
            if i > 0 {
                q.before = 0.0;
                q.first_line = 0.0;
            }
            if i + 1 < n {
                q.after = 0.0;
            }
            q
        })
        .collect()
}

/// 블록 목록 → docx 바이트
pub fn write(blocks: &[Block], page: Page) -> Result<Vec<u8>, String> {
    let mut doc = Document::new();
    // 한글 기본 탭 간격(40pt)
    doc.settings.default_tab = 40.0;
    let sp = &mut doc.last_section;
    sp.page_w = page.w as f32;
    sp.page_h = page.h as f32;
    sp.landscape = page.w > page.h;
    sp.margin_top = page.margin[0] as f32;
    sp.margin_right = page.margin[1] as f32;
    sp.margin_bottom = page.margin[2] as f32;
    sp.margin_left = page.margin[3] as f32;
    sp.header = (page.margin[0] / 2.0) as f32;
    sp.footer = (page.margin[2] / 2.0) as f32;
    let mut body: Vec<Arc<WBlock>> = Vec::new();
    let mut brk = false;
    let mut prev_table = false;
    for bl in blocks {
        match bl {
            Block::Break => brk = true,
            Block::Para(p) => {
                for (i, q) in split_justified(p).iter().enumerate() {
                    body.push(Arc::new(WBlock::Para(paragraph(q, brk && i == 0))));
                }
                brk = false;
                prev_table = false;
            }
            Block::Image(im) => {
                if let Some(q) = image_para(&mut doc, im, brk) {
                    body.push(Arc::new(WBlock::Para(q)));
                }
                brk = false;
                prev_table = false;
            }
            Block::Table(t) => {
                // 쪽 나눔·표 둘이 붙는 곳(Word 가 한 표로 합친다)에는 작은 빈 문단을 둔다
                if brk || prev_table {
                    let mut gap = Para::new(Vec::new());
                    gap.line = Some(1.0);
                    gap.size = 1.0;
                    body.push(Arc::new(WBlock::Para(paragraph(&gap, brk))));
                    brk = false;
                }
                body.push(Arc::new(WBlock::Table(table(t))));
                prev_table = true;
            }
        }
    }
    // 문서는 문단으로 끝나야 한다
    if prev_table || body.is_empty() {
        body.push(Arc::new(WBlock::Para(Paragraph::new())));
    }
    doc.body = body;
    wordcraft_engine::io::save_bytes("out.docx", &doc)
}
