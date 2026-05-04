//! POSIX (`#[cfg(unix)]`) backend for [`super::ProcessCleanup`].

use std::process::Command;

use super::ProcessInfo;

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
        // Why: stdout from `lsof -ti` is one PID per line; trim+parse failures mean the
        // OS returned non-numeric output, which we treat as "no PID".
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .and_then(|pid| pid.trim().parse::<u32>().ok())
    }
}

pub(super) fn kill_process(pid: u32) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL).is_ok()
}

pub(super) async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    if signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM).is_err() {
        return false;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}

pub(super) fn process_exists(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

pub(super) fn kill_by_pattern(pattern: &str) -> usize {
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

    // Why: `lsof -ti` may produce empty stdout when the port is free, and
    // non-numeric output indicates an OS error we cannot recover from here.
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
        .to_string();

    Some(ProcessInfo { pid, name, port })
}
