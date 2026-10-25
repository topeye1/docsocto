//! ohwp <명령> …
//!   resave <입력> <출력>   고치지 않고 다시 저장(보존 저장 비교용)

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("resave") if args.len() == 3 => {
            let data = match std::fs::read(&args[1]) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{} 읽기 실패: {e}", args[1]);
                    return ExitCode::from(1);
                }
            };
            match docsocto_ohwp::resave_hwpx(&data).and_then(|b| std::fs::write(&args[2], b).map_err(|e| e.to_string())) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("사용법: ohwp resave <입력> <출력>");
            ExitCode::from(2)
        }
    }
}