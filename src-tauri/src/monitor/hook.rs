//! Keyboard hook module
//!
//! WH_KEYBOARD_LL global keyboard hook for intercepting Ctrl+V to implement queue paste mode.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    keybd_event, GetAsyncKeyState, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VK_CONTROL, VK_V,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, PostThreadMessageW, SetWindowsHookExA, UnhookWindowsHookEx,
    DispatchMessageW, TranslateMessage, MSG, WH_KEYBOARD_LL, KBDLLHOOKSTRUCT, WM_QUIT,
};

/// Thread ID of the hook message loop, for sending WM_QUIT on stop.
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);

/// Set while simulate_paste() is injecting Ctrl+V — hook ignores input during this window.
static SIMULATING: AtomicBool = AtomicBool::new(false);

/// Global keyboard hook for intercepting Ctrl+V to implement queue paste mode.
///
/// Uses a channel-based approach: the hook sends a Ctrl+V signal,
/// and a processing thread receives it and performs the paste operation.
pub struct KeyboardHook {
    running: Arc<AtomicBool>,
    thread_handle: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl KeyboardHook {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: Mutex::new(None),
        }
    }

    /// Start the keyboard hook.
    pub fn start(&self, on_ctrl_v: Arc<dyn Fn() + Send + Sync + 'static>) -> bool {
        let running = self.running.clone();
        running.store(true, Ordering::SeqCst);

        // Set the callback in thread-local storage
        set_hook_callback(on_ctrl_v);

        let handle = std::thread::spawn(move || {
            // Store thread ID so stop() can post WM_QUIT
            let thread_id = unsafe { windows::Win32::System::Threading::GetCurrentThreadId() };
            HOOK_THREAD_ID.store(thread_id, Ordering::SeqCst);

            let hinstance: HINSTANCE = unsafe {
                windows::Win32::System::LibraryLoader::GetModuleHandleA(None)
                    .unwrap_or_default()
                    .into()
            };

            let hook = unsafe {
                SetWindowsHookExA(WH_KEYBOARD_LL, Some(Self::hook_proc), Some(hinstance), 0)
            };

            let hook = match hook {
                Ok(h) if !h.is_invalid() => h,
                _ => {
                    log::error!("[hook] Failed to set WH_KEYBOARD_LL hook");
                    return;
                }
            };

            log::info!("[hook] WH_KEYBOARD_LL installed (thread id={})", thread_id);

            let mut msg = MSG::default();
            loop {
                let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if result.0 == 0 || msg.message == WM_QUIT {
                    break;
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }
            }

            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
            clear_hook_callback();
            log::info!("[hook] WH_KEYBOARD_LL uninstalled");
        });

        *self.thread_handle.lock().unwrap() = Some(handle);
        true
    }

    /// Stop the keyboard hook
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        clear_hook_callback();
        // Post WM_QUIT to the hook thread's message loop so GetMessageW returns and the thread exits
        let tid = HOOK_THREAD_ID.load(Ordering::SeqCst);
        if tid != 0 {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_QUIT, WPARAM(0), LPARAM(0));
            }
        }
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
        HOOK_THREAD_ID.store(0, Ordering::SeqCst);
    }

    /// Simulate Ctrl+V key press.
    /// Sets SIMULATING flag so the hook ignores the injected keystrokes.
    pub fn simulate_paste() {
        log::info!("[hook] simulate_paste: setting SIMULATING flag, injecting Ctrl+V");
        SIMULATING.store(true, Ordering::SeqCst);
        unsafe {
            keybd_event(VK_CONTROL.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            keybd_event(VK_V.0 as u8, 0, KEYBD_EVENT_FLAGS(0), 0);
            std::thread::sleep(std::time::Duration::from_millis(10));
            keybd_event(VK_V.0 as u8, 0, KEYEVENTF_KEYUP, 0);
            keybd_event(VK_CONTROL.0 as u8, 0, KEYEVENTF_KEYUP, 0);
        }
        // Small delay: wait for the keyup events to be processed before clearing the flag
        std::thread::sleep(std::time::Duration::from_millis(50));
        log::info!("[hook] simulate_paste: clearing SIMULATING flag, done");
        SIMULATING.store(false, Ordering::SeqCst);
    }

    unsafe extern "system" fn hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        const HC_ACTION: i32 = 0;
        const WM_KEYDOWN: u32 = 0x0100;

        // Ignore keystrokes injected by simulate_paste()
        if SIMULATING.load(Ordering::SeqCst) {
            return CallNextHookEx(None, code, wparam, lparam);
        }

        if code >= HC_ACTION && wparam.0 as u32 == WM_KEYDOWN {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            if kb.vkCode == VK_V.0 as u32 {
                let ctrl_pressed = GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000 != 0;
                if ctrl_pressed {

                    with_hook_callback(|cb| cb());
                    return LRESULT(1);
                }
            }
        }

        CallNextHookEx(None, code, wparam, lparam)
    }
}

impl Default for KeyboardHook {
    fn default() -> Self {
        Self::new()
    }
}

// ── Callback management ────────────────────────────────────────────────-

use std::sync::Arc;

static HOOK_CALLBACK: once_cell::sync::Lazy<Mutex<Option<Arc<dyn Fn() + Send + Sync + 'static>>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(None));

fn set_hook_callback(cb: Arc<dyn Fn() + Send + Sync + 'static>) {
    let mut guard = HOOK_CALLBACK.lock().unwrap();
    *guard = Some(cb);
}

fn clear_hook_callback() {
    let mut guard = HOOK_CALLBACK.lock().unwrap();
    *guard = None;
}

fn with_hook_callback<F>(f: F)
where
    F: FnOnce(&Arc<dyn Fn() + Send + Sync + 'static>),
{
    if let Ok(guard) = HOOK_CALLBACK.lock() {
        if let Some(ref cb) = *guard {
            f(cb);
        }
    }
}
