use crate::config;
use crate::layout;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::thread;
use std::time::Duration;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static INIT: Once = Once::new();

pub fn init() {
    INIT.call_once(|| {
        thread::spawn(|| loop {
            thread::sleep(Duration::from_millis(1000));
            if ACTIVE.load(Ordering::Relaxed) {
                let iv = config::load().tiling_interval_ms;
                // simple: tile every interval
                layout::fit_all();
                thread::sleep(Duration::from_millis(iv.saturating_sub(1000)));
            }
        });
    });
}

pub fn is_active() -> bool { ACTIVE.load(Ordering::Relaxed) }

pub fn toggle() -> bool {
    let new = !ACTIVE.load(Ordering::Relaxed);
    ACTIVE.store(new, Ordering::Relaxed);
    new
}