//! Build the `Command` used to spawn a detached agent subprocess and rotate its
//! log file.

use crate::services::shared::Result;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use systemprompt_models::{CliPaths, Config, Secrets};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

pub(super) fn rotate_log_if_needed(log_path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::metadata(log_path) {
        if metadata.len() > MAX_LOG_SIZE {
            let backup_path = log_path.with_extension("log.old");
            fs::rename(log_path, &backup_path)?;
        }
    }
    Ok(())
}

pub(super) fn prepare_agent_log_file(
    agent_name: &str,
    log_dir: &Path,
) -> OrchestrationResult<File> {
    if let Err(e) = fs::create_dir_all(log_dir) {
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

    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)
        .map_err(|e| {
            OrchestrationError::ProcessSpawnFailed(format!(
                "Failed to create log file {}: {}",
                log_file_path.display(),
                e
            ))
        })
}

pub(super) struct BuildAgentCommandParams<'a> {
    pub binary_path: &'a PathBuf,
    pub agent_name: &'a str,
    pub port: u16,
    pub profile_path: &'a str,
    pub secrets: &'a Secrets,
    pub config: &'a Config,
    pub log_file: File,
}

pub(super) fn build_agent_command(params: BuildAgentCommandParams<'_>) -> Command {
    let BuildAgentCommandParams {
        binary_path,
        agent_name,
        port,
        profile_path,
        secrets,
        config,
        log_file,
    } = params;
    let mut command = Command::new(binary_path);
    for arg in CliPaths::agent_run_args() {
        command.arg(arg);
    }
    command
        .arg("--agent-name")
        .arg(agent_name)
        .arg("--port")
        .arg(port.to_string())
        .env_clear();
    if let Ok(path) = std::env::var("PATH") {
        command.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        command.env("HOME", home);
    }
    command
        .env("SYSTEMPROMPT_PROFILE", profile_path)
        .env("SYSTEMPROMPT_SUBPROCESS", "1")
        .env("AGENT_NAME", agent_name)
        .env("AGENT_PORT", port.to_string())
        .env("DATABASE_TYPE", &config.database_type)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());

    for (k, v) in secrets.to_subprocess_env() {
        command.env(k, v);
    }

    if let Ok(fly_app) = std::env::var("FLY_APP_NAME") {
        command.env("FLY_APP_NAME", fly_app);
    }

    command
}
