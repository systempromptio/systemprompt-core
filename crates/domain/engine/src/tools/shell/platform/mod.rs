//! Stopping a shell command with everything it started: a Job Object on
//! Windows, a process group elsewhere.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use tokio::process::Child;

const TERM_GRACE: Duration = Duration::from_secs(2);

#[derive(Debug)]
pub(super) struct ProcessTree {
    #[cfg(windows)]
    job: Option<job::JobObject>,
    #[cfg(unix)]
    group: Option<i32>,
}

impl ProcessTree {
    #[cfg(windows)]
    pub(super) fn attach(child: &Child) -> Self {
        let job = child.raw_handle().and_then(job::JobObject::adopt);
        if job.is_none() {
            tracing::debug!("could not place the shell command in a job object");
        }
        Self { job }
    }

    #[cfg(unix)]
    pub(super) fn attach(child: &Child) -> Self {
        Self {
            group: child.id().and_then(|pid| i32::try_from(pid).ok()),
        }
    }

    pub(super) async fn kill(&self, child: &mut Child) {
        #[cfg(windows)]
        {
            if let Some(job) = &self.job {
                job.terminate();
            }
        }
        #[cfg(unix)]
        {
            if let Some(group) = self.group {
                group::signal(group, libc::SIGTERM);
                if tokio::time::timeout(TERM_GRACE, child.wait())
                    .await
                    .is_err()
                {
                    tracing::debug!(group, "shell command outlived its terminate grace period");
                }
                group::signal(group, libc::SIGKILL);
            }
        }
        if let Err(err) = child.start_kill() {
            tracing::trace!(error = %err, "shell command had already exited when killed");
        }
        match tokio::time::timeout(TERM_GRACE * 2, child.wait()).await {
            Ok(Ok(_)) => {},
            Ok(Err(err)) => tracing::debug!(error = %err, "could not reap the shell command"),
            Err(_) => tracing::debug!("shell command did not exit after it was killed"),
        }
    }

    #[cfg(windows)]
    pub(super) fn release(&mut self) {
        if let Some(job) = self.job.take() {
            job.terminate();
        }
    }

    #[cfg(unix)]
    pub(super) fn release(&mut self) {
        if let Some(group) = self.group.take() {
            group::signal(group, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "kill(2) is the only way to signal the process group a shell command runs in"
)]
mod group {
    pub(super) fn signal(group: i32, signal: libc::c_int) {
        // SAFETY: kill(2) with a negative pid signals the process group created for
        // this command; no memory is shared.
        let sent = unsafe { libc::kill(-group, signal) };
        if sent != 0 {
            tracing::trace!(
                group,
                signal,
                "process group was already gone when signalled"
            );
        }
    }
}

#[cfg(windows)]
#[expect(
    unsafe_code,
    reason = "Job Objects are reachable only through the Win32 API"
)]
mod job {
    use std::os::windows::io::RawHandle;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    #[derive(Debug)]
    pub(super) struct JobObject(HANDLE);

    // SAFETY: a job handle is a kernel object handle that may be used and closed
    // from any thread.
    unsafe impl Send for JobObject {}
    // SAFETY: the only operations, TerminateJobObject and CloseHandle, are
    // thread-safe kernel calls.
    unsafe impl Sync for JobObject {}

    impl JobObject {
        pub(super) fn adopt(process: RawHandle) -> Option<Self> {
            // SAFETY: null security attributes and a null name create an anonymous job with
            // default security.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return None;
            }
            let job = Self(handle);
            let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).ok()?;
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
                return None;
            }
            // SAFETY: `process` is the live process handle owned by the tokio Child, valid
            // for this call.
            let assigned = unsafe { AssignProcessToJobObject(job.0, process.cast()) };
            if assigned == 0 {
                return None;
            }
            Some(job)
        }

        pub(super) fn terminate(&self) {
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
