//! Process spawning helpers used by the agent orchestrator.
//!
//! - `command` builds the `Command` for an agent subprocess and rotates its log
//!   file.
//!
//! Liveness, identity and termination of a spawned agent go through
//! [`systemprompt_loader::subprocess`]: a recorded pid is signalled only after
//! its agent marker is read back from the live process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod command;

use systemprompt_config::paths::{AppPaths, BuildPaths};
use systemprompt_config::{ProfileBootstrap, SecretsBootstrap};
use systemprompt_identifiers::AgentName;
use systemprompt_models::Config;

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

pub async fn spawn_detached(
    paths: &AppPaths,
    agent_name: &AgentName,
    port: u16,
) -> OrchestrationResult<u32> {
    let binary_path = BuildPaths::resolve_self()
        .map_err(|e| OrchestrationError::spawn("Failed to resolve running binary", e))?;

    let config = Config::get().map_err(|e| OrchestrationError::spawn("Failed to get config", e))?;

    let secrets = SecretsBootstrap::get()
        .map_err(|e| OrchestrationError::spawn("Failed to get secrets", e))?;

    let profile_path = ProfileBootstrap::get_path()
        .map_err(|e| OrchestrationError::spawn("Failed to get profile path", e))?;

    let log_file = command::prepare_agent_log_file(agent_name, &paths.system().logs())?;

    let cmd = command::build_agent_command(command::BuildAgentCommandParams {
        binary_path: &binary_path,
        agent_name,
        port,
        profile_path,
        secrets,
        config,
        log_file,
    });

    let pid = systemprompt_loader::subprocess::spawn_supervised(cmd)
        .map_err(|e| OrchestrationError::spawn(format!("Failed to spawn {agent_name}"), e))?;

    if !systemprompt_loader::subprocess::is_running(pid).await {
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
