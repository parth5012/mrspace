
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
