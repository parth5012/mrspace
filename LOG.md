
## 2026-11-08 — Flow plugin + palette typing fix (b403414)
- Status: Done
- Flow Launcher executable plugin: `ms` keyword, flat action list, `w <filter>` per-window mode with hwnd-baked commands, `ms.exec` → daemon IPC. plugin.json + icon installed to %APPDATA%\FlowLauncher\Plugins\MrSpace.
- Palette "stuck" root cause: keystroke handler gated everything behind Shift (`only_shift`) — plain typing/backspace ignored. Fixed: modifier-free chars + backspace + space accepted.
- Verified: 3 query paths + ms.exec round-trip via `mrspace.exe flow '{...}'`; plugin files present in Flow plugin dir. In-app Flow reload (F5 / reload plugin data) by user still pending.
- Skipped ocr-review (no UI screenshot available for palette this session).
