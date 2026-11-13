use crate::win32;

pub fn pin_left(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let w = (wa.right - wa.left - 2 * go) / 2;
    let h = (wa.bottom - wa.top) - 2 * go;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + go, wa.top + go, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_right(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let half = (wa.right - wa.left - 2 * go) / 2;
    let w = (wa.right - wa.left - 2 * go) - half;
    let h = (wa.bottom - wa.top) - 2 * go;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + go + half, wa.top + go, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_top(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let w = (wa.right - wa.left) - 2 * go;
    let h = (wa.bottom - wa.top - 2 * go) / 2;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + go, wa.top + go, w, h);
    crate::borders::note_window(hwnd);
}

pub fn pin_bottom(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let w = (wa.right - wa.left) - 2 * go;
    let half = (wa.bottom - wa.top - 2 * go) / 2;
    let h = (wa.bottom - wa.top - 2 * go) - half;
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + go, wa.top + go + half, w, h);
    crate::borders::note_window(hwnd);
}

pub fn maximize(hwnd: windows_sys::Win32::Foundation::HWND) {
    let wa = win32::work_area_for(hwnd);
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    win32::restore(hwnd);
    win32::move_resize(hwnd, wa.left + go, wa.top + go, (wa.right - wa.left) - 2 * go, (wa.bottom - wa.top) - 2 * go);
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
    // re-tile the whole set in the new order using the shared min-aware grid
    crate::layout::retile(&new_order);
    let _ = rects;
}
/// Split `avail` (gaps already subtracted by the caller) among items, giving
/// every item at least its minimum and sharing the remainder equally. If the
/// minimums alone exceed `avail`, the minimums are returned as-is: the
/// sequential layout then overflows the area edge instead of overlapping —
/// a window that further clamps to its own minimum can only shrink inside
/// its slot, so "no overlap" always holds.
pub(crate) fn distribute(avail: i32, mins: &[i32]) -> Vec<i32> {
    let n = mins.len();
    if n == 0 { return Vec::new(); }
    let mins: Vec<i32> = mins.iter().map(|&m| m.max(0)).collect();
    let sum: i32 = mins.iter().sum();
    if sum >= avail { return mins; }
    let extra = avail - sum;
    let per = extra / n as i32;
    let rem = extra % n as i32;
    mins.iter().enumerate()
        .map(|(i, &m)| m + per + if (i as i32) < rem { 1 } else { 0 })
        .collect()
}

/// Can a row-major grid of `cols` columns hold every window's minimum size?
/// Horizontal: each row's min widths + gaps must fit. Vertical: the sum of
/// each row's tallest min height + gaps must fit.
pub(crate) fn fits_with(cols: i32, mins: &[(i32, i32)], area_w: i32, area_h: i32, gi: i32) -> bool {
    let n = mins.len() as i32;
    if n == 0 { return true; }
    let cols = cols.max(1);
    let rows = (n + cols - 1) / cols;
    let mut y_need = (rows - 1) * gi;
    for r in 0..rows {
        let start = (r * cols) as usize;
        let end = (((r + 1) * cols) as usize).min(mins.len());
        let row = &mins[start..end];
        let wneed: i32 = row.iter().map(|m| m.0.max(0)).sum::<i32>() + (row.len() as i32 - 1) * gi;
        if wneed > area_w { return false; }
        y_need += row.iter().map(|m| m.1.max(0)).max().unwrap_or(0);
    }
    y_need <= area_h
}

/// Largest feasible column count (≤ ceil(sqrt(n)), ≥ 1) whose grid can hold
/// every minimum. When even one column overflows (e.g. two windows that each
/// need more vertical space than the work area), 1 is returned and the
/// layout accepts edge overflow rather than overlap.
pub(crate) fn choose_cols(mins: &[(i32, i32)], area_w: i32, area_h: i32, gi: i32) -> i32 {
    let n = mins.len() as i32;
    if n == 0 { return 1; }
    let mut cols = ((n as f64).sqrt().ceil() as i32).clamp(1, n);
    while cols > 1 && !fits_with(cols, mins, area_w, area_h, gi) {
        cols -= 1;
    }
    cols
}

/// Tile `hwnds` row-major with `cols` columns, honoring each window's
/// minimum tracking size: row heights come from the tallest min height in
/// the row, widths from a min-aware equal split, gaps between everything.
pub(crate) fn tile_grid(
    hwnds: &[windows_sys::Win32::Foundation::HWND],
    cols: i32,
    mins: &[(i32, i32)],
    area_left: i32,
    area_top: i32,
    area_w: i32,
    area_h: i32,
    gi: i32,
) {
    if hwnds.is_empty() || area_w <= 0 || area_h <= 0 { return; }
    let cols = cols.max(1);
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in 0..hwnds.len() {
        let r = i as i32 / cols;
        if r as usize >= rows.len() { rows.push(Vec::new()); }
        rows[r as usize].push(i);
    }
    let row_mins: Vec<i32> = rows.iter()
        .map(|row| row.iter().map(|&i| mins[i].1.max(0)).max().unwrap_or(0))
        .collect();
    let heights = distribute(area_h - (rows.len() as i32 - 1) * gi, &row_mins);
    let mut y = area_top;
    for (r, row) in rows.iter().enumerate() {
        let width_mins: Vec<i32> = row.iter().map(|&i| mins[i].0.max(0)).collect();
        let widths = distribute(area_w - (row.len() as i32 - 1) * gi, &width_mins);
        let mut x = area_left;
        for (c, &i) in row.iter().enumerate() {
            win32::restore(hwnds[i]);
            win32::move_resize(hwnds[i], x, y, widths[c], heights[r]);
            crate::borders::note_window(hwnds[i]);
            x += widths[c] + gi;
        }
        y += heights[r] + gi;
    }
}

/// Re-tile an explicit window order on the primary work area grid,
/// automatically choosing the largest feasible column count.
pub(crate) fn retile(hwnds: &[windows_sys::Win32::Foundation::HWND]) {
    if hwnds.is_empty() { return; }
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let gi = cfg.gap_inner.max(0);
    let wa = win32::primary_work_area();
    let area_w = (wa.right - wa.left) - 2 * go;
    let area_h = (wa.bottom - wa.top) - 2 * go;
    if area_w <= 0 || area_h <= 0 { return; }
    let mins: Vec<(i32, i32)> = hwnds.iter().map(|&h| win32::min_track_size(h)).collect();
    let cols = choose_cols(&mins, area_w, area_h, gi);
    tile_grid(hwnds, cols, &mins, wa.left + go, wa.top + go, area_w, area_h, gi);
}

pub fn fit_all() {
    use windows_sys::Win32::Foundation::HWND;
    let hwnds: Vec<HWND> = crate::win32::enum_windows()
        .into_iter()
        .filter(|&h| crate::win32::is_manageable(h))
        .collect();
    retile(&hwnds);
}