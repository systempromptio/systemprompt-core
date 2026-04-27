#![allow(unsafe_op_in_unsafe_fn)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::Path;
use std::ptr;
use std::sync::OnceLock;
use std::sync::mpsc::Sender;

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    COLOR_WINDOW, DEFAULT_GUI_FONT, GetStockObject, HBRUSH, HFONT, UpdateWindow,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::EnableWindow;
use windows_sys::Win32::UI::Shell::ShellExecuteW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    BS_PUSHBUTTON, CREATESTRUCTW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW,
    DestroyWindow, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_PASSWORD, ES_READONLY,
    GWLP_USERDATA, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, IDC_ARROW,
    LoadCursorW, RegisterClassW, SW_HIDE, SW_SHOW, SW_SHOWNORMAL, SendMessageW,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowTextW, ShowWindow, WM_CLOSE, WM_COMMAND,
    WM_CREATE, WM_DESTROY, WM_SETFONT, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD,
    WS_MINIMIZEBOX, WS_OVERLAPPED, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

use crate::gui::events::UiEvent;
use crate::gui::state::AppStateSnapshot;

const CLASS_NAME: &str = "SystempromptCoworkSettings";

const ID_LOGIN: usize = 1001;
const ID_LOGOUT: usize = 1002;
const ID_SYNC: usize = 1003;
const ID_VALIDATE: usize = 1004;
const ID_OPEN_FOLDER: usize = 1005;
const ID_CLOSE: usize = 1006;
const ID_PAT_EDIT: usize = 1100;
const ID_GATEWAY_EDIT: usize = 1101;
const ID_STATUS_EDIT: usize = 1200;
const ID_IDENT_LABEL: usize = 1201;
const ID_LAST_SYNC_LABEL: usize = 1202;

struct WindowState {
    tx: Sender<UiEvent>,
    controls: RefCell<HashMap<usize, HWND>>,
}

pub struct PlatformWindow {
    hwnd: RefCell<HWND>,
    state: Box<WindowState>,
}

unsafe impl Send for PlatformWindow {}
unsafe impl Sync for PlatformWindow {}

impl PlatformWindow {
    pub fn new(tx: Sender<UiEvent>) -> Result<Self, String> {
        register_class()?;
        Ok(Self {
            hwnd: RefCell::new(ptr::null_mut()),
            state: Box::new(WindowState {
                tx,
                controls: RefCell::new(HashMap::new()),
            }),
        })
    }

    pub fn show(&self, snapshot: &AppStateSnapshot) {
        let mut existing = self.hwnd.borrow_mut();
        if !existing.is_null() {
            unsafe {
                ShowWindow(*existing, SW_SHOW);
                SetForegroundWindow(*existing);
            }
            self.refresh_inner(*existing, snapshot);
            return;
        }

        let class = wide(CLASS_NAME);
        let title = wide("systemprompt-cowork");
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                560,
                520,
                ptr::null_mut(),
                ptr::null_mut(),
                GetModuleHandleW(ptr::null()),
                self.state.as_ref() as *const WindowState as *const c_void as *mut c_void,
            );
            if hwnd.is_null() {
                return;
            }
            *existing = hwnd;
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            self.refresh_inner(hwnd, snapshot);
        }
    }

    pub fn refresh(&self, snapshot: &AppStateSnapshot) {
        let hwnd = *self.hwnd.borrow();
        if !hwnd.is_null() {
            self.refresh_inner(hwnd, snapshot);
        }
    }

    pub fn close(&self) {
        let mut hwnd = self.hwnd.borrow_mut();
        if !hwnd.is_null() {
            unsafe { DestroyWindow(*hwnd) };
            *hwnd = ptr::null_mut();
        }
    }

    fn refresh_inner(&self, _hwnd: HWND, snap: &AppStateSnapshot) {
        let controls = self.state.controls.borrow();
        let set_text = |id: usize, text: &str| {
            if let Some(h) = controls.get(&id) {
                let w = wide(text);
                unsafe { SetWindowTextW(*h, w.as_ptr()) };
            }
        };

        let ident = snap
            .identity
            .clone()
            .unwrap_or_else(|| "(not signed in)".to_string());
        set_text(
            ID_IDENT_LABEL,
            &format!("Identity: {ident}\r\nGateway:  {}", snap.gateway_url),
        );
        set_text(
            ID_LAST_SYNC_LABEL,
            &format!(
                "Last sync: {}\r\nPlugins dir: {}\r\nSkills: {}    Agents: {}",
                snap.last_sync_summary
                    .clone()
                    .unwrap_or_else(|| "never".into()),
                snap.plugins_dir.clone().unwrap_or_else(|| "?".into()),
                snap.skill_count.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
                snap.agent_count.map(|n| n.to_string()).unwrap_or_else(|| "?".into()),
            ),
        );
        set_text(ID_GATEWAY_EDIT, &snap.gateway_url);

        let mut status = String::new();
        if snap.sync_in_flight {
            status.push_str("Sync in progress…\r\n\r\n");
        }
        if let Some(msg) = &snap.last_action_message {
            status.push_str(msg);
            status.push_str("\r\n\r\n");
        }
        if let Some(report) = &snap.last_validation {
            status.push_str(&report.rendered().replace('\n', "\r\n"));
        }
        set_text(ID_STATUS_EDIT, &status);

        if let Some(h) = controls.get(&ID_SYNC) {
            unsafe { EnableWindow(*h, if snap.sync_in_flight { 0 } else { 1 }) };
        }
    }
}

fn register_class() -> Result<(), String> {
    static REGISTERED: OnceLock<()> = OnceLock::new();
    if REGISTERED.get().is_some() {
        return Ok(());
    }
    unsafe {
        let class = wide(CLASS_NAME);
        let wc = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: GetModuleHandleW(ptr::null()),
            hIcon: ptr::null_mut(),
            hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW as usize + 1) as HBRUSH,
            lpszMenuName: ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        if RegisterClassW(&wc) == 0 {
            return Err("RegisterClassW failed".to_string());
        }
    }
    let _ = REGISTERED.set(());
    Ok(())
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_CREATE => {
            let cs = lparam as *const CREATESTRUCTW;
            let state_ptr = (*cs).lpCreateParams as *const WindowState;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, state_ptr as isize);
            create_controls(hwnd, &*state_ptr);
            0
        },
        WM_COMMAND => {
            let id = (wparam & 0xFFFF) as usize;
            let state = window_state(hwnd);
            if let Some(state) = state {
                handle_command(state, hwnd, id);
            }
            0
        },
        WM_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
            0
        },
        WM_DESTROY => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            0
        },
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn window_state<'a>(hwnd: HWND) -> Option<&'a WindowState> {
    let p = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState;
    if p.is_null() { None } else { Some(&*p) }
}

unsafe fn create_controls(hwnd: HWND, state: &WindowState) {
    let font = GetStockObject(DEFAULT_GUI_FONT as i32) as HFONT;
    let mut controls = state.controls.borrow_mut();

    let mk_static =
        |id: usize, x: i32, y: i32, w: i32, h: i32, text: &str, multi: bool| -> HWND {
            let class = wide("STATIC");
            let t = wide(text);
            let style = WS_CHILD | WS_VISIBLE | if multi { 0 } else { 0 };
            let hctl = CreateWindowExW(
                0,
                class.as_ptr(),
                t.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                hwnd,
                id as *mut c_void,
                GetModuleHandleW(ptr::null()),
                ptr::null_mut(),
            );
            SendMessageW(hctl, WM_SETFONT, font as WPARAM, 1);
            hctl
        };

    let mk_button = |id: usize, x: i32, y: i32, w: i32, h: i32, text: &str| -> HWND {
        let class = wide("BUTTON");
        let t = wide(text);
        let hctl = CreateWindowExW(
            0,
            class.as_ptr(),
            t.as_ptr(),
            WS_CHILD | WS_VISIBLE | WS_TABSTOP | BS_PUSHBUTTON as u32,
            x,
            y,
            w,
            h,
            hwnd,
            id as *mut c_void,
            GetModuleHandleW(ptr::null()),
            ptr::null_mut(),
        );
        SendMessageW(hctl, WM_SETFONT, font as WPARAM, 1);
        hctl
    };

    let mk_edit =
        |id: usize, x: i32, y: i32, w: i32, h: i32, password: bool, multi: bool| -> HWND {
            let class = wide("EDIT");
            let empty = wide("");
            let mut style = WS_CHILD | WS_VISIBLE | WS_TABSTOP | WS_BORDER;
            if password {
                style |= ES_PASSWORD as u32;
            }
            if multi {
                style |= (ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32 | WS_VSCROLL;
            } else {
                style |= ES_AUTOHSCROLL as u32;
            }
            let hctl = CreateWindowExW(
                0,
                class.as_ptr(),
                empty.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                hwnd,
                id as *mut c_void,
                GetModuleHandleW(ptr::null()),
                ptr::null_mut(),
            );
            SendMessageW(hctl, WM_SETFONT, font as WPARAM, 1);
            hctl
        };

    controls.insert(ID_IDENT_LABEL, mk_static(ID_IDENT_LABEL, 16, 14, 520, 36, "", true));
    controls.insert(
        ID_LAST_SYNC_LABEL,
        mk_static(ID_LAST_SYNC_LABEL, 16, 56, 520, 60, "", true),
    );

    mk_static(0, 16, 130, 80, 20, "Gateway:", false);
    controls.insert(
        ID_GATEWAY_EDIT,
        mk_edit(ID_GATEWAY_EDIT, 100, 128, 436, 22, false, false),
    );

    mk_static(0, 16, 162, 80, 20, "PAT:", false);
    controls.insert(
        ID_PAT_EDIT,
        mk_edit(ID_PAT_EDIT, 100, 160, 436, 22, true, false),
    );

    controls.insert(ID_LOGIN, mk_button(ID_LOGIN, 16, 196, 100, 28, "Login"));
    controls.insert(ID_LOGOUT, mk_button(ID_LOGOUT, 124, 196, 100, 28, "Logout"));
    controls.insert(ID_SYNC, mk_button(ID_SYNC, 232, 196, 100, 28, "Sync now"));
    controls.insert(
        ID_VALIDATE,
        mk_button(ID_VALIDATE, 340, 196, 100, 28, "Validate"),
    );
    controls.insert(
        ID_OPEN_FOLDER,
        mk_button(ID_OPEN_FOLDER, 16, 432, 160, 28, "Open config folder"),
    );
    controls.insert(ID_CLOSE, mk_button(ID_CLOSE, 436, 432, 100, 28, "Close"));

    controls.insert(
        ID_STATUS_EDIT,
        mk_edit(ID_STATUS_EDIT, 16, 232, 520, 192, false, true),
    );
}

unsafe fn handle_command(state: &WindowState, hwnd: HWND, id: usize) {
    match id {
        ID_LOGIN => {
            let pat = read_text(state, ID_PAT_EDIT);
            let gateway = read_text(state, ID_GATEWAY_EDIT);
            let gateway = if gateway.trim().is_empty() {
                None
            } else {
                Some(gateway)
            };
            let _ = state.tx.send(UiEvent::LoginRequested {
                token: pat,
                gateway,
            });
        },
        ID_LOGOUT => {
            let _ = state.tx.send(UiEvent::LogoutRequested);
        },
        ID_SYNC => {
            let _ = state.tx.send(UiEvent::SyncRequested);
        },
        ID_VALIDATE => {
            let _ = state.tx.send(UiEvent::ValidateRequested);
        },
        ID_OPEN_FOLDER => {
            let _ = state.tx.send(UiEvent::OpenConfigFolder);
        },
        ID_CLOSE => {
            ShowWindow(hwnd, SW_HIDE);
        },
        _ => {},
    }
}

unsafe fn read_text(state: &WindowState, id: usize) -> String {
    let controls = state.controls.borrow();
    let Some(h) = controls.get(&id) else {
        return String::new();
    };
    let len = GetWindowTextLengthW(*h) as usize;
    if len == 0 {
        return String::new();
    }
    let mut buf: Vec<u16> = vec![0; len + 1];
    let n = GetWindowTextW(*h, buf.as_mut_ptr(), buf.len() as i32) as usize;
    String::from_utf16_lossy(&buf[..n])
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn open_path(path: &Path) {
    let target = wide(&path.display().to_string());
    let verb = wide("open");
    unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            verb.as_ptr(),
            target.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        );
    }
}
