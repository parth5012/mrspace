//! Focus-follows-mouse (ROADMAP Tier 2.7, optional). Installed as a
//! low-level mouse hook that only acts when `config.focus_follows_mouse`
//! is true; otherwise it passes through with near-zero cost.

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::OnceLock;

static LAST_MOVE_MS: AtomicI64 = AtomicI64::new(0);

pub fn start() {
    std::thread::spawn(|| unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::*;
        use windows_sys::Win32::Foundation::*;
        let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(hook_proc), std::ptr::null_mut(), 0);
        if hook.is_null() { return; }
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if GetMessageW(&mut msg, 0 as _, 0, 0) <= 0 { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn hook_proc(code: i32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    use windows_sys::Win32::Foundation::*;
    if code >= 0 && wparam == WM_MOUSEMOVE as usize {
        let cfg = crate::config::load();
        if cfg.focus_follows_mouse {
            let info = &*(lparam as *const MSLLHOOKSTRUCT);
            let now = (std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0));
            let last = LAST_MOVE_MS.load(Ordering::Relaxed);
            // only focus when the cursor has settled a bit (avoid churn)
            if now - last > 60 {
                LAST_MOVE_MS.store(now, Ordering::Relaxed);
                let pt = info.pt;
                let hwnd = WindowFromPoint(pt);
                if !hwnd.is_null() {
                    let fg = GetForegroundWindow();
                    if hwnd != fg && crate::win32::is_manageable(hwnd) {
                        SetForegroundWindow(hwnd);
                    }
                }
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}