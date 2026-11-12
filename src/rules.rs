//! Window rules engine (ROADMAP Tier 2.4): match title substring | class | exe
//! name per window -> float / ignore / no-manage. Evaluated in the re-tiler
//! (not the hook callback, which must stay cheap) and consulted by
//! `win32::is_manageable`.

use crate::config::WindowRule;

pub fn matches(rule: &WindowRule, hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    if let Some(p) = &rule.match_title {
        if !p.is_empty() {
            let t = crate::win32::window_title(hwnd).to_lowercase();
            if !t.contains(&p.to_lowercase()) { return false; }
        }
    }
    if let Some(p) = &rule.match_class {
        if !p.is_empty() {
            let c = crate::win32::class_name(hwnd);
            if !c.to_lowercase().contains(&p.to_lowercase()) { return false; }
        }
    }
    if let Some(p) = &rule.match_exe {
        if !p.is_empty() {
            let e = crate::win32::process_name(hwnd).to_lowercase();
            if !e.contains(&p.to_lowercase()) { return false; }
        }
    }
    // at least one criterion must be set
    rule.match_title.is_some() || rule.match_class.is_some() || rule.match_exe.is_some()
}

pub fn action_for(hwnd: windows_sys::Win32::Foundation::HWND) -> Option<String> {
    let cfg = crate::config::load();
    for r in &cfg.window_rules {
        if matches(r, hwnd) {
            return Some(r.action.clone());
        }
    }
    None
}

pub fn is_ignored(hwnd: windows_sys::Win32::Foundation::HWND) -> bool {
    match action_for(hwnd) {
        Some(a) => a == "ignore" || a == "no-manage",
        None => false,
    }
}

/// Apply float rules to a window (mark floating without re-tiling). Called from
/// the re-tiler for every window before counting.
pub fn apply_float_rules(hwnd: windows_sys::Win32::Foundation::HWND) {
    if let Some(a) = action_for(hwnd) {
        if a == "float" && !crate::managed::is_floating(hwnd as isize) {
            crate::managed::set_floating(hwnd as isize, true);
        }
    }
}