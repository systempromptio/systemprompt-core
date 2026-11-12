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
pub mod grants;

use std::path::PathBuf;
use std::sync::Arc;
use systemprompt_config::SecretsService;
use systemprompt_config::paths::{AppPaths, BuildPaths};
use systemprompt_identifiers::AgentName;
use systemprompt_loader::ServicesState;
use systemprompt_loader::subprocess::Spawner;
use systemprompt_manifest::Config;
use systemprompt_manifest::child_env::ChildBoot;

use crate::services::agent_orchestration::{OrchestrationError, OrchestrationResult};

#[derive(Debug, Clone)]
pub struct SpawnEnvironment {
    pub config: Arc<Config>,
    pub secrets: Arc<SecretsService>,
    pub services: ServicesState,
    pub profile_path: PathBuf,
}

pub async fn spawn_detached(
    paths: &AppPaths,
    spawner: &Spawner,
    environment: &SpawnEnvironment,
    agent_name: &AgentName,
    port: u16,
) -> OrchestrationResult<u32> {
    let binary_path = BuildPaths::resolve_self()
        .map_err(|e| OrchestrationError::spawn("Failed to resolve running binary", e))?;

    let secrets_env = agent_secrets(environment, agent_name)?;
    let profile_path = environment.profile_path.to_str().ok_or_else(|| {
        OrchestrationError::ProcessSpawnFailed(format!(
            "profile path {} is not valid UTF-8",
            environment.profile_path.display()
        ))
    })?;

    let log_file = command::prepare_agent_log_file(agent_name, &paths.system().logs())?;

    let cmd = command::build_agent_command(command::BuildAgentCommandParams {
        binary_path: &binary_path,
        agent_name,
        port,
        profile_path,
        secrets_env: &secrets_env,
        config: &environment.config,
        log_file,
    });

    let pid = spawner
        .spawn_supervised(cmd)
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

fn agent_secrets(
    environment: &SpawnEnvironment,
    agent_name: &AgentName,
) -> OrchestrationResult<Vec<(String, String)>> {
    let services = environment.services.config();
    let agent = services.agents.get(agent_name.as_str()).ok_or_else(|| {
        OrchestrationError::ProcessSpawnFailed(format!(
            "agent {agent_name} is not declared in the services configuration"
        ))
    })?;
    let derived = grants::model_credential_secrets(agent, environment.services.catalog());
    let env = environment
        .secrets
        .child_env(ChildBoot::Agent, &agent.secrets, &derived)
        .map_err(|e| {
            OrchestrationError::spawn(
                format!("Failed to resolve the granted secrets for agent {agent_name}"),
                e,
            )
        })?;
    if agent
        .secrets
        .contains(&systemprompt_models::subprocess::SecretGrant::EncryptionMasterKey)
    {
        tracing::info!(agent = %agent_name, grant = "encryption_master_key", "agent granted the encryption master key");
    }
    Ok(env)
}

pub fn is_port_in_use(port: u16) -> bool {
    use std::net::TcpListener;
    TcpListener::bind(format!("127.0.0.1:{port}")).is_err()
}
