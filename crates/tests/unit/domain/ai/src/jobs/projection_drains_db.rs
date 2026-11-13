//! The AI crate's projection drainers against an idle, migrated database.

use std::sync::Arc;

use systemprompt_ai::jobs::AiRequestToolFlagRollupJob;
use systemprompt_identifiers::{Actor, UserId};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{Dependencies, Job, JobContext};

#[tokio::test]
async fn ai_request_tool_flag_rollup_drains() {
    let pool = test_db_pool().await;
    let context = JobContext::new(
        Actor::job(UserId::new("projection-job-admin"), "test".to_owned()),
        Dependencies::new().with(Arc::clone(&pool)),
    );
    let result = AiRequestToolFlagRollupJob
        .execute(&context)
        .await
        .expect("drains an idle backlog");
    assert!(result.success);
}
