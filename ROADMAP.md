# Mr. Space â Feature Roadmap

Grounded against komorebi (README surveyed 2026-11), GlazeWM v3 (README surveyed 2026-11),
Workspacer, bug.n and PowerToys FancyZones. RAM is the design constraint: every feature
must pay for itself in wakeups, not cost too much in memory.

## Baseline (where we are)

- 9 on-demand layouts (grid, columns-2/3, rows-2, main-left/right/top/bottom, focus) + cycle
- Pin-tiling with count-diff re-tile (1 s poller), focus walking, chord resize-mode
- Virtual-desktop nav (next/prev/move), border overlays (accent/muted), exclusions
- Win-key chord engine, palette, settings GUI, TCP IPC (port 17331), Flow Launcher plugin
- Footprint: **~16 MB daemon** (+20 MB while the settings app is open)

| Capability | Mr. Space | komorebi | GlazeWM v3 | FancyZones |
|---|---|---|---|---|
| Tiling model | re-pin on count change | event-driven tree | i3-style tree | static zones |
| Window rules (launch â float/ignore/assign) | â | â | â | â |
| Float / fullscreen / minimized states | â | â | â | â |
| Per-workspace layout memory | â | â | â | n/a |
| Gaps inner/outer | â | â | â | â |
| Focus follows mouse | â | â | â | â |
| Scratchpad (summon/park) | â | â | â | â |
| Border overlays | â | â | â (Win11) | â |
| Win-key chords | â native | external (whkd) | external | â |
| Built-in palette | â | â | â | â |
| Settings GUI | â | YAML+CLI | YAML+CLI | partial |
| Event subscriptions for bar widgets | â | â pipes/UDS | â CLI | â |
| RAM | **~16 MB** | ~30â60 MB | ~50â100 MB | heavy runtime |

## Hard rules (RAM guardrails)

1. No idle polling for anything â event-driven only.
2. State = flat Vec of window records per workspace (hwnd, class, rect, state). No serde
   tree per event, no per-window caches beyond what is *managed* (<5 KB typical).
3. No animations, no scripting runtime, no bundled bar. Bar-style integrations consume
   our IPC events *externally* (komorebi's model).
4. One process stays cheap: `mrspace.exe` keeps single-digit-MB private memory as the
   feature set grows; the settings app is the only fat process and only lives on demand.

## Tier 1 â event-driven core

### 1. SetWinEventHook re-tile engine *(status: done)*
Replace the 1 s polling thread in `tiling.rs` with `SetWinEventHook`
(`EVENT_OBJECT_CREATE/DESTROY/SHOW/HIDE`, `EVENT_SYSTEM_MINIMIZESTART/END`,
`EVENT_OBJECT_CLOAKED/UNCLOAKED`, `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS`). Hook
thread pumps a `GetMessageW`+`DispatchMessageW` loop; events flip a `DIRTY` flag and
signal an auto-reset event that a re-tiler thread waits on (`WaitForSingleObject`,
zero idle wakeups) with a 150 ms debounce.
**Bugs found and fixed while landing this (both would hang the daemon):**
- `GetWindowTextW` *sends* `WM_GETTEXT` cross-process â a suspended UWP/notepad window
  thread blocks it forever â replaced with `SendMessageTimeoutW(WM_GETTEXT,
  SMTO_ABORTIFHUNG|SMTO_BLOCK, 300ms)` in `win32::window_title`.
- `SetWindowPos`/`MoveWindow` wait synchronously on the target window's thread during
  window destroy bursts â `SWP_ASYNCWINDOWPOS` in `win32::move_resize`.
- Acceptance (verified): opening notepad auto-tiles it into a grid cell within ~2 s,
  closing re-tiles the remainder, no perceived lag; traced wake/debounce/count cycle in
  `/tmp/h.log` (MRS_TRACE); idle CPU delta â 0 for 2 s; daemon RAM ~32 MB working set
  while tiling is ON; tiling toggled back OFF after the soak.

### 2. Workspace-scoped layout memory *(status: done)*
- `workspace.rs` (new): flat Vec<(GUID, DeskState)> capped at 8, FIFO eviction.
- `is_cloaked` filter added to `win32::is_manageable` (DWMWA_CLOAKED)  windows on
  other desktops excluded from tiling/counting/borders.
- Re-tiler detects desktop switches via GUID hash on the same event wake (cloak +
  foreground); on switch re-applies the remembered layout, or leaves geometry
  untouched when none is remembered. Count-change re-tiles apply stored layout or
  grid fallback. `note_layout` recorded by `layouts::apply` on success.
- `current_guid()` resolves via temp window on the active desktop (consistent
  identity across note/restore), cached 1000 ms.
- `VirtualDesktopManager` now created fresh per call (was a cross-apartment COM
  hazard that could crash the daemon).
- Note: `move-next-desktop`/`move-prev-desktop` cannot move foreign-process windows
  (IVirtualDesktopManager::MoveWindowToDesktop returns E_ACCESSDENIED)  Windows
  limitation, not a regression. Workspace memory does not depend on it.
- Acceptance: switch detection verified via MRS_TRACE ("desk switch: no stored
  layout"/"layout restored"); GUID consistency via temp-probe-only resolution.

### 3. Managed-window record + state commands (status: done - 2026-11-12)
- `managed.rs` (new): `FLOATING: Mutex<HashSet<isize>>`; `is_floating`, `toggle_float`,
  `clear`. `toggle_float` flips the flag and re-tiles + syncs borders.
- `win32::is_manageable` returns false for floating windows - excluded from tiling,
  counting, borders (left where the user put them).
- `layout::swap_next(hwnd)`: swaps rects of focused + next manageable window in z-order.
- `layout::promote(hwnd)`: re-tiles the whole set with focused window in slot 0.
- IPC + CLI + default hotkeys: toggle-float (F), swap-next (S), promote (A); wired in
  hook.rs `is_action_key` + `execute`.
- Float toggle round-trips: float -> window leaves tiling set; un-float -> re-joins via
  fit_all.
- Acceptance: toggle-float IPC round-trips OK; build clean.

### 4. Window rules engine *(config: `name|modifiers|vk|action` layer stays as-is)*
Match `title substring | class | exe name` per window â `float`, `ignore` (extends the
exclusion cards we have), `no-manage`. Evaluated once, inside the window-create path of
the hook. komorebi/GlazeWM issue-trackers' #1 feature; cheap for us.
- Acceptance: rules apply on launch (not only on next re-tile); settings GUI can manage.

### 5. Scratchpad (summon/park)
`Win+Space`-adjacent chord: park the focused window offscreen (restorable without
losing z-order semantics), summon any parked window. Needs only the window record
from item 3 + `SetWindowPos`.
- Acceptance: parked window invisible with no taskbar flash; summon restores exact
  geometry; parked set survives daemon restart via config on demand (optional flag).

### 6. Gaps inner/outer
`fit_all()` arithmetic resourced from config: `gap_inner`, `gap_outer` px. Settings GUI
fields in Basics.
- Acceptance: visible at all 9 layouts; â¤0 results clamp to 0; parse errors fall back
  to zero.

### 7. Focus niceties
`focus_follows_mouse` (option in Basics) + `toggle-back-previous-desktop` (i3
back-and-forth). Both O(1) and hook-cheap.
- Acceptance: no focus lag loop when the pointer crosses a tiled window boundary;
  previous-desktop toggle works even when the previous desktop has no windows.

## Tier 3 â polish without bloat

### 8. `state --json` + event subscriptions (IPC)
`mrspace.exe state --json` (flat state snapshot) + `mrspace.exe subscribe --port N`
(newline-delimited JSON event stream over the existing TCP IPC). Lets bar integrations exist
*outside* the process â komorebi/YASB model, ~zero RAM cost for us.
- Acceptance: state JSON â¤ 2 KB for 10 managed windows; subscription survives a
  client reconnect; no new threads per subscriber beyond the socket read.

### 9. Layout-change preview overlay
FancyZones' best idea, implemented with our border-overlay machinery (`borders.rs`):
show the destination layout as outlined regions for ~800 ms after layout-cycle
before committing. Acceptance: overlay never traps focus; excluded when tiling is off.

### 10. Binding modes
Generalize the resize chord into named modes (resize / move / mono) reusing
`hook.rs` action-key dispatch; `Esc` exits to the base chord set like today.
- Acceptance: no new global listener per mode; the chordâmode map is data, not code
  per mode (add a mode = add a table row).

## Non-goals
- Per-window animations or transparency effects (frame churn, battery).
- Shipping a status bar; selling one integrable state stream (item 8).
- A general plugin/runtime (Lua/AHK/JS) â IPC + config covers the same need.
