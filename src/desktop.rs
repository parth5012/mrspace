use crate::virtual_desktop::VirtualDesktopManager;
use crate::win32;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP,
};

const VK_CONTROL: u16 = 0x11;
const VK_LWIN: u16 = 0x5B;
const VK_RIGHT: u16 = 0x27;
const VK_LEFT: u16 = 0x25;

fn send_combo(keys: &[u16], release: bool) {
    // keys: modifiers then final key; send down then up
    let mut inputs: Vec<INPUT> = Vec::new();
    for &k in keys {
        let mut i: INPUT = unsafe { std::mem::zeroed() };
        i.r#type = INPUT_KEYBOARD;
        i.Anonymous.ki.wVk = k;
        inputs.push(i);
    }
    if release {
        for &k in keys.iter().rev() {
            let mut i: INPUT = unsafe { std::mem::zeroed() };
            i.r#type = INPUT_KEYBOARD;
            i.Anonymous.ki.wVk = k;
            i.Anonymous.ki.dwFlags = KEYEVENTF_KEYUP;
            inputs.push(i);
        }
    }
    unsafe { SendInput(inputs.len() as u32, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32); }
}

pub fn next_desktop_view() {
    // Ctrl+Win+Right
    send_combo(&[VK_CONTROL, VK_LWIN, VK_RIGHT], true);
}

pub fn prev_desktop_view() {
    send_combo(&[VK_CONTROL, VK_LWIN, VK_LEFT], true);
}

fn create_temp_window() -> HWND {
    unsafe {
        let cls: Vec<u16> = "MrspaceTemp\0".encode_utf16().collect();
        let mut wc: windows_sys::Win32::UI::WindowsAndMessaging::WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(temp_proc);
        wc.lpszClassName = cls.as_ptr();
        windows_sys::Win32::UI::WindowsAndMessaging::RegisterClassW(&wc);
        let hwnd = windows_sys::Win32::UI::WindowsAndMessaging::CreateWindowExW(
            0, cls.as_ptr(), std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW,
            0, 0, 10, 10, 0 as HWND, 0 as _, 0 as _, std::ptr::null(),
        );
        hwnd
    }
}

unsafe extern "system" fn temp_proc(hwnd: HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::*;
    match msg {
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

pub fn move_to_next_desktop() {
    move_to_adjacent(true);
}

pub fn move_to_prev_desktop() {
    move_to_adjacent(false);
}

pub fn noop(_vk: u32) {}

fn move_to_adjacent(next: bool) {
    let hwnd = win32::foreground();
    if hwnd.is_null() { return; }
    // switch view
    if next { next_desktop_view(); } else { prev_desktop_view(); }
    // wait for switch
    std::thread::sleep(std::time::Duration::from_millis(300));
    // create temp window on new desktop to get its GUID
    let temp = create_temp_window();
    if temp.is_null() { return; }
    // show it (hidden is fine but needs to be on desktop)
    win32::show(temp);
    std::thread::sleep(std::time::Duration::from_millis(100));
    let vdm = match VirtualDesktopManager::new() { Ok(v) => v, Err(_) => { destroy_window(temp); return; } };
    let guid = match vdm.current_desktop_id(temp) { Ok(g) => g, Err(_) => { destroy_window(temp); return; } };
    // move foreground window
    let _ = vdm.move_to_desktop(hwnd, &guid);
    destroy_window(temp);
}

fn destroy_window(hwnd: HWND) {
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow(hwnd); }
}