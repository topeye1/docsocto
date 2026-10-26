//! 요청형 변환(기획서 3절 기능 8, M4): HTML → docx·HWPX.
//!
//! HTML 은 Edge 가 그린 결과(최종 스타일·자리)를 받아 만든다(결정 #33).
//! - 흐름 방식([`flow`]): 문단·표 블록으로 바꿔 글이 쪽을 넘겨 흐른다 — 여러 쪽 본문.
//! - 자리 고정 방식([`fixed`]): 꾸민 상자는 도형, 글은 글상자로 Edge 가 그린 자리에 둔다 — 표지처럼 한 쪽에 짜인 화면.
//! 블록 목록은 docx·HWPX 출력부([`emit`])가 함께 쓴다. 옮기지 못하거나 바꿔 그린 것은 변환 보고서([`Report`])에 적는다.

pub mod css;
pub mod emit;
pub mod fixed;
pub mod flow;
pub mod html;

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{json, Value};

use crate::Format;

/// 변환 보고서 — 옮기지 못한 것·바꿔 그린 것
#[derive(Default, Debug)]
pub struct Report {
    notes: BTreeMap<String, (String, usize)>,
    skipped: BTreeMap<String, usize>,
}

impl Report {
    pub fn note(&mut self, key: &str, msg: &str) {
        self.notes.entry(key.to_string()).or_insert_with(|| (msg.to_string(), 0)).1 += 1;
    }

    /// 옮기지 않은 요소(태그)
    pub fn skip(&mut self, tag: &str) {
        *self.skipped.entry(tag.to_string()).or_insert(0) += 1;
    }

    pub fn to_json(&self) -> Value {
        json!({
            "changed": self.notes.iter().map(|(k, (m, n))| json!({"id": k, "what": m, "count": n})).collect::<Vec<_>>(),
            "not_converted": self.skipped.iter().map(|(t, n)| json!({"element": t, "count": n})).collect::<Vec<_>>(),
        })
    }
}

/// 변환 방식
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Auto,
    Flow,
    Fixed,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s {
            "auto" => Ok(Mode::Auto),
            "flow" => Ok(Mode::Flow),
            "fixed" => Ok(Mode::Fixed),
            _ => Err(format!("--mode {s}? (auto|flow|fixed)")),
        }
    }
}

const PX_PER_MM: f64 = 96.0 / 25.4;

/// 자리 고정이 어울리는 화면인가: 절대 자리 요소나 그러데이션 바탕이 있다
fn wants_fixed(n: &Value) -> bool {
    matches!(css::sv(n, "position"), "absolute" | "fixed")
        || css::gradient(css::sv(n, "background-image")).is_some()
        || css::kids(n).iter().any(wants_fixed)
}

/// 결과: (변환 명령, 쓴 방식, 보고서, 그러데이션 그림 임시 폴더)
pub struct Plan {
    pub cmds: emit::Cmds,
    pub mode: Mode,
    pub report: Report,
    pub pages_hint: f64,
}

/// HTML → 엔진 명령 목록
pub fn plan_html(src: &Path, to: Format, mode: Mode, tmp: &Path) -> Result<Plan, String> {
    let mut rep = Report::default();
    // 1차: 쪽 폭 전체(A4 794px)로 그려 @page 규칙을 읽는다
    let d0 = html::dump(src, 794)?;
    let (pw, ph, m) = html::page_rule(&d0);
    let content_w = ((pw - m[1] - m[3]) * PX_PER_MM).round().max(100.0) as u32;
    let d = if content_w == 794 { d0 } else { html::dump(src, content_w)? };
    let content_h = (ph - m[0] - m[2]) * PX_PER_MM;
    let doc_h = d["page"]["h"].as_f64().unwrap_or(0.0);
    let mode = match mode {
        Mode::Auto if wants_fixed(&d["root"]) && doc_h <= content_h * 1.02 => Mode::Fixed,
        Mode::Auto => Mode::Flow,
        m => m,
    };
    let page = emit::Page { w: pw * css::MM, h: ph * css::MM, margin: m.map(|v| v * css::MM) };
    let hwpx = matches!(to, Format::Hwpx);
    let cmds = match mode {
        Mode::Fixed => {
            if doc_h > content_h * 1.02 {
                rep.note("fixed_overflow", "자리 고정 방식인데 내용이 한 쪽보다 깁니다 — 넘친 부분은 쪽 밖에 놓임");
            }
            let mut root = d["root"].clone();
            html::shift(&mut root, m[3] * PX_PER_MM, m[0] * PX_PER_MM);
            if hwpx {
                fixed::hwpx(&root, tmp, &mut rep)?
            } else {
                fixed::docx(&root, &mut rep)
            }
        }
        _ => {
            let blocks = flow::blocks(&d["root"], &mut rep);
            emit::emit(&blocks, hwpx, page)
        }
    };
    Ok(Plan { cmds, mode, report: rep, pages_hint: doc_h / content_h })
}

fn blank(to: Format) -> Result<Vec<u8>, String> {
    match to {
        Format::Hwpx => octodocs_ohwp::blank_hwpx(),
        Format::Docx => wordcraft_engine::io::save_bytes("new.docx", &wordcraft_doc::Document::new()),
        _ => Err("변환 결과 형식은 docx·hwpx 만 됩니다".into()),
    }
}

/// 변환해 out 에 쓴다. 보고서(JSON)를 돌려준다.
pub fn convert(src: &str, out: &str, mode: Mode) -> Result<Value, String> {
    let to = Format::from_path(out).filter(|f| matches!(f, Format::Docx | Format::Hwpx)).ok_or("결과 파일은 .docx 또는 .hwpx 로 주십시오")?;
    let ext = src.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    if !matches!(ext.as_str(), "html" | "htm") {
        return Err(format!("{src}: 지금은 HTML(.html·.htm) 원본만 변환합니다"));
    }
    let tmp = std::env::temp_dir().join(format!("octodocs_convert_{}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let r = (|| {
        let mut plan = plan_html(Path::new(src), to, mode, &tmp)?;
        let source = std::fs::read_to_string(src).unwrap_or_default().to_ascii_lowercase();
        if source.contains("<script") {
            plan.report.note("script", "스크립트 — 옮기지 않음(그려진 결과만 옮김)");
        }
        if source.contains("@keyframes") || source.contains("animation:") || source.contains("transition:") {
            plan.report.note("animation", "애니메이션·전환 효과 — 옮기지 않음");
        }
        let mut e = crate::open(&format!("new.{}", to.name()), blank(to)?)?;
        for (i, (id, p)) in plan.cmds.iter().enumerate() {
            crate::run(e.as_mut(), id, p).map_err(|err| format!("명령 {i} {id}: {err}"))?;
        }
        let (bytes, _) = e.save()?;
        std::fs::write(out, bytes).map_err(|err| format!("{out}: {err}"))?;
        let mut rep = plan.report.to_json();
        rep["source"] = json!(src);
        rep["output"] = json!(out);
        rep["mode"] = json!(match plan.mode {
            Mode::Fixed => "fixed",
            _ => "flow",
        });
        rep["commands"] = json!(plan.cmds.len());
        rep["pages_in_edge"] = json!((plan.pages_hint * 100.0).round() / 100.0);
        Ok(rep)
    })();
    let _ = std::fs::remove_dir_all(&tmp);
    r
}

/// 명령 목록만 JSON(octodocs run --script 형식)으로 — 시험·확인용
pub fn script_json(cmds: &emit::Cmds) -> Value {
    json!({"commands": cmds.iter().map(|(c, p)| json!({"command": c, "params": p})).collect::<Vec<_>>()})
}
