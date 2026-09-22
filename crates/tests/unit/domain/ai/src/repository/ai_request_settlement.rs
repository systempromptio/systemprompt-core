// DB-backed tests for the transactional settlement of a gateway request:
// owner check, replay safety, and the completion-over-failure precedence.

use serde_json::json;
use systemprompt_ai::error::RepositoryError;
use systemprompt_ai::repository::ai_requests::{
    ORPHANED_REASON, SettleCompletion, SettledFailure, SettledToolCall, SettlementOutcome,
    SettlementUsage,
};
use systemprompt_ai::repository::{AiRequestRepository, UpsertPayloadParams};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, AiToolCallId, UserId};

use super::{pool_or_skip, seed_request, user};

fn completion<'a>(
    body: &'a serde_json::Value,
    sha256: &'a str,
    tools: &'a [SettledToolCall],
) -> SettlementOutcome<'a> {
    SettlementOutcome::Completed(SettleCompletion {
        usage: SettlementUsage {
            input_tokens: 10,
            output_tokens: 5,
            cache_read_tokens: 2,
            cache_creation_tokens: 0,
            reasoning_tokens: 1,
            tokens_used: 15,
        },
        cost_microdollars: 1_234,
        latency_ms: 80,
        upstream_latency_ms: Some(60),
        finish_reason: Some("end_turn"),
        payload: UpsertPayloadParams {
            body: Some(body),
            excerpt: Some("hi"),
            truncated: false,
            bytes: Some(12),
            sha256: Some(sha256),
        },
        assistant_text: Some("the answer"),
        tool_calls: tools,
    })
}

#[tokio::test]
async fn payload_write_fault_rolls_back_the_entire_completion_settlement() {
    systemprompt_test_fixtures::ensure_test_bootstrap();
    let database =
        systemprompt_test_fixtures::DisposableDb::installed("ai_settlement_payload_fault")
            .await
            .expect("private AI database");
    let pool = database.pool().await.expect("private AI pool");
    let owner = user();
    let request_id = seed_request(&pool, &owner).await;
    let repo = AiRequestRepository::new(&pool).expect("AI request repository");
    let writer = pool.pool_arc().expect("private SQL pool");
    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "CREATE FUNCTION reject_settlement_payload() RETURNS trigger LANGUAGE plpgsql AS $$ \
         BEGIN RAISE EXCEPTION 'fixture payload persistence rejection'; END $$; \
         CREATE TRIGGER reject_settlement_payload BEFORE INSERT ON ai_request_payloads \
         FOR EACH ROW EXECUTE FUNCTION reject_settlement_payload()",
    ))
    .execute(writer.as_ref())
    .await
    .expect("install private payload-write fault");
    let body = json!({"content": "must not partially settle"});
    let tools = vec![SettledToolCall {
        id: AiToolCallId::new("toolu_rollback"),
        name: "lookup".to_owned(),
        input: "{}".to_owned(),
    }];

    let error = repo
        .settle(
            &request_id,
            &owner,
            completion(&body, "rollback-sha", &tools),
        )
        .await
        .expect_err("payload fault rejects completion settlement");
    assert!(
        error
            .to_string()
            .contains("fixture payload persistence rejection"),
        "{error}"
    );
    assert_eq!(row(&pool, &request_id).await.0, "pending");
    assert_eq!(turn_counts(&pool, &request_id).await, (0, 0));
    let payload_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai_request_payloads WHERE ai_request_id = $1")
            .bind(request_id.as_str())
            .fetch_one(writer.as_ref())
            .await
            .expect("payload row count");
    assert_eq!(payload_rows, 0);

    sqlx::raw_sql(sqlx::AssertSqlSafe(
        "DROP TRIGGER reject_settlement_payload ON ai_request_payloads; \
         DROP FUNCTION reject_settlement_payload()",
    ))
    .execute(writer.as_ref())
    .await
    .expect("remove private payload-write fault");
    repo.settle(
        &request_id,
        &owner,
        completion(&body, "rollback-sha", &tools),
    )
    .await
    .expect("the same operation recovers after the payload store is repaired");
    repo.settle(
        &request_id,
        &owner,
        completion(&body, "rollback-sha", &tools),
    )
    .await
    .expect("replaying the recovered completion is idempotent");
    assert_eq!(row(&pool, &request_id).await.0, "completed");
    assert_eq!(turn_counts(&pool, &request_id).await, (1, 1));
    let payload_rows: i64 =
        sqlx::query_scalar("SELECT count(*) FROM ai_request_payloads WHERE ai_request_id = $1")
            .bind(request_id.as_str())
            .fetch_one(writer.as_ref())
            .await
            .expect("recovered payload row count");
    assert_eq!(payload_rows, 1);

    drop(writer);
    drop(repo);
    pool.pool_arc().expect("private SQL pool").close().await;
    drop(pool);
    database.drop_now().await;
}

async fn row(pool: &DbPool, id: &AiRequestId) -> (String, Option<i32>, i64, Option<String>) {
    let read = pool.pool_arc().expect("read pool");
    let r = sqlx::query!(
        "SELECT status, tokens_used, cost_microdollars, error_message FROM ai_requests WHERE id = $1",
        id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("request row");
    (
        r.status,
        r.tokens_used,
        r.cost_microdollars,
        r.error_message,
    )
}

async fn finish_reason(pool: &DbPool, id: &AiRequestId) -> Option<String> {
    let read = pool.pool_arc().expect("read pool");
    sqlx::query_scalar!(
        "SELECT finish_reason FROM ai_requests WHERE id = $1",
        id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("request row")
}

async fn turn_counts(pool: &DbPool, id: &AiRequestId) -> (i64, i64) {
    let read = pool.pool_arc().expect("read pool");
    let messages = sqlx::query_scalar!(
        r#"SELECT count(*) as "n!" FROM ai_request_messages WHERE request_id = $1 AND role = 'assistant'"#,
        id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("messages");
    let tools = sqlx::query_scalar!(
        r#"SELECT count(*) as "n!" FROM ai_request_tool_calls WHERE request_id = $1"#,
        id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("tool calls");
    (messages, tools)
}

#[tokio::test]
async fn a_completion_settles_usage_payload_and_turn_in_one_transaction() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let body = json!({"content": "hi"});
    let tools = vec![SettledToolCall {
        id: AiToolCallId::new("toolu_1"),
        name: "lookup".to_owned(),
        input: "{}".to_owned(),
    }];

    repo.settle(&id, &uid, completion(&body, "sha-a", &tools))
        .await
        .expect("settle");

    let (status, tokens, cost, error) = row(&pool, &id).await;
    assert_eq!(status, "completed");
    assert_eq!(tokens, Some(15));
    assert_eq!(cost, 1_234);
    assert!(error.is_none());
    assert_eq!(
        finish_reason(&pool, &id).await.as_deref(),
        Some("end_turn"),
        "the upstream's own finish reason is persisted beside the status"
    );
    assert_eq!(turn_counts(&pool, &id).await, (1, 1));
}

#[tokio::test]
async fn replaying_the_same_completion_does_not_duplicate_the_turn() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let body = json!({"content": "hi"});
    let tools = vec![SettledToolCall {
        id: AiToolCallId::new("toolu_1"),
        name: "lookup".to_owned(),
        input: "{}".to_owned(),
    }];

    repo.settle(&id, &uid, completion(&body, "sha-a", &tools))
        .await
        .expect("first settle");
    repo.settle(&id, &uid, completion(&body, "sha-a", &tools))
        .await
        .expect("replayed settle");

    assert_eq!(turn_counts(&pool, &id).await, (1, 1));
}

#[tokio::test]
async fn a_different_terminal_response_is_rejected() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let body = json!({"content": "hi"});

    repo.settle(&id, &uid, completion(&body, "sha-a", &[]))
        .await
        .expect("first settle");
    let err = repo
        .settle(&id, &uid, completion(&body, "sha-b", &[]))
        .await
        .expect_err("conflicting response");

    assert!(
        matches!(err, RepositoryError::SettlementConflict { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn another_owner_cannot_settle_the_request() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");

    let err = repo
        .settle(
            &id,
            &UserId::new("someone-else"),
            SettlementOutcome::Failed(SettledFailure {
                error: "boom",
                ..Default::default()
            }),
        )
        .await
        .expect_err("owner mismatch");

    assert!(
        matches!(err, RepositoryError::SettlementConflict { .. }),
        "{err}"
    );
    assert_eq!(row(&pool, &id).await.0, "pending");
}

#[tokio::test]
async fn a_missing_request_row_is_a_settlement_conflict_not_a_database_error() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let repo = AiRequestRepository::new(&pool).expect("repo");

    let err = repo
        .settle(
            &AiRequestId::generate(),
            &user(),
            SettlementOutcome::Failed(SettledFailure {
                error: "boom",
                ..Default::default()
            }),
        )
        .await
        .expect_err("no row");

    assert!(
        matches!(err, RepositoryError::SettlementConflict { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn a_failure_never_overwrites_a_settled_completion() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let body = json!({"content": "hi"});

    repo.settle(&id, &uid, completion(&body, "sha-a", &[]))
        .await
        .expect("completion");
    repo.settle(
        &id,
        &uid,
        SettlementOutcome::Failed(SettledFailure {
            error: "late abort",
            ..Default::default()
        }),
    )
    .await
    .expect("late failure is accepted and ignored");

    let (status, _, cost, error) = row(&pool, &id).await;
    assert_eq!(status, "completed");
    assert_eq!(cost, 1_234);
    assert!(error.is_none());
}

#[tokio::test]
async fn a_failure_marks_the_request_failed_with_its_reason() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");

    repo.settle(
        &id,
        &uid,
        SettlementOutcome::Failed(SettledFailure {
            error: "upstream stream ended without stop event",
            ..Default::default()
        }),
    )
    .await
    .expect("failure");

    let (status, _, _, error) = row(&pool, &id).await;
    assert_eq!(status, "failed");
    assert_eq!(
        error.as_deref(),
        Some("upstream stream ended without stop event")
    );
}

#[tokio::test]
async fn the_orphan_sweep_fails_only_pending_rows_older_than_the_bound() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let stale = seed_request(&pool, &uid).await;
    let fresh = seed_request(&pool, &uid).await;
    let settled = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let write = pool.write_pool_arc().expect("write pool");
    sqlx::query!(
        "UPDATE ai_requests SET created_at = NOW() - INTERVAL '3 hours' WHERE id = ANY($1)",
        &[stale.as_str().to_owned(), settled.as_str().to_owned()]
    )
    .execute(write.as_ref())
    .await
    .expect("age rows");
    repo.settle(
        &settled,
        &uid,
        SettlementOutcome::Failed(SettledFailure {
            error: "upstream refused",
            ..Default::default()
        }),
    )
    .await
    .expect("settle");

    let orphaned = repo
        .fail_orphaned_pending(std::time::Duration::from_secs(3600))
        .await
        .expect("sweep");
    let mine: Vec<_> = orphaned
        .iter()
        .filter(|orphan| orphan.owner == uid)
        .collect();
    assert_eq!(mine.len(), 1, "only the stale pending row is swept");
    assert_eq!(mine[0].id, stale);

    let (status, _, _, error) = row(&pool, &stale).await;
    assert_eq!(status, "failed");
    assert_eq!(error.as_deref(), Some(ORPHANED_REASON));
    assert_eq!(row(&pool, &fresh).await.0, "pending");
    assert_eq!(
        row(&pool, &settled).await.3.as_deref(),
        Some("upstream refused"),
        "an already settled row keeps its own verdict"
    );
    assert!(
        repo.fail_orphaned_pending(std::time::Duration::from_secs(3600))
            .await
            .expect("second sweep")
            .iter()
            .all(|orphan| orphan.owner != uid),
        "the sweep is idempotent"
    );
}

fn native_priced_completion<'a>(
    body: &'a serde_json::Value,
    digest: &'a str,
) -> SettlementOutcome<'a> {
    let SettlementOutcome::Completed(mut value) = completion(body, digest, &[]) else {
        unreachable!("completion helper")
    };
    value.usage.input_tokens = 11;
    value.usage.output_tokens = 7;
    value.usage.tokens_used = 18;
    value.cost_microdollars = 25;
    SettlementOutcome::Completed(value)
}

#[tokio::test]
async fn accounting_failure_preserves_paid_completion_across_identical_and_conflicting_retries() {
    let pool = pool_or_skip()
        .await
        .expect("accounting regression requires fixture database");
    let uid = user();
    let id = seed_request(&pool, &uid).await;
    let repo = AiRequestRepository::new(&pool).expect("repo");
    let body = json!({"content":"native completed response"});
    repo.settle(
        &id,
        &uid,
        native_priced_completion(&body, "native-receipt-a"),
    )
    .await
    .expect("paid completion");
    repo.mark_accounting_failed(&id, &uid, "quota write failed")
        .await
        .expect("accounting projection");
    repo.mark_accounting_failed(&id, &uid, "quota write failed")
        .await
        .expect("identical retry");
    assert!(
        repo.mark_accounting_failed(&id, &user(), "quota write failed")
            .await
            .is_err(),
        "foreign owner cannot mark accounting failure"
    );
    assert!(
        repo.mark_accounting_failed(&id, &uid, "different evidence")
            .await
            .is_err(),
        "conflicting accounting evidence must not replace retained failure"
    );
    repo.settle(
        &id,
        &uid,
        native_priced_completion(&body, "native-receipt-a"),
    )
    .await
    .expect("identical completion retry");
    assert!(
        matches!(
            repo.settle(
                &id,
                &uid,
                native_priced_completion(&json!({"different":true}), "native-receipt-b")
            )
            .await,
            Err(RepositoryError::SettlementConflict { .. })
        ),
        "conflicting provider terminal receipt stays rejected"
    );
    repo.settle(
        &id,
        &uid,
        SettlementOutcome::Failed(SettledFailure {
            error: "late provider failure",
            ..Default::default()
        }),
    )
    .await
    .expect("generic failure retry remains harmless");
    let read = pool.pool_arc().expect("pool");
    let stored=sqlx::query!("SELECT status,input_tokens,output_tokens,cost_microdollars,accounting_error,accounting_failed_at,error_message FROM ai_requests WHERE id=$1",id.as_str()).fetch_one(read.as_ref()).await.expect("stored projection");
    assert_eq!(stored.status, "failed");
    assert_eq!(stored.input_tokens, Some(11));
    assert_eq!(stored.output_tokens, Some(7));
    assert_eq!(stored.cost_microdollars, 25);
    assert_eq!(
        stored.accounting_error.as_deref(),
        Some("quota write failed")
    );
    assert_eq!(stored.error_message.as_deref(), Some("quota write failed"));
    assert!(stored.accounting_failed_at.is_some());
    assert_eq!(
        turn_counts(&pool, &id).await,
        (1, 0),
        "accounting and completion retries cannot duplicate paid turn"
    );
}
