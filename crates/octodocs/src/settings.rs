//! 사용자 설정(4절): flyocto 사용자 설정 위치 아래 Docs\settings.json.
//! Windows: %APPDATA%\com.flyocto\flyocto\Docs, 그 밖: ~/.config/com.flyocto/flyocto/Docs.
//! 기본 형식은 HWPX·docx 둘 중 하나(결정 #15), 언어는 ko·en(결정 #22).

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Settings {
    /// "hwpx" | "docx" — 처음 실행할 때 고른다
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_format: Option<String>,
    /// "ko" | "en" — 없으면 OS 언어로 정한다
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
}

pub fn dir() -> PathBuf {
    if let Ok(d) = std::env::var("OCTODOCS_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var("APPDATA").map(PathBuf::from).unwrap_or_else(|_| {
        std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")).unwrap_or_else(|_| PathBuf::from("."))
    });
    base.join("com.flyocto").join("flyocto").join("Docs")
}

pub fn path() -> PathBuf {
    dir().join("settings.json")
}

pub fn load() -> Settings {
    std::fs::read_to_string(path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(s: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    std::fs::write(path(), serde_json::to_string_pretty(s).unwrap_or_default()).map_err(|e| e.to_string())
}

/// 기본 형식 값 검사: 선택지는 hwpx·docx 둘뿐(md·txt 없음, 결정 #15).
pub fn check_default_format(v: &str) -> Result<String, String> {
    match v.to_ascii_lowercase().as_str() {
        "hwpx" => Ok("hwpx".into()),
        "docx" => Ok("docx".into()),
        _ => Err(crate::i18n::t("bad_default")),
    }
}

/// 언어: 설정 값 → 없으면 OS 언어가 한국어면 ko, 아니면 en(결정 #22).
pub fn language() -> String {
    if let Some(l) = load().language {
        return l;
    }
    os_language()
}

pub fn os_language() -> String {
    let loc = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
    if loc.starts_with("ko") { "ko".into() } else { "en".into() }
}
