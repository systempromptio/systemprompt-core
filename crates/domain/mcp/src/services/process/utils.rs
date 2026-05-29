#[cfg(windows)]
use std::process::Command;

#[cfg(unix)]
pub fn process_exists(pid: u32) -> bool {
    use nix::sys::signal;
    use nix::unistd::Pid;
    signal::kill(Pid::from_raw(pid as i32), None).is_ok()
}

#[cfg(windows)]
pub fn process_exists(pid: u32) -> bool {
    match Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid), "/NH"])
        .output()
    {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            !stdout.contains("INFO: No tasks") && !stdout.trim().is_empty()
        },
        Err(e) => {
            tracing::warn!(
                pid = pid,
                error = %e,
                "failed to run `tasklist` while checking process; assuming dead",
            );
            false
        },
    }
}

#[cfg(unix)]
pub fn kill_process(pid: u32) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;
    let raw = pid as i32;
    // Refuse to signal ourselves, and reject ids that cast to a non-positive
    // `i32` — `Pid::from_raw(<= 0)` targets a process group (e.g. `u32::MAX - 1`
    // becomes `-2`), which would broadcast the signal far beyond `pid`.
    if raw <= 0 || pid == std::process::id() {
        return false;
    }
    signal::kill(Pid::from_raw(raw), Signal::SIGKILL).is_ok()
}

#[cfg(windows)]
pub fn kill_process(pid: u32) -> bool {
    match Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output()
    {
        Ok(output) => output.status.success(),
        Err(e) => {
            tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /PID {pid} /F`");
            false
        },
    }
}

#[cfg(unix)]
pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    if (pid as i32) <= 0 || pid == std::process::id() {
        return false;
    }

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

#[cfg(windows)]
pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    if let Err(e) = Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
    {
        tracing::warn!(pid = pid, error = %e, "failed to run `taskkill /PID {pid}`");
        return false;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}
