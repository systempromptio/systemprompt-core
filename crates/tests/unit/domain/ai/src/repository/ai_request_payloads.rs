// DB-backed tests for AiRequestPayloadRepository upsert paths (FK to ai_requests).

use serde_json::json;
use systemprompt_ai::repository::{AiRequestPayloadRepository, UpsertPayloadParams};

use super::{pool, seed_request, user};

#[tokio::test]
async fn upsert_request_then_response_coexist() {
    let Some(pool) = pool().await else {
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
        },
    )
    .await
    .expect("upsert request");

    let resp_body = json!({"content": "hi"});
    repo.upsert_response(
        &request_id,
        UpsertPayloadParams {
            body: Some(&resp_body),
            excerpt: Some("hi"),
            truncated: true,
            bytes: Some(8),
        },
    )
    .await
    .expect("upsert response");

    // Read back both columns directly to confirm the second upsert took the
    // ON CONFLICT branch rather than overwriting the request payload.
    let read = pool.pool_arc().expect("read pool");
    let row = sqlx::query!(
        r#"SELECT request_excerpt, response_excerpt, request_truncated, response_truncated
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
}

#[tokio::test]
async fn upsert_request_twice_updates_in_place() {
    let Some(pool) = pool().await else {
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
