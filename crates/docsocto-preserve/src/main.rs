//! docsocto-preserve <원본> <고치지않은저장본> <고친저장본> <출력> [--report 보고서.json]

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let report_path = args.iter().position(|a| a == "--report").and_then(|i| args.get(i + 1)).cloned();
    let pos: Vec<&String> = {
        let mut skip = false;
        args.iter()
            .filter(|a| {
                if skip {
                    skip = false;
                    return false;
                }
                if *a == "--report" {
                    skip = true;
                    return false;
                }
                true
            })
            .collect()
    };
    if pos.len() != 4 {
        eprintln!("사용법: docsocto-preserve <원본> <고치지 않은 저장본> <고친 저장본> <출력> [--report 보고서.json]");
        return ExitCode::from(2);
    }
    let read = |p: &str| std::fs::read(p).map_err(|e| format!("{p} 읽기 실패: {e}"));
    let run = || -> Result<docsocto_preserve::Report, String> {
        let (o, b0, b1) = (read(pos[0])?, read(pos[1])?, read(pos[2])?);
        let (out, report) = docsocto_preserve::preserve(&o, &b0, &b1).map_err(|e| e.to_string())?;
        std::fs::write(pos[3], out).map_err(|e| format!("{} 쓰기 실패: {e}", pos[3]))?;
        Ok(report)
    };
    match run() {
        Ok(report) => {
            let json = serde_json::to_string_pretty(&report).unwrap_or_default();
            match &report_path {
                Some(p) => {
                    if let Err(e) = std::fs::write(p, &json) {
                        eprintln!("보고서 쓰기 실패: {e}");
                    }
                }
                None => println!("{json}"),
            }
            let engine = report.parts.iter().filter(|p| p.action == "engine").count();
            let merged = report.parts.iter().filter(|p| p.action == "merged").count();
            eprintln!("보존 저장 완료: 항목 {}개 중 병합 {merged}, 엔진 그대로 {engine}, 관계 추가 {}", report.parts.len(), report.rels_added);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}
