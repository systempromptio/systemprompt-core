//! [`AgentRunsApiExtension`] declares `/api/v1/agent-runs`: the user authz
//! gate, the user context middleware, the caller's `agent_runs` rate limit
//! on the tasks budget, and the `Agent` route group, so every node role that
//! serves the agent protocol serves it.
//!
//! The extension requires the run repository the `agent_runs` extension
//! `provide`s: a profile that disables `agent_runs` while `agent_runs_api`
//! stays enabled is refused at boot with `LoaderError::UnmetStateRequirement`.
//! A profile that disables `agent_runs_api` (alone or with `agent_runs`)
//! mounts no agent-runs route, and the path answers 404.
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

    fn requires(&self) -> Vec<StateRequirement> {
        vec![StateRequirement::of::<AgentRunRepository>()]
    }

    fn router(&self, ctx: &dyn ExtensionContext) -> Option<ExtensionRouter> {
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
