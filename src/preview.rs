//! Layout-change preview overlay (ROADMAP Tier 3.2 — FancyZones' best idea).
//! When the layout-cycle hotkey fires, the *destination* preset is drawn as
//! translucent ghost regions (reusing borders.rs layered-window machinery)
//! for ~800 ms before it is committed. Pressing the hotkey again inside the
//! window advances to the next preset and restarts the timer.
//!
//! Guardrails: the preview thread sleeps on a condvar while idle (no polling
//! loop), ghosts are click-through layered windows (never trap focus), and
//! when tiling is off the cycle applies immediately like before.

use std::sync::{Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

/// Ghost lifetime after the last cycle keypress.
const PREVIEW_MS: u128 = 800;
/// Interior fill alpha of a ghost region (0–255; ring is always solid).
const GHOST_FILL_ALPHA: u8 = 64;

struct Preview {
    idx: usize,
    deadline: Instant,
    slots: Vec<crate::layouts::Slot>,
    rendered: bool,
}

static STATE: LazyLock<Mutex<Option<Preview>>> = LazyLock::new(|| Mutex::new(None));
static CV: Condvar = Condvar::new();

/// Called from the hotkey path (and layout-cycle IPC / CLI): show the next
/// preset as ghosts, commit it PREVIEW_MS after the last press. When tiling
/// is off, falls back to the legacy immediate cycle.
pub fn cycle() -> &'static str {
    if !crate::tiling::is_active() {
        return crate::layouts::cycle();
    }
    let names = crate::layouts::preset_names();
    let idx = {
        let mut st = STATE.lock().unwrap();
        let next = match &*st {
            Some(p) => (p.idx + 1) % names.len(),
            None => (crate::layouts::cycle_index() + 1) % names.len(),
        };
        match preview_slots(names[next]) {
            // no computable preview (e.g. no manageable windows) — apply now
            None => {
                drop(st);
                return crate::layouts::cycle();
            }
            Some(slots) => {
                *st = Some(Preview { idx: next, deadline: Instant::now() + Duration::from_millis(PREVIEW_MS as u64), slots, rendered: false });
                CV.notify_all();
                next
            }
        }
    };
    names[idx]
}

/// Compute the would-be slot rects for a preset without moving anything.
/// This runs on the hotkey thread: it sends WM_GETMINMAXINFO to each window
/// (150 ms bounded each) — acceptable because the preview is on-demand, but
/// hung windows degrade to (0,0) fallback rather than stalling.
fn preview_slots(name: &str) -> Option<Vec<crate::layouts::Slot>> {
    crate::layouts::apply_or_preview(name, false)
}

/// Start the single preview/commit thread (once).
pub fn start() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        std::thread::spawn(|| {
            let mut guard = STATE.lock().unwrap();
            loop {
                if guard.is_none() {
                    guard = CV.wait(guard).unwrap();
                    continue;
                }
                // render pending ghosts before waiting out the timer
                if let Some(p) = guard.as_mut() {
                    if !p.rendered {
                        render_ghosts(&p.slots);
                        p.rendered = true;
                    }
                    let remaining = p.deadline.saturating_duration_since(Instant::now());
                    if !remaining.is_zero() {
                        let (g, _) = CV.wait_timeout(guard, remaining).unwrap();
                        guard = g;
                        continue;
                    }
                }
                // deadline reached (or entry appeared already expired): commit
                let done = guard.take().unwrap();
                if done.deadline.saturating_duration_since(Instant::now()) > Duration::from_millis(5) {
                    // an advance landed in the commit window — honor the new
                    // preview instead of committing it prematurely
                    *guard = Some(done);
                    let (g, _) = CV.wait_timeout(guard, Duration::from_millis(1)).unwrap();
                    guard = g;
                    continue;
                }
                drop(guard);
                destroy_ghosts();
                crate::layouts::apply(crate::layouts::preset_names()[done.idx]);
                crate::layouts::set_cycle_index(done.idx + 1);
                trace("preview commit");
                guard = STATE.lock().unwrap();
            }
        });
    });
}

static GHOSTS: Mutex<Vec<isize>> = Mutex::new(Vec::new());

fn render_ghosts(slots: &[crate::layouts::Slot]) {
    destroy_ghosts();
    let color = crate::borders::accent_color();
    let mut made = Vec::new();
    for &(hwnd, x, y, w, h) in slots {
        if w <= 0 || h <= 0 { continue; }
        unsafe {
            let g = crate::borders::create_ghost(w, h, x, y, color, GHOST_FILL_ALPHA);
            if g != 0 { made.push(g); }
        }
        let _ = hwnd;
    }
    *GHOSTS.lock().unwrap() = made;
    trace("preview render");
}

fn destroy_ghosts() {
    let ghosts = std::mem::take(&mut *GHOSTS.lock().unwrap());
    for g in ghosts {
        unsafe { crate::desktop::destroy_window(g as windows_sys::Win32::Foundation::HWND); }
    }
}

// TEMP debug trace, gated by MRS_TRACE=path (same pattern as tiling.rs/borders.rs).
fn trace(line: &str) {
    use std::io::Write;
    static TRACE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
    if let Some(p) = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok()) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let _ = writeln!(f, "[preview] {line}");
        }
    }
}
