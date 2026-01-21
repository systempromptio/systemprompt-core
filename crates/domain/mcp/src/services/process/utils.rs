use std::path::Path;
use std::process::Command;

pub fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn kill_process(pid: u32) -> bool {
    Command::new("kill")
        .args(["-9", &pid.to_string()])
        .output()
        .is_ok_and(|output| output.status.success())
}

pub async fn terminate_gracefully(pid: u32, grace_period_ms: u64) -> bool {
    if Command::new("kill")
        .args(["-15", &pid.to_string()])
        .output()
        .is_err()
    {
        return false;
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(grace_period_ms)).await;

    if process_exists(pid) {
        kill_process(pid)
    } else {
        true
    }
}
