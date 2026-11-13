use windows_sys::Win32::Foundation::{HWND, LPARAM, RECT};
use windows_sys::core::BOOL;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect,
    GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, SetWindowPos, ShowWindow,
    SendMessageTimeoutW, SMTO_ABORTIFHUNG, SMTO_BLOCK, WM_GETTEXT, SW_RESTORE, SW_SHOW,
    SWP_NOZORDER, SWP_SHOWWINDOW, SWP_ASYNCWINDOWPOS, GWL_EXSTYLE,
    GWL_STYLE, WS_CAPTION, WS_EX_TOOLWINDOW,
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
    // GetWindowTextW SENDS WM_GETTEXT for cross-process windows, which blocks
    // forever on a suspended/UWP window thread (notepad suspends in seconds)
    // and hung the daemon. SendMessageTimeoutW with ABORTIFHUNG never wedges.
    unsafe {
        let mut res: usize = 0;
        let sent = SendMessageTimeoutW(
            hwnd,
            WM_GETTEXT,
            buf.len() as usize,
            buf.as_mut_ptr() as LPARAM,
            SMTO_ABORTIFHUNG | SMTO_BLOCK,
            300,
            &mut res,
        );
        if sent == 0 || res == 0 {
            return String::new();
        }
        let n = (res as usize).min(buf.len());
        if n == 0 {
            return String::new();
        }
        OsString::from_wide(&buf[..n]).to_string_lossy().to_string()
    }
}

pub fn window_rect(hwnd: HWND) -> Option<RECT> {
    let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if unsafe { GetWindowRect(hwnd, &mut r) } != 0 { Some(r) } else { None }
}

/// True when the window is cloaked by DWM — i.e. it lives on another virtual
/// desktop or is a suspended UWP frame. Cheap, in-proc, never cross-process.
pub fn is_cloaked(hwnd: HWND) -> bool {
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    let mut val: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED as u32,
            &mut val as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
    }
    val != 0
}

pub fn process_id(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid); }
    pid
}

pub fn process_name(hwnd: HWND) -> String {
    let pid = process_id(hwnd);
    if pid == 0 { return String::new(); }
    unsafe {
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW};
        use windows_sys::Win32::Foundation::CloseHandle;
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() { return String::new(); }
        let mut buf = [0u16; 512];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 { return String::new(); }
        let full = String::from_utf16_lossy(&buf[..size as usize]);
        full.rsplit('\\').next().unwrap_or(&full).to_string()
    }
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
        // ASYNCWINDOWPOS: post the change instead of SendMessage-waiting on the
        // target window's thread — a hung/destroying window can otherwise block
        // our caller forever during auto-tile bursts (kills the IPC daemon).
        let _ = SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            x,
            y,
            w,
            h,
            SWP_NOZORDER | SWP_SHOWWINDOW | SWP_ASYNCWINDOWPOS,
        );
    }
}

pub fn class_name(hwnd: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), 256) };
    if n <= 0 { return String::new(); }
    OsString::from_wide(&buf[..n as usize]).to_string_lossy().to_string()
}

/// Minimum tracking size a window will clamp SetWindowPos to (e.g. modern
/// Paint enforces 704x500 logical — 1056x750 physical at 150% scaling, which
/// is bigger than a half-screen tile and used to break the grid). Queried via
/// WM_GETMINMAXINFO with a bounded timeout: hung windows yield (0, 0) and get
/// the nominal tile (previous behavior) instead of stalling the re-tiler.
pub fn min_track_size(hwnd: HWND) -> (i32, i32) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SendMessageTimeoutW, MINMAXINFO, SMTO_ABORTIFHUNG, SMTO_NORMAL,
        SM_CXMAXTRACK, SM_CXMINTRACK, SM_CYMAXTRACK, SM_CYMINTRACK, WM_GETMINMAXINFO,
    };
    unsafe {
        let mut mmi: MINMAXINFO = std::mem::zeroed();
        mmi.ptMaxTrackSize.x = GetSystemMetrics(SM_CXMAXTRACK);
        mmi.ptMaxTrackSize.y = GetSystemMetrics(SM_CYMAXTRACK);
        mmi.ptMinTrackSize.x = GetSystemMetrics(SM_CXMINTRACK);
        mmi.ptMinTrackSize.y = GetSystemMetrics(SM_CYMINTRACK);
        let mut res: usize = 0;
        let ok = SendMessageTimeoutW(
            hwnd,
            WM_GETMINMAXINFO,
            0,
            &mut mmi as *mut _ as isize,
            SMTO_NORMAL | SMTO_ABORTIFHUNG,
            150,
            &mut res,
        );
        if ok == 0 { return (0, 0); }
        (mmi.ptMinTrackSize.x, mmi.ptMinTrackSize.y)
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
    // Floating windows are user-placed: excluded from tiling/counting/borders.
    if crate::managed::is_floating(hwnd as isize) { return false; }
    // Window rules: ignore / no-manage → not manageable.
    if crate::rules::is_ignored(hwnd) { return false; }
    // Cloaked windows live on other virtual desktops (or are suspended UWP):
    // they must never be tiled or counted — DWM does the check, no cross-process
    // message sent, so it cannot hang the way WM_GETTEXT can.
    if is_cloaked(hwnd) { return false; }
    let title = window_title(hwnd);
    // skip empty-title windows and our own? keep simple
    if title.is_empty() { return false; }
    let r = match window_rect(hwnd) { Some(r) => r, None => return false };
    let w = r.right - r.left; let h = r.bottom - r.top;
    if w < 50 || h < 50 { return false; }
    // Require a normal app window: has a title bar, is not a tool/overlay window.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let exstyle = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if style & WS_CAPTION != WS_CAPTION { return false; }
        if exstyle & WS_EX_TOOLWINDOW != 0 { return false; }
        if exstyle & windows_sys::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE != 0 { return false; }
    }
    // User exclusion list (substring title match, case-insensitive; plus pid match).
    let cfg = crate::config::load();
    let t = title.to_lowercase();
    for pat in &cfg.exclude_titles {
        if !pat.is_empty() && t.contains(&pat.to_lowercase()) { return false; }
    }
    // Built-in: never manage our own UI windows (settings app spawns as a normal
    // top-level window and would otherwise be tiled away mid-use).
    let cls = class_name(hwnd);
    if cls.eq_ignore_ascii_case("MrspaceSettings") { return false; }
    // Class-based exclusion (widget frameworks like Rainmeter).
    for pat in &cfg.exclude_classes {
        if !pat.is_empty() && cls.eq_ignore_ascii_case(pat) { return false; }
    }
    let pid = process_id(hwnd);
    if cfg.exclude_pids.contains(&pid) { return false; }
    true
}

pub fn debug_windows() -> String {
    let mut out = String::new();
    for h in enum_windows() {
        let title = window_title(h);
        unsafe {
            let style = GetWindowLongPtrW(h, GWL_STYLE) as u32;
            let ex = GetWindowLongPtrW(h, GWL_EXSTYLE) as u32;
            let r = match window_rect(h) { Some(r) => r, None => continue };
            out.push_str(&format!(
                "hwnd=0x{:x}|pid={}|{}x{}|style=0x{:08x}|ex=0x{:08x}|class={}|manageable={}|{}\n",
                h as usize, process_id(h), r.right - r.left, r.bottom - r.top, style, ex,
                class_name(h), is_manageable(h), title
            ));
        }
    }
    out
}