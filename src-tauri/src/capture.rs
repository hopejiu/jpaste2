//! Clipboard capture → toast payload assembly.
//!
//! Owns the "what gets shown in the toast" domain logic so it can be reasoned
//! about (and tested) independently of the Tauri window/thread wiring in
//! lib.rs. lib.rs only calls `build_toast` from the clipboard worker and then
//! creates the toast window — it no longer knows how actions, message, or
//! should_show are computed.

use crate::clipboard::{ClipboardContent, pipeline::ClipboardPipeline};
use crate::command::AppState;
use crate::model;
use crate::toast::ToastPayload;
use std::sync::{Arc, Mutex};
use tauri::Emitter;

/// Build the toast payload for a captured clipboard item.
///
/// Returns `None` when nothing should be shown (empty clipboard, or settings
/// say not to notify for plain text). Performs the history save, QR decode,
/// filostack push, queue auto-exit, action detection, and the
/// `EVENT_CLIPBOARD_UPDATED` emit. Window creation is left to the caller.
pub fn build_toast(
    state: &Arc<Mutex<AppState>>,
    handle: Option<&tauri::AppHandle>,
    content: &ClipboardContent,
) -> Option<ToastPayload> {
    // Skip saving if clipboard is completely empty
    if content.text.is_empty() && !content.has_image && !content.has_file_uri {
        return None;
    }

    // Resolve image bytes: prefer in-memory, fallback to temp file
    let image_bytes = if content.image_data.is_some() {
        content.image_data.clone()
    } else if let Some(ref path) = content.image_temp_path {
        std::fs::read(path).ok()
    } else {
        None
    };

    let hash = if let Some(ref img) = image_bytes {
        crate::util::sha256_bytes(img)
    } else {
        crate::util::sha256_hex(&content.text)
    };

    // Decode QR code from image at capture time (before pipeline.process)
    let qr_text = if content.has_image {
        if let Some(ref img) = image_bytes {
            let detected = crate::qrcode::decode_qr_from_image(img);
            log::debug!(
                "build_toast: QR decode for image — has_image=true image_bytes={} qr_found={}",
                img.len(),
                detected.is_some()
            );
            detected.unwrap_or_default()
        } else {
            log::debug!(
                "build_toast: has_image=true but image_bytes empty — no QR decode"
            );
            String::new()
        }
    } else {
        String::new()
    };

    let pipeline = ClipboardPipeline::new(state.clone());

    let result = pipeline.process(
        &content.text,
        content.has_image,
        content.has_file_uri,
        &hash,
        image_bytes.as_deref(),
        &qr_text,
    );

    // Clean up temp file after processing (image is now saved to final location)
    if let Some(ref path) = content.image_temp_path {
        let _ = std::fs::remove_file(path);
    }

    match result {
        Ok(payload) => {
            if let Some(app) = handle {
                let _ = app.emit(
                    model::EVENT_CLIPBOARD_UPDATED,
                    serde_json::to_value(&payload).unwrap_or_default(),
                );
            }

            // Push to filo stack (direct state access, no pipeline indirection)
            if !content.text.is_empty() {
                if let Ok(s) = state.lock() {
                    s.filostack.push(&content.text);
                }
            }

            // Queue mode auto-exit
            if let Ok(s) = state.lock() {
                if s.filostack.mode() == "queue" && (content.has_image || content.has_file_uri) {
                    drop(s);
                    log::info!("filostack: auto-exit queue mode due to non-text content");
                    if let Ok(s) = state.lock() {
                        s.filostack.set_mode("normal");
                        if let Ok(mut new_settings) = s.settings.get_settings() {
                            new_settings.paste_order = "normal".to_string();
                            let _ = s.settings.save_settings(new_settings);
                        }
                    }
                    if let Some(app) = handle {
                        let _ = app.emit(model::EVENT_PASTE_ORDER_CHANGED, "normal");
                    }
                }
            }

            // Detect actions for toast (single source of truth — see build_toast_actions)
            let actions = build_toast_actions(content, &qr_text);

            // Collect toast info (direct state access, no pipeline indirection)
            let should_show = content.has_image
                || content.has_file_uri
                || (!content.text.is_empty() && state.lock()
                    .ok()
                    .and_then(|s| s.settings.get_settings().ok())
                    .map(|s| s.notify_enabled)
                    .unwrap_or(false));
            log::debug!(
                "build_toast: should_show={} (has_image={}, has_file_uri={}, text.len={})",
                should_show, content.has_image, content.has_file_uri, content.text.len()
            );
            if should_show
            {
                let (message, icon) = if content.has_image {
                    let msg = if !qr_text.is_empty() {
                        format!("图片 · 检测到二维码")
                    } else {
                        "图片".to_string()
                    };
                    (msg, "image".into())
                } else if content.has_file_uri {
                    let files: Vec<&str> = content.text.lines().collect();
                    let msg = if files.len() <= 1 {
                        files.first()
                            .and_then(|p| std::path::Path::new(p).file_name())
                            .and_then(|n| n.to_str())
                            .map(|n| crate::util::truncate(n, 60))
                            .unwrap_or_else(|| "文件".into())
                    } else {
                        format!("{} 个文件", files.len())
                    };
                    (msg, "document".into())
                } else if !content.text.is_empty() {
                    let preview = crate::util::truncate(&content.text, 60);
                    if payload.auto_favorited {
                        (format!("{} ⭐ 已自动收藏", preview), "clipboard".into())
                    } else {
                        (preview, "clipboard".into())
                    }
                } else {
                    return None;
                };
                log::debug!(
                    "build_toast: RETURNING toast payload — message={:?} icon={:?} entry_id={} text.len={} actions={:?}",
                    message, icon, payload.id, content.text.len(), actions
                );
                Some(ToastPayload {
                    message,
                    icon,
                    entry_id: payload.id,
                    text: content.text.clone(),
                    actions,
                })
            } else {
                None
            }
        }
        Err(e) => {
            log::error!("Failed to save clipboard content: {}", e);
            None
        }
    }
}

/// Detect which actions to surface in the toast for a captured clipboard item.
///
/// Combines the text-based `detect_actions` (model.rs) with the image-only QR
/// action and prunes the `folder` action when its parent directory doesn't
/// exist. Centralizes all "what actions go in the toast" rules in one place
/// instead of splitting them between model.rs and lib.rs.
fn build_toast_actions(content: &ClipboardContent, qr_text: &str) -> Vec<String> {
    let mut actions: Vec<String> = if !content.text.is_empty() && !content.has_image {
        model::detect_actions(&content.text)
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    };

    // ponytail: QR action is the only image-based action.
    // If we add more image-based actions later, extract a shared helper.
    if content.has_image && !qr_text.is_empty() {
        actions.push("qrcode".to_string());
    }

    // For single file paths: if parent dir doesn't exist, remove the
    // "folder" action — no point offering to open a dead path.
    let lines: Vec<&str> = content.text.lines().collect();
    let is_single_path = lines.len() == 1
        && (content.has_file_uri || model::is_windows_path(lines[0]));
    if is_single_path {
        let path = lines[0].trim();
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.exists() {
                actions.retain(|a| a != "folder");
            }
        }
    }

    actions
}
