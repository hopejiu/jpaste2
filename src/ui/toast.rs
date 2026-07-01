//! Toast notification — pure Win32 frameless layered window.
//!
//! Runs on a background thread with its own message pump.
//! Shows a dark-colored overlay at the bottom-right of the screen,
//! auto-hides after 3 seconds. Uses GDI for background fill.

use std::thread;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, EndPaint, FillRect, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, GetSystemMetrics,
    KillTimer, LoadCursorW, RegisterClassW, SetLayeredWindowAttributes, SetTimer,
    SetWindowPos, SetWindowTextW, ShowWindow, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    HWND_TOPMOST, IDC_ARROW, LAYERED_WINDOW_ATTRIBUTES_FLAGS, SM_CXSCREEN, SM_CYSCREEN,
    SW_HIDE, WINDOW_EX_STYLE, WINDOW_STYLE, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_POPUP,
};

// ═══════════════════════════════════════════════════════════════════
// Constants
// ═══════════════════════════════════════════════════════════════════

const WIDTH: i32 = 340;
const HEIGHT: i32 = 80;
const MARGIN: i32 = 16;
const AUTO_HIDE_MS: u32 = 3000;
const LWA_ALPHA: u32 = 0x00000002;

// ═══════════════════════════════════════════════════════════════════
// Public API
// ═══════════════════════════════════════════════════════════════════

/// Commands for the toast window thread.
#[derive(Debug, Clone)]
pub enum ToastCommand {
    Show { title: String, message: String, opacity: u8 },
    Hide,
    SetOpacity(u8),
    Quit,
}

/// Handle for controlling the toast window from the main thread.
pub struct ToastHandle {
    cmd_tx: Sender<ToastCommand>,
}

impl ToastHandle {
    /// Show a toast notification.
    pub fn show(&self, title: &str, message: &str, opacity: u8) {
        let _ = self.cmd_tx.send(ToastCommand::Show {
            title: title.into(),
            message: message.into(),
            opacity,
        });
    }

    /// Hide the toast.
    pub fn hide(&self) {
        let _ = self.cmd_tx.send(ToastCommand::Hide);
    }

    /// Stop the toast thread.
    pub fn quit(&self) {
        let _ = self.cmd_tx.send(ToastCommand::Quit);
    }
}

/// Start the toast window thread. Returns a handle for control.
pub fn start_toast_thread() -> ToastHandle {
    let (cmd_tx, cmd_rx) = crossbeam_channel::unbounded::<ToastCommand>();

    thread::Builder::new()
        .name("toast-window".into())
        .spawn(move || {
            run_toast_loop(cmd_rx);
        })
        .expect("spawn toast thread");

    ToastHandle { cmd_tx }
}

// ═══════════════════════════════════════════════════════════════════
// Window management
// ═══════════════════════════════════════════════════════════════════

struct ToastState {
    title: String,
    message: String,
    opacity: u8,
}

unsafe fn register_class() -> u16 {
    use windows::Win32::UI::WindowsAndMessaging::WNDCLASSW;
    let hinstance = GetModuleHandleA(None).unwrap_or_default();
    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance.into(),
        hIcon: Default::default(),
        hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
        hbrBackground: Default::default(),
        lpszMenuName: windows::core::PCWSTR::null(),
        lpszClassName: w!("jPasteToast"),
    };
    RegisterClassW(&wc)
}

unsafe fn create_window() -> HWND {
    CreateWindowExW(
        WINDOW_EX_STYLE(WS_EX_TOOLWINDOW.0 | WS_EX_TOPMOST.0 | WS_EX_LAYERED.0),
        w!("jPasteToast"),
        w!("jPaste"),
        WINDOW_STYLE(WS_POPUP.0),
        0, 0, WIDTH, HEIGHT,
        None,
        None,
        GetModuleHandleA(None).ok().map(|h| h.into()),
        None,
    )
    .unwrap_or_default()
}

unsafe fn show_toast(hwnd: HWND, state: &ToastState) {
    let screen_w = GetSystemMetrics(SM_CXSCREEN);
    let screen_h = GetSystemMetrics(SM_CYSCREEN);
    let x = screen_w - WIDTH - MARGIN;
    let y = screen_h - HEIGHT - MARGIN;

    let _ = SetLayeredWindowAttributes(hwnd, COLORREF::default(), state.opacity, LAYERED_WINDOW_ATTRIBUTES_FLAGS(LWA_ALPHA));
    let _ = SetWindowPos(
        hwnd,
        Some(HWND_TOPMOST),
        x, y, WIDTH, HEIGHT,
        windows::Win32::UI::WindowsAndMessaging::SWP_NOACTIVATE
            | windows::Win32::UI::WindowsAndMessaging::SWP_SHOWWINDOW,
    );

    // Store text — WM_PAINT can read it via GetWindowTextW
    let display = format!("{}|{}", state.title, state.message);
    let wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = SetWindowTextW(hwnd, windows::core::PCWSTR::from_raw(wide.as_ptr()));

    let _ = SetTimer(Some(hwnd), 1, AUTO_HIDE_MS, None);
}

unsafe fn hide_toast(hwnd: HWND) {
    let _ = KillTimer(Some(hwnd), 1);
    let _ = ShowWindow(hwnd, SW_HIDE);
}

/// Main message loop.
fn run_toast_loop(rx: Receiver<ToastCommand>) {
    unsafe {
        if register_class() == 0 {
            log::error!("toast: RegisterClassW failed");
            return;
        }

        let hwnd = create_window();
        if hwnd.is_invalid() {
            log::error!("toast: CreateWindowExW failed");
            return;
        }

        let mut state = ToastState {
            title: String::new(),
            message: String::new(),
            opacity: 100,
        };

        let mut msg = std::mem::zeroed();
        loop {
            // Non-blocking crossbeam command check
            loop {
                match rx.try_recv() {
                    Ok(ToastCommand::Show { title, message, opacity }) => {
                        state.title = title;
                        state.message = message;
                        state.opacity = opacity;
                        show_toast(hwnd, &state);
                    }
                    Ok(ToastCommand::Hide) => hide_toast(hwnd),
                    Ok(ToastCommand::SetOpacity(o)) => {
                        state.opacity = o;
                        let _ = SetLayeredWindowAttributes(hwnd, COLORREF::default(), o, LAYERED_WINDOW_ATTRIBUTES_FLAGS(LWA_ALPHA));
                    }
                    Ok(ToastCommand::Quit) => {
                        let _ = DestroyWindow(hwnd);
                        return;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        let _ = DestroyWindow(hwnd);
                        return;
                    }
                }
            }

            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                return; // WM_QUIT
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
// Window procedure
// ═══════════════════════════════════════════════════════════════════

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // WM_PAINT — fill background with dark color + purple accent bar
        windows::Win32::UI::WindowsAndMessaging::WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if !hdc.is_invalid() {
                let bg = RECT { left: 0, top: 0, right: WIDTH, bottom: HEIGHT };
                let brush = CreateSolidBrush(COLORREF(0x2A2834));
                let _ = FillRect(hdc, &bg as *const RECT, brush);
                let _ = DeleteObject(brush.into());

                let accent = RECT { left: 0, top: 0, right: 4, bottom: HEIGHT };
                let accent_brush = CreateSolidBrush(COLORREF(0x9C80FF));
                let _ = FillRect(hdc, &accent as *const RECT, accent_brush);
                let _ = DeleteObject(accent_brush.into());

                let _ = EndPaint(hwnd, &ps);
            }
            LRESULT(0)
        }
        // WM_TIMER — auto-hide after 3 seconds
        windows::Win32::UI::WindowsAndMessaging::WM_TIMER => {
            let _ = KillTimer(Some(hwnd), 1);
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        // WM_CLOSE — hide instead of destroying
        windows::Win32::UI::WindowsAndMessaging::WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        // WM_DESTROY — post quit
        windows::Win32::UI::WindowsAndMessaging::WM_DESTROY => {
            windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0);
            LRESULT(0)
        }
        _ => windows::Win32::UI::WindowsAndMessaging::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ═══════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_toast_show_hide() {
        let handle = start_toast_thread();
        handle.show("jPaste", "测试通知", 100);
        handle.hide();
        handle.quit();
    }
}
