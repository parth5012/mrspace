use crate::config::Config;
use crate::layout;
use crate::win32;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

pub fn run(cfg: Config) {
    crate::resize::init();
    crate::tiling::init();
    crate::hook::start();
    crate::borders::configure(&cfg.borders);
    crate::borders::start();
    crate::mouse_focus::start();
    let port = cfg.ipc_port;

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

/// Launch the palette in its own process. gpui runs its own event loop, so a
/// dedicated process keeps the daemon, IPC and hook threads free (and a crashed
/// palette can never wedge the daemon).
/// If a palette is already open (holds the single-instance mutex), raise it to
/// the foreground instead — alleviate without stacking duplicate processes.
pub fn spawn_palette_process() {
    if raise_existing() {
        return;
    }
    let exe = match std::env::current_exe() { Ok(e) => e, Err(_) => return };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("palette");
    // direct mode: otherwise the child would forward "palette" back over IPC
    // and we'd spawn children forever
    cmd.env("MRSPACE_NO_IPC", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: no console flash
    }
    // trace palette stderr for crash triage (%TEMP%\mrspace-palette.log)
    if let Ok(f) = std::fs::File::create(std::env::temp_dir().join("mrspace-palette.log")) {
        use std::process::Stdio;
        cmd.stdout(Stdio::from(f.try_clone().unwrap_or_else(|_| {
            std::fs::File::create(std::env::temp_dir().join("mrspace-palette.log")).unwrap()
        })));
        cmd.stderr(Stdio::from(f));
    }
    let _ = cmd.spawn();
}

/// Launch the settings app in its own process (same isolations as the
/// palette). Raises an already-open settings window instead of spawning a
/// second one.
pub fn spawn_settings_process() {
    if crate::settings::raise_settings() {
        return;
    }
    let exe = match std::env::current_exe() { Ok(e) => e, Err(_) => return };
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("settings");
    cmd.env("MRSPACE_NO_IPC", "1");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    if let Ok(f) = std::fs::File::create(std::env::temp_dir().join("mrspace-settings.log")) {
        use std::process::Stdio;
        cmd.stdout(Stdio::from(f.try_clone().unwrap_or_else(|_| {
            std::fs::File::create(std::env::temp_dir().join("mrspace-settings.log")).unwrap()
        })));
        cmd.stderr(Stdio::from(f));
    }
    let _ = cmd.spawn();
}

/// Bring an already-open palette window to the foreground. Reads the hwnd
/// record the palette process published; returns false if none is alive.
#[cfg(windows)]
pub fn raise_existing() -> bool {
    crate::palette::raise_other()
}
#[cfg(not(windows))]
pub fn raise_existing() -> bool {
    false
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
    // some commands accept an optional target hwnd (decimal) — e.g.
    // "pin-left 2090304" pins that specific window instead of the foreground
    let explicit_hwnd: Option<isize> = parts.get(1).and_then(|p| p.parse::<isize>().ok());
    let target = |fallback: isize| -> isize {
        let fg = win32::foreground() as isize;
        if let Some(h) = explicit_hwnd {
            if h != 0 && win32::is_manageable(h as _) { h } else { fg }
        } else {
            fallback
        }
    };
    match parts[0] {
        "pin-left" => { let h = target(win32::foreground() as isize); if h != 0 { layout::pin_left(h as _); } "OK".into() }
        "pin-right" => { let h = target(win32::foreground() as isize); if h != 0 { layout::pin_right(h as _); } "OK".into() }
        "pin-top" => { let h = target(win32::foreground() as isize); if h != 0 { layout::pin_top(h as _); } "OK".into() }
        "pin-bottom" => { let h = target(win32::foreground() as isize); if h != 0 { layout::pin_bottom(h as _); } "OK".into() }
        "maximize" => { let h = target(win32::foreground() as isize); if h != 0 { layout::maximize(h as _); } "OK".into() }
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
        "debug-windows" => {
            win32::debug_windows()
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
        "settings" => { crate::daemon::spawn_settings_process(); "OK".into() }
        "reload-config" => {
            let c = crate::config::load();
            crate::borders::configure(&c.borders);
            // note: resize step/timeout and exclusions are re-read from disk
            // on every use already; borders are the only cached options.
            format!("reloaded (borders {}, port {})",
                if c.borders.enabled { "on" } else { "off" }, c.ipc_port)
        }
        "focus-left" => { crate::focus::focus_direction("left"); "OK".into() }
        "focus-down" => { crate::focus::focus_direction("down"); "OK".into() }
        "focus-up" => { crate::focus::focus_direction("up"); "OK".into() }
        "focus-right" => { crate::focus::focus_direction("right"); "OK".into() }
        "resize-mode" => { crate::resize::toggle(); "OK".into() }
        "grow" => { crate::resize::grow(); "OK".into() }
        "shrink" => { crate::resize::shrink(); "OK".into() }
        "next-desktop" => { crate::desktop::next_desktop_view(); "OK".into() }
        "prev-desktop" => { crate::desktop::prev_desktop_view(); "OK".into() }
        "move-next-desktop" => { crate::desktop::move_to_next_desktop(); "OK".into() }
        "move-prev-desktop" => { crate::desktop::move_to_prev_desktop(); "OK".into() }
        "palette" => { crate::daemon::spawn_palette_process(); "OK".into() }
        "toggle-tiling" => { let on = crate::tiling::toggle(); format!("tiling {}", if on {"ON"} else {"OFF"}) }
        "layout" => {
            let name = parts.get(1).copied().unwrap_or("");
            if crate::layouts::apply(name) { format!("layout {name} applied") } else { format!("ERR unknown layout: {name}") }
        }
        "exec" => {
            // escape hatch: run the remainder as a direct action
            execute_action(&parts[1..].join(" "));
            "OK".into()
        }
        "layout-cycle" => { let n = crate::layouts::cycle(); format!("layout {n}") }
        "enter-mode" => { let name = parts.get(1).copied().unwrap_or("base"); crate::modes::enter(name); format!("mode {name}") }
        "toggle-float" => { let h = win32::foreground(); if !h.is_null() { crate::managed::toggle_float(h as isize); } "OK".into() }
        "swap-next" => { let h = win32::foreground(); if !h.is_null() { layout::swap_next(h); } "OK".into() }
        "promote" => { let h = win32::foreground(); if !h.is_null() { layout::promote(h); } "OK".into() }
        "park" => { let h = win32::foreground(); if !h.is_null() { crate::scratchpad::park(h); } "OK".into() }
        "summon" => { if crate::scratchpad::summon() { "OK".into() } else { "ERR no parked windows".into() } }
        "state" => {
            // flat JSON snapshot of managed windows
            let mut out = String::from("{\"windows\":[");
            let mut first = true;
            for h in win32::enum_windows() {
                if !win32::is_manageable(h) { continue; }
                let title = win32::window_title(h).replace('"', "\\\"");
                let class = win32::class_name(h).replace('"', "\\\"");
                let exe = win32::process_name(h).replace('"', "\\\"");
                let r = win32::window_rect(h);
                let (x,y,w,ht) = match r { Some(r) => (r.left, r.top, r.right-r.left, r.bottom-r.top), None => (0,0,0,0) };
                if !first { out.push(','); }
                first = false;
                out.push_str(&format!(
                    "{{\"hwnd\":{},\"pid\":{},\"title\":\"{}\",\"class\":\"{}\",\"exe\":\"{}\",\"x\":{},\"y\":{},\"w\":{},\"h\":{},\"floating\":{}}}",
                    h as usize, win32::process_id(h), title, class, exe, x, y, w, ht,
                    crate::managed::is_floating(h as isize)
                ));
            }
            out.push_str("]}");
            out
        }
        "park" => { let h = win32::foreground(); if !h.is_null() { crate::scratchpad::park(h); } "OK".into() }
        "summon" => { if crate::scratchpad::summon() { "OK".into() } else { "ERR no parked windows".into() } }
        other => format!("ERR unknown command: {other}"),
    }
}

fn handle_action(action: &str) {
    execute_action(action);
}

pub fn execute_action(action: &str) {
    // resize mode: arrows nudge instead of pin
    let rm = crate::resize::is_active();
    match action {
        "pin-left" => { if rm { crate::resize::nudge("left"); } else { let h = win32::foreground(); if !h.is_null() { layout::pin_left(h); } } }
        "pin-right" => { if rm { crate::resize::nudge("right"); } else { let h = win32::foreground(); if !h.is_null() { layout::pin_right(h); } } }
        "pin-top" => { if rm { crate::resize::nudge("up"); } else { let h = win32::foreground(); if !h.is_null() { layout::pin_top(h); } } }
        "pin-bottom" => { if rm { crate::resize::nudge("down"); } else { let h = win32::foreground(); if !h.is_null() { layout::pin_bottom(h); } } }
        "maximize" => { let h = win32::foreground(); if !h.is_null() { layout::maximize(h); } }
        "fit-all" => layout::fit_all(),
        "focus-left" => crate::focus::focus_direction("left"),
        "focus-down" => crate::focus::focus_direction("down"),
        "focus-up" => crate::focus::focus_direction("up"),
        "focus-right" => crate::focus::focus_direction("right"),
        "resize-mode" => crate::resize::toggle(),
        "grow" => crate::resize::grow(),
        "shrink" => crate::resize::shrink(),
        "next-desktop" => crate::desktop::next_desktop_view(),
        "prev-desktop" => crate::desktop::prev_desktop_view(),
        "move-next-desktop" => crate::desktop::move_to_next_desktop(),
        "move-prev-desktop" => crate::desktop::move_to_prev_desktop(),
        "palette" => crate::daemon::spawn_palette_process(),
        "toggle-tiling" => { let _ = crate::tiling::toggle(); },
        "layout-cycle" => { crate::layouts::cycle(); },
        "toggle-float" => { let h = win32::foreground(); if !h.is_null() { crate::managed::toggle_float(h as isize); } },
        "swap-next" => { let h = win32::foreground(); if !h.is_null() { layout::swap_next(h); } },
        "promote" => { let h = win32::foreground(); if !h.is_null() { layout::promote(h); } },
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

/// Non-failing sender for the settings UI: returns None instead of an "ERR"
/// string when no daemon is listening.
pub fn try_send_command(port: u16, cmd: &str) -> Option<String> {
    let r = send_command(port, cmd);
    if r.starts_with("ERR connect") {
        None
    } else {
        Some(r)
    }
}

pub fn is_daemon_running(port: u16) -> bool {
    TcpStream::connect(format!("127.0.0.1:{port}")).is_ok()
}

pub fn run_command(cmd: &str) -> String {
    dispatch(cmd)
}