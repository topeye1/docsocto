//! docsocto 명령줄 — 사용법은 `docsocto` (인자 없이) 또는 i18n::USAGE.

use std::process::ExitCode;

use docsocto::{i18n::t, settings, Format};
use serde_json::{json, Value};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("init") => init(&args[1..]),
        Some("commands") => commands(&args[1..]),
        Some("inspect") if args.len() == 2 => docsocto::inspect(&args[1]).map(|v| println!("{}", pretty(&v))),
        Some("run") => run(&args[1..]),
        Some("new") if args.len() >= 2 => docsocto::new_document(&args[1], args.iter().any(|a| a == "--overwrite")).map(|p| println!("{p}")),
        Some("render") => render(&args[1..]),
        Some("mcp") => {
            docsocto::mcp::Server::new().serve();
            Ok(())
        }
        Some("--version") => {
            println!("docsocto {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        _ => Err(t("usage")),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(if e == t("usage") { 2 } else { 1 })
        }
    }
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

fn opt(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

fn init(args: &[String]) -> Result<(), String> {
    let mut s = settings::load();
    let def = opt(args, "--default");
    let lang = opt(args, "--lang");
    if def.is_none() && lang.is_none() {
        if s.default_format.is_none() {
            return Err(t("init_choose"));
        }
        println!("{}", pretty(&json!({"settings": s, "path": settings::path()})));
        return Ok(());
    }
    if let Some(d) = def {
        s.default_format = Some(settings::check_default_format(&d)?);
    }
    if let Some(l) = lang {
        if l != "ko" && l != "en" {
            return Err("--lang 은 ko 또는 en".into());
        }
        s.language = Some(l);
    }
    settings::save(&s)?;
    println!("{}: {}", t("init_done"), settings::path().display());
    Ok(())
}

fn commands(args: &[String]) -> Result<(), String> {
    let as_json = args.iter().any(|a| a == "--json");
    let file = args.iter().find(|a| !a.starts_with("--"));
    let ko = docsocto::i18n::lang() == "ko";
    let native = match file {
        Some(f) => {
            let data = std::fs::read(f).map_err(|e| format!("{f}: {e}"))?;
            docsocto::open(f, data)?.native_commands()
        }
        None => Vec::new(),
    };
    if as_json {
        println!(
            "{}",
            pretty(&json!({
                "common": docsocto::COMMON.iter().map(|c| json!({"id": c.0, "ko": c.1, "en": c.2, "params": c.3})).collect::<Vec<_>>(),
                "native": native.iter().map(|c| json!({"id": c.0, "ko": c.1, "en": c.2})).collect::<Vec<_>>(),
            }))
        );
    } else {
        for c in docsocto::COMMON {
            println!("{:<20} {:<14} {}", c.0, if ko { c.1 } else { c.2 }, c.3);
        }
        if !native.is_empty() {
            println!("\n{}", if ko { "— 이 형식 고유 명령 —" } else { "— commands of this format —" });
            for c in &native {
                println!("{:<24} {}", c.0, if ko { &c.1 } else { &c.2 });
            }
        }
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut cmds: Vec<(String, Value)> = Vec::new();
    let (mut save, mut overwrite) = (None, false);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cmd" => {
                i += 1;
                let c = args.get(i).ok_or("--cmd 뒤에 'id={json}' 이 필요합니다")?;
                let (id, js) = c.split_once('=').unwrap_or((c.as_str(), "{}"));
                cmds.push((id.trim().to_string(), serde_json::from_str(js).map_err(|e| format!("{id} JSON: {e}"))?));
            }
            "--script" => {
                i += 1;
                let path = args.get(i).ok_or("--script 뒤에 JSON 파일 경로가 필요합니다")?;
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path}: {e}"))?;
                let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{path}: {e}"))?;
                for c in v.get("commands").and_then(Value::as_array).ok_or("commands 배열이 필요합니다")? {
                    match c {
                        Value::String(id) => cmds.push((id.clone(), json!({}))),
                        _ => cmds.push((
                            c.get("command").and_then(Value::as_str).ok_or("command 가 필요합니다")?.to_string(),
                            c.get("params").cloned().unwrap_or_else(|| json!({})),
                        )),
                    }
                }
            }
            "--save" => {
                i += 1;
                save = Some(args.get(i).cloned().unwrap_or_default());
            }
            "--overwrite" => overwrite = true,
            a if a.starts_with("--") => return Err(format!("{a}?\n{}", t("usage"))),
            a => input = Some(a.to_string()),
        }
        i += 1;
    }
    let input = input.ok_or_else(|| t("usage"))?;
    let data = std::fs::read(&input).map_err(|e| format!("{input}: {e}"))?;
    let mut e = docsocto::open(&input, data)?;
    for (id, p) in &cmds {
        let v = docsocto::run(e.as_mut(), id, p).map_err(|err| format!("{id}: {err}"))?;
        println!("{}", json!({"command": id, "result": v}));
    }
    if let Some(out) = save {
        let out = if out.is_empty() { docsocto::default_save_path(&input, e.format()) } else { out };
        if std::path::Path::new(&out).exists() && !overwrite {
            return Err(format!("{out}: {}", t("exists")));
        }
        let (bytes, report) = e.save()?;
        std::fs::write(&out, bytes).map_err(|err| format!("{out}: {err}"))?;
        if report.get("whole_engine").and_then(Value::as_bool).unwrap_or(false) {
            eprintln!("{}", t("whole_engine"));
        }
        eprintln!("{}: {out}", t("saved"));
    }
    Ok(())
}

fn render(args: &[String]) -> Result<(), String> {
    let file = args.iter().find(|a| !a.starts_with("--")).ok_or_else(|| t("usage"))?;
    let page: usize = opt(args, "--page").and_then(|p| p.parse().ok()).ok_or("--page <N>(1부터)")?;
    let out = opt(args, "--out").ok_or("--out <그림.svg|pdf>")?;
    match Format::from_path(file) {
        Some(Format::Hwpx) | Some(Format::Hwp) => {
            let data = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let mut s = docsocto_ohwp::Session::open(data)?;
            s.run("render.page", &json!({"page": page.saturating_sub(1), "path": out}))?;
            println!("{out}");
            Ok(())
        }
        // docx: WordCraft 내보내기로 문서 전체를 PDF로 낸다(쪽 하나만 고르는 것은 HWPX만)
        Some(Format::Docx) => {
            if !out.to_ascii_lowercase().ends_with(".pdf") {
                return Err("docx 는 --out 에 .pdf 를 주십시오(문서 전체를 PDF로 냅니다)".into());
            }
            let data = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
            let doc = wordcraft_engine::io::open_bytes(file, &data)?;
            let bytes = wordcraft_engine::io::save_bytes(&out, &doc)?;
            std::fs::write(&out, bytes).map_err(|e| format!("{out}: {e}"))?;
            println!("{out}");
            Ok(())
        }
        _ => Err("render 는 HWPX·docx만 됩니다".into()),
    }
}
