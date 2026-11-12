//! Managed-window state (ROADMAP Tier 1.3): per-window tiled/floating flag.
//! Floating windows are excluded from tiling (and from `is_manageable`) so they
//! are left where the user put them; toggling back re-joins the tiling set.

use std::collections::HashSet;
use std::sync::{LazyLock, Mutex};

static FLOATING: LazyLock<Mutex<HashSet<isize>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

pub fn is_floating(hwnd: isize) -> bool {
    FLOATING.lock().unwrap().contains(&hwnd)
}

pub fn toggle_float(hwnd: isize) -> bool {
    let mut set = FLOATING.lock().unwrap();
    let was = set.contains(&hwnd);
    if was {
        set.remove(&hwnd);
    } else {
        set.insert(hwnd);
    }
    drop(set);
    // re-tile the (now changed) manageable set
    crate::layout::fit_all();
    crate::borders::sync_all_managed();
    !was
}

pub fn set_floating(hwnd: isize, floating: bool) {
    let mut set = FLOATING.lock().unwrap();
    if floating { set.insert(hwnd); } else { set.remove(&hwnd); }
}