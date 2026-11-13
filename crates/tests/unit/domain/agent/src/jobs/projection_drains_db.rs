//! The agent crate's projection drainer against an idle, migrated database.

use std::sync::Arc;

use systemprompt_agent::jobs::UserContextCountersRollupJob;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{Dependencies, Job, JobContext};

#[tokio::test]
async fn user_context_counters_rollup_drains() {
    let pool = test_db_pool().await;
    let context = JobContext::new(
        Actor::job(UserId::new("projection-job-admin"), "test".to_owned()),
        Dependencies::new().with(Arc::clone(&pool)),
    );
    let result = UserContextCountersRollupJob
        .execute(&context)
        .await
        .expect("drains an idle backlog");
    assert!(result.success);
}
