// DB-backed tests for AiRequestPayloadRepository upsert paths (FK to
// ai_requests).

use serde_json::json;
use systemprompt_ai::repository::ai_requests::{
    SettleCompletion, SettlementOutcome, SettlementUsage,
};
use systemprompt_ai::repository::{
    AiRequestPayloadRepository, AiRequestRepository, UpsertPayloadParams,
};
use systemprompt_identifiers::AiRequestId;

use super::{pool_or_skip, seed_request, user};

#[tokio::test]
async fn upsert_request_then_response_coexist() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiRequestPayloadRepository::new(&pool).expect("repo");

    let req_body = json!({"prompt": "hello"});
    repo.upsert_request(
        &request_id,
        UpsertPayloadParams {
            body: Some(&req_body),
            excerpt: Some("hello"),
            truncated: false,
            bytes: Some(20),
            sha256: Some("aaaa"),
        },
    )
    .await
    .expect("upsert request");

    let resp_body = json!({"content": "hi"});
    AiRequestRepository::new(&pool)
        .expect("requests repo")
        .settle(
            &request_id,
            &uid,
            SettlementOutcome::Completed(SettleCompletion {
                usage: SettlementUsage::default(),
                cost_microdollars: 0,
                latency_ms: 1,
                upstream_latency_ms: None,
                finish_reason: None,
                payload: UpsertPayloadParams {
                    body: Some(&resp_body),
                    excerpt: Some("hi"),
                    truncated: true,
                    bytes: Some(8),
                    sha256: Some("bbbb"),
                },
                assistant_text: None,
                tool_calls: &[],
            }),
        )
        .await
        .expect("settle response");

    // Read back both columns directly to confirm the second upsert took the
    // ON CONFLICT branch rather than overwriting the request payload.
    let read = pool.pool_arc().expect("read pool");
    let row = sqlx::query!(
        r#"SELECT request_excerpt, response_excerpt, request_truncated, response_truncated,
                  request_body_sha256, response_body_sha256
           FROM ai_request_payloads WHERE ai_request_id = $1"#,
        request_id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("fetch");
    assert_eq!(row.request_excerpt.as_deref(), Some("hello"));
    assert_eq!(row.response_excerpt.as_deref(), Some("hi"));
    assert!(!row.request_truncated);
    assert!(row.response_truncated);
    assert_eq!(row.request_body_sha256.as_deref(), Some("aaaa"));
    assert_eq!(row.response_body_sha256.as_deref(), Some("bbbb"));
}

#[tokio::test]
async fn upsert_request_twice_updates_in_place() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiRequestPayloadRepository::new(&pool).expect("repo");

    repo.upsert_request(
        &request_id,
        UpsertPayloadParams {
            body: None,
            excerpt: Some("first"),
            truncated: false,
            bytes: None,
            sha256: None,
        },
    )
    .await
    .expect("first");
    repo.upsert_request(
        &request_id,
        UpsertPayloadParams {
            body: None,
            excerpt: Some("second"),
            truncated: false,
            bytes: None,
            sha256: None,
        },
    )
    .await
    .expect("second");

    let read = pool.pool_arc().expect("read pool");
    let count = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_request_payloads WHERE ai_request_id = $1",
        request_id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("count");
    assert_eq!(count, Some(1));
    let excerpt = sqlx::query_scalar!(
        "SELECT request_excerpt FROM ai_request_payloads WHERE ai_request_id = $1",
        request_id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("excerpt");
    assert_eq!(excerpt.as_deref(), Some("second"));
}

#[tokio::test]
async fn upsert_prepared_does_not_clobber_request_payload() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiRequestPayloadRepository::new(&pool).expect("repo");

    let req_body = json!({"prompt": "hello"});
    repo.upsert_request(
        &request_id,
        UpsertPayloadParams {
            body: Some(&req_body),
            excerpt: None,
            truncated: false,
            bytes: Some(20),
            sha256: Some("received-digest"),
        },
    )
    .await
    .expect("upsert request");

    let tools = json!([{"name": "read", "input_schema": {"type": "object"}}]);
    repo.upsert_prepared(&request_id, "prepared-digest", Some(&tools))
        .await
        .expect("upsert prepared");

    let read = pool.pool_arc().expect("read pool");
    let row = sqlx::query!(
        r#"SELECT request_body_sha256, prepared_body_sha256, prepared_tools, request_bytes
           FROM ai_request_payloads WHERE ai_request_id = $1"#,
        request_id.as_str()
    )
    .fetch_one(read.as_ref())
    .await
    .expect("fetch");
    assert_eq!(row.request_body_sha256.as_deref(), Some("received-digest"));
    assert_eq!(row.prepared_body_sha256.as_deref(), Some("prepared-digest"));
    assert_eq!(row.prepared_tools, Some(tools.clone()));
    assert_eq!(row.request_bytes, Some(20));

    let prepared = repo
        .find_prepared(&request_id)
        .await
        .expect("find prepared")
        .expect("row present");
    assert_eq!(
        prepared.prepared_body_sha256.as_deref(),
        Some("prepared-digest")
    );
    assert_eq!(prepared.prepared_tools, Some(tools));
}

#[tokio::test]
async fn upsert_prepared_without_tools_clears_a_stale_slice() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let uid = user();
    let request_id = seed_request(&pool, &uid).await;
    let repo = AiRequestPayloadRepository::new(&pool).expect("repo");

    let tools = json!([{"name": "read"}]);
    repo.upsert_prepared(&request_id, "first", Some(&tools))
        .await
        .expect("first prepared");
    repo.upsert_prepared(&request_id, "second", None)
        .await
        .expect("second prepared");

    let prepared = repo
        .find_prepared(&request_id)
        .await
        .expect("find prepared")
        .expect("row present");
    assert_eq!(prepared.prepared_body_sha256.as_deref(), Some("second"));
    assert!(
        prepared.prepared_tools.is_none(),
        "a re-prepared body without tools must not keep the earlier slice"
    );
}

#[tokio::test]
async fn find_prepared_is_none_for_an_unknown_request() {
    let Some(pool) = pool_or_skip().await else {
        return;
    };
    let repo = AiRequestPayloadRepository::new(&pool).expect("repo");
    let missing = AiRequestId::generate();
    assert!(repo.find_prepared(&missing).await.expect("query").is_none());
}
