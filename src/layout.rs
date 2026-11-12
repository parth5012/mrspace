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

/// Swap the focused window with the next manageable window in z-order.
pub fn swap_next(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Foundation::HWND;
    let hwnds: Vec<HWND> = crate::win32::enum_windows()
        .into_iter()
        .filter(|&h| crate::win32::is_manageable(h))
        .collect();
    let pos = hwnds.iter().position(|&h| h == hwnd);
    let Some(i) = pos else { return; };
    if i + 1 >= hwnds.len() { return; }
    let a = hwnds[i];
    let b = hwnds[i + 1];
    let ra = crate::win32::window_rect(a);
    let rb = crate::win32::window_rect(b);
    if let (Some(ra), Some(rb)) = (ra, rb) {
        crate::win32::restore(a);
        crate::win32::restore(b);
        crate::win32::move_resize(a, rb.left, rb.top, rb.right - rb.left, rb.bottom - rb.top);
        crate::win32::move_resize(b, ra.left, ra.top, ra.right - ra.left, ra.bottom - ra.top);
        crate::borders::note_window(a);
        crate::borders::note_window(b);
    }
}

/// Promote the focused window to the first (main) slot and shift the rest.
pub fn promote(hwnd: windows_sys::Win32::Foundation::HWND) {
    use windows_sys::Win32::Foundation::HWND;
    let hwnds: Vec<HWND> = crate::win32::enum_windows()
        .into_iter()
        .filter(|&h| crate::win32::is_manageable(h))
        .collect();
    if hwnds.is_empty() || hwnds[0] == hwnd { return; }
    let Some(idx) = hwnds.iter().position(|&h| h == hwnd) else { return; };
    // capture rects before moving
    let rects: Vec<_> = hwnds.iter().map(|&h| crate::win32::window_rect(h)).collect();
    // new order: hwnd first, then the rest in original order
    let mut new_order = vec![hwnd];
    for (i, &h) in hwnds.iter().enumerate() {
        if i != idx { new_order.push(h); }
    }
    // apply the first window's old rect to hwnd's new slot? Instead, just
    // re-tile the whole set in the new order using fit_all-style grid.
    let wa = crate::win32::primary_work_area();
    let n = new_order.len() as i32;
    let cols = (n as f64).sqrt().ceil() as i32;
    let rows = (n + cols - 1) / cols;
    let cell_w = (wa.right - wa.left) / cols;
    let cell_h = (wa.bottom - wa.top) / rows;
    for (i, &h) in new_order.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        crate::win32::restore(h);
        crate::win32::move_resize(h, wa.left + col * cell_w, wa.top + row * cell_h, cell_w, cell_h);
        crate::borders::note_window(h);
    }
    let _ = rects;
}
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