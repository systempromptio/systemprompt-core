//! `/bridge/profile/usage` driven against a user who actually has AI requests.
//!
//! The existing coverage hits the endpoint for a user with no history, so every
//! projection inside it — the per-model token-share mapping, the conversation
//! grouping by model and agent, the recent-conversation list — maps over an
//! empty vector and never runs. Seeding a handful of completed requests for one
//! freshly-minted user puts all of them on their real path.
//!
//! Every assertion is scoped to that user's own id. The underlying queries are
//! top-N per user, so asserting anything about the global ordering would be
//! reading a limit, not a result.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, ContextId, UserId};
use systemprompt_test_fixtures::{
    AuthedFixture, ensure_test_bootstrap, fixture_app_context, fixture_db_pool,
    install_test_signing_key, seed_admin_credential,
};
use tower::ServiceExt;
use uuid::Uuid;

use super::common::body_to_string;

struct Seeded {
    cred: AuthedFixture,
    context: ContextId,
}

async fn app() -> Result<(Router, DbPool)> {
    let b = ensure_test_bootstrap();
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((
        gateway_router(&ctx).expect("gateway router available"),
        pool,
    ))
}

async fn seed_request(
    pool: &DbPool,
    user: &UserId,
    context: &ContextId,
    model: &str,
    tokens: i32,
    cost: i64,
) -> Result<()> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    let id = AiRequestId::generate();
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, context_id, provider, model, \
         requested_model, tokens_used, input_tokens, output_tokens, cost_microdollars, cache_hit, \
         is_streaming, status, actor_kind, actor_id, completed_at) \
         VALUES ($1, $1, $2, $3, 'anthropic', $4, $4, $5, $5, $5, $6, false, false, 'completed', \
         'user', $2, NOW())",
    )
    .bind(id.as_str())
    .bind(user.as_str())
    .bind(context.as_str())
    .bind(model)
    .bind(tokens)
    .bind(cost)
    .execute(pg.as_ref())
    .await?;
    Ok(())
}

async fn seed_history(pool: &DbPool, email: &str) -> Result<Seeded> {
    let cred = seed_admin_credential(pool, email).await?;
    let context = ContextId::generate();
    seed_request(pool, &cred.user_id, &context, "claude-usage-a", 1_000, 500).await?;
    seed_request(pool, &cred.user_id, &context, "claude-usage-a", 500, 250).await?;
    seed_request(pool, &cred.user_id, &context, "claude-usage-b", 500, 125).await?;
    Ok(Seeded { cred, context })
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request must build")
}

async fn usage(app: Router, cred: &AuthedFixture) -> Result<serde_json::Value> {
    let (status, body) = body_to_string(
        app.oneshot(authed_get("/bridge/profile/usage", cred.jwt.as_str()))
            .await?,
    )
    .await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    Ok(serde_json::from_str(&body)?)
}

#[tokio::test]
async fn the_usage_report_breaks_spend_down_by_model() -> Result<()> {
    let (app, pool) = app().await?;
    let seeded = seed_history(
        &pool,
        &format!("usage-a-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;

    let report = usage(app, &seeded.cred).await?;

    let models = report["top_models"]
        .as_array()
        .expect("the report lists per-model usage");
    let names: Vec<&str> = models.iter().filter_map(|m| m["model"].as_str()).collect();
    assert!(names.contains(&"claude-usage-a"), "{report}");
    assert!(names.contains(&"claude-usage-b"), "{report}");

    let a = models
        .iter()
        .find(|m| m["model"].as_str() == Some("claude-usage-a"))
        .expect("the busier model is present");
    assert_eq!(
        a["requests"].as_i64(),
        Some(2),
        "both requests against that model are counted: {report}"
    );
    assert_eq!(a["tokens"].as_i64(), Some(1_500), "{report}");
    assert_eq!(a["cost_microdollars"].as_i64(), Some(750), "{report}");
    Ok(())
}

#[tokio::test]
async fn the_token_shares_are_proportions_of_this_users_own_total() -> Result<()> {
    let (app, pool) = app().await?;
    let seeded = seed_history(
        &pool,
        &format!("usage-b-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;

    let report = usage(app, &seeded.cred).await?;
    let models = report["top_models"].as_array().expect("per-model usage");

    let share_of = |model: &str| -> f64 {
        models
            .iter()
            .find(|m| m["model"].as_str() == Some(model))
            .and_then(|m| m["token_share"].as_f64())
            .unwrap_or_default()
    };

    // 1500 of 2000 tokens against model a, 500 against model b.
    assert!((share_of("claude-usage-a") - 0.75).abs() < 1e-6, "{report}");
    assert!((share_of("claude-usage-b") - 0.25).abs() < 1e-6, "{report}");
    let total: f64 = models
        .iter()
        .filter_map(|m| m["token_share"].as_f64())
        .sum();
    assert!(
        (total - 1.0).abs() < 1e-6,
        "the shares must sum to the whole: {report}"
    );
    Ok(())
}

#[tokio::test]
async fn the_usage_report_groups_the_users_conversations() -> Result<()> {
    let (app, pool) = app().await?;
    let seeded = seed_history(
        &pool,
        &format!("usage-c-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;

    let report = usage(app, &seeded.cred).await?;
    let conversations = &report["conversations"];

    assert_eq!(
        conversations["total_conversations"].as_i64(),
        Some(1),
        "all three requests share one context: {report}"
    );
    assert_eq!(
        conversations["total_ai_requests"].as_i64(),
        Some(3),
        "{report}"
    );

    let by_model = conversations["by_model"]
        .as_array()
        .expect("conversations are grouped by model");
    assert!(!by_model.is_empty(), "{report}");

    let recent = conversations["recent"]
        .as_array()
        .expect("the report lists recent conversations");
    assert!(
        recent
            .iter()
            .any(|r| r["context_id"].as_str() == Some(seeded.context.as_str())),
        "the user's own conversation must appear in their recent list: {report}"
    );
    Ok(())
}

#[tokio::test]
async fn a_user_with_no_history_gets_an_empty_report_not_an_error() -> Result<()> {
    let (app, pool) = app().await?;
    let cred = seed_admin_credential(
        &pool,
        &format!("usage-d-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;

    let report = usage(app, &cred).await?;

    assert_eq!(
        report["top_models"].as_array().map(Vec::len),
        Some(0),
        "{report}"
    );
    assert_eq!(
        report["conversations"]["total_ai_requests"].as_i64(),
        Some(0),
        "a fresh user must not inherit anyone else's usage: {report}"
    );
    Ok(())
}

#[tokio::test]
async fn one_users_usage_is_not_visible_to_another() -> Result<()> {
    let (app, pool) = app().await?;
    let busy = seed_history(
        &pool,
        &format!("usage-e-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;
    let bystander = seed_admin_credential(
        &pool,
        &format!("usage-f-{}@example.invalid", Uuid::new_v4()),
    )
    .await?;

    let report = usage(app, &bystander).await?;

    let names: Vec<&str> = report["top_models"]
        .as_array()
        .map(|m| m.iter().filter_map(|e| e["model"].as_str()).collect())
        .unwrap_or_default();
    assert!(
        !names.contains(&"claude-usage-a"),
        "usage is per-user; another account's models must not leak: {report}"
    );
    let _ = busy;
    Ok(())
}
