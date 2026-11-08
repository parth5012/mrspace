use crate::win32;

pub fn pin_left(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let w = (wa.right - wa.left) / 2;
    let h = wa.bottom - wa.top;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left, wa.top, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_right(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let half = (wa.right - wa.left) / 2;
    let w = (wa.right - wa.left) - half;
    let h = wa.bottom - wa.top;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + half, wa.top, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_top(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let w = wa.right - wa.left;
    let h = (wa.bottom - wa.top) / 2;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left, wa.top, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_bottom(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let w = wa.right - wa.left;
    let half = (wa.bottom - wa.top) / 2;
    let h = (wa.bottom - wa.top) - half;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left, wa.top + half, w, h);
    crate::borders::note_window(hwnd);
}

pub fn maximize(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left, wa.top, wa.right - wa.left, wa.bottom - wa.top);
    crate::borders::note_window(hwnd);
}

/// Auto-fit: tile all manageable visible windows on the primary monitor into a grid.
pub fn fit_all() {
    use windows_sys::Win32::Foundation::HWND;
    let hwnds: Vec<HWND> = crate::win32::enum_windows()
        .into_iter()
        .filter(|&h| crate::win32::is_manageable(h))
        .collect();
    if hwnds.is_empty() { return; }
    let wa = win32::primary_work_area();
    let n = hwnds.len();
    let cols = (n as f64).sqrt().ceil() as i32;
    let rows = ((n as i32) + cols - 1) / cols;
    let cell_w = (wa.right - wa.left) / cols;
    let cell_h = (wa.bottom - wa.top) / rows;
    for (i, &h) in hwnds.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        win32::restore(h);
        win32::move_resize(h, wa.left + col * cell_w, wa.top + row * cell_h, cell_w, cell_h);
        crate::borders::note_window(h);
    }
}