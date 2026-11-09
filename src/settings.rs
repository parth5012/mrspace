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
    DRAWITEMSTRUCT, ODS_DISABLED, ODS_SELECTED, ODT_BUTTON, ODT_LISTBOX, ODT_STATIC,
};
use windows_sys::Win32::UI::HiDpi::{
    GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetDlgCtrlID, GetDlgItem,
    GetMessageW, GetPropW, GetWindowTextW, IsDialogMessageW, MoveWindow,
    PostQuitMessage, RegisterClassW, RemovePropW, SendMessageW, SetForegroundWindow, SetPropW,
    SetWindowTextW, ShowWindow, TranslateMessage, BM_GETCHECK, BM_SETCHECK, CW_USEDEFAULT,
    ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, MSG, SW_HIDE, SW_RESTORE, SW_SHOW, WM_CLOSE,
    WM_COMMAND, WM_CREATE, WM_CTLCOLORBTN, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC,
    WM_DESTROY, WM_DRAWITEM, WM_ERASEBKGND, WM_PAINT, WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CHILD,
    WS_EX_TOOLWINDOW, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE,
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
// designed list rows (owner-draw listboxes; raw text lives in the hidden edits)
const ID_LB_TITLES: i32 = 161;
const ID_LB_CLASSES: i32 = 162;
const ID_LB_PIDS: i32 = 163;
const ID_LB_HOTKEYS: i32 = 164;
// per-card row actions: ＋ add / ✕ remove
const ID_ADD: i32 = 181; // ..181..184, del 185..188
const ID_DEL: i32 = 185;
// shared in-place overlay edit for a single list row
const ID_OVERLAY: i32 = 190;
const ID_LAST_ROWBTN: i32 = ID_DEL + 3;
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
    hl: bool, // draw hairline separator under the header (wider list-style cards)
}

unsafe fn card(parent: HWND, x: i32, y: i32, w: i32, h: i32, glyph: char, title: &str, desc: &str) {
    make_card(parent, x, y, w, h, glyph, title, desc, false)
}

/// Same card, but with the Untitled-UI hairline divider under the header row.
#[allow(clippy::too_many_arguments)]
unsafe fn card_w(parent: HWND, x: i32, y: i32, w: i32, h: i32, glyph: char, title: &str, desc: &str) {
    make_card(parent, x, y, w, h, glyph, title, desc, true)
}

unsafe fn make_card(
    parent: HWND,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    glyph: char,
    title: &str,
    desc: &str,
    hl: bool,
) {
    let g = wide(&glyph.to_string());
    let data = Box::new(CardData {
        glyph: [g[0], 0],
        title: title.into(),
        desc: desc.into(),
        hl,
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

                // quiet "…" affordance, top-right (decorative)
                let dots: Vec<u16> = "\u{E712}".encode_utf16().collect();
                SetTextColor(hdc, CLR_TEXT_DIM);
                TextOutW(hdc, w - s(30), s(12), dots.as_ptr(), dots.len() as i32);

                // hairline separator under the header (Untitled UI row pattern)
                if d.hl {
                    let op = SelectObject(hdc, pen as *mut _);
                    windows_sys::Win32::Graphics::Gdi::MoveToEx(hdc, s(14), s(54), std::ptr::null_mut());
                    windows_sys::Win32::Graphics::Gdi::LineTo(hdc, w - s(14), s(54));
                    SelectObject(hdc, op);
                }
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
        // in-content page header (Untitled UI pattern): big title + helper line
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(t) = THEME.get() {
                SetBkMode(hdc, TRANSPARENT);
                SelectObject(hdc, t.titlef as *mut _);
                SetTextColor(hdc, CLR_TEXT);
                let title: Vec<u16> = "Settings".encode_utf16().collect();
                TextOutW(hdc, s(22), s(12), title.as_ptr(), title.len() as i32);
                SelectObject(hdc, t.smallf as *mut _);
                SetTextColor(hdc, CLR_TEXT_DIM);
                let helper: Vec<u16> =
                    "Fine-tune layout, exclusions, borders and Win-chords — everything lands in config.json"
                        .encode_utf16()
                        .collect();
                TextOutW(hdc, s(22), s(48), helper.as_ptr(), helper.len() as i32);
            }
            EndPaint(hwnd, &mut ps);
            0
        }
        WM_DRAWITEM => {
            let d = lparam as *const DRAWITEMSTRUCT;
            if (*d).CtlType as u32 == ODT_STATIC {
                draw_pill(d);
            } else if (*d).CtlType as u32 == ODT_LISTBOX {
                let narrow = (*d).CtlID as i32 != ID_LB_HOTKEYS;
                draw_list_row(d, narrow);
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
        // listboxes (and the overlay row editor) sit on cards
        WM_CTLCOLORLISTBOX => {
            let t = THEME.get().unwrap();
            SetBkMode(wparam as *mut _, TRANSPARENT);
            SetTextColor(wparam as *mut _, CLR_TEXT);
            t.card_bg as LRESULT
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
                // ---- designed list rows: add / remove / overlay editing --------
                ID_ADD..=ID_LAST_ROWBTN => {
                    // a pending overlay commit fires first via EN_KILLFOCUS
                    overlay_commit(hwnd);
                    let which = (id - ID_ADD) % 4; // 0..=3
                    let lbid = [
                        ID_LB_TITLES,
                        ID_LB_CLASSES,
                        ID_LB_PIDS,
                        ID_LB_HOTKEYS,
                    ][which as usize];
                    if id < ID_DEL {
                        // ＋: open an empty editor in the append slot
                        let h = GetDlgItem(hwnd, lbid);
                        let prev = SendMessageW(h, LB_GETCURSEL, 0, 0) as i32;
                        let seed = if prev >= 0 {
                            let sl = SendMessageW(h, LB_GETTEXTLEN, prev as _, 0) as i32;
                            let mut buf = vec![0u16; (sl.max(0) as usize) + 1];
                            SendMessageW(h, LB_GETTEXT, prev as _, buf.as_mut_ptr() as LPARAM);
                            String::from_utf16_lossy(&buf[..sl as usize])
                        } else {
                            String::new()
                        };
                        overlay_open(hwnd, lbid, -1, &seed);
                    } else {
                        // ✕: remove the selected row
                        let h = GetDlgItem(hwnd, lbid);
                        let sel = SendMessageW(h, LB_GETCURSEL, 0, 0) as i32;
                        if sel >= 0 {
                            SendMessageW(h, LB_DELETESTRING, sel as _, 0);
                            SendMessageW(h, LB_SETCURSEL, sel.max(0) as usize, 0);
                            sync_hidden(hwnd, lbid);
                        }
                    }
                    0
                }
                ID_OVERLAY => {
                    // overlay edit: commit on kill focus; double-click opens it
                    let ntf = ((wparam >> 16) & 0xffff) as i32;
                    if ntf == EN_KILLFOCUS {
                        overlay_commit(hwnd);
                    }
                    0
                }
                _ => {
                    // listbox notifications arrive by control id: LBN_DBLCLK(2)
                    let ntf = ((wparam >> 16) & 0xffff) as i32;
                    if ntf == LBN_DBLCLK && (ID_LB_TITLES..=ID_LB_HOTKEYS).contains(&id) {
                        let h = GetDlgItem(hwnd, id);
                        let sel = SendMessageW(h, LB_GETCURSEL, 0, 0) as i32;
                        if sel >= 0 {
                            let sl = SendMessageW(h, LB_GETTEXTLEN, sel as _, 0) as i32;
                            let mut buf = vec![0u16; sl as usize + 1];
                            SendMessageW(h, LB_GETTEXT, sel as _, buf.as_mut_ptr() as LPARAM);
                            let text = String::from_utf16_lossy(&buf[..sl as usize]);
                            overlay_open(hwnd, id, sel, &text);
                        }
                        return 0;
                    }
                    0
                }
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

// ---------------------------------------------------------------------------
// Designed list rows (Linear-style): owner-draw listboxes; hotkey rows get
// name + modifier chips + keycap + dim action, exclusion rows plain text.
// Raw text is kept in the hidden sibling EDIT (all save paths unchanged).
// ---------------------------------------------------------------------------
const LBS_NOTIFY: u32 = 0x0001;
const LBS_OWNERDRAWFIXED: u32 = 0x0010;
const LBS_HASSTRINGS: u32 = 0x0040;
const LBS_NOINTEGRALHEIGHT: u32 = 0x0100;
const LBX: u32 = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER
    | LBS_NOTIFY | LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOINTEGRALHEIGHT | 0x0020_0000;
const LB_ADDSTRING: u32 = 0x0180;
const LB_INSERTSTRING: u32 = 0x0181;
const LB_DELETESTRING: u32 = 0x0182;
const LB_RESETCONTENT: u32 = 0x0184;
const LB_SETCURSEL: u32 = 0x0186;
const LB_GETCURSEL: u32 = 0x0188;
const LB_GETTEXT: u32 = 0x0189;
const LB_GETTEXTLEN: u32 = 0x018A;
const LB_GETCOUNT: u32 = 0x018B;
const LB_GETITEMRECT: u32 = 0x0198;
const LB_SETITEMHEIGHT: u32 = 0x01A0;
const LBN_DBLCLK: i32 = 2;
const EN_KILLFOCUS: i32 = 0x0201;
const ODTS_SELECTED: u32 = 1; // ODS_SELECTED

/// Hidden raw-text edits: same ids as before, invisible, save paths unchanged.
const MLH: u32 = WS_CHILD | (ES_MULTILINE | ES_AUTOVSCROLL) as u32;

fn hidden_of(lbid: i32) -> i32 {
    match lbid {
        ID_LB_TITLES => ID_EX_TITLES,
        ID_LB_CLASSES => ID_EX_CLASSES,
        ID_LB_PIDS => ID_EX_PIDS,
        _ => ID_HOTKEYS,
    }
}

fn vk_name(vk: u16) -> String {
    match vk {
        0x25 => "←".into(),
        0x26 => "↑".into(),
        0x27 => "→".into(),
        0x28 => "↓".into(),
        0x09 => "⇥".into(),
        0x20 => "⎵".into(),
        0x21 => "PgUp".into(),
        0x22 => "PgDn".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x70..=0x7B => format!("F{}", vk - 0x70 + 1),
        0x41..=0x5A => (vk as u8 as char).to_string(),
        0x30..=0x39 => (vk as u8 as char).to_string(),
        _ => format!("{vk:#x}"),
    }
}

/// parse "name|mods|vk|action" (anything else → plain single-field row)
fn parse_row(line: &str) -> (String, u16, Option<u16>, String) {
    let p: Vec<&str> = line.splitn(4, '|').collect();
    if p.len() < 4 {
        let mut it = p.into_iter();
        let name = it.next().unwrap_or("").to_string();
        return (name, 0, None, String::new());
    }
    let mods = p[1].trim().parse::<u16>().unwrap_or(0);
    // vk is serialized as {:#x} ("0x25") — accept both hex and decimal
    let vkraw = p[2].trim();
    let vk = if let Some(hex) = vkraw.strip_prefix("0x").or_else(|| vkraw.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        vkraw.parse::<u32>().ok()
    }
    .and_then(|v| u16::try_from(v).ok());
    (p[0].trim().to_string(), mods, vk, p[3].to_string())
}

/// One designed row. Exclusion cards pass narrow=true (plain text row).
unsafe fn draw_list_row(d: *const DRAWITEMSTRUCT, narrow: bool) {
    let hdc = (*d).hDC;
    let rc = (*d).rcItem;
    let w = rc.right - rc.left;
    let h = rc.bottom - rc.top;
    let sel = (*d).itemState & ODTS_SELECTED != 0;
    let hwnd = (*d).hwndItem;
    let idx = (*d).itemID as usize;

    // erase the row first (DC covers the whole list), then tint if selected
    let base = CreateSolidBrush(CLR_CARD);
    let bp = CreatePen(PS_SOLID, 1, CLR_CARD);
    let ob = SelectObject(hdc, base as *mut _);
    let op = SelectObject(hdc, bp as *mut _);
    RoundRect(hdc, rc.left, rc.top, rc.right + 2, rc.bottom + 2, s(10), s(10));
    SelectObject(hdc, ob);
    SelectObject(hdc, op);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(base as *mut _);
    windows_sys::Win32::Graphics::Gdi::DeleteObject(bp as *mut _);

    // selection tint (Linear-style: soft primaryContainer band)
    if sel {
        let b = CreateSolidBrush(CLR_ICON_TILE);
        let p = CreatePen(PS_SOLID, 1, CLR_ICON_TILE);
        let ob = SelectObject(hdc, b as *mut _);
        let op = SelectObject(hdc, p as *mut _);
        RoundRect(hdc, rc.left + 1, rc.top, rc.right, rc.bottom, s(8), s(8));
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        windows_sys::Win32::Graphics::Gdi::DeleteObject(b as *mut _);
        windows_sys::Win32::Graphics::Gdi::DeleteObject(p as *mut _);
    }

    let sl = SendMessageW(hwnd, LB_GETTEXTLEN, idx as _, 0) as i32;
    if sl <= 0 {
        return;
    }
    let mut buf = vec![0u16; sl as usize + 1];
    SendMessageW(hwnd, LB_GETTEXT, idx as _, buf.as_mut_ptr() as LPARAM);
    let line = String::from_utf16_lossy(&buf[..sl as usize]);
    let (name, mods, vk, action) = parse_row(&line);

    let Some(t) = THEME.get() else { return };
    SetBkMode(hdc, TRANSPARENT);
    let mut x = rc.left + s(12);

    if narrow {
        SetTextColor(hdc, CLR_TEXT);
        SelectObject(hdc, t.font as *mut _);
        let chars: Vec<u16> = name.encode_utf16().collect();
        TextOutW(hdc, x, rc.top + (h - s(16)) / 2, chars.as_ptr(), chars.len() as i32);
        return;
    }

    // hotkey row: name · modifier chips · keycap · → action
    SetTextColor(hdc, CLR_TEXT);
    SelectObject(hdc, t.hfont as *mut _);
    let chars: Vec<u16> = name.encode_utf16().collect();
    let mut szn = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
    GetTextExtentPoint32W(hdc, chars.as_ptr(), chars.len() as i32, &mut szn);
    TextOutW(hdc, x, rc.top + (h - s(20)) / 2, chars.as_ptr(), chars.len() as i32);
    x += szn.cx + s(10);

    // chips for set modifier bits (8=win 1=alt 2=ctrl 4=shift)
    let chips: [(&'static str, u16); 4] = [("Win", 8), ("Alt", 1), ("Ctrl", 2), ("Shift", 4)];
    for (label, bit) in chips {
        if mods & bit != 0 {
            let wch: Vec<u16> = label.encode_utf16().collect();
            let mut sz = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
            SelectObject(hdc, t.smallf as *mut _);
            GetTextExtentPoint32W(hdc, wch.as_ptr(), wch.len() as i32, &mut sz);
            x += s(2);
            let cw = sz.cx + s(10);
            let cy = s(16);
            let cy0 = rc.top + (h - cy) / 2;
            let b = CreateSolidBrush(CLR_FIELD);
            let p = CreatePen(PS_SOLID, 1, CLR_CARD_OUT);
            let ob = SelectObject(hdc, b as *mut _);
            let op = SelectObject(hdc, p as *mut _);
            RoundRect(hdc, x, cy0, x + cw, cy0 + cy, s(8), s(8));
            SelectObject(hdc, ob);
            SelectObject(hdc, op);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(b as *mut _);
            windows_sys::Win32::Graphics::Gdi::DeleteObject(p as *mut _);
            SetTextColor(hdc, CLR_PRIMARY_DIM);
            TextOutW(hdc, x + s(5), cy0 + s(1), wch.as_ptr(), wch.len() as i32);
            SetTextColor(hdc, CLR_TEXT);
            x += cw;
        }
    }

    if let Some(vk) = vk {
        // keycap: 20x20 inset box with the key glyph
        x += s(4);
        let kw = s(22);
        let kh = s(18);
        let ky = rc.top + (h - kh) / 2;
        let b = CreateSolidBrush(CLR_FIELD);
        let p = CreatePen(PS_SOLID, 1, CLR_CARD_OUT);
        let ob = SelectObject(hdc, b as *mut _);
        let op = SelectObject(hdc, p as *mut _);
        RoundRect(hdc, x, ky, x + kw, ky + kh, s(4), s(4));
        SelectObject(hdc, ob);
        SelectObject(hdc, op);
        windows_sys::Win32::Graphics::Gdi::DeleteObject(b as *mut _);
        windows_sys::Win32::Graphics::Gdi::DeleteObject(p as *mut _);
        SetTextColor(hdc, CLR_TEXT);
        SelectObject(hdc, t.smallf as *mut _);
        let glyph: Vec<u16> = vk_name(vk).encode_utf16().collect();
        let mut szg = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, glyph.as_ptr(), glyph.len() as i32, &mut szg);
        TextOutW(hdc, x + (kw - szg.cx) / 2, ky + (kh - s(14)) / 2, glyph.as_ptr(), glyph.len() as i32);
        x += kw;
    }

    // action, dim, right-aligned
    if !action.is_empty() {
        SetTextColor(hdc, CLR_TEXT_DIM);
        SelectObject(hdc, t.smallf as *mut _);
        let acts: Vec<u16> = format!("→ {}", action.trim()).encode_utf16().collect();
        let mut sza = windows_sys::Win32::Foundation::SIZE { cx: 0, cy: 0 };
        GetTextExtentPoint32W(hdc, acts.as_ptr(), acts.len() as i32, &mut sza);
        TextOutW(hdc, rc.right - s(12) - sza.cx, rc.top + (h - s(16)) / 2, acts.as_ptr(), acts.len() as i32);
    }
}

/// Command order to keep hidden edits in sync with what the lists show.
unsafe fn sync_hidden(parent: HWND, lbid: i32) {
    let h = GetDlgItem(parent, lbid);
    let n = SendMessageW(h, LB_GETCOUNT, 0, 0) as i32;
    let mut parts: Vec<String> = Vec::new();
    for i in 0..n {
        let sl = SendMessageW(h, LB_GETTEXTLEN, i as _, 0) as i32;
        if sl < 0 {
            continue;
        }
        let mut buf = vec![0u16; sl as usize + 1];
        SendMessageW(h, LB_GETTEXT, i as _, buf.as_mut_ptr() as LPARAM);
        parts.push(String::from_utf16_lossy(&buf[..sl as usize]));
    }
    set_text(parent, hidden_of(lbid), &parts.join("\r\n"));
    if lbid == ID_LB_HOTKEYS {
        set_text(parent, ID_PILL_BINDINGS, &format!("{} bindings", parts.len()));
    }
}

// overlay edit state: (list id, row — row -1 = "append at end")
/// Coords: Gdi-exposed MapWindowPoints works on POINTs; map a whole RECT.
unsafe fn map_rect(hfrom: HWND, hto: HWND, rc: &mut windows_sys::Win32::Foundation::RECT) {
    let mut pts = [
        windows_sys::Win32::Foundation::POINT { x: rc.left, y: rc.top },
        windows_sys::Win32::Foundation::POINT { x: rc.right, y: rc.bottom },
    ];
    windows_sys::Win32::Graphics::Gdi::MapWindowPoints(hfrom, hto, pts.as_mut_ptr(), 2);
    rc.left = pts[0].x;
    rc.top = pts[0].y;
    rc.right = pts[1].x;
    rc.bottom = pts[1].y;
}

static OVERLAY: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

/// open the in-place row editor over `row` (or the append slot when row<0)
unsafe fn overlay_open(parent: HWND, lbid: i32, row: i32, text: &str) {
    let h = GetDlgItem(parent, lbid);
    let n = SendMessageW(h, LB_GETCOUNT, 0, 0) as i32;
    *OVERLAY.lock().unwrap() = Some((lbid, row));
    let he = GetDlgItem(parent, ID_OVERLAY);
    let wt = wide(text);
    SetWindowTextW(he, wt.as_ptr());
    overlay_position(parent, lbid, row, n);
    // select all text
    SendMessageW(he, 0x00B1 /*EM_SETSEL*/, 0, -1);
    ShowWindow(he, SW_SHOW);
    SetForegroundWindow(he);
}

/// place the overlay edit over the target row (parent coordinates)
unsafe fn overlay_position(parent: HWND, lbid: i32, row: i32, n: i32) {
    let he = GetDlgItem(parent, ID_OVERLAY);
    if row >= 0 {
        // over an existing row: item rect mapped into parent coords
        let h = GetDlgItem(parent, lbid);
        let mut rc: windows_sys::Win32::Foundation::RECT = std::mem::zeroed();
        SendMessageW(h, LB_GETITEMRECT, row as _, &mut rc as *mut _ as LPARAM);
        map_rect(h, parent, &mut rc);
        rc.left += s(2);
        rc.right -= s(2);
        rc.top += s(1);
        MoveWindow(he, rc.left, rc.top, rc.right - rc.left, rc.bottom - rc.top, 1);
    } else {
        // append slot: one row-height below the list, same width
        let (lx, ly, lw) = match lbid {
            ID_LB_TITLES => (32, 240, 184),
            ID_LB_CLASSES => (256, 240, 184),
            ID_LB_PIDS => (480, 240, 184),
            _ => (34, 512, 628),
        };
        let rh = if lbid == ID_LB_HOTKEYS { 24 } else { 20 };
        let y = ly + n * rh;
        MoveWindow(he, s(lx + 1), s(y + 1), s(lw), s(rh), 1);
    }
}

/// commit the overlay row editor (WM_COMMAND from EN_KILLFOCUS)
unsafe fn overlay_commit(parent: HWND) {
    let Some(state) = OVERLAY.lock().unwrap().take() else { return };
    let (lbid, row) = state;
    let text = get_text(parent, ID_OVERLAY).trim().to_string();
    let he = GetDlgItem(parent, ID_OVERLAY);
    ShowWindow(he, SW_HIDE);
    let h = GetDlgItem(parent, lbid);
    if row < 0 {
        if !text.is_empty() {
            SendMessageW(h, LB_ADDSTRING, 0, wide(&text).as_ptr() as LPARAM);
        }
    } else if row >= 0 {
        if text.is_empty() {
            // editing a row to empty = delete it
            SendMessageW(h, LB_DELETESTRING, row as _, 0);
        } else {
            // LBS_HASSTRINGS has no in-place replace → remove old, insert new
            SendMessageW(h, LB_INSERTSTRING, row as _, wide(&text).as_ptr() as LPARAM);
            SendMessageW(h, LB_DELETESTRING, (row + 1) as _, 0);
        }
    }
    sync_hidden(parent, lbid);
}

/// M3-style rounded buttons drawn by hand: primary = filled lavender for/// "Save & Apply", others = outlined tonal buttons.
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

/// owner-draw listbox with a fixed designed row height (px, logical)
unsafe fn listbox(parent: HWND, x: i32, y: i32, w: i32, h: i32, id: i32, rowh: i32) {
    child(parent, "LISTBOX", "", x, y, w, h, LBX, id, false);
    SendMessageW(GetDlgItem(parent, id), LB_SETITEMHEIGHT, 0, s(rowh) as LPARAM);
}

/// tiny ＋/✕ owner-draw glyph button on a card header
unsafe fn mini(parent: HWND, x: i32, y: i32, w: i32, h: i32, glyph: &'static str, id: i32) {
    child(parent, "BUTTON", glyph, x, y, w, h, TXTBTN, id, false);
}

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
    card(parent, 20, 76, 660, 96, '\u{E713}', "Basics", "IPC port · resize step · resize timeout · tiling interval (ms)");
    child(parent, "STATIC", "IPC port", 34, 124, 70, 16, LBL, LBLCARD_BASE + 0, false);
    child(parent, "EDIT", "", 34, 142, 96, 26, E, ID_PORT, false);
    child(parent, "STATIC", "Resize step", 146, 124, 90, 16, LBL, LBLCARD_BASE + 1, false);
    child(parent, "EDIT", "", 146, 142, 96, 26, E, ID_STEP, false);
    child(parent, "STATIC", "Resize timeout", 258, 124, 120, 16, LBL, LBLCARD_BASE + 2, false);
    child(parent, "EDIT", "", 258, 142, 96, 26, E, ID_TIMEOUT, false);
    child(parent, "STATIC", "Tiling interval", 370, 124, 120, 16, LBL, LBLCARD_BASE + 3, false);
    child(parent, "EDIT", "", 370, 142, 96, 26, E, ID_TILING_INTERVAL, false);

    // ---- Exclusions — three dedicated cards ------------------------------------
    card(parent, 20, 188, 212, 150, '\u{E8A5}', "Window titles", "exclude by title substring");
    child(parent, "EDIT", "", 32, 240, 188, 84, MLH, ID_EX_TITLES, false);
    listbox(parent, 32, 240, 188, 84, ID_LB_TITLES, 20);
    mini(parent, 232 - 52, 200, 22, 20, "＋", ID_ADD); // ＋ / ✕ in the card header
    mini(parent, 232 - 26, 200, 22, 20, "✕", ID_DEL);
    card(parent, 244, 188, 212, 150, '\u{E943}', "Classes", "exclude by Win32 class");
    child(parent, "EDIT", "", 256, 240, 188, 84, MLH, ID_EX_CLASSES, false);
    listbox(parent, 256, 240, 188, 84, ID_LB_CLASSES, 20);
    mini(parent, 456 - 52, 200, 22, 20, "＋", ID_ADD + 1);
    mini(parent, 456 - 26, 200, 22, 20, "✕", ID_DEL + 1);
    card(parent, 468, 188, 212, 150, '\u{E946}', "Process IDs", "exclude by image pid");
    child(parent, "EDIT", "", 480, 240, 188, 84, MLH, ID_EX_PIDS, false);
    listbox(parent, 480, 240, 188, 84, ID_LB_PIDS, 20);
    mini(parent, 680 - 52, 200, 22, 20, "＋", ID_ADD + 2);
    mini(parent, 680 - 26, 200, 22, 20, "✕", ID_DEL + 2);

    // ---- Border overlays (card) ------------------------------------------------
    card(parent, 20, 354, 660, 84, '\u{E7B3}', "Border overlays", "ring drawn around managed windows");
    child(parent, "BUTTON", "Enable", 32, 404, 74, 24, WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_AUTOCHECKBOX, ID_BORDERS_ENABLE, false);
    child(parent, "STATIC", "Thickness", 122, 408, 70, 16, LBL, LBLCARD_BASE + 4, false);
    child(parent, "EDIT", "", 204, 404, 46, 24, E, ID_BORDERS_THICKNESS, false);
    child(parent, "STATIC", "Accent", 268, 408, 48, 16, LBL, LBLCARD_BASE + 5, false);
    child(parent, "EDIT", "", 322, 404, 76, 24, E, ID_BORDERS_ACCENT, false);
    child(parent, "STATIC", "Muted", 416, 408, 48, 16, LBL, LBLCARD_BASE + 6, false);
    child(parent, "EDIT", "", 470, 404, 76, 24, E, ID_BORDERS_MUTED, false);

    // ---- Hotkeys (card + bindings pill) ----------------------------------------
    card_w(parent, 20, 454, 660, 206, '\u{E70F}', "Hotkeys", "one per line ·  name | modifiers | vk | action");
    child(
        parent, "STATIC", "",
        560, 464, 110, 26,
        WS_CHILD | WS_VISIBLE | SS_OWNERDRAW, ID_PILL_BINDINGS, false,
    );
    set_text(parent, ID_PILL_BINDINGS, &format!("{} bindings", cfg.hotkeys.len()));
    child(parent, "EDIT", "", 32, 528, 636, 120, MLH, ID_HOTKEYS, false);
    listbox(parent, 34, 512, 628, 100, ID_LB_HOTKEYS, 24);
    child(parent, "BUTTON", "＋ Add", 506, 620, 72, 24, TXTBTN, ID_ADD + 3, false);
    child(parent, "BUTTON", "✕ Remove", 584, 620, 86, 24, TXTBTN, ID_DEL + 3, false);

    // shared in-place row editor (hidden until a row is double-clicked)
    child(parent, "EDIT", "", -2000, -2000, 10, 10, WS_CHILD | WS_BORDER | WS_TABSTOP, ID_OVERLAY, false);

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

    // seed the designed lists from the same raw text
    let seed = |id_raw: i32, id_lb: i32| {
        for line in get_text(parent, id_raw).lines() {
            SendMessageW(GetDlgItem(parent, id_lb), LB_ADDSTRING, 0, wide(line).as_ptr() as LPARAM);
        }
    };
    seed(ID_EX_TITLES, ID_LB_TITLES);
    seed(ID_EX_CLASSES, ID_LB_CLASSES);
    seed(ID_EX_PIDS, ID_LB_PIDS);
    seed(ID_HOTKEYS, ID_LB_HOTKEYS);
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
