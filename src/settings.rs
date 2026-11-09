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
    BeginPaint, CreateFontW, CreatePen, CreateSolidBrush, DrawTextW, EndPaint,
    GetTextExtentPoint32W, RoundRect, SelectObject, SetBkMode, SetTextColor, TextOutW, DT_CENTER,
    DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, PAINTSTRUCT,
};
use windows_sys::Win32::UI::Controls::{
    DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, ODT_BUTTON, ODT_STATIC,
};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetDlgCtrlID, GetDlgItem,
    GetMessageW, GetPropW, GetWindowTextW, IsDialogMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, SetForegroundWindow, SetPropW, SetWindowTextW, ShowWindow,
    RemovePropW, TranslateMessage, BM_GETCHECK, BM_SETCHECK, CW_USEDEFAULT, ES_AUTOHSCROLL, ES_AUTOVSCROLL,
    ES_MULTILINE, MSG, SW_RESTORE, SW_SHOW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN,
    WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_PAINT,
    WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CHILD, WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, WS_TABSTOP,
    WS_VISIBLE,
};

// constants that live in other feature modules; inlined to keep the feature set small
const BST_CHECKED: isize = 1;
const BST_UNCHECKED: isize = 0;
const BS_PUSHBUTTON: u32 = 0x10;
const BS_AUTOCHECKBOX: u32 = 0x3;
const BS_OWNERDRAW: u32 = 0xB;
const SS_OWNERDRAW: u32 = 13;
const PS_SOLID: i32 = 0;
const TRANSPARENT: i32 = 1;
// COLORREF-style 0x00BBGGRR from (r,g,b)
const fn c3(r: u32, g: u32, b: u32) -> u32 {
    (b << 16) | (g << 8) | r
}

// M3-dark palette
const CLR_BG: u32 = c3(0x16, 0x14, 0x1C); // surface
const CLR_CARD: u32 = c3(0x20, 0x1E, 0x28); // surfaceContainerLow (card blocks)
const CLR_FIELD: u32 = c3(0x28, 0x26, 0x32); // input field inside a card
const CLR_TEXT: u32 = c3(0xE6, 0xE0, 0xEA); // onSurface
const CLR_TEXT_DIM: u32 = c3(0x9A, 0x93, 0xA6); // onSurfaceVariant (was too light)
const CLR_PRIMARY: u32 = c3(0xD0, 0xBC, 0xFF); // primary
const CLR_PRIMARY_DIM: u32 = c3(0xA9, 0x8F, 0xE0);
const CLR_ON_PRIMARY: u32 = c3(0x2B, 0x1A, 0x4F);
const CLR_ICON_TILE: u32 = c3(0x4A, 0x44, 0x58); // primaryContainer (icon chip)
const CLR_OUTLINE: u32 = c3(0x93, 0x8F, 0x99);
const CLR_CARD_OUT: u32 = c3(0x39, 0x36, 0x44); // outlineVariant — subtle card border
const CLR_BORDER_ACCENT: u32 = c3(0x7C, 0x4D, 0xFF); // live accent from config.json

const CLASS_NAME: &str = "MrspaceSettings";
const CARD_CLASS: &str = "MrspaceCard";

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
// pills (SS_OWNERDRAW statics)
const ID_PILL_DAEMON: i32 = 141;
const ID_PILL_BINDINGS: i32 = 142;
// labels painted on a card (WM_CTLCOLORSTATIC paints them with card bg)
const LBLCARD_BASE: i32 = 500;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// ---------------------------------------------------------------------------
// Card blocks — designed panels behind each settings section (Linear/M3 style:
// icon chip, section title, one-line helper description, rounded elevation).
// ---------------------------------------------------------------------------
struct CardData {
    glyph: [u16; 2], // MDL2 char + NUL
    title: String,
    desc: String,
}

unsafe fn card(parent: HWND, x: i32, y: i32, w: i32, h: i32, glyph: char, title: &str, desc: &str) {
    let g = wide(&glyph.to_string());
    let data = Box::new(CardData {
        glyph: [g[0], 0],
        title: title.into(),
        desc: desc.into(),
    });
    let cls = wide(CARD_CLASS);
    let h2 = CreateWindowExW(
        0,
        cls.as_ptr(),
        std::ptr::null(),
        WS_CHILD | WS_VISIBLE,
        s(x),
        s(y),
        s(w),
        s(h),
        parent,
        0 as _,
        0 as _,
        std::ptr::null(),
    );
    SetPropW(h2, wide("mrspace-card").as_ptr(), Box::into_raw(data) as *mut _);
}

unsafe extern "system" fn card_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_DESTROY {
        let p = GetPropW(hwnd, wide("mrspace-card").as_ptr());
        if !p.is_null() {
            RemovePropW(hwnd, wide("mrspace-card").as_ptr());
            drop(Box::from_raw(p as *mut CardData));
        }
        return 0;
    }
    if msg == WM_PAINT {
        let p = GetPropW(hwnd, wide("mrspace-card").as_ptr());
        let mut ps: PAINTSTRUCT = std::mem::zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        if let Some(t) = THEME.get() {
            let mut rc2 = std::mem::zeroed();
            windows_sys::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rc2);
            let w = rc2.right;
            let h = rc2.bottom;

            // body: rounded elevated surface
            let brush = CreateSolidBrush(CLR_CARD);
            let pen = CreatePen(PS_SOLID, 1, CLR_CARD_OUT);
            let ob = SelectObject(hdc, brush as *mut _);
            let op = SelectObject(hdc, pen as *mut _);
            RoundRect(hdc, 0, 0, w, h, s(24), s(24));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(brush as *mut _);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(pen as *mut _);

            if !p.is_null() {
                let d = &*(p as *const CardData);
                // icon chip: primary-container rounded square with the glyph
                let tile = s(30);
                let ib = CreateSolidBrush(CLR_ICON_TILE);
                let ip = CreatePen(PS_SOLID, 1, CLR_ICON_TILE);
                let ob = SelectObject(hdc, ib as *mut _);
                let op = SelectObject(hdc, ip as *mut _);
                RoundRect(hdc, s(14), s(12), s(14) + tile, s(12) + tile, s(18), s(18));
                SelectObject(hdc, ob);
                SelectObject(hdc, op);
                windows_sys::Win32::Graphics::Gdi::DeleteObject(ib as *mut _);
                windows_sys::Win32::Graphics::Gdi::DeleteObject(ip as *mut _);
                SetBkMode(hdc, TRANSPARENT);
                SetTextColor(hdc, CLR_PRIMARY);
                SelectObject(hdc, t.iconf as *mut _);
                TextOutW(hdc, s(14) + tile / 2 - s(7), s(12) + tile / 2 - s(8), d.glyph.as_ptr(), 1);
                // title + helper description
                SetTextColor(hdc, CLR_TEXT);
                SelectObject(hdc, t.hfont as *mut _);
                let title: Vec<u16> = d.title.encode_utf16().collect();
                TextOutW(hdc, s(54), s(14), title.as_ptr(), title.len() as i32);
                SetTextColor(hdc, CLR_TEXT_DIM);
                SelectObject(hdc, t.smallf as *mut _);
                let desc: Vec<u16> = d.desc.encode_utf16().collect();
                TextOutW(hdc, s(54), s(36), desc.as_ptr(), desc.len() as i32);
            }
        }
        EndPaint(hwnd, &mut ps);
        return 0;
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}

/// Theme handles (brushes + fonts), created once before children exist.
struct Theme {
    font: isize,     // body font
    hfont: isize,    // header font (semibold)
    smallf: isize,   // small helper/font
    titlef: isize,   // app title
    iconf: isize,    // Segoe MDL2 Assets glyph font
    win_bg: isize,   // brush
    field_bg: isize, // brush
    card_bg: isize,  // brush
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
    let fam = wide("Segoe UI");
    let mdl = wide("Segoe MDL2 Assets");
    let font = CreateFontW(
        s(18), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0x22, fam.as_ptr(),
    ) as isize;
    let hfont = CreateFontW(
        s(20), 0, 0, 0, 600, 0, 0, 0, 0, 0, 0, 0, 0x22, fam.as_ptr(),
    ) as isize;
    let smallf = CreateFontW(
        s(14), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0x22, fam.as_ptr(),
    ) as isize;
    let titlef = CreateFontW(
        s(26), 0, 0, 0, 700, 0, 0, 0, 0, 0, 0, 0, 0x22, fam.as_ptr(),
    ) as isize;
    let iconf = CreateFontW(
        s(15), 0, 0, 0, 400, 0, 0, 0, 0, 0, 0, 0, 0x22, mdl.as_ptr(),
    ) as isize;
    Theme {
        font,
        hfont,
        smallf,
        titlef,
        iconf,
        win_bg: CreateSolidBrush(CLR_BG) as isize,
        field_bg: CreateSolidBrush(CLR_FIELD) as isize,
        card_bg: CreateSolidBrush(CLR_CARD) as isize,
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

        // decorative card blocks that hold each section's controls
        let card_name = wide(CARD_CLASS);
        let mut cc: WNDCLASSW = std::mem::zeroed();
        cc.lpfnWndProc = Some(card_proc);
        cc.lpszClassName = card_name.as_ptr();
        cc.hbrBackground = theme.card_bg as _;
        RegisterClassW(&cc);

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
            let d = lparam as *const DRAWITEMSTRUCT;
            if (*d).CtlType as u32 == ODT_STATIC {
                draw_pill(d);
            } else {
                draw_button(d);
            }
            1
        }
        // system asks which brush to paint behind our children
        WM_CTLCOLOREDIT => {
            let t = THEME.get().unwrap();
            SetBkMode(wparam as *mut _, TRANSPARENT);
            SetTextColor(wparam as *mut _, CLR_TEXT);
            t.field_bg as LRESULT
        }
        WM_CTLCOLORBTN => {
            // only the native checkboxes; draw them on the card surface
            let t = THEME.get().unwrap();
            SetBkMode(wparam as *mut _, TRANSPARENT);
            SetTextColor(wparam as *mut _, CLR_TEXT);
            t.card_bg as LRESULT
        }
        WM_CTLCOLORSTATIC => {
            let t = THEME.get().unwrap();
            let id = GetDlgCtrlID(lparam as HWND);
            SetBkMode(wparam as *mut _, TRANSPARENT);
            let (color, brush) = if (SEC_BASE..SEC_BASE + 16).contains(&id) {
                (CLR_PRIMARY, t.win_bg) // accent section headers on the canvas
            } else if (LBLCARD_BASE..LBLCARD_BASE + 64).contains(&id) {
                (CLR_PRIMARY_DIM, t.card_bg) // field labels inside cards
            } else {
                (CLR_TEXT_DIM, t.win_bg)
            };
            SetTextColor(wparam as *mut _, color);
            brush as LRESULT
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

/// Pill chips (SS_OWNERDRAW statics): rounded chip with optional status dot —
/// used for "daemon running/offline" and "{n} bindings".
unsafe fn draw_pill(d: *const DRAWITEMSTRUCT) {
    if (*d).CtlType as u32 != ODT_STATIC {
        return;
    }
    let hdc = (*d).hDC;
    let rc = (*d).rcItem;
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    let id = (*d).CtlID as i32;

    let brush = CreateSolidBrush(CLR_CARD);
    let pen = CreatePen(PS_SOLID, 1, CLR_CARD_OUT);
    let ob = SelectObject(hdc, brush as *mut _);
    let op = SelectObject(hdc, pen as *mut _);
    RoundRect(hdc, 0, 0, w, h, s(16), s(16));
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(brush as *mut _);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(pen as *mut _);

    if let Some(t) = THEME.get() {
        SetBkMode(hdc, TRANSPARENT);
        SelectObject(hdc, t.smallf as *mut _);
        let text = get_text_by_hwnd((*d).hwndItem);
        let chars: Vec<u16> = text.encode_utf16().collect();
        let dot = text.starts_with('●');
        let (dx, body) = if dot {
            (s(12), text.chars().skip(1).collect::<String>()) // skip the dot char, drawn colored
        } else {
            (0, text.clone())
        };
        let mut sz = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
        let b2: Vec<u16> = body.trim_start().encode_utf16().collect();
        GetTextExtentPoint32W(hdc, b2.as_ptr(), b2.len() as i32, &mut sz);
        let mut tx = (w - sz.cx) / 2;
        if dot {
            tx += s(8);
        }
        if tx < s(10) + dx {
            tx = s(10) + dx;
        }
        if dot {
            SetTextColor(hdc, if id == ID_PILL_DAEMON { CLR_PRIMARY } else { CLR_PRIMARY_DIM });
            TextOutW(hdc, tx - s(9), (h - s(18)) / 2, chars.as_ptr(), 1);
        }
        SetTextColor(hdc, if id == ID_PILL_DAEMON { CLR_TEXT_DIM } else { CLR_PRIMARY_DIM });
        TextOutW(hdc, tx, (h - s(18)) / 2, b2.as_ptr(), b2.len() as i32);
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
    let cfg = config::load();

    // ---- header (painted in WM_PAINT) + daemon status pill --------------------
    let daemon_up = crate::daemon::try_send_command(cfg.ipc_port, "current-desktop")
        .map(|r| !r.contains("ERR"))
        .unwrap_or(false);
    child(
        parent, "STATIC", "",
        520, 14, 168, 30,
        WS_CHILD | WS_VISIBLE | SS_OWNERDRAW, ID_PILL_DAEMON, false,
    );
    set_text(parent, ID_PILL_DAEMON, &format!("●{}", if daemon_up { " daemon running" } else { " daemon offline" }));

    // ---- Basics — port / steps / timeouts (card) ------------------------------
    card(parent, 20, 58, 660, 96, '\u{E713}', "Basics", "IPC port · resize step · resize timeout · tiling interval (ms)");
    child(parent, "STATIC", "IPC port", 34, 106, 70, 16, LBL, LBLCARD_BASE + 0, false);
    child(parent, "EDIT", "", 34, 124, 96, 26, E, ID_PORT, false);
    child(parent, "STATIC", "Resize step", 146, 106, 90, 16, LBL, LBLCARD_BASE + 1, false);
    child(parent, "EDIT", "", 146, 124, 96, 26, E, ID_STEP, false);
    child(parent, "STATIC", "Resize timeout", 258, 106, 120, 16, LBL, LBLCARD_BASE + 2, false);
    child(parent, "EDIT", "", 258, 124, 96, 26, E, ID_TIMEOUT, false);
    child(parent, "STATIC", "Tiling interval", 370, 106, 120, 16, LBL, LBLCARD_BASE + 3, false);
    child(parent, "EDIT", "", 370, 124, 96, 26, E, ID_TILING_INTERVAL, false);

    // ---- Exclusions — three dedicated cards ------------------------------------
    card(parent, 20, 170, 212, 150, '\u{E8A5}', "Window titles", "exclude by title substring");
    child(parent, "EDIT", "", 32, 222, 188, 84, ML, ID_EX_TITLES, false);
    card(parent, 244, 170, 212, 150, '\u{E943}', "Window classes", "exclude by Win32 class");
    child(parent, "EDIT", "", 256, 222, 188, 84, ML, ID_EX_CLASSES, false);
    card(parent, 468, 170, 212, 150, '\u{E946}', "Process IDs", "exclude by image pid");
    child(parent, "EDIT", "", 480, 222, 188, 84, ML, ID_EX_PIDS, false);

    // ---- Border overlays (card) ------------------------------------------------
    card(parent, 20, 336, 660, 84, '\u{E7B3}', "Border overlays", "ring drawn around managed windows");
    child(parent, "BUTTON", "Enable", 32, 386, 74, 24, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX, ID_BORDERS_ENABLE, false);
    child(parent, "STATIC", "Thickness", 122, 390, 70, 16, LBL, LBLCARD_BASE + 4, false);
    child(parent, "EDIT", "", 204, 386, 46, 24, E, ID_BORDERS_THICKNESS, false);
    child(parent, "STATIC", "Accent", 268, 390, 48, 16, LBL, LBLCARD_BASE + 5, false);
    child(parent, "EDIT", "", 322, 386, 76, 24, E, ID_BORDERS_ACCENT, false);
    child(parent, "STATIC", "Muted", 416, 390, 48, 16, LBL, LBLCARD_BASE + 6, false);
    child(parent, "EDIT", "", 470, 386, 76, 24, E, ID_BORDERS_MUTED, false);

    // ---- Hotkeys (card + bindings pill) ----------------------------------------
    card(parent, 20, 436, 660, 232, '\u{E70F}', "Hotkeys", "one per line ·  name | modifiers | vk | action");
    child(
        parent, "STATIC", "",
        560, 446, 110, 26,
        WS_CHILD | WS_VISIBLE | SS_OWNERDRAW, ID_PILL_BINDINGS, false,
    );
    set_text(parent, ID_PILL_BINDINGS, &format!("{} bindings", cfg.hotkeys.len()));
    child(parent, "EDIT", "", 32, 494, 636, 158, ML, ID_HOTKEYS, false);

    // ---- bottom bar -----------------------------------------------------------
    child(parent, "BUTTON", "", 20, 678, 128, 30, BTN, ID_SAVE_APPLY, false);
    child(parent, "BUTTON", "Save", 156, 678, 84, 30, BTN, ID_SAVE, false);
    child(parent, "BUTTON", "Cancel", 248, 678, 84, 30, BTN, ID_CANCEL, false);
    child(parent, "STATIC", "", 344, 678, 220, 30, PLAIN, ID_STATUS, false); // feedback sits right of the buttons
    child(parent, "BUTTON", "Open Config", 578, 678, 102, 30, BTN, ID_OPEN_CONFIG, false);
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
