# Mr. Space — Manual Test Guide (shortcut-only)

How to exercise every feature using **keyboard shortcuts only** — no CLI. Hold **Win**, tap the key (Win+X). Some features need a one-time config edit in `%APPDATA%\mrspace\config.json` (noted). Restart the daemon after config edits.

Probe windows: use **mspaint** (classic Win32, never suspends). Avoid Win11 Notepad (store app suspends).

---

## Tier 1 — event-driven core

### 1.1 Event-driven re-tile engine
1. Launch 2 mspaint windows → both auto-tile into a 2-col grid within ~1 s.
2. Close one → the other expands to fill.
3. Launch 3 more → grid reflows.
4. Drag a window → no re-tile fights you (only count changes re-tile).

### 1.2 Workspace-scoped layout memory
1. Desktop 1: launch 2 mspaint, press **Win+O** a few times to land on "main-left" (windows split 2/3 + 1/3). Or **Win+G** for grid.
2. Switch desktop (Ctrl+Win+Right). Launch 1 mspaint on desktop 2, **Win+O** to cycle to a different layout.
3. Switch back to desktop 1 → windows re-arrange to the layout you left.
4. Switch to desktop 2 → windows re-arrange to its layout.

### 1.3 Managed-window state commands
- **Toggle float**: focus a window, press **Win+F** → window leaves the grid, others re-tile. Press **Win+F** again → window rejoins the grid.
- **Swap next**: **Win+S** → focused window swaps position with the next one.
- **Promote**: **Win+A** → focused window moves to the main (top-left) slot.

---

## Tier 2 — switch tools

### 2.1 Window rules engine
Edit `%APPDATA%\mrspace\config.json`, add to `window_rules`:
```json
{ "match_exe": "mspaint.exe", "action": "float" }
```
Restart daemon. Launch mspaint → it floats (not tiled). Change `"action"` to `"ignore"` → mspaint not managed at all (no border).

### 2.2 Scratchpad (park/summon)
1. Focus a window, press **Win+Y** → window disappears (no taskbar flash), others re-tile.
2. Press **Win+U** → window returns at its exact original rect.
3. Repeat park/summon → each summon restores the most recently parked.

### 2.3 Gaps
Edit config: `"gap_inner": 12, "gap_outer": 8`. Restart daemon, press **Win+G** → visible gaps between and around tiles. Set to 0 → flush (default).

### 2.4 Focus-follows-mouse
Edit config: `"focus_follows_mouse": true`. Restart daemon. Move the mouse into a tiled window → it gains focus. Set false → no focus change on mouse move.

---

## Tier 3 — polish

### 3.1 State snapshot
(No shortcut — verify via the Flow Launcher plugin or skip.)

### 3.3 Binding modes
- **Win+V** → enters "move" mode: arrow keys **move** the focused window (no resize). Press **Esc** → back to base.
- **Win+R** → toggles resize mode (existing): arrows nudge size.

---

## Border overlay (separate fix)

### Stale "middle line"
1. Tile 2 windows, drag one to a new position (without resizing) → the ring follows; no stale line left behind.

### Fullscreen corner marks
1. Maximize a window (Win+Up) → its border ring disappears.
2. Restore (Win+Down) → ring returns.

---

## All shortcuts (Win + key)

| Key | Action |
|---|---|
| Arrows | pin-left/right/top/bottom |
| M | maximize |
| G | fit-all (grid) |
| T | toggle auto-tiling |
| O | cycle layouts |
| H/J/K/L | focus left/down/up/right |
| R | resize mode (toggle) |
| V | move mode (enter) |
| N / P | next / prev desktop |
| 1–9 | switch to desktop N |
| F | toggle float |
| S | swap with next window |
| A | promote to main slot |
| Y | park (scratchpad) |
| U | summon (scratchpad) |
| Space | command palette |
| Esc | exit resize/move mode |
| + / - | grow / shrink (resize mode) |

## Config location
`%APPDATA%\mrspace\config.json` (restart daemon after edits).