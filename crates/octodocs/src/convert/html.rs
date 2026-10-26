//! HTML → Edge 덤프(결정 #33): Edge(headless)가 그린 요소별 최종 스타일·자리, 글줄별 글자·자리를 JSON으로 받는다.
//! 원본은 건드리지 않는다 — 같은 폴더에 사본을 만들어(상대 경로 그림·CSS 가 그대로 보이게) 덤프 스크립트를 붙인다.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde_json::Value;

const DUMP_JS: &str = include_str!("../../../../tools/m4/html_dump.js");

fn edge() -> Result<PathBuf, String> {
    if let Ok(p) = std::env::var("OCTODOCS_EDGE") {
        return Ok(PathBuf::from(p));
    }
    [
        r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
    .ok_or_else(|| "Edge(msedge.exe)를 찾지 못했습니다 — OCTODOCS_EDGE 로 경로를 줄 수 있습니다".into())
}

/// 파일 경로 → file:/// URL (한글·빈칸은 %인코딩)
fn file_url(p: &Path) -> String {
    // canonicalize 는 \\?\E:\… 꼴(확장 길이 경로)을 준다 — 앞머리를 떼야 Edge 가 연다
    let raw = p.to_string_lossy();
    let raw = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let s = raw.replace('\\', "/");
    let mut out = String::from("file:///");
    for b in s.trim_start_matches('/').bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b':' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&amp;", "&")
}

/// HTML 을 폭 width_px 로 그려 덤프를 받는다
pub fn dump(html: &Path, width_px: u32) -> Result<Value, String> {
    let src = std::fs::canonicalize(html).map_err(|e| format!("{}: {e}", html.display()))?;
    let body = std::fs::read(&src).map_err(|e| format!("{}: {e}", src.display()))?;
    let body = String::from_utf8_lossy(body.strip_prefix(b"\xef\xbb\xbf").unwrap_or(&body)).into_owned();
    let dir = src.parent().ok_or("원본 폴더?")?;
    let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("doc.html");
    let tmp = dir.join(format!(".octodocs_dump_{}_{name}", std::process::id()));
    // 그릴 폭은 창 크기에 맡기지 않고 html 폭으로 못박는다 — 같은 --window-size 에서도 Edge 가 본문을
    // 39px 좁게 그린 일이 있었다(680 → 641)
    let fix = format!("<style>html{{width:{width_px}px!important;min-width:{width_px}px!important;max-width:{width_px}px!important}}</style>");
    let page = format!("{body}\n{fix}\n<script>window.addEventListener('load',function(){{{DUMP_JS}}});</script>");
    std::fs::write(&tmp, page).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let r = run_edge(&tmp, width_px);
    if std::env::var_os("OCTODOCS_DEBUG").is_none() {
        let _ = std::fs::remove_file(&tmp);
    }
    let dom = r?;
    let tag = "<pre id=\"octodocs-dump\">";
    // 스크립트 글에도 같은 꼬리표가 있으니 마지막 것을 쓴다
    let i = dom.rfind(tag).ok_or("Edge 결과에 덤프가 없습니다(스크립트가 돌지 않음)")?;
    let rest = &dom[i + tag.len()..];
    let json = unescape(&rest[..rest.find("</pre>").ok_or("덤프 끝이 없습니다")?]);
    serde_json::from_str(&json).map_err(|e| format!("덤프 JSON: {e}"))
}

fn run_edge(page: &Path, width_px: u32) -> Result<String, String> {
    let prof = std::env::temp_dir().join("octodocs_edge_profile");
    // msedge 는 자식 프로세스에 일을 넘기고 먼저 끝난다. 표준 출력을 Rust 에서 파일로 넘기면 덤프가 오지 않았다 —
    // html_dump.ps1 처럼 cmd 리다이렉트로 받고, 덤프가 다 써질 때까지(최대 60초) 기다린다
    let out_path = std::env::temp_dir().join(format!("octodocs_dom_{}_{width_px}.txt", std::process::id()));
    let _ = std::fs::remove_file(&out_path);
    let line = format!(
        "\"\"{}\" --headless=new --disable-gpu --no-first-run \"--user-data-dir={}\" --window-size={width_px},1123 --virtual-time-budget=5000 --dump-dom \"{}\" > \"{}\" 2> nul\"",
        edge()?.display(),
        prof.display(),
        file_url(page),
        out_path.display()
    );
    let mut cmd = Command::new("cmd");
    {
        use std::os::windows::process::CommandExt;
        cmd.arg("/c").raw_arg(&line);
    }
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e| format!("Edge 실행: {e}"))?;
    let start = std::time::Instant::now();
    let mut dom = String::new();
    while start.elapsed() < Duration::from_secs(60) {
        if let Ok(b) = std::fs::read(&out_path) {
            dom = String::from_utf8_lossy(&b).into_owned();
            if dom.contains("</html>") {
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(300));
    }
    let _ = child.wait();
    if std::env::var_os("OCTODOCS_DEBUG").is_some() {
        eprintln!("edge: {line}
출력 {} 바이트: {}", dom.len(), out_path.display());
    } else {
        let _ = std::fs::remove_file(&out_path);
    }
    if !dom.contains("</html>") {
        return Err("Edge 가 60초 안에 덤프를 내지 않았습니다".into());
    }
    Ok(dom)
}

/// CSS 길이 → mm
fn len_mm(v: &str) -> Option<f64> {
    let v = v.trim();
    let num = |s: &str| s.trim().parse::<f64>().ok();
    if v == "0" {
        return Some(0.0);
    }
    for (u, k) in [("mm", 1.0), ("cm", 10.0), ("in", 25.4), ("pt", 25.4 / 72.0), ("px", 25.4 / 96.0)] {
        if let Some(s) = v.strip_suffix(u) {
            return num(s).map(|x| x * k);
        }
    }
    None
}

/// @page 규칙 → (쪽 폭 mm, 쪽 높이 mm, [위, 오른쪽, 아래, 왼쪽] mm). 없으면 A4·여백 10mm(Edge 기본 인쇄 여백 근사).
pub fn page_rule(d: &Value) -> (f64, f64, [f64; 4]) {
    let r = &d["pageRule"];
    let s = |k: &str| r[k].as_str().unwrap_or("");
    let (mut w, mut h) = (210.0, 297.0);
    let size = s("size").to_ascii_lowercase();
    let named = |n: &str| match n {
        "a3" => Some((297.0, 420.0)),
        "a4" => Some((210.0, 297.0)),
        "a5" => Some((148.0, 210.0)),
        "b4" => Some((250.0, 353.0)),
        "b5" => Some((176.0, 250.0)),
        "letter" => Some((215.9, 279.4)),
        "legal" => Some((215.9, 355.6)),
        _ => None,
    };
    let toks: Vec<&str> = size.split_whitespace().collect();
    if let Some((a, b)) = toks.iter().find_map(|t| named(t)) {
        (w, h) = (a, b);
    } else if toks.len() >= 2 {
        if let (Some(a), Some(b)) = (len_mm(toks[0]), len_mm(toks[1])) {
            (w, h) = (a, b);
        }
    }
    if toks.contains(&"landscape") {
        (w, h) = (h, w);
    }
    let m = |k: &str| len_mm(s(k)).unwrap_or(10.0);
    (w, h, [m("margin-top"), m("margin-right"), m("margin-bottom"), m("margin-left")])
}

/// 덤프의 모든 자리(상자·글줄)를 (dx, dy) px 만큼 옮긴다 — 자리 고정 방식에서 쪽 여백만큼
pub fn shift(n: &mut Value, dx: f64, dy: f64) {
    let mv = |b: &mut Value| {
        if let Some(x) = b.get("x").and_then(Value::as_f64) {
            b["x"] = (x + dx).into();
        }
        if let Some(y) = b.get("y").and_then(Value::as_f64) {
            b["y"] = (y + dy).into();
        }
    };
    if let Some(b) = n.get_mut("box") {
        mv(b);
    }
    if let Some(ls) = n.get_mut("lines").and_then(Value::as_array_mut) {
        for l in ls {
            if let Some(b) = l.get_mut("box") {
                mv(b);
            }
        }
    }
    if let Some(ks) = n.get_mut("kids").and_then(Value::as_array_mut) {
        for k in ks {
            shift(k, dx, dy);
        }
    }
}
