use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::core::BOOL;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId,
    IsWindowVisible, MoveWindow, SetForegroundWindow, SetWindowPos, ShowWindow, SW_RESTORE,
    SW_SHOW, SWP_NOZORDER, SWP_SHOWWINDOW,
};
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;

pub fn enum_windows() -> Vec<HWND> {
    let mut out: Vec<HWND> = Vec::new();
    unsafe {
        EnumWindows(Some(enum_cb), &mut out as *mut _ as LPARAM);
    }
    out
}

unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let list = &mut *(lparam as *mut Vec<HWND>);
    if IsWindowVisible(hwnd) != 0 {
        list.push(hwnd);
    }
    1
}

pub fn window_title(hwnd: HWND) -> String {
    let mut buf = [0u16; 512];
    let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
    if n <= 0 { return String::new(); }
    OsString::from_wide(&buf[..n as usize]).to_string_lossy().to_string()
}

pub fn window_rect(hwnd: HWND) -> Option<RECT> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if unsafe { GetWindowRect(hwnd, &mut r) } != 0 { Some(r) } else { None }
}

pub fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid); }
    pid
}

pub fn foreground() -> HWND {
    unsafe { GetForegroundWindow() }
}

pub fn set_foreground(hwnd: HWND) {
    unsafe { SetForegroundWindow(hwnd); }
}

pub fn restore(hwnd: HWND) {
    unsafe { ShowWindow(hwnd, SW_RESTORE); }
}

pub fn show(hwnd: HWND) {
    unsafe { ShowWindow(hwnd, SW_SHOW); }
}

pub fn move_resize(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) {
    unsafe {
        MoveWindow(hwnd, x, y, w, h, 1);
        let _ = SetWindowPos(hwnd, std::ptr::null_mut(), x, y, w, h, SWP_NOZORDER | SWP_SHOWWINDOW);
    }
}

pub fn work_area_for(hwnd: HWND) -> RECT {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITOR_DEFAULTTONEAREST, MONITORINFO};
    let hmon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    let mut mi: MONITORINFO = unsafe { std::mem::zeroed() };
    mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
    if unsafe { GetMonitorInfoW(hmon, &mut mi) } != 0 {
        mi.rcWork
    } else {
        RECT { left: 0, top: 0, right: 1920, bottom: 1080 }
    }
}

pub fn primary_work_area() -> RECT {
    use windows_sys::Win32::UI::WindowsAndMessaging::SystemParametersInfoW;
    use windows_sys::Win32::UI::WindowsAndMessaging::SPI_GETWORKAREA;
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    unsafe { SystemParametersInfoW(SPI_GETWORKAREA, 0, &mut r as *mut _ as *mut _, 0); }
    r
}

pub fn is_manageable(hwnd: HWND) -> bool {
    if hwnd.is_null() { return false; }
    let title = window_title(hwnd);
    // skip empty-title windows and our own? keep simple
    if title.is_empty() { return false; }
    let r = match window_rect(hwnd) { Some(r) => r, None => return false };
    let w = r.right - r.left; let h = r.bottom - r.top;
    if w < 50 || h < 50 { return false; }
    true
}