use crate::win32;
use windows_sys::Win32::Foundation::HWND;
const PRESETS: &[&str] = &[
    "grid", "columns-2", "columns-3", "rows-2",
    "main-left", "main-right", "main-top", "main-bottom", "focus",
];

static mut CYCLE_IDX: usize = 0;

pub fn preset_names() -> &'static [&'static str] { PRESETS }

pub fn apply(name: &str) -> bool {
    let hwnds: Vec<HWND> = win32::enum_windows().into_iter().filter(|&h| win32::is_manageable(h)).collect();
    if hwnds.is_empty() { return false; }
    let wa = win32::primary_work_area();
    let w = wa.right - wa.left;
    let h = wa.bottom - wa.top;
    let ok = match name {
        "grid" => { crate::layout::fit_all(); true }
        "columns-2" => { tile_cols(&hwnds, 2, &wa); true }
        "columns-3" => { tile_cols(&hwnds, 3, &wa); true }
        "rows-2" => { tile_rows(&hwnds, 2, &wa); true }
        "main-left" => { main_split(&hwnds, true, true, &wa); true }
        "main-right" => { main_split(&hwnds, true, false, &wa); true }
        "main-top" => { main_split(&hwnds, false, true, &wa); true }
        "main-bottom" => { main_split(&hwnds, false, false, &wa); true }
        "focus" => {
            let fg = win32::foreground();
            if fg.is_null() { return false; }
            crate::layout::maximize(fg);
            true
        }
        _ => false,
    } == true;
    if ok {
        // presets re-tile the full window set — refresh the border ring set
        crate::borders::sync_all_managed();
        // remember this choice for the desktop it was made on (restored on switch)
        crate::workspace::note_layout(name);
    }
    ok
}

fn tile_cols(hwnds: &[HWND], cols: i32, wa: &windows_sys::Win32::Foundation::RECT) {
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let gi = cfg.gap_inner.max(0);
    let area_left = wa.left + go;
    let area_top = wa.top + go;
    let area_w = (wa.right - wa.left) - 2 * go;
    let area_h = (wa.bottom - wa.top) - 2 * go;
    if area_w <= 0 || area_h <= 0 { return; }
    let n = hwnds.len() as i32;
    let rows = (n + cols - 1) / cols;
    let cw = (area_w - (cols - 1) * gi) / cols;
    let ch = (area_h - (rows - 1).max(0) * gi) / rows.max(1);
    if cw <= 0 || ch <= 0 { return; }
    for (i, &hwnd) in hwnds.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        let x = area_left + col * (cw + gi);
        let y = area_top + row * (ch + gi);
        win32::restore(hwnd);
        win32::move_resize(hwnd, x, y, cw, ch);
    }
}

fn tile_rows(hwnds: &[HWND], rows: i32, wa: &windows_sys::Win32::Foundation::RECT) {
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let gi = cfg.gap_inner.max(0);
    let area_left = wa.left + go;
    let area_top = wa.top + go;
    let area_w = (wa.right - wa.left) - 2 * go;
    let area_h = (wa.bottom - wa.top) - 2 * go;
    if area_w <= 0 || area_h <= 0 { return; }
    let n = hwnds.len() as i32;
    let cols = (n + rows - 1) / rows;
    let cw = (area_w - (cols - 1).max(0) * gi) / cols.max(1);
    let ch = (area_h - (rows - 1) * gi) / rows;
    if cw <= 0 || ch <= 0 { return; }
    for (i, &hwnd) in hwnds.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        let x = area_left + col * (cw + gi);
        let y = area_top + row * (ch + gi);
        win32::restore(hwnd);
        win32::move_resize(hwnd, x, y, cw, ch);
    }
}

fn main_split(hwnds: &[HWND], vertical: bool, main_first: bool, wa: &windows_sys::Win32::Foundation::RECT) {
    let cfg = crate::config::load();
    let go = cfg.gap_outer.max(0);
    let gi = cfg.gap_inner.max(0);
    let area_left = wa.left + go;
    let area_top = wa.top + go;
    let area_w = (wa.right - wa.left) - 2 * go;
    let area_h = (wa.bottom - wa.top) - 2 * go;
    if area_w <= 0 || area_h <= 0 { return; }
    let w = area_w;
    let h = area_h;
    let main = win32::foreground();
    let main = if main.is_null() || !hwnds.contains(&main) { hwnds[0] } else { main };
    let rest: Vec<HWND> = hwnds.iter().copied().filter(|&x| x != main).collect();
    if vertical {
        let mw = (w - gi) * 2 / 3;
        let rw = w - mw - gi;
        if main_first {
            win32::restore(main);
            win32::move_resize(main, area_left, area_top, mw, h);
        } else {
            win32::restore(main);
            win32::move_resize(main, area_left + rw + gi, area_top, mw, h);
        }
        let n = rest.len() as i32;
        let ch = if n > 0 { (h - (n - 1) * gi) / n } else { h };
        if ch <= 0 { return; }
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            let x = if main_first { area_left + mw + gi } else { area_left };
            win32::move_resize(hwnd, x, area_top + i as i32 * (ch + gi), rw, ch);
        }
    } else {
        let mh = (h - gi) * 2 / 3;
        let rh = h - mh - gi;
        if main_first {
            win32::restore(main);
            win32::move_resize(main, area_left, area_top, w, mh);
        } else {
            win32::restore(main);
            win32::move_resize(main, area_left, area_top + rh + gi, w, mh);
        }
        let n = rest.len() as i32;
        let cw = if n > 0 { (w - (n - 1) * gi) / n } else { w };
        if cw <= 0 { return; }
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            let y = if main_first { area_top + mh + gi } else { area_top };
            win32::move_resize(hwnd, area_left + i as i32 * (cw + gi), y, cw, rh);
        }
    }
}

pub fn cycle() -> &'static str {
    unsafe {
        let name = PRESETS[CYCLE_IDX % PRESETS.len()];
        apply(name);
        CYCLE_IDX = (CYCLE_IDX + 1) % PRESETS.len();
        name
    }
}