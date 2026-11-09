
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
