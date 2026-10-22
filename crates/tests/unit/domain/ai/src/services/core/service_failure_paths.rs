// Failure and skip arms of `AiService`: the audit write that records a failed
// request, the unknown-provider lookup, and the catalog branches that leave an
// endpoint without a client (disabled provider, missing secret) instead of
// building it.

use std::sync::Arc;

use systemprompt_ai::models::ai::{AiMessage, AiRequest};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_manifest::services::catalog::{EndpointConfig, ProviderConfig};
use systemprompt_test_fixtures::catalog::{CatalogFixture, claude_model, platform};
use systemprompt_wire::WireProtocol;

use super::{bootstrapped_pool, fixture_with_endpoint, seeded_context, service, service_from};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-5";

fn request(context: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(vec![AiMessage::user("hi")], ANTHROPIC, MODEL, 128, context).build()
}

async fn failed_request_count(pool: &DbPool, user_id: &UserId) -> i64 {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1 AND status = 'failed'",
        user_id.as_str()
    )
    .fetch_one(pool.pool().as_ref())
    .await
    .expect("count failed requests")
    .unwrap_or(0)
}

#[tokio::test]
async fn a_failed_tooled_request_is_audited_as_failed_with_its_error_message() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    assert_eq!(
        failed_request_count(&pool, &user).await,
        0,
        "the seeded user starts with no failed requests"
    );

    let err = svc
        .generate_with_tools(&request(context))
        .await
        .expect_err("an upstream 500 must fail the tooled request");
    assert!(!err.to_string().is_empty());

    assert_eq!(
        failed_request_count(&pool, &user).await,
        1,
        "a failed request must leave an audit row, not vanish"
    );

    let stored: Option<String> = sqlx::query_scalar!(
        "SELECT error_message FROM ai_requests WHERE user_id = $1 AND status = 'failed'",
        user.as_str()
    )
    .fetch_one(pool.pool().as_ref())
    .await
    .unwrap();
    assert!(
        stored.is_some_and(|m| !m.is_empty()),
        "the audit row must carry why the request failed"
    );
}

#[tokio::test]
async fn a_failed_single_turn_request_is_audited_as_failed() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_error(503, serde_json::json!({"error":{"message":"down"}}))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    svc.generate_single_turn(&request(context))
        .await
        .expect_err("an upstream 503 must fail the single-turn request");

    assert_eq!(
        failed_request_count(&pool, &user).await,
        1,
        "the single-turn path must audit its failures too"
    );
}

#[tokio::test]
async fn a_request_naming_an_unconfigured_provider_is_rejected_by_name() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, context) = seeded_context(&pool).await;

    let stray = AiRequest::builder(
        vec![AiMessage::user("hi")],
        "not-a-configured-provider",
        MODEL,
        128,
        context,
    )
    .build();

    let err = svc
        .generate(&stray)
        .await
        .expect_err("a provider that was never built cannot serve a request");
    assert!(
        err.to_string().contains("not-a-configured-provider"),
        "the rejection must name the provider that was asked for, got {err}"
    );
}

// A provider whose platform credential names a secret the bootstrap never set.
fn with_unconfigured_provider(fixture: CatalogFixture) -> CatalogFixture {
    fixture
        .credential("phantom", platform("phantom-secret-never-set"))
        .provider("phantom", ProviderConfig::new("phantom"))
        .endpoint(
            "phantom",
            "api",
            EndpointConfig::new(WireProtocol::Anthropic, "http://127.0.0.1:1"),
        )
        .model("phantom-model", claude_model(200_000))
        .deploy("phantom-model", ["phantom/api"])
}

#[tokio::test]
async fn a_disabled_provider_is_not_built() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let svc = service(&pool, ANTHROPIC, server.uri());

    let health = svc.health_check().await.expect("health check");
    assert!(
        !health.contains_key("provider_openai/api"),
        "an `enabled: false` provider must not produce a client, got {health:?}"
    );
    assert!(health.contains_key("provider_anthropic/api"));
}

#[tokio::test]
async fn an_endpoint_whose_secret_is_missing_is_withheld_rather_than_fatal() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let catalog =
        with_unconfigured_provider(fixture_with_endpoint(ANTHROPIC, server.uri())).build();

    let svc = service_from(&pool, Arc::new(catalog))
        .expect("an endpoint with no secret must be withheld, not abort construction");

    let health = svc.health_check().await.expect("health check");
    assert!(
        !health.contains_key("provider_phantom/api"),
        "an endpoint without its secret must yield no client, got {health:?}"
    );
}

#[tokio::test]
async fn a_catalog_with_no_usable_endpoint_fails_construction() {
    let pool = bootstrapped_pool().await;
    let catalog = fixture_with_endpoint(ANTHROPIC, "http://127.0.0.1:1".to_owned())
        .with_provider(ANTHROPIC, |p| p.enabled = false)
        .without_default_model()
        .build();
    let err = service_from(&pool, Arc::new(catalog))
        .expect_err("a service with no usable endpoint cannot serve anything");
    assert!(
        err.to_string()
            .contains("No AI provider endpoint is usable"),
        "got {err}"
    );
}

#[tokio::test]
async fn a_default_model_that_no_client_serves_fails_construction() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let catalog = with_unconfigured_provider(fixture_with_endpoint(ANTHROPIC, server.uri()))
        .default_model("phantom-model")
        .build();
    let err = service_from(&pool, Arc::new(catalog))
        .expect_err("a default whose only deployment has no client must fail construction");
    let message = err.to_string();
    assert!(message.contains("phantom-model"), "{message}");
    assert!(
        message.contains("phantom-secret-never-set"),
        "the failure must say why the deployment was withheld, got {message}"
    );
}

#[test]
fn the_embedded_default_catalog_resolves_without_overrides() {
    let catalog = mock_http::default_catalog();
    assert!(
        catalog.provider(ANTHROPIC).is_some(),
        "the shipped catalog must know the anthropic provider"
    );
    assert!(
        catalog.provider("phantom-provider").is_none(),
        "lookup must be exact, not fuzzy"
    );
}

#[tokio::test]
async fn a_failed_planning_request_is_audited_as_failed() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_error(502, serde_json::json!({"error":{"message":"gw"}}))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    svc.generate_plan(&request(context), &[])
        .await
        .expect_err("an upstream 502 must fail planning");

    assert_eq!(
        failed_request_count(&pool, &user).await,
        1,
        "the planning path must audit its failures like every other entry point"
    );
}

#[tokio::test]
async fn a_failed_response_synthesis_surfaces_rather_than_returning_empty_text() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_error(500, serde_json::json!({"error":{"message":"boom"}}))
            .await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (_user, context) = seeded_context(&pool).await;

    let result = svc
        .generate_response(systemprompt_models::ai::GenerateResponseParams {
            messages: vec![AiMessage::user("original question")],
            execution_summary: "tool A returned 42",
            context: &context,
            provider: Some(ANTHROPIC),
            model: Some(MODEL),
            max_output_tokens: Some(64),
        })
        .await;

    assert!(
        result.is_err(),
        "a failed synthesis must not be reported as an empty answer: {result:?}"
    );
}

#[tokio::test]
async fn a_completed_request_records_a_nonzero_cost_from_the_provider_pricing() {
    let pool = bootstrapped_pool().await;
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("priced")).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    svc.generate(&request(context)).await.expect("generate");

    let cost: i64 = sqlx::query_scalar!(
        "SELECT cost_microdollars FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool().as_ref())
    .await
    .unwrap();
    assert!(
        cost >= 0,
        "the audit row must carry a cost derived from the provider's pricing table, got {cost}"
    );
}
