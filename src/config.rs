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
    pub hotkeys: Vec<Hotkey>,
}

impl Default for Config {
    fn default() -> Self {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{MOD_CONTROL, MOD_SHIFT, VK_LEFT, VK_RIGHT, VK_UP, VK_DOWN};
        Self {
            ipc_port: 17331,
            hotkeys: vec![
                Hotkey { name: "pin-left".into(), modifiers: (MOD_CONTROL | MOD_SHIFT) as u32, vk: VK_LEFT as u32, action: "pin-left".into() },
                Hotkey { name: "pin-right".into(), modifiers: (MOD_CONTROL | MOD_SHIFT) as u32, vk: VK_RIGHT as u32, action: "pin-right".into() },
                Hotkey { name: "fit-all".into(), modifiers: (MOD_CONTROL | MOD_SHIFT) as u32, vk: VK_UP as u32, action: "fit-all".into() },
                Hotkey { name: "maximize".into(), modifiers: (MOD_CONTROL | MOD_SHIFT) as u32, vk: VK_DOWN as u32, action: "maximize".into() },
            ],
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