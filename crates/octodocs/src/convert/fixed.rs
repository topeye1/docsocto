//! 자리 고정 방식(M4-H2·H4, 결정 #33): 꾸민 상자(배경·테두리·그러데이션)는 도형, 글이 든 블록은 투명 글상자로
//! Edge 가 그린 자리에 그대로 놓는다. 표지처럼 한 쪽에 짜인 화면용이다.

use serde_json::{json, Value};

use super::css::*;
use super::emit::{Cmds, FIXED_X, FONT};
use super::Report;

/// WordCraft 글상자 기본 안쪽 여백(pt)
const INSET_X: f64 = 7.2;
const INSET_Y: f64 = 3.6;
/// Word·한글 글자가 Edge 보다 조금 넓다 — 글상자 폭 여유
const SLACK: f64 = 1.10;

fn decorated(n: &Value) -> bool {
    bg(n).is_some() || gradient(sv(n, "background-image")).is_some() || !sides(n).is_empty()
}

fn is_block(n: &Value) -> bool {
    !matches!(sv(n, "display"), "inline" | "inline-block") || decorated(n) || matches!(tag(n), "table" | "tr" | "td" | "th")
}

/// 블록이 바로 품은 글줄들의 바깥 상자(px)
fn text_bounds(n: &Value) -> Option<Rect> {
    let mut xs: Vec<Rect> = Vec::new();
    for k in kids(n) {
        if tag(k) == "#text" {
            xs.extend(k["lines"].as_array().into_iter().flatten().map(|l| rect(&l["box"])));
        } else if tag(k) != "br" && !is_block(k) {
            xs.extend(text_bounds(k));
        }
    }
    if xs.is_empty() {
        return None;
    }
    let x0 = xs.iter().map(|b| b.x).fold(f64::MAX, f64::min);
    let y0 = xs.iter().map(|b| b.y).fold(f64::MAX, f64::min);
    let x1 = xs.iter().map(|b| b.x + b.w).fold(f64::MIN, f64::max);
    let y1 = xs.iter().map(|b| b.y + b.h).fold(f64::MIN, f64::max);
    Some(Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
}

/// 글상자 가로 자리(px): 정렬에 맞춰 Edge 가 그린 글의 기준선(왼끝·가운데·오른끝)을 지키고 남는 폭은 그 반대쪽으로 준다.
fn text_frame(n: &Value, b: Rect, t: Rect) -> (f64, f64, &'static str) {
    let (pl, pr) = (spx(n, "padding-left"), spx(n, "padding-right"));
    let w = (b.w - pl - pr).max(t.w) * SLACK;
    match sv(n, "text-align") {
        "center" => (t.x + t.w / 2.0 - w / 2.0, w, "center"),
        "right" | "end" => (t.x + t.w - w, w, "right"),
        _ => (t.x, w, "left"),
    }
}

/// 블록 안 글줄을 (줄 바꿈 포함) 서식 조각으로 편다 — 서식은 그 글을 품은 마디
fn runs_of<'a>(n: &'a Value, out: &mut Vec<(String, Option<&'a Value>)>, rep: &mut Report) {
    for k in kids(n) {
        match tag(k) {
            "#text" => {
                for (i, l) in k["lines"].as_array().into_iter().flatten().enumerate() {
                    if i > 0 {
                        out.push(("\n".into(), None));
                    }
                    out.push((l["text"].as_str().unwrap_or("").to_string(), Some(n)));
                }
            }
            "br" => out.push(("\n".into(), None)),
            "img" | "svg" | "canvas" | "video" => rep.skip(tag(k)),
            _ if !is_block(k) => runs_of(k, out, rep),
            _ => {}
        }
    }
}

fn weight(n: &Value) -> bool {
    sv(n, "font-weight").parse::<u32>().unwrap_or(400) >= 600
}

fn size_pt(n: &Value) -> f64 {
    (spx(n, "font-size") * PX * 2.0).round() / 2.0
}

fn align_word(n: &Value) -> &'static str {
    match sv(n, "text-align") {
        "center" => "center",
        "right" | "end" => "right",
        _ => "left",
    }
}

// ---------------------------------------------------------------- docx

struct DocxGen<'r> {
    ops: Vec<Cmds>,
    rep: &'r mut Report,
}

impl DocxGen<'_> {
    fn begin(&mut self) {
        self.ops.push(Vec::new());
        self.c("caret.set", json!({"pos": {"story": "body", "path": [0], "off": 0}}));
    }

    fn c(&mut self, cmd: &str, p: Value) {
        self.ops.last_mut().unwrap().push((cmd.to_string(), p));
    }

    /// 새 개체는 본문 맨 앞(0번 자리)에 들어가 늘 첫 번째 개체다
    fn select_last(&mut self) {
        self.c("select.objects", json!({"index": 0}));
    }

    fn shape(&mut self, n: &Value) {
        let b = bx(n);
        let rad = spx(n, "border-top-left-radius");
        self.begin();
        self.c("insert.shape", json!({"kind": if rad > 0.5 { "roundedRectangle" } else { "rectangle" }, "width": b.w * PX, "height": b.h * PX}));
        self.select_last();
        self.c("arrange.wrap", json!({"wrap": "behindText"}));
        self.c("arrange.position", json!({"x": b.x * PX, "y": b.y * PX}));
        if let Some(g) = gradient(sv(n, "background-image")) {
            let stops: Vec<Value> = g.stops.iter().map(|s| json!({"pos": s.pos, "color": s.color, "transparency": ((1.0 - s.alpha) * 100.0).round()})).collect();
            // CSS 각도(0=위로, 180=아래로) → Word 각도(0=왼→오, 90=위→아래)
            self.c("shape.fill", json!({"gradient": {"stops": stops, "type": "linear", "angle": (g.css_angle - 90.0).rem_euclid(360.0)}}));
        } else if let Some((col, a)) = rgba(sv(n, "background-color")).filter(|c| c.1 > 0.001) {
            self.c("shape.fill", json!({"color": col, "transparency": ((1.0 - a) * 100.0).round()}));
        } else {
            self.c("shape.fill", json!({"color": null}));
        }
        let sd = sides(n);
        if sd.len() == 4 && sd.iter().all(|s| s.1 == sd[0].1) {
            let (col, w) = &sd[0].1;
            self.c("shape.outline", json!({"color": col, "width": w}));
        } else {
            self.c("shape.outline", json!({"color": null}));
            for (s, (col, w)) in sd {
                // 한 면만 있는 테두리는 선으로
                let (x, y, ww, hh) = (b.x * PX, b.y * PX, b.w * PX, b.h * PX);
                let seg = match s {
                    "top" => (x, y, ww, 0.0),
                    "bottom" => (x, y + hh, ww, 0.0),
                    "left" => (x, y, 0.0, hh),
                    _ => (x + ww, y, 0.0, hh),
                };
                self.begin();
                self.c("insert.shape", json!({"kind": "line", "width": seg.2.max(0.1), "height": seg.3.max(0.1), "stroke": col}));
                self.select_last();
                self.c("arrange.wrap", json!({"wrap": "behindText"}));
                self.c("arrange.position", json!({"x": seg.0, "y": seg.1}));
                self.c("shape.outline", json!({"color": col, "width": w}));
            }
        }
    }

    fn textbox(&mut self, n: &Value, runs: &[(String, Option<&Value>)]) {
        let b = bx(n);
        let t = text_bounds(n).unwrap_or(b);
        let (fx, fw, _) = text_frame(n, b, t);
        self.begin();
        self.c("insert.textBox", json!({"text": "", "width": fw * PX + 2.0 * INSET_X, "height": t.h * PX * 1.3 + 2.0 * INSET_Y}));
        let align = align_word(n);
        if align != "left" {
            self.c("para.align", json!({"value": align}));
        }
        let lh = sv(n, "line-height");
        if lh.ends_with("px") {
            self.c("para.lineSpacing", json!({"exactly": px(lh) * PX}));
        }
        self.c("para.spacing", json!({"before": 0, "after": 0}));
        for (text, src) in runs {
            let Some(s) = src else {
                self.c("text.lineBreak", json!({}));
                continue;
            };
            self.c("format.font", json!({"name": FONT, "eastAsia": FONT}));
            self.c("format.size", json!({"size": size_pt(s)}));
            self.c("format.color", json!({"color": rgba(sv(s, "color")).map(|c| c.0).unwrap_or_else(|| "000000".into())}));
            self.c("format.bold", json!({"value": weight(s)}));
            self.c("text.insert", json!({"text": text}));
        }
        self.select_last();
        self.c("arrange.wrap", json!({"wrap": "inFrontOfText"}));
        self.c("arrange.position", json!({"x": fx * PX - INSET_X, "y": t.y * PX - INSET_Y}));
        self.c("shape.fill", json!({"color": null}));
        self.c("shape.outline", json!({"color": null}));
    }

    fn walk(&mut self, n: &Value) {
        if tag(n) == "#text" {
            return;
        }
        if decorated(n) {
            self.shape(n);
        }
        // 글을 바로 품은 블록은 글상자 하나로(안쪽 인라인 조각 포함)
        if kids(n).iter().any(|k| tag(k) == "#text") {
            let mut runs = Vec::new();
            runs_of(n, &mut runs, self.rep);
            if runs.iter().any(|(t, _)| !t.trim().is_empty()) {
                self.textbox(n, &runs);
            }
            for k in kids(n) {
                if tag(k) != "#text" && is_block(k) {
                    self.walk(k);
                }
            }
            return;
        }
        for k in kids(n) {
            if MEDIA.contains(&tag(k)) {
                self.rep.skip(tag(k));
            }
            self.walk(k);
        }
    }
}

const MEDIA: &[&str] = &["img", "svg", "canvas", "video"];

pub fn docx(root: &Value, rep: &mut Report) -> Cmds {
    let mut g = DocxGen { ops: Vec::new(), rep };
    g.walk(root);
    let mut cmds: Cmds = vec![
        ("layout.size".into(), json!({"name": "A4"})),
        ("layout.margins".into(), json!({"top": 0, "bottom": 0, "left": 0, "right": 0})),
    ];
    // 새 개체가 늘 본문 맨 앞에 들어가 먼저 그려지므로(아래 깔림), 바깥 상자부터 그려지게 거꾸로 낸다
    for op in g.ops.into_iter().rev() {
        cmds.extend(op);
    }
    cmds
}

// ---------------------------------------------------------------- HWPX

enum Under {
    Grad(Rect, Gradient),
    Rgb([u8; 3]),
}

struct HwpxGen<'r> {
    cmds: Cmds,
    under: Vec<Under>,
    pics: usize,
    dir: std::path::PathBuf,
    rep: &'r mut Report,
}

impl HwpxGen<'_> {
    fn c(&mut self, cmd: &str, p: Value) {
        self.cmds.push((cmd.to_string(), p));
    }

    /// 상자 b 가운데 아래에 깔린 색
    fn backdrop(&self, b: Rect) -> [u8; 3] {
        let cy = b.y + b.h / 2.0;
        for u in self.under.iter().rev() {
            match u {
                Under::Grad(ob, g) => return g.color_at((cy - ob.y) / ob.h.max(1.0)),
                Under::Rgb(c) => return *c,
            }
        }
        [255, 255, 255]
    }

    fn shape(&mut self, n: &Value) -> Result<(), String> {
        let b = bx(n);
        let (x, y, w, h) = (b.x * PX, b.y * PX, b.w * PX, b.h * PX);
        let rad = spx(n, "border-top-left-radius") * PX;
        let sd = sides(n);
        let line = (sd.len() == 4 && sd.iter().all(|s| s.1 == sd[0].1)).then(|| json!({"color": sd[0].1 .0, "width": sd[0].1 .1}));
        let mut fill = Value::Null;
        if let Some(g) = gradient(sv(n, "background-image")) {
            self.under.push(Under::Grad(b, g.clone()));
            if rad <= 0.5 && line.is_none() {
                // 한글은 그러데이션을 띠로 그려 PDF 에 가는 줄이 남는다 — 그림으로 바꿔 넣는다
                self.pics += 1;
                let path = self.dir.join(format!("grad{}.png", self.pics));
                gradient_png(&g, w, h, &path)?;
                self.rep.note("gradient_picture", "그러데이션 바탕 — 그림으로 넣음");
                self.c("picture.add", json!({"path": path.to_string_lossy(), "x": x, "y": y, "w": w, "h": h, "wrap": "BehindText"}));
                return Ok(());
            }
            let stops: Vec<Value> = g.stops.iter().map(|s| json!({"pos": s.pos, "color": s.color, "alpha": s.alpha})).collect();
            fill = json!({"gradient": {"angle": (180.0 - g.css_angle).rem_euclid(360.0), "stops": stops}});
        } else if let Some((col, a)) = rgba(sv(n, "background-color")).filter(|c| c.1 > 0.001) {
            let rgb = if a < 0.999 {
                // 반투명은 아래 색과 섞어 단색으로(한글은 반투명을 무늬로 찍었다)
                let (under, top) = (self.backdrop(b), hexrgb(&col));
                self.rep.note("alpha_blended", "반투명 바탕 — 아래 색과 섞은 단색으로 넣음");
                [0, 1, 2].map(|i| (top[i] as f64 * a + under[i] as f64 * (1.0 - a)).round() as u8)
            } else {
                hexrgb(&col)
            };
            fill = json!({"color": format!("{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]), "alpha": 1});
            self.under.push(Under::Rgb(rgb));
        }
        // 배경 그림만 글 뒤에 깔고, 나머지 꾸밈 도형은 그 위(글 앞)에 둔다 — 같은 "글 뒤"끼리는 그림이 도형을 덮었다
        let mut p = json!({"kind": "rectangle", "x": x, "y": y, "w": w, "h": h, "wrap": "InFrontOfText", "line": line, "fill": fill});
        if rad > 0.5 {
            p["round"] = json!(((rad / w.min(h).max(1.0)) * 100.0).round().min(50.0) as i64);
        }
        let has_line = !p["line"].is_null();
        self.c("shape.add", p);
        if !has_line {
            for (s, (col, lw)) in sd {
                // 한 면만 있는 테두리는 선으로
                let seg = match s {
                    "top" => (x, y, w, 0.0),
                    "bottom" => (x, y + h, w, 0.0),
                    "left" => (x, y, 0.0, h),
                    _ => (x + w, y, 0.0, h),
                };
                self.c("shape.add", json!({"kind": "line", "x": seg.0, "y": seg.1, "w": seg.2.max(0.1), "h": seg.3.max(0.1),
                    "wrap": "InFrontOfText", "line": {"color": col, "width": lw}}));
            }
        }
        Ok(())
    }

    fn textbox(&mut self, n: &Value, runs: &[(String, Option<&Value>)]) {
        let b = bx(n);
        let t = text_bounds(n).unwrap_or(b);
        let (fx, fw, _) = text_frame(n, b, t);
        self.c("shape.add", json!({"kind": "textbox", "x": fx * PX, "y": t.y * PX, "w": fw * PX, "h": t.h * PX * 1.3,
            "wrap": "InFrontOfText", "line": null, "fill": null, "margin": [0, 0, 0, 0]}));
        let mut para = json!({"alignment": align_word(n)});
        let lh = sv(n, "line-height");
        if lh.ends_with("px") {
            para["lineSpacingType"] = json!("Fixed");
            para["lineSpacing"] = json!((px(lh) * PX * 100.0).round() as i64 * FIXED_X as i64); // 짝수로(emit::para_props 참고)
        }
        let mut pending = "";
        for (text, src) in runs {
            let Some(s) = src else {
                pending = "\n";
                continue;
            };
            let props = json!({"fontName": FONT, "fontSize": (spx(s, "font-size") * PX * 100.0).round() as i64,
                "textColor": format!("#{}", rgba(sv(s, "color")).map(|c| c.0).unwrap_or_else(|| "000000".into())),
                "bold": weight(s), "useFontSpace": true});
            self.c("shape.text", json!({"text": format!("{pending}{text}"), "props": props, "paraProps": para}));
            pending = "";
        }
    }

    fn walk(&mut self, n: &Value) -> Result<(), String> {
        if tag(n) == "#text" {
            return Ok(());
        }
        let depth = self.under.len();
        let r = self.walk_in(n);
        self.under.truncate(depth); // 이 상자의 바탕은 그 안쪽에만 깔린다
        r
    }

    fn walk_in(&mut self, n: &Value) -> Result<(), String> {
        if decorated(n) {
            self.shape(n)?;
        }
        if kids(n).iter().any(|k| tag(k) == "#text") {
            let mut runs = Vec::new();
            runs_of(n, &mut runs, self.rep);
            if runs.iter().any(|(t, _)| !t.trim().is_empty()) {
                self.textbox(n, &runs);
            }
            for k in kids(n) {
                if tag(k) != "#text" && is_block(k) {
                    self.walk(k)?;
                }
            }
            return Ok(());
        }
        for k in kids(n) {
            if MEDIA.contains(&tag(k)) {
                self.rep.skip(tag(k));
            }
            self.walk(k)?;
        }
        Ok(())
    }
}

pub fn hwpx(root: &Value, dir: &std::path::Path, rep: &mut Report) -> Result<Cmds, String> {
    let mut g = HwpxGen { cmds: Vec::new(), under: Vec::new(), pics: 0, dir: dir.to_path_buf(), rep };
    // 도형·글상자는 쪽 기준 자리라 빈 양식(A4)의 여백을 그대로 둔다
    g.walk(root)?;
    Ok(g.cmds)
}

/// 그러데이션 → PNG(96dpi 화소 — 한글은 그림을 화소 크기 기준으로 다루므로 쪽 크기와 맞춘다)
fn gradient_png(g: &Gradient, w_pt: f64, h_pt: f64, path: &std::path::Path) -> Result<(), String> {
    let (w, h) = (((w_pt * 96.0 / 72.0).round() as u32).max(1), ((h_pt * 96.0 / 72.0).round() as u32).max(1));
    let vertical = g.vertical();
    let a = g.css_angle.rem_euclid(360.0);
    // 180°(위→아래)·90°(왼→오)가 정방향, 0°·270°는 거꾸로
    let reverse = if vertical { a < 90.0 || a > 270.0 } else { a > 180.0 };
    let n = if vertical { h } else { w };
    let line: Vec<[u8; 3]> = (0..n)
        .map(|i| {
            let f = i as f64 / (n.max(2) - 1) as f64;
            g.color_at(if reverse { 1.0 - f } else { f })
        })
        .collect();
    let mut data = Vec::with_capacity((w * h * 3) as usize);
    for y in 0..h {
        for x in 0..w {
            data.extend_from_slice(&line[if vertical { y } else { x } as usize]);
        }
    }
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    wr.write_image_data(&data).map_err(|e| e.to_string())?;
    Ok(())
}
