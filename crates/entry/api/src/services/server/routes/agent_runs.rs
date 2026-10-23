//! Mounts `/api/v1/agent-runs` with its router-scoped state, the caller's
//! rate limit and the user authz gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::LoaderError;
use systemprompt_models::modules::ApiPaths;

use super::protocol::MountCtx;
use crate::routes::agent_runs::{AgentRunsState, router as agent_runs_router};
use crate::services::middleware::RouterExt;
use crate::services::middleware::authz::AuthzPolicy;

pub(super) fn mount_agent_runs(
    router: Router,
    mount: &MountCtx<'_>,
) -> Result<Router, LoaderError> {
    let rate_config = &mount.ctx.config().rate_limits;
    Ok(router.nest(
        ApiPaths::AGENT_RUNS,
        agent_runs_router(AgentRunsState::build(mount.ctx))
            .with_rate_limit(mount.limits, rate_config.tasks_per_second, "agent_runs")?
            .with_auth(mount.user_middleware.clone(), AuthzPolicy::user()),
    ))
}
