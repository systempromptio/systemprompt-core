//! Linux backend for child supervision: the `prctl` parent-death signal plus
//! the `/proc`-backed identity and zombie checks.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::process::Command;

#[expect(
    unsafe_code,
    reason = "std::os::unix::process::CommandExt::pre_exec is an unsafe fn; there is no safe way \
              to run code in the forked child before exec, and the parent-death signal must be \
              armed there to cover children that never opt in"
)]
pub(super) fn arm_parent_death_signal(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;

    let supervisor = std::process::id();

    // SAFETY: the closure runs in the forked child between `fork` and `execve`,
    // where only async-signal-safe calls are permitted. `prctl`, `getppid`, and
    // `_exit` are all on that list; nothing here allocates, locks, or logs.
    unsafe {
        cmd.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            // Why: Linux does not deliver the parent-death signal if the parent died before
            // `PR_SET_PDEATHSIG` was installed.
            if libc::getppid() != supervisor as libc::pid_t {
                libc::_exit(0);
            }
            Ok(())
        });
    }
}

#[must_use]
pub fn live_pid_is_subprocess(pid: u32, name_key: &str, service_name: &str) -> bool {
    match std::fs::read(format!("/proc/{pid}/environ")) {
        Ok(environ) => super::environ_identifies_child(&environ, name_key, service_name),
        Err(e) => {
            tracing::warn!(pid, error = %e, "Could not read process environ to verify child identity");
            false
        },
    }
}

#[must_use]
pub fn is_zombie(pid: u32) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    // Why: Linux `/proc/<pid>/stat` permits spaces and `)` inside the parenthesised
    // comm field.
    let Some((_, after_comm)) = stat.rsplit_once(')') else {
        return false;
    };
    after_comm.split_whitespace().next() == Some("Z")
}
