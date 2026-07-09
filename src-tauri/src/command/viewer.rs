//! Viewer window commands
//!
//! Windows are created via `WebviewUrl::App("/")` or `WebviewUrl::External`
//! depending on whether `devServerAlive()` detects the Vite dev server.
//! The SPA route is set in the init script (`window.location.hash` +
//! `window.__INITIAL_HASH__`), with an `eval()` after `build()` as fallback.
//!
//! Commands are `async` because `WebviewWindowBuilder::build()` blocks
//! (it dispatches to the main thread and waits synchronously).  Async
//! commands run on the async runtime, unblocking the main thread.
//!
//! All viewers share one `open_viewer(route, id)` command driven by the
//! `VIEWERS` registry below — adding a new viewer is a one-line change.

use std::net::{TcpStream, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Config, Manager, State, WebviewUrl};
use uuid::Uuid;

/// Return `true` if the Vite dev server is reachable on its configured port.
pub(crate) fn dev_server_alive(config: &Config) -> bool {
    let dev_url = match &config.build.dev_url {
        Some(u) => u,
        None => return false,
    };
    let host = dev_url.host_str().unwrap_or("127.0.0.1");
    let port = dev_url.port().unwrap_or(1420);
    let addr = format!("{}:{}", host, port);
    match addr.to_socket_addrs() {
        Ok(mut addrs) => addrs.any(|sock| TcpStream::connect_timeout(&sock, Duration::from_millis(100)).is_ok()),
        Err(_) => false,
    }
}

/// Find the built dist on disk and return an `asset://localhost/…` URL.
///
/// This is the fallback when the Vite dev server is unreachable but the
/// frontend dist is still available on disk (e.g. after `npm run build`).
fn resolve_dist_asset_url(config: &Config) -> Option<url::Url> {
    let frontend = config.build.frontend_dist.as_ref()?;
    let rel = PathBuf::from(frontend.to_string());
    let cwd = std::env::current_dir().ok()?;

    // frontend_dist is relative to the config file (src-tauri/).
    // Try CWD as project root or as src-tauri/.
    for base in [cwd.join("src-tauri"), cwd] {
        let dist = base.join(&rel);
        if dist.join("index.html").exists() {
            let abs = std::fs::canonicalize(&dist).ok()?;
            let path_str = abs.to_str()?.replace('\\', "/");
            let url_str = format!("asset://localhost/{}/index.html", path_str);
            log::debug!("resolve_dist_asset_url: found dist at {url_str}");
            return url_str.parse().ok();
        }
    }
    log::warn!("resolve_dist_asset_url: dist/index.html not found on disk");
    None
}

/// Select the best URL for a viewer/toast window.
///
/// Priority: dev server reachable → `App("/")`, else asset:// fallback
/// (dev mode with dead server), else `App("/")` (production bundled assets).
pub(crate) fn resolve_window_url(config: &Config) -> WebviewUrl {
    if dev_server_alive(config) {
        log::debug!("resolve_window_url: dev server reachable, using App URL");
        WebviewUrl::App("/".into())
    } else if config.build.dev_url.is_some() {
        match resolve_dist_asset_url(config) {
            Some(asset_url) => WebviewUrl::External(asset_url),
            None => {
                log::warn!("resolve_window_url: dev server dead and no dist on disk — window may be blank");
                WebviewUrl::App("/".into())
            }
        }
    } else {
        WebviewUrl::App("/".into())
    }
}

/// Metadata describing how to open one viewer type.
struct ViewerMeta {
    /// Window label prefix (label = `{label_prefix}-{id}`).
    label_prefix: &'static str,
    /// SPA route the window navigates to.
    route: &'static str,
    /// Window title.
    title: &'static str,
    w: f64,
    h: f64,
    min_w: f64,
    min_h: f64,
}

/// Registry of all viewer windows. Adding a viewer = adding one row here.
const VIEWERS: &[ViewerMeta] = &[
    ViewerMeta { label_prefix: "json-viewer", route: "/viewer/json", title: "JSON 查看", w: 1200.0, h: 800.0, min_w: 600.0, min_h: 400.0 },
    ViewerMeta { label_prefix: "curl-viewer", route: "/viewer/curl", title: "HTTP 调试", w: 1000.0, h: 750.0, min_w: 700.0, min_h: 500.0 },
    ViewerMeta { label_prefix: "ws-viewer", route: "/viewer/ws", title: "WS 调试", w: 900.0, h: 650.0, min_w: 600.0, min_h: 450.0 },
    ViewerMeta { label_prefix: "calc-viewer", route: "/viewer/calc", title: "计算器", w: 450.0, h: 600.0, min_w: 400.0, min_h: 500.0 },
    ViewerMeta { label_prefix: "decoder-viewer", route: "/viewer/decoder", title: "解码工具", w: 750.0, h: 550.0, min_w: 550.0, min_h: 450.0 },
    ViewerMeta { label_prefix: "timestamp-viewer", route: "/viewer/timestamp", title: "时间戳转换", w: 550.0, h: 500.0, min_w: 450.0, min_h: 400.0 },
    ViewerMeta { label_prefix: "qr-viewer", route: "/viewer/qr", title: "二维码生成", w: 520.0, h: 620.0, min_w: 460.0, min_h: 520.0 },
    ViewerMeta { label_prefix: "svg-viewer", route: "/viewer/svg", title: "SVG 转 PNG", w: 640.0, h: 640.0, min_w: 520.0, min_h: 520.0 },
];

fn viewer_meta(route: &str) -> Option<&'static ViewerMeta> {
    VIEWERS.iter().find(|v| v.route == route)
}

/// Build a viewer window.
pub(crate) async fn build_viewer_window(
    app: AppHandle,
    label: String,
    hash: String,
    title: String,
    w: f64,
    h: f64,
    min_w: f64,
    min_h: f64,
) -> Result<(), String> {
    let url = resolve_window_url(&app.config());

    let init_script = format!(
        "window.__INITIAL_HASH__ = '{}'; window.location.hash = '{}';",
        hash, hash
    );
    let eval_js = format!("window.location.hash = '{}';", hash);

    let label_for_log = label.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        let builder = tauri::WebviewWindowBuilder::new(&app, &label, url)
            .title(&title)
            .inner_size(w, h)
            .min_inner_size(min_w, min_h)
            .initialization_script(&init_script);
        match builder.build() {
            Ok(window) => {
                if let Err(e) = window.eval(&eval_js) {
                    log::error!("build_viewer_window: label={} eval FAILED: {}", label, e);
                }
                // Bring window to front so it's not hidden behind other windows
                let _ = window.set_focus();
                Ok(window)
            }
            Err(e) => {
                log::error!("build_viewer_window: label={} build FAILED: {}", label, e);
                Err(e)
            }
        }
    })
    .await
    .map_err(|e| {
        log::error!("build_viewer_window: label={} join FAILED: {}", label_for_log, e);
        e.to_string()
    })?;

    match result {
        Ok(_window) => Ok(()),
        Err(e) => {
            log::error!("build_viewer_window: label={} done FAILED: {}", label_for_log, e);
            Err(e.to_string())
        }
    }
}

/// Open a viewer window by route + entry id.
///
/// `route` selects the viewer from the `VIEWERS` registry (or the special
/// `/viewer/image` case with dynamic sizing). If a window for this entry
/// already exists, it is reused (no duplicate window).
///
/// `id = -1` opens a blank viewer (no data loading), used by the toolbox.
/// `id > 0` opens a viewer populated with clipboard entry data.
#[tauri::command]
pub async fn open_viewer(
    app: AppHandle,
    state: State<'_, Arc<Mutex<crate::command::AppState>>>,
    route: String,
    id: i64,
) -> Result<(), String> {
    if id == -1 {
        return open_blank_viewer(app, &route).await;
    }

    // Image viewer needs special handling for dynamic sizing.
    if route == "/viewer/image" {
        let label = format!("image-viewer-{}", id);
        if let Some(existing) = app.get_webview_window(&label) {
            let _ = existing.set_focus();
            return Ok(());
        }

        let (win_w, win_h) = {
            let s = state.lock().map_err(|e| e.to_string())?;
            let entry = s.history.get_entry_image_dimensions(id).map_err(|e| e.to_string())?;
            entry
        };

        let max_w = 800.0;
        let max_h = 600.0;
        let min_w = 400.0;
        let min_h = 300.0;

        let w = win_w.clamp(min_w, max_w);
        let h = win_h.clamp(min_h, max_h);

        return build_viewer_window(
            app,
            label,
            format!("/viewer/image?id={}", id),
            "图片查看".into(),
            w,
            h,
            min_w,
            min_h,
        )
        .await;
    }

    let meta = viewer_meta(&route).ok_or_else(|| format!("unknown viewer route: {}", route))?;
    let label = format!("{}-{}", meta.label_prefix, id);
    if let Some(existing) = app.get_webview_window(&label) {
        let _ = existing.set_focus();
        return Ok(());
    }
    let hash = format!("{}?id={}", meta.route, id);
    build_viewer_window(app, label, hash, meta.title.into(), meta.w, meta.h, meta.min_w, meta.min_h).await
}

/// Open a blank viewer window (toolbox-triggered, no entry data).
async fn open_blank_viewer(app: AppHandle, route: &str) -> Result<(), String> {
    let meta = viewer_meta(route).ok_or_else(|| format!("unknown viewer route: {}", route))?;
    // Use a unique label so each toolbox click opens a fresh window
    let uid = Uuid::new_v4().to_string()[..8].to_string();
    let label = format!("{}-blank-{}", meta.label_prefix, uid);
    let hash = format!("{}?id=-1", meta.route);
    build_viewer_window(app, label, hash, meta.title.into(), meta.w, meta.h, meta.min_w, meta.min_h).await
}
