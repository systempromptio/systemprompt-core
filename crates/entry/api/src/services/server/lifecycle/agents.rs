//! Agent reconciliation during server startup.
//!
//! [`reconcile_agents`] starts every enabled agent into a known-clean state
//! through the agent member's supervised process set (`ServiceProcessSet::
//! restart`), retrying failed agents once, and fails server startup if any
//! required agent cannot be brought up — agents are a hard dependency of the
//! API. With no agent process set admitted no agent is supervised here and
//! reconciliation starts nothing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use anyhow::Result;
use futures_util::future::join_all;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_extension::service_process::ServiceProcessSets;
use systemprompt_identifiers::ServiceName;
use systemprompt_manifest::AgentConfig;
use systemprompt_runtime::AppContext;
use systemprompt_traits::{
    DynServiceProcessSet, RestartMode, ServiceProcessError, ServiceProcessKind, StartupEventExt,
    StartupEventSender,
};

pub(super) fn process_set(
    ctx: &AppContext,
    kind: ServiceProcessKind,
) -> Option<DynServiceProcessSet> {
    ctx.kernel()
        .state()
        .find::<Arc<ServiceProcessSets>>()
        .and_then(|sets| sets.of_kind(kind).cloned())
}

pub(crate) fn enabled_agents(ctx: &AppContext) -> Vec<AgentConfig> {
    let is_cloud = ctx.config().is_cloud;
    ctx.services_state()
        .view()
        .config()
        .agents
        .values()
        .filter(|agent| agent.enabled && !(agent.dev_only && is_cloud))
        .cloned()
        .collect()
}

pub async fn reconcile_agents(
    ctx: &AppContext,
    events: Option<&StartupEventSender>,
) -> Result<usize> {
    let enabled_agents = enabled_agents(ctx);
    let required_count = enabled_agents.len();
    let Some(agents) = process_set(ctx, ServiceProcessKind::Agent) else {
        if required_count > 0 {
            tracing::warn!(
                required = required_count,
                "no agent process set is admitted; enabled agents are not supervised here"
            );
        }
        return Ok(0);
    };

    let (started, failed_agents) = start_agents(agents.as_ref(), &enabled_agents, events).await;

    let started = if failed_agents.is_empty() {
        started
    } else {
        events.warning_with_context(
            format!(
                "{} agent(s) failed to start on first attempt",
                failed_agents.len()
            ),
            "Attempting cleanup and retry",
        );
        for (agent, first_error) in &failed_agents {
            tracing::warn!(agent = %agent.name, error = %describe(first_error), "Agent failed on first start attempt");
        }
        let retry: Vec<AgentConfig> = failed_agents.into_iter().map(|(agent, _)| agent).collect();
        let (restarted, retry_failed) = start_agents(agents.as_ref(), &retry, events).await;
        if !retry_failed.is_empty() {
            for (agent, error) in &retry_failed {
                events.agent_failed(agent.name.clone(), describe(error));
            }
            return Err(fatal(&retry_failed));
        }
        started + restarted
    };

    if started < required_count {
        return Err(anyhow::anyhow!(
            "FATAL: Only {}/{} required agents started successfully\n\nAll enabled agents must be \
             running for API to start.",
            started,
            required_count
        ));
    }

    Ok(started)
}

async fn start_agents(
    agents: &dyn systemprompt_traits::ServiceProcessSet,
    configs: &[AgentConfig],
    events: Option<&StartupEventSender>,
) -> (usize, Vec<(AgentConfig, ServiceProcessError)>) {
    let results = join_all(configs.iter().map(|config| async move {
        start_agent(agents, config, events)
            .await
            .map_err(|e| (config.clone(), e))
    }))
    .await;
    let started = results.iter().filter(|result| result.is_ok()).count();
    let failed = results.into_iter().filter_map(Result::err).collect();
    (started, failed)
}

pub(super) async fn start_agent(
    agents: &dyn systemprompt_traits::ServiceProcessSet,
    config: &AgentConfig,
    events: Option<&StartupEventSender>,
) -> Result<(), ServiceProcessError> {
    let name = ServiceName::new(config.name.as_str());
    if let Some(tx) = events {
        tx.agent_starting(config.name.clone(), config.port);
    }
    let start = Instant::now();
    for run in agents.restart(Some(&name), RestartMode::Restart).await? {
        run.outcome?;
    }
    if let Some(tx) = events {
        tx.agent_ready(config.name.clone(), config.port, start.elapsed());
    }
    Ok(())
}

fn describe(error: &ServiceProcessError) -> String {
    let mut text = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        text.push_str(": ");
        text.push_str(&cause.to_string());
        source = cause.source();
    }
    text
}

fn fatal(retry_failed: &[(AgentConfig, ServiceProcessError)]) -> anyhow::Error {
    let agent_names: Vec<&str> = retry_failed
        .iter()
        .map(|(agent, _)| agent.name.as_str())
        .collect();
    anyhow::anyhow!(
        "FATAL: {} required agent(s) failed to start after retry: {}\n\nsystemprompt.io OS \
         cannot operate without all enabled agents.\nAgents are the core service \
         layer.\n\nFailures:\n{}\n\nPossible causes:\n  - Agent binaries not built (run: \
         cargo build)\n  - Ports occupied by non-agent processes (check with: lsof -i:PORT)\n  \
         - Missing environment variables (check .env file)\n  - File permission \
         issues\n\nBuild agents with: cargo build",
        retry_failed.len(),
        agent_names.join(", "),
        retry_failed
            .iter()
            .map(|(agent, err)| format!("  - {}: {}", agent.name, describe(err)))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
