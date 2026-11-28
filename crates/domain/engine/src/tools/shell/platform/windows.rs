//! Windows: `PowerShell` 7, then Git Bash, then Windows `PowerShell` 5.1, then
//! cmd; output that is not UTF-8 is read in the console code page; a command
//! runs in a Job Object that is terminated to stop it with everything it
//! started, and that ends its processes when the engine itself goes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::path::PathBuf;

use tokio::process::{Child, Command};

use super::{Platform, Tree, powershell};
use crate::tools::shell::select::{ShellKind, ShellSpec};

pub(super) const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Windows;

impl Platform for Windows {
    type Tree = job::JobObject;
    type TreeError = job::JobError;

    const UNNAMED_SHELL: ShellKind = ShellKind::GitBash;

    fn default_shell() -> ShellSpec {
        on_path(ShellKind::Pwsh, "pwsh")
            .or_else(|| {
                git_bash().map(|path| ShellSpec {
                    path,
                    kind: ShellKind::GitBash,
                })
            })
            .or_else(|| on_path(ShellKind::WindowsPowerShell, "powershell"))
            .unwrap_or_else(|| ShellSpec {
                path: which::which("cmd").unwrap_or_else(|_| PathBuf::from("cmd.exe")),
                kind: ShellKind::Cmd,
            })
    }

    fn prepare(command: &mut Command, kind: ShellKind, script: &str) {
        match kind {
            ShellKind::Pwsh | ShellKind::WindowsPowerShell => powershell(command, script),
            ShellKind::Cmd => {
                // Why: cmd.exe parses its own command line, so the script is passed verbatim
                // inside /s /c "...".
                command.raw_arg(format!("/d /s /c \"{script}\""));
            },
            ShellKind::GitBash | ShellKind::Posix => {
                command.arg("-c").arg(script);
            },
        }
    }

    fn spawn(&self, mut command: Command) -> io::Result<Child> {
        command.spawn()
    }

    fn adopt(child: &Child) -> Result<job::JobObject, job::JobError> {
        let process = child.raw_handle().ok_or(job::JobError::Reaped)?;
        job::JobObject::adopt(process)
    }

    fn decode(bytes: &[u8]) -> String {
        code_page::decode(bytes).unwrap_or_else(|| String::from_utf8_lossy(bytes).into_owned())
    }
}

fn on_path(kind: ShellKind, name: &str) -> Option<ShellSpec> {
    which::which(name).ok().map(|path| ShellSpec { path, kind })
}

// Why: System32\bash.exe is the WSL launcher, so Git Bash is found next to
// git.exe, not on PATH.
fn git_bash() -> Option<PathBuf> {
    if let Ok(git) = which::which("git") {
        for root in git.ancestors().skip(1).take(3) {
            let candidate = root.join("bin").join("bash.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    [
        r"C:\Program Files\Git\bin\bash.exe",
        r"C:\Program Files (x86)\Git\bin\bash.exe",
    ]
    .iter()
    .map(PathBuf::from)
    .find(|candidate| candidate.is_file())
}

#[expect(
    unsafe_code,
    reason = "Job Objects are reachable only through the Win32 API"
)]
mod job {
    use std::io;
    use std::num::TryFromIntError;
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    use super::Tree;

    #[derive(Debug, thiserror::Error)]
    pub(in crate::tools::shell::platform) enum JobError {
        #[error("the command has no process handle; it was already reaped")]
        Reaped,
        #[error("could not create a job object")]
        Create(#[source] io::Error),
        #[error("the job limit structure does not fit the size the system takes")]
        Layout(#[from] TryFromIntError),
        #[error("could not set the job object to end its processes when it closes")]
        Configure(#[source] io::Error),
        #[error("could not place the command in its job object")]
        Assign(#[source] io::Error),
    }

    #[derive(Debug)]
    pub(in crate::tools::shell::platform) struct JobObject(HANDLE);

    // SAFETY: a job handle is a kernel object handle that may be used and closed
    // from any thread.
    unsafe impl Send for JobObject {}
    // SAFETY: the only operations, TerminateJobObject and CloseHandle, are
    // thread-safe kernel calls.
    unsafe impl Sync for JobObject {}

    impl JobObject {
        pub(super) fn adopt(process: RawHandle) -> Result<Self, JobError> {
            // SAFETY: null security attributes and a null name create an anonymous job with
            // default security.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(JobError::Create(io::Error::last_os_error()));
            }
            let job = Self(handle);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())?;
            // SAFETY: `info` is a live, correctly sized
            // JOBOBJECT_EXTENDED_LIMIT_INFORMATION for this class.
            let configured = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&raw const info).cast(),
                    size,
                )
            };
            if configured == 0 {
                return Err(JobError::Configure(io::Error::last_os_error()));
            }
            // SAFETY: `process` is the live process handle owned by the tokio Child, valid
            // for this call.
            let assigned = unsafe { AssignProcessToJobObject(job.0, process.cast()) };
            if assigned == 0 {
                return Err(JobError::Assign(io::Error::last_os_error()));
            }
            Ok(job)
        }
    }

    impl Tree for JobObject {
        fn ask(&self) -> bool {
            false
        }

        fn end(&self) {
            // SAFETY: the handle is owned by this value and open until drop.
            unsafe {
                TerminateJobObject(self.0, 1);
            }
        }
    }

    impl Drop for JobObject {
        fn drop(&mut self) {
            // SAFETY: the handle is owned by this value and closed exactly once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[expect(
    unsafe_code,
    reason = "console code pages are reachable only through the Win32 API"
)]
mod code_page {
    pub(super) fn decode(bytes: &[u8]) -> Option<String> {
        use windows_sys::Win32::Globalization::{
            CP_OEMCP, MB_ERR_INVALID_CHARS, MultiByteToWideChar,
        };
        use windows_sys::Win32::System::Console::GetConsoleOutputCP;

        let len = i32::try_from(bytes.len()).ok().filter(|len| *len > 0)?;
        // SAFETY: GetConsoleOutputCP takes no arguments and only reads process state.
        let console = unsafe { GetConsoleOutputCP() };
        let code_page = if console == 0 { CP_OEMCP } else { console };
        // SAFETY: the input pointer and length describe `bytes`; a null output buffer
        // asks for the required size.
        let size = unsafe {
            MultiByteToWideChar(
                code_page,
                MB_ERR_INVALID_CHARS,
                bytes.as_ptr(),
                len,
                std::ptr::null_mut(),
                0,
            )
        };
        let capacity = usize::try_from(size).ok().filter(|size| *size > 0)?;
        let mut wide = vec![0u16; capacity];
        // SAFETY: `wide` has exactly `size` elements, as the first call asked for.
        let written = unsafe {
            MultiByteToWideChar(
                code_page,
                MB_ERR_INVALID_CHARS,
                bytes.as_ptr(),
                len,
                wide.as_mut_ptr(),
                size,
            )
        };
        let written = usize::try_from(written)
            .ok()
            .filter(|written| *written > 0)?;
        String::from_utf16(&wide[..written.min(capacity)]).ok()
    }
}
