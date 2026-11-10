//! Every retention target deletes through its owning repository: with one
//! stale and one fresh row seeded per table, an enforced run removes exactly
//! the stale row, and `ai_request_payloads` keeps its row with the bodies
//! released. `engagement_events` and `fingerprint_reputation`, which carry
//! personal data, are bounded the same way, except that an idle fingerprint
//! flagged for abuse is kept with its IP address and user agent cleared.

use std::sync::Arc;

use systemprompt_scheduler::DatabaseCleanupJob;
use systemprompt_test_fixtures::{test_app_context, test_database_url, test_db_pool};
use systemprompt_traits::{Dependencies, Job, JobContext};

const STALE_DAYS: i32 = 4000;

fn ctx(pool: &systemprompt_database::DbPool, url: &str) -> JobContext {
    use systemprompt_identifiers::{Actor, UserId};

    let app_ctx = test_app_context(pool, url);
    let dependencies = Dependencies::new()
        .with(Arc::clone(app_ctx.app_paths_arc()))
        .with(Arc::clone(pool))
        .with(app_ctx);
    JobContext::new(
        Actor::job(UserId::new("retention-targets-admin"), "test".to_owned()),
        dependencies,
    )
    .with_enforce(true)
}

struct Seeded {
    suffix: String,
}

impl Seeded {
    fn id(&self, table: &str, age: &str) -> String {
        format!("ret_{table}_{age}_{}", self.suffix)
    }
}

async fn exec(pool: &sqlx::PgPool, sql: &'static str, id: &str, days_ago: i32) {
    sqlx::query(sql)
        .bind(id)
        .bind(days_ago)
        .execute(pool)
        .await
        .unwrap_or_else(|e| panic!("seed {id}: {e}"));
}

async fn seed_all(pool: &sqlx::PgPool, seeded: &Seeded) {
    for (age, days) in [("old", STALE_DAYS), ("fresh", 0)] {
        exec(
            pool,
            "INSERT INTO logs (id, timestamp, level, module, message, user_id) \
             VALUES ($1, NOW() - make_interval(days => $2), 'INFO', 'retention_targets', 'm', 'system')",
            &seeded.id("logs", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO analytics_events (id, user_id, event_type, event_category, severity, timestamp) \
             VALUES ($1, 'system', 'retention_test', 'test', 'info', NOW() - make_interval(days => $2))",
            &seeded.id("analytics_events", age),
            days,
        )
        .await;
        let request = seeded.id("ai_requests", age);
        exec(
            pool,
            "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, actor_kind, actor_id, status, created_at) \
             VALUES ($1, $1, 'system', 'ctx-retention', 'test', 'test-model', 'job', 'retention_test', 'completed', NOW() - make_interval(days => $2))",
            &request,
            days,
        )
        .await;
        sqlx::query(
            "INSERT INTO ai_request_messages (id, request_id, role, content, sequence_number, created_at) \
             VALUES ($1, $2, 'user', 'hello', 0, NOW() - make_interval(days => $3))",
        )
        .bind(seeded.id("ai_request_messages", age))
        .bind(&request)
        .bind(days)
        .execute(pool)
        .await
        .expect("seed ai_request_messages");
        exec(
            pool,
            "INSERT INTO ai_request_payloads (ai_request_id, request_body, response_body, created_at) \
             VALUES ($1, '{\"k\":1}'::jsonb, '{\"k\":2}'::jsonb, NOW() - make_interval(days => $2))",
            &request,
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO ai_quota_buckets (id, subject_id, window_seconds, window_start) \
             VALUES ($1, $1, 60, NOW() - make_interval(days => $2))",
            &seeded.id("ai_quota_buckets", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO mcp_tool_executions (mcp_execution_id, tool_name, server_name, started_at, input, user_id, status, created_at) \
             VALUES ($1, 'retention_tool', 'retention_server', NOW() - make_interval(days => $2), '{}', 'system', 'success', NOW() - make_interval(days => $2))",
            &seeded.id("mcp_tool_executions", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO event_outbox (id, channel, user_id, payload, actor_kind, actor_id, origin_instance_id, created_at) \
             VALUES ($1, 'retention_test', 'system', '{}'::jsonb, 'job', 'retention_test', 'retention-node', NOW() - make_interval(days => $2))",
            &seeded.id("event_outbox", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO governance_decisions (id, user_id, session_id, tool_name, decision, policy, reason, actor_kind, actor_id, context_id, created_at) \
             VALUES ($1, 'system', 'sess-retention', 'retention_tool', 'allow', 'test', 'test', 'job', 'retention_test', 'ctx-retention', NOW() - make_interval(days => $2))",
            &seeded.id("governance_decisions", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO engagement_events (id, session_id, user_id, page_url, created_at) \
             VALUES ($1, 'sess-retention', 'system', '/retention', NOW() - make_interval(days => $2))",
            &seeded.id("engagement_events", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO fingerprint_reputation (fingerprint_hash, last_seen_at, last_ip_address, last_user_agent) \
             VALUES ($1, NOW() - make_interval(days => $2), '203.0.113.7', 'retention-agent')",
            &seeded.id("fingerprint_reputation", age),
            days,
        )
        .await;
        exec(
            pool,
            "INSERT INTO fingerprint_reputation (fingerprint_hash, last_seen_at, last_ip_address, last_user_agent, is_flagged, flag_reason, flagged_at) \
             VALUES ($1, NOW() - make_interval(days => $2), '203.0.113.9', 'flagged-agent', TRUE, 'abuse', NOW())",
            &seeded.id("fingerprint_flagged", age),
            days,
        )
        .await;
    }
}

async fn exists(pool: &sqlx::PgPool, table: &str, key: &str, id: &str) -> bool {
    let sql = format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE {key} = $1)");
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap_or_else(|e| panic!("probe {table}: {e}"))
}

async fn cleanup(pool: &sqlx::PgPool, seeded: &Seeded) {
    let pattern = format!("ret_%_{}", seeded.suffix);
    for (table, key) in [
        ("logs", "id"),
        ("analytics_events", "id"),
        ("ai_requests", "id"),
        ("ai_quota_buckets", "id"),
        ("mcp_tool_executions", "mcp_execution_id"),
        ("event_outbox", "id"),
        ("governance_decisions", "id"),
        ("engagement_events", "id"),
        ("fingerprint_reputation", "fingerprint_hash"),
    ] {
        let sql = format!("DELETE FROM {table} WHERE {key} LIKE $1");
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(&pattern)
            .execute(pool)
            .await
            .ok();
    }
}

#[tokio::test]
async fn every_target_removes_the_stale_row_and_keeps_the_fresh_one() {
    let url = test_database_url();
    let pool = test_db_pool().await;
    let pg = pool.write_pool();
    let seeded = Seeded {
        suffix: uuid::Uuid::new_v4().simple().to_string(),
    };
    seed_all(&pg, &seeded).await;

    let result = DatabaseCleanupJob
        .execute(&ctx(&pool, &url))
        .await
        .expect("enforced retention run");
    assert!(result.success, "{:?}", result.message);

    for (table, key) in [
        ("logs", "id"),
        ("analytics_events", "id"),
        ("ai_request_messages", "id"),
        ("ai_quota_buckets", "id"),
        ("mcp_tool_executions", "mcp_execution_id"),
        ("event_outbox", "id"),
        ("governance_decisions", "id"),
        ("engagement_events", "id"),
        ("fingerprint_reputation", "fingerprint_hash"),
    ] {
        assert!(
            !exists(&pg, table, key, &seeded.id(table, "old")).await,
            "{table}: the stale row must be deleted"
        );
        assert!(
            exists(&pg, table, key, &seeded.id(table, "fresh")).await,
            "{table}: the fresh row must survive"
        );
    }

    let bodies: (bool, bool) = sqlx::query_as(
        "SELECT request_body IS NULL, response_body IS NULL FROM ai_request_payloads WHERE ai_request_id = $1",
    )
    .bind(seeded.id("ai_requests", "old"))
    .fetch_one(&*pg)
    .await
    .expect("stale payload row still present");
    assert_eq!(bodies, (true, true), "stale payload bodies are released");
    let fresh_bodies: (bool, bool) = sqlx::query_as(
        "SELECT request_body IS NULL, response_body IS NULL FROM ai_request_payloads WHERE ai_request_id = $1",
    )
    .bind(seeded.id("ai_requests", "fresh"))
    .fetch_one(&*pg)
    .await
    .expect("fresh payload row");
    assert_eq!(fresh_bodies, (false, false), "fresh payload bodies stay");

    let pii = |age: &'static str| {
        let id = seeded.id("fingerprint_flagged", age);
        let pg = std::sync::Arc::clone(&pg);
        async move {
            sqlx::query_as::<_, (Option<String>, Option<String>, bool)>(
                "SELECT last_ip_address, last_user_agent, is_flagged FROM fingerprint_reputation WHERE fingerprint_hash = $1",
            )
            .bind(id)
            .fetch_optional(&*pg)
            .await
            .expect("probe flagged fingerprint")
        }
    };
    assert_eq!(
        pii("old").await,
        Some((None, None, true)),
        "an idle flagged fingerprint is kept with its IP address and user agent cleared"
    );
    assert_eq!(
        pii("fresh").await,
        Some((
            Some("203.0.113.9".to_owned()),
            Some("flagged-agent".to_owned()),
            true
        )),
        "a recently seen flagged fingerprint keeps its personal data"
    );

    cleanup(&pg, &seeded).await;
}
