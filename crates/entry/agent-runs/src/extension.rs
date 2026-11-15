//! [`AgentRunsApiExtension`] (`agent_runs_api`): the `/api/v1/agent-runs`
//! route set, behind the user gate with a user context, the caller's
//! `agent_runs` rate limit on the tasks budget and the default body limit, in
//! the `Agent` route group, so every node role that serves the agent protocol
//! serves it. The shape equals the reference composition's conferral for
//! `agent_runs_api` at `Api(AgentRuns)`.
//!
//! The routes read the run repository the `agent_runs` member provides, which
//! this member `requires`, and the admitted run origins:
//!
//! | profile disables | agent-runs routes |
//! |---|---|
//! | `agent_runs_api` | 404 |
//! | `agent_runs` | boot refused |
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

pub const AGENT_RUNS_API_EXTENSION: &str = "agent_runs_api";

/// The member that contributes the agent-runs route set.
#[derive(Debug, Clone, Copy, Default)]
pub struct AgentRunsApiExtension;

impl Extension for AgentRunsApiExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: AGENT_RUNS_API_EXTENSION,
            name: "Agent runs API",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        #[cfg(feature = "http")]
        crate::http::contribute(c);
        #[cfg(not(feature = "http"))]
        let _ = c;
        Ok(())
    }
}
