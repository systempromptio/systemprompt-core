use std::sync::Arc;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use systemprompt_analytics::projection::SOURCE_DEFINITIONS;
use systemprompt_database::{Database, DbPool};
use systemprompt_identifiers::ConnectionId;
use systemprompt_runtime::reporting;

async fn fixture() -> (PgPool, DbPool, String) {
    let url = crate::boot::database_url().expect("PostgreSQL test URL");
    let admin = PgPool::connect(&url).await.unwrap();
    let schema = format!(
        "reporting_{}",
        ConnectionId::generate().to_string().replace('-', "_")
    );
    sqlx::query(sqlx::AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
        .execute(&admin)
        .await
        .unwrap();
    let search_path = schema.clone();
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |connection, _| {
            let search_path = search_path.clone();
            Box::pin(async move {
                sqlx::query("SELECT set_config('search_path', $1, false)")
                    .bind(search_path)
                    .execute(connection)
                    .await?;
                Ok(())
            })
        })
        .connect(&url)
        .await
        .unwrap();
    for source in SOURCE_DEFINITIONS {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE TABLE {} (LIKE public.{} INCLUDING ALL)",
            source.table, source.table,
        )))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::raw_sql(include_str!(
        "../../../../../infra/events/schema/event_outbox.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../../../../domain/analytics/schema/reporting.sql"
    ))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(systemprompt_analytics::projection::REPORTING_STATE_SEED)
        .execute(&pool)
        .await
        .unwrap();
    let pool = Arc::new(pool);
    let db = Arc::new(Database::from_pools(pool, None));
    (admin, db, schema)
}

#[tokio::test]
async fn capture_rebuild_worker_preserve_source_reports_and_pending_failures() {
    let (admin, db, schema) = fixture().await;
    let pool = db.write_pool_arc().unwrap();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ('u', 'baseline', 'u@example.test')")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_sessions (session_id, user_id, landing_page, request_count, is_ai_crawler) VALUES ('human', 'u', '/', 1, false), ('crawler', 'u', '/', 1, true)")
        .execute(&*pool).await.unwrap();
    reporting::initialize(&db).await.unwrap();
    assert_eq!(reporting::status(&db).await.unwrap().generation, 1);
    reporting::initialize(&db).await.unwrap();
    assert_eq!(reporting::status(&db).await.unwrap().generation, 1);
    let baseline: String =
        sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id = 'u'")
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert_eq!(baseline, "baseline");
    let human_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_report_v_clean_traffic")
            .fetch_one(&*pool)
            .await
            .unwrap();
    let crawler_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM analytics_report_v_bot_sessions")
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert_eq!((human_count, crawler_count), (1, 1));

    let mut rolled_back = pool.begin().await.unwrap();
    sqlx::query("UPDATE users SET name = 'rolled back' WHERE id = 'u'")
        .execute(&mut *rolled_back)
        .await
        .unwrap();
    rolled_back.rollback().await.unwrap();
    assert_eq!(reporting::status(&db).await.unwrap().pending_count, 0);

    sqlx::query("UPDATE users SET name = 'updated' WHERE id = 'u'")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, actor_kind, actor_id, cost_microdollars, input_tokens, output_tokens) VALUES ('r', 'r', 'u', 'ctx', 'test', 'model', 'job', 'test', 1234567, 5, 9)")
        .execute(&*pool).await.unwrap();
    assert_eq!(reporting::status(&db).await.unwrap().pending_count, 2);
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 2);
    let parity: bool = sqlx::query_scalar("SELECT (SELECT sum(cost_microdollars) FROM ai_requests) = (SELECT sum(cost_microdollars) FROM analytics_report_ai_requests)")
        .fetch_one(&*pool).await.unwrap();
    assert!(parity);
    assert!(
        reporting::status(&db)
            .await
            .unwrap()
            .last_processed_at
            .is_some()
    );

    sqlx::query("UPDATE ai_requests SET cost_microdollars = 2000000 WHERE id = 'r'")
        .execute(&*pool)
        .await
        .unwrap();
    reporting::rebuild(&db).await.unwrap();
    assert_eq!(reporting::status(&db).await.unwrap().generation, 2);
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 1);
    let cost: i64 = sqlx::query_scalar(
        "SELECT cost_microdollars FROM analytics_report_ai_requests WHERE id = 'r'",
    )
    .fetch_one(&*pool)
    .await
    .unwrap();
    assert_eq!(cost, 2000000);

    let mut merge = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO users (id, name, email) VALUES ('target', 'merged', 'target@example.test')",
    )
    .execute(&mut *merge)
    .await
    .unwrap();
    sqlx::query("UPDATE ai_requests SET user_id = 'target' WHERE user_id = 'u'")
        .execute(&mut *merge)
        .await
        .unwrap();
    sqlx::query("UPDATE user_sessions SET user_id = 'target' WHERE user_id = 'u'")
        .execute(&mut *merge)
        .await
        .unwrap();
    sqlx::query("DELETE FROM users WHERE id = 'u'")
        .execute(&mut *merge)
        .await
        .unwrap();
    merge.commit().await.unwrap();
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 5);
    let owners: Vec<String> = sqlx::query_scalar("SELECT user_id FROM analytics_report_ai_requests UNION SELECT user_id FROM analytics_report_user_sessions")
        .fetch_all(&*pool).await.unwrap();
    assert_eq!(owners, vec!["target"]);
    let users: Vec<String> = sqlx::query_scalar("SELECT id FROM analytics_report_users")
        .fetch_all(&*pool)
        .await
        .unwrap();
    assert_eq!(users, vec!["target"]);
    sqlx::query("UPDATE users SET id = 'renamed' WHERE id = 'target'")
        .execute(&*pool)
        .await
        .unwrap();
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 2);
    let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM analytics_report_users ORDER BY id")
        .fetch_all(&*pool)
        .await
        .unwrap();
    assert_eq!(ids, vec!["renamed"]);
    sqlx::query("DELETE FROM ai_requests WHERE id = 'r'")
        .execute(&*pool)
        .await
        .unwrap();
    assert_eq!(reporting::process_pending(&db, 100).await.unwrap(), 1);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM analytics_report_ai_requests")
        .fetch_one(&*pool)
        .await
        .unwrap();
    assert_eq!(count, 0);

    sqlx::query("UPDATE users SET name = 'future' WHERE id = 'renamed'")
        .execute(&*pool)
        .await
        .unwrap();
    sqlx::query("UPDATE event_outbox SET fact = jsonb_set(fact, '{version}', '2') WHERE processed_at IS NULL")
        .execute(&*pool).await.unwrap();
    assert!(reporting::process_pending(&db, 100).await.is_err());
    assert_eq!(reporting::status(&db).await.unwrap().pending_count, 1);
    sqlx::query("UPDATE event_outbox SET fact = jsonb_set(fact, '{version}', '1') WHERE processed_at IS NULL")
        .execute(&*pool).await.unwrap();
    let worker = reporting::spawn(&db).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if reporting::status(&db).await.unwrap().pending_count == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    let name: String =
        sqlx::query_scalar("SELECT name FROM analytics_report_users WHERE id = 'renamed'")
            .fetch_one(&*pool)
            .await
            .unwrap();
    assert_eq!(name, "future");
    pool.close().await;
    sqlx::query(sqlx::AssertSqlSafe(format!("DROP SCHEMA {schema} CASCADE")))
        .execute(&admin)
        .await
        .unwrap();
}

#[test]
fn reporting_sql_is_accepted_by_install_time_schema_linter() {
    for (name, sql) in [
        (
            "analytics reporting",
            include_str!("../../../../../domain/analytics/schema/reporting.sql"),
        ),
        (
            "outbox capture",
            include_str!("../../../../../infra/events/schema/reporting_capture.sql"),
        ),
        ("users capture", systemprompt_users::REPORTING_CAPTURE_SQL),
        ("agent capture", systemprompt_agent::REPORTING_CAPTURE_SQL),
        ("AI capture", systemprompt_ai::REPORTING_CAPTURE_SQL),
        ("MCP capture", systemprompt_mcp::REPORTING_CAPTURE_SQL),
        (
            "content capture",
            systemprompt_content::REPORTING_CAPTURE_SQL,
        ),
        (
            "logging capture",
            systemprompt_logging::REPORTING_CAPTURE_SQL,
        ),
    ] {
        systemprompt_database::services::lint_declarative_schema(sql, name)
            .unwrap_or_else(|errors| panic!("{name}: {errors:?}"));
    }
}
