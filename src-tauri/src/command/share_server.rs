//! ShareServer — LAN HTTP file/text sharing.
//!
//! Triggered by a Toolbox card that opens a singleton "share panel" viewer
//! (fixed label `share-panel`). Opening the panel starts a local HTTP service;
//! destroying the panel stops it and clears the temp directory. The service is
//! bound to `0.0.0.0` on a random free port; the panel enumerates physical
//! NIC IPv4 addresses and shows one URL (+ collapsible QR) per address.
//!
//! State is kept in an independent `Arc<Mutex<ShareState>>` managed by Tauri
//! (not `AppState`), minimizing blast radius.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use tauri::{AppHandle, Manager, WebviewWindowBuilder};
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::command::viewer::resolve_window_url;

// ── State model ──────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ShareState {
    pub session: Option<ShareSession>,
}

pub struct ShareSession {
    /// Shared with the axum router so handlers read live items.
    pub items: Arc<Mutex<Vec<ShareItem>>>,
    /// Temp dir holding copied files; dropped on stop → auto-cleaned.
    pub temp_dir: tempfile::TempDir,
    pub port: u16,
    pub shutdown_tx: Option<oneshot::Sender<()>>,
}

/// A shared entry. Serialized to the frontend (file_path skipped).
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShareItem {
    pub id: String,
    /// "file" | "text"
    pub kind: String,
    pub name: String,
    pub size: u64,
    /// Present only for text items (host panel may show/edit).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Server-side only: where the file lives (in temp dir). Skipped on serialize.
    #[serde(skip)]
    pub file_path: Option<PathBuf>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShareUrl {
    /// Network interface name (e.g. "Wi-Fi", "Ethernet") for locating the NIC.
    pub name: String,
    pub ip: String,
    pub port: u16,
    pub url: String,
}

type Items = Arc<Mutex<Vec<ShareItem>>>;

// ── Window: open singleton share panel ───────────────────────────────────────

#[tauri::command]
pub async fn open_share_panel(
    app: AppHandle,
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
) -> Result<(), String> {
    open_share_panel_inner(app, state.inner().clone()).await
}

/// Core logic for opening the share panel, callable from both Tauri commands
/// and non-command contexts (e.g. global hotkey handler).
pub async fn open_share_panel_inner(
    app: AppHandle,
    share_state: Arc<Mutex<ShareState>>,
) -> Result<(), String> {
    if let Some(existing) = app.get_webview_window("share-panel") {
        let _ = existing.set_focus();
        return Ok(());
    }

    let url = resolve_window_url(&app.config());
    let init_script = "window.__INITIAL_HASH__ = '/share'; window.location.hash = '/share';";
    let eval_js = "window.location.hash = '/share';";
    let label = "share-panel".to_string();

    tauri::async_runtime::spawn_blocking(move || {
        let builder = WebviewWindowBuilder::new(&app, &label, url)
            .title("HTTP 共享")
            .inner_size(560.0, 760.0)
            .min_inner_size(460.0, 560.0)
            .initialization_script(init_script);
        match builder.build() {
            Ok(window) => {
                let _ = window.eval(eval_js);
                let _ = window.set_focus();
                // Closing the panel = stop the service (single source of truth).
                let ss = share_state.clone();
                window.on_window_event(move |event| {
                    if matches!(event, tauri::WindowEvent::Destroyed) {
                        log::info!("share-panel destroyed → stopping share server");
                        stop_share_server_inner(&ss);
                    }
                });
            }
            Err(e) => log::error!("open_share_panel: build failed: {}", e),
        }
    })
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Service lifecycle ─────────────────────────────────────────────────────────

#[tauri::command]
pub async fn start_share_server(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
) -> Result<Vec<ShareUrl>, String> {
    // Already running → return existing URLs (singleton).
    {
        let s = state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = &s.session {
            return Ok(build_urls(session.port));
        }
    }

    let listener = tokio::net::TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("无法绑定端口: {}", e))?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();

    let temp_dir = tempfile::TempDir::new().map_err(|e| format!("临时目录创建失败: {}", e))?;
    let items: Items = Arc::new(Mutex::new(Vec::new()));

    let app = Router::new()
        .route("/", get(list_handler))
        .route("/api/items", get(api_items_handler))
        .route("/d/:id", get(download_handler))
        .route("/t/:id", get(text_handler))
        .with_state(items.clone());

    let (tx, rx) = oneshot::channel::<()>();

    {
        let mut s = state.lock().map_err(|e| e.to_string())?;
        s.session = Some(ShareSession {
            items,
            temp_dir,
            port,
            shutdown_tx: Some(tx),
        });
    }

    tauri::async_runtime::spawn(async move {
        if let Err(e) = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = rx.await;
            })
            .await
        {
            log::error!("share server axum error: {}", e);
        }
    });

    Ok(build_urls(port))
}

#[tauri::command]
pub async fn stop_share_server(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
) -> Result<(), String> {
    stop_share_server_inner(state.inner());
    Ok(())
}

/// Inner stop used by both the command and the window `Destroyed` event.
/// Sends the graceful-shutdown signal; dropping the session frees the temp dir.
pub fn stop_share_server_inner(state: &Arc<Mutex<ShareState>>) {
    if let Ok(mut s) = state.lock() {
        if let Some(session) = s.session.take() {
            if let Some(tx) = session.shutdown_tx {
                let _ = tx.send(());
            }
            // `session.temp_dir` (TempDir) drops here → files removed.
        }
    }
}

// ── Entries ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn add_share_file(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
    path: String,
) -> Result<ShareItem, String> {
    let src = PathBuf::from(&path);
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);

    // Copy into the temp dir so source deletion doesn't break sharing.
    let dest = {
        let s = state.lock().map_err(|e| e.to_string())?;
        let session = s.session.as_ref().ok_or("共享服务未启动")?;
        let stem = Uuid::new_v4().to_string();
        let safe_name = sanitize_filename(&name);
        let dest = session.temp_dir.path().join(format!("{}_{}", stem, safe_name));
        std::fs::copy(&src, &dest).map_err(|e| format!("复制失败: {}", e))?;
        dest
    };

    let item = ShareItem {
        id: Uuid::new_v4().to_string(),
        kind: "file".into(),
        name,
        size,
        text: None,
        file_path: Some(dest),
    };

    {
        let s = state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = &s.session {
            if let Ok(mut items) = session.items.lock() {
                items.push(item.clone());
            }
        }
    }

    Ok(item)
}

#[tauri::command]
pub async fn add_share_text(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
    name: String,
    text: String,
) -> Result<ShareItem, String> {
    if text.is_empty() {
        return Err("文本内容不能为空".into());
    }
    let display_name = if name.trim().is_empty() {
        // Use first line as a readable name.
        text.lines()
            .next()
            .map(|l| {
                let t = l.trim();
                if t.len() > 24 { format!("{}…", &t[..24]) } else { t.to_string() }
            })
            .unwrap_or_else(|| "文本".into())
    } else {
        name.trim().to_string()
    };

    let item = ShareItem {
        id: Uuid::new_v4().to_string(),
        kind: "text".into(),
        name: display_name,
        size: text.len() as u64,
        text: Some(text),
        file_path: None,
    };

    {
        let s = state.lock().map_err(|e| e.to_string())?;
        if let Some(session) = &s.session {
            if let Ok(mut items) = session.items.lock() {
                items.push(item.clone());
            }
        }
    }

    Ok(item)
}

#[tauri::command]
pub async fn remove_share_item(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
    id: String,
) -> Result<(), String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    if let Some(session) = &s.session {
        if let Ok(mut items) = session.items.lock() {
            items.retain(|i| i.id != id);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_share_items(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
) -> Result<Vec<ShareItem>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    match &s.session {
        Some(session) => Ok(session
            .items
            .lock()
            .map(|v| v.clone())
            .unwrap_or_default()),
        None => Ok(vec![]),
    }
}

#[tauri::command]
pub async fn get_share_urls(
    state: tauri::State<'_, Arc<Mutex<ShareState>>>,
) -> Result<Vec<ShareUrl>, String> {
    let s = state.lock().map_err(|e| e.to_string())?;
    match &s.session {
        Some(session) => Ok(build_urls(session.port)),
        None => Ok(vec![]),
    }
}

// ── File picker (multi-select) ────────────────────────────────────────────────

#[tauri::command]
pub async fn pick_share_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app
        .dialog()
        .file()
        .blocking_pick_files();
    match picked {
        Some(paths) => Ok(paths
            .into_iter()
            .map(|p| p.to_string())
            .collect()),
        None => Ok(vec![]),
    }
}

// ── axum handlers ────────────────────────────────────────────────────────────

async fn list_handler(State(items): State<Items>) -> Response {
    let guard = match items.lock() {
        Ok(g) => g,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let body = render_list_page(&*guard);
    drop(guard);
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
        body,
    )
        .into_response()
}

/// JSON list of items, used by the client page to refresh without a full reload.
async fn api_items_handler(State(items): State<Items>) -> Response {
    let guard = match items.lock() {
        Ok(g) => g,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let json = match serde_json::to_string(&*guard) {
        Ok(j) => j,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json; charset=utf-8"),
        )],
        json,
    )
        .into_response()
}

/// Build a 404 response whose plain-text body explains *why* the download
/// failed (surfaced by the client debug panel and server log) instead of a bare
/// empty 404 that gives no clue on mobile.
fn download_not_found(reason: String) -> Response {
    log::warn!("share download 404: {}", reason);
    let mut r = Response::new(Body::from(reason));
    if let Ok(v) = HeaderValue::from_str("text/plain; charset=utf-8") {
        r.headers_mut().insert(header::CONTENT_TYPE, v);
    }
    *r.status_mut() = StatusCode::NOT_FOUND;
    r
}

async fn download_handler(
    State(items): State<Items>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let path: PathBuf = {
        let guard = match items.lock() {
            Ok(g) => g,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
        match guard.iter().find(|i| i.id == id && i.kind == "file") {
            Some(item) => match &item.file_path {
                Some(p) => p.clone(),
                None => return download_not_found(format!(
                    "条目 {id} 是 file 但 file_path 为空（数据异常）"
                )),
            },
            None => {
                let total = guard.len();
                let files = guard.iter().filter(|i| i.kind == "file").count();
                let id_only = guard.iter().any(|i| i.id == id);
                return download_not_found(format!(
                    "未找到 id={id}（共 {total} 条，file {files} 条，id 命中但类型不符: {id_only}）"
                ));
            }
        }
    };

    let file = match tokio::fs::File::open(&path).await {
        Ok(f) => f,
        Err(e) => {
            return download_not_found(format!("打开文件失败: {e} (path={path:?})"));
        }
    };
    let fname = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file")
        .to_string();

    let stream = tokio_util::io::ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut resp = Response::new(body);
    resp.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/octet-stream"),
    );
    // RFC 5987: keep an ASCII fallback in `filename` and a UTF-8 percent-encoded
    // value in `filename*`. Raw non-ASCII in the legacy `filename` is invalid and
    // makes some mobile browsers (iOS Safari) silently refuse the download.
    let disp = rfc5987_disposition(&fname);
    match HeaderValue::from_str(&disp) {
        Ok(v) => {
            resp.headers_mut().insert(header::CONTENT_DISPOSITION, v);
        }
        Err(_) => {}
    }
    resp
}

/// Build a `Content-Disposition: attachment` header value safe for non-ASCII
/// (e.g. Chinese) filenames across desktop and mobile browsers.
fn rfc5987_disposition(name: &str) -> String {
    let ascii_fallback: String = name.chars().filter(|c| c.is_ascii()).collect();
    let ascii_fallback = if ascii_fallback.is_empty() {
        "file".to_string()
    } else {
        ascii_fallback.replace(['"', '\\'], "")
    };
    let encoded: String = name
        .bytes()
        .map(|b| {
            let c = b as char;
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
                c.to_string()
            } else {
                format!("%{:02X}", b)
            }
        })
        .collect();
    format!(
        "attachment; filename=\"{}\"; filename*=UTF-8''{}",
        ascii_fallback, encoded
    )
}

async fn text_handler(
    State(items): State<Items>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    let text = {
        let guard = match items.lock() {
            Ok(g) => g,
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
        match guard.iter().find(|i| i.id == id && i.kind == "text") {
            Some(item) => item.text.clone().unwrap_or_default(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };
    (
        [(header::CONTENT_TYPE, HeaderValue::from_static("text/plain; charset=utf-8"))],
        text,
    )
        .into_response()
}

// ── Rendering & helpers ──────────────────────────────────────────────────────

/// Client page assets, compiled into the binary via `include_str!` so the
/// share server ships as a single executable with no extra static files.
/// HTML/CSS/JS live in `src/share-client/` for normal editor support & linting.
const LIST_HTML: &str = include_str!("../share-client/template.html");
const LIST_CSS: &str = include_str!("../share-client/styles.css");
const CLIENT_JS: &str = include_str!("../share-client/client.js");

fn render_list_page(items: &[ShareItem]) -> String {
    let rows: String = if items.is_empty() {
        "<div class=\"empty\">还没有共享内容，在 jPaste 面板中添加文件或文本。</div>".to_string()
    } else {
        items
            .iter()
            .map(|item| match item.kind.as_str() {
                "file" => format!(
                    "<div class=\"entry\"><span class=\"ico\"><svg class=\"ico-svg\"><use href=\"#doc-icon\"/></svg></span>\
                     <span class=\"name\">{name}</span>\
                     <span class=\"size\">{size}</span>\
                     <a class=\"btn\" href=\"/d/{id}\"><svg class=\"btn-svg\"><use href=\"#dl-icon\"/></svg><span>下载</span></a></div>",
                    id = item.id,
                    name = escape_html(&item.name),
                    size = human_size(item.size),
                ),
                _ => format!(
                    "<div class=\"entry\"><span class=\"ico\"><svg class=\"ico-svg\"><use href=\"#text-icon\"/></svg></span>\
                     <span class=\"name\">{name}</span>\
                     <span class=\"size\">{size}</span>\
                     <button class=\"btn\" type=\"button\" onclick=\"copyText('t-{id}', this)\"><svg class=\"btn-svg\"><use href=\"#cp-icon\"/></svg><span>复制</span></button></div>\
                     <pre id=\"t-{id}\" class=\"text collapsed\">{text}</pre>\
                     <button class=\"toggle\" type=\"button\" onclick=\"toggleText(this)\">展开</button>",
                    id = item.id,
                    name = escape_html(&item.name),
                    size = human_size(item.size),
                    text = escape_html(item.text.as_deref().unwrap_or("")),
                ),
            })
            .collect()
    };

    LIST_HTML
        .replace("__CSS__", LIST_CSS)
        .replace("__ROWS__", &rows)
        .replace("__JS__", CLIENT_JS)
        .to_string()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut i = 0;
    while size >= 1024.0 && i < UNITS.len() - 1 {
        size /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", size, UNITS[i])
    }
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '.' || c == '-' || c == '_' || c == ' ' {
            c
        } else {
            '_'
        })
        .collect()
}

// ── Physical NIC enumeration (delegated to crate::net) ────────────────────

fn build_urls(port: u16) -> Vec<ShareUrl> {
    crate::net::list_physical_nics()
        .into_iter()
        .map(|(name, ip)| ShareUrl {
            name,
            ip: ip.clone(),
            port,
            url: format!("http://{}:{}/", ip, port),
        })
        .collect()
}
