use crate::daemon;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage, RegisterClassW,
    SendMessageW, ShowWindow, TranslateMessage, CW_USEDEFAULT, LB_ADDSTRING, LB_GETCURSEL,
    LB_GETTEXT, LB_GETTEXTLEN, LB_RESETCONTENT, MSG, WM_COMMAND, WM_DESTROY, WM_KEYDOWN,
    WNDCLASSW, WS_BORDER, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

static SEARCH_HWND: AtomicIsize = AtomicIsize::new(0);
static LIST_HWND: AtomicIsize = AtomicIsize::new(0);

const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;

const COMMANDS: &[(&str, &str)] = &[
    ("Pin left half", "pin-left"),
    ("Pin right half", "pin-right"),
    ("Pin top half", "pin-top"),
    ("Pin bottom half", "pin-bottom"),
    ("Maximize", "maximize"),
    ("Auto-fit all (grid)", "fit-all"),
    ("Toggle tiling mode", "toggle-tiling"),
    ("Focus left", "focus-left"),
    ("Focus down", "focus-down"),
    ("Focus up", "focus-up"),
    ("Focus right", "focus-right"),
    ("Toggle resize mode", "resize-mode"),
    ("Grow window", "grow"),
    ("Shrink window", "shrink"),
    ("Next desktop", "next-desktop"),
    ("Prev desktop", "prev-desktop"),
    ("Move to next desktop", "move-next-desktop"),
    ("Move to prev desktop", "move-prev-desktop"),
    ("List windows", "list-windows"),
    ("Current desktop", "current-desktop"),
    ("Settings", "settings"),
];

pub fn show() {
    unsafe {
        let cls: Vec<u16> = "MrspacePalette\0".encode_utf16().collect();
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(palette_proc);
        wc.lpszClassName = cls.as_ptr();
        RegisterClassW(&wc);

        let title: Vec<u16> = "Mr. Space — Command Palette\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            0, cls.as_ptr(), title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT, CW_USEDEFAULT, 420, 360,
            0 as HWND, 0 as _, 0 as _, std::ptr::null(),
        );

        let edit_cls: Vec<u16> = "EDIT\0".encode_utf16().collect();
        let search = CreateWindowExW(
            0, edit_cls.as_ptr(), std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_BORDER,
            10, 10, 390, 24,
            hwnd, 0 as _, 0 as _, std::ptr::null(),
        );
        SEARCH_HWND.store(search as isize, Ordering::SeqCst);

        let list_cls: Vec<u16> = "LISTBOX\0".encode_utf16().collect();
        let list = CreateWindowExW(
            0, list_cls.as_ptr(), std::ptr::null(),
            WS_CHILD | WS_VISIBLE | WS_BORDER,
            10, 40, 390, 290,
            hwnd, 0 as _, 0 as _, std::ptr::null(),
        );
        LIST_HWND.store(list as isize, Ordering::SeqCst);

        // populate
        for (label, _) in COMMANDS {
            let w: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
            SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
        }

        ShowWindow(hwnd, 5); // SW_SHOW

        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0 as HWND, 0, 0);
            if r <= 0 { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

fn filter_list(search_text: &str) {
    unsafe {
        let list = LIST_HWND.load(Ordering::SeqCst) as HWND;
        SendMessageW(list, LB_RESETCONTENT, 0, 0);
        let q = search_text.to_lowercase();
        for (label, _) in COMMANDS {
            if q.is_empty() || label.to_lowercase().contains(&q) {
                let w: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
                SendMessageW(list, LB_ADDSTRING, 0, w.as_ptr() as LPARAM);
            }
        }
    }
}

fn run_selected() {
    unsafe {
        let list = LIST_HWND.load(Ordering::SeqCst) as HWND;
        let idx = SendMessageW(list, LB_GETCURSEL, 0, 0) as usize;
        // get text
        let len = SendMessageW(list, LB_GETTEXTLEN, idx, 0) as usize;
        let mut buf = vec![0u16; len + 1];
        SendMessageW(list, LB_GETTEXT, idx, buf.as_mut_ptr() as LPARAM);
        let text = String::from_utf16_lossy(&buf[..len]);
        for (label, cmd) in COMMANDS {
            if *label == text {
                daemon::run_command(cmd);
                break;
            }
        }
    }
}

unsafe extern "system" fn palette_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as u32;
            let code = ((wparam >> 16) & 0xFFFF) as u32;
            let _ = id;
            let ctrl = lparam as HWND;
            let search = SEARCH_HWND.load(Ordering::SeqCst) as HWND;
            if ctrl == search {
                let mut buf = [0u16; 256];
                let n = windows_sys::Win32::UI::WindowsAndMessaging::GetWindowTextW(search, buf.as_mut_ptr(), 256);
                let text = String::from_utf16_lossy(&buf[..n as usize]);
                filter_list(&text);
            }
            if ctrl != search && code == LBN_DBLCLK {
                run_selected();
                PostQuitMessage(0);
            }
            0
        }
        WM_KEYDOWN => {
            if wparam as u32 == VK_RETURN {
                run_selected();
                PostQuitMessage(0);
                0
            } else if wparam as u32 == VK_ESCAPE {
                PostQuitMessage(0);
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}