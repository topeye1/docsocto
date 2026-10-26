//! Edge 덤프(html_dump.js) 마디 읽기 도우미 — 최종 스타일 값(CSS px·rgb()) 풀기.

use serde_json::Value;

/// CSS px → pt
pub const PX: f64 = 0.75;
/// mm → pt
pub const MM: f64 = 72.0 / 25.4;

static EMPTY: Vec<Value> = Vec::new();

/// 상자(px, 문서 기준)
#[derive(Clone, Copy, Debug, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn rect(v: &Value) -> Rect {
    let f = |k: &str| v.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    Rect { x: f("x"), y: f("y"), w: f("w"), h: f("h") }
}

pub fn bx(n: &Value) -> Rect {
    rect(&n["box"])
}

pub fn tag(n: &Value) -> &str {
    n["tag"].as_str().unwrap_or("")
}

pub fn kids(n: &Value) -> &Vec<Value> {
    n["kids"].as_array().unwrap_or(&EMPTY)
}

pub fn sv<'a>(n: &'a Value, k: &str) -> &'a str {
    n["style"][k].as_str().unwrap_or("")
}

/// "12.5px" → 12.5 (px 가 아니면 0)
pub fn px(v: &str) -> f64 {
    v.strip_suffix("px").and_then(|s| s.trim().parse().ok()).unwrap_or(0.0)
}

pub fn spx(n: &Value, k: &str) -> f64 {
    px(sv(n, k))
}

/// "rgb(1, 2, 3)"·"rgba(1, 2, 3, 0.5)" → ("010203", 알파)
pub fn rgba(s: &str) -> Option<(String, f64)> {
    let s = s.trim();
    let start = s.find('(')?;
    if !s.starts_with("rgb") {
        return None;
    }
    let body = &s[start + 1..s.find(')')?];
    let v: Vec<f64> = body.replace('/', ",").split(|c| c == ',' || c == ' ').filter(|t| !t.is_empty()).filter_map(|t| t.parse().ok()).collect();
    if v.len() < 3 {
        return None;
    }
    let a = v.get(3).copied().unwrap_or(1.0);
    Some((format!("{:02X}{:02X}{:02X}", v[0].round() as u8, v[1].round() as u8, v[2].round() as u8), a))
}

/// 보이는 바탕색(알파가 0이 아닌 것)
pub fn bg(n: &Value) -> Option<String> {
    rgba(sv(n, "background-color")).filter(|c| c.1 > 0.001).map(|c| c.0)
}

/// 보이는 테두리 한 면: (색, pt)
pub fn side(n: &Value, s: &str) -> Option<(String, f64)> {
    let w = spx(n, &format!("border-{s}-width"));
    let st = sv(n, &format!("border-{s}-style"));
    if w <= 0.0 || st == "none" || st == "hidden" {
        return None;
    }
    rgba(sv(n, &format!("border-{s}-color"))).filter(|c| c.1 > 0.001).map(|c| (c.0, (w * PX * 100.0).round() / 100.0))
}

pub const SIDES: [&str; 4] = ["top", "right", "bottom", "left"];

/// 네 면이 같은 테두리면 (색, pt)
pub fn box_border(n: &Value) -> Option<(String, f64)> {
    let s: Vec<_> = SIDES.iter().map(|k| side(n, k)).collect();
    match &s[0] {
        Some(first) if s.iter().all(|x| x.as_ref() == Some(first)) => Some(first.clone()),
        _ => None,
    }
}

/// 보이는 테두리 면 목록
pub fn sides(n: &Value) -> Vec<(&'static str, (String, f64))> {
    SIDES.iter().filter_map(|k| side(n, k).map(|v| (*k, v))).collect()
}

pub fn pads(n: &Value) -> [f64; 4] {
    [spx(n, "padding-top") * PX, spx(n, "padding-right") * PX, spx(n, "padding-bottom") * PX, spx(n, "padding-left") * PX]
}

/// 그러데이션 멈춤점
#[derive(Clone, Debug)]
pub struct Stop {
    pub pos: f64,
    pub color: String,
    pub alpha: f64,
}

/// linear-gradient — angle 은 CSS 각도(0=아래→위, 180=위→아래)
#[derive(Clone, Debug)]
pub struct Gradient {
    pub css_angle: f64,
    pub stops: Vec<Stop>,
}

pub fn gradient(s: &str) -> Option<Gradient> {
    let i = s.find("linear-gradient(")?;
    let body = &s[i + "linear-gradient(".len()..s.rfind(')')?];
    let mut angle = 180.0;
    let first = body.split(',').next().unwrap_or("").trim();
    if let Some(d) = first.strip_suffix("deg") {
        angle = d.trim().parse().unwrap_or(180.0);
    } else if let Some(dir) = first.strip_prefix("to ") {
        angle = match dir.trim() {
            "top" => 0.0,
            "right" => 90.0,
            "left" => 270.0,
            _ => 180.0,
        };
    }
    let mut raw = Vec::new();
    let mut rest = body;
    while let Some(p) = rest.find("rgb") {
        rest = &rest[p..];
        let end = rest.find(')').unwrap_or(rest.len() - 1);
        let col = &rest[..=end];
        rest = &rest[end + 1..];
        let pos = rest.trim_start().split(|c| c == ',' || c == ')').next().unwrap_or("").trim();
        let pos = pos.strip_suffix('%').and_then(|v| v.trim().parse::<f64>().ok());
        if let Some((c, a)) = rgba(col) {
            raw.push((c, a, pos));
        }
    }
    if raw.is_empty() {
        return None;
    }
    let n = raw.len();
    let stops = raw
        .into_iter()
        .enumerate()
        .map(|(i, (color, alpha, pos))| Stop { pos: pos.unwrap_or(100.0 * i as f64 / (n.max(2) - 1) as f64), color, alpha })
        .collect();
    Some(Gradient { css_angle: angle, stops })
}

pub fn hexrgb(h: &str) -> [u8; 3] {
    let v = |i: usize| u8::from_str_radix(h.get(i..i + 2).unwrap_or("00"), 16).unwrap_or(0);
    [v(0), v(2), v(4)]
}

impl Gradient {
    /// frac(0~1, 그러데이션 진행 방향) 자리의 색
    pub fn color_at(&self, frac: f64) -> [u8; 3] {
        let mut st = self.stops.clone();
        st.sort_by(|a, b| a.pos.partial_cmp(&b.pos).unwrap());
        let p = frac * 100.0;
        for w in st.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if a.pos <= p && p <= b.pos {
                let t = (p - a.pos) / (b.pos - a.pos).max(1e-6);
                let (ca, cb) = (hexrgb(&a.color), hexrgb(&b.color));
                return [0, 1, 2].map(|i| (ca[i] as f64 + (cb[i] as f64 - ca[i] as f64) * t).round() as u8);
            }
        }
        hexrgb(if p > st.last().unwrap().pos { &st.last().unwrap().color } else { &st[0].color })
    }

    /// 세로(위↔아래) 진행인가
    pub fn vertical(&self) -> bool {
        let a = self.css_angle.rem_euclid(180.0);
        (a - 0.0).abs() < 45.0 || (a - 180.0).abs() < 45.0
    }
}
