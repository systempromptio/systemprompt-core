//! Tests for `AnalyticsQueryRepository::get_ai_provider_usage` (dynamic-SQL
//! aggregation over `ai_requests`, with and without the user filter) and the
//! `ProviderUsage::from_json_row` decoder.

use serde_json::json;
use systemprompt_analytics::{AnalyticsQueryRepository, ProviderUsage};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_database_url, fixture_db_pool};
use uuid::Uuid;

async fn insert_ai_request(pool: &DbPool, user_id: &str, provider: &str, model: &str) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query(
        r"
        INSERT INTO ai_requests
            (id, request_id, user_id, provider, model, tokens_used,
             cost_microdollars, latency_ms, status, actor_kind, actor_id)
        VALUES ($1, $2, $3, $4, $5, 100, 2500, 40, 'completed', 'user', $3)
        ",
    )
    .bind(Uuid::new_v4().to_string())
    .bind(Uuid::new_v4().to_string())
    .bind(user_id)
    .bind(provider)
    .bind(model)
    .execute(p.as_ref())
    .await
    .expect("insert ai_request");
}

async fn cleanup(pool: &DbPool, provider: &str) {
    let p = pool.write_pool_arc().expect("write pool");
    sqlx::query("DELETE FROM ai_requests WHERE provider = $1")
        .bind(provider)
        .execute(p.as_ref())
        .await
        .ok();
}

#[tokio::test]
async fn get_ai_provider_usage_aggregates_by_provider_and_model() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = AnalyticsQueryRepository::new(&pool);

    let provider = format!("prov-{}", Uuid::new_v4());
    let user_a = format!("user-{}", Uuid::new_v4());
    let user_b = format!("user-{}", Uuid::new_v4());
    insert_ai_request(&pool, &user_a, &provider, "model-x").await;
    insert_ai_request(&pool, &user_a, &provider, "model-x").await;
    insert_ai_request(&pool, &user_b, &provider, "model-y").await;

    let usage = repo.get_ai_provider_usage(7, None).await.expect("usage");
    let mine: Vec<_> = usage.iter().filter(|u| u.provider == provider).collect();
    assert_eq!(mine.len(), 2);

    let model_x = mine.iter().find(|u| u.model == "model-x").expect("model-x");
    assert_eq!(model_x.request_count, 2);
    assert_eq!(model_x.total_tokens, Some(200));
    assert_eq!(model_x.unique_users, 1);
    assert!(model_x.avg_latency_ms.is_some());

    cleanup(&pool, &provider).await;
}

#[tokio::test]
async fn get_ai_provider_usage_filters_by_user() {
    let Ok(url) = fixture_database_url() else {
        return;
    };
    ensure_test_bootstrap();
    let pool = fixture_db_pool(&url).await.expect("pool");
    let repo = AnalyticsQueryRepository::new(&pool);

    let provider = format!("prov-{}", Uuid::new_v4());
    let user_a = format!("user-{}", Uuid::new_v4());
    let user_b = format!("user-{}", Uuid::new_v4());
    insert_ai_request(&pool, &user_a, &provider, "model-x").await;
    insert_ai_request(&pool, &user_b, &provider, "model-y").await;

    let uid = UserId::new(user_a);
    let usage = repo
        .get_ai_provider_usage(7, Some(&uid))
        .await
        .expect("usage");
    let mine: Vec<_> = usage.iter().filter(|u| u.provider == provider).collect();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].model, "model-x");

    cleanup(&pool, &provider).await;
}

#[test]
fn from_json_row_decodes_complete_row() {
    let row = json!({
        "provider": "anthropic",
        "model": "claude-x",
        "request_count": 3,
        "total_tokens": 900,
        "total_cost_microdollars": 1200,
        "avg_latency_ms": 55.5,
        "unique_users": 2,
        "unique_sessions": 3
    });
    let map: std::collections::HashMap<String, serde_json::Value> = row
        .as_object()
        .expect("object")
        .clone()
        .into_iter()
        .collect();

    let usage = ProviderUsage::from_json_row(&map).expect("decode");
    assert_eq!(usage.provider, "anthropic");
    assert_eq!(usage.model, "claude-x");
    assert_eq!(usage.request_count, 3);
    assert_eq!(usage.total_tokens, Some(900));
    assert_eq!(usage.total_cost_microdollars, Some(1200));
    assert_eq!(usage.avg_latency_ms, Some(55.5));
    assert_eq!(usage.unique_users, 2);
    assert_eq!(usage.unique_sessions, 3);
}

#[test]
fn from_json_row_tolerates_null_optionals() {
    let row = json!({
        "provider": "anthropic",
        "model": "claude-x",
        "request_count": 1,
        "total_tokens": null,
        "total_cost_microdollars": null,
        "avg_latency_ms": null,
        "unique_users": 1,
        "unique_sessions": 1
    });
    let map: std::collections::HashMap<String, serde_json::Value> = row
        .as_object()
        .expect("object")
        .clone()
        .into_iter()
        .collect();

    let usage = ProviderUsage::from_json_row(&map).expect("decode");
    assert_eq!(usage.total_tokens, None);
    assert_eq!(usage.total_cost_microdollars, None);
    assert_eq!(usage.avg_latency_ms, None);
}

#[test]
fn from_json_row_rejects_missing_required_fields() {
    for missing in [
        "provider",
        "model",
        "request_count",
        "unique_users",
        "unique_sessions",
    ] {
        let mut row = json!({
            "provider": "anthropic",
            "model": "claude-x",
            "request_count": 1,
            "unique_users": 1,
            "unique_sessions": 1
        });
        row.as_object_mut().expect("object").remove(missing);
        let map: std::collections::HashMap<String, serde_json::Value> = row
            .as_object()
            .expect("object")
            .clone()
            .into_iter()
            .collect();

        let err = ProviderUsage::from_json_row(&map).expect_err("missing field");
        assert!(err.to_string().contains(missing), "field {missing}");
    }
}
