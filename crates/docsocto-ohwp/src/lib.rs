//! ohwp — rhwp 위에 얹는 DocsOcto의 HWPX 엔진 층.

pub use rhwp;

/// 문서를 열어 아무것도 고치지 않고 HWPX로 다시 저장한다(보존 저장의 B0).
pub fn resave_hwpx(data: &[u8]) -> Result<Vec<u8>, String> {
    let doc = rhwp::wasm_api::HwpDocument::from_bytes(data).map_err(|e| format!("열기 실패: {e:?}"))?;
    doc.export_hwpx_native().map_err(|e| format!("저장 실패: {e:?}"))
}