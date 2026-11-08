use crate::config;
use crate::win32;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Once;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST: AtomicU64 = AtomicU64::new(0);

static INIT: Once = Once::new();
pub fn init() {
    INIT.call_once(|| {
        thread::spawn(|| loop {
            thread::sleep(std::time::Duration::from_millis(500));
            if ACTIVE.load(Ordering::Relaxed) {
                let now = now_ms();
                let last = LAST.load(Ordering::Relaxed);
                let cfg = config::load();
                if now.saturating_sub(last) > cfg.resize_timeout_ms {
                    set_active(false);
                }
            }
        });
    });
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub fn is_active() -> bool { ACTIVE.load(Ordering::Relaxed) }

pub fn toggle() {
    let new = !ACTIVE.load(Ordering::Relaxed);
    set_active(new);
}

fn set_active(v: bool) {
    ACTIVE.store(v, Ordering::Relaxed);
    if v {
        LAST.store(now_ms(), Ordering::Relaxed);
        show_overlay(true);
    } else {
        show_overlay(false);
    }
}

fn touch() { LAST.store(now_ms(), Ordering::Relaxed); }

pub fn nudge(dir: &str) {
    if !is_active() { return; }
    touch();
    let step = config::load().resize_step;
    let h = win32::foreground();
    if h.is_null() { return; }
    let r = match win32::window_rect(h) { Some(r) => r, None => return };
    let (x, y, w, hgt) = match dir {
        "left" => (r.left - step, r.top, r.right - r.left, r.bottom - r.top),
        "right" => (r.left + step, r.top, r.right - r.left, r.bottom - r.top),
        "up" => (r.left, r.top - step, r.right - r.left, r.bottom - r.top),
        "down" => (r.left, r.top + step, r.right - r.left, r.bottom - r.top),
        _ => return,
    };
    win32::move_resize(h, x, y, w, hgt);
}

pub fn grow() {
    if !is_active() { return; }
    touch();
    let step = config::load().resize_step;
    let h = win32::foreground();
    if h.is_null() { return; }
    let r = match win32::window_rect(h) { Some(r) => r, None => return };
    let w = (r.right - r.left) + step;
    let hg = (r.bottom - r.top) + step;
    let x = r.left - step / 2;
    let y = r.top - step / 2;
    win32::move_resize(h, x, y, w, hg);
}

pub fn shrink() {
    if !is_active() { return; }
    touch();
    let step = config::load().resize_step;
    let h = win32::foreground();
    if h.is_null() { return; }
    let r = match win32::window_rect(h) { Some(r) => r, None => return };
    let w = (r.right - r.left) - step;
    let hg = (r.bottom - r.top) - step;
    if w < 100 || hg < 100 { return; }
    let x = r.left + step / 2;
    let y = r.top + step / 2;
    win32::move_resize(h, x, y, w, hg);
}

// Overlay hint window
static mut OVERLAY: windows_sys::Win32::Foundation::HWND = std::ptr::null_mut();

fn show_overlay(show: bool) {
    unsafe {
        if show {
            if OVERLAY.is_null() {
                let cls: Vec<u16> = "MrspaceOverlay\0".encode_utf16().collect();
                let mut wc: windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSW = std::mem::zeroed();
                wc.lpfnWndProc = Some(overlay_proc);
                wc.lpszClassName = cls.as_ptr();
                windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassW(&wc);
                let title: Vec<u16> = "RESIZE MODE (Esc to exit)\0".encode_utf16().collect();
                let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TOPMOST
                        | windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_TOOLWINDOW,
                    cls.as_ptr(),
                    title.as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::WS_POPUP,
                    100, 40, 260, 32,
                    0 as windows_sys::Win32::Foundation::HWND, 0 as _, 0 as _, std::ptr::null(),
                );
                OVERLAY = hwnd;
            }
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(OVERLAY, windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOW);
        } else if !OVERLAY.is_null() {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(OVERLAY, windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE);
        }
    }
}

unsafe extern "system" fn overlay_proc(hwnd: windows_sys::Win32::Foundation::HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_CLOSE | WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}