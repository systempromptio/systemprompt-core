use anyhow::Result;
use std::process::Command;

pub const MAX_PORT_CLEANUP_ATTEMPTS: u32 = 5;
pub const PORT_BACKOFF_BASE_MS: u64 = 200;
pub const POST_KILL_DELAY_MS: u64 = 500;

pub async fn prepare_port(port: u16) -> Result<()> {
    tracing::debug!(port = port, "Preparing port");

    if is_port_in_use(port) {
        tracing::debug!(port = port, "Port is in use, cleaning up");
        cleanup_port_processes(port).await?;
    }

    tracing::debug!(port = port, "Port is ready");
    Ok(())
}

pub fn is_port_in_use(port: u16) -> bool {
    std::net::TcpStream::connect(format!("127.0.0.1:{port}")).is_ok()
}

pub fn is_port_responsive(port: u16) -> bool {
    is_port_in_use(port)
}

#[cfg(unix)]
pub async fn cleanup_port_processes(port: u16) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    let output = Command::new("lsof")
        .args(["-ti", &format!(":{port}")])
        .output()?;

    if !output.stdout.is_empty() {
        let pids = String::from_utf8_lossy(&output.stdout);
        for pid_str in pids.lines() {
            if let Ok(pid) = pid_str.trim().parse::<i32>() {
                tracing::debug!(port = port, pid = pid, "Stopping process on port");

                let _ = signal::kill(Pid::from_raw(pid), Signal::SIGTERM);

                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                let _ = signal::kill(Pid::from_raw(pid), Signal::SIGKILL);
            }
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }

    Ok(())
}

#[cfg(windows)]
pub async fn cleanup_port_processes(port: u16) -> Result<()> {
    let output = Command::new("netstat")
        .args(["-ano", "-p", "TCP"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let port_pattern = format!(":{port} ");

    for line in stdout.lines() {
        if line.contains(&port_pattern) {
            if let Some(pid_str) = line.split_whitespace().last() {
                if pid_str.parse::<u32>().is_ok() {
                    tracing::debug!(port = port, pid = %pid_str, "Stopping process on port");

                    let _ = Command::new("taskkill").args(["/PID", pid_str]).output();

                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

                    let _ = Command::new("taskkill")
                        .args(["/PID", pid_str, "/F"])
                        .output();
                }
            }
        }
    }

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

    Ok(())
}

pub async fn wait_for_port_release(port: u16) -> Result<()> {
    let max_attempts = 10;
    let delay = std::time::Duration::from_millis(100);

    for attempt in 1..=max_attempts {
        if !is_port_in_use(port) {
            return Ok(());
        }

        if attempt < max_attempts {
            tokio::time::sleep(delay).await;
        }
    }

    Err(anyhow::anyhow!(
        "Port {port} did not become available after {max_attempts} attempts"
    ))
}

pub async fn wait_for_port_release_with_retry(port: u16, max_cleanup_attempts: u32) -> Result<()> {
    for cleanup_attempt in 1..=max_cleanup_attempts {
        if !is_port_in_use(port) {
            return Ok(());
        }

        tracing::debug!(
            port = port,
            attempt = cleanup_attempt,
            max_attempts = max_cleanup_attempts,
            "Port still in use, attempting cleanup"
        );

        cleanup_port_processes(port).await?;

        match wait_for_port_release(port).await {
            Ok(()) => return Ok(()),
            Err(_) if cleanup_attempt < max_cleanup_attempts => {
                let backoff = std::time::Duration::from_millis(
                    PORT_BACKOFF_BASE_MS * u64::from(cleanup_attempt),
                );
                tokio::time::sleep(backoff).await;
            },
            Err(e) => return Err(e),
        }
    }

    Err(anyhow::anyhow!(
        "Port {port} could not be acquired after {max_cleanup_attempts} cleanup attempts"
    ))
}

pub const fn cleanup_port_resources(_port: u16) {}

pub fn find_available_port(start_port: u16, end_port: u16) -> Result<u16> {
    for port in start_port..=end_port {
        if !is_port_in_use(port) {
            return Ok(port);
        }
    }

    Err(anyhow::anyhow!(
        "No available ports in range {start_port}-{end_port}"
    ))
}
