//! Job::execute() smoke tests that exercise the real query paths against the
//! test Postgres database. Each test wires the job's required dependencies
//! (DbPool / AppPaths) into a JobContext and asserts that the job reports
//! success.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_scheduler::{
    BehavioralAnalysisJob, CleanupEmptyContextsJob, CleanupInactiveSessionsJob, DatabaseCleanupJob,
    GhostSessionCleanupJob, MaliciousIpBlacklistJob, NoJsCleanupJob,
};
use systemprompt_test_fixtures::{fixture_actor, fixture_database_url, fixture_db_pool};
use systemprompt_traits::{Job, JobContext};

async fn try_pool() -> Option<DbPool> {
    let url = fixture_database_url().ok()?;
    fixture_db_pool(&url).await.ok()
}

fn make_ctx(pool: &DbPool) -> JobContext {
    let pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(Arc::clone(pool));
    let ctx_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let paths_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    JobContext::new(fixture_actor(), pool_any, ctx_any, paths_any)
}

#[tokio::test]
async fn database_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = DatabaseCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn cleanup_inactive_sessions_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = CleanupInactiveSessionsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn cleanup_empty_contexts_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = CleanupEmptyContextsJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn behavioral_analysis_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = BehavioralAnalysisJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn ghost_session_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = GhostSessionCleanupJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn malicious_ip_blacklist_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = MaliciousIpBlacklistJob
        .execute(&ctx)
        .await
        .expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn no_js_cleanup_job_execute_succeeds() {
    let Some(pool) = try_pool().await else {
        return;
    };
    let ctx = make_ctx(&pool);
    let result = NoJsCleanupJob.execute(&ctx).await.expect("job runs");
    assert!(result.success);
}

#[tokio::test]
async fn jobs_fail_when_dbpool_missing() {
    let pool_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let ctx_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let paths_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
    let ctx = JobContext::new(fixture_actor(), pool_any, ctx_any, paths_any);

    assert!(DatabaseCleanupJob.execute(&ctx).await.is_err());
    assert!(CleanupInactiveSessionsJob.execute(&ctx).await.is_err());
    assert!(CleanupEmptyContextsJob.execute(&ctx).await.is_err());
    assert!(BehavioralAnalysisJob.execute(&ctx).await.is_err());
    assert!(GhostSessionCleanupJob.execute(&ctx).await.is_err());
    assert!(MaliciousIpBlacklistJob.execute(&ctx).await.is_err());
    assert!(NoJsCleanupJob.execute(&ctx).await.is_err());
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

    async fn seed(tag: &str) -> Option<Fixture> {
        let pool = try_pool().await?;
        let user_id = unique_user_id(tag);
        seed_user_row(
            &pool,
            &user_id,
            &format!("{}@{tag}.invalid", user_id.as_str()),
        )
        .await
        .expect("seed user");

        let context_id = format!("{tag}-{}", uuid::Uuid::new_v4());
        let raw = pool.pool_arc().expect("raw pool");
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

        Some(Fixture {
            pool,
            user_id,
            context_id,
        })
    }

    impl Fixture {
        async fn context_exists(&self) -> bool {
            let raw = self.pool.pool_arc().expect("raw pool");
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_contexts WHERE context_id = $1)",
            )
            .bind(&self.context_id)
            .fetch_one(raw.as_ref())
            .await
            .expect("context probe")
        }

        async fn cleanup(&self) {
            let raw = self.pool.pool_arc().expect("raw pool");
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
        let Some(fixture) = seed("enfoff").await else {
            return;
        };
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
        let Some(fixture) = seed("enfon").await else {
            return;
        };
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
