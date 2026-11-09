// ---------------------------------------------------------------------------
// Settings desktop app — native Win32 window covering every config.json field.
//
// Runs in its own process (like the palette): the daemon spawns it with
// MRSPACE_NO_IPC=1 so the UI loop never blocks IPC handling; a second settings
// window closes itself and raises the already-open one (single instance).
//
// Material-3 dark inspired theme: near-black surfaces, purple accent,
// rounded owner-draw buttons, dark + rounded DWM title bar.
//
// The form edits the serde Config struct directly:
//   * IPC port, resize step/timeout, tiling interval
//   * managed-window exclusions (titles / classes / pids), one per line
//   * border overlay options (enable, thickness, accent & muted hex colors)
//   * the full hotkey table (name|modifiers|vk|action, one per line)
// ---------------------------------------------------------------------------

use crate::config::{self, Config, Hotkey};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute;
use windows_sys::Win32::Graphics::Gdi::{
    CreateFontW, CreatePen, CreateSolidBrush, DrawTextW, RoundRect, SelectObject, SetBkMode,
    SetTextColor, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
};
use windows_sys::Win32::UI::Controls::{DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, ODT_BUTTON};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetDlgCtrlID, GetDlgItem,
    GetMessageW, GetWindowTextW, IsDialogMessageW, PostQuitMessage, RegisterClassW, SendMessageW,
    SetForegroundWindow, SetWindowTextW, ShowWindow, TranslateMessage, BM_GETCHECK, BM_SETCHECK,
    CW_USEDEFAULT, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, MSG, SW_RESTORE, SW_SHOW,
    WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_DRAWITEM, WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_TOOLWINDOW,
    WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
};

// constants that live in other feature modules; inlined to keep the feature set small
const BST_CHECKED: isize = 1;
const BST_UNCHECKED: isize = 0;
const BS_PUSHBUTTON: u32 = 0x10;
const BS_AUTOCHECKBOX: u32 = 0x3;
const BS_OWNERDRAW: u32 = 0xB;
const PS_SOLID: i32 = 0;
const TRANSPARENT: i32 = 1;
// COLORREF-style 0x00BBGGRR from (r,g,b)
const fn c3(r: u32, g: u32, b: u32) -> u32 {
    (b << 16) | (g << 8) | r
}

// M3-dark palette
const CLR_BG: u32 = c3(0x16, 0x14, 0x1C); // surface
const CLR_FIELD: u32 = c3(0x24, 0x22, 0x2C); // surfaceContainer
const CLR_TEXT: u32 = c3(0xE6, 0xE0, 0xEA); // onSurface
const CLR_TEXT_DIM: u32 = c3(0xCA, 0xC4, 0xD2); // onSurfaceVariant
const CLR_PRIMARY: u32 = c3(0xD0, 0xBC, 0xFF); // primary
const CLR_PRIMARY_DIM: u32 = c3(0xA9, 0x8F, 0xE0);
const CLR_ON_PRIMARY: u32 = c3(0x2B, 0x1A, 0x4F);
const CLR_OUTLINE: u32 = c3(0x93, 0x8F, 0x99);

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
// section header statics (accent-colored)
const SEC_BASE: i32 = 301;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Theme handles (brushes + fonts), created once before children exist.
struct Theme {
    font: isize,     // body font
    hfont: isize,    // header font (semibold)
    win_bg: isize,   // brush
    field_bg: isize, // brush
}
static THEME: std::sync::OnceLock<Theme> = std::sync::OnceLock::new();
static MAIN_HWND: std::sync::OnceLock<isize> = std::sync::OnceLock::new();
static DPI: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

/// Scale a logical 96-dpi design value to the system DPI (Per-Monitor v2 aware process).
fn s(v: i32) -> i32 {
    let d = DPI.get().copied().unwrap_or(96).max(96) as i32;
    v * d / 96
}

unsafe fn make_theme() -> Theme {
    let font = CreateFontW(
        s(18), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0x22, wide("Segoe UI").as_ptr(),
    ) as isize;
    let hfont = CreateFontW(
        s(20), 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 0, 0x22, wide("Segoe UI").as_ptr(),
    ) as isize;
    Theme {
        font,
        hfont,
        win_bg: CreateSolidBrush(CLR_BG) as isize,
        field_bg: CreateSolidBrush(CLR_FIELD) as isize,
    }
}

pub fn show() {
    unsafe {
        // Opt into real pixel coordinates so the layout is crisp at any display
        // scaling (Windows otherwise virtualizes a DPI-unaware process).
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = DPI.set(GetDpiForSystem());

        let class_name = wide(CLASS_NAME);
        let theme = make_theme();
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.lpszClassName = class_name.as_ptr();
        wc.hbrBackground = theme.win_bg as _; // M3 surface fills the window
        RegisterClassW(&wc);

        let _ = THEME.set(theme);

        let title = wide("Mr. Space — Settings");
        let hwnd = CreateWindowExW(
            // Toolwindow: komorebi/PowerToys-style tiling WMs (and our own tiler)
            // leave it alone, and it stays out of Alt-Tab. Tradeoff: no taskbar
            // button — the app is opened from the palette / Flow launcher anyway.
            WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            s(716),
            s(760),
            0 as HWND,
            0 as _,
            0 as _,
            std::ptr::null(),
        );
        let _ = MAIN_HWND.set(hwnd as isize);

        // Win11 polish: dark title bar + rounded window corners
        let dark: u32 = 1;
        DwmSetWindowAttribute(
            hwnd,
            20, // DWMWA_USE_IMMERSIVE_DARK_MODE
            &dark as *const u32 as *const _,
            4,
        );
        let round: u32 = 2; // DWMWCP_ROUND
        DwmSetWindowAttribute(
            hwnd,
            33, // DWMWA_WINDOW_CORNER_PREFERENCE
            &round as *const u32 as *const _,
            4,
        );

        ShowWindow(hwnd, SW_SHOW);

        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0 as HWND, 0, 0);
            if r <= 0 {
                break;
            }
            if let Some(h) = MAIN_HWND.get() {
                if IsDialogMessageW(*h as _, &mut msg) != 0 {
                    continue; // TAB navigation / accelerators handled
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Raise an already-open settings window (single instance). Returns true if one existed.
pub fn raise_settings() -> bool {
    unsafe {
        // FindWindowW proved unreliable for this class here, so enumerate instead.
        let mut target: HWND = std::ptr::null_mut();
        for h in crate::win32::enum_windows() {
            if crate::win32::class_name(h).eq_ignore_ascii_case(CLASS_NAME) {
                target = h;
                break;
            }
        }
        if target.is_null() {
            return false;
        }
        ShowWindow(target, SW_RESTORE);
        SetForegroundWindow(target);
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
    header: bool,
) {
    let cls = wide(class);
    let txt = wide(text);
    let h = CreateWindowExW(
        0,
        cls.as_ptr(),
        txt.as_ptr(),
        style,
        s(x),
        s(y),
        s(w),
        s(h),
        parent,
        id as _,
        0 as _,
        std::ptr::null(),
    );
    if let Some(t) = THEME.get() {
        let f = if header { t.hfont } else { t.font };
        SendMessageW(h, WM_SETFONT, f as usize, 1);
    }
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
        WM_DRAWITEM => {
            draw_button(lparam as *const DRAWITEMSTRUCT);
            1
        }
        // system asks which brush to paint behind our children
        WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
            let t = THEME.get().unwrap();
            SetBkMode(wparam as *mut _, TRANSPARENT);
            SetTextColor(wparam as *mut _, CLR_TEXT);
            t.field_bg as LRESULT
        }
        WM_CTLCOLORSTATIC => {
            let t = THEME.get().unwrap();
            let id = GetDlgCtrlID(lparam as HWND);
            SetBkMode(wparam as *mut _, TRANSPARENT);
            let color = if (SEC_BASE..SEC_BASE + 16).contains(&id) {
                CLR_PRIMARY // section headers
            } else {
                CLR_TEXT_DIM
            };
            SetTextColor(wparam as *mut _, color);
            t.win_bg as LRESULT
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
                    if let Some(dir) = p.parent() {
                        let _ = std::process::Command::new("explorer")
                            .arg("/select,")
                            .arg(&p)
                            .current_dir(dir)
                            .spawn();
                    }
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

/// M3-style rounded buttons drawn by hand: primary = filled lavender for
/// "Save & Apply", others = outlined tonal buttons.
unsafe fn draw_button(d: *const DRAWITEMSTRUCT) {
    if (*d).CtlType as u32 != ODT_BUTTON {
        return;
    }
    let hdc = (*d).hDC;
    let rc = (*d).rcItem;
    let l = rc.left;
    let t = rc.top;
    let r = rc.right;
    let b = rc.bottom;
    let pressed = (*d).itemState & ODS_SELECTED != 0;
    let disabled = (*d).itemState & ODS_DISABLED != 0;
    let id = (*d).CtlID as i32;
    let primary = id == ID_SAVE_APPLY;

    let (fill, text, outline) = if primary {
        (CLR_PRIMARY, CLR_ON_PRIMARY, CLR_PRIMARY)
    } else {
        (CLR_FIELD, CLR_PRIMARY, CLR_OUTLINE)
    };
    let (fill, text) = if disabled {
        (CLR_FIELD, CLR_TEXT_DIM)
    } else if pressed {
        (CLR_PRIMARY_DIM, CLR_ON_PRIMARY)
    } else {
        (fill, text)
    };

    let brush = CreateSolidBrush(fill);
    // 1px pen around: for the primary button the pen matches the fill.
    let pen = CreatePen(PS_SOLID, 1, if primary { fill } else { outline });
    let old_brush = SelectObject(hdc, brush as *mut _);
    let old_pen = SelectObject(hdc, pen as *mut _);
    RoundRect(hdc, l, t, r - 1, b - 1, 20, 20);
    SelectObject(hdc, old_brush);
    SelectObject(hdc, old_pen);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(brush as *mut _);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(pen as *mut _);

    SetBkMode(hdc, TRANSPARENT);
    SetTextColor(hdc, text);
    if let Some(t) = THEME.get() {
        let old_font = SelectObject(hdc, t.font as *mut _);
        // drawn below, then restore
        let txt = if primary {
            "Save & Apply".to_string()
        } else {
            get_text_by_hwnd((*d).hwndItem)
        };
        let len = txt.encode_utf16().count() as i32;
        let chars: Vec<u16> = txt.encode_utf16().collect();
        let mut box_ = rc;
        DrawTextW(
            hdc,
            chars.as_ptr(),
            len,
            &mut box_,
            (DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX) as u32,
        );
        SelectObject(hdc, old_font);
    }
}

unsafe fn get_text_by_hwnd(h: HWND) -> String {
    let mut buf = [0u16; 256];
    let n = GetWindowTextW(h, buf.as_mut_ptr(), 256);
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

const PLAIN: u32 = WS_CHILD | WS_VISIBLE;
const LBL: u32 = PLAIN;
const E: u32 = WS_CHILD | WS_VISIBLE | WS_BORDER | WS_TABSTOP;
const ML: u32 = E | (ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL) as u32;
const BTN: u32 = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW;
/// Clickable caption-style button (Material "text button"): flat lavender text.
const TXTBTN: u32 = WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_OWNERDRAW;

#[allow(non_snake_case)]
unsafe fn build_form(parent: HWND) {
    // ---- Behavior -----------------------------------------------------------
    child(parent, "STATIC", "Behavior", 20, 16, 160, 22, LBL, SEC_BASE + 0, true);
    child(parent, "STATIC", "IPC port", 20, 46, 70, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 20, 64, 100, 26, E, ID_PORT, false);
    child(parent, "STATIC", "Resize step", 150, 46, 90, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 150, 64, 100, 26, E, ID_STEP, false);
    child(parent, "STATIC", "Resize timeout", 280, 46, 120, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 280, 64, 100, 26, E, ID_TIMEOUT, false);
    child(parent, "STATIC", "Tiling interval", 410, 46, 120, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 410, 64, 100, 26, E, ID_TILING_INTERVAL, false);
    child(parent, "STATIC", "ms — daemon port / moves per tick / resize-mode timeout / auto-tile check", 540, 64, 160, 58, PLAIN, 0, false);

    // ---- Exclusions ----------------------------------------------------------
    child(parent, "STATIC", "Never manage — one entry per line", 20, 108, 420, 22, LBL, SEC_BASE + 1, true);
    child(parent, "STATIC", "Titles", 20, 136, 200, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 20, 154, 216, 116, ML, ID_EX_TITLES, false);
    child(parent, "STATIC", "Classes", 252, 136, 200, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 252, 154, 216, 116, ML, ID_EX_CLASSES, false);
    child(parent, "STATIC", "Process ids", 484, 136, 100, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 484, 154, 190, 116, ML, ID_EX_PIDS, false);

    // ---- Borders ---------------------------------------------------------------
    child(parent, "STATIC", "Border overlays", 20, 288, 180, 22, LBL, SEC_BASE + 2, true);
    child(parent, "BUTTON", "Enable", 20, 318, 78, 26, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX, ID_BORDERS_ENABLE, false);
    child(parent, "STATIC", "Thickness", 116, 322, 70, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 190, 318, 50, 26, E, ID_BORDERS_THICKNESS, false);
    child(parent, "STATIC", "Accent (RRGGBB)", 260, 322, 120, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 384, 318, 90, 26, E, ID_BORDERS_ACCENT, false);
    child(parent, "STATIC", "Muted (RRGGBB)", 494, 322, 110, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 608, 318, 66, 26, E, ID_BORDERS_MUTED, false);

    // ---- Hotkeys -----------------------------------------------------------------
    child(parent, "STATIC", "Hotkeys", 20, 364, 120, 22, LBL, SEC_BASE + 3, true);
    child(parent, "STATIC", "one per line:  name|modifiers|vk|action   (vk hex like 0x25; modifiers decimal: 1=alt 2=ctrl 4=shift 8=win)", 20, 392, 660, 16, PLAIN, 0, false);
    child(parent, "EDIT", "", 20, 414, 654, 230, ML, ID_HOTKEYS, false);

    // ---- bottom bar -----------------------------------------------------------
    child(parent, "BUTTON", "", 20, 660, 130, 32, BTN, ID_SAVE_APPLY, false);
    child(parent, "BUTTON", "Save", 162, 660, 90, 32, BTN, ID_SAVE, false);
    child(parent, "BUTTON", "Cancel", 264, 660, 90, 32, BTN, ID_CANCEL, false);
    child(parent, "STATIC", "", 372, 660, 200, 32, PLAIN, ID_STATUS, false); // Save&Apply feedback lives right of the buttons
    child(parent, "BUTTON", "Open Config", 584, 660, 90, 32, BTN, ID_OPEN_CONFIG, false);

    // ---- fill from the real config ------------------------------------------
    let cfg = config::load();
    set_text(parent, ID_PORT, &cfg.ipc_port.to_string());
    set_text(parent, ID_STEP, &cfg.resize_step.to_string());
    set_text(parent, ID_TIMEOUT, &cfg.resize_timeout_ms.to_string());
    set_text(parent, ID_TILING_INTERVAL, &cfg.tiling_interval_ms.to_string());
    set_text(parent, ID_EX_TITLES, &cfg.exclude_titles.join("\r\n"));
    set_text(parent, ID_EX_CLASSES, &cfg.exclude_classes.join("\r\n"));
    set_text(parent, ID_EX_PIDS, &cfg.exclude_pids.iter().map(|p| p.to_string()).collect::<Vec<_>>().join("\r\n"));
    SendMessageW(GetDlgItem(parent, ID_BORDERS_ENABLE), BM_SETCHECK, if cfg.borders.enabled { BST_CHECKED as usize } else { BST_UNCHECKED as usize }, 0);
    set_text(parent, ID_BORDERS_THICKNESS, &cfg.borders.thickness.to_string());
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
    // new + old port (skip duplicate when unchanged)
    let mut ports = vec![cfg.ipc_port];
    if old_port != cfg.ipc_port {
        ports.push(old_port);
    }
    for port in ports {
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
