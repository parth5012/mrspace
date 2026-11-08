use crate::config;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
    RegisterClassW, ShowWindow, TranslateMessage, CW_USEDEFAULT, MSG, SW_SHOW, WM_CLOSE,
    WM_DESTROY, WNDCLASSW, WS_OVERLAPPEDWINDOW,
};

pub fn show() {
    unsafe {
        let class_name: Vec<u16> = "MrspaceSettings\0".encode_utf16().collect();
        let mut wc: WNDCLASSW = std::mem::zeroed();
        wc.lpfnWndProc = Some(wnd_proc);
        wc.lpszClassName = class_name.as_ptr();
        wc.hInstance = 0 as _;
        RegisterClassW(&wc);

        let title: Vec<u16> = "Mr. Space - Settings\0".encode_utf16().collect();
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT, 480, 320,
            0 as HWND, 0 as _, 0 as _, std::ptr::null(),
        );
        // add a static text with config path
        let cfg = config::load();
        let info = format!("Config: {}\nIPC port: {}\nHotkeys: {}", config::path().display(), cfg.ipc_port, cfg.hotkeys.len());
        let info_w: Vec<u16> = info.encode_utf16().chain(std::iter::once(0)).collect();
        let static_class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        CreateWindowExW(0, static_class.as_ptr(), info_w.as_ptr(), 0, 10, 10, 440, 120, hwnd, 0 as _, 0 as _, std::ptr::null());

        ShowWindow(hwnd, SW_SHOW);
        let mut msg: MSG = std::mem::zeroed();
        loop {
            let r = GetMessageW(&mut msg, 0 as HWND, 0, 0);
            if r <= 0 { break; }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_CLOSE => { PostQuitMessage(0); 0 }
        WM_DESTROY => { PostQuitMessage(0); 0 }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}