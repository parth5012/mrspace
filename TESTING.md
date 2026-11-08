# Mr. Space (Windows) — End-to-End Test Guide

Test everything: daemon, hotkeys, layouts, focus, resize, palette, tiling, desktops, exclusions, and the agent CLI.

**Shortcut modifier is the `Win` key.**

**Chord mode**: the Win key is tracked by a low-level keyboard hook (`WH_KEYBOARD_LL`) — you don't press `Win+K` as a combo; you *hold* Win, then tap as many action keys as you want, then release. Tapping Win alone opens the Start menu as usual; only mapped action keys are intercepted while Win is held.

## 0. Prep (one-time per test session)

1. Close/kills competing window managers so nothing fights mrspace:
   ```powershell
   taskkill /F /IM komorebi.exe; taskkill /F /IM komorebi-bar.exe; taskkill /F /IM whkd.exe
   ```
2. Kill any old mrspace instance:
   ```powershell
   taskkill /F /IM mrspace.exe
   ```
3. Start the daemon (hotkeys + IPC live here):
   ```powershell
   cd D:\data\.t3\scratch\mr-space-win
   target\release\mrspace.exe daemon
   ```
   Expect: `mrspace daemon listening on 127.0.0.1:17331` and a hotkey-registration warning **only** if a key is already taken globally.
4. Open **2–4 normal app windows** (e.g., Zen Browser, VS Code, Notepad). Keep Rainmeter widgets visible — they should never be touched.

## 1. Sanity — CLI works

```powershell
target\release\mrspace.exe list-windows   # shows only real app windows, no Rainmeter .ini titles
target\release\mrspace.exe exec "debug-windows"  # shows class/style per window (diagnostics)
```
✅ Expected: Rainmeter skins (`class=RainmeterMeterWindow`) appear in debug output but `manageable=false`.

## 2. Pinning (on-demand, not forced)

Fore­ground each app and press:

| Keys | Expected |
| --- | --- |
| `Win+←` | Window snaps to left half, work-area height |
| `Win+→` | Right half |
| `Win+↑` | Top half |
| `Win+↓` | Bottom half |
| `Win+M` | Fills whole work area (not Windows-maximize) |

✅ Expected: window moves once; nothing else moves. Rainmeter unaffected.

## 3. Named layouts

| Keys / command | Expected |
| --- | --- |
| `target\release\mrspace.exe layout grid` | all app windows in equal grid |
| `... layout columns-2` | two equal columns |
| `... layout columns-3` | three equal columns |
| `... layout rows-2` | two equal rows |
| `... layout main-left` | foreground ≈2/3 left, others stacked right |
| `... layout main-right` | mirrored |
| `... layout main-top` / `main-bottom` | horizontal variant |
| `... layout focus` | foreground only fills screen |
| `Win+O` | cycles: grid → columns-2 → columns-3 → rows-2 → main-left → … → focus → grid |

✅ Expected: exactly one rearrange per command; pressing `Win+O` repeatedly walks the list.

**Border highlighting** (new): after any pin/fit-all/preset/tile command, every arranged window gets a thin **grey** ring; the **focused** window's ring turns **purple** (following focus automatically). Rings are click-through and die with the window.
1. Run `win+fit-all` → all tiles have grey rings.
2. Click around the tiled windows → the clicked window's ring turns purple, others grey.
3. Close a window → its ring disappears (watchdog tick, ≤200ms).
4. `Win+T` (tiling off) → all rings removed.

## 4. Directional focus (geometric nearest)

Let each app sit somewhere different (no grid), then press with cursor over one app:

| Keys | Expected |
| --- | --- |
| `Win+H` | focus switches to the window to the **left** (center-to-center) |
| `Win+J` | below |
| `Win+K` | above |
| `Win+L` | to the **right** |

✅ Expected: focus lands on the *nearest* window in that direction; edge case = nothing better exists (no change or no-op).

## 5. Resize mode

1. Press `Win+R` → expect a small **"RESIZE MODE (Esc to exit)"** hint appears top-left.
2. Nudge with `Win+←/→/↑/↓` (10px steps).
3. Grow/shrink `Win+=` / `Win+-`.
4. Press `Esc` → hint disappears, mode ends.
5. Press `Win+R`, then **do nothing for 5s** → hint disappears (auto-exit).
6. While resize mode is ON, note that the same arrow keys nudge — **pinning is suspended** until mode exits.

## 6. Command palette (gpui, separate process)

1. Press `Win+Space` → dark popup (560×420, centered) opens **in the foreground** and stays above other windows.
2. Type `main` (plain typing, **no shift needed** — regression: an early build only accepted input while Shift was held) → filters to `main-left/main-right/main-top/main-bottom`; arrows move the highlight, clicks also run. Backspace edits the query normally.
3. Press `Enter` → runs selection, palette closes.
4. `Esc` closes with no action.
5. While the palette is open, the daemon keeps responding: `mrspace exec "current-desktop"` returns immediately (palette is its own process, IPC unaffected).
6. Press `Win+Space` again while open → **no second instance**; the open palette is raised instead (single-instance).

## 6b. Settings app (native, own process)

Open it via the palette's `Settings` row, `mrspace settings`, or the Flow plugin's Settings row.

1. Window opens **in its own process** (2nd mrspace process); daemon stays responsive (`mrspace exec "current-desktop"` returns while it's open).
2. Open it a second time → no duplicate; the running window is raised instead.
3. Change border thickness to `6`, click **Save & Apply** → status shows `daemon reloaded`; run `mrspace exec "fit-all"` → rings render at 6px. Set back to `3` when done.
4. Uncheck **Enable** under Borders, **Save & Apply** → existing rings disappear; re-enable + Apply → they come back on the next layout command.
5. Add a bogus entries: non-numeric port or a hotkey line `pin-left|3|0x25` (missing the 4th field) → **Save** shows `ERR: ...` and nothing is written.
6. Edit exclusions: add a window title, **Save & Apply** → that window stops being managed (exclusions are re-read on every manage decision).
7. `Open Config` button → explorer opens at `%APPDATA%\mrspace\`.
8. Hotkey table edits are honored after daemon restart (hotkeys are registered at startup; a `#` reload of them is not live).

## 6c. Flow Launcher plugin (preferred control surface)

Installed at `%APPDATA%\FlowLauncher\Plugins\MrSpace\` (same `mrspace.exe` is copied there).

1. `Flow Settings → Plugin` → reload plugin data (or press the reload button) — `Mr. Space` should show as loaded.
2. Open Flow, type `ms` → all mrspace actions rendered (Pin Left/Right/Top/Bottom, Maximize, layouts, focus, desktops, palette…).
3. `ms lay` → filters to `Layout:*` rows; `Enter` on `Layout: 2 columns` applies it via the daemon.
4. `ms w` → every manageable window listed as five rows each (Left/Right/Top/Bottom Half, Full Work Area); type partial title to filter; `Enter` pins *that exact window* (hwnd is baked into the command).
5. Without the daemon running, actions no-op silently (plugin prints nothing to stderr unless the request itself is malformed).

## 7. Toggle-tiling (auto-fit when you want it)

1. With 3 windows open, press `Win+T` → grid applied **once**.
2. Move/resize a window freely → **gets left alone** (no re-tile on a timer).
3. Open a new window (launch Notepad) → all re-tile automatically (count changed).
4. Close Notepad → re-tiles again.
5. Press `Win+T` again → auto-tiling OFF; opening windows no longer rearranges anything.

## 8. Virtual desktops

1. `target\release\mrspace.exe exec "current-desktop"` → prints a GUID.
2. `Win+N` / `Win+P` → switches to next/prev desktop (Windows native animation).
3. `target\release\mrspace.exe exec "move-next-desktop"` → foreground window moves to the next desktop (creates a temp window to read the desktop GUID — you should see no flicker).
4. `Win+1..9` → moves foreground to desktop N (best-effort; needs the desktop to exist).

## 9. Alt/modifier lockout check

1. Open any app, press plain `Alt` → does the in-app menu bar react normally on first press? (yes = good)


## 10. Agent / automation interface

```powershell
target\release\mrspace.exe exec "pin-left"
target\release\mrspace.exe exec "fit-all"
target\release\mrspace.exe exec "toggle-tiling"
```
✅ Expected: `OK`, and visible effect identical to the hotkey. These are the hooks AI agents (Claude Code etc.) can call.

## 11. Troubleshooting

| Symptom | Cause | Fix |
| --- | --- | --- |
| Hotkey does nothing | daemon not running, or key taken | restart daemon; read the warning line for the name(s) that failed |
| Hotkey does something weird in a browser/editor | collision with that app's own `Win+X` | remap in `%APPDATA%\mrspace\config.json` |
| Run dialog (`Win+R`) and Lock (`Win+L`) don't fire | mrspace claims these keys: `R` = resize-mode, `L` = focus-right | remapped to pass-thru in `src/hook.rs` if you prefer OS behavior |
| Stuck modifier keys | shouldn't happen (we synthesize key-ups) | if it does, report — known gap |
| Rainmeter panel got tiled | older build | ensure config has `exclude_classes: ["RainmeterMeterWindow"]`, restart daemon |
| `.ini` titles in `list-windows` | Rainmeter skins | they're filtered now; verify with `debug-windows` |
| Anything broken | | `taskkill /F /IM mrspace.exe` restores normal desktop instantly |

## 12. Clean shutdown

```powershell
taskkill /F /IM mrspace.exe
```
