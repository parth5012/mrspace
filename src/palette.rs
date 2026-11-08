// Command palette rendered with gpui (https://gpui.rs).
// Borderless popup: search field + filtered action list, dark theme,
// arrow navigation, Enter runs, Esc closes. Runs as a standalone process
// (`mrspace palette`) and quits as soon as the palette closes.
use gpui::{
    App, Application, Bounds, ClickEvent, Context as GpuiContext, Window, WindowBounds, WindowKind,
    WindowOptions, div, prelude::*, px, rgb, rgba, size,
};

struct PaletteState {
    query: String,
    filtered: Vec<usize>,
    selected: usize,
}

const COMMANDS: &[(&str, &str)] = &[
    ("Pin left half", "pin-left"),
    ("Pin right half", "pin-right"),
    ("Pin top half", "pin-top"),
    ("Pin bottom half", "pin-bottom"),
    ("Maximize", "maximize"),
    ("Auto-fit all (grid)", "fit-all"),
    ("Toggle tiling mode", "toggle-tiling"),
    ("Focus left", "focus-left"),
    ("Focus down", "focus-down"),
    ("Focus up", "focus-up"),
    ("Focus right", "focus-right"),
    ("Toggle resize mode", "resize-mode"),
    ("Grow window", "grow"),
    ("Shrink window", "shrink"),
    ("Next desktop", "next-desktop"),
    ("Prev desktop", "prev-desktop"),
    ("Move to next desktop", "move-next-desktop"),
    ("Move to prev desktop", "move-prev-desktop"),
    ("Layout: grid", "layout grid"),
    ("Layout: columns-2", "layout columns-2"),
    ("Layout: columns-3", "layout columns-3"),
    ("Layout: rows-2", "layout rows-2"),
    ("Layout: main-left", "layout main-left"),
    ("Layout: main-right", "layout main-right"),
    ("Layout: main-top", "layout main-top"),
    ("Layout: main-bottom", "layout main-bottom"),
    ("Layout: focus", "layout focus"),
    ("Layout: cycle", "layout-cycle"),
    ("List windows", "list-windows"),
    ("Current desktop", "current-desktop"),
    ("Settings", "settings"),
];

const BG: u32 = 0x161618;
const ROW_SELECTED: u32 = 0x2A2A30;
const ROW_HOVER: u32 = 0x222226;
const ACCENT: u32 = 0x7C4DFF;
const MUTED: u32 = 0x6E6E76;

impl PaletteState {
    fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = COMMANDS
            .iter()
            .enumerate()
            .filter(|(_, (label, _))| q.is_empty() || label.to_lowercase().contains(&q))
            .map(|(i, _)| i)
            .collect();
        self.selected = 0;
    }

    fn run_selected(&mut self) {
        if let Some(&idx) = self.filtered.get(self.selected) {
            let cmd = COMMANDS[idx].1;
            crate::daemon::run_command(cmd);
        }
    }
}

impl gpui::Render for PaletteState {
    fn render(&mut self, _window: &mut Window, cx: &mut GpuiContext<Self>) -> impl IntoElement {
        let mut rows: Vec<gpui::AnyElement> = Vec::new();
        for (row, cmd_idx) in self.filtered.iter().enumerate() {
            let (label, _) = COMMANDS[*cmd_idx];
            let sel = row == self.selected;
            let built =
                div()
                    .id(label)
                    .flex()
                    .items_center()
                    .px_3()
                    .py_1_5()
                    .rounded_md()
                    .cursor_pointer()
                    .bg(if sel {
                        rgb(ROW_SELECTED)
                    } else {
                        rgba(0x00000000)
                    })
                    .text_color(if sel {
                        rgb(0xFFFFFF)
                    } else {
                        rgb(0xCFCFCF)
                    })
                    .text_size(px(13.))
                    .hover(move |st| {
                        st.bg(if sel {
                            rgb(ROW_SELECTED)
                        } else {
                            rgba(ROW_HOVER)
                        })
                    })
                    .on_click(cx.listener(move |s, _: &ClickEvent, _, cx| {
                        s.selected = row;
                        s.run_selected();
                        cx.quit();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1_5()
                            .child(
                                div()
                                    .w(px(3.))
                                    .h(px(16.))
                                    .rounded(px(2.))
                                    .bg(if sel {
                                        rgb(ACCENT)
                                    } else {
                                        rgba(0x7C4DFF00)
                                    }),
                            )
                            .child(label.to_string()),
                    )
            ;
            rows.push(built.into_any_element());
        }

        let list_children: Vec<gpui::AnyElement> = if self.filtered.is_empty() {
            vec![div()
                .px_3()
                .py_1_5()
                .text_color(rgb(MUTED))
                .text_size(px(13.))
                .child("No matches")
                .into_any_element()]
        } else {
            rows
        };

        let query_text = if self.query.is_empty() {
            "Type a command...".to_string()
        } else {
            self.query.clone()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .rounded_lg()
            .overflow_hidden()
            .bg(rgb(BG))
            .border_1()
            .border_color(rgb(0x2A2A30))
            // search row
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_4()
                    .py_3()
                    .child(
                        div()
                            .text_size(px(15.))
                            .text_color(if self.query.is_empty() {
                                rgb(MUTED)
                            } else {
                                rgb(0xEDEDED)
                            })
                            .child(query_text),
                    )
                    // caret
                    .child(
                        div()
                            .w(px(2.))
                            .h(px(18.))
                            .rounded(px(1.))
                            .bg(rgb(ACCENT)),
                    ),
            )
            // separator
            .child(div().h(px(1.)).w_full().bg(rgb(0x2A2A30)))
            // list
            .child(
                div()
                    .id("list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_scroll()
                    .p_2()
                    .gap(px(2.))
                    .children(list_children),
            )
            // footer
            .child(
                div()
                    .flex()
                    .justify_between()
                    .px_4()
                    .py_2()
                    .bg(rgb(0x111113))
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child("arrows select · enter run · esc close"),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(MUTED))
                            .child("Mr. Space"),
                    ),
            )
    }
}

/// All keys handled via global keystroke observation — no IME/text-input
/// machinery needed: letters build the query, arrows/enter/escape navigate.
pub fn show() {
    // Single instance: if a palette is already open, exit — the daemon or CLI
    // that detected us will raise our window instead of spawning a duplicate.
    if !acquire_single_instance_mutex() {
        return;
    }

    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(560.), px(420.)), cx);
        let win = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    kind: WindowKind::PopUp,
                    focus: true,
                    is_movable: true,
                    is_resizable: false,
                    is_minimizable: false,
                    ..Default::default()
                },
                |_, cx| {
                    let mut s = PaletteState {
                        query: String::new(),
                        filtered: COMMANDS.iter().enumerate().map(|(i, _)| i).collect(),
                        selected: 0,
                    };
                    s.refilter();
                    cx.new(|_| s)
                },
            )
            .unwrap();

        // Raise + focus: gpui's Windows activate() is only wired to tab
        // switching, so an open palette would otherwise sit behind everything.
        // The window is shown slightly after open_window returns (first frame),
        // so retry the topmost/foreground dance from a std thread until it is
        // actually visible.
        {
            let _ = win.clone();
            std::thread::spawn(|| {
                for i in 0..12 {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    let r = unsafe { apply_palette_window_state() };
                    if r != 0 {
                        break;
                    }
                }
            });
        }
        win.update(cx, |_, window, _| {
            window.activate_window();
        })
        .unwrap();

        cx.observe_keystrokes(move |ev, _window, cx| {
            let ks = ev.keystroke.clone();
            let root = win.clone();
            let _ = root.update(cx, |s, _window, cx| {
                let m = &ks.modifiers;
                let only_shift = m.shift && !m.control && !m.alt && !m.platform;
                match ks.key.as_str() {
                    "up" => {
                        if s.selected > 0 {
                            s.selected -= 1;
                        }
                    }
                    "down" => {
                        if s.selected + 1 < s.filtered.len() {
                            s.selected += 1;
                        }
                    }
                    "enter" => {
                        s.run_selected();
                        cx.quit();
                    }
                    "escape" => {
                        cx.quit();
                    }
                    "backspace" if only_shift => {
                        s.query.pop();
                        s.refilter();
                    }
                    _ => {
                        if only_shift {
                            if let Some(ch) = ks.key_char.as_deref().and_then(|c| c.chars().next())
                            {
                                s.query.push(ch);
                                s.refilter();
                            }
                        }
                    }
                }
                cx.notify();
            });
        })
        .detach();

        cx.activate(true);
    });
}

// --- Windows plumbing: single instance, topmost, hwnd record for raising ---

use windows_sys::core::BOOL;
use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, HANDLE, HWND, LPARAM, WPARAM};
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetWindowLongPtrW,
    GetWindowThreadProcessId, IsWindowVisible, SetWindowLongPtrW, SetWindowPos, GetWindowRect,
    GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW,
    WS_EX_TOPMOST,
};

const MUTEX_NAME: &str = "mrspace.palette.single";

fn acquire_single_instance_mutex() -> bool {
    unsafe {
        let name: Vec<u16> = MUTEX_NAME.encode_utf16().chain([0]).collect();
        let h: HANDLE = CreateMutexW(std::ptr::null(), 0, name.as_ptr());
        if h.is_null() {
            return true; // failure to acquire shouldn't block opening
        }
        // ERROR_ALREADY_EXISTS means another palette already owns it.
        !(windows_sys::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS)
    }
}

struct HwndFind {
    pid: u32,
    found: isize,
}

unsafe extern "system" fn find_own_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    if hwnd.is_null() {
        return 1;
    }
    let state = &mut *(lparam as *mut HwndFind);
    let mut pid: u32 = 0;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid != state.pid {
        return 1;
    }
    if IsWindowVisible(hwnd) == 0 {
        return 1;
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(hwnd, cls.as_mut_ptr(), 64);
    if n <= 0 {
        return 1;
    }
    if String::from_utf16_lossy(&cls[..n as usize]) == "Zed::Window" {
        state.found = hwnd as isize;
        return 0; // stop
    }
    1
}

/// Make the palette float above everything until closed (launcher behaviour),
/// and remember its hwnd so other processes can raise it later.
/// Find the palette's own visible window, pin it topmost, foreground it, and
/// publish its hwnd so other mrspace processes can raise it. Returns the hwnd
/// (0 when the window isn't shown yet — gpui shows it after the first frame).
unsafe fn apply_palette_window_state() -> isize {
    let mut state = HwndFind { pid: GetCurrentProcessId(), found: 0 };
    EnumWindows(Some(find_own_window), &mut state as *mut _ as LPARAM);
    if state.found == 0 {
        return 0;
    }
    let hwnd = state.found as HWND;
    let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex | WS_EX_TOPMOST as isize) as isize);
    SetWindowPos(
        hwnd, HWND_TOPMOST, 0, 0, 0, 0,
        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    // publish hwnd so the daemon can raise us without spawning stacks of processes
    let record = format!("{}:{}", state.pid, state.found);
    let _ = std::fs::write(std::env::temp_dir().join("mrspace-palette.hwnd"), record);
    // Bring to foreground: Windows requires the "input-active" look — the same
    // alt-tap trick gpui itself uses (see platform/windows/window.rs activate()).
    {
        use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
        };
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::SetActiveWindow;
        use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

        let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let _ = AttachThreadInput(GetCurrentThreadId(), target_thread, 1);
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT { wVk: VK_MENU, dwFlags: 0, ..std::mem::zeroed() },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT { wVk: VK_MENU, dwFlags: KEYEVENTF_KEYUP, ..std::mem::zeroed() },
                },
            },
        ];
        SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        SetActiveWindow(hwnd);
        SetForegroundWindow(hwnd);
    }
    state.found
}

/// From another process (the daemon or a second `mrspace palette`): focus and
/// raise the already-open palette window, if its hwnd record is live.
#[cfg(windows)]
pub fn raise_other() -> bool {
    
    use windows_sys::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{VK_MENU, KEYEVENTF_KEYUP, KEYBDINPUT, INPUT, INPUT_0, INPUT_KEYBOARD, SendInput};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowThreadProcessId, IsWindow,
        SetForegroundWindow, ShowWindow, SW_RESTORE,
    };

    let record = match std::fs::read_to_string(std::env::temp_dir().join("mrspace-palette.hwnd")) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let (pid, hwnd) = match record.split_once(':')
        .and_then(|(p, h)| Some((p.parse::<u32>().ok()?, h.parse::<isize>().ok()?)))
    {
        Some(v) => v,
        None => return false,
    };
    let hwnd = hwnd as HWND;

    unsafe {
        // live check: window exists AND still belongs to the same process
        if IsWindow(hwnd) == 0 {
            return false;
        }
        let mut live_pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut live_pid);
        if live_pid != pid {
            return false;
        }

        let target_thread = GetWindowThreadProcessId(hwnd, std::ptr::null_mut());
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, std::ptr::null_mut());
        let this_thread = GetCurrentThreadId();
        let attached = AttachThreadInput(this_thread, target_thread, 1);
        let _ = fg_thread;
        if IsWindowVisible(hwnd) == 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        // gpui's own trick: Windows needs the process to look input-active
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: 0,
                        ..std::mem::zeroed()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..std::mem::zeroed()
                    },
                },
            },
        ];
        SendInput(2, inputs.as_ptr(), std::mem::size_of::<INPUT>() as i32);
        SetForegroundWindow(hwnd);
        let _ = attached;
        true
    }
}


