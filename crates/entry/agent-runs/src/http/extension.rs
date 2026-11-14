//! Mounts `/api/v1/agent-runs` with its router-scoped state, the caller's
//! rate limit and the user authz gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RateBudget, RateLimit, RouteGroup};
use systemprompt_models::modules::ApiPaths;

use super::protocol::{MountCtx, declare};
use crate::routes::agent_runs::{AgentRunsState, router as agent_runs_router};
use crate::services::middleware::MountError;

pub(super) fn mount_agent_runs(router: Router, mount: &MountCtx<'_>) -> Result<Router, MountError> {
    mount.mount(
        router,
        declare(
            agent_runs_router(AgentRunsState::build(mount.ctx)),
            ApiPaths::AGENT_RUNS,
            AuthzPolicy::user(),
            ContextKind::User,
            RouteGroup::Agent,
        )
        .with_rate_limit(RateLimit::new("agent_runs", RateBudget::Tasks)),
    )
}
