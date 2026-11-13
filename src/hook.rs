// Leader-style global input: hold the Win key, then tap keys in sequence.
// A low-level hook tracks the Win key; every mapped action key pressed while
// it is held is executed and swallowed. The Win key-up is swallowed too *only
// if* an action was used while it was held — tapping Win alone still opens
// the Start menu, and unmapped Win+key combos (Win+D, Win+E…) pass through.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, DispatchMessageW, TranslateMessage,
    KBDLLHOOKSTRUCT, LLKHF_UP, WH_KEYBOARD_LL, MSG, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;

struct State {
    win_down: AtomicBool,
    acted: AtomicBool,
    tx: Sender<u32>,
}

static STATE: std::sync::OnceLock<State> = std::sync::OnceLock::new();
static HOOK: AtomicU64 = AtomicU64::new(0);

pub fn start() {
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    let _ = STATE.set(State {
        win_down: AtomicBool::new(false),
        acted: AtomicBool::new(false),
        tx,
    });
    // executor thread: actions run off the input pipeline
    std::thread::spawn(move || {
        for vk in rx {
            execute(vk);
        }
    });
    // hook thread
    std::thread::spawn(move || unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), std::ptr::null_mut(), 0);
        if hook.is_null() {
            eprintln!("failed to install keyboard hook");
            return;
        }
        HOOK.store(hook as u64, Ordering::SeqCst);
        let mut msg: MSG = std::mem::zeroed();
        loop {
            if GetMessageW(&mut msg, 0 as _, 0, 0) <= 0 { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        HOOK.store(0, Ordering::SeqCst);
    });
}

fn is_win_key(vk: u32) -> bool {
    vk == VK_LWIN || vk == VK_RWIN
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        let vk = info.vkCode;
        // ignore our own injected key-ups (modifier release)
        if info.flags & 0x10 != 0 { return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam); }
        let is_down = wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize;
        let is_up = wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize;
        if let Some(state) = STATE.get() {
            if is_win_key(vk) {
                if is_down {
                    state.win_down.store(true, Ordering::Relaxed);
                    state.acted.store(false, Ordering::Relaxed);
                } else if is_up {
                    state.win_down.store(false, Ordering::Relaxed);
                    // if we ran actions while Win was held, swallow the up so
                    // the Start menu doesn't pop open
                    if state.acted.swap(false, Ordering::Relaxed) {
                        return 1;
                    }
                }
                return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
            }
            // action key: only consumed while Win is held *and* it is mapped.
            // If Ctrl is also held, let Windows handle it (Ctrl+Win+Arrow =
            // desktop switch, Ctrl+Win+D = new desktop, etc.).
            let ctrl_held = unsafe { GetAsyncKeyState(0x11) as u32 & 0x8000 != 0 };
            let held = state.win_down.load(Ordering::Relaxed);
            if held && is_down && !ctrl_held {
                if is_action_key(vk) {
                    state.acted.store(true, Ordering::Relaxed);
                    let _ = state.tx.send(vk);
                    return 1; // swallow
                }
                // unmapped keys pass through untouched (Win+D, Win+E …)
            }
        }
    }
    CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam)
}

// action map
const VK_ESCAPE_U32: u32 = 0x1B;
const VK_LEFT_U32: u32 = 0x25;
const VK_RIGHT_U32: u32 = 0x27;
const VK_UP_U32: u32 = 0x26;
const VK_DOWN_U32: u32 = 0x28;

fn is_action_key(vk: u32) -> bool {
    matches!(
        vk,
        0x25..=0x28          // arrows
        | 0x20               // Space (palette)
        | 0xBB | 0xBD        // +/= and -/_
        | 0x4D | 0x47 | 0x4F | 0x54 | 0x48 | 0x4A | 0x4B | 0x4C | 0x52 | 0x4E | 0x50 | 0x46 | 0x53 | 0x41 | 0x56 | 0x59 | 0x55
        | 0x31..=0x39        // 1–9 desktop numbers
        | 0x1B               // Esc (resize-mode exit)
    )
}

fn run_mode_action(act: &str) {
    let step = crate::config::load().resize_step;
    let h = crate::win32::foreground();
    if h.is_null() { return; }
    let r = match crate::win32::window_rect(h) { Some(r) => r, None => return };
    match act {
        "nudge-left" => {
            crate::resize::touch();
            crate::win32::move_resize(h, r.left - step, r.top, r.right - r.left, r.bottom - r.top);
        }
        "nudge-right" => {
            crate::resize::touch();
            crate::win32::move_resize(h, r.left + step, r.top, r.right - r.left, r.bottom - r.top);
        }
        "nudge-up" => {
            crate::resize::touch();
            crate::win32::move_resize(h, r.left, r.top - step, r.right - r.left, r.bottom - r.top);
        }
        "nudge-down" => {
            crate::resize::touch();
            crate::win32::move_resize(h, r.left, r.top + step, r.right - r.left, r.bottom - r.top);
        }
        "move-left" => {
            crate::win32::move_resize(h, r.left - step, r.top, r.right - r.left, r.bottom - r.top);
        }
        "move-right" => {
            crate::win32::move_resize(h, r.left + step, r.top, r.right - r.left, r.bottom - r.top);
        }
        "move-up" => {
            crate::win32::move_resize(h, r.left, r.top - step, r.right - r.left, r.bottom - r.top);
        }
        "move-down" => {
            crate::win32::move_resize(h, r.left, r.top + step, r.right - r.left, r.bottom - r.top);
        }
        _ => {}
    }
    crate::borders::note_window(h);
}

fn execute(vk: u32) {
    // binding modes: Esc exits; mode-specific actions take precedence
    let mode = crate::modes::current();
    if mode != "base" && vk == VK_ESCAPE_U32 {
        crate::modes::exit();
        return;
    }
    if let Some(act) = crate::modes::action_for(&mode, vk) {
        run_mode_action(act);
        return;
    }
    let action = match vk {
        VK_LEFT_U32 => "pin-left",
        VK_RIGHT_U32 => "pin-right",
        VK_UP_U32 => "pin-top",
        VK_DOWN_U32 => "pin-bottom",
        0x4D /*M*/ => "maximize",
        0x47 /*G*/ => "fit-all",
        0x4F /*O*/ => "layout-cycle",
        0x59 /*Y*/ => "park",
        0x55 /*U*/ => "summon",
        0x46 /*F*/ => "toggle-float",
        0x53 /*S*/ => "swap-next",
        0x41 /*A*/ => "promote",
        0x54 /*T*/ => "toggle-tiling",
        0x48 /*H*/ => "focus-left",
        0x4A /*J*/ => "focus-down",
        0x4B /*K*/ => "focus-up",
        0x4C /*L*/ => "focus-right",
        0x52 /*R*/ => "resize-mode",
        0x56 /*V*/ => "move-mode",
        0x4E /*N*/ => "next-desktop",
        0x50 /*P*/ => "prev-desktop",
        0xBB /*+/=*/ => "grow",
        0xBD /*-/_*/ => "shrink",
        0x20 /*Space*/ => "palette",
        VK_ESCAPE_U32 => "exit-resize",
        vk if (0x31..=0x39).contains(&vk) => {
            crate::desktop::noop(vk);
            return;
        }
        _ => return,
    };
    if action == "exit-resize" {
        if crate::resize::is_active() { crate::resize::toggle(); }
        return;
    }
    // resize mode: arrows nudge
    let rm = crate::resize::is_active();
    match action {
        "pin-left" if rm => crate::resize::nudge("left"),
        "pin-right" if rm => crate::resize::nudge("right"),
        "pin-top" if rm => crate::resize::nudge("up"),
        "pin-bottom" if rm => crate::resize::nudge("down"),
        "resize-mode" => crate::resize::toggle(),
        "move-mode" => crate::modes::enter("move"),
        "park" => { let h = crate::win32::foreground(); if !h.is_null() { crate::scratchpad::park(h); } },
        "summon" => { crate::scratchpad::summon(); },
        _ => crate::daemon::execute_action(action),
    }
}