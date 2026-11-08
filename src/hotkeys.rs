use crate::config::Config;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, SendInput, UnregisterHotKey, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP,
    VK_MENU, VK_CONTROL, VK_SHIFT, VK_LWIN,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, TranslateMessage, MSG, WM_HOTKEY,
};

pub struct HotkeyLoop {
    ids: Vec<i32>,
}

impl HotkeyLoop {
    pub fn register(cfg: &Config) -> Self {
        let mut ids = Vec::new();
        for (i, hk) in cfg.hotkeys.iter().enumerate() {
            let id = (i as i32) + 1;
            let ok = unsafe { RegisterHotKey(0 as HWND, id, hk.modifiers, hk.vk) };
            if ok == 0 {
                eprintln!("warning: failed to register hotkey '{}' (mod={}, vk={})", hk.name, hk.modifiers, hk.vk);
            } else {
                ids.push(id);
            }
        }
        Self { ids }
    }

    /// Blocking message loop. Calls `on_action` with action name on hotkey.
    pub fn run(&self, cfg: &Config, mut on_action: impl FnMut(&str)) {
        unsafe {
            let mut msg: MSG = std::mem::zeroed();
            loop {
                let r = GetMessageW(&mut msg, 0 as HWND, 0, 0);
                if r <= 0 { break; }
                if msg.message == WM_HOTKEY {
                    let id = msg.wParam as i32;
                    if let Some(idx) = self.ids.iter().position(|&x| x == id) {
                        if let Some(hk) = cfg.hotkeys.get(idx) {
                            let mods = hk.modifiers;
                            on_action(&hk.action);
                            // Release modifier keys so Alt/Ctrl/Shift don't get stuck
                            // in apps after the global hotkey fires (avoids "locked Alt").
                            release_modifiers(mods);
                        }
                    }
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

impl Drop for HotkeyLoop {
    fn drop(&mut self) {
        for id in &self.ids {
            unsafe { UnregisterHotKey(0 as HWND, *id); }
        }
    }
}

fn release_modifiers(mods: u32) {
    // MOD_ALT=1, MOD_CONTROL=2, MOD_SHIFT=4, MOD_WIN=8
    const MOD_ALT: u32 = 1;
    const MOD_CONTROL: u32 = 2;
    const MOD_SHIFT: u32 = 4;
    const MOD_WIN: u32 = 8;
    let mut inputs: Vec<INPUT> = Vec::new();
    if mods & MOD_ALT != 0 {
        inputs.push(keyup(VK_MENU));
    }
    if mods & MOD_CONTROL != 0 {
        inputs.push(keyup(VK_CONTROL));
    }
    if mods & MOD_SHIFT != 0 {
        inputs.push(keyup(VK_SHIFT));
    }
    if mods & MOD_WIN != 0 {
        inputs.push(keyup(VK_LWIN));
    }
    if inputs.is_empty() { return; }
    unsafe {
        SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
    }
}

fn keyup(vk: u16) -> INPUT {
    let mut inp: INPUT = unsafe { std::mem::zeroed() };
    inp.r#type = INPUT_KEYBOARD;
    inp.Anonymous.ki.wVk = vk;
    inp.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
    inp
}