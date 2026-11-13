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
    let mins: Vec<(i32, i32)> = hwnds.iter().map(|&h| win32::min_track_size(h)).collect();
    // Shrink the requested column count only as far as needed so every
    // window's minimum fits; below that, accept edge overflow (never overlap).
    let mut cols = cols.max(1).min(hwnds.len() as i32);
    while cols > 1 && !crate::layout::fits_with(cols, &mins, area_w, area_h, gi) {
        cols -= 1;
    }
    crate::layout::tile_grid(hwnds, cols, &mins, area_left, area_top, area_w, area_h, gi);
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
    let mins: Vec<(i32, i32)> = hwnds.iter().map(|&h| win32::min_track_size(h)).collect();
    let n = hwnds.len() as i32;
    let rows = rows.max(1).min(n);
    let cols = (n + rows - 1) / rows;
    // Rows beyond the first may exceed the work area if the minimum heights
    // don't fit; distribute() then places each row at its minimum, stacked
    // sequentially — overflow past the edge instead of overlap.
    crate::layout::tile_grid(hwnds, cols, &mins, area_left, area_top, area_w, area_h, gi);
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
        // Stack width must at least fit the widest minimum in the stack; the
        // main pane takes the rest (and keeps its own minimum even if that
        // overflows the area — overflow past the edge instead of overlap).
        let main_min_w = win32::min_track_size(main).0.max(0);
        let rest_min_w = rest.iter().map(|&x| win32::min_track_size(x).0.max(0)).max().unwrap_or(0);
        let mut rw = w - (w - gi) * 2 / 3 - gi;
        if rw < rest_min_w {
            rw = rest_min_w;
        }
        let mw = (w - rw - gi).max(main_min_w);
        let stack_x = if main_first { area_left + mw + gi } else { area_left };
        let main_x = if main_first { area_left } else { area_left + rw + gi };
        let n = rest.len() as i32;
        let rest_min_h: Vec<i32> = rest.iter().map(|&x| win32::min_track_size(x).1.max(0)).collect();
        let heights = crate::layout::distribute(h - (n - 1).max(0) * gi, &rest_min_h);
        win32::restore(main);
        win32::move_resize(main, main_x, area_top, mw, h);
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            win32::move_resize(hwnd, stack_x, area_top + i as i32 * (heights[i] + gi), rw, heights[i]);
        }
    } else {
        let main_min_h = win32::min_track_size(main).1.max(0);
        let rest_min_h = rest.iter().map(|&x| win32::min_track_size(x).1.max(0)).max().unwrap_or(0);
        let mut rh = h - (h - gi) * 2 / 3 - gi;
        if rh < rest_min_h {
            rh = rest_min_h;
        }
        let mh = (h - rh - gi).max(main_min_h);
        let stack_y = if main_first { area_top + mh + gi } else { area_top };
        let main_y = if main_first { area_top } else { area_top + rh + gi };
        let n = rest.len() as i32;
        let rest_min_w: Vec<i32> = rest.iter().map(|&x| win32::min_track_size(x).0.max(0)).collect();
        let widths = crate::layout::distribute(w - (n - 1).max(0) * gi, &rest_min_w);
        win32::restore(main);
        win32::move_resize(main, area_left, main_y, w, mh);
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            win32::move_resize(hwnd, area_left + i as i32 * (widths[i] + gi), stack_y, widths[i], rh);
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