//! Source tracking — identifies the application that wrote clipboard content.

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
};

#[derive(Debug, Clone, Default)]
pub struct ClipboardSource {
    pub exe: String,
    pub title: String,
}

/// Get the source application info for clipboard content.
pub fn get_clipboard_source() -> ClipboardSource {
    let hwnd = unsafe { GetForegroundWindow() };

    if hwnd.is_invalid() {
        return ClipboardSource::default();
    }

    let title = get_window_title(hwnd);
    let exe = get_process_path(hwnd);

    ClipboardSource { exe, title }
}

fn get_window_title(hwnd: HWND) -> String {
    unsafe {
        let mut buf = [0u16; 256];
        let len = GetWindowTextW(hwnd, &mut buf);
        if len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

fn get_process_path(hwnd: HWND) -> String {
    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }
    if pid == 0 {
        return String::new();
    }

    unsafe {
        let handle = match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };
        if handle.is_invalid() {
            return String::new();
        }

        let mut buf = [0u16; 260];
        let mut len = 260u32;
        let result = QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_FORMAT(0),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        );
        let _ = CloseHandle(handle);

        if result.is_ok() && len > 0 {
            String::from_utf16_lossy(&buf[..len as usize])
        } else {
            String::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_source() {
        let s = ClipboardSource::default();
        assert_eq!(s.exe, "");
        assert_eq!(s.title, "");
    }
}
