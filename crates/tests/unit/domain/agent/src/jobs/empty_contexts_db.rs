//! DB-backed tests for [`EmptyContextRepository`], the candidate query and
//! delete behind the `cleanup_empty_contexts` job.

use std::sync::Arc;

use systemprompt_agent::repository::EmptyContextRepository;
use systemprompt_database::DbPool;
use systemprompt_mcp::repository::ToolUsageRepository;
use systemprompt_test_fixtures::test_db_pool;

fn empty_context_repository(pool: &DbPool) -> EmptyContextRepository {
    EmptyContextRepository::new(pool, Arc::new(ToolUsageRepository::new(pool)))
}

fn unique_job_name(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{prefix}_{}_{}_{}", std::process::id(), seq, nanos)
}

mod empty_context_repository {
    use super::*;

    #[tokio::test]
    async fn cleanup_empty_contexts_returns_rows_affected_count() {
        let pool = test_db_pool().await;
        let repo = empty_context_repository(&pool);

        // No seeded contexts; the DELETE simply affects whatever stale empty
        // contexts exist (0 on a fresh DB). The call must succeed and return a
        // count.
        let affected = repo
            .cleanup_empty_contexts(1)
            .await
            .expect("cleanup should execute");
        let _ = affected;
    }

    #[tokio::test]
    async fn new_succeeds() {
        let pool = test_db_pool().await;
        let _repo = empty_context_repository(&pool);
    }

    #[tokio::test]
    async fn cleanup_empty_contexts_executes_for_various_windows() {
        let pool = test_db_pool().await;
        let repo = empty_context_repository(&pool);

        for hours in [0_i64, 1, 24, 168] {
            repo.cleanup_empty_contexts(hours)
                .await
                .expect("cleanup query should execute for any window");
        }
    }

    #[tokio::test]
    async fn cleanup_collects_orphaned_cli_contexts_but_spares_session_bound_ones() {
        let pool = test_db_pool().await;
        let repo = empty_context_repository(&pool);
        let raw = pool.pool();

        let user_id = systemprompt_test_fixtures::unique_user_id("schedgc");
        let session_id = systemprompt_identifiers::SessionId::generate();
        systemprompt_test_fixtures::seed_user_row(
            &pool,
            &user_id,
            &format!("{}@schedgc.invalid", user_id.as_str()),
        )
        .await
        .expect("seed user");
        systemprompt_test_fixtures::seed_user_session(&pool, &user_id, &session_id)
            .await
            .expect("seed session");

        let live_ctx = systemprompt_identifiers::ContextId::generate().to_string();
        let orphan_ctx = systemprompt_identifiers::ContextId::generate().to_string();
        let old = chrono::Utc::now() - chrono::Duration::hours(48);
        for (ctx, sess) in [(&live_ctx, Some(session_id.as_str())), (&orphan_ctx, None)] {
            sqlx::query(
                "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, \
                 created_at, updated_at) VALUES ($1, $2, $3, $4, 'cli_session', $5, $5)",
            )
            .bind(ctx)
            .bind(user_id.as_str())
            .bind(sess)
            .bind("CLI Session - schedgc")
            .bind(old)
            .execute(raw.as_ref())
            .await
            .expect("seed context");
        }

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        let survivors: Vec<String> =
            sqlx::query_scalar("SELECT context_id FROM user_contexts WHERE user_id = $1")
                .bind(user_id.as_str())
                .fetch_all(raw.as_ref())
                .await
                .expect("list survivors");
        assert!(
            survivors.contains(&live_ctx),
            "empty CLI context bound to a live session must survive"
        );
        assert!(
            !survivors.contains(&orphan_ctx),
            "session-orphaned CLI context must be collected"
        );

        let _ = sqlx::query("DELETE FROM user_contexts WHERE user_id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
        let _ = sqlx::query("DELETE FROM user_sessions WHERE user_id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id.as_str())
            .execute(raw.as_ref())
            .await;
    }
}

// Contexts holding audit rows (MCP tool executions, governance decisions) are
// not "empty" regardless of age — the audit tables reference `context_id`
// without an FK, so nothing else protects them.
mod empty_context_audit_guards {
    use super::*;

    struct Seed {
        pool: systemprompt_database::DbPool,
        user_id: systemprompt_identifiers::UserId,
        session_id: systemprompt_identifiers::SessionId,
    }

    impl Seed {
        async fn new(tag: &str) -> Self {
            let pool = test_db_pool().await;
            let user_id = systemprompt_test_fixtures::unique_user_id(tag);
            let session_id = systemprompt_identifiers::SessionId::generate();
            systemprompt_test_fixtures::seed_user_row(
                &pool,
                &user_id,
                &format!("{}@{tag}.invalid", user_id.as_str()),
            )
            .await
            .expect("seed user");
            systemprompt_test_fixtures::seed_user_session(&pool, &user_id, &session_id)
                .await
                .expect("seed session");
            Self {
                pool,
                user_id,
                session_id,
            }
        }

        fn raw(&self) -> std::sync::Arc<sqlx::PgPool> {
            self.pool.pool()
        }

        async fn seed_old_context(&self, context_id: &str) {
            let old = chrono::Utc::now() - chrono::Duration::hours(72);
            sqlx::query(
                "INSERT INTO user_contexts (context_id, user_id, session_id, name, kind, \
                 created_at, updated_at) VALUES ($1, $2, NULL, $3, 'conversation', $4, $4)",
            )
            .bind(context_id)
            .bind(self.user_id.as_str())
            .bind("audit-guard fixture")
            .bind(old)
            .execute(self.raw().as_ref())
            .await
            .expect("seed context");
        }

        async fn seed_tool_execution(&self, execution_id: &str, context_id: &str) {
            sqlx::query(
                "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, \
                 started_at, input, status, user_id, session_id, context_id) VALUES ($1, \
                 'audit_guard_tool', 'audit_guard_server', NOW() - INTERVAL '72 hours', '{}', \
                 'success', $2, $3, $4)",
            )
            .bind(execution_id)
            .bind(self.user_id.as_str())
            .bind(self.session_id.as_str())
            .bind(context_id)
            .execute(self.raw().as_ref())
            .await
            .expect("seed mcp_tool_executions");
        }

        async fn seed_governance_decision(&self, decision_id: &str, context_id: &str) {
            sqlx::query(
                "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, \
                 policy, reason, actor_kind, actor_id, act_chain, context_id, created_at) VALUES \
                 ($1, $2, $3, 'audit_guard_tool', 'allow', 'audit-guard', 'fixture', 'user', $2, \
                 '[]'::jsonb, $4, NOW() - INTERVAL '72 hours')",
            )
            .bind(decision_id)
            .bind(self.user_id.as_str())
            .bind(self.session_id.as_str())
            .bind(context_id)
            .execute(self.raw().as_ref())
            .await
            .expect("seed governance_decisions");
        }

        async fn context_exists(&self, context_id: &str) -> bool {
            sqlx::query_scalar::<_, bool>(
                "SELECT EXISTS(SELECT 1 FROM user_contexts WHERE context_id = $1)",
            )
            .bind(context_id)
            .fetch_one(self.raw().as_ref())
            .await
            .expect("context probe")
        }

        async fn cleanup(&self) {
            let raw = self.raw();
            for stmt in [
                "DELETE FROM governance_decisions WHERE user_id = $1",
                "DELETE FROM mcp_tool_executions WHERE user_id = $1",
                "DELETE FROM user_contexts WHERE user_id = $1",
                "DELETE FROM user_sessions WHERE user_id = $1",
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
    async fn context_with_tool_execution_survives_cleanup() {
        let seed = Seed::new("auditmte").await;
        let repo = empty_context_repository(&seed.pool);
        // The audit row is written first: a concurrent sweep would otherwise
        // collect the context through the gap before it is protected.
        let ctx_id = systemprompt_identifiers::ContextId::generate().to_string();
        seed.seed_tool_execution(&unique_job_name("auditexec"), &ctx_id)
            .await;
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            seed.context_exists(&ctx_id).await,
            "a context referenced by mcp_tool_executions must not be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn context_with_governance_decision_survives_cleanup() {
        let seed = Seed::new("auditgd").await;
        let repo = empty_context_repository(&seed.pool);
        let ctx_id = systemprompt_identifiers::ContextId::generate().to_string();
        seed.seed_governance_decision(&unique_job_name("auditdec"), &ctx_id)
            .await;
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            seed.context_exists(&ctx_id).await,
            "a context referenced by governance_decisions must not be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn truly_empty_old_context_is_deleted() {
        let seed = Seed::new("auditbare").await;
        let repo = empty_context_repository(&seed.pool);
        let ctx_id = systemprompt_identifiers::ContextId::generate().to_string();
        seed.seed_old_context(&ctx_id).await;

        repo.cleanup_empty_contexts(1).await.expect("cleanup runs");

        assert!(
            !seed.context_exists(&ctx_id).await,
            "an old context with no messages and no audit rows must be collected"
        );
        seed.cleanup().await;
    }

    #[tokio::test]
    async fn count_empty_contexts_counts_what_cleanup_deletes() {
        let seed = Seed::new("auditcount").await;
        let repo = empty_context_repository(&seed.pool);
        let collectable = systemprompt_identifiers::ContextId::generate().to_string();
        let protected = systemprompt_identifiers::ContextId::generate().to_string();
        seed.seed_tool_execution(&unique_job_name("auditexec"), &protected)
            .await;
        seed.seed_old_context(&protected).await;
        seed.seed_old_context(&collectable).await;

        // The sweep is table-wide and shards run in parallel, so only the
        // seeded rows can be asserted on: a concurrent cleanup may already
        // have collected this one between the count and the delete.
        let counted = repo.count_empty_contexts(1).await.expect("count");
        if seed.context_exists(&collectable).await {
            assert!(
                counted >= 1,
                "a live collectable context must be counted, got {counted}"
            );
        }

        repo.cleanup_empty_contexts(1).await.expect("cleanup");
        assert!(!seed.context_exists(&collectable).await);
        assert!(seed.context_exists(&protected).await);

        seed.cleanup().await;
    }

    // Regression: an MCP tool execution whose context has already been deleted
    // must survive every retention sweep — audit rows outlive their context.
    #[tokio::test]
    async fn orphaned_tool_execution_survives_all_retention_sweeps() {
        let seed = Seed::new("auditorphan").await;
        let repo = empty_context_repository(&seed.pool);
        let exec_id = unique_job_name("auditexec_orphan");
        let missing_ctx = systemprompt_identifiers::ContextId::generate().to_string();
        seed.seed_tool_execution(&exec_id, &missing_ctx).await;

        repo.cleanup_empty_contexts(1).await.expect("context sweep");

        let logs = systemprompt_logging::LoggingRepository::new(&seed.pool);
        let seen = logs.distinct_log_user_ids().await.expect("log owners");
        let orphans = systemprompt_users::UserRepository::new(&seed.pool)
            .missing_ids(&seen)
            .await
            .expect("missing owners");
        logs.delete_logs_for_users(&orphans)
            .await
            .expect("orphaned logs");
        logs.purge_before(chrono::Utc::now() - chrono::Duration::days(30), 5000)
            .await
            .expect("old logs");
        systemprompt_oauth::repository::OauthCleanupRepository::new(&seed.pool)
            .delete_expired()
            .await
            .expect("oauth expiry sweep");

        let survived = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM mcp_tool_executions WHERE mcp_execution_id = $1)",
        )
        .bind(&exec_id)
        .fetch_one(seed.raw().as_ref())
        .await
        .expect("execution probe");
        assert!(
            survived,
            "an MCP tool execution must outlive the context it referenced"
        );

        seed.cleanup().await;
    }
}
