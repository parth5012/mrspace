// Border overlays for laid-out windows: a muted ring around every window that
// mrspace arranged (pins / grid / presets), and an accent ring around the
// currently focused one. Implemented as click-through, layered, topmost popup
// windows drawn with per-pixel alpha (UpdateLayeredWindow). A watchdog thread
// keeps them glued to their targets, recolors on focus change, and removes
// overlays when a target disappears or is minimized.
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{LazyLock, Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, COLORREF, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject,
    BLENDFUNCTION, BITMAPINFO, BITMAPINFOHEADER, DIB_RGB_COLORS, AC_SRC_ALPHA, AC_SRC_OVER,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, GetForegroundWindow, GetMessageW, GetWindowRect, IsIconic,
    IsWindow, IsWindowVisible, IsZoomed, RegisterClassExW, SetWindowPos, ShowWindow,
    TranslateMessage, DispatchMessageW, UpdateLayeredWindow, MSG, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOWNA, ULW_ALPHA, WM_PAINT, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP, WS_VISIBLE,
    WNDCLASSEXW, HWND_TOPMOST,
};

/// Runtime-configurable options (loaded from config.json's `borders` block).
pub struct BorderOptions {
    pub enabled: bool,
    pub thickness: i32,
    pub accent: u32, // COLORREF-style 0x00BBGGRR
    pub muted: u32,
}

static OPTS: Mutex<BorderOptions> = Mutex::new(BorderOptions {
    enabled: true,
    thickness: 3,
    accent: (0xFF << 16) | (0x4D << 8) | 0x7C, // purple 7C4DFF
    muted: (0x5E << 16) | (0x55 << 8) | 0x55,  // grey 55555E
});

fn opts() -> std::sync::MutexGuard<'static, BorderOptions> {
    OPTS.lock().unwrap()
}

/// Accent color for other modules drawing overlays (layout preview ghosts).
pub(crate) fn accent_color() -> u32 { OPTS.lock().unwrap().accent }

/// Parse "RRGGBB" hex into COLORREF-style 0x00BBGGRR (tolerant: '#' allowed).
pub fn parse_color(s: &str, fallback: u32) -> u32 {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return fallback;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok().unwrap_or(0) as u32;
    let g = u8::from_str_radix(&s[2..4], 16).ok().unwrap_or(0) as u32;
    let b = u8::from_str_radix(&s[4..6], 16).ok().unwrap_or(0) as u32;
    (b << 16) | (g << 8) | r
}

/// Apply settings from config.json. Disabling removes any existing rings.
pub fn configure(b: &crate::config::BorderSettings) {
    let mut g = OPTS.lock().unwrap();
    g.enabled = b.enabled;
    g.thickness = b.thickness.clamp(1, 20);
    g.accent = parse_color(&b.accent, g.accent);
    g.muted = parse_color(&b.muted, g.muted);
    if !b.enabled {
        hide_all();
    }
}

const CLASS_NAME: &[u16] = &[
    b'm' as u16, b'r' as u16, b's' as u16, b'p' as u16, b'a' as u16, b'c' as u16, b'e' as u16,
    b'.' as u16, b'b' as u16, b'o' as u16, b'r' as u16, b'd' as u16, b'e' as u16, b'r' as u16, 0,
];

struct Entry {
    overlay: isize,   // our layered window's hwnd
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    color: u32,
    hidden: bool,
}

static PENDING: LazyLock<Mutex<Vec<isize>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static ENTRIES: LazyLock<Mutex<HashMap<isize, Entry>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static RUNNING: AtomicBool = AtomicBool::new(false);
static CLASS_READY: OnceLock<()> = OnceLock::new();

/// Mark a window as mrspace-managed so it gets a border ring (call after it
/// has been pinned / tiled / preset-applied).
pub fn note_window(hwnd: HWND) {
    if hwnd.is_null() {
        return;
    }
    PENDING.lock().unwrap().push(hwnd as isize);
}

/// Re-create the border set from the current list of manageable windows
/// (used by fit-all / cycle / preset application to keep the ring set fresh).
pub fn sync_all_managed() {
    for h in crate::win32::enum_windows() {
        if crate::win32::is_manageable(h) {
            PENDING.lock().unwrap().push(h as isize);
        }
    }
}

/// Remove all managed windows from the ring set (e.g. tiling turned off while
/// un-tiled windows shouldn't be highlighted anymore). Windows currently in
/// the set stop being refreshed and their overlays dissolve on the next tick.
pub fn clear_pending() {
    PENDING.lock().unwrap().clear();
}

/// Start the watchdog thread (safe to call once; additional calls no-op).
pub fn start() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    std::thread::spawn(|| {
        ensure_class();
        loop {
            if !opts().enabled {
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe { tick() })).ok();
        }
    });
}

#[allow(static_mut_refs)]
fn ensure_class() {
    if CLASS_READY.get().is_some() {
        return;
    }
    unsafe {
        let hinstance = windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(
            std::ptr::null(),
        );
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0,
            lpfnWndProc: Some(overlay_wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance as isize as *mut _,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: CLASS_NAME.as_ptr(),
            hIconSm: std::ptr::null_mut(),
        };
        RegisterClassExW(&wc);
    }
    std::thread::spawn(|| unsafe {
        // message pump for the overlay windows (created on this thread)
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if GetMessageW(&mut msg, 0 as _, 0, 0) <= 0 {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

unsafe extern "system" fn overlay_wndproc(
    hwnd: HWND,
    msg: u32,
    w: WPARAM,
    l: LPARAM,
) -> LRESULT {
    if msg == WM_PAINT {
        // layered windows don't need WM_PAINT handling
        return 0;
    }
    DefWindowProcW(hwnd, msg, w, l)
}

unsafe fn tick() {
    // adopt newly-annotated windows
    let mut pending = PENDING.lock().unwrap().drain(..).collect::<Vec<_>>();
    pending.dedup();
    // (pending entries are additive; dead ones get swept below)

    let fg = {
        let f = GetForegroundWindow();
        f as isize
    };

    let mut entries = ENTRIES.lock().unwrap();
    for tgt in pending {
        if !entries.contains_key(&tgt) {
            // create the overlay lazily on first tick that sees the target
            entries.insert(
                tgt,
                Entry { overlay: 0, x: 0, y: 0, w: 0, h: 0, color: 0, hidden: true },
            );
        }
    }
    let _tg = trace_tick_start(entries.len());
    if let Some(ref tg) = _tg {
        for (tgt, e) in entries.iter() {
            trace_entry(tg, *tgt, e);
        }
    }

    let mut to_remove: Vec<isize> = Vec::new();
    for (tgt, e) in entries.iter_mut() {
        let th = (*tgt) as _;
        if IsWindow(th) == 0 || IsWindowVisible(th) == 0 || IsIconic(th) != 0 {
            if e.overlay != 0 {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(e.overlay as _);
            }
            to_remove.push(*tgt);
            continue;
        }
        let mut r = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        if GetWindowRect(th, &mut r) == 0 {
            to_remove.push(*tgt);
            continue;
        }
        let w = r.right - r.left;
        let h = r.bottom - r.top;
        // no ring on maximized / fullscreen tiles: GetWindowRect extends past
        // or matches the screen there, so the ring either paints offscreen
        // (corner fragments) or just mirrors the screen edge
        let maximized = IsZoomed(th) != 0;
        let fullscreen = {
            let wa = crate::win32::primary_work_area();
            w >= wa.right - wa.left && h >= wa.bottom - wa.top
        };
        if maximized || fullscreen {
            if e.overlay != 0 && !e.hidden {
                windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                    e.overlay as HWND, SW_HIDE,
                );
                e.hidden = true;
            }
            continue;
        }
        if e.overlay != 0 && e.hidden {
            windows_sys::Win32::UI::WindowsAndMessaging::ShowWindow(
                e.overlay as HWND, SW_SHOWNA,
            );
            e.hidden = false;
            // force a redraw+reposition below (rect will not have changed
            // if the window was maximized and is restored to the same size)
            e.w = 0;
        }
        let th = opts().thickness;
        let min = th.clamp(1, 20) * 2;
        if w < min || h < min {
            to_remove.push(*tgt);
            continue;
        }
        let color = if fg == *tgt { opts().accent } else { opts().muted };
        if e.overlay == 0 {
            e.overlay = create_overlay(w, h, r.left, r.top, color);
            if e.overlay != 0 {
                e.x = r.left;
                e.y = r.top;
                e.w = w;
                e.h = h;
                e.color = color;
                e.hidden = false;
            }
        } else if e.w != w || e.h != h || e.color != color {
            // size/color change: full redraw (UpdateLayeredWindow also moves)
            redraw_overlay(e.overlay as HWND, w, h, r.left, r.top, color, 0);
            e.x = r.left;
            e.y = r.top;
            e.w = w;
            e.h = h;
            e.color = color;
        } else if e.x != r.left || e.y != r.top {
            // move-only: don't leave the ring where the window used to be
            SetWindowPos(
                e.overlay as HWND,
                HWND_TOPMOST,
                r.left,
                r.top,
                0,
                0,
                SWP_NOSIZE | SWP_NOACTIVATE,
            );
            e.x = r.left;
            e.y = r.top;
        }
    }
    for t in to_remove {
        entries.remove(&t);
    }
}

unsafe fn create_overlay(w: i32, h: i32, x: i32, y: i32, color: u32) -> isize {
    create_overlay_ex(w, h, x, y, color, 0)
}

/// Same layered click-through window, but with a translucent interior fill
/// (0 = transparent interior, like the border rings). Used by the layout
/// preview: ghosts need to be seen as *regions*, not just outlines.
pub(crate) unsafe fn create_ghost(w: i32, h: i32, x: i32, y: i32, color: u32, fill_alpha: u8) -> isize {
    create_overlay_ex(w, h, x, y, color, fill_alpha as u32)
}

unsafe fn create_overlay_ex(w: i32, h: i32, x: i32, y: i32, color: u32, fill_alpha: u32) -> isize {
    ensure_class();
    let hwnd = CreateWindowExW(
        WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
        CLASS_NAME.as_ptr(),
        std::ptr::null(),
        WS_POPUP | WS_VISIBLE,
        0,
        0,
        0,
        0,
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        windows_sys::Win32::System::LibraryLoader::GetModuleHandleW(std::ptr::null()) as isize
            as *mut _,
        std::ptr::null(),
    );
    if hwnd.is_null() {
        return 0;
    }
    SetWindowPos(
        hwnd, windows_sys::Win32::UI::WindowsAndMessaging::HWND_TOPMOST, 0, 0, 0, 0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    redraw_overlay(hwnd, w, h, x, y, color, fill_alpha);
    hwnd as isize
}

unsafe fn redraw_overlay(hwnd: HWND, w: i32, h: i32, x: i32, y: i32, color: u32, fill_alpha: u32) {
    let scr_dc = GetDC(std::ptr::null_mut());
    if scr_dc.is_null() {
        return;
    }
    let mem_dc = CreateCompatibleDC(scr_dc);
    let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();

    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            // negative height => top-down rows
            biHeight: -h,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB_raw(),
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        },
        bmiColors: [Default::default(); 1],
    };
    let hbmp = CreateDIBSection(
        mem_dc,
        &bi,
        DIB_RGB_COLORS,
        &mut bits,
        std::ptr::null_mut(),
        0,
    );
    if hbmp.is_null() || bits.is_null() {
        DeleteDC(mem_dc);
        ReleaseDC(std::ptr::null_mut(), scr_dc);
        return;
    }
    SelectObject(mem_dc, hbmp as *mut _);

    // fill: transparent (or translucent) interior, opaque ring (premultiplied
    // alpha, a=255 == solid color as-is)
    let px = bits as *mut u32;
    let stride = w as usize;
    let bw = opts().thickness.clamp(1, 20) as usize;
    let (cw, ch) = (w as usize, h as usize);
    let fill = color | (0xFFu32 << 24);
    let inner = color | (fill_alpha << 24);
    for row in 0..ch {
        for col in 0..cw {
            let on_ring =
                row < bw || (ch - 1 - row) < bw || col < bw || (cw - 1 - col) < bw;
            if on_ring {
                *px.add(row * stride + col) = fill;
            } else {
                *px.add(row * stride + col) = inner;
            }
        }
    }

    let blink = BLENDFUNCTION {
        BlendOp: AC_SRC_OVER as u8,
        BlendFlags: 0,
        SourceConstantAlpha: 255,
        AlphaFormat: AC_SRC_ALPHA as u8,
    };
    let pt_src = POINT { x: 0, y: 0 };
    let pt_dst = POINT { x, y };
    let size = SIZE { cx: w, cy: h };
    UpdateLayeredWindow(
        hwnd,
        scr_dc,
        &pt_dst,
        &size,
        mem_dc,
        &pt_src,
        0 as COLORREF,
        &blink,
        ULW_ALPHA,
    );

    DeleteObject(hbmp as *mut _);
    DeleteDC(mem_dc);
    ReleaseDC(std::ptr::null_mut(), scr_dc);
    fn BI_RGB_raw() -> u32 { 0u32 }
}

// TEMP debug trace, gated by MRS_TRACE=path (mirrors tiling.rs).
// One cadence counter for the whole watchdog tick: every 25th tick emits the
// entry-set header + per-entry detail lines. No-op when MRS_TRACE is unset.
static TRACE: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
struct TraceGuard;
impl Drop for TraceGuard {
    fn drop(&mut self) {
        use std::io::Write;
        if let Some(p) = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok()) {
            if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
                let _ = writeln!(f, "[detail end]");
            }
        }
    }
}
fn trace_tick_start(n: usize) -> Option<TraceGuard> {
    static CT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let c = CT.fetch_add(1, Ordering::Relaxed);
    if c % 25 != 0 {
        return None;
    }
    use std::io::Write;
    if let Some(p) = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok()) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            if writeln!(f, "[tick {}] entries={}", c, n).is_ok() {
                return Some(TraceGuard);
            }
        }
    }
    None
}
fn trace_entry(g: &TraceGuard, tgt: isize, e: &Entry) {
    use std::io::Write;
    if let Some(p) = TRACE.get_or_init(|| std::env::var("MRS_TRACE").ok()) {
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(p) {
            let alive = if e.overlay != 0 {
                unsafe { IsWindow(e.overlay as HWND) != 0 }
            } else {
                false
            };
            let mut rr = RECT { left: 0, top: 0, right: 0, bottom: 0 };
            if e.overlay != 0 {
                unsafe { GetWindowRect(e.overlay as HWND, &mut rr); }
            }
            let _ = writeln!(
                f,
                "  tgt={:x} overlay={:x} alive={} hidden={} pos={},{}/{}x{} eres={}x{}",
                tgt, e.overlay, alive, e.hidden,
                e.x, e.y, e.w, e.h,
                rr.right - rr.left, rr.bottom - rr.top
            );
        }
    }
}

/// Remove all overlays immediately (used on cleanup / tiling off).
pub fn hide_all() {
    clear_pending();
    let mut entries = ENTRIES.lock().unwrap();
    for (_, e) in entries.iter_mut() {
        if e.overlay != 0 {
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(e.overlay as _);
            }
            e.overlay = 0;
        }
    }
    entries.clear();
}
