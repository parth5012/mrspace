use crate::config;
use crate::layout;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Once;
use std::thread;
use std::time::Duration;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static LAST_COUNT: AtomicU64 = AtomicU64::new(0);
static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        thread::spawn(|| loop {
            thread::sleep(Duration::from_millis(1000));
            if !ACTIVE.load(Ordering::Relaxed) {
                LAST_COUNT.store(0, Ordering::Relaxed);
                continue;
            }
            // Only re-tile when the number of manageable windows changes,
            // so it doesn't fight the user while they resize/move things.
            let count = crate::win32::enum_windows()
                .into_iter()
                .filter(|&h| crate::win32::is_manageable(h))
                .count() as u64;
            let last = LAST_COUNT.load(Ordering::Relaxed);
            if count != last {
                crate::layout::fit_all();
                LAST_COUNT.store(count, Ordering::Relaxed);
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
        LAST_COUNT.store(
            crate::win32::enum_windows().into_iter().filter(|&h| crate::win32::is_manageable(h)).count() as u64,
            Ordering::Relaxed,
        );
    }
    new
}