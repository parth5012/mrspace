//! Workspace-scoped layout memory (ROADMAP Tier 1.2): remember the last-chosen
//! layout per virtual desktop and restore it on desktop switch / count-change
//! re-tiles. Storage is a flat Vec keyed by desktop GUID, FIFO-capped
//! (RAM guardrail: one small record per desktop, no serde trees, no polling —
//! everything runs inside the re-tiler's event wake).

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use windows_sys::core::GUID;

/// max desktops remembered before oldest eviction (typical usage: 2–4)
const CAP: usize = 8;

#[derive(Clone)]
struct DeskState {
    layout: Option<String>,
}

static DESKS: LazyLock<Mutex<Vec<(GUID, DeskState)>>> = LazyLock::new(|| Mutex::new(Vec::new()));
// last-switch-detected desktop (hash-folded GUID; hash covers identity)
static LAST_DESK_KNOWN: AtomicBool = AtomicBool::new(false);
static LAST_DESK_HASH: AtomicU64 = AtomicU64::new(0);

fn fold(g: &GUID) -> u64 {
    let a = u64::from(g.data1) ^ (u64::from(g.data2) << 32) ^ u64::from(g.data3);
    let b = u64::from_le_bytes([g.data4[0], g.data4[1], g.data4[2], g.data4[3], g.data4[4], g.data4[5], g.data4[6], g.data4[7]]);
    a ^ b.rotate_left(17)
}

fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1
        && a.data2 == b.data2
        && a.data3 == b.data3
        && a.data4 == b.data4
}

/// Current virtual desktop GUID, probed via the foreground window (no temp
/// window, no shell roundtrip). None when the foreground can't be resolved
/// (e.g. an empty desktop) — callers treat that as "no info this tick".
/// Current virtual desktop GUID. Fast path: foreground window's desktop. If that
/// returns the same GUID as the last-known desktop (ambiguous on empty
/// desktops, where the foreground is the taskbar), confirm via the temp-window
/// probe (a window always lands on the active desktop). The probe is cached
/// for 200 ms so rapid event bursts don't spawn windows each tick.
pub fn current_guid() -> Option<GUID> {
    probe_desktop()
}

// Desktop probing runs on a DEDICATED thread, never on the retile/hook/preview
// threads. Rationale (WER AppHang reports): `VirtualDesktopManager` COM +
// creating a temp window inline on a thread that never pumps messages is a
// deadlock recipe — an apartment-threaded COM object + a window that can
// receive synchronous messages, both hosted on a no-pump thread. The prober
// initializes COM as MTA (pump-free), owns the temp window + 300 ms settle
// sleep, and answers requests over a channel; callers wait with a bounded
// timeout so no hot path (retile wake, layout commit) can ever hang on this.
static PROBER: OnceLock<std::sync::mpsc::Sender<std::sync::mpsc::SyncSender<Option<GUID>>>> =
    OnceLock::new();

/// Last probe result cache lives inside the prober (rapid event bursts and
/// the 300 ms switch animation shouldn't each spawn a temp window).
const CACHE_MS: u128 = 1000;
/// Hard ceiling a caller will wait for a fresh probe.
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(700);

fn probe_desktop() -> Option<GUID> {
    use std::sync::mpsc;
    use std::time::Instant;
    let tx = PROBER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<mpsc::SyncSender<Option<GUID>>>();
        std::thread::spawn(move || {
            // MTA: cross-apartment COM calls are handled internally, no message
            // pumping required on this thread — ever.
            unsafe {
                let _ = windows_sys::Win32::System::Com::CoInitializeEx(
                    std::ptr::null(),
                    windows_sys::Win32::System::Com::COINIT_MULTITHREADED as u32,
                );
            }
            let mut cached: Option<(GUID, Instant)> = None;
            for reply in rx {
                let now = Instant::now();
                if let Some((g, t)) = cached {
                    if now.duration_since(t).as_millis() < CACHE_MS {
                        let _ = reply.send(Some(g));
                        continue;
                    }
                }
                let g = do_probe();
                if let Some(g) = g {
                    cached = Some((g, now));
                }
                let _ = reply.send(g);
            }
        });
        tx
    });
    let (rtx, rrx) = mpsc::sync_channel(1);
    if tx.send(rtx).is_err() {
        return None;
    }
    rrx.recv_timeout(REPLY_TIMEOUT).ok().flatten()
}

fn do_probe() -> Option<GUID> {
    let v = crate::virtual_desktop::VirtualDesktopManager::new().ok()?;
    let temp = crate::desktop::create_temp_window();
    if temp.is_null() {
        return None;
    }
    crate::win32::show(temp);
    std::thread::sleep(std::time::Duration::from_millis(300));
    struct TempGuard(windows_sys::Win32::Foundation::HWND);
    impl Drop for TempGuard {
        fn drop(&mut self) { crate::desktop::destroy_window(self.0); }
    }
    let _guard = TempGuard(temp);
    v.current_desktop_id(temp).ok()
}

/// Record the layout the user just applied for the desktop they're viewing.
pub fn note_layout(name: &str) {
    let Some(g) = current_guid() else { return };
    let mut desks = DESKS.lock().unwrap();
    store(&mut desks, g, DeskState { layout: Some(name.to_string()) });
}

fn store(desks: &mut Vec<(GUID, DeskState)>, g: GUID, st: DeskState) {
    if let Some(slot) = desks.iter_mut().find(|(id, _)| guid_eq(id, &g)) {
        slot.1 = st;
        return;
    }
    desks.push((g, st));
    if desks.len() > CAP {
        desks.remove(0); // FIFO eviction of the oldest
    }
}

fn stored_layout(g: &GUID) -> Option<String> {
    let desks = DESKS.lock().unwrap();
    desks.iter().find(|(id, _)| guid_eq(id, g)).and_then(|(_, s)| s.layout.clone())
}

/// Called once per re-tile tick (inside the debounce wake). Detects a desktop
/// switch by GUID hash and returns whether one happened (updating the memo).
/// GUID None just means the foreground wasn't resolvable — treated as no info.
pub fn on_retile_tick() -> bool {
    let Some(g) = current_guid() else { return false };
    let h = fold(&g);
    if !LAST_DESK_KNOWN.swap(true, Ordering::Relaxed) {
        LAST_DESK_HASH.store(h, Ordering::Relaxed);
        return false;
    }
    if LAST_DESK_HASH.load(Ordering::Relaxed) == h {
        return false;
    }
    LAST_DESK_HASH.store(h, Ordering::Relaxed);
    true
}

/// Desktop switch: re-apply the remembered layout for the new desktop
/// (anchored to the current workarea, never archived pixels). Returns false
/// when that desktop has no remembered layout — geometry stays untouched.
pub fn restore_layout_for_current() -> bool {
    let Some(g) = current_guid() else { return false };
    let Some(name) = stored_layout(&g) else { return false };
    crate::layouts::apply(&name)
}

/// Count-change re-tile on the *same* desktop: apply the remembered layout,
/// or fall back to the plain grid when this desktop has none.
pub fn retile_current() {
    let mut name = None;
    if let Some(g) = current_guid() {
        name = stored_layout(&g);
    }
    match name {
        Some(n) => {
            crate::layouts::apply(&n);
        }
        None => crate::layout::fit_all(),
    }
}

/// Tiling turned off: forget which desktop we were on so the next enable
/// starts fresh (memory itself persists — it's tiny and user-chosen).
pub fn clear_last_desk() {
    LAST_DESK_KNOWN.store(false, Ordering::Relaxed);
}
