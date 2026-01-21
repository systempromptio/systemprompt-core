use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use std::process::Command;
use systemprompt_models::{AppPaths, CliPaths, Config, ProfileBootstrap, SecretsBootstrap};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

fn rotate_log_if_needed(log_path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::metadata(log_path) {
        if metadata.len() > MAX_LOG_SIZE {
            let backup_path = log_path.with_extension("log.old");
            fs::rename(log_path, &backup_path)?;
        }
    }
    Ok(())
}

pub async fn spawn_detached(agent_name: &str, port: u16) -> OrchestrationResult<u32> {
    let paths = AppPaths::get()
        .map_err(|e| OrchestrationError::ProcessSpawnFailed(format!("Failed to get paths: {e}")))?;

    let binary_path = paths.build().resolve_binary("systemprompt").map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to find systemprompt binary: {e}"))
    })?;

    let config = Config::get().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get config: {e}"))
    })?;

    let log_dir = paths.system().logs();
    if let Err(e) = fs::create_dir_all(&log_dir) {
        tracing::error!(
            error = %e,
            path = %log_dir.display(),
            "Failed to create agent log directory - agent may fail to start"
        );
    }

    let log_file_path = log_dir.join(format!("agent-{}.log", agent_name));
    if let Err(e) = rotate_log_if_needed(&log_file_path) {
        tracing::warn!(
            error = %e,
            path = %log_file_path.display(),
            "Failed to rotate agent log file"
        );
    }

    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            OrchestrationError::ProcessSpawnFailed(format!(
                "Failed to create log file {}: {}",
                log_file_path.display(),
                e
            ))
        })?;

    let secrets = SecretsBootstrap::get().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get secrets: {e}"))
    })?;

    let profile_path = ProfileBootstrap::get_path().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get profile path: {e}"))
    })?;

    let mut command = Command::new(&binary_path);
    for arg in CliPaths::agent_run_args() {
        command.arg(arg);
    }
    command
        .arg("--agent-name")
        .arg(agent_name)
        .arg("--port")
        .arg(port.to_string())
        .envs(std::env::vars())
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("JWT_SECRET", &secrets.jwt_secret)
        .env("DATABASE_URL", &secrets.database_url)
        .env("AGENT_NAME", agent_name)
        .env("AGENT_PORT", port.to_string())
        .env("DATABASE_TYPE", &config.database_type)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());

    if let Some(ref key) = secrets.gemini {
        command.env("GEMINI_API_KEY", key);
    }
    if let Some(ref key) = secrets.anthropic {
        command.env("ANTHROPIC_API_KEY", key);
    }
    if let Some(ref key) = secrets.openai {
        command.env("OPENAI_API_KEY", key);
    }
    if let Some(ref key) = secrets.github {
        command.env("GITHUB_TOKEN", key);
    }

    if !secrets.custom.is_empty() {
        let custom_keys: Vec<&str> = secrets.custom.keys().map(String::as_str).collect();
        command.env("SYSTEMPROMPT_CUSTOM_SECRETS", custom_keys.join(","));
        for (key, value) in &secrets.custom {
            command.env(key, value);
        }
    }

    let child = command.spawn().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to spawn {agent_name}: {e}"))
    })?;

    let pid = child.id();

    std::mem::forget(child);

    if !verify_process_started(pid) {
        return Err(OrchestrationError::ProcessSpawnFailed(format!(
            "Agent {} (PID {}) died immediately after spawn",
            agent_name, pid
        )));
    }

    tracing::debug!(pid = %pid, agent_name = %agent_name, "Detached process spawned");
    Ok(pid)
}

fn verify_process_started(pid: u32) -> bool {
    use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
    use nix::unistd::Pid;

    match waitpid(Pid::from_raw(pid as i32), Some(WaitPidFlag::WNOHANG)) {
        Ok(WaitStatus::StillAlive) => true,
        Ok(_) => false,
        Err(_) => process_exists(pid),
    }
}

pub fn process_exists(pid: u32) -> bool {
    Path::new(&format!("/proc/{}", pid)).exists()
}

pub fn terminate_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGTERM)
        .with_context(|| format!("Failed to send SIGTERM to PID {pid}"))?;

    Ok(())
}

pub fn force_kill_process(pid: u32) -> Result<()> {
    use nix::sys::signal::{self, Signal};
    use nix::unistd::Pid;

    signal::kill(Pid::from_raw(pid as i32), Signal::SIGKILL)
        .with_context(|| format!("Failed to send SIGKILL to PID {pid}"))?;

    Ok(())
}

pub async fn terminate_gracefully(pid: u32, timeout_secs: u64) -> Result<()> {
    terminate_process(pid)?;

    let check_interval = tokio::time::Duration::from_millis(100);
    let max_checks = (timeout_secs * 1000) / 100;

    for _ in 0..max_checks {
        if !process_exists(pid) {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    force_kill_process(pid)?;

    for _ in 0..50 {
        if !process_exists(pid) {
            return Ok(());
        }
        tokio::time::sleep(check_interval).await;
    }

    Err(anyhow::anyhow!(
        "Failed to kill process {} even with SIGKILL",
        pid
    ))
}

pub fn kill_process(pid: u32) -> bool {
    terminate_process(pid).is_ok()
}

pub fn is_port_in_use(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(format!("127.0.0.1:{port}")).is_err()
}

pub async fn spawn_detached_process(agent_name: &str, port: u16) -> OrchestrationResult<u32> {
    spawn_detached(agent_name, port).await
}

pub fn validate_agent_binary() -> Result<()> {
    let paths = AppPaths::get().map_err(|e| anyhow::anyhow!("{}", e))?;
    let binary_path = paths.build().resolve_binary("systemprompt")?;

    let metadata = fs::metadata(&binary_path)
        .with_context(|| format!("Failed to get metadata for: {}", binary_path.display()))?;

    if !metadata.is_file() {
        return Err(anyhow::anyhow!(
            "Agent binary is not a file: {}",
            binary_path.display()
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            return Err(anyhow::anyhow!(
                "Agent binary is not executable: {}",
                binary_path.display()
            ));
        }
    }

    Ok(())
}
