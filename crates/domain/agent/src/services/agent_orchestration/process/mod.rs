//! Process spawning and lifecycle helpers used by the agent orchestrator.
//!
//! - `command` builds the `Command` for an agent subprocess and rotates its log
//!   file.
//! - `signals` cross-platform `process_exists`, `terminate_process`,
//!   `force_kill_process`, `terminate_gracefully`, `kill_process`.

mod command;
mod signals;

use std::fs;
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_models::{AppPaths, Config};

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

pub use signals::{
    force_kill_process, kill_process, process_exists, terminate_gracefully, terminate_process,
};

pub fn spawn_detached(paths: &AppPaths, agent_name: &str, port: u16) -> OrchestrationResult<u32> {
    let binary_path = paths.build().resolve_binary("systemprompt").map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to find systemprompt binary: {e}"))
    })?;

    let config = Config::get().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get config: {e}"))
    })?;

    let secrets = SecretsBootstrap::get().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get secrets: {e}"))
    })?;

    let profile_path = ProfileBootstrap::get_path().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to get profile path: {e}"))
    })?;

    let log_file = command::prepare_agent_log_file(agent_name, &paths.system().logs())?;

    let mut cmd = command::build_agent_command(command::BuildAgentCommandParams {
        binary_path: &binary_path,
        agent_name,
        port,
        profile_path,
        secrets,
        config,
        log_file,
    });

    let child = cmd.spawn().map_err(|e| {
        OrchestrationError::ProcessSpawnFailed(format!("Failed to spawn {agent_name}: {e}"))
    })?;

    let pid = child.id();
    #[expect(
        clippy::mem_forget,
        reason = "detached agent process: skip Child's drop-time wait so the spawned agent keeps running after this fn returns"
    )]
    std::mem::forget(child);

    if !signals::verify_process_started(pid) {
        return Err(OrchestrationError::ProcessSpawnFailed(format!(
            "Agent {} (PID {}) died immediately after spawn",
            agent_name, pid
        )));
    }

    tracing::debug!(pid = %pid, agent_name = %agent_name, "Detached process spawned");
    Ok(pid)
}

pub fn is_port_in_use(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(format!("127.0.0.1:{port}")).is_err()
}

pub fn spawn_detached_process(
    paths: &AppPaths,
    agent_name: &str,
    port: u16,
) -> OrchestrationResult<u32> {
    spawn_detached(paths, agent_name, port)
}

pub fn validate_agent_binary(paths: &AppPaths) -> crate::error::AgentResult<()> {
    let binary_path = paths
        .build()
        .resolve_binary("systemprompt")
        .map_err(|e| crate::error::AgentError::Validation(e.to_string()))?;

    let metadata = fs::metadata(&binary_path).map_err(crate::error::AgentError::Io)?;

    if !metadata.is_file() {
        return Err(crate::error::AgentError::Validation(format!(
            "Agent binary is not a file: {}",
            binary_path.display()
        )));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = metadata.permissions();
        if permissions.mode() & 0o111 == 0 {
            return Err(crate::error::AgentError::Validation(format!(
                "Agent binary is not executable: {}",
                binary_path.display()
            )));
        }
    }

    Ok(())
}
