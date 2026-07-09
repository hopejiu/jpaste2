//! QuickLaunch commands
//!
//! CRUD for LaunchTarget entries and execution (launch target).
//! Data is stored in SettingsService.Data.launch_targets (settings.json).

use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use crate::command::viewer::build_viewer_window;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::Shortcut;

use crate::command::AppState;
use crate::command::lock_state;
use crate::service::settings::{LaunchTarget, LaunchTargetKind};

/// Return all launch targets.
#[tauri::command]
pub fn get_launch_targets(state: State<'_, Arc<Mutex<AppState>>>) -> Result<Vec<LaunchTarget>, String> {
    let s = lock_state!(state);
    s.settings.get_launch_targets()
}

/// Persist the full list of launch targets (replaces all).
/// Also differential-registers/unregisters the global shortcuts.
#[tauri::command]
pub fn save_launch_targets(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    targets: Vec<LaunchTarget>,
) -> Result<(), String> {
    // Snapshot old targets for diff
    let old_targets = {
        let s = lock_state!(state);
        s.settings.get_launch_targets()?
    };

    // Save to settings
    {
        let s = lock_state!(state);
        s.settings.save_launch_targets(targets.clone())?;
    }

    // Diff-register hotkeys (skip Alt+V — managed by setup_hotkeys)
    sync_launch_hotkeys(&app, &old_targets, &targets);

    // Keep the dispatch map (hotkey string → target id) in sync so the global
    // shortcut handler can resolve runtime-added/edited targets. Keyed by the
    // canonical shortcut string to match the handler's `shortcut.to_string()`.
    {
        let s = lock_state!(state);
        let mut map = s.launch_hotkey_map.lock().map_err(|e| e.to_string())?;
        map.clear();
        for t in &targets {
            if t.enabled {
                if let Some(ref hk) = t.hotkey {
                    if let Ok(sc) = hk.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                        map.insert(sc.to_string(), t.id.clone());
                    }
                }
            }
        }
    }

    Ok(())
}

/// Execute a launch target by id.
/// - web: create/show/hide a WebView window (toggle)
/// - file: spawn process via opener
#[tauri::command]
pub fn launch_target(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    id: String,
) -> Result<(), String> {
    let s = lock_state!(state);
    do_launch_by_id(&app, &s, &id)
}

/// Internal: find target by id and execute it (no Tauri State dependency).
pub fn launch_target_by_hotkey(app: &AppHandle, s: &AppState, id: &str) -> Result<(), String> {
    do_launch_by_id(app, s, id)
}

fn do_launch_by_id(app: &AppHandle, s: &AppState, id: &str) -> Result<(), String> {
    let target = s.settings
        .get_launch_targets()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("launch target not found: {}", id))?;

    if !target.enabled {
        return Err(format!("launch target '{}' is disabled", target.name));
    }

    log::info!("quicklaunch: executing {} ({})", target.name, serde_json::to_string(&target.kind).unwrap_or_default());

    match target.kind {
        LaunchTargetKind::Web => launch_web(app, &target),
        LaunchTargetKind::File => launch_file(app, &target),
    }
}

fn launch_web(app: &AppHandle, target: &LaunchTarget) -> Result<(), String> {
    let label = format!("webview-{}", target.id);

    // Check if window already exists → toggle
    if let Some(window) = app.get_webview_window(&label) {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            // Toggle: hide it
            let _ = window.hide();
            log::debug!("quicklaunch: web toggle hidden {}", target.name);
        } else {
            let _ = window.show();
            let _ = window.set_focus();
            log::debug!("quicklaunch: web toggle shown {}", target.name);
        }
        return Ok(());
    }

    // Create new webview window
    let url_str = target.target.trim().to_string();
    let url = url::Url::parse(&url_str)
        .map_err(|e| format!("invalid URL '{}': {}", url_str, e))?;

    let label_clone = label.clone();
    let name_clone = target.name.clone();
    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let builder = tauri::WebviewWindowBuilder::new(
            &app_clone,
            &label_clone,
            tauri::WebviewUrl::External(url),
        )
        .title(&name_clone)
        .inner_size(1024.0, 768.0)
        .min_inner_size(640.0, 480.0);

        match builder.build() {
            Ok(window) => {
                let _ = window.show();
                let _ = window.set_focus();

                // Run lifecycle management in a background thread
                let label = label_clone.clone();
                let app = app_clone.clone();
                let name_for_thread = name_clone.clone();
                std::thread::spawn(move || {
                    manage_web_window_lifecycle(&app, &label, &name_for_thread);
                });

                log::info!("quicklaunch: web window created for {}", name_clone);
            }
            Err(e) => {
                log::error!("quicklaunch: failed to create web window for {}: {}", name_clone, e);
            }
        }
    });

    Ok(())
}

/// Lifecycle management for a single web window:
/// - On focus loss: start 1min timer → hide
/// - After hide: start 10min timer → destroy
fn manage_web_window_lifecycle(app: &AppHandle, label: &str, _name: &str) {
    use std::time::Duration;

    loop {
        std::thread::sleep(Duration::from_secs(5));

        let window = match app.get_webview_window(label) {
            Some(w) => w,
            None => {
                log::debug!("quicklaunch: web window {} gone, ending lifecycle", label);
                return;
            }
        };

        // If window was closed manually, stop
        let visible = window.is_visible().unwrap_or(false);
        if !visible {
            // Already hidden — wait 10min then destroy
            log::debug!("quicklaunch: web window {} hidden, 10min destroy timer", label);
            // Poll every 30s, check if it was re-shown
            let mut elapsed = 0u64;
            loop {
                std::thread::sleep(Duration::from_secs(30));
                elapsed += 30;
                let w = app.get_webview_window(label);
                match w {
                    None => return, // manually closed
                    Some(ref w2) => {
                        if w2.is_visible().unwrap_or(false) {
                            // Re-shown, abort destroy
                            log::debug!("quicklaunch: web window {} re-shown, cancelling destroy", label);
                            break;
                        }
                    }
                }
                if elapsed >= 600 {
                    // 10min elapsed
                    log::info!("quicklaunch: destroying hidden web window {}", label);
                    if let Some(w) = app.get_webview_window(label) {
                        let _ = w.destroy();
                    }
                    return;
                }
            }
        } else {
            // Window is visible — check if focused
            let focused = window.is_focused().unwrap_or(false);
            if !focused {
                // Not focused, wait for 1min of unfocused before hiding
                let mut unfocused_elapsed = 0u64;
                loop {
                    std::thread::sleep(Duration::from_secs(10));
                    unfocused_elapsed += 10;
                    let w = app.get_webview_window(label);
                    match w {
                        None => return,
                        Some(ref w2) => {
                            if w2.is_focused().unwrap_or(false) {
                                // Refocused, reset timer
                                unfocused_elapsed = 0;
                                continue;
                            }
                            if !w2.is_visible().unwrap_or(false) {
                                // Already hidden, switch to destroy loop
                                break;
                            }
                        }
                    }
                    if unfocused_elapsed >= 60 {
                        // 1min unfocused → hide
                        log::debug!("quicklaunch: hiding web window {} (1min unfocused)", label);
                        if let Some(w) = app.get_webview_window(label) {
                            let _ = w.hide();
                        }
                        break;
                    }
                }
            }
        }
    }
}

fn launch_file(_app: &AppHandle, target: &LaunchTarget) -> Result<(), String> {
    let path = &target.target;
    // Spawn process directly (simpler than opener API for exe/lnk)
    std::process::Command::new(path)
        .spawn()
        .map_err(|e| format!("启动文件失败 ({}): {}", path, e))?;
    log::info!("quicklaunch: launched file {}", target.name);
    Ok(())
}

/// Validate a hotkey string for a launch target.
/// Returns Ok(()) if valid, Err with description if invalid.
#[tauri::command]
pub fn check_target_hotkey(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    hotkey_str: String,
    editing_id: Option<String>,
) -> Result<(), String> {
    // 1. Must not be empty
    if hotkey_str.trim().is_empty() {
        return Err("快捷键不能为空".into());
    }

    // 2. Must not equal the current global hotkey (window toggle key, now changeable/cleared)
    let global_hk = {
        let s = lock_state!(state);
        s.settings.get_settings().ok().map(|d| d.hotkey)
    };
    if let Some(ref ghk) = global_hk {
        if !ghk.trim().is_empty() && ghk.eq_ignore_ascii_case(&hotkey_str) {
            return Err(format!("快捷键已被全局唤出键占用 ({})", ghk));
        }
    }

    // 3. Must not conflict with other launch targets
    let targets = {
        let s = lock_state!(state);
        s.settings.get_launch_targets()?
    };
    for t in &targets {
        if Some(t.id.as_str()) == editing_id.as_deref() {
            continue; // skip self
        }
        if let Some(ref hk) = t.hotkey {
            if hk.eq_ignore_ascii_case(&hotkey_str) {
                return Err(format!("快捷键已被“{}”占用", t.name));
            }
        }
    }

    // 4. Try to parse as valid shortcut (syntax check)
    let _: Shortcut = hotkey_str
        .parse()
        .map_err(|_| format!("无效的快捷键格式: {}", hotkey_str))?;

    // 5. Temporary register + unregister to detect OS-level conflicts
    // (This will fail if the shortcut is already registered)
    let shortcut: Shortcut = hotkey_str.parse().map_err(|_| format!("无效的快捷键格式: {}", hotkey_str))?;
    if let Err(e) = app.global_shortcut().register(shortcut.clone()) {
        return Err(format!("系统快捷键冲突: {}", e));
    }
    let _ = app.global_shortcut().unregister(shortcut);

    Ok(())
}

/// Sync the difference between old and new launch target hotkeys.
/// Registered: new keys not in old set.
/// Unregistered: old keys not in new set.
pub fn sync_launch_hotkeys(app: &AppHandle, old: &[LaunchTarget], new: &[LaunchTarget]) {
    let old_hotkeys: Vec<&str> = old
        .iter()
        .filter(|t| t.enabled)
        .filter_map(|t| t.hotkey.as_deref())
        .collect();
    let new_hotkeys: Vec<&str> = new
        .iter()
        .filter(|t| t.enabled)
        .filter_map(|t| t.hotkey.as_deref())
        .collect();

    // Unregister removed
    for hk in &old_hotkeys {
        if !new_hotkeys.contains(hk) {
            if let Ok(s) = hk.parse::<Shortcut>() {
                let _ = app.global_shortcut().unregister(s);
                log::debug!("quicklaunch: unregistered hotkey {}", hk);
            }
        }
    }

    // Register added
    for hk in &new_hotkeys {
        if !old_hotkeys.contains(hk) {
            if let Ok(s) = hk.parse::<Shortcut>() {
                let _ = app.global_shortcut().register(s);
                log::debug!("quicklaunch: registered hotkey {}", hk);
            }
        }
    }
}

/// Open a native file picker for exe/lnk files.
#[tauri::command]
pub async fn pick_file_path(app: AppHandle) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app.dialog()
        .file()
        .add_filter("可执行程序", &["exe", "lnk"])
        .blocking_pick_file();
    file.and_then(|f| f.as_path().map(|p| p.to_string_lossy().to_string()))
}

/// Open a separate window with the full Quick Launch UI (list + add/edit).
///
/// A single reused window: opening it again refocuses instead of stacking.
#[tauri::command]
pub async fn open_quicklaunch(app: AppHandle) -> Result<(), String> {
    let label = "quicklaunch";
    if let Some(existing) = app.get_webview_window(label) {
        let _ = existing.set_focus();
        return Ok(());
    }
    build_viewer_window(
        app,
        label.into(),
        "/quicklaunch".into(),
        "快速启动".into(),
        480.0,
        620.0,
        440.0,
        480.0,
    )
    .await
}
