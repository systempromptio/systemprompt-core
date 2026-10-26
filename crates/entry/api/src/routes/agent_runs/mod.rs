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
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handlers;
mod stream;

use std::sync::Arc;

use axum::Router;
use axum::routing::{get, post};
use systemprompt_agent_runs::notify::RunEventNotifier;
use systemprompt_agent_runs::repository::AgentRunRepositories;
use systemprompt_agent_runs::{A2aRunnerConfig, AgentRunRepository};
use systemprompt_runtime::AppContext;

/// Router-scoped state: the run repository, the run layout root and the one
/// event notifier, built once when the routes are mounted.
#[derive(Clone)]
pub struct AgentRunsState {
    pub(crate) runs: AgentRunRepository,
    pub(crate) layouts: Arc<A2aRunnerConfig>,
    pub(crate) notifier: RunEventNotifier,
}

impl std::fmt::Debug for AgentRunsState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentRunsState")
            .field("runs_root", &self.layouts.runs_root)
            .finish_non_exhaustive()
    }
}

impl AgentRunsState {
    #[must_use]
    pub const fn runs(&self) -> &AgentRunRepository {
        &self.runs
    }

    #[must_use]
    pub fn build(ctx: &AppContext) -> Self {
        let config = ctx.config();
        Self {
            runs: AgentRunRepositories::new(ctx.db_pool(), ctx.oauth_repositories().oauth.clone())
                .runs,
            notifier: RunEventNotifier::spawn(
                ctx.background_tasks(),
                ctx.db_pool().write_pool().as_ref().clone(),
            ),
            layouts: Arc::new(A2aRunnerConfig::new(
                &config.api_internal_url,
                &config.jwt_issuer,
                ctx.app_paths().storage().data(),
            )),
        }
    }
}

pub fn router(state: AgentRunsState) -> Router {
    Router::new()
        .route("/", post(handlers::enqueue))
        .route("/{id}", get(handlers::get_run))
        .route("/{id}/cancel", post(handlers::cancel))
        .route("/{id}/events", get(handlers::list_events))
        .route("/{id}/events/stream", get(stream::events_stream))
        .route("/{id}/outputs", get(handlers::list_outputs))
        .route("/{id}/outputs/{*path}", get(handlers::read_output))
        .with_state(state)
}
