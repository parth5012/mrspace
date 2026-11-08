mod borders;
mod config;
mod daemon;
mod desktop;
mod error;
mod focus;
mod hotkeys;
mod hook;
mod layout;
mod layouts;
mod palette;
mod resize;
mod settings;
mod tiling;
mod virtual_desktop;
mod win32;

use clap::{Parser, Subcommand};
use config::Config;

#[derive(Parser)]
#[command(name = "mrspace", version, about = "Windows on-demand window manager (Rust). Inspired by Mr. Space.")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the background daemon (hotkeys + IPC).
    Daemon,
    /// Pin the foreground window to the left half.
    PinLeft,
    /// Pin the foreground window to the right half.
    PinRight,
    /// Pin the foreground window to the top half.
    PinTop,
    /// Pin the foreground window to the bottom half.
    PinBottom,
    /// Maximize the foreground window to the work area.
    Maximize,
    /// Auto-fit all visible windows into a grid.
    FitAll,
    /// List manageable windows.
    ListWindows,
    /// Show current desktop id of the foreground window.
    CurrentDesktop,
    /// Move foreground window to a desktop by GUID.
    SendTo { desktop: String },
    /// Show settings window.
    Settings,
    /// Open command palette.
    Palette,
    /// Toggle automatic tiling mode.
    ToggleTiling,
    /// Apply a named layout (grid, columns-2, main-left, ...).
    Layout { name: String },
    /// Cycle through layouts.
    LayoutCycle,
    /// Focus window to the left (geometric).
    FocusLeft,
    FocusDown,
    FocusUp,
    FocusRight,
    /// Toggle resize mode.
    ResizeMode,
    Grow,
    Shrink,
    NextDesktop,
    PrevDesktop,
    MoveNextDesktop,
    MovePrevDesktop,
    /// Print config path.
    ConfigPath,
    /// Run a raw command string against the daemon (agent-friendly).
    Exec { command: String },
}

fn cfg() -> Config { config::load() }

fn main() {
    resize::init();
    tiling::init();
    let cli = Cli::parse();
    let c = cfg();
    let p = c.ipc_port;
    match cli.cmd {
        None => {
            // default: if daemon not running, start it; else print help hint
            if !daemon::is_daemon_running(p) {
                daemon::run(c);
            } else {
                println!("daemon already running. use subcommands.");
            }
        }
        Some(Commands::Daemon) => daemon::run(c),
        Some(Commands::PinLeft) => run_or_direct(p, "pin-left", || { let h = win32::foreground(); if !h.is_null() { layout::pin_left(h); } }),
        Some(Commands::PinRight) => run_or_direct(p, "pin-right", || { let h = win32::foreground(); if !h.is_null() { layout::pin_right(h); } }),
        Some(Commands::PinTop) => run_or_direct(p, "pin-top", || { let h = win32::foreground(); if !h.is_null() { layout::pin_top(h); } }),
        Some(Commands::PinBottom) => run_or_direct(p, "pin-bottom", || { let h = win32::foreground(); if !h.is_null() { layout::pin_bottom(h); } }),
        Some(Commands::Maximize) => run_or_direct(p, "maximize", || { let h = win32::foreground(); if !h.is_null() { layout::maximize(h); } }),
        Some(Commands::FitAll) => run_or_direct(p, "fit-all", || layout::fit_all()),
        Some(Commands::ListWindows) => {
            if daemon::is_daemon_running(p) {
                print!("{}", daemon::send_command(p, "list-windows"));
            } else {
                for h in win32::enum_windows() {
                    if win32::is_manageable(h) {
                        println!("{}|{}|{}", h as usize, win32::process_id(h), win32::window_title(h));
                    }
                }
            }
        }
        Some(Commands::CurrentDesktop) => {
            if daemon::is_daemon_running(p) {
                print!("{}", daemon::send_command(p, "current-desktop"));
            } else {
                let v = virtual_desktop::VirtualDesktopManager::new().unwrap();
                let h = win32::foreground();
                if !h.is_null() { let g = v.current_desktop_id(h).unwrap(); println!("{{{:08x}-{:04x}-{:04x}-{:04x}-{:012x}}}", g.data1, g.data2, g.data3, u16::from_be_bytes([g.data4[0],g.data4[1]]), u64::from_be_bytes([0,0,g.data4[2],g.data4[3],g.data4[4],g.data4[5],g.data4[6],g.data4[7]])); }
            }
        }
        Some(Commands::SendTo { desktop }) => {
            let cmd = format!("send-to {desktop}");
            if daemon::is_daemon_running(p) {
                print!("{}", daemon::send_command(p, &cmd));
            } else {
                let v = virtual_desktop::VirtualDesktopManager::new().unwrap();
                let h = win32::foreground();
                if !h.is_null() {
                    if let Some(g) = parse_guid_cli(&desktop) { let _ = v.move_to_desktop(h, &g); println!("OK"); }
                }
            }
        }
        Some(Commands::Settings) => {
            if daemon::is_daemon_running(p) {
                let _ = daemon::send_command(p, "settings");
            } else {
                settings::show();
            }
        }
        Some(Commands::Palette) => run_or_direct(p, "palette", || palette::show()),
        Some(Commands::ToggleTiling) => run_or_direct(p, "toggle-tiling", || { let _ = tiling::toggle(); }),
        Some(Commands::Layout { name }) => run_or_direct(p, &format!("layout {name}"), || { let _ = layouts::apply(&name); }),
        Some(Commands::LayoutCycle) => run_or_direct(p, "layout-cycle", || { layouts::cycle(); }),
        Some(Commands::FocusLeft) => run_or_direct(p, "focus-left", || focus::focus_direction("left")),
        Some(Commands::FocusDown) => run_or_direct(p, "focus-down", || focus::focus_direction("down")),
        Some(Commands::FocusUp) => run_or_direct(p, "focus-up", || focus::focus_direction("up")),
        Some(Commands::FocusRight) => run_or_direct(p, "focus-right", || focus::focus_direction("right")),
        Some(Commands::ResizeMode) => run_or_direct(p, "resize-mode", || resize::toggle()),
        Some(Commands::Grow) => run_or_direct(p, "grow", || resize::grow()),
        Some(Commands::Shrink) => run_or_direct(p, "shrink", || resize::shrink()),
        Some(Commands::NextDesktop) => run_or_direct(p, "next-desktop", || desktop::next_desktop_view()),
        Some(Commands::PrevDesktop) => run_or_direct(p, "prev-desktop", || desktop::prev_desktop_view()),
        Some(Commands::MoveNextDesktop) => run_or_direct(p, "move-next-desktop", || desktop::move_to_next_desktop()),
        Some(Commands::MovePrevDesktop) => run_or_direct(p, "move-prev-desktop", || desktop::move_to_prev_desktop()),
        Some(Commands::ConfigPath) => println!("{}", config::path().display()),
        Some(Commands::Exec { command }) => {
            if daemon::is_daemon_running(p) {
                print!("{}", daemon::send_command(p, &command));
            } else {
                println!("daemon not running");
            }
        }
    }
}

fn run_or_direct(p: u16, name: &str, direct: impl FnOnce()) {
    // Children spawned by the daemon (palette/settings windows) must run the
    // UI directly — forwarding back over IPC would recurse (child → daemon →
    // child → …).
    if std::env::var("MRSPACE_NO_IPC").as_deref() == Ok("1") {
        direct();
        return;
    }
    if daemon::is_daemon_running(p) {
        print!("{}", daemon::send_command(p, name));
    } else {
        direct();
    }
}

fn parse_guid_cli(s: &str) -> Option<windows_sys::core::GUID> {
    // reuse daemon's parser via a quick inline
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