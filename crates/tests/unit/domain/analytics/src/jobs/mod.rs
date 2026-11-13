//! Tests for the analytics crate's jobs. [`job_ctx`] assembles the context
//! the scheduler hands them: the pool, the analytics repositories and the
//! users crate's ban store as the shared `DynIpBanStore`. [`job_ctx_over`]
//! builds every collaborator a job reads (pool, repositories, ban store)
//! over `ctx_pool`, so a closed `ctx_pool` reaches every query the job runs.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::app_context::fixture_analytics_repositories;
use systemprompt_traits::{Dependencies, DynIpBanStore, JobContext};
use systemprompt_users::BannedIpRepository;

mod ip_ban_detectors_db;
mod ip_ban_detectors_seeded_db;
mod job_execute_db;
mod job_properties;

pub(crate) fn job_ctx_over(ctx_pool: &DbPool) -> JobContext {
    let repositories = fixture_analytics_repositories(ctx_pool)
        .unwrap_or_else(|e| panic!("analytics repositories over the job pool: {e:#}"));
    let bans: DynIpBanStore = Arc::new(BannedIpRepository::new(ctx_pool));
    let dependencies = Dependencies::new()
        .with(Arc::clone(ctx_pool))
        .with(Arc::new(repositories))
        .with(bans);
    JobContext::new(
        Actor::job(UserId::new("analytics-job-test-admin"), "test".to_owned()),
        dependencies,
    )
}

pub(crate) fn job_ctx(pool: &DbPool) -> JobContext {
    job_ctx_over(pool)
}
