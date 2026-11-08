# Mr. Space (Windows) — End-to-End Test Guide

Test everything: daemon, hotkeys, layouts, focus, resize, palette, tiling, desktops, exclusions, and the agent CLI.

**Shortcut modifier is `Ctrl+Alt`** (not Ctrl+Shift, which conflicts with common app shortcuts).

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
| `Ctrl+Alt+←` | Window snaps to left half, work-area height |
| `Ctrl+Alt+→` | Right half |
| `Ctrl+Alt+↑` | Top half |
| `Ctrl+Alt+↓` | Bottom half |
| `Ctrl+Alt+M` | Fills whole work area (not Windows-maximize) |

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
| `Ctrl+Alt+O` | cycles: grid → columns-2 → columns-3 → rows-2 → main-left → … → focus → grid |

✅ Expected: exactly one rearrange per command; pressing `Ctrl+Alt+O` repeatedly walks the list.

## 4. Directional focus (geometric nearest)

Let each app sit somewhere different (no grid), then press with cursor over one app:

| Keys | Expected |
| --- | --- |
| `Ctrl+Alt+H` | focus switches to the window to the **left** (center-to-center) |
| `Ctrl+Alt+J` | below |
| `Ctrl+Alt+K` | above |
| `Ctrl+Alt+L` | to the **right** |

✅ Expected: focus lands on the *nearest* window in that direction; edge case = nothing better exists (no change or no-op).

## 5. Resize mode

1. Press `Ctrl+Alt+R` → expect a small **"RESIZE MODE (Esc to exit)"** hint appears top-left.
2. Nudge with `Ctrl+Alt+←/→/↑/↓` (10px steps).
3. Grow/shrink `Ctrl+Alt+=` / `Ctrl+Alt+-`.
4. Press `Esc` → hint disappears, mode ends.
5. Press `Ctrl+Alt+R`, then **do nothing for 5s** → hint disappears (auto-exit).
6. While resize mode is ON, note that the same arrow keys nudge — **pinning is suspended** until mode exits.

## 6. Command palette

1. Press `Ctrl+Alt+Space` → palette window opens with all actions listed.
2. Type `main` → filters to `main-left/main-right/main-top/main-bottom`.
3. Press `Enter` → runs selection, palette closes.
4. `Esc` closes with no action.

## 7. Toggle-tiling (auto-fit when you want it)

1. With 3 windows open, press `Ctrl+Alt+T` → grid applied **once**.
2. Move/resize a window freely → **gets left alone** (no re-tile on a timer).
3. Open a new window (launch Notepad) → all re-tile automatically (count changed).
4. Close Notepad → re-tiles again.
5. Press `Ctrl+Alt+T` again → auto-tiling OFF; opening windows no longer rearranges anything.

## 8. Virtual desktops

1. `target\release\mrspace.exe exec "current-desktop"` → prints a GUID.
2. `Ctrl+Alt+N` / `Ctrl+Alt+P` → switches to next/prev desktop (Windows native animation).
3. `target\release\mrspace.exe exec "move-next-desktop"` → foreground window moves to the next desktop (creates a temp window to read the desktop GUID — you should see no flicker).
4. `Ctrl+Alt+1..9` → moves foreground to desktop N (best-effort; needs the desktop to exist).

## 9. Alt/modifier lockout check

1. Open any app, press plain `Alt` → does the in-app menu bar react normally on first press? (yes = good)
2. Trigger any mrspace hotkey, then immediately type `Alt+F4`-free text in an app → no stuck Alt/Ctrl/Shift.

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
| Hotkey does something weird in a browser/editor | collision with that app's own `Ctrl+Alt+X` | remap in `%APPDATA%\mrspace\config.json` |
| Stuck modifier keys | shouldn't happen (we synthesize key-ups) | if it does, report — known gap |
| Rainmeter panel got tiled | older build | ensure config has `exclude_classes: ["RainmeterMeterWindow"]`, restart daemon |
| `.ini` titles in `list-windows` | Rainmeter skins | they're filtered now; verify with `debug-windows` |
| Anything broken | | `taskkill /F /IM mrspace.exe` restores normal desktop instantly |

## 12. Clean shutdown

```powershell
taskkill /F /IM mrspace.exe
```
