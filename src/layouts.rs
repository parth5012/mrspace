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
    }
    ok
}

fn tile_cols(hwnds: &[HWND], cols: i32, wa: &windows_sys::Win32::Foundation::RECT) {
    let w = wa.right - wa.left;
    let h = wa.bottom - wa.top;
    let n = hwnds.len() as i32;
    let rows = (n + cols - 1) / cols;
    let cw = w / cols;
    let ch = h / rows.max(1);
    for (i, &hwnd) in hwnds.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        win32::restore(hwnd);
        win32::move_resize(hwnd, wa.left + col * cw, wa.top + row * ch, cw, ch);
    }
}

fn tile_rows(hwnds: &[HWND], rows: i32, wa: &windows_sys::Win32::Foundation::RECT) {
    let w = wa.right - wa.left;
    let h = wa.bottom - wa.top;
    let n = hwnds.len() as i32;
    let cols = (n + rows - 1) / rows;
    let cw = w / cols.max(1);
    let ch = h / rows;
    for (i, &hwnd) in hwnds.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        win32::restore(hwnd);
        win32::move_resize(hwnd, wa.left + col * cw, wa.top + row * ch, cw, ch);
    }
}

fn main_split(hwnds: &[HWND], vertical: bool, main_first: bool, wa: &windows_sys::Win32::Foundation::RECT) {
    // main window takes 2/3, rest stacked in 1/3
    let w = wa.right - wa.left;
    let h = wa.bottom - wa.top;
    let main = win32::foreground();
    let main = if main.is_null() || !hwnds.contains(&main) { hwnds[0] } else { main };
    let rest: Vec<HWND> = hwnds.iter().copied().filter(|&x| x != main).collect();
    if vertical {
        let mw = w * 2 / 3;
        let rw = w - mw;
        if main_first {
            win32::restore(main);
            win32::move_resize(main, wa.left, wa.top, mw, h);
        } else {
            win32::restore(main);
            win32::move_resize(main, wa.left + rw, wa.top, mw, h);
        }
        let n = rest.len() as i32;
        let ch = if n > 0 { h / n } else { h };
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            let x = if main_first { wa.left + mw } else { wa.left };
            win32::move_resize(hwnd, x, wa.top + i as i32 * ch, rw, ch);
        }
    } else {
        let mh = h * 2 / 3;
        let rh = h - mh;
        if main_first {
            win32::restore(main);
            win32::move_resize(main, wa.left, wa.top, w, mh);
        } else {
            win32::restore(main);
            win32::move_resize(main, wa.left, wa.top + rh, w, mh);
        }
        let n = rest.len() as i32;
        let cw = if n > 0 { w / n } else { w };
        for (i, &hwnd) in rest.iter().enumerate() {
            win32::restore(hwnd);
            let y = if main_first { wa.top + mh } else { wa.top };
            win32::move_resize(hwnd, wa.left + i as i32 * cw, y, cw, rh);
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