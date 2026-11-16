//! `/api/v1/agent-runs`: queue, read, cancel and observe headless runs.
//!
//! A caller queues a [`RunRequest`](systemprompt_agent_runs::RunRequest)
//! under a registered origin with an idempotency key; the origin's claim
//! loop executes it. Every route is scoped to the caller: a run another user
//! queued is 404. The server owns
//! the run's filesystem: the request's `workspace` is replaced with the run
//! layout's, and a request naming server paths (`add_dirs`, `plugin_dirs`)
//! or a stdio MCP server is refused.
//!
//! [`AgentRunsApiExtension`](crate::extension::AgentRunsApiExtension)
//! contributes the route set; the API server mounts it with the user gate
//! and the `agent_runs` rate limit without naming this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod error;
mod handlers;
mod stream;

use std::sync::Arc;

use axum::http::Method;
use systemprompt_agent_runs::notify::RunEventNotifier;
use systemprompt_agent_runs::origin::AdmittedRunOrigins;
use systemprompt_agent_runs::{A2aRunnerConfig, AgentRunRepository};
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_extension::RouteGroup;
use systemprompt_extension::prelude::*;
use systemprompt_extension::router::{
    ApiBase, BasePath, BodyLimit, RouteLimits, RouteSet, RouteSpec,
};
use systemprompt_manifest::Config;


/// Route-set state: the run repository, the admitted run origins, the run
/// layout root and the one event notifier, resolved once when the routes are
/// mounted.
#[derive(Clone)]
pub struct AgentRunsState {
    pub(crate) runs: AgentRunRepository,
    pub(crate) origins: AdmittedRunOrigins,
    pub(crate) layouts: Arc<A2aRunnerConfig>,
    pub(crate) notifier: RunEventNotifier,
}

impl std::fmt::Debug for AgentRunsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRunsState")
            .field("runs_root", &self.layouts.runs_root)
            .field("origins", &self.origins.names())
            .finish_non_exhaustive()
    }
}

impl AgentRunsState {
    #[must_use]
    pub const fn runs(&self) -> &AgentRunRepository {
        &self.runs
    }
}

impl FromState for AgentRunsState {
    fn from_state(r: &Resolved<'_>) -> Result<Self, MissingDependency> {
        let db = r.get::<DbPool>()?;
        let paths = r.get::<Arc<AppPaths>>()?;
        let config = r.get::<Arc<Config>>()?;
        Ok(Self {
            runs: r.get::<AgentRunRepository>()?,
            origins: r.get::<AdmittedRunOrigins>()?,
            notifier: RunEventNotifier::spawn(r.tasks(), db.write_pool().as_ref().clone()),
            layouts: Arc::new(A2aRunnerConfig::new(
                &config.api_internal_url,
                &config.jwt_issuer,
                paths.storage().data(),
            )),
        })
    }
}

const LIMITS: RouteLimits = RouteLimits {
    rate: RateLimit::new("agent_runs", RateBudget::Tasks),
    body: BodyLimit::Default,
};
const USER: RouteSpec = RouteSpec::new(AuthzPolicy::user(), ContextKind::User, LIMITS);

#[must_use]
pub fn routes() -> RouteSet {
    RouteSet::at::<AgentRunsState>(BasePath::Api(ApiBase::AgentRuns), RouteGroup::Agent)
        .route(Method::POST, "/", handlers::enqueue, USER)
        .route(Method::GET, "/{id}", handlers::get_run, USER)
        .route(Method::POST, "/{id}/cancel", handlers::cancel, USER)
        .route(Method::GET, "/{id}/events", handlers::list_events, USER)
        .route(
            Method::GET,
            "/{id}/events/stream",
            stream::events_stream,
            USER,
        )
        .route(Method::GET, "/{id}/outputs", handlers::list_outputs, USER)
        .route(
            Method::GET,
            "/{id}/outputs/{*path}",
            handlers::read_output,
            USER,
        )
        .build()
}

pub(crate) fn contribute(c: &mut Contributions<'_>) {
    c.requires::<AgentRunRepository>().routes(routes());
}
