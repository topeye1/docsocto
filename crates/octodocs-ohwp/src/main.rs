//! ohwp — Octo Docs HWPX 엔진 명령줄
//!
//!   ohwp commands [--json]                      명령 목록
//!   ohwp run <입력> [--script <명령.json>] --cmd 'id={json}' … [--save <출력>] [--engine-save] [--report <r.json>]
//!                                               명령을 차례로 실행하고 결과를 JSON 줄로 낸다.
//!                                               --save 는 보존 저장, --engine-save 는 엔진 결과 그대로.
//!   ohwp resave <입력> <출력>                   고치지 않고 다시 저장(보존 저장 비교용)

use std::process::ExitCode;

use serde_json::{json, Value};

const USAGE: &str = "사용법:
  ohwp commands [--json]
  ohwp run <입력> [--script <명령.json>] --cmd 'id={json}' … [--save <출력>] [--engine-save] [--report <보고서.json>]
  ohwp resave <입력> <출력>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("commands") => {
            list(args.iter().any(|a| a == "--json"));
            Ok(())
        }
        Some("resave") if args.len() == 3 => std::fs::read(&args[1])
            .map_err(|e| format!("{} 읽기 실패: {e}", args[1]))
            .and_then(|d| octodocs_ohwp::resave_hwpx(&d))
            .and_then(|b| std::fs::write(&args[2], b).map_err(|e| e.to_string())),
        Some("run") => run(&args[1..]),
        _ => Err(USAGE.to_string()),
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(if e == USAGE { 2 } else { 1 })
        }
    }
}

fn list(as_json: bool) {
    let cmds = octodocs_ohwp::commands();
    if as_json {
        let v: Vec<Value> = cmds.iter().map(|c| json!({"id": c.id, "ko": c.ko, "en": c.en, "params": c.params})).collect();
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    } else {
        for c in cmds {
            println!("{:<22} {:<12} {}", c.id, c.ko, c.params);
        }
    }
}

fn run(args: &[String]) -> Result<(), String> {
    let mut input = None;
    let mut cmds: Vec<(String, Value)> = Vec::new();
    let (mut save, mut engine_save, mut report) = (None, false, None);
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--cmd" => {
                i += 1;
                let c = args.get(i).ok_or("--cmd 뒤에 'id={json}'이 필요합니다")?;
                let (id, p) = match c.split_once('=') {
                    Some((id, js)) => (id.trim().to_string(), serde_json::from_str(js).map_err(|e| format!("{id} 매개변수 JSON 오류: {e}"))?),
                    None => (c.trim().to_string(), json!({})),
                };
                cmds.push((id, p));
            }
            "--script" => {
                // {"commands":[{"command":"id","params":{…}} | "id", …]} — 셸 따옴표 문제 없이 넘길 때
                i += 1;
                let path = args.get(i).ok_or("--script 뒤에 JSON 파일 경로가 필요합니다")?;
                let text = std::fs::read_to_string(path).map_err(|e| format!("{path} 읽기 실패: {e}"))?;
                let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|e| format!("{path} JSON 오류: {e}"))?;
                for c in v.get("commands").and_then(Value::as_array).ok_or("script 에 commands 배열이 필요합니다")? {
                    match c {
                        Value::String(id) => cmds.push((id.clone(), json!({}))),
                        _ => cmds.push((
                            c.get("command").and_then(Value::as_str).ok_or("각 명령에 command 가 필요합니다")?.to_string(),
                            c.get("params").cloned().unwrap_or_else(|| json!({})),
                        )),
                    }
                }
            }
            "--save" => {
                i += 1;
                save = Some(args.get(i).ok_or("--save 뒤에 출력 경로가 필요합니다")?.clone());
            }
            "--engine-save" => engine_save = true,
            "--report" => {
                i += 1;
                report = Some(args.get(i).ok_or("--report 뒤에 경로가 필요합니다")?.clone());
            }
            a if a.starts_with('-') => return Err(format!("모르는 옵션: {a}\n{USAGE}")),
            a => {
                if input.replace(a.to_string()).is_some() {
                    return Err("입력 파일은 하나만 줄 수 있습니다".into());
                }
            }
        }
        i += 1;
    }
    let input = input.ok_or(USAGE)?;
    let data = std::fs::read(&input).map_err(|e| format!("{input} 읽기 실패: {e}"))?;
    let mut s = octodocs_ohwp::Session::open(data)?;
    for (id, p) in &cmds {
        let v = s.run(id, p).map_err(|e| format!("{id}: {e}"))?;
        println!("{}", json!({ "command": id, "result": v }));
    }
    if let Some(out) = save {
        if engine_save {
            std::fs::write(&out, s.engine_bytes()?).map_err(|e| format!("{out} 쓰기 실패: {e}"))?;
        } else {
            let (bytes, rep) = s.save()?;
            std::fs::write(&out, bytes).map_err(|e| format!("{out} 쓰기 실패: {e}"))?;
            if let Some(r) = report {
                std::fs::write(&r, serde_json::to_string_pretty(&rep).unwrap_or_default()).map_err(|e| format!("{r} 쓰기 실패: {e}"))?;
            }
            eprintln!("보존 저장: {out}{}", if rep.whole_engine { " (맞대기 실패 — 엔진 결과 전체 사용)" } else { "" });
        }
    }
    Ok(())
}
