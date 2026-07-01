//! WH_KEYBOARD_LL global keyboard hook for FiloStack.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, UnhookWindowsHookEx, MSG, WH_KEYBOARD_LL,
    KBDLLHOOKSTRUCT,
};

static CALLBACK: Mutex<Option<Box<dyn FnMut() -> bool + Send>>> = Mutex::new(None);
static HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub fn install_hook<F>(on_ctrl_v: F)
where
    F: FnMut() -> bool + Send + 'static,
{
    *CALLBACK.lock().unwrap() = Some(Box::new(on_ctrl_v));

    HOOK_INSTALLED.store(true, Ordering::SeqCst);

    thread::Builder::new()
        .name("filo-hook".into())
        .spawn(move || unsafe {
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(hook_callback),
                Some(HINSTANCE::default()),
                0,
            )
            .expect("SetWindowsHookExW failed");

            if hook.0.is_null() {
                HOOK_INSTALLED.store(false, Ordering::SeqCst);
                return;
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}

            let _ = UnhookWindowsHookEx(hook);
            HOOK_INSTALLED.store(false, Ordering::SeqCst);
        })
        .expect("spawn filo-hook thread");
}

pub fn is_hook_installed() -> bool {
    HOOK_INSTALLED.load(Ordering::SeqCst)
}

unsafe extern "system" fn hook_callback(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    const WM_KEYDOWN: u32 = 0x0100;
    const VK_V: u16 = 0x56;
    const VK_CONTROL: i32 = 0x11;

    if code >= 0 && wparam.0 == WM_KEYDOWN as usize {
        let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        if kb.vkCode as u16 == VK_V {
            let ctrl_state =
                windows::Win32::UI::Input::KeyboardAndMouse::GetKeyState(VK_CONTROL);
            if ctrl_state < 0 {
                if let Ok(mut guard) = CALLBACK.lock() {
                    if let Some(ref mut cb) = *guard {
                        if cb() {
                            return LRESULT(1);
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_not_installed_by_default() {
        assert!(!is_hook_installed());
    }
}
