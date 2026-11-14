//! [`AgentRunsApiExtension`] declares `/api/v1/agent-runs`: the user authz
//! gate, the user context middleware, the caller's `agent_runs` rate limit
//! on the tasks budget, and the `Agent` route group, so every node role that
//! serves the agent protocol serves it.
//!
//! The run repository the routes read is the one the `agent_runs` extension
//! `provide`s. A profile that disables either `agent_runs` or
//! `agent_runs_api` therefore mounts no agent-runs route, and the path
//! answers 404.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_agent_runs::AgentRunRepository;
use systemprompt_extension::RouteGroup;
use systemprompt_extension::prelude::*;
use systemprompt_models::modules::ApiPaths;

use super::{AgentRunsState, router};

/// The extension that declares the agent-runs route group.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentRunsApiExtension;

impl Extension for AgentRunsApiExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "agent_runs_api",
            name: "Agent runs API",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
        if ctx.extension_state().find::<AgentRunRepository>().is_none() {
            tracing::info!("agent_runs is disabled; agent-runs routes not declared");
            return None;
        }
        let state = match AgentRunsState::from_extension(ctx) {
            Ok(state) => state,
            Err(error) => {
                tracing::error!(%error, "agent-runs routes not declared");
                return None;
            },
        };
        Some(
            ExtensionRouter::new(
                router(state),
                ApiPaths::AGENT_RUNS,
                AuthzPolicy::user(),
                ContextKind::User,
            )
            .in_group(RouteGroup::Agent)
            .with_rate_limit(RateLimit::new("agent_runs", RateBudget::Tasks)),
        )
    }
}

register_extension!(AgentRunsApiExtension);
