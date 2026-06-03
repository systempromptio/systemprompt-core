#![cfg(target_os = "windows")]
#![allow(unsafe_code, reason = "Win32 process / window-manipulation FFI")]

use std::env;
use std::mem::MaybeUninit;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_CANCELLED, GetLastError, HANDLE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
};
use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole, FreeConsole};
use windows_sys::Win32::System::Threading::{
    GetCurrentProcess, GetExitCodeProcess, INFINITE, OpenProcessToken, WaitForSingleObject,
};
use windows_sys::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

// Write-only path. Reads must go through `crate::config::store` in-process FFI
// to avoid reg.exe's quoting + Wow6432 redirection footguns.
pub(crate) fn reg_command() -> Command {
    silenced_command(system32_path("reg.exe"))
}

fn silenced_command(exe: PathBuf) -> Command {
    let mut cmd = Command::new(exe);
    cmd.creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(root) = env::var_os("SystemRoot") {
        cmd.current_dir(root);
    }
    cmd
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CloseHandle(self.0) };
        }
    }
}

unsafe fn open_current_process_token() -> Option<OwnedHandle> {
    let mut token: HANDLE = std::ptr::null_mut();
    let ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) };
    if ok == 0 {
        return None;
    }
    Some(OwnedHandle(token))
}

unsafe fn token_is_elevated(token: &OwnedHandle) -> bool {
    let mut elevation = MaybeUninit::<TOKEN_ELEVATION>::zeroed();
    let mut ret_len: u32 = 0;
    let ok = unsafe {
        GetTokenInformation(
            token.0,
            TokenElevation,
            elevation.as_mut_ptr().cast(),
            u32::try_from(size_of::<TOKEN_ELEVATION>()).unwrap_or(u32::MAX),
            &raw mut ret_len,
        )
    };
    ok != 0 && unsafe { elevation.assume_init() }.TokenIsElevated != 0
}

pub(crate) fn is_elevated() -> bool {
    unsafe { open_current_process_token() }
        .as_ref()
        .is_some_and(|t| unsafe { token_is_elevated(t) })
}

pub(crate) fn attach_parent_console_if_present() {
    unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}

pub(crate) fn detach_console() {
    unsafe { FreeConsole() };
}

fn system32_path(exe: &str) -> PathBuf {
    let root =
        env::var_os("SystemRoot").map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
    root.join("System32").join(exe)
}

pub(crate) enum ElevationOutcome {
    Completed { exit_code: u32 },
    Declined,
    Failed(String),
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// CommandLineToArgvW quoting: a child reconstructs argv from this single
// string, so paths with spaces or embedded quotes must round-trip exactly.
fn quote_arg(arg: &str) -> String {
    if !arg.is_empty() && !arg.contains([' ', '\t', '"']) {
        return arg.to_string();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('"');
    let mut backslashes = 0usize;
    for ch in arg.chars() {
        match ch {
            '\\' => {
                backslashes += 1;
                out.push('\\');
            },
            '"' => {
                for _ in 0..=backslashes {
                    out.push('\\');
                }
                backslashes = 0;
                out.push('"');
            },
            _ => {
                backslashes = 0;
                out.push(ch);
            },
        }
    }
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

pub(crate) fn run_elevated(exe: &Path, args: &[&str]) -> ElevationOutcome {
    let params = args
        .iter()
        .map(|a| quote_arg(a))
        .collect::<Vec<_>>()
        .join(" ");
    let verb = to_wide("runas");
    let file = to_wide(&exe.to_string_lossy());
    let params_w = to_wide(&params);

    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = u32::try_from(size_of::<SHELLEXECUTEINFOW>()).unwrap_or(0);
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params_w.as_ptr();
    info.nShow = SW_HIDE;

    let launched = unsafe { ShellExecuteExW(&raw mut info) };
    if launched == 0 {
        let code = unsafe { GetLastError() };
        if code == ERROR_CANCELLED {
            return ElevationOutcome::Declined;
        }
        return ElevationOutcome::Failed(format!(
            "ShellExecuteExW(runas) failed with status {code}"
        ));
    }
    if info.hProcess.is_null() {
        return ElevationOutcome::Failed("elevated process handle was null".into());
    }
    let handle = OwnedHandle(info.hProcess);
    let wait = unsafe { WaitForSingleObject(handle.0, INFINITE) };
    if wait != WAIT_OBJECT_0 {
        return ElevationOutcome::Failed(format!("WaitForSingleObject returned {wait}"));
    }
    let mut exit_code: u32 = 0;
    let got = unsafe { GetExitCodeProcess(handle.0, &raw mut exit_code) };
    if got == 0 {
        return ElevationOutcome::Failed("GetExitCodeProcess failed".into());
    }
    ElevationOutcome::Completed { exit_code }
}
