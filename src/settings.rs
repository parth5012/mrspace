// ---------------------------------------------------------------------------
// Settings desktop app — native Win32 window covering every config.json field.
//
// Runs in its own process (like the palette): the daemon spawns it with
// MRSPACE_NO_IPC=1 so the UI loop never blocks IPC handling; a second settings
// window closes itself and raises the already-open one (single instance).
//
// The form edits the serde Config struct directly:
//   * IPC port, resize step/timeout, tiling interval
//   * managed-window exclusions (titles / classes / pids), one per line
//   * border overlay options (enable, thickness, accent & muted hex colors)
//   * the full hotkey table (name|modifiers|vk|action, one per line)
// Buttons:
//   Save — write config.json
//   Save & Apply — write config.json + tell the daemon to reload live options
//   Open Config — open config.json in the default editor
// ---------------------------------------------------------------------------

use crate::config::{self, Config, Hotkey};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, FindWindowW, GetDlgItem, GetMessageW,
    GetWindowTextW, PostQuitMessage, RegisterClassW, SendMessageW, SetForegroundWindow,
    SetWindowTextW, ShowWindow, TranslateMessage, BM_GETCHECK, BM_SETCHECK, CW_USEDEFAULT,
    ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, MSG, SW_RESTORE, SW_SHOW, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
    WS_VISIBLE,
};

// BST_ constants live in the Controls feature; inline them to avoid the import.
const BST_CHECKED: isize = 1;
const BST_UNCHECKED: isize = 0;

const CLASS_NAME: &str = "MrspaceSettings";

// control ids
const ID_PORT: i32 = 101;
const ID_STEP: i32 = 102;
const ID_TIMEOUT: i32 = 103;
const ID_TILING_INTERVAL: i32 = 104;
const ID_EX_TITLES: i32 = 111;
const ID_EX_CLASSES: i32 = 112;
const ID_EX_PIDS: i32 = 113;
const ID_BORDERS_ENABLE: i32 = 114;
const ID_BORDERS_THICKNESS: i32 = 115;
const ID_BORDERS_ACCENT: i32 = 116;
const ID_BORDERS_MUTED: i32 = 117;
const ID_HOTKEYS: i32 = 121;
const ID_SAVE_APPLY: i32 = 201;
const ID_SAVE: i32 = 202;
const ID_CANCEL: i32 = 203;
const ID_OPEN_CONFIG: i32 = 204;
const ID_STATUS: i32 = 130;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn show() {
    unsafe {
        let class_name = wide(CLASS_NAME);
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.lpszClassName = class_name.as_ptr();
        RegisterClassW(&wc);

        let title = wide("Mr. Space — Settings");
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            664,
            600,
            0 as HWND,
            0 as _,
            0 as _,
            std::ptr::null(),
        );
        ShowWindow(hwnd, SW_SHOW);
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0 as HWND, 0, 0);
            if r <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Raise an already-open settings window (single instance). Returns true if one existed.
pub fn raise_settings() -> bool {
    unsafe {
        let cls = wide(CLASS_NAME);
        let h = FindWindowW(cls.as_ptr(), std::ptr::null());
        if h.is_null() {
            return false;
        }
        ShowWindow(h, SW_RESTORE);
        SetForegroundWindow(h);
        true
    }
}

unsafe fn child(
    parent: HWND,
    class: &str,
    text: &str,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    style: u32,
    id: i32,
) {
    let cls = wide(class);
    let txt = wide(text);
    CreateWindowExW(
        0,
        cls.as_ptr(),
        txt.as_ptr(),
        style,
        x,
        y,
        w,
        h,
        parent,
        id as _,
        0 as _,
        std::ptr::null(),
    );
}

unsafe fn get_text(parent: HWND, id: i32) -> String {
    let item = GetDlgItem(parent, id);
    if item.is_null() {
        return String::new();
    }
    let mut buf = [0u16; 4096];
    let n = GetWindowTextW(item, buf.as_mut_ptr(), buf.len() as i32);
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

#[allow(non_snake_case)]
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => {
            build_form(hwnd);
            0
        }
        WM_COMMAND => {
            let id = (wparam & 0xffff) as i32;
            match id {
                ID_SAVE_APPLY | ID_SAVE => {
                    match collect_config(hwnd) {
                        Ok(cfg) => {
                            let old_port = config::load().ipc_port;
                            if let Err(e) = config::save(&cfg) {
                                set_status(hwnd, &format!("ERR save: {e}"));
                            } else {
                                set_status(hwnd, "saved ✓");
                                if id == ID_SAVE_APPLY {
                                    let msg = try_apply(&cfg, old_port);
                                    set_status(hwnd, &msg);
                                }
                            }
                        }
                        Err(e) => set_status(hwnd, &format!("ERR: {e}")),
                    }
                    0
                }
                ID_CANCEL => {
                    PostQuitMessage(0);
                    0
                }
                ID_OPEN_CONFIG => {
                    let p = config::path();
                    let _ = std::process::Command::new("explorer")
                        .arg(&p.parent().unwrap_or_else(|| std::path::Path::new(".")))
                        .spawn();
                    0
                }
                _ => 0,
            }
        }
        WM_CLOSE | WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn build_form(parent: HWND) {
    const B: u32 = WS_CHILD | WS_VISIBLE | WS_BORDER;
    const E: u32 = B | WS_TABSTOP;
    const ML: u32 = E | (ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL) as u32;

    // top row scalars
    child(parent, "STATIC", "IPC port", 10, 14, 70, 16, B, 0);
    child(parent, "EDIT", "", 10, 32, 70, 22, E, ID_PORT);
    child(parent, "STATIC", "resize step", 95, 14, 70, 16, B, 0);
    child(parent, "EDIT", "", 95, 32, 70, 22, E, ID_STEP);
    child(parent, "STATIC", "resize timeout", 180, 14, 90, 16, B, 0);
    child(parent, "EDIT", "", 180, 32, 70, 22, E, ID_TIMEOUT);
    child(parent, "STATIC", "tiling interval", 265, 14, 90, 16, B, 0);
    child(parent, "EDIT", "", 265, 32, 70, 22, E, ID_TILING_INTERVAL);
    child(parent, "STATIC", "(ms)", 340, 32, 40, 22, B, 0);

    // exclusions
    child(parent, "STATIC", "Exclusions — one entry per line", 10, 64, 300, 16, B, 0);
    child(parent, "STATIC", "titles", 10, 82, 200, 14, B, 0);
    child(parent, "EDIT", "", 10, 98, 220, 104, ML, ID_EX_TITLES);
    child(parent, "STATIC", "classes", 240, 82, 200, 14, B, 0);
    child(parent, "EDIT", "", 240, 98, 220, 104, ML, ID_EX_CLASSES);
    child(parent, "STATIC", "pids", 470, 82, 80, 14, B, 0);
    child(parent, "EDIT", "", 470, 98, 180, 104, ML, ID_EX_PIDS);

    // borders
    child(parent, "STATIC", "Borders", 10, 212, 100, 16, B, 0);
    child(parent, "BUTTON", "Enable", 90, 210, 60, 22, B | 0x3 /*BS_AUTOCHECKBOX*/, ID_BORDERS_ENABLE);
    child(parent, "STATIC", "thickness", 158, 212, 60, 16, B, 0);
    child(parent, "EDIT", "", 222, 210, 40, 22, E, ID_BORDERS_THICKNESS);
    child(parent, "STATIC", "accent (hex RRGGBB)", 272, 212, 110, 16, B, 0);
    child(parent, "EDIT", "", 388, 210, 70, 22, E, ID_BORDERS_ACCENT);
    child(parent, "STATIC", "muted", 470, 212, 40, 16, B, 0);
    child(parent, "EDIT", "", 512, 210, 70, 22, E, ID_BORDERS_MUTED);

    // hotkeys
    child(parent, "STATIC", "Hotkeys — one per line: name|modifiers|vk|action (vk hex e.g. 0x25, modifiers decimal 1=alt 2=ctrl 4=shift 8=win)", 10, 248, 640, 16, B, 0);
    child(parent, "EDIT", "", 10, 268, 640, 216, ML, ID_HOTKEYS);

    // buttons
    const BTN: u32 = WS_CHILD | WS_VISIBLE | WS_TABSTOP | 0x10; // BS_PUSHBUTTON
    child(parent, "BUTTON", "Save && Apply", 10, 494, 110, 26, BTN, ID_SAVE_APPLY);
    child(parent, "BUTTON", "Save", 126, 494, 70, 26, BTN, ID_SAVE);
    child(parent, "BUTTON", "Cancel", 202, 494, 70, 26, BTN, ID_CANCEL);
    child(parent, "BUTTON", "Open Config", 560, 494, 90, 26, BTN, ID_OPEN_CONFIG);
    child(parent, "STATIC", "", 10, 528, 640, 26, B, ID_STATUS);

    // fill from the real config
    let cfg = config::load();
    set_text(parent, ID_PORT, &cfg.ipc_port.to_string());
    set_text(parent, ID_STEP, &cfg.resize_step.to_string());
    set_text(parent, ID_TIMEOUT, &cfg.resize_timeout_ms.to_string());
    set_text(parent, ID_TILING_INTERVAL, &cfg.tiling_interval_ms.to_string());
    set_text(parent, ID_EX_TITLES, &cfg.exclude_titles.join("\r\n"));
    set_text(parent, ID_EX_CLASSES, &cfg.exclude_classes.join("\r\n"));
    set_text(parent, ID_EX_PIDS, &cfg.exclude_pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("\r\n"));
    SendMessageW(GetDlgItem(parent, ID_BORDERS_ENABLE), BM_SETCHECK, if cfg.borders.enabled { BST_CHECKED as usize } else { BST_UNCHECKED as usize }, 0);    set_text(parent, ID_BORDERS_THICKNESS, &cfg.borders.thickness.to_string());
    set_text(parent, ID_BORDERS_ACCENT, &cfg.borders.accent.clone());
    set_text(parent, ID_BORDERS_MUTED, &cfg.borders.muted.clone());
    let hklines: Vec<String> = cfg.hotkeys.iter().map(|h| {
        format!("{}|{}|{:#x}|{}", h.name, h.modifiers, h.vk, h.action)
    }).collect();
    set_text(parent, ID_HOTKEYS, &hklines.join("\r\n"));
}

unsafe fn set_text(parent: HWND, id: i32, txt: &str) {
    let w = wide(txt);
    SetWindowTextW(GetDlgItem(parent, id), w.as_ptr());
}
unsafe fn set_status(parent: HWND, txt: &str) {
    set_text(parent, ID_STATUS, txt);
}

/// Parse the form into a Config, or an error message on the first bad field.
#[allow(non_snake_case)]
unsafe fn collect_config(parent: HWND) -> Result<Config, String> {
    let ip = get_text(parent, ID_PORT).trim().parse::<u16>().map_err(|_| format!("IPC port: not a number (got {:?})", get_text(parent, ID_PORT)))?;
    let step = get_text(parent, ID_STEP).trim().parse::<i32>().map_err(|_| "resize step: not a number")?;
    let timeout = get_text(parent, ID_TIMEOUT).trim().parse::<u64>().map_err(|_| "resize timeout: not a number")?;
    let interval = get_text(parent, ID_TILING_INTERVAL).trim().parse::<u64>().map_err(|_| "tiling interval: not a number")?;

    let lines = |id: i32| -> Vec<String> {
        get_text(parent, id)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect()
    };
    let exclude_titles = lines(ID_EX_TITLES);
    let exclude_classes = lines(ID_EX_CLASSES);
    let exclude_pids: Result<Vec<u32>, String> = lines(ID_EX_PIDS).iter().map(|l| l.parse::<u32>().map_err(|_| format!("exclusion pids: not a number (line {l:?})"))).collect();
    let exclude_pids = exclude_pids?;

    let enabled = SendMessageW(GetDlgItem(parent, ID_BORDERS_ENABLE), BM_GETCHECK, 0, 0) == BST_CHECKED as isize;
    let thickness = get_text(parent, ID_BORDERS_THICKNESS).trim().parse::<i32>().map_err(|_| "border thickness: not a number")?.clamp(1, 20);
    let raw_accent = get_text(parent, ID_BORDERS_ACCENT);
    let raw_muted = get_text(parent, ID_BORDERS_MUTED);

    // hotkeys: name|modifiers|vk|action
    let mut hotkeys = Vec::new();
    for (i, l) in get_text(parent, ID_HOTKEYS).lines().enumerate() {
        let l = l.trim();
        if l.is_empty() {
            continue;
        }
        let f: Vec<&str> = l.split('|').collect();
        if f.len() != 4 {
            return Err(format!("hotkey line {}: need name|modifiers|vk|action", i + 1));
        }
        let modifiers = f[1].trim().parse::<u32>().map_err(|_| format!("hotkey line {}: modifiers not a number", i + 1))?;
        let vs = f[2].trim();
        let vk = u32::from_str_radix(vs.trim_start_matches("0x"), 16).map_err(|_| format!("hotkey line {}: vk not hex", i + 1))?;
        let action = f[3].trim().to_string();
        let name = f[0].trim().to_string();
        if name.is_empty() || action.is_empty() {
            return Err(format!("hotkey line {}: empty name/action", i + 1));
        }
        hotkeys.push(Hotkey { name, modifiers, vk, action });
    }

    Ok(Config {
        ipc_port: ip,
        resize_step: step,
        resize_timeout_ms: timeout,
        tiling_interval_ms: interval,
        exclude_titles,
        exclude_classes,
        exclude_pids,
        borders: crate::config::BorderSettings {
            enabled,
            thickness,
            accent: raw_accent.trim().trim_start_matches('#').to_string(),
            muted: raw_muted.trim().trim_start_matches('#').to_string(),
        },
        hotkeys,
    })
}

fn try_apply(cfg: &Config, old_port: u16) -> String {
    let mut msgs: Vec<String> = Vec::new();
    for port in [cfg.ipc_port, old_port] {
        if let Some(reply) = crate::daemon::try_send_command(port, "reload-config") {
            if reply.contains("ERR") {
                msgs.push(format!("reload @{}: {reply}", port));
            } else {
                msgs.push(format!("daemon reloaded (port {port})"));
            }
        }
    }
    if msgs.is_empty() {
        "saved ✓ but no running daemon found — restart it to pick changes up".into()
    } else {
        msgs.join("; ")
    }
}
