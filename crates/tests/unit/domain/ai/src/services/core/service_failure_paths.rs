// Failure and skip arms of `AiService`: the audit write that records a failed
// request, the unknown-provider lookup, and the two provider-construction
// branches that drop a configured provider instead of building it.

use std::sync::Arc;

use systemprompt_ai::models::ai::{AiMessage, AiRequest};
use systemprompt_ai::{AiService, NoopToolProvider};
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::profile::ProviderRegistry;
use systemprompt_models::services::{AiConfig, AiProviderConfig};

use super::{
    ai_config, noop_session_provider, pool, registry_with_endpoint, seeded_context, service,
};
use crate::services::providers::mock_http;

const ANTHROPIC: &str = "anthropic";
const MODEL: &str = "claude-sonnet-4-6";

fn request(context: systemprompt_models::RequestContext) -> AiRequest {
    AiRequest::builder(vec![AiMessage::user("hi")], ANTHROPIC, MODEL, 128, context).build()
}

async fn failed_request_count(pool: &DbPool, user_id: &UserId) -> i64 {
    sqlx::query_scalar!(
        "SELECT COUNT(*) FROM ai_requests WHERE user_id = $1 AND status = 'failed'",
        user_id.as_str()
    )
    .fetch_one(pool.pool_arc().expect("read pool").as_ref())
    .await
    .expect("count failed requests")
    .unwrap_or(0)
}

#[tokio::test]
async fn a_failed_tooled_request_is_audited_as_failed_with_its_error_message() {
    let Some(pool) = pool().await else {
        return;
    };
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
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert!(
        stored.is_some_and(|m| !m.is_empty()),
        "the audit row must carry why the request failed"
    );
}

#[tokio::test]
async fn a_failed_single_turn_request_is_audited_as_failed() {
    let Some(pool) = pool().await else {
        return;
    };
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
    let Some(pool) = pool().await else {
        return;
    };
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

#[tokio::test]
async fn a_disabled_provider_entry_is_not_built() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let registry = registry_with_endpoint(ANTHROPIC, server.uri());

    let mut config = ai_config(ANTHROPIC);
    config.providers.insert(
        "openai".to_owned(),
        AiProviderConfig {
            enabled: false,
            ..AiProviderConfig::default()
        },
    );

    let svc = AiService::new(
        &pool,
        &registry,
        &config,
        Arc::new(NoopToolProvider::new()),
        noop_session_provider(),
    )
    .expect("a disabled entry must not stop the service building");

    let health = svc.health_check().await.expect("health check");
    assert!(
        !health.contains_key("provider_openai"),
        "an `enabled: false` policy entry must not produce a provider, got {health:?}"
    );
    assert!(health.contains_key("provider_anthropic"));
}

#[tokio::test]
async fn an_enabled_provider_with_no_registry_entry_is_skipped_rather_than_fatal() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;

    // A registry that knows only anthropic, against a policy that also enables
    // a provider the registry has never heard of.
    let mut registry = registry_with_endpoint(ANTHROPIC, server.uri());
    registry.providers.retain(|p| p.name.as_str() == ANTHROPIC);

    let mut config = ai_config(ANTHROPIC);
    config.providers.insert(
        "phantom-provider".to_owned(),
        AiProviderConfig {
            enabled: true,
            ..AiProviderConfig::default()
        },
    );

    let svc = AiService::new(
        &pool,
        &registry,
        &config,
        Arc::new(NoopToolProvider::new()),
        noop_session_provider(),
    )
    .expect("an unknown provider name must be skipped, not abort construction");

    let health = svc.health_check().await.expect("health check");
    assert!(
        !health.contains_key("provider_phantom-provider"),
        "a policy entry with no connectivity entry must yield no provider, got {health:?}"
    );
}

#[tokio::test]
async fn a_default_provider_that_was_never_built_fails_construction() {
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("x")).await;
    let registry = registry_with_endpoint(ANTHROPIC, server.uri());

    // No policy entries at all: validation stops construction before the
    // default-provider lookup is even reached.
    let empty = AiConfig {
        default_provider: "phantom-provider".to_owned(),
        default_max_output_tokens: Some(512),
        providers: std::collections::HashMap::new(),
        ..AiConfig::default()
    };
    let err = AiService::new(
        &pool,
        &registry,
        &empty,
        Arc::new(NoopToolProvider::new()),
        noop_session_provider(),
    )
    .expect_err("a service with no enabled provider cannot serve anything");
    assert!(
        err.to_string().contains("No AI providers are enabled"),
        "got {err}"
    );

    // One provider enabled, but the default names a different one — this is the
    // arm that reports an unresolvable default by name.
    let mut mismatched = ai_config(ANTHROPIC);
    mismatched.default_provider = "phantom-provider".to_owned();
    let err = AiService::new(
        &pool,
        &registry,
        &mismatched,
        Arc::new(NoopToolProvider::new()),
        noop_session_provider(),
    )
    .expect_err("a default naming an unbuilt provider must fail construction");
    assert!(
        err.to_string().contains("phantom-provider"),
        "the failure must name the unresolvable default, got {err}"
    );
}

#[tokio::test]
async fn the_default_registry_seed_is_usable_without_endpoint_overrides() {
    let seed = ProviderRegistry::default_seed().expect("the embedded catalog must parse");
    assert!(
        seed.find_provider(ANTHROPIC).is_some(),
        "the shipped catalog must know the provider the default policy names"
    );
    assert!(
        seed.find_provider("phantom-provider").is_none(),
        "lookup must be exact, not fuzzy"
    );
}

#[tokio::test]
async fn a_failed_planning_request_is_audited_as_failed() {
    let Some(pool) = pool().await else {
        return;
    };
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
    let Some(pool) = pool().await else {
        return;
    };
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
    let Some(pool) = pool().await else {
        return;
    };
    let server =
        mock_http::anthropic_messages_success(mock_http::anthropic_response_body("priced")).await;
    let svc = service(&pool, ANTHROPIC, server.uri());
    let (user, context) = seeded_context(&pool).await;

    svc.generate(&request(context)).await.expect("generate");

    let cost: i64 = sqlx::query_scalar!(
        "SELECT cost_microdollars FROM ai_requests WHERE user_id = $1",
        user.as_str()
    )
    .fetch_one(pool.pool_arc().unwrap().as_ref())
    .await
    .unwrap();
    assert!(
        cost >= 0,
        "the audit row must carry a cost derived from the provider's pricing table, got {cost}"
    );
}
