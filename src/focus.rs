use crate::win32;
use windows_sys::Win32::Foundation::HWND;

pub fn focus_direction(dir: &str) {
    let cur = win32::foreground();
    if cur.is_null() { return; }
    let cur_r = match win32::window_rect(cur) { Some(r) => r, None => return };
    let cur_cx = (cur_r.left + cur_r.right) / 2;
    let cur_cy = (cur_r.top + cur_r.bottom) / 2;

    let mut best: Option<(HWND, i64)> = None;
    for h in win32::enum_windows() {
        if h == cur || !win32::is_manageable(h) { continue; }
        let r = match win32::window_rect(h) { Some(r) => r, None => continue };
        let cx = (r.left + r.right) / 2;
        let cy = (r.top + r.bottom) / 2;
        let ok = match dir {
            "left" => cx < cur_cx - 10,
            "right" => cx > cur_cx + 10,
            "up" => cy < cur_cy - 10,
            "down" => cy > cur_cy + 10,
            _ => false,
        };
        if !ok { continue; }
        let dx = (cx - cur_cx) as i64;
        let dy = (cy - cur_cy) as i64;
        let dist = dx*dx + dy*dy;
        if best.map(|(_, d)| dist < d).unwrap_or(true) {
            best = Some((h, dist));
        }
    }
    if let Some((h, _)) = best {
        win32::restore(h);
        win32::set_foreground(h);
    }
}