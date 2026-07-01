//! Toolbox hotkey dispatch.
//!
//! When the user presses a toolbox hotkey (globally registered via
//! `setup_hotkeys` in lib.rs), the handler resolves the route and calls
//! `open_toolbox_item` here.  The function is sync — it spawns async work
//! via `tauri::async_runtime::spawn` — so it can be called from the
//! global-shortcut handler (which is a sync closure).

use tauri::{AppHandle, Manager};

/// Open a toolbox item by its route.  Spawns async work; returns immediately.
///
/// Routes:
/// - `/viewer/*` → blank viewer window (same as toolbox card click)
/// - `/quicklaunch` → quicklaunch panel
/// - `/share` → share panel
pub fn open_toolbox_item(app: &AppHandle, route: &str) {
    let app = app.clone();
    let route = route.to_string();

    if route.starts_with("/viewer/") {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::command::viewer::open_blank_viewer(app, &route).await {
                log::error!("toolbox: open viewer {} failed: {}", route, e);
            }
        });
    } else if route == "/quicklaunch" {
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::command::quicklaunch::open_quicklaunch(app).await {
                log::error!("toolbox: open quicklaunch failed: {}", e);
            }
        });
    } else if route == "/share" {
        let share_state = app.state::<std::sync::Arc<std::sync::Mutex<crate::command::share_server::ShareState>>>().inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = crate::command::share_server::open_share_panel_inner(app, share_state).await {
                log::error!("toolbox: open share panel failed: {}", e);
            }
        });
    } else {
        log::warn!("toolbox: unknown route '{}' for hotkey dispatch", route);
    }
}
