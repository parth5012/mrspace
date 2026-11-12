//! Binding modes (ROADMAP Tier 3.10): named modes (resize / move) that reuse
//! the chord dispatch. The mode -> action map is data, not code per mode.

use std::sync::{LazyLock, Mutex};

static MODE: LazyLock<Mutex<String>> = LazyLock::new(|| Mutex::new("base".into()));

pub fn current() -> String {
    MODE.lock().unwrap().clone()
}

pub fn enter(name: &str) {
    *MODE.lock().unwrap() = name.to_string();
}

pub fn exit() {
    *MODE.lock().unwrap() = "base".to_string();
}

/// Returns the action for a key in the given mode, if any.
pub fn action_for(mode: &str, vk: u32) -> Option<&'static str> {
    match mode {
        "resize" => match vk {
            0x25 => Some("nudge-left"),
            0x27 => Some("nudge-right"),
            0x26 => Some("nudge-up"),
            0x28 => Some("nudge-down"),
            _ => None,
        },
        "move" => match vk {
            0x25 => Some("move-left"),
            0x27 => Some("move-right"),
            0x26 => Some("move-up"),
            0x28 => Some("move-down"),
            _ => None,
        },
        _ => None,
    }
}