//! The `source` CHECK of `mcp_tool_executions` and `mcp_artifacts` against a
//! migrated database: every `ExecutionSource` is stored, `hook_harness`
//! included, and a string outside the vocabulary is refused.

use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::mcp::ExecutionSource;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

async fn seed_user(pool: &DbPool) -> UserId {
    let user = UserId::new(format!("source-check-{}", Uuid::new_v4().simple()));
    systemprompt_test_fixtures::seed_user_row(
        pool,
        &user,
        &format!("{}@source-check.invalid", user.as_str()),
    )
    .await
    .expect("user row");
    user
}

async fn insert_execution(
    pool: &DbPool,
    execution_id: &str,
    user: &UserId,
    source: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO mcp_tool_executions
            (mcp_execution_id, tool_name, server_name, started_at, input, status, user_id, source)
        VALUES ($1, 'Read', $2, NOW(), '{}', 'success', $3, $2)
        ",
    )
    .bind(execution_id)
    .bind(source)
    .bind(user.as_str())
    .execute(pool.write_pool().as_ref())
    .await
    .map(|_| ())
}

async fn insert_artifact(
    pool: &DbPool,
    execution_id: &str,
    source: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO mcp_artifacts
            (artifact_id, mcp_execution_id, server_name, artifact_type, source, data)
        VALUES ($1, $2, $3, 'tool_result', $3, '{}'::jsonb)
        ",
    )
    .bind(format!("art-{}", Uuid::new_v4().simple()))
    .bind(execution_id)
    .bind(source)
    .execute(pool.write_pool().as_ref())
    .await
    .map(|_| ())
}

async fn stored_sources(pool: &DbPool, execution_id: &str) -> [String; 2] {
    let execution =
        sqlx::query_scalar("SELECT source FROM mcp_tool_executions WHERE mcp_execution_id = $1")
            .bind(execution_id)
            .fetch_one(pool.write_pool().as_ref())
            .await
            .expect("stored execution");
    let artifact =
        sqlx::query_scalar("SELECT source FROM mcp_artifacts WHERE mcp_execution_id = $1")
            .bind(execution_id)
            .fetch_one(pool.write_pool().as_ref())
            .await
            .expect("stored artifact");
    [execution, artifact]
}

fn is_check_violation(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .is_some_and(sqlx::error::DatabaseError::is_check_violation)
}

async fn cleanup(pool: &DbPool, user: &UserId) {
    sqlx::query("DELETE FROM mcp_tool_executions WHERE user_id = $1")
        .bind(user.as_str())
        .execute(pool.write_pool().as_ref())
        .await
        .expect("cleanup");
}

#[tokio::test]
async fn both_tables_store_every_execution_source() {
    let pool = test_db_pool().await;
    let user = seed_user(&pool).await;

    for source in ExecutionSource::ALL {
        let execution_id = Uuid::new_v4().to_string();
        insert_execution(&pool, &execution_id, &user, source.as_str())
            .await
            .unwrap_or_else(|e| panic!("mcp_tool_executions refuses {source}: {e}"));
        insert_artifact(&pool, &execution_id, source.as_str())
            .await
            .unwrap_or_else(|e| panic!("mcp_artifacts refuses {source}: {e}"));
        for stored in stored_sources(&pool, &execution_id).await {
            assert_eq!(ExecutionSource::parse(&stored), Some(source));
        }
    }

    cleanup(&pool, &user).await;
}

#[tokio::test]
async fn a_harness_hook_row_is_stored_and_an_unlisted_source_is_refused() {
    let pool = test_db_pool().await;
    let user = seed_user(&pool).await;

    let execution_id = Uuid::new_v4().to_string();
    insert_execution(&pool, &execution_id, &user, "hook_harness")
        .await
        .expect("mcp_tool_executions admits hook_harness");
    insert_artifact(&pool, &execution_id, "hook_harness")
        .await
        .expect("mcp_artifacts admits hook_harness");

    let refused = insert_execution(&pool, &Uuid::new_v4().to_string(), &user, "hook_unlisted")
        .await
        .expect_err("an unlisted source is refused");
    assert!(is_check_violation(&refused), "{refused}");

    let second = Uuid::new_v4().to_string();
    insert_execution(&pool, &second, &user, "hook_harness")
        .await
        .expect("second execution");
    let refused = insert_artifact(&pool, &second, "hook_unlisted")
        .await
        .expect_err("an unlisted artifact source is refused");
    assert!(is_check_violation(&refused), "{refused}");

    cleanup(&pool, &user).await;
}
