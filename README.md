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

Hold the **Win key**, then tap an action key (chord mode — no need to re-press modifiers):

`←` pin left, `→` pin right, `↑` fit-all, `↓` maximize, `Space` command palette.

Tapping Win alone still opens the Start menu, and unmapped Win-combos (Win+D, Win+E…) still work — only mapped action keys are swallowed. Edit `%APPDATA%\mrspace\config.json`.

## Architecture

- `src/win32.rs` — Win32 window enumeration, move/resize, monitor work areas.
- `src/virtual_desktop.rs` — COM `IVirtualDesktopManager` (move windows across native virtual desktops).
- `src/layout.rs` — on-demand layouts (pin halves, maximize, auto-fit grid).
- `src/hook.rs` — hold-Win chord engine via `WH_KEYBOARD_LL` low-level hook.
- `src/borders.rs` — accent/muted border overlays for laid-out windows (layered topmost windows).
- `src/hotkeys.rs` — global hotkeys via `RegisterHotKey` + message loop.
- `src/daemon.rs` — background daemon, hotkey thread, TCP IPC server (127.0.0.1:17331).
- `src/palette.rs` — command palette built with **gpui** (GPU-rendered, separate process).
- `src/settings.rs` — native settings desktop app (own process; edits the whole config: port, timing, exclusions, borders, full hotkey table).
- `src/main.rs` — CLI entry + Flow Launcher JSON-RPC bridge (`flow` hidden subcommand).
- `src/settings.rs` — minimal Win32 settings window.
- `src/config.rs` — JSON config.

## Building the palette (gpui)

The gpui Windows build compiles HLSL shaders at build time and needs `fxc.exe` from the Windows 10 SDK. The repo's `.cargo/config.toml` points `GPUI_FXC_PATH` at `D:\WindowsKits\bin\10.0.26100.0\x64\fxc.exe` — adjust if your SDK is elsewhere.

## Settings app

`Win+Space` → `Settings` (palette), `mrspace settings`, or the Flow plugin's Settings row opens a native desktop window in its own process (single-instance: reopening raises the existing window). It edits **every** config.json field:

- IPC port, resize step, resize timeout, tiling interval
- managed-window exclusions (titles / classes / pids), one per line
- border overlays: enable toggle, thickness, accent & muted hex colors
- the full hotkey table: `name|modifiers|vk|action`, one per line (vk hex like `0x25`; modifiers decimal — 1=alt, 2=ctrl, 4=shift, 8=win)

Buttons: **Save** (writes config.json), **Save & Apply** (writes + tells the running daemon to re-read live options — borders change immediately; port/step/timeout take effect as they're used), **Open Config** (explorer at the config folder), **Cancel**.

## Flow Launcher plugin

Mr. Space ships as a native Flow Launcher plugin (`flow-plugin/`, installed under `%APPDATA%\FlowLauncher\Plugins\MrSpace\` — action keyword `ms`):

- `ms` alone — every mrspace action (pin halves, maximize, fit-all, layouts, focus, desktops, resize mode, palette, settings)
- `ms <filter>` — fuzzy filter over actions
- `ms w <title filter>` — per-window mode: pick a *specific* window and tile it (left/right/top/bottom half, full work area)

Actions run through the daemon's IPC, so the plugin works with the running daemon; the executable-plugin binary talks JSON-RPC on argv[1]/stdout (`mrspace.exe flow <request>`).

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