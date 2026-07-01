//! jPaste v2 — Entry Point
//!
//! Bootstraps the application: single instance check, then starts the event loop.

use anyhow::Result;

fn main() -> Result<()> {
    // ── Single instance check ─────────────────────────────────
    if !check_single_instance() {
        eprintln!("jPaste is already running.");
        std::process::exit(0);
    }

    // ── Initialize app ────────────────────────────────────────
    let data_dir = jpastev2::app::data_dir();
    std::fs::create_dir_all(&data_dir)?;

    let app = jpastev2::app::App::new(&data_dir)?;

    // ── Run event loop (takes ownership) ──────────────────────
    app.run()
}

/// Check for existing instance via named mutex.
fn check_single_instance() -> bool {
    use windows::core::w;
    use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        let mutex = CreateMutexW(None, false, w!("jPastev2")).unwrap_or_default();
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            return false;
        }
        let _ = mutex;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_single_instance_ok() {
        // Just verify the function signature works
        let _ = check_single_instance;
    }
}
