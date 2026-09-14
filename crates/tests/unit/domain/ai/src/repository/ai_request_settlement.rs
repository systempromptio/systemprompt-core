// DB-backed tests for the transactional settlement of a gateway request:
// owner check, replay safety, and the completion-over-failure precedence.

use serde_json::json;
use systemprompt_ai::error::RepositoryError;
use systemprompt_ai::repository::ai_requests::{
    ORPHANED_REASON, SettleCompletion, SettledToolCall, SettlementOutcome, SettlementUsage,
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
            SettlementOutcome::Failed { error: "boom" },
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
            SettlementOutcome::Failed { error: "boom" },
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
        SettlementOutcome::Failed {
            error: "late abort",
        },
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
        SettlementOutcome::Failed {
            error: "upstream stream ended without stop event",
        },
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
        SettlementOutcome::Failed {
            error: "upstream refused",
        },
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
