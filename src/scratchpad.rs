//! Scratchpad (ROADMAP Tier 2.5): park the focused window offscreen (hidden,
//! no taskbar flash) and summon it back later, restoring exact geometry.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::RECT;

static PARKED: LazyLock<Mutex<Vec<isize>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static SAVED: LazyLock<Mutex<HashMap<isize, RECT>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn park(hwnd: HWND) {
    if hwnd.is_null() { return; }
    let h = hwnd as isize;
    // save geometry
    if let Some(r) = crate::win32::window_rect(hwnd) {
        SAVED.lock().unwrap().insert(h, r);
    }
    // exclude from tiling
    crate::managed::set_floating(h, true);
    // hide offscreen without taskbar flash
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, SWP_HIDEWINDOW, SWP_NOMOVE, SWP_NOSIZE, HWND_BOTTOM};
        // move offscreen first (still visible), then hide
        SetWindowPos(hwnd, HWND_BOTTOM, -32000, -32000, 0, 0, SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOZORDER);
        SetWindowPos(hwnd, hwnd, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_HIDEWINDOW);
    }
    PARKED.lock().unwrap().push(h);
    crate::layout::fit_all();
    crate::borders::sync_all_managed();
}

pub fn summon() -> bool {
    let h = {
        let mut p = PARKED.lock().unwrap();
        p.pop()
    };
    let Some(h) = h else { return false; };
    let hwnd = h as HWND;
    // restore geometry
    if let Some(r) = SAVED.lock().unwrap().get(&h).copied() {
        crate::win32::restore(hwnd);
        crate::win32::move_resize(hwnd, r.left, r.top, r.right - r.left, r.bottom - r.top);
    }
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowPos, SWP_NOZORDER, SWP_NOACTIVATE, SWP_SHOWWINDOW, SWP_NOMOVE, SWP_NOSIZE};
        SetWindowPos(hwnd, hwnd, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW);
    }
    crate::managed::set_floating(h, false);
    SAVED.lock().unwrap().remove(&h);
    crate::borders::note_window(hwnd);
    crate::layout::fit_all();
    crate::borders::sync_all_managed();
    true
}

pub fn parked_count() -> usize {
    PARKED.lock().unwrap().len()
}