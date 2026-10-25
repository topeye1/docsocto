//! MCP 서버(3절 기능 3): 표준 입출력으로 JSON-RPC 2.0 한 줄씩 주고받는다.
//! 도구: list_commands, open, run, save, close, inspect.

use std::collections::HashMap;
use std::io::{BufRead, Write};

use serde_json::{json, Value};

use crate::{default_save_path, Engine};

struct Open {
    path: String,
    engine: Box<dyn Engine>,
}

pub struct Server {
    docs: HashMap<String, Open>,
    next: u32,
}

fn tool(name: &str, desc: &str, props: Value, required: &[&str]) -> Value {
    json!({"name": name, "description": desc, "inputSchema": {"type": "object", "properties": props, "required": required}})
}

fn tools() -> Value {
    json!([
        tool("list_commands", "Octo Docs 공통 명령 목록(한글·영어 이름, 매개변수). path를 주면 그 형식 엔진 고유 명령도 함께.",
             json!({"path": {"type": "string"}}), &[]),
        tool("open", "문서를 연다(hwpx·hwp·docx·md·txt). 돌려받은 handle로 run·save 한다.",
             json!({"path": {"type": "string"}}), &["path"]),
        tool("run", "열린 문서에 명령 하나를 실행한다. 예: command='table.setCell', params={table:0,row:1,col:1,text:'값'}",
             json!({"handle": {"type": "string"}, "command": {"type": "string"}, "params": {"type": "object"}}), &["handle", "command"]),
        tool("save", "보존 저장한다. path를 안 주면 <원본>_수정.<확장자>. 원본 경로에 쓰려면 overwrite=true.",
             json!({"handle": {"type": "string"}, "path": {"type": "string"}, "overwrite": {"type": "boolean"}}), &["handle"]),
        tool("close", "문서를 닫는다(저장하지 않음).", json!({"handle": {"type": "string"}}), &["handle"]),
        tool("read_reference", "참고 자료(hwpx·hwp·docx·md·txt·pptx·xlsx·pdf)를 글자로 읽는다. 각 조각에 위치(at: 문단·표 행·시트!행·슬라이드·쪽)가 붙는다 — 숫자를 쓸 때 이 위치를 출처로 밝힐 것.",
             json!({"path": {"type": "string"}}), &["path"]),
        tool("inspect", "파일을 열어 문서 정보·표 목록·본문 글자를 돌려준다(고치지 않음).",
             json!({"path": {"type": "string"}}), &["path"]),
    ])
}

impl Server {
    pub fn new() -> Self {
        Server { docs: HashMap::new(), next: 1 }
    }

    fn call(&mut self, name: &str, a: &Value) -> Result<Value, String> {
        let s = |k: &str| a.get(k).and_then(Value::as_str).map(str::to_string).ok_or_else(|| format!("'{k}' 가 필요합니다"));
        match name {
            "list_commands" => {
                let mut v = json!({"common": crate::COMMON.iter().map(|c| json!({"id": c.0, "ko": c.1, "en": c.2, "params": c.3})).collect::<Vec<_>>()});
                if let Ok(path) = s("path") {
                    let data = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
                    let e = crate::open(&path, data)?;
                    v["native"] = json!(e.native_commands().iter().map(|c| json!({"id": c.0, "ko": c.1, "en": c.2})).collect::<Vec<_>>());
                }
                Ok(v)
            }
            "open" => {
                let path = s("path")?;
                let data = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
                let engine = crate::open(&path, data)?;
                let h = format!("d{}", self.next);
                self.next += 1;
                let fmt = engine.format().name();
                self.docs.insert(h.clone(), Open { path, engine });
                Ok(json!({"handle": h, "format": fmt}))
            }
            "run" => {
                let h = s("handle")?;
                let d = self.docs.get_mut(&h).ok_or("열린 문서가 아닙니다(handle 확인)")?;
                crate::run(d.engine.as_mut(), &s("command")?, a.get("params").unwrap_or(&json!({})))
            }
            "save" => {
                let h = s("handle")?;
                let d = self.docs.get_mut(&h).ok_or("열린 문서가 아닙니다(handle 확인)")?;
                let out = s("path").unwrap_or_else(|_| default_save_path(&d.path, d.engine.format()));
                let overwrite = a.get("overwrite").and_then(Value::as_bool).unwrap_or(false);
                if std::path::Path::new(&out).exists() && !overwrite {
                    return Err(crate::i18n::t("exists"));
                }
                let (bytes, report) = d.engine.save()?;
                std::fs::write(&out, bytes).map_err(|e| format!("{out}: {e}"))?;
                Ok(json!({"path": out, "report": report}))
            }
            "close" => {
                self.docs.remove(&s("handle")?);
                Ok(json!({"ok": true}))
            }
            "read_reference" => crate::read::read(&s("path")?),
            "inspect" => {
                let path = s("path")?;
                crate::inspect(&path)
            }
            other => Err(format!("모르는 도구: {other}")),
        }
    }

    /// 요청 하나 처리. 알림(id 없음)이면 None.
    pub fn handle(&mut self, req: &Value) -> Option<Value> {
        let id = req.get("id").cloned()?;
        let method = req.get("method").and_then(Value::as_str).unwrap_or("");
        let result = match method {
            "initialize" => Ok(json!({
                "protocolVersion": req.pointer("/params/protocolVersion").cloned().unwrap_or(json!("2024-11-05")),
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "octodocs", "version": env!("CARGO_PKG_VERSION")},
                "instructions": "Octo Docs: HWPX·docx·md·txt를 같은 명령으로 고친다. open → run(명령) → save. 명령 목록은 list_commands. 저장은 원본을 덮어쓰지 않고 손대지 않은 부분은 원본 그대로 둔다."
            })),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(json!({"tools": tools()})),
            "tools/call" => {
                let name = req.pointer("/params/name").and_then(Value::as_str).unwrap_or("");
                let args = req.pointer("/params/arguments").cloned().unwrap_or(json!({}));
                let (text, is_err) = match self.call(name, &args) {
                    Ok(v) => (serde_json::to_string(&v).unwrap_or_default(), false),
                    Err(e) => (e, true),
                };
                Ok(json!({"content": [{"type": "text", "text": text}], "isError": is_err}))
            }
            _ => Err(json!({"code": -32601, "message": format!("method not found: {method}")})),
        };
        Some(match result {
            Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": e}),
        })
    }

    pub fn serve(&mut self) {
        let stdin = std::io::stdin();
        let mut out = std::io::stdout();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let resp = match serde_json::from_str::<Value>(&line) {
                Ok(req) => self.handle(&req),
                Err(e) => Some(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": e.to_string()}})),
            };
            if let Some(r) = resp {
                let _ = writeln!(out, "{r}");
                let _ = out.flush();
            }
        }
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initialize_and_list_tools() {
        let mut s = Server::new();
        let r = s.handle(&json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}})).unwrap();
        assert_eq!(r["result"]["serverInfo"]["name"], "octodocs");
        let r = s.handle(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap();
        assert!(r["result"]["tools"].as_array().unwrap().len() >= 6);
        assert!(s.handle(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
    }

    #[test]
    fn open_run_save_text_file() {
        let dir = std::env::temp_dir().join("octodocs_mcp_test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("메모.txt");
        std::fs::write(&src, "첫 줄\r\n둘째 줄\r\n").unwrap();
        let out = dir.join("메모_수정.txt");
        let _ = std::fs::remove_file(&out);
        let mut s = Server::new();
        let call = |s: &mut Server, name: &str, args: Value| {
            let r = s.handle(&json!({"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":name,"arguments":args}})).unwrap();
            assert_eq!(r["result"]["isError"], false, "{r}");
            serde_json::from_str::<Value>(r["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
        };
        let h = call(&mut s, "open", json!({"path": src.to_string_lossy()}))["handle"].as_str().unwrap().to_string();
        call(&mut s, "run", json!({"handle": h, "command": "text.replace", "params": {"find":"둘째","replace":"두 번째"}}));
        call(&mut s, "save", json!({"handle": h}));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "첫 줄\r\n두 번째 줄\r\n");
        assert_eq!(std::fs::read_to_string(&src).unwrap(), "첫 줄\r\n둘째 줄\r\n", "원본은 그대로");
    }
}
