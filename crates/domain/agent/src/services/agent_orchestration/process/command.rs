//! Build the `Command` used to spawn a detached agent subprocess and rotate its
//! log file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::services::shared::Result;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;
use systemprompt_models::{CliPaths, Config, Secrets};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

const MAX_LOG_SIZE: u64 = 10 * 1024 * 1024;

pub fn rotate_log_if_needed(log_path: &Path) -> Result<()> {
    if let Ok(metadata) = fs::metadata(log_path)
        && metadata.len() > MAX_LOG_SIZE
    {
        let backup_path = log_path.with_extension("log.old");
        fs::rename(log_path, &backup_path)?;
    }
    Ok(())
}

pub fn prepare_agent_log_file(agent_name: &str, log_dir: &Path) -> OrchestrationResult<File> {
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

#[derive(Debug)]
pub struct BuildAgentCommandParams<'a> {
    pub binary_path: &'a PathBuf,
    pub agent_name: &'a str,
    pub port: u16,
    pub profile_path: &'a str,
    pub secrets: &'a Secrets,
    pub config: &'a Config,
    pub log_file: File,
}

/// The environment an agent child is spawned with.
///
/// Returned as pairs rather than applied straight onto a [`Command`] so it can
/// be asserted on directly. The MCP spawner's equivalent already had this shape
/// and was covered by tests that inject a lookup; this one only had a `Command`,
/// which is why its copy of the inherited list could drift without anything
/// noticing.
pub fn build_agent_environment(
    agent_name: &str,
    port: u16,
    profile_path: &str,
    database_type: &str,
    secrets: &Secrets,
    lookup: impl Fn(&str) -> Option<String>,
) -> Vec<(String, String)> {
    // Why: shared with the MCP spawner rather than copied. The two lists drifted
    // once already — this one kept the deployment-host marker through a bolt-on
    // while the MCP one silently lost it, and every MCP server on a deployed
    // host then believed it was somewhere else.
    let mut env = systemprompt_models::subprocess::inherited_parent_env(lookup);

    env.push(("SYSTEMPROMPT_PROFILE".to_owned(), profile_path.to_owned()));
    env.push((
        systemprompt_models::subprocess::SUBPROCESS_MARKER_ENV.to_owned(),
        "1".to_owned(),
    ));
    env.push((
        systemprompt_models::subprocess::AGENT_NAME_ENV.to_owned(),
        agent_name.to_owned(),
    ));
    env.push(("AGENT_PORT".to_owned(), port.to_string()));
    env.push(("DATABASE_TYPE".to_owned(), database_type.to_owned()));
    env.extend(secrets.to_subprocess_env());

    env
}

pub fn build_agent_command(params: BuildAgentCommandParams<'_>) -> Command {
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

    for (key, value) in build_agent_environment(
        agent_name,
        port,
        profile_path,
        &config.database_type,
        secrets,
        |name| std::env::var(name).ok(),
    ) {
        command.env(key, value);
    }

    command
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::from(log_file))
        .stdin(std::process::Stdio::null());

    systemprompt_models::subprocess::place_in_own_process_group(&mut command);

    command
}
