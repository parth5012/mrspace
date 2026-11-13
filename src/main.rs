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
mod managed;
mod modes;
mod mouse_focus;
mod rules;
mod scratchpad;
mod palette;
mod preview;
mod resize;
mod settings;
mod tiling;
mod workspace;
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
    /// Flow Launcher plugin bridge (JSON-RPC via argv[1]); not for manual use.
    #[command(hide = true)]
    Flow { rpc: String },
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
    /// Show command palette (alias).
    Palette,
    /// Print daemon state as JSON.
    State,
    /// Toggle automatic tiling mode.
    ToggleTiling,
    /// Apply a named layout (grid, columns-2, main-left, ...).
    Layout { name: String },
    /// Cycle through layouts.
    LayoutCycle,
    /// Toggle floating state of the foreground window.
    ToggleFloat,
    /// Swap the foreground window with the next manageable window.
    SwapNext,
    /// Promote the foreground window to the main slot.
    Promote,
    /// Park the foreground window offscreen (scratchpad).
    Park,
    /// Summon the last parked window.
    Summon,
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

/// Opt into real pixel coordinates for the whole process, before any window
/// exists. Without this the daemon runs DPI-unaware: on a 150%-scaled display
/// every coordinate it reads/writes is virtualized (×1/1.5), and when the
/// settings window flips awareness mid-run the coordinate space changes
/// under the tiler — tiles overlap and run off-screen (the "bad layout" bug).
fn set_dpi_aware() {
    use windows_sys::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
    unsafe {
        // Fails harmlessly if awareness was already set (or unsupported).
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

fn main() {
    set_dpi_aware();
    resize::init();
    tiling::init();
    let cli = Cli::parse();
    if let Some(Commands::Flow { rpc }) = &cli.cmd {
        flow_protocol(rpc);
        return;
    }
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
        Some(Commands::Flow { .. }) => unreachable!("handled before config load"),
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
        Some(Commands::Settings) => run_or_direct(p, "settings", || settings::show()),
        Some(Commands::Palette) => run_or_direct(p, "palette", || palette::show()),
        Some(Commands::State) => run_or_direct(p, "state", || {}),
        Some(Commands::ToggleTiling) => run_or_direct(p, "toggle-tiling", || { let _ = tiling::toggle(); }),
        Some(Commands::Layout { name }) => run_or_direct(p, &format!("layout {name}"), || { let _ = layouts::apply(&name); }),
        Some(Commands::LayoutCycle) => run_or_direct(p, "layout-cycle", || { preview::cycle(); }),
        Some(Commands::ToggleFloat) => run_or_direct(p, "toggle-float", || { let h = win32::foreground(); if !h.is_null() { managed::toggle_float(h as isize); } }),
        Some(Commands::SwapNext) => run_or_direct(p, "swap-next", || { let h = win32::foreground(); if !h.is_null() { layout::swap_next(h); } }),
        Some(Commands::Promote) => run_or_direct(p, "promote", || { let h = win32::foreground(); if !h.is_null() { layout::promote(h); } }),
        Some(Commands::Park) => run_or_direct(p, "park", || { let h = win32::foreground(); if !h.is_null() { scratchpad::park(h); } }),
        Some(Commands::Summon) => run_or_direct(p, "summon", || { scratchpad::summon(); }),
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
// ---------------------------------------------------------------------------
// Flow Launcher plugin protocol ("Language": "executable")
//
// Flow runs `mrspace.exe flow <json-rpc-request>`:
//   query          -> list of results to render (filtered mrspace actions)
//   ms.exec        -> (our custom action) run the command through the daemon
// Everything else -> empty result set.
// Only stdout may carry the JSON response; diagnostics go to stderr.
// ---------------------------------------------------------------------------

type FlowValue = serde_json::Value;

struct FlowResult {
    title: String,
    subtitle: String,
    score: i32,
    cmd: Option<String>, // None => informational result
}

const FLOW_ACTIONS: &[(&str, &str, &str)] = &[
    // (query title, subtitle, daemon command)
    ("Pin Left", "Foreground → left half of the work area", "pin-left"),
    ("Pin Right", "Foreground → right half of the work area", "pin-right"),
    ("Pin Top", "Foreground → top half of the work area", "pin-top"),
    ("Pin Bottom", "Foreground → bottom half of the work area", "pin-bottom"),
    ("Maximize", "Foreground → full work area", "maximize"),
    ("Fit All (grid)", "Tile all visible windows in an even grid", "fit-all"),
    ("Layout: grid", "Same as fit-all", "layout grid"),
    ("Layout: 2 columns", "Two equal columns", "layout columns-2"),
    ("Layout: 3 columns", "Three equal columns", "layout columns-3"),
    ("Layout: 2 rows", "Two equal rows", "layout rows-2"),
    ("Layout: main-left", "Foreground big on the left, others stacked right", "layout main-left"),
    ("Layout: main-right", "Foreground big on the right, others stacked left", "layout main-right"),
    ("Layout: main-top", "Foreground big on top, others stacked below", "layout main-top"),
    ("Layout: main-bottom", "Foreground big at bottom, others stacked above", "layout main-bottom"),
    ("Toggle Auto-Tiling", "Re-tile automatically when windows open/close", "toggle-tiling"),
    ("Cycle Layouts", "grid → 2 cols → 3 cols → 2 rows → main-* → focus", "layout-cycle"),
    ("Focus Left", "Switch focus to the nearest window on the left", "focus-left"),
    ("Focus Down", "Switch focus to the nearest window below", "focus-down"),
    ("Focus Up", "Switch focus to the nearest window above", "focus-up"),
    ("Focus Right", "Switch focus to the nearest window on the right", "focus-right"),
    ("Next Virtual Desktop", "Switch desktop view (native animation)", "next-desktop"),
    ("Prev Virtual Desktop", "Switch back one desktop", "prev-desktop"),
    ("Move Window to Next Desktop", "Foreground → next desktop", "move-next-desktop"),
    ("Move Window to Prev Desktop", "Foreground → previous desktop", "move-prev-desktop"),
    ("Resize Mode", "Nudge the foreground window with arrows; Esc exits", "resize-mode"),
    ("Grow Window", "Resize-mode: grow the edge under the cursor", "grow"),
    ("Shrink Window", "Resize-mode: shrink the edge under the cursor", "shrink"),
    ("Open Palette", "The built-in mrspace palette (gpui)", "palette"),
    ("Settings", "Open the mrspace settings window", "settings"),
];

fn flow_protocol(rpc: &str) {
    let req: FlowValue = match serde_json::from_str(rpc) {
        Ok(v) => v,
        Err(e) => { eprintln!("bad rpc: {e}"); return; }
    };
    let method = req.get("method").and_then(FlowValue::as_str).unwrap_or("");
    let params = req.get("parameters").and_then(FlowValue::as_array).cloned().unwrap_or_default();

    let results: Vec<serde_json::Value> = match method {
        "query" => {
            let q = params.first().and_then(FlowValue::as_str).unwrap_or("");
            flow_query(q)
        }
        "ms.exec" => {
            let cmd = params
                .first()
                .and_then(FlowValue::as_object)
                .and_then(|o| o.get("command"))
                .and_then(FlowValue::as_str)
                .unwrap_or("")
                .to_string();
            flow_exec(&cmd);
            Vec::new()
        }
        _ => Vec::new(),
    };
    if method.starts_with("ms.") {
        // custom actions don't return results to render
        println!("{{}}");
        return;
    }
    println!("{}", serde_json::json!({ "result": results }));
}

fn flow_ipc(cmd: &str) -> String {
    let port = crate::config::Config::default().ipc_port;
    crate::daemon::send_command(port, cmd)
}

fn flow_exec(cmd: &str) {
    flow_ipc(cmd);
}

fn flow_query(q: &str) -> Vec<serde_json::Value> {
    // per-window mode: `win ...` or `w ...` — list every manageable window,
    // each carrying its hwnd-already-baked-in layout commands (dispatch
    // accepts "pin-left <hwnd>" etc.). Otherwise show the flat action list.
    if let Some(rest) = q.strip_prefix("win").or_else(|| q.strip_prefix("w ")) {
        let rest = rest.trim();
        let mut out: Vec<FlowResult> = Vec::new();
        if let Some(items) = flow_context_windows() {
            for r in items {
                if rest.is_empty() || fuzzy_match(&r.title, rest) {
                    out.push(r);
                }
            }
        }
        return flow_results(out);
    }

    let mut out: Vec<FlowResult> = Vec::new();
    for (title, sub, cmd) in FLOW_ACTIONS.iter().copied() {
        if q.is_empty() || fuzzy_match(title, q) || fuzzy_match(cmd, q) {
            out.push(FlowResult {
                title: title.to_string(),
                subtitle: sub.to_string(),
                score: if q.is_empty() { 100 - out.len() as i32 } else { 90 },
                cmd: Some(cmd.to_string()),
            });
        }
    }
    // trailing results that query the window list for a specific target
    out.push(FlowResult {
        title: "Tile a specific window…".into(),
        subtitle: "Type `w` (or more) to pick a window, then choose its layout".into(),
        score: 10,
        cmd: None,
    });
    flow_results(out)
}

fn flow_results(items: Vec<FlowResult>) -> Vec<serde_json::Value> {
    items
        .into_iter()
        .filter_map(|r| {
            let cmd = r.cmd?;
            let action = serde_json::json!({
                "method": "ms.exec",
                "parameters": [{ "command": cmd }],
                "dontHideAfterAction": false
            });
            Some(serde_json::json!({
                "Title": r.title,
                "SubTitle": r.subtitle,
                "IcoPath": "Images/icon.png",
                "Score": r.score,
                "JsonRPCAction": action,
            }))
        })
        .collect()
}

/// Enumerate manageable windows and produce per-window layout results with
/// the hwnd baked into the daemon command ("pin-left <hwnd>", ...).
/// None => daemon unreachable.
fn flow_context_windows() -> Option<Vec<FlowResult>> {
    let listing = flow_ipc("list-windows");
    let mut out = Vec::new();
    for line in listing.lines() {
        let mut it = line.splitn(3, '|');
        let hwnd = it.next().unwrap_or("0").to_string();
        let pid = it.next().unwrap_or("?").to_string();
        let title = it.next().unwrap_or("").to_string();
        if title.is_empty() || hwnd == "0" {
            continue;
        }
        for (dir, label) in [("pin-left", "Left Half"), ("pin-right", "Right Half"), ("pin-top", "Top Half"), ("pin-bottom", "Bottom Half"), ("maximize", "Full Work Area")] {
            out.push(FlowResult {
                title: format!("{} — {}", label, title),
                subtitle: format!("Pin this specific window (hwnd {hwnd}, pid {pid})"),
                score: 70,
                cmd: Some(format!("{dir} {hwnd}")),
            });
        }
    }
    Some(out)
}

/// Fuzzy match: case-insensitive substring on the title; whitespace in the
/// needle is collapsed so multi-word queries behave predictably.
fn fuzzy_match(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let h = haystack.to_lowercase();
    let n: String = needle.split_whitespace().collect::<String>().to_lowercase();
    h.contains(&n)
}
