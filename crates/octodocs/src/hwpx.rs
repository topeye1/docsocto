//! HWPX·HWP 엔진: ohwp(rhwp)에 그대로 넘긴다. ohwp 명령 이름이 공통 명령과 같다.

use serde_json::{json, Value};

use crate::{Engine, Format};

pub struct HwpxEngine {
    fmt: Format,
    s: octodocs_ohwp::Session,
}

impl HwpxEngine {
    pub fn open(fmt: Format, data: Vec<u8>) -> Result<Self, String> {
        Ok(HwpxEngine { fmt, s: octodocs_ohwp::Session::open(data)? })
    }
}

impl Engine for HwpxEngine {
    fn format(&self) -> Format {
        self.fmt
    }

    fn run(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        self.s.run(id, params)
    }

    fn save(&mut self) -> Result<(Vec<u8>, Value), String> {
        let (bytes, report) = self.s.save()?;
        Ok((bytes, serde_json::to_value(&report).unwrap_or_else(|_| json!({}))))
    }

    fn native_commands(&self) -> Vec<(String, String, String)> {
        octodocs_ohwp::commands().iter().map(|c| (c.id.to_string(), c.ko.to_string(), c.en.to_string())).collect()
    }
}
