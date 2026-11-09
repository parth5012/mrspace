use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Once;
use std::thread;
use std::time::Duration;

use windows_sys::Win32::Foundation::HANDLE;
use windows_sys::Win32::System::Threading::{
    CreateEventW, SetEvent, WaitForSingleObject, INFINITE,
};
use windows_sys::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook, WINEVENTPROC};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EVENT_OBJECT_CLOAKED, EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
    EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, GetMessageW, MSG, OBJID_WINDOW, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS, DispatchMessageW,
};

// TEMP debug tracing, gated by MRS_TRACE=path (removed once the hook engine
// has soaked); every call only does work when the env var was set at startup
static TRACE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
fn trace(msg: &str) {
    let path = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok());
    if let Some(p) = path {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            use std::io::Write;
            let _ = writeln!(f, "[{}] {}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0), msg);
        }
    }
}

/// event hook registration results, for the MRS_TRACE debug log
unsafe fn hook_log(h: HWINEVENTHOOK, min: u32, max: u32) {
    trace(&format!(
        "hook {}..={} registered={}",
        min, max, !h.is_null()
    ));
}

static ACTIVE: AtomicBool = AtomicBool::new(false);
static DIRTY: AtomicBool = AtomicBool::new(false);
static LAST_COUNT: AtomicU64 = AtomicU64::new(0);
static INIT: Once = Once::new();
/// auto-reset event the hook flips and the re-tiler waits on (zero idle wakeups)
static WAKE: AtomicU64 = AtomicU64::new(0);

// a window-level event that can change the *set* of manageable windows.
// LOCATIONCHANGE is deliberately not hooked: dragging/resizing must not re-tile.
const INTERESTING: [u32; 8] = [
    EVENT_OBJECT_CREATE,
    EVENT_OBJECT_DESTROY,
    EVENT_OBJECT_SHOW,
    EVENT_OBJECT_HIDE,
    EVENT_SYSTEM_MINIMIZESTART,
    EVENT_SYSTEM_MINIMIZEEND,
    EVENT_OBJECT_CLOAKED,
    EVENT_OBJECT_UNCLOAKED,
];

unsafe extern "system" fn hook_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: windows_sys::Win32::Foundation::HWND,
    idobject: i32,
    idchild: i32,
    _thread: u32,
    _time: u32,
) {
    if !ACTIVE.load(Ordering::Relaxed) {
        return;
    }
    if idobject != OBJID_WINDOW || idchild != 0 {
        return;
    }
    if !INTERESTING.contains(&event) {
        return;
    }
    DIRTY.store(true, Ordering::Relaxed);
    let h = WAKE.load(Ordering::Relaxed);
    if h != 0 {
        SetEvent(h as HANDLE);
    }
}

/// manage event: how many manageable windows do we have right now
fn manageable_count() -> u64 {
    crate::win32::enum_windows()
        .into_iter()
        .filter(|&h| crate::win32::is_manageable(h))
        .count() as u64
}

pub fn init() {
    INIT.call_once(|| unsafe {
        // auto-reset event used as the debounce-free wakeup channel
        let ev = CreateEventW(std::ptr::null(), 0, 0, std::ptr::null());
        WAKE.store(ev as usize as u64, Ordering::Relaxed);

        // hook thread: WINEVENT_OUTOFCONTEXT callbacks arrive via this
        // thread's message queue, so it must pump messages.
        thread::spawn(|| {
            let proc: WINEVENTPROC = Some(hook_proc);
            // CREATE..HIDE is one contiguous range (0x8000..=0x8003)
            hook_log(SetWinEventHook(0x8000, 0x8003, std::ptr::null_mut(), proc, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS), 0x8000, 0x8003);
            hook_log(SetWinEventHook(EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND, std::ptr::null_mut(), proc, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS), EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MINIMIZEEND);
            hook_log(SetWinEventHook(EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED, std::ptr::null_mut(), proc, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS), EVENT_OBJECT_CLOAKED, EVENT_OBJECT_UNCLOAKED);
            let mut msg: MSG = std::mem::zeroed();
            loop {
                let r = GetMessageW(&mut msg, 0 as _, 0, 0);
                if r <= 0 {
                    break;
                }
                // OUTOFCONTEXT event callbacks are delivered during dispatch —
                // a bare GetMessage drain silently drops them
                DispatchMessageW(&mut msg);
            }
        });

        // re-tiler thread: sleeps in WaitForSingleObject (no polling);
        // wakes only when the hook marks a set-changing event, debounces
        // 150 ms to batch bursts (e.g. an app opening 3 windows), and
        // re-tiles only when the manageable *set* changed — so drags,
        // resizes and moves never trigger a re-tile.
        thread::spawn(|| {
            loop {
                let h = WAKE.load(Ordering::Relaxed) as HANDLE;
                WaitForSingleObject(h, INFINITE);
                trace(&format!(
                    "wake dirty={} active={}",
                    DIRTY.load(Ordering::Relaxed),
                    ACTIVE.load(Ordering::Relaxed)
                ));
                thread::sleep(Duration::from_millis(150));
                DIRTY.store(false, Ordering::Relaxed);
                if !ACTIVE.load(Ordering::Relaxed) {
                    LAST_COUNT.store(0, Ordering::Relaxed);
                    continue;
                }
                let count = manageable_count();
                let last = LAST_COUNT.load(Ordering::Relaxed);
                let changed = count != last;
                trace(&format!("count={} last={} retile={}", count, last, changed));
                if changed {
                    crate::layout::fit_all();
                    LAST_COUNT.store(count, Ordering::Relaxed);
                }
            }
        });
    });
}

pub fn is_active() -> bool { ACTIVE.load(Ordering::Relaxed) }

pub fn toggle() -> bool {
    let new = !ACTIVE.load(Ordering::Relaxed);
    ACTIVE.store(new, Ordering::Relaxed);
    if new {
        // tile immediately once when turning on
        crate::layout::fit_all();
        crate::borders::sync_all_managed();
        LAST_COUNT.store(manageable_count(), Ordering::Relaxed);
    } else {
        // borders only make sense for mrspace-arranged windows
        crate::borders::hide_all();
    }
    new
}
