//! `CleanupEmptyContextsJob::execute` against the test database: the smoke
//! path and the `enforce` gate.

use std::sync::Arc;

use systemprompt_agent::jobs::CleanupEmptyContextsJob;
use systemprompt_database::DbPool;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_test_fixtures::{fixture_actor, test_db_pool};
use systemprompt_traits::{Dependencies, DynToolExecutionLookup, Job, JobContext};

#[test]
fn name_description_and_schedule_are_unchanged() {
    let job = CleanupEmptyContextsJob;
    assert_eq!(job.name(), "cleanup_empty_contexts");
    assert_eq!(
        job.description(),
        "Deletes empty, audit-free conversation contexts (parameter retention_hours, default 24); requires enforce"
    );
    assert_eq!(job.schedule(), "0 0 */2 * * *");
}

fn make_ctx(pool: &DbPool) -> JobContext {
    let executions: DynToolExecutionLookup = Arc::new(ToolUsageRepository::new(pool));
    JobContext::new(
        fixture_actor(),
        Dependencies::new().with(Arc::clone(pool)).with(executions),
    )
}

#[tokio::test]
async fn cleanup_empty_contexts_job_execute_succeeds() {
    let pool = test_db_pool().await;
    let ctx = make_ctx(&pool);
    let result = CleanupEmptyContextsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn cleanup_empty_contexts_fails_when_dbpool_missing() {
    let ctx = JobContext::new(fixture_actor(), Dependencies::new());
    assert!(CleanupEmptyContextsJob.execute(&ctx).await.is_err());
}

// `enforce` is the operator's opt-in to destructive retention deletes. With it
// off the job must observe and report only.
mod cleanup_empty_contexts_enforce_gate {
    use super::*;
    use systemprompt_database::DbPool;
    use systemprompt_test_fixtures::{seed_user_row, unique_user_id};

    struct Fixture {
        pool: DbPool,
        user_id: systemprompt_identifiers::UserId,
        context_id: String,
    }

    async fn seed(tag: &str) -> Fixture {
        let pool = test_db_pool().await;
        let user_id = unique_user_id(tag);
        seed_user_row(
            &pool,
            &user_id,
            &format!("{}@{tag}.invalid", user_id.as_str()),
        )
        .await
        .expect("seed user");

        let context_id = uuid::Uuid::new_v4().to_string();
        let raw = pool.pool();
        sqlx::query(
            "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, created_at, \
             updated_at) VALUES ($1, $2, NULL, 'enforce gate fixture', 'conversation', NOW() - \
             INTERVAL '72 hours', NOW() - INTERVAL '72 hours')",
        )
        .bind(&context_id)
        .bind(user_id.as_str())
        .execute(raw.as_ref())
        .await
        .expect("seed context");

        Fixture {
            pool,
            user_id,
            context_id,
        }
    }

    impl Fixture {
        async fn context_exists(&self) -> bool {
            let raw = self.pool.pool();
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_contexts WHERE context_id = $1)",
            )
            .bind(&self.context_id)
            .fetch_one(raw.as_ref())
            .await
            .expect("context probe")
        }

        async fn cleanup(&self) {
            let raw = self.pool.pool();
            for stmt in [
                "DELETE FROM user_contexts WHERE user_id = $1",
                "DELETE FROM users WHERE id = $1",
            ] {
                let _ = sqlx::query(stmt)
                    .bind(self.user_id.as_str())
                    .execute(raw.as_ref())
                    .await;
            }
        }
    }

    #[tokio::test]
    async fn without_enforce_the_context_survives_and_nothing_is_reported_processed() {
        let fixture = seed("enfoff").await;
        let ctx = make_ctx(&fixture.pool).with_parameters(
            [("retention_hours".to_owned(), "1".to_owned())]
                .into_iter()
                .collect(),
        );

        let result = CleanupEmptyContextsJob
            .execute(&ctx)
            .await
            .expect("job runs");
        assert!(result.success);
        assert_eq!(
            result.items_processed,
            Some(0),
            "observe-only mode must report zero deletions"
        );
        assert!(
            fixture.context_exists().await,
            "enforce=false must not delete anything"
        );

        fixture.cleanup().await;
    }

    #[tokio::test]
    async fn with_enforce_the_old_empty_context_is_deleted() {
        let fixture = seed("enfon").await;
        let ctx = make_ctx(&fixture.pool).with_enforce(true).with_parameters(
            [("retention_hours".to_owned(), "1".to_owned())]
                .into_iter()
                .collect(),
        );

        let result = CleanupEmptyContextsJob
            .execute(&ctx)
            .await
            .expect("job runs");
        assert!(result.success);
        assert!(
            !fixture.context_exists().await,
            "enforce=true must collect the old empty context"
        );

        fixture.cleanup().await;
    }
}
