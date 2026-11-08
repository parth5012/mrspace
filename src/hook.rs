// Leader-style global input: hold Ctrl+Alt, then tap keys in sequence.
// A low-level hook tracks the modifiers; every action key pressed while both
// are held is executed and swallowed. Modifier keys themselves are passed
// through untouched, so plain Alt keeps working in apps.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, SetWindowsHookExW, DispatchMessageW, TranslateMessage,
    KBDLLHOOKSTRUCT, LLKHF_UP, WH_KEYBOARD_LL, MSG, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

const VK_CONTROL: u32 = 0x11;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_MENU: u32 = 0x12;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;

struct State {
    ctrl: AtomicBool,
    alt: AtomicBool,
    tx: Sender<u32>,
}

static STATE: std::sync::OnceLock<State> = std::sync::OnceLock::new();
static HOOK: AtomicU64 = AtomicU64::new(0);

pub fn start() {
    let (tx, rx) = std::sync::mpsc::channel::<u32>();
    let _ = STATE.set(State {
        ctrl: AtomicBool::new(false),
        alt: AtomicBool::new(false),
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

fn is_modifier_key(vk: u32) -> bool {
    matches!(
        vk,
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL | VK_MENU | VK_LMENU | VK_RMENU | VK_LSHIFT | VK_RSHIFT
    )
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let info = &*(lparam as *const KBDLLHOOKSTRUCT);
        let vk = info.vkCode;
        let up = info.flags & LLKHF_UP != 0;
        // ignore our own injected key-ups (modifier release)
        if info.flags & 0x10 != 0 { return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam); }
        let is_down = wparam == WM_KEYDOWN as usize || wparam == WM_SYSKEYDOWN as usize;
        let is_up = wparam == WM_KEYUP as usize || wparam == WM_SYSKEYUP as usize;
        if let Some(state) = STATE.get() {
            if is_modifier_key(vk) {
                // pass modifiers through untouched
                if is_down || is_up {
                    let ctrl = matches!(vk, VK_CONTROL | VK_LCONTROL | VK_RCONTROL);
                    let alt = matches!(vk, VK_MENU | VK_LMENU | VK_RMENU);
                    if !up {
                        if ctrl { state.ctrl.store(true, Ordering::Relaxed); }
                        if alt { state.alt.store(true, Ordering::Relaxed); }
                    } else {
                        if ctrl { state.ctrl.store(false, Ordering::Relaxed); }
                        if alt { state.alt.store(false, Ordering::Relaxed); }
                    }
                }
                return CallNextHookEx(std::ptr::null_mut(), code, wparam, lparam);
            }
            // action key
            let held = state.ctrl.load(Ordering::Relaxed) && state.alt.load(Ordering::Relaxed);
            if held && !is_up {
                let _ = state.tx.send(vk);
                return 1; // swallow
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

fn execute(vk: u32) {
    let action = match vk {
        VK_LEFT_U32 => "pin-left",
        VK_RIGHT_U32 => "pin-right",
        VK_UP_U32 => "pin-top",
        VK_DOWN_U32 => "pin-bottom",
        0x4D /*M*/ => "maximize",
        0x47 /*G*/ => "fit-all",
        0x4F /*O*/ => "layout-cycle",
        0x54 /*T*/ => "toggle-tiling",
        0x48 /*H*/ => "focus-left",
        0x4A /*J*/ => "focus-down",
        0x4B /*K*/ => "focus-up",
        0x4C /*L*/ => "focus-right",
        0x52 /*R*/ => "resize-mode",
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
        _ => crate::daemon::execute_action(action),
    }
}