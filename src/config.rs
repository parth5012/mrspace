use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hotkey {
    pub name: String,
    pub modifiers: u32,
    pub vk: u32,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub ipc_port: u16,
    pub resize_step: i32,
    pub resize_timeout_ms: u64,
    pub tiling_interval_ms: u64,
    /// Window titles to never manage (substring, case-insensitive).
    pub exclude_titles: Vec<String>,
    /// Window classes to never manage (e.g. Rainmeter skin windows).
    pub exclude_classes: Vec<String>,
    /// Process IDs to never manage.
    pub exclude_pids: Vec<u32>,
    pub hotkeys: Vec<Hotkey>,
}

impl Default for Config {
    fn default() -> Self {
        const MOD: u32 = 2 | 1; // Ctrl+Alt
        let mut hk = Vec::new();
        let add = |hk: &mut Vec<Hotkey>, name: &str, vk: u32, action: &str| {
            hk.push(Hotkey { name: name.into(), modifiers: MOD, vk, action: action.into() });
        };
        add(&mut hk, "pin-left", 0x25, "pin-left"); // VK_LEFT
        add(&mut hk, "pin-right", 0x27, "pin-right");
        add(&mut hk, "pin-top", 0x26, "pin-top");
        add(&mut hk, "pin-bottom", 0x28, "pin-bottom");
        add(&mut hk, "maximize", 0x4D, "maximize"); // M
        add(&mut hk, "fit-all", 0x47, "fit-all"); // G
        add(&mut hk, "palette", 0x20, "palette"); // Space
        add(&mut hk, "toggle-tiling", 0x54, "toggle-tiling"); // T
        add(&mut hk, "layout-cycle", 0x4F, "layout-cycle"); // O
        add(&mut hk, "focus-left", 0x48, "focus-left"); // H
        add(&mut hk, "focus-down", 0x4A, "focus-down"); // J
        add(&mut hk, "focus-up", 0x4B, "focus-up"); // K
        add(&mut hk, "focus-right", 0x4C, "focus-right"); // L
        add(&mut hk, "resize-mode", 0x52, "resize-mode"); // R
        add(&mut hk, "next-desktop", 0x4E, "next-desktop"); // N
        add(&mut hk, "prev-desktop", 0x50, "prev-desktop"); // P
        for i in 1..=9 {
            add(&mut hk, &format!("desktop-{i}"), 0x30 + i as u32, &format!("desktop-{i}"));
        }
        Self {
            ipc_port: 17331,
            resize_step: 10,
            resize_timeout_ms: 5000,
            tiling_interval_ms: 2000,
            exclude_titles: vec![
                "Program Manager".into(),
                "Windows Input Experience".into(),
                "Settings".into(),
                "Microsoft Text Input Application".into(),
            ],
            exclude_classes: vec!["RainmeterMeterWindow".into()],
            exclude_pids: vec![],
            hotkeys: hk,
        }
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("mrspace").join("config.json")
}

pub fn load() -> Config {
    let p = config_path();
    if let Ok(s) = fs::read_to_string(&p) {
        if let Ok(c) = serde_json::from_str(&s) { return c; }
    }
    let c = Config::default();
    let _ = save(&c);
    c
}
pub fn save(c: &Config) -> Result<()> {
    let p = config_path();
    if let Some(parent) = p.parent() { fs::create_dir_all(parent)?; }
    let s = serde_json::to_string_pretty(c)?;
    fs::write(&p, s)?;
    Ok(())
}

pub fn path() -> PathBuf { config_path() }