//! POSIX (`#[cfg(unix)]`) backend for [`super::ProcessCleanup`].

use std::process::Command;

use super::ProcessInfo;

fn is_safe_pattern(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 128
        && p.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
}

pub(super) fn check_port(port: u16) -> Option<u32> {
    let output = match Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                port = port,
                error = %e,
                "failed to run `lsof -ti :{port}` while checking port; treating as unknown",
            );
            return None;
        },
    };

    if output.stdout.is_empty() {
        None
    } else {
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
    }
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

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}

/// Signal a whole process group, escalating SIGTERM to SIGKILL after a grace
/// period, but only when `pgid` still leads its own group — our children are
/// placed in a fresh group (`process_group(0)`, pgid == pid), so a mismatch
/// means the id is recycled and `kill(-pid)` would reach an unrelated session.
/// In that case, and for any non-signalable id, it falls back to single-PID
/// termination rather than broadcasting.
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

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pgid) {
        signal::kill(group, Signal::SIGKILL).is_ok()
    } else {
        true
    }
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
                "failed to run `pkill -9 -f {pattern}`",
            );
            0
        },
    }
}

pub(super) fn get_process_by_port(port: u16) -> Option<ProcessInfo> {
    let output = match Command::new("lsof")
        .args(["-ti", &format!(":{}", port)])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                port = port,
                error = %e,
                "failed to run `lsof -ti :{port}` while inspecting port; treating as unknown",
            );
            return None;
        },
    };

    let pid: u32 = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()?
        .trim()
        .parse()
        .ok()?;

    let comm_output = match Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "comm="])
        .output()
    {
        Ok(output) => output,
        Err(e) => {
            tracing::warn!(
                pid = pid,
                error = %e,
                "failed to run `ps -p {pid} -o comm=` while inspecting process",
            );
            return None;
        },
    };

    let name = String::from_utf8_lossy(&comm_output.stdout)
        .trim()
        .to_owned();

    Some(ProcessInfo { pid, name, port })
}
