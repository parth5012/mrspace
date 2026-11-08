# Mr. Space — Windows (Rust)

A Windows port of the on-demand window-manager idea behind Scott Tolinski's macOS **Mr. Space**: instead of forcing every window into a tiled grid (like AeroSpace), you trigger layouts on demand — pin left/right/top/bottom, maximize, auto-fit all, and route windows across Windows Virtual Desktops.

Built in Rust on raw Win32 + `IVirtualDesktopManager`. No forced tiling. Runs as a background daemon with global hotkeys and a localhost IPC socket so both you and AI coding agents can drive it.

## Build

```sh
cargo build --release
# binary: target/release/mrspace.exe
```

Requires Rust 1.80+ and Windows 10/11.

## Usage

```sh
# Start the daemon (hotkeys + IPC server)
mrspace daemon

# Or just run `mrspace` with no args; it starts the daemon if not running.

# Commands (work via daemon IPC if running, else directly)
mrspace pin-left          # pin foreground window to left half
mrspace pin-right         # right half
mrspace pin-top           # top half
mrspace pin-bottom        # bottom half
mrspace maximize          # fill work area
mrspace fit-all           # auto-tile all visible windows into a grid
mrspace list-windows      # enumerate manageable windows
mrspace current-desktop   # GUID of foreground window's virtual desktop
mrspace send-to <GUID>    # move foreground window to that virtual desktop
mrspace settings          # open settings window
mrspace config-path       # print config file location

# Agent-friendly: run any command string against the daemon
mrspace exec "pin-left"
```

## Default hotkeys

`Ctrl+Shift+Left` pin left, `Ctrl+Shift+Right` pin right, `Ctrl+Shift+Up` fit-all, `Ctrl+Shift+Down` maximize. Edit `%APPDATA%\mrspace\config.json`.

## Architecture

- `src/win32.rs` — Win32 window enumeration, move/resize, monitor work areas.
- `src/virtual_desktop.rs` — COM `IVirtualDesktopManager` (move windows across native virtual desktops).
- `src/layout.rs` — on-demand layouts (pin halves, maximize, auto-fit grid).
- `src/hotkeys.rs` — global hotkeys via `RegisterHotKey` + message loop.
- `src/daemon.rs` — background daemon, hotkey thread, TCP IPC server (127.0.0.1:17331).
- `src/settings.rs` — minimal Win32 settings window.
- `src/config.rs` — JSON config.

## Agent integration

The daemon exposes a line-based TCP protocol on `127.0.0.1:17331`. Send a command name (optionally with args) followed by newline; receive the result. This lets AI agents and scripts move/arrange windows without a GUI. Example (PowerShell):

```powershell
$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1",17331)
$stream = $client.GetStream()
$w = New-Object System.IO.StreamWriter($stream); $w.WriteLine("fit-all"); $w.Flush()
$r = New-Object System.IO.StreamReader($stream); $r.ReadLine()
```

## Notes

- Virtual desktop enumeration beyond moving windows is not exposed by the public `IVirtualDesktopManager` interface; `list-desktops` returns a notice. Creating/switching desktops requires undocumented interfaces.
- Auto-start at login is not implemented (run `mrspace daemon` via Task Scheduler or Startup folder).
EOF
echo done