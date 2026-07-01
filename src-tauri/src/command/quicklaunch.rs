//! QuickLaunch commands
//!
//! CRUD for LaunchTarget entries and execution (launch target).
//! Data is stored in SettingsService.Data.launch_targets (settings.json).

use crate::command::viewer::build_viewer_window;
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_global_shortcut::Shortcut;

use crate::command::lock_state;
use crate::command::AppState;
use crate::service::settings::{LaunchTarget, LaunchTargetKind};

// ── Shared shortcut helpers ─────────────────────────────────────────────

/// Build the canonical shortcut → target_id map from launch targets.
///
/// Normalises each shortcut via `Shortcut::parse()` so keys match the global
/// shortcut handler's `shortcut.to_string()`.  DRY reference for both
/// `build_services` (initial build) and `save_launch_targets` (rebuild).
pub fn build_launch_hotkey_map(targets: &[LaunchTarget]) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for t in targets {
        if t.enabled {
            if let Some(ref hk) = t.hotkey {
                let key = hk
                    .parse::<Shortcut>()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|_| hk.clone());
                map.insert(key, t.id.clone());
            }
        }
    }
    map
}

// ── Public API ──────────────────────────────────────────────────────────

/// Return all launch targets.
#[tauri::command]
pub fn get_launch_targets(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<Vec<LaunchTarget>, String> {
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
        *map = build_launch_hotkey_map(&targets);
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
    let target = s
        .settings
        .get_launch_targets()?
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| format!("launch target not found: {}", id))?;

    if !target.enabled {
        return Err(format!("launch target '{}' is disabled", target.name));
    }

    log::info!(
        "quicklaunch: executing {} ({})",
        target.name,
        serde_json::to_string(&target.kind).unwrap_or_default()
    );

    match target.kind {
        LaunchTargetKind::Web => launch_web(app, &target),
        LaunchTargetKind::File => launch_file(app, &target),
    }
}

fn launch_web(app: &AppHandle, target: &LaunchTarget) -> Result<(), String> {
    let label = format!("webview-{}", target.id);

    // Check if window already exists → toggle
    if let Some(window) = app.get_webview_window(&label) {
        let minimized = window.is_minimized().unwrap_or(false);
        if minimized {
            // Minimized (still is_visible()==true on Windows) → restore & focus.
            // show() alone does NOT un-minimize on Windows; must call unminimize()
            // first or the window stays collapsed in the taskbar.
            let _ = window.unminimize();
            crate::show_focus_window(&window);
            log::debug!("quicklaunch: web restored from minimized {}", target.name);
        } else if window.is_visible().unwrap_or(false) {
            // Toggle: hide it
            let _ = window.hide();
            log::debug!("quicklaunch: web toggle hidden {}", target.name);
        } else {
            crate::show_focus_window(&window);
            log::debug!("quicklaunch: web toggle shown {}", target.name);
        }
        return Ok(());
    }

    // Create new webview window
    let url_str = target.target.trim().to_string();
    let url = url::Url::parse(&url_str).map_err(|e| format!("invalid URL '{}': {}", url_str, e))?;

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
                crate::show_focus_window(&window);

                // Event-driven lifecycle: Focused(false) → 1min → hide,
                // then hidden → 10min → destroy. Replaces the old busy-polling
                // manage_web_window_lifecycle.
                setup_web_lifecycle(&window, &app_clone, &label_clone);

                log::info!("quicklaunch: web window created for {}", name_clone);
            }
            Err(e) => {
                log::error!(
                    "quicklaunch: failed to create web window for {}: {}",
                    name_clone,
                    e
                );
            }
        }
    });

    Ok(())
}

/// Event-driven lifecycle for a web window.
///
/// Replaces the old busy-polling `manage_web_window_lifecycle`.
/// - Focused(false) → 1min `start_autohide_timer` → hide
/// - Focused(true) → cancel pending hide
/// - After hiding → 10min timer → destroy
fn setup_web_lifecycle(window: &tauri::WebviewWindow, app: &AppHandle, label: &str) {
    let pending_hide: std::sync::Arc<std::sync::Mutex<Option<std::sync::mpsc::Sender<()>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let app_life = app.clone();
    let label_life = label.to_string();

    let ph = pending_hide.clone();
    let app2 = app.clone();
    let label2 = label.to_string();
    window.on_window_event(move |event| {
        match event {
            tauri::WindowEvent::Focused(false) => {
                let tx = crate::start_autohide_timer(&app_life, &label_life, 60_000);
                if let Ok(mut p) = ph.lock() {
                    *p = Some(tx);
                }
                log::debug!("quicklaunch: web {} auto-hide timer started (1min)", label_life);
            }
            tauri::WindowEvent::Focused(true) => {
                if let Ok(mut p) = ph.lock() {
                    drop(p.take()); // drop Sender → cancels timer
                }
                log::debug!("quicklaunch: web {} focused, hide cancelled", label_life);
            }
            tauri::WindowEvent::Destroyed => {
                log::debug!("quicklaunch: web {} destroyed", label_life);
            }
            _ => {}
        }
    });

    // 10min inactivity destroy: poll less aggressively but check hidden → destroy.
    // ponytail: a single background sleep thread per web window is much lighter
    // than the old nested-polling loop.
    std::thread::spawn(move || {
        use std::time::Duration;
        loop {
            std::thread::sleep(Duration::from_secs(30));
            let w = match app2.get_webview_window(&label2) {
                Some(w) => w,
                None => return, // window gone
            };
            if w.is_visible().unwrap_or(true) {
                continue; // still visible, skip
            }
            // Hidden — wait 10min, checking every 30s if re-shown
            let mut elapsed = 0u64;
            loop {
                std::thread::sleep(Duration::from_secs(30));
                elapsed += 30;
                let w2 = match app2.get_webview_window(&label2) {
                    Some(w) => w,
                    None => return,
                };
                if w2.is_visible().unwrap_or(false) {
                    log::debug!("quicklaunch: web {} re-shown, abort destroy", label2);
                    break;
                }
                if elapsed >= 600 {
                    log::info!("quicklaunch: destroying hidden web window {}", label2);
                    let _ = w2.destroy();
                    return;
                }
            }
        }
    });
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

/// Validate a hotkey string for a launch target or toolbox item.
/// Returns Ok(()) if valid, Err with description if invalid.
///
/// `editing_route` is the toolbox route being edited (excluded from self-conflict).
/// `toolbox_hotkeys` is the current map of all toolbox hotkeys (for cross-checking).
#[tauri::command]
pub fn check_target_hotkey(
    app: AppHandle,
    state: State<'_, Arc<Mutex<AppState>>>,
    hotkey_str: String,
    editing_id: Option<String>,
    editing_route: Option<String>,
    toolbox_hotkeys: std::collections::HashMap<String, String>,
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

    // 4. Must not conflict with toolbox hotkeys (unless editing the same route)
    for (route, hk) in &toolbox_hotkeys {
        if let Some(ref er) = editing_route {
            if er == route {
                continue; // skip self
            }
        }
        if hk.eq_ignore_ascii_case(&hotkey_str) {
            return Err(format!("快捷键已被工具箱占用 ({})", route));
        }
    }

    // 5. Try to parse as valid shortcut (syntax check)
    let _: Shortcut = hotkey_str
        .parse()
        .map_err(|_| format!("无效的快捷键格式: {}", hotkey_str))?;

    // 6. Temporary register + unregister to detect OS-level conflicts
    // (This will fail if the shortcut is already registered)
    let shortcut: Shortcut = hotkey_str
        .parse()
        .map_err(|_| format!("无效的快捷键格式: {}", hotkey_str))?;
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
    let file = app
        .dialog()
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
