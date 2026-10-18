//! [`A2aRunnerService`]: the runner service that claims A2A-origin runs.
//!
//! The A2A server queues a run per message to an agent whose manifest carries
//! `metadata.agent_run`, with `origin_kind` [`AGENT_RUN_ORIGIN_KIND`]. This
//! service runs a [`ClaimLoop`] over that origin on every `all` and `runner`
//! node and hands each claimed run to an [`A2aRunHandler`], which executes it
//! and finishes the row; `finish` moves the run's task.
//!
//! [`A2aRunnerConfig`] holds what the handler needs from the host: the
//! gateway base URL and hook endpoints (both from `Config::api_internal_url`),
//! the JWT issuer for hook tokens, and the root that run directories live
//! under (`<storage data>/agent-runs/<run id>`). [`credential_request`] is the
//! claimed row and request mapped to the run's credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod handler;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_config::paths::AppPaths;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AgentRunId, UserId};
use systemprompt_manifest::Config;
use systemprompt_provider_contracts::JobContext;
use systemprompt_traits::RunnerService;
use systemprompt_users::ApiKeyLifetime;
use tokio_util::sync::CancellationToken;

pub use handler::A2aRunHandler;
pub use systemprompt_agent::services::a2a_server::handlers::request::agent_run::AGENT_RUN_ORIGIN_KIND;

use crate::credentials::{IssueRunCredentials, RunCredentialLimits, RunCredentialService};
use crate::governance::HookBases;
use crate::layout::RunLayout;
use crate::repository::AgentRunRepositories;
use crate::request::RunRequest;
use crate::service::{ClaimLoop, claimer_id};

pub const A2A_RUNNER_MAX_CONCURRENT: usize = 2;
pub const A2A_RUNS_DIR: &str = "agent-runs";
pub const CREDENTIAL_GRACE: Duration = Duration::from_secs(300);
const SERVICE_NAME: &str = "agent_runs_a2a";
const GOVERN_PATH: &str = "/api/public/hooks/govern";
const TRACK_PATH: &str = "/api/public/hooks/track";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A2aRunnerConfig {
    pub gateway_base_url: String,
    pub hooks: HookBases,
    pub issuer: String,
    pub runs_root: PathBuf,
}

impl A2aRunnerConfig {
    #[must_use]
    pub fn new(api_internal_url: &str, issuer: &str, data_root: &Path) -> Self {
        let base = api_internal_url.trim_end_matches('/');
        Self {
            gateway_base_url: base.to_owned(),
            hooks: HookBases {
                govern_url: format!("{base}{GOVERN_PATH}"),
                track_url: format!("{base}{TRACK_PATH}"),
            },
            issuer: issuer.to_owned(),
            runs_root: data_root.join(A2A_RUNS_DIR),
        }
    }

    #[must_use]
    pub fn layout(&self, run_id: &AgentRunId) -> RunLayout {
        RunLayout::new(self.runs_root.join(run_id.as_str()))
    }
}

#[must_use]
pub fn credential_request(
    user_id: &UserId,
    run_id: &AgentRunId,
    request: &RunRequest,
) -> IssueRunCredentials {
    IssueRunCredentials {
        user_id: user_id.clone(),
        name: format!("a2a-run-{run_id}"),
        ttl: request.wall_clock + CREDENTIAL_GRACE,
        scopes: Vec::new(),
        plugin_id: request.plugin_id.clone(),
        limits: RunCredentialLimits {
            max_budget_usd: request.max_budget_usd,
            max_requests: None,
        },
        purpose: None,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct A2aRunnerService;

#[async_trait]
impl RunnerService for A2aRunnerService {
    fn name(&self) -> &'static str {
        SERVICE_NAME
    }

    async fn run(&self, ctx: JobContext, shutdown: CancellationToken) {
        let Ok(db) = ctx.get::<DbPool>().cloned() else {
            tracing::error!("A2A agent-run runner has no DbPool; no A2A run will be claimed");
            return;
        };
        let Ok(paths) = ctx.get::<Arc<AppPaths>>() else {
            tracing::error!("A2A agent-run runner has no AppPaths; no A2A run will be claimed");
            return;
        };
        let config = match Config::get() {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(%error, "config unavailable; no A2A run will be claimed");
                return;
            },
        };
        let runner = A2aRunnerConfig::new(
            &config.api_internal_url,
            &config.jwt_issuer,
            paths.storage().data(),
        );
        let repos = AgentRunRepositories::new(&db);
        let handler = A2aRunHandler::new(
            RunCredentialService::new(
                repos.credentials,
                &runner.issuer,
                ApiKeyLifetime::from_max_days(config.api_key_max_lifetime_days),
            ),
            runner,
        );
        let mut claim = ClaimLoop::new(
            repos.runs,
            AGENT_RUN_ORIGIN_KIND,
            claimer_id(config.instance_id.as_str()),
        );
        claim.max_concurrent = A2A_RUNNER_MAX_CONCURRENT;
        claim.run(shutdown, Arc::new(handler)).await;
    }
}
