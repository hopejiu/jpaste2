//! FiloStack (paste queue) related Tauri commands

use crate::command::{lock_state, AppState};
use crate::monitor::hook::KeyboardHook;
use std::sync::{Arc, Mutex, mpsc};
use tauri::{State, Emitter};

#[tauri::command]
pub fn get_filo_status(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<crate::service::filostack::FiloStatus, String> {
    let s = lock_state!(state);
    Ok(s.filostack.get_status())
}

#[tauri::command]
pub fn filo_set_mode(
    state: State<'_, Arc<Mutex<AppState>>>,
    mode: String,
) -> Result<(), String> {
    log::debug!("cmd::filo_set_mode: mode={}", mode);
    let mut s = lock_state!(state);

    // If entering queue mode, start the keyboard hook
    if mode == "queue" {
        s.keyboard_hook.stop();

        let (tx, rx) = mpsc::channel::<()>();
        s.ctrl_v_sender = Mutex::new(Some(tx.clone()));

        let filostack = s.filostack.clone();
        let clipboard_mgr = s.clipboard_mgr.clone();
        let app_handle = s.app_handle.clone();

        s.keyboard_hook.start(Arc::new(move || {
            let _ = tx.send(());
        }));

        std::thread::spawn(move || {
            log::debug!("[filothread] Ctrl+V processing thread started");
            let mut last_pasted: Option<String> = None;
            while let Ok(()) = rx.recv() {
                match filostack.pop() {
                    Some(text) => {
                        last_pasted = Some(text.clone());
                        filostack.mark_self_write(&text);
                        if let Ok(mut mgr) = clipboard_mgr.lock() {
                            if let Err(e) = mgr.write_text(&text) {
                                log::error!("[filothread] write_text FAILED: {}", e);
                            }
                        }
                        KeyboardHook::simulate_paste();
                        // Show toast when queue just became empty
                        if filostack.len() == 0 {
                            if let Some(ref app) = app_handle {
                                crate::toast::create_toast_window_inner(app, "队列已清空", "jPaste", "clipboard", 0, "", &[]);
                            }
                        }
                    }
                    None => {
                        // Queue empty: paste last item if available
                        if let Some(ref text) = last_pasted {
                            filostack.mark_self_write(text);
                            if let Ok(mut mgr) = clipboard_mgr.lock() {
                                if let Err(e) = mgr.write_text(text) {
                                    log::error!("[filothread] write_text (reuse) FAILED: {}", e);
                                }
                            }
                            KeyboardHook::simulate_paste();
                        }
                    }
                }
            }
            log::debug!("[filothread] Ctrl+V processing thread exiting");
        });
    } else {
        s.keyboard_hook.stop();
        s.ctrl_v_sender = Mutex::new(None);
    }

    s.filostack.set_mode(&mode);
    if let Ok(settings) = s.settings.get_settings() {
        let mut new_settings = settings;
        new_settings.paste_order = mode.clone();
        let _ = s.settings.save_settings(new_settings);
    }
    if let Some(ref app) = s.app_handle {
        let _ = app.emit("paste-order-changed", &mode);
    }
    Ok(())
}

#[tauri::command]
pub fn filo_clear(
    state: State<'_, Arc<Mutex<AppState>>>,
) -> Result<(), String> {
    let s = lock_state!(state);
    s.filostack.clear();
    log::info!("cmd::filo_clear: cleared queue");
    Ok(())
}
