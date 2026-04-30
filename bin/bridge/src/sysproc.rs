//! Subprocess-free process enumeration.
//!
//! Replaces `tasklist.exe` / `/bin/ps` shellouts that caused console flicker on
//! Windows and a needless fork-per-probe on macOS/Linux. All platforms expose a
//! uniform `list_processes()` returning image basename and (when available) the
//! full executable path, which is what the integration-probe callers actually
//! filter on.

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub name: String,
    pub path: Option<String>,
}

#[must_use]
pub fn list_processes() -> Vec<ProcInfo> {
    #[cfg(target_os = "windows")]
    {
        windows::list()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        linux::list()
    }
    #[cfg(not(any(unix, windows)))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "windows")]
mod windows {
    #![allow(unsafe_code)]

    use super::ProcInfo;
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
        TH32CS_SNAPPROCESS,
    };

    pub(super) fn list() -> Vec<ProcInfo> {
        let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snap == INVALID_HANDLE_VALUE || snap.is_null() {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut entry = unsafe { std::mem::zeroed::<PROCESSENTRY32W>() };
        entry.dwSize = u32::try_from(std::mem::size_of::<PROCESSENTRY32W>()).unwrap_or(0);
        if unsafe { Process32FirstW(snap, &mut entry) } != 0 {
            loop {
                let exe_field = &entry.szExeFile;
                let len = exe_field
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(exe_field.len());
                let name = String::from_utf16_lossy(&exe_field[..len]);
                out.push(ProcInfo { name, path: None });
                if unsafe { Process32NextW(snap, &mut entry) } == 0 {
                    break;
                }
            }
        }
        unsafe { CloseHandle(snap) };
        out
    }
}

#[cfg(target_os = "macos")]
mod macos {
    #![allow(unsafe_code)]

    use super::ProcInfo;

    const PROC_ALL_PIDS: u32 = 1;
    const PATH_MAX_BYTES: usize = 4096;

    unsafe extern "C" {
        fn proc_listpids(
            r#type: u32,
            typeinfo: u32,
            buffer: *mut libc::c_void,
            buffersize: libc::c_int,
        ) -> libc::c_int;

        fn proc_pidpath(
            pid: libc::c_int,
            buffer: *mut libc::c_void,
            buffersize: u32,
        ) -> libc::c_int;
    }

    pub(super) fn list() -> Vec<ProcInfo> {
        let needed = unsafe { proc_listpids(PROC_ALL_PIDS, 0, std::ptr::null_mut(), 0) };
        if needed <= 0 {
            return Vec::new();
        }
        let count = (needed as usize) / std::mem::size_of::<libc::pid_t>();
        let mut pids = vec![0_i32; count + 32];
        let bytes = i32::try_from(pids.len() * std::mem::size_of::<libc::pid_t>()).unwrap_or(0);
        let written = unsafe {
            proc_listpids(
                PROC_ALL_PIDS,
                0,
                pids.as_mut_ptr().cast::<libc::c_void>(),
                bytes,
            )
        };
        if written <= 0 {
            return Vec::new();
        }
        let n = (written as usize) / std::mem::size_of::<libc::pid_t>();
        let mut out = Vec::with_capacity(n);
        let mut buf = vec![0_u8; PATH_MAX_BYTES];
        for &pid in pids.iter().take(n) {
            if pid <= 0 {
                continue;
            }
            let len = unsafe {
                proc_pidpath(
                    pid,
                    buf.as_mut_ptr().cast::<libc::c_void>(),
                    PATH_MAX_BYTES as u32,
                )
            };
            if len <= 0 {
                continue;
            }
            let path = String::from_utf8_lossy(&buf[..len as usize]).to_string();
            let name = std::path::Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(ProcInfo {
                name,
                path: Some(path),
            });
        }
        out
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod linux {
    use super::ProcInfo;
    use std::fs;

    pub(super) fn list() -> Vec<ProcInfo> {
        let entries = match fs::read_dir("/proc") {
            Ok(e) => e,
            Err(_) => return Vec::new(),
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let file_name = entry.file_name();
            let name_str = match file_name.to_str() {
                Some(s) => s,
                None => continue,
            };
            if name_str.parse::<u32>().is_err() {
                continue;
            }
            let comm =
                fs::read_to_string(format!("/proc/{name_str}/comm")).unwrap_or_default();
            let name = comm.trim().to_string();
            let path = fs::read_link(format!("/proc/{name_str}/exe"))
                .ok()
                .map(|p| p.display().to_string());
            out.push(ProcInfo { name, path });
        }
        out
    }
}
