//! Tests for the analytics crate's jobs. [`job_ctx`] assembles the context
//! the scheduler hands them: the pool, the analytics repositories and the
//! users crate's ban store as the shared `DynIpBanStore`.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::test_app_context;
use systemprompt_traits::{Dependencies, DynIpBanStore, JobContext};
use systemprompt_users::BannedIpRepository;

mod ip_ban_detectors_db;
mod ip_ban_detectors_seeded_db;
mod job_execute_db;
mod job_properties;

pub(crate) fn job_ctx_over(app_pool: &DbPool, ctx_pool: &DbPool, url: &str) -> JobContext {
    let app_ctx = test_app_context(app_pool, url);
    let bans: DynIpBanStore = Arc::new(BannedIpRepository::new(ctx_pool));
    let dependencies = Dependencies::new()
        .with(Arc::clone(ctx_pool))
        .with(Arc::clone(app_ctx.analytics_repositories()))
        .with(bans);
    JobContext::new(
        Actor::job(UserId::new("analytics-job-test-admin"), "test".to_owned()),
        dependencies,
    )
}

pub(crate) fn job_ctx(pool: &DbPool, url: &str) -> JobContext {
    job_ctx_over(pool, pool, url)
}
