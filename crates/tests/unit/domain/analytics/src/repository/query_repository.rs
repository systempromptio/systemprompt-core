//! Tests for `AnalyticsQueryRepository::get_ai_provider_usage`
//! (`query_as!` aggregation over `ai_requests`, with and without the user
//! filter).

use systemprompt_analytics::AnalyticsQueryRepository;
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
    let repo = AnalyticsQueryRepository::new(&pool).expect("repo");

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
    let repo = AnalyticsQueryRepository::new(&pool).expect("repo");

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
