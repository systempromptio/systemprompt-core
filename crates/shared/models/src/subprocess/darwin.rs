//! macOS backend for child supervision: identity from `sysctl(KERN_PROCARGS2)`
//! and the zombie check from `proc_pidinfo`.
//!
//! Darwin has no `/proc`, but the kernel will hand back another process's
//! argument-and-environment blob over `sysctl` for a process owned by the same
//! uid, which is exactly the case for a child this installation spawned.
//!
//! One target is out of reach: a binary running under Apple's hardened runtime
//! — `/bin/sleep` and the rest of the signed system tools — has its environment
//! withheld from the blob even for a same-uid parent, and the withholding
//! follows the code signature rather than the path, so copying the binary
//! elsewhere does not lift it. Ordinary binaries are readable, which covers the
//! MCP servers and agents this supervisor actually spawns (our own executables,
//! `node`, `python3`). A child that is a hardened system binary simply never
//! verifies, and the fail-closed path below applies to it like any other
//! unreadable target: it is reported as unverified, never signalled on a guess.
//!
//! Every failure path — a dead pid, a process owned by someone else, a
//! hardened target, a short read — is fail-closed.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;

#[must_use]
pub fn live_pid_is_subprocess(pid: u32, name_key: &str, service_name: &str) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };

    match process_args_blob(pid) {
        Ok(blob) => super::environ_from_procargs2(&blob).is_some_and(|environ| {
            super::environ_identifies_child(environ, name_key, service_name)
        }),
        Err(e) => {
            tracing::warn!(pid, error = %e, "Could not read process environ to verify child identity");
            false
        },
    }
}

#[must_use]
#[expect(
    unsafe_code,
    reason = "proc_pidinfo is the only interface exposing a process state on Darwin; libc \
              publishes no kinfo_proc for Apple, so there is no safe wrapper to call instead"
)]
pub fn is_zombie(pid: u32) -> bool {
    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    let Ok(want) = libc::c_int::try_from(size_of::<libc::proc_bsdshortinfo>()) else {
        return false;
    };

    let mut info = std::mem::MaybeUninit::<libc::proc_bsdshortinfo>::zeroed();
    // SAFETY: `info` is a live, correctly sized, correctly aligned allocation
    // for the `PROC_PIDT_SHORTBSDINFO` flavour, and `want` is its exact size.
    // The kernel writes at most that many bytes and returns how many it wrote.
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDT_SHORTBSDINFO,
            0,
            info.as_mut_ptr().cast(),
            want,
        )
    };
    if written != want {
        return false;
    }

    // SAFETY: the call above returned a full-size write, so every field is
    // initialised.
    let info = unsafe { info.assume_init() };
    info.pbsi_status == libc::SZOMB
}

#[expect(
    unsafe_code,
    reason = "sysctl is the only interface exposing another process's environment on Darwin; \
              there is no safe wrapper for KERN_PROCARGS2 in the dependency set"
)]
fn process_args_blob(pid: libc::c_int) -> io::Result<Vec<u8>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut buf = vec![0u8; arg_max()?];
    let mut len = buf.len();

    // SAFETY: `mib` holds the three entries `namelen` claims, and `buf`/`len`
    // describe one live allocation whose length the kernel both reads as a cap
    // and overwrites with the byte count it produced.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buf.as_mut_ptr().cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    buf.truncate(len);
    Ok(buf)
}

#[expect(
    unsafe_code,
    reason = "sizing the KERN_PROCARGS2 buffer needs KERN_ARGMAX, which is only readable through \
              the same raw sysctl interface"
)]
fn arg_max() -> io::Result<usize> {
    let mut mib = [libc::CTL_KERN, libc::KERN_ARGMAX];
    let mut value: libc::c_int = 0;
    let mut len = size_of::<libc::c_int>();

    // SAFETY: `mib` holds the two entries `namelen` claims, and `value`/`len`
    // describe a single live `c_int` of exactly the size the kernel writes.
    let rc = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            2,
            (&raw mut value).cast(),
            &raw mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }

    usize::try_from(value).map_err(|e| io::Error::other(format!("KERN_ARGMAX is unusable: {e}")))
}
