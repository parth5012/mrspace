use crate::config::Config;
use crate::layout;
use crate::win32;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

pub fn run(cfg: Config) {
    let port = cfg.ipc_port;
    // hotkey thread
    let cfg_clone = cfg.clone();
    thread::spawn(move || {
        let loop_ = crate::hotkeys::HotkeyLoop::register(&cfg_clone);
        loop_.run(&cfg_clone, move |action| {
            handle_action(action);
        });
    });

    // ipc server
    let listener = match TcpListener::bind(format!("127.0.0.1:{port}")) {
        Ok(l) => l,
        Err(e) => { eprintln!("bind failed: {e}"); return; }
    };
    println!("mrspace daemon listening on 127.0.0.1:{port}");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                thread::spawn(move || handle_client(s));
            }
            Err(e) => eprintln!("accept err: {e}"),
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() { return; }
    let line = line.trim().to_string();
    let resp = dispatch(&line);
    let _ = stream.write_all(resp.as_bytes());
    let _ = stream.write_all(b"\n");
}

fn dispatch(cmd: &str) -> String {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    if parts.is_empty() { return "ERR empty".into(); }
    match parts[0] {
        "pin-left" => { let h = win32::foreground(); if !h.is_null() { layout::pin_left(h); } "OK".into() }
        "pin-right" => { let h = win32::foreground(); if !h.is_null() { layout::pin_right(h); } "OK".into() }
        "pin-top" => { let h = win32::foreground(); if !h.is_null() { layout::pin_top(h); } "OK".into() }
        "pin-bottom" => { let h = win32::foreground(); if !h.is_null() { layout::pin_bottom(h); } "OK".into() }
        "maximize" => { let h = win32::foreground(); if !h.is_null() { layout::maximize(h); } "OK".into() }
        "fit-all" => { layout::fit_all(); "OK".into() }
        "list-windows" => {
            let mut out = String::new();
            for h in win32::enum_windows() {
                if win32::is_manageable(h) {
                    out.push_str(&format!("{}|{}|{}\n", h as usize, win32::process_id(h), win32::window_title(h)));
                }
            }
            out
        }
        "list-desktops" => "not-enumerable (IVirtualDesktopManager only moves windows)\n".into(),
        "send-to" => {
            if parts.len() < 2 { return "ERR usage: send-to <desktop-guid>".into(); }
            let hwnd = win32::foreground();
            if hwnd.is_null() { return "ERR no foreground".into(); }
            let gstr = parts[1];
            let guid = match parse_guid(gstr) { Some(g) => g, None => return "ERR bad guid".into() };
            let v = match crate::virtual_desktop::VirtualDesktopManager::new() { Ok(v)=>v, Err(e)=>return format!("ERR {e}") };
            match v.move_to_desktop(hwnd, &guid) { Ok(_) => "OK".into(), Err(e) => format!("ERR {e}") }
        }
        "current-desktop" => {
            let hwnd = win32::foreground();
            if hwnd.is_null() { return "ERR no foreground".into(); }
            let v = match crate::virtual_desktop::VirtualDesktopManager::new() { Ok(v)=>v, Err(e)=>return format!("ERR {e}") };
            match v.current_desktop_id(hwnd) {
                Ok(g) => format!("{:08x}-{:04x}-{:04x}-{:04x}-{:012x}", g.data1, g.data2, g.data3, u16::from_be_bytes([g.data4[0],g.data4[1]]), u64::from_be_bytes([0,0,g.data4[2],g.data4[3],g.data4[4],g.data4[5],g.data4[6],g.data4[7]])),
                Err(e) => format!("ERR {e}")
            }
        }
        "settings" => { crate::settings::show(); "OK".into() }
        other => format!("ERR unknown command: {other}"),
    }
}

fn handle_action(action: &str) {
    match action {
        "pin-left" => { let h = win32::foreground(); if !h.is_null() { layout::pin_left(h); } }
        "pin-right" => { let h = win32::foreground(); if !h.is_null() { layout::pin_right(h); } }
        "pin-top" => { let h = win32::foreground(); if !h.is_null() { layout::pin_top(h); } }
        "pin-bottom" => { let h = win32::foreground(); if !h.is_null() { layout::pin_bottom(h); } }
        "maximize" => { let h = win32::foreground(); if !h.is_null() { layout::maximize(h); } }
        "fit-all" => layout::fit_all(),
        _ => {}
    }
}

fn parse_guid(s: &str) -> Option<windows_sys::core::GUID> {
    let s = s.trim_matches(|c| c == '{' || c == '}');
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 { return None; }
    let a = u32::from_str_radix(parts[0], 16).ok()?;
    let b = u16::from_str_radix(parts[1], 16).ok()?;
    let c = u16::from_str_radix(parts[2], 16).ok()?;
    let d = u16::from_str_radix(parts[3], 16).ok()?;
    let e = u64::from_str_radix(parts[4], 16).ok()?;
    let mut g = windows_sys::core::GUID { data1: a, data2: b, data3: c, data4: [0;8] };
    g.data4[0] = (d >> 8) as u8;
    g.data4[1] = (d & 0xff) as u8;
    let bytes = e.to_be_bytes();
    for i in 0..6 { g.data4[2+i] = bytes[2+i]; }
    Some(g)
}

pub fn send_command(port: u16, cmd: &str) -> String {
    use std::io::{BufRead, BufReader};
    let mut stream = match TcpStream::connect(format!("127.0.0.1:{port}")) {
        Ok(s) => s,
        Err(e) => return format!("ERR connect: {e}"),
    };
    let _ = stream.write_all(cmd.as_bytes());
    let _ = stream.write_all(b"\n");
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut r = BufReader::new(stream);
    let mut line = String::new();
    let _ = r.read_line(&mut line);
    line
}

pub fn is_daemon_running(port: u16) -> bool {
    TcpStream::connect(format!("127.0.0.1:{port}")).is_ok()
}