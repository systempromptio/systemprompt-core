//! POSIX (`#[cfg(unix)]`) backend for [`super::ProcessCleanup`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::num::NonZeroU16;
use std::process::Command;

use super::listener::parse_lsof_pids;

const TERMINATION_POLL_INTERVAL_MS: u64 = 50;

fn is_safe_pattern(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 128
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
}

pub(super) fn listener_pids(port: NonZeroU16) -> Vec<u32> {
    let port_filter = format!("-iTCP:{port}");
    match Command::new("lsof")
        .args(["-nP", &port_filter, "-sTCP:LISTEN", "-t"])
        .output()
    {
        Ok(output) => parse_lsof_pids(&String::from_utf8_lossy(&output.stdout)),
        Err(e) => {
            tracing::warn!(
                port = port.get(),
                error = %e,
                "Failed to run lsof while checking port; treating as unknown",
            );
            vec![]
        },
    }
}

pub(super) fn process_name(pid: u32) -> Option<String> {
    let output = match Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(pid, error = %e, "Failed to run ps while inspecting process");
            return None;
        },
    };
    let name = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!name.is_empty()).then_some(name)
}

pub(super) fn kill_process(pid: u32) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;
    let Some(pid) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return false;
    };
    signal::kill(Pid::from_raw(pid), Signal::SIGKILL).is_ok()
}

pub(super) fn process_group(pid: u32) -> Option<u32> {
    use nix::unistd::{Pid, getpgid};
    let pid = systemprompt_models::subprocess::signalable_pid(pid)?;
    getpgid(Some(Pid::from_raw(pid)))
        .ok()
        .map(|pgid| pgid.as_raw() as u32)
}

pub(super) async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let Some(raw) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return false;
    };

    if signal::kill(Pid::from_raw(raw), Signal::SIGTERM).is_err() {
        return false;
    }

    if wait_for_exit(pid, grace_period_ms).await {
        return true;
    }

    kill_process(pid)
}

async fn wait_for_exit(pid: u32, grace_period_ms: u64) -> bool {
    let mut waited = 0;
    while waited < grace_period_ms {
        if !process_is_live(pid) {
            return true;
        }
        let step = TERMINATION_POLL_INTERVAL_MS.min(grace_period_ms - waited);
        tokio::time::sleep(tokio::time::Duration::from_millis(step)).await;
        waited += step;
    }
    !process_is_live(pid)
}

fn process_is_live(pid: u32) -> bool {
    process_exists(pid) && !systemprompt_loader::subprocess::is_zombie(pid)
}

// Why: POSIX reuses process IDs; negative kill targets address groups.
pub(super) async fn terminate_group_gracefully(pgid: u32, grace_period_ms: u64) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::{Pid, getpgid};

    let Some(raw) = systemprompt_models::subprocess::signalable_pid(pgid) else {
        return false;
    };
    let leader = Pid::from_raw(raw);

    if getpgid(Some(leader)) != Ok(leader) {
        return terminate_gracefully(pgid, grace_period_ms).await;
    }

    let group = Pid::from_raw(-raw);

    if signal::kill(group, Signal::SIGTERM).is_err() {
        return terminate_gracefully(pgid, grace_period_ms).await;
    }

    if wait_for_exit(pgid, grace_period_ms).await {
        return true;
    }

    signal::kill(group, Signal::SIGKILL).is_ok()
}

pub(super) fn process_exists(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    let Some(pid) = systemprompt_models::subprocess::signalable_pid(pid) else {
        return false;
    };
    signal::kill(Pid::from_raw(pid), None).is_ok()
}

pub(super) fn kill_by_pattern(pattern: &str) -> usize {
    if !is_safe_pattern(pattern) {
        tracing::warn!(pattern = %pattern, "rejecting kill_by_pattern: pattern contains unsafe characters");
        return 0;
    }
    match Command::new("pkill").args(["-9", "-f", pattern]).output() {
        Ok(output) => usize::from(output.status.success()),
        Err(e) => {
            tracing::warn!(
                pattern = %pattern,
                error = %e,
                "Failed to run pkill",
            );
            0
        },
    }
}
