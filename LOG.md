
## 2026-11-08 — Flow plugin + palette typing fix (b403414)
- Status: Done
- Flow Launcher executable plugin: `ms` keyword, flat action list, `w <filter>` per-window mode with hwnd-baked commands, `ms.exec` → daemon IPC. plugin.json + icon installed to %APPDATA%\FlowLauncher\Plugins\MrSpace.
- Palette "stuck" root cause: keystroke handler gated everything behind Shift (`only_shift`) — plain typing/backspace ignored. Fixed: modifier-free chars + backspace + space accepted.
- Verified: 3 query paths + ms.exec round-trip via `mrspace.exe flow '{...}'`; plugin files present in Flow plugin dir. In-app Flow reload (F5 / reload plugin data) by user still pending.
- Skipped ocr-review (no UI screenshot available for palette this session).

## 2026-11-09 — Settings desktop app (df81407)
- Status: Done
- settings.rs is a full native desktop app (own process, single-instance, raise): ipc port, resize step/timeout, tiling interval, exclusion lists (titles/classes/pids), border enable/thickness/accent/muted, full hotkey table — per-field validation, Save / Save & Apply / Open Config / Cancel.
- config.rs gained a serde(default) `borders` block; borders.rs consts → Mutex options + configure(); daemon 'settings' spawns the app (no longer blocks IPC), new 'reload-config' live-applies borders.
- Verified: window visible (class MrspaceSettings), daemon responsive while open, remote WM_COMMAND Save wrote enriched config.json, thickness 6 reload applied live, fit-all overlays still render. In-app click testing (mouse/keyboard) left to the user.

## 2026-11-09 — Settings app Material-3 dark restyle
- Status: Done
- M3-dark theme: near-black surface + container fields, purple accent headers/primary button, owner-draw rounded buttons (Save & Apply = filled lavender, others tonal outlined, pressed/disabled states), dark + rounded DWM title bar, Segoe UI.
- Process is now Per-Monitor-DPI-aware: `s()` scales every layout value and fonts to system DPI — crisp at 150%, designed 716×760 logical geometry is exact on 1080p@150%.
- Root-caused the persistent "wrong window geometry" mystery: the user's komorebi (third-party tiling WM) was grabbing and rezoning the settings window 1-3s after creation (even daemon-off; even WS_EX_TOOLWINDOW). Fix: `komorebic ignore-rule class MrspaceSettings` + hard-coded class exclusion in win32::is_manageable.
- Also: hotkey edit enlarged, vertical layout compressed to fit the 1080p@150% client, DT_NOPREFIX so "&" renders in button text, try_apply dedupes port reload message.
- Verified: full-window screenshots at design geometry, Save + Save & Apply smoke tests (config written, daemon reloaded live), single-instance raise via second `exec settings` — one window. Flow plugin exe copy refreshed.
- Testing discovery logged in TESTING.md 6b (komorebi rule requirement).

## 2026-11-09 — Settings app card-block restyle
- Status: Done
- Form restructured into designed card blocks (MrspaceCard window class, own WM_PAINT): rounded elevated surface + outlineVariant border + icon chip (Segoe MDL2 glyph in primaryContainer tile) + semibold title + small helper description — Linear/Raycast/Material "icon tile" card pattern.
- Cards: Basics (gear), Window titles / Window classes / Process IDs as three separate exclusion cards (distinct glyphs: document / code braces / info), Border overlays (eye), Hotkeys (pencil).
- Small UI elements: SS_OWNERDRAW pills — top-right "● daemon running/offline" (probes daemon via try_send_command at open) and Hotkeys "{n} bindings" count chip; app title "Mr. Space — Settings" painted in main WM_PAINT; WM_CTLCOLORSTATIC/WM_CTLCOLORBTN split so labels and the native checkbox erase with the card color (LBLCARD_BASE id range).
- Card data passed via SetPropW/GetPropW (boxed CardData, freed in WM_DESTROY).
- Verified: build clean, full-window screenshot at design geometry (cards, pills, distinct exclusion glyphs all render), Save + Save & Apply smoke (config written, deduped "daemon reloaded" reply), single-instance raise. Flow plugin exe copy refreshed.

## 2026-11-09 — Settings inspo pass (browser/CDN research)
- Status: Done
- Research route: builtin websearch 402; no Tavily/Firecrawl MCP configured; desktop browser disconnected — used DuckDuckGo html endpoint + webfetch/curl + Muzli embeds + Pinterest pin CDN to pull real reference screenshots (Untitled UI settings page; Muzli dark gallery).
- Applied: in-content page header "Settings" + muted helper line (WM_PAINT), quiet "…" affordance on every card, Untitled-UI hairline separator under the Hotkeys card header (Basics/Borders too tight for it — skipped deliberately), cards shifted +18px for the header, hotkeys edit re-gapped.
- Verified: build clean, full screenshot (header, dots, separator render correctly, no clipping), Save smoke OK, Flow plugin exe copy refreshed.

## 2026-11-09 — Settings designed list rows (inside-block content)
- Status: Done
- Replaced the raw multiline `pin-left|3|0x25|pin-left` edits with designed owner-draw rows: hotkey rows = name + modifier chips (Win/Alt/Ctrl/Shift, field-tinted rounded chips) + keycap box with vk glyph (←↑↓ arrows, F-keys, else raw hex) + right-aligned dim "→ action"; exclusion rows = clean plain text on the card surface; selection = primaryContainer tint band.
- Raw text kept in hidden sibling EDITs (same control ids, MLH style without WS_VISIBLE) so every save/collect/validation path is unchanged; listboxes (LBS_OWNERDRAWFIXED|HASSTRINGS, LB_SETITEMHEIGHT 24/20) are seeded from config at load and synced back via sync_hidden() (also refreshes the "{n} bindings" pill).
- Editing: double-click a row (or ＋ Add) opens an in-place overlay EDIT over the row (append slot below the last row); commit on EN_KILLFOCUS; empty edit = delete row. ＋/✕: tiny tonal buttons on each exclusion card header (ID_ADD/ID_DEL ranges), ＋ seeds from the current selection; hotkeys card has a ＋ Add / ✕ Remove TXTBTN strip at the bottom.
- Win32 listbox lessons learned (cost real time): the item DC covers the *whole* list — every row must erase its own rect and offset its drawing by rcItem.top, or all rows stack at the top; LBS_HASSTRINGS has no in-place replace (delete+insert); `MapWindowPoints` lives in Graphics::Gdi on windows-sys 0.61 (POINT-based → map_rect() helper); pattern ranges can't contain non-const expressions (ID_LAST_ROWBTN const).
- vk parse bug: config serializes vk as {:#x} ("0x25") but parse_row used base-10 parse — keycaps never rendered. Fixed with explicit 0x-prefix radix parsing.
- Renamed "Window classes" card title → "Classes" (header collided with the ＋/✕ buttons at 212 logical px card width).
- Known nits (accepted): system-white listbox scrollbar; only ~4 rows visible before scrolling (by design, rows scroll).
- Verified: build clean, full-window screenshot (rows render separated with chips/keycaps/actions), LB_GETITEMHEIGHT probe (30/30/30/36 physical = correct), Save smoke → config.txt byte-identical, Flow plugin exe copy refreshed, daemon restarted.

## 2026-11-09 — Settings polish round 2 (dark controls, hints, swatches)
- Status: Done
- Dark-mode common controls: SetWindowTheme(h, "DarkMode_Explorer") on all four listboxes (dark scrollbar) and the Enable checkbox (dark glyph).
- Empty-state hints in the three exclusion lists ("nothing yet — ＋ adds a …"), SS_CENTER statics layered on top of each list, toggled from sync_hidden/update_hints by row count; CTLCOLORSTATIC renders them CLR_TEXT_DIM on card bg.
- Live color swatches: 12×20 rounded owner-draw statics next to the Accent/Muted fields, parse_color("#RRGGBB"→COLORREF 0x00BBGGRR), invalidated on EN_CHANGE — typing a hex updates the preview instantly.
- Bugs fixed this round: swatch statics were created without WS_VISIBLE (invisible), swatch DC origin is the static's client (draw 0,0..w,h like pills), and initial parse interpreted RRGGBB as COLORREF directly → BGR-swapped salmon; now explicit per-channel parse.
- Verified: build clean, full screenshot (dark scrollbar+checkbox, hints show in empty lists, swatches correct purple/gray), Save smoke → config identical, Flow plugin exe refreshed.

## 2026-11-09 — Settings polish round 3 (button hover, hint row, Enter/Esc)
- Status: Done
- Button hover feedback: BS_OWNERDRAW never reports ODS_HOTLIGHT without an activation-context manifest, so hover is driven by a WM_TIMER poll — SendTimer(hwnd,1,60ms) armed at WM_CREATE; hover_poll() diffs hovered_btn() (screen-space GetCursorPos vs each button's GetWindowRect, gated on GetForegroundWindow==parent) against BTN_HOVER atomic and invalidates only the changing buttons; draw_button lifts the surface to CLR_ICON_TILE when hot.
- Root-cause lesson: the first hovered_btn converted the rect with ScreenToClient but compared raw screen cursor coords — two mismatched spaces, hover silently never true (title showed T ticks incrementing, H=-1). Compare screen vs screen directly.
- Subclass (GetWindowLongPtrW/SetWindowLongPtrW + TrackMouseEvent TME_LEAVE) was attempted first and abandoned — cross-process probes can't verify subclass state and the timer poll is simpler and didn't need it.
- Hint row under the Hotkeys list: "double-click a row to edit · ⏎ commits · blank = delete" (LBLCARD_BASE+7 static, CLR_TEXT_DIM on card bg).
- Overlay row editor: removed WS_TABSTOP (Tab could land on the parked offscreen invisible EDIT — invisible-focus-trap bug); Enter=commit / Esc=cancel handled at the pump level before IsDialogMessageW (VK_RETURN/VK_ESCAPE from KeyboardAndMouse module; overlay_commit reads via GetDlgItemTextW so no focus dependency).
- Verified: build clean, hover probe (real cursor onto button GetWindowRect center + screen capture: ＋ Add renders lit CLR_ICON_TILE fill cold-render stays dim), Save smoke → "daemon reloaded (port 17331)", full screenshot (hint row + swatches + dark controls all render), Flow plugin exe refreshed, komorebi ignore-rule re-applied after restart (EXIT=0) at 1074×1106 geometry.

## 2026-11-12 — Tier 2.2: scratchpad (summon/park)
- Status: Done
- New module `scratchpad.rs`: `PARKED: Vec<isize>`, `SAVED: HashMap<isize, RECT>`.
  `park(hwnd)`: save rect, mark floating, move offscreen (-32000,-32000),
  hide via SWP_HIDEWINDOW (no taskbar flash), re-tile.
  `summon()`: pop last parked, restore rect + show, un-float, re-tile.
- IPC + CLI: park, summon. Acceptance: both round-trip OK; build clean.

## 2026-11-12 — Tier 2.1: window rules engine
- Status: Done
- New module `rules.rs`: `WindowRule { match_title, match_class, match_exe, action }`;
  `matches`, `action_for`, `is_ignored`, `apply_float_rules`.
- `win32::is_manageable` consults `rules::is_ignored`.
- Re-tiler applies float rules to every window before counting.
- Config: `window_rules: Vec<WindowRule>` (default empty); settings.rs updated.
- Acceptance: build clean.

## 2026-11-12 — Tier 1.3: managed-window record + state commands
- Status: Done
- New module `managed.rs`: `FLOATING: Mutex<HashSet<isize>>`; `is_floating`, `toggle_float`,
  `clear`. `toggle_float` flips the flag and re-tiles + syncs borders.
- `win32::is_manageable` returns false for floating windows — excluded from tiling,
  counting, borders (left where the user put them).
- `layout::swap_next(hwnd)`: swaps rects of focused + next manageable window in z-order.
- `layout::promote(hwnd)`: re-tiles the whole set with focused window in slot 0.
- IPC + CLI + default hotkeys: toggle-float (F), swap-next (S), promote (A); wired in
  hook.rs `is_action_key` + `execute`.
- Float toggle round-trips: float -> window leaves tiling set; un-float -> re-joins via
  fit_all.
- Acceptance: toggle-float IPC round-trips OK; build clean.

## 2026-11-12 — Tier 1.2: workspace-scoped layout memory
- Status: Done
- New module `workspace.rs`: flat Vec<(GUID, DeskState)> capped at 8 (FIFO eviction);
  `note_layout` recorded by `layouts::apply` on success; `on_retile_tick` detects
  desktop switches via GUID hash on the same event wake; `restore_layout_for_current`
  re-applies the remembered layout (or leaves geometry untouched); `retile_current`
  applies stored layout or grid fallback on count change.
- `is_cloaked` filter (DWMWA_CLOAKED) added to `win32::is_manageable` — windows on
  other desktops excluded from tiling/counting/borders entirely.
- `current_guid()` resolves via temp window on the active desktop (consistent
  identity across note/restore), cached 1000 ms (300 ms switch-animation sleep).
- `VirtualDesktopManager` now created fresh per call — the shared singleton was a
  cross-apartment COM hazard that could crash the daemon (found during soak).
- `move-next-desktop`/`move-prev-desktop` cannot move foreign-process windows
  (IVirtualDesktopManager::MoveWindowToDesktop returns E_ACCESSDENIED) — Windows
  limitation, not a regression; workspace memory does not depend on it.
- `send_combo` now sends keys with 60 ms delays (Win-key combos dropped without).
- Verified: switch detection fires ("desk switch: no stored layout"/"layout restored"
  in MRS_TRACE); no daemon crash after the VDM fix; build clean.
- Flow plugin exe refreshed.

## 2026-11-10 — Border overlay fixes (stale "middle line" + fullscreen corner marks)
- Status: Done
- Root cause 1 (middle line): tick() only redrew overlays when w/h/color changed — a window that moved without resizing left its ring at the old rect (the "line" was a stale ring edge). Root cause 2 (corner marks): maximized/fullscreen targets have GetWindowRect == or beyond the screen, so rings painted offscreen leaving corner fragments.
- Fix in borders.rs: Entry gains x/y/hidden; tick() computes IsZoomed(th) and a workarea-covering rect test; maximized/fullscreen targets get overlay ShowWindow(SW_HIDE) + skip (no ring on full-screen tiles, komorebi-style); un-hide via SW_SHOWNA + forced e.w=0 redraw; move-only changes reposition via SetWindowPos(HWND_TOPMOST, SWP_NOSIZE|SWP_NOACTIVATE); size/color changes still full-redraw (UpdateLayeredWindow moves too).
- Probe lesson: Win11 notepad (store app) suspends when unfocused → SendMessageTimeout returns nothing → drains out of is_manageable; a hidden `Start-Process notepad -WindowStyle Hidden` yields MainWindowHandle=0. mspaint (classic Win32) is a reliable non-suspending probe. Console windows are per-instance conhost-hosted — process pid is NOT the window owner; MainWindowHandle stays 0 for spawned console apps from .NET.
- Verified with EnumWindows probe (mrspace.border class): tiled → both rings visible; SW_MAXIMIZE → probe ring hidden (vis=False), sibling ring stays; SW_RESTORE → ring returns; single window tiled to full workarea → no overlay at all (stale line gone). Confirmed overlay handles alive (IsWindow) and rect-accurate via GetWindowRect; earlier "no overlays" reads were a <200ms tick race plus a stale daemon.
- Debug tooling kept env-gated (MRS_TRACE=path): tiling.rs hook/wake/count lines and borders.rs tick cadence trace ([tick N] entries=k + per-entry tgt/overlay/alive/hidden/pos) — no-op in production runs.
- Flow plugin exe refreshed; tiling left ON (user's normal state).

## 2026-11-10 — Tier 1.1: event-driven re-tile engine (SetWinEventHook)
- Status: Done
- ROADMAP.md added: competitor matrix (komorebi/GlazeWM v3/FancyZones vs us, RAM prioritized), the 10-item tiered plan, and the RAM guardrails (no idle polling, flat Vec window records, no scripting runtime/bundled bar/animations).
- Replaced the 1 s poller in tiling.rs with three SetWinEventHook registrations (CREATE/DESTROY/SHOW/HIDE, MINIMIZESTART/END, CLOAKED/UNCLOAKED; WINEVENT_OUTOFCONTEXT|SKIPOWNPROCESS), a pump thread (GetMessageW→DispatchMessageW — dispatch is REQUIRED for OUTOFCONTEXT callback delivery), and a re-tiler thread waiting on an auto-reset event (SetEvent from the hook proc, 150 ms debounce, re-tile only when the manageable set changes — drags/resizes never re-tile).
- Two daemon-hang bugs found via cdb thread-stack attach (`D:/WindowsKits/Debuggers/x64/cdb.exe -p PID -c "~* kc12"`) and fixed: (1) GetWindowTextW sends WM_GETTEXT cross-process → blocked forever on suspended UWP/notepad threads; window_title() now uses SendMessageTimeoutW + SMTO_ABORTIFHUNG + 300 ms. (2) SetWindowPos/MoveWindow waited on dying/g window threads during destroy bursts; move_resize() now uses SWP_ASYNCWINDOWPOS (MoveWindow import dropped in win32.rs, settings.rs retains its own import for same-process controls).
- Temp debug: MRS_TRACE=path env-gated trace (hook reg + re-tiler wake/count/retile lines) left in tiling.rs — no-op in normal runs; remove after soak.
- Verified: hooks registered (trace: 3 registrations true), notepad auto-tiled to grid cell 0,0-640,720 within seconds of opening, close re-tiles remainder, trace shows wake→debounce→count→retile cycles with no wedge, idle CPU delta 0.016 s per 2 s, daemon ~32 MB working set with tiling ON (16 MB was with tiling off), fit-all + toggle OFF smoke pass, Flow plugin exe refreshed.
