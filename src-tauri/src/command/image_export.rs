//! Image generation & export commands (toolbox: QR generate, SVG→PNG export)
//!
//! - `generate_qr`: encode text/URL to a PNG QR code (base64), via rxing + image.
//! - `write_clipboard_image`: copy PNG bytes to the system clipboard.
//! - `save_image_dialog`: save PNG bytes to a user-chosen file.
//! - `get_clipboard_text`: read current clipboard text (SVG viewer input source).

use crate::command::{lock_state, AppState};
use base64::Engine;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, State};

/// Parse a `#RRGGBB` (or `RRGGBB`) hex color into RGB, falling back to `default`.
fn parse_hex(s: &str, default: [u8; 3]) -> [u8; 3] {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return default;
    }
    match (
        u8::from_str_radix(&h[0..2], 16),
        u8::from_str_radix(&h[2..4], 16),
        u8::from_str_radix(&h[4..6], 16),
    ) {
        (Ok(r), Ok(g), Ok(b)) => [r, g, b],
        _ => default,
    }
}

/// Generate a QR code PNG (base64-encoded, no data URI prefix).
///
/// `size` is the target pixel size; rxing enlarges to fit whole modules plus
/// the `margin` quiet zone. `ec_level` is one of "L"/"M"/"Q"/"H".
#[tauri::command]
pub fn generate_qr(
    content: String,
    size: u32,
    ec_level: String,
    margin: u32,
    fg: String,
    bg: String,
) -> Result<String, String> {
    use rxing::qrcode::QRCodeWriter;
    use rxing::{BarcodeFormat, EncodeHints, Writer};

    if content.is_empty() {
        return Err("内容不能为空".into());
    }

    let fg_rgb = parse_hex(&fg, [0, 0, 0]);
    let bg_rgb = parse_hex(&bg, [255, 255, 255]);

    let mut hints = EncodeHints::default();
    hints.ErrorCorrection = Some(ec_level);
    hints.Margin = Some(margin.to_string());
    hints.CharacterSet = Some("UTF-8".to_string());

    let matrix = QRCodeWriter
        .encode_with_hints(
            &content,
            &BarcodeFormat::QR_CODE,
            size as i32,
            size as i32,
            &hints,
        )
        .map_err(|e| format!("二维码编码失败: {}", e))?;

    let width = matrix.getWidth();
    let height = matrix.getHeight();
    let mut img = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let [r, g, b] = if matrix.get(x, y) { fg_rgb } else { bg_rgb };
            img.put_pixel(x, y, image::Rgba([r, g, b, 255]));
        }
    }

    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
        .map_err(|e| format!("PNG 编码失败: {}", e))?;

    Ok(base64::engine::general_purpose::STANDARD.encode(&buf))
}

/// Copy PNG image bytes to the system clipboard.
#[tauri::command]
pub fn write_clipboard_image(
    state: State<'_, Arc<Mutex<AppState>>>,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    let mut c = mgr.lock().map_err(|e| e.to_string())?;
    c.write_image(&bytes)
}

/// Read the current clipboard text (SVG viewer "read from clipboard" input).
#[tauri::command]
pub fn get_clipboard_text(state: State<'_, Arc<Mutex<AppState>>>) -> Result<String, String> {
    let mgr = {
        let s = lock_state!(state);
        s.clipboard_mgr.clone()
    };
    let c = mgr.lock().map_err(|e| e.to_string())?;
    Ok(c.get_text())
}

/// Save PNG bytes to a user-chosen file. Returns `false` if the user cancelled.
#[tauri::command]
pub async fn save_image_dialog(
    app: AppHandle,
    bytes: Vec<u8>,
    default_name: String,
) -> Result<bool, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("PNG 图片", &["png"])
        .set_file_name(&default_name)
        .blocking_save_file();
    match file {
        Some(path) => {
            let p = path.as_path().ok_or_else(|| "无效的保存路径".to_string())?;
            std::fs::write(p, &bytes).map_err(|e| format!("保存失败: {}", e))?;
            Ok(true)
        }
        None => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex("#FF0000", [0, 0, 0]), [255, 0, 0]);
        assert_eq!(parse_hex("00ff00", [0, 0, 0]), [0, 255, 0]);
        assert_eq!(parse_hex("bad", [1, 2, 3]), [1, 2, 3]); // fallback
        assert_eq!(parse_hex("#GGGGGG", [9, 9, 9]), [9, 9, 9]); // invalid hex
    }

    #[test]
    fn test_generate_qr_produces_decodable_png() {
        let content = "https://example.com/generate-test";
        let b64 = generate_qr(
            content.to_string(),
            200,
            "M".to_string(),
            2,
            "#000000".to_string(),
            "#FFFFFF".to_string(),
        )
        .expect("generate should succeed");
        let png = base64::engine::general_purpose::STANDARD
            .decode(&b64)
            .expect("valid base64");
        // Round-trip: the generated QR must decode back to the same content.
        let decoded = crate::qrcode::decode_qr_from_image(&png);
        assert_eq!(decoded.as_deref(), Some(content));
    }

    #[test]
    fn test_generate_qr_empty_errors() {
        assert!(generate_qr(
            String::new(),
            200,
            "M".to_string(),
            2,
            "#000000".to_string(),
            "#FFFFFF".to_string()
        )
        .is_err());
    }
}
