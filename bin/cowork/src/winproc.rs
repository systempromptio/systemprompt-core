#![cfg(target_os = "windows")]
#![allow(unsafe_code)]

use std::env;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const DETACHED_PROCESS: u32 = 0x0000_0008;

pub(crate) fn reg_command() -> Command {
    silenced_command(system32_path("reg.exe"))
}

pub(crate) fn tasklist_command() -> Command {
    silenced_command(system32_path("tasklist.exe"))
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

pub(crate) fn is_elevated() -> bool {
    use std::mem::MaybeUninit;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION, TOKEN_QUERY, TokenElevation,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        let mut elevation = MaybeUninit::<TOKEN_ELEVATION>::zeroed();
        let mut ret_len: u32 = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevation,
            elevation.as_mut_ptr().cast(),
            u32::try_from(std::mem::size_of::<TOKEN_ELEVATION>()).unwrap_or(u32::MAX),
            &mut ret_len,
        );
        let elevated = ok != 0 && elevation.assume_init().TokenIsElevated != 0;
        CloseHandle(token);
        elevated
    }
}

pub(crate) fn attach_parent_console_if_present() {
    use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
    unsafe {
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
}

pub(crate) fn detach_console() {
    use windows_sys::Win32::System::Console::FreeConsole;
    unsafe {
        FreeConsole();
    }
}

fn system32_path(exe: &str) -> PathBuf {
    let root = env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    root.join("System32").join(exe)
}
