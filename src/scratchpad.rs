//! Scratchpad (ROADMAP Tier 2.5): park the focused window offscreen (hidden,
//! no taskbar flash) and summon it back later, restoring exact geometry.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Foundation::RECT;

static PARKED: LazyLock<Mutex<Vec<isize>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static SAVED: LazyLock<Mutex<HashMap<isize, RECT>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// One parked window as persisted to disk. hwnds are process-stable, but a
/// dead process's handle value can be recycled by Windows — so pid + title
/// are stored too and re-verified before touching anything on restore.
#[derive(serde::Serialize, serde::Deserialize)]
struct ParkedEntry {
    hwnd: isize,
    pid: u32,
    title: String,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

fn state_path() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    std::path::PathBuf::from(base).join("mrspace").join("parked.json")
}

/// Write the current parked set to disk (a few bytes, event-driven — called
/// only when a window is parked or summoned). If the daemon dies while
/// windows are parked offscreen, the next start un-strands them.
fn persist() {
    let parked = PARKED.lock().unwrap();
    let saved = SAVED.lock().unwrap();
    let entries: Vec<ParkedEntry> = parked.iter().filter_map(|&h| {
        let r = saved.get(&h).copied()?;
        Some(ParkedEntry {
            hwnd: h,
            pid: crate::win32::process_id(h as HWND),
            title: crate::win32::window_title(h as HWND),
            left: r.left,
            top: r.top,
            right: r.right,
            bottom: r.bottom,
        })
    }).collect();
    drop(saved);
    let path = state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if entries.is_empty() {
        let _ = std::fs::remove_file(&path);
        return;
    }
    if let Ok(json) = serde_json::to_string(&entries) {
        let _ = std::fs::write(&path, json);
    }
}

/// On daemon start: un-strand windows parked by a previous (dead) daemon.
/// Only windows that are still alive, belong to the same process and carry
/// the same title are restored — recycled handle values are skipped.
pub fn restore_on_start() {
    let Ok(json) = std::fs::read_to_string(state_path()) else { return; };
    let Ok(entries) = serde_json::from_str::<Vec<ParkedEntry>>(&json) else { return; };
    let mut restored = 0usize;
    for e in entries {
        let hwnd = e.hwnd as HWND;
        if hwnd.is_null() { continue; }
        let alive = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::IsWindow(hwnd) != 0 };
        if !alive { continue; }
        if crate::win32::process_id(hwnd) != e.pid { continue; }
        if crate::win32::window_title(hwnd) != e.title { continue; }
        crate::win32::restore(hwnd);
        crate::win32::move_resize(hwnd, e.left, e.top, e.right - e.left, e.bottom - e.top);
        crate::borders::note_window(hwnd);
        restored += 1;
    }
    if restored > 0 {
        crate::layout::fit_all();
        crate::borders::sync_all_managed();
    }
    let _ = std::fs::remove_file(state_path());
    trace(&format!("parked-restore: {} windows un-stranded", restored));
}

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
    persist();
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
    persist();
    crate::borders::note_window(hwnd);
    crate::layout::fit_all();
    crate::borders::sync_all_managed();
    true
}

pub fn parked_count() -> usize {
    PARKED.lock().unwrap().len()
}

// TEMP debug trace, gated by MRS_TRACE=path (same pattern as tiling.rs).
fn trace(line: &str) {
    use std::io::Write;
    static TRACE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(p) = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok()) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "[scratchpad] {line}");
        }
    }
}