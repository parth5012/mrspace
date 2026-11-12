# Mr. Space — Manual Test Guide

How to exercise every feature. Assumes the daemon is running (`mrspace daemon`) and tiling is ON (`mrspace toggle-tiling`). IPC commands are invoked as `mrspace <command>` (they talk to the daemon on port 17331). Hotkeys are **Win + key** (hold Win, tap the key).

Probe windows: use **mspaint** (classic Win32, never suspends) or a PowerShell console — avoid Win11 Notepad (store app, suspends and breaks probing).

---

## Tier 1 — event-driven core

### 1.1 Event-driven re-tile engine
1. `mrspace toggle-tiling` → "tiling ON".
2. Launch 2 mspaint windows → both auto-tile into a 2-col grid within ~1 s.
3. Close one → the other expands to fill.
4. Launch 3 more → grid reflows (2×2 then 3 cols).
5. Drag a window → no re-tile fights you (only count changes re-tile).
6. Idle CPU: `powershell -c "(Get-Process mrspace).CPU"` twice 2 s apart → delta ≈ 0.

### 1.2 Workspace-scoped layout memory
1. Desktop 1: launch 2 mspaint, `mrspace layout main-left` (records main-left).
2. Switch desktop (Ctrl+Win+Right). Launch 1 mspaint on desktop 2, `mrspace layout columns-2`.
3. Switch back to desktop 1 → windows re-arrange to main-left (trace "desk switch: layout restored").
4. Switch to desktop 2 → windows re-arrange to columns-2.
5. Verify windows on the *other* desktop are NOT tiled (cloaked filter).

### 1.3 Managed-window state commands
- **Toggle float**: focus a window, press **Win+F** → window leaves the grid, others re-tile. Press **Win+F** again → window rejoins the grid. (`mrspace toggle-float`)
- **Swap next**: **Win+S** → focused window swaps position with the next one.
- **Promote**: **Win+A** → focused window moves to the main (top-left) slot.

---

## Tier 2 — switch tools

### 2.1 Window rules engine
Add to `%APPDATA%\mrspace\config.json` under `window_rules`, e.g.:
```json
{ "match_exe": "mspaint.exe", "action": "float" }
```
Reload: `mrspace reload-config`. Launch mspaint → it floats (not tiled). Change action to `"ignore"` → mspaint not managed at all (no border).

### 2.2 Scratchpad (park/summon)
1. Focus a window, `mrspace park` → window disappears (no taskbar flash), others re-tile.
2. `mrspace summon` → window returns at its exact original rect.
3. Repeat park/summon 3 times → each summon restores the most recently parked.

### 2.3 Gaps
Edit config: `"gap_inner": 12, "gap_outer": 8`. `mrspace reload-config`, `mrspace fit-all` → visible gaps between and around tiles. Set to 0 → flush (default).

### 2.4 Focus-follows-mouse
Set `"focus_follows_mouse": true` in config, `mrspace reload-config`. Move the mouse into a tiled window → it gains focus. Set false → no focus change on mouse move.

---

## Tier 3 — polish

### 3.1 State snapshot
`mrspace state` → valid JSON with hwnd/pid/title/class/exe/rect/floating for every managed window.

### 3.3 Binding modes
- `mrspace exec "enter-mode move"` → mode = move. Arrow keys now **move** the focused window (Win+arrow). Press Esc (or `mrspace exec "enter-mode base"`) → back to base.
- Resize mode: **Win+R** (toggles) → arrows nudge size (existing resize mode).

---

## Border overlay (separate fix)

### Stale "middle line"
1. Tile 2 windows, drag one to a new position (without resizing) → the ring follows; no stale line left behind.

### Fullscreen corner marks
1. Maximize a window (Win+Up or `mrspace maximize`) → its border ring disappears.
2. Restore → ring returns.
3. Fullscreen app (F11) → no corner marks.

---

## Useful commands

| Command | Purpose |
|---|---|
| `mrspace daemon` | start daemon |
| `mrspace toggle-tiling` | toggle auto-tiling |
| `mrspace fit-all` | tile all manageable windows |
| `mrspace layout <name>` | apply named layout (grid, columns-2, main-left, …) |
| `mrspace layout-cycle` | cycle layouts |
| `mrspace state` | JSON snapshot |
| `mrspace park` / `summon` | scratchpad |
| `mrspace toggle-float` | float/unfloat foreground |
| `mrspace swap-next` / `promote` | reorder |
| `mrspace reload-config` | reload config (borders etc.) |
| `mrspace toggle-tiling` | on/off |

## Config location
`%APPDATA%\mrspace\config.json`

## Debug tracing (temporary, remove after soak)
`MRS_TRACE=/tmp/h.log mrspace daemon` → logs hook registration, re-tile wake/count/retile cycles, and border tick details. No-op when unset.