//! Gateway failover keeps the governed request intact while rebinding it to a
//! healthy provider after an upstream failure.

use axum::body::to_bytes;
use systemprompt_api::services::gateway::protocol::outbound::UpstreamError;
use systemprompt_api::services::gateway::service::{DispatchError, GatewayService};
use systemprompt_identifiers::ProviderId;
use systemprompt_models::services::{ApiSurface, WireProtocol};
use systemprompt_test_fixtures::seed_admin_credential;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::setup_ctx;
use super::gateway_pipeline::{
    MODEL, PROVIDER, canonical_request, gateway_config, gw_repos, inputs, install_provider_api_key,
    provider_registry,
};

fn upstream_response(text: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "msg_failover",
        "type": "message",
        "role": "assistant",
        "model": MODEL,
        "content": [{"type": "text", "text": text}],
        "stop_reason": "end_turn",
        "usage": {"input_tokens": 3, "output_tokens": 2}
    })
}

fn assert_recorded_status(error: DispatchError, status: u16) {
    let DispatchError::Recorded(error) = error else {
        panic!("upstream failures must be recorded before returning");
    };
    assert!(
        matches!(
            error.downcast_ref::<UpstreamError>(),
            Some(UpstreamError::Status { status: actual, .. }) if *actual == status
        ),
        "expected recorded upstream status {status}, got {error:#}"
    );
}

#[tokio::test]
async fn gateway_rebinds_one_governed_request_to_its_fallback_after_primary_500()
-> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "failover@example.invalid").await?;
    let primary = MockServer::start().await;
    let fallback = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("primary unavailable"))
        .mount(&primary)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(upstream_response("fallback served")),
        )
        .mount(&fallback)
        .await;

    let mut registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut fallback_provider = registry.providers[0].clone();
    fallback_provider.name = ProviderId::new("anthropic-fallback");
    fallback_provider.endpoint = fallback.uri();
    registry.providers.push(fallback_provider);
    let mut config = gateway_config(PROVIDER);
    config.routes[0].fallback_provider = Some(ProviderId::new("anthropic-fallback"));

    let dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = dispatch.ctx.ai_request_id.clone();
    let response = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), dispatch)
        .await
        .expect("fallback response succeeds");
    assert_eq!(response.status(), http::StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("fallback served"));

    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("primary request history")
            .len(),
        1,
        "the primary gets its one bounded 5xx attempt"
    );
    assert_eq!(
        fallback
            .received_requests()
            .await
            .expect("fallback request history")
            .len(),
        1,
        "the fallback receives the re-bound request once"
    );
    let database = pool.pool_arc().expect("read pool");
    let served: Option<String> =
        sqlx::query_scalar("SELECT served_provider FROM ai_requests WHERE id = $1")
            .bind(request_id.as_str())
            .fetch_optional(database.as_ref())
            .await?;
    assert_eq!(served.as_deref(), Some("anthropic-fallback"));
    Ok(())
}

#[tokio::test]
async fn gateway_does_not_send_client_errors_to_a_configured_fallback() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "failover-client-error@example.invalid").await?;
    let primary = MockServer::start().await;
    let fallback = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(400).set_body_string("invalid request"))
        .mount(&primary)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response("must not run")))
        .mount(&fallback)
        .await;
    let mut registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut fallback_provider = registry.providers[0].clone();
    fallback_provider.name = ProviderId::new("anthropic-client-error-fallback");
    fallback_provider.endpoint = fallback.uri();
    registry.providers.push(fallback_provider);
    let mut config = gateway_config(PROVIDER);
    config.routes[0].fallback_provider = Some(ProviderId::new("anthropic-client-error-fallback"));

    let error = GatewayService::dispatch(
        &config,
        &registry,
        &pool,
        &gw_repos(&pool),
        inputs(&credential, canonical_request(MODEL, false), false),
    )
    .await
    .expect_err("a primary client error is returned directly");
    assert_recorded_status(error, 400);
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("primary request history")
            .len(),
        1
    );
    assert!(
        fallback
            .received_requests()
            .await
            .expect("fallback request history")
            .is_empty(),
        "the fallback cannot turn a client error into a second upstream call"
    );
    Ok(())
}

#[tokio::test]
async fn gateway_preserves_primary_failure_when_fallback_provider_cannot_be_bound()
-> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "failover-unbound@example.invalid").await?;
    let primary = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("primary unavailable"))
        .mount(&primary)
        .await;
    let registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut config = gateway_config(PROVIDER);
    config.routes[0].fallback_provider = Some(ProviderId::new("missing-fallback-provider"));

    let error = GatewayService::dispatch(
        &config,
        &registry,
        &pool,
        &gw_repos(&pool),
        inputs(&credential, canonical_request(MODEL, false), false),
    )
    .await
    .expect_err("an unavailable fallback cannot replace the primary verdict");
    assert_recorded_status(error, 500);
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("primary request history")
            .len(),
        1
    );
    Ok(())
}

#[tokio::test]
async fn gateway_records_the_fallback_when_both_upstreams_fail() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "failover-both-fail@example.invalid").await?;
    let primary = MockServer::start().await;
    let fallback = MockServer::start().await;
    for server in [&primary, &fallback] {
        Mock::given(method("POST"))
            .and(path("/messages"))
            .respond_with(ResponseTemplate::new(500).set_body_string("unavailable"))
            .mount(server)
            .await;
    }
    let mut registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut fallback_provider = registry.providers[0].clone();
    fallback_provider.name = ProviderId::new("anthropic-both-fail-fallback");
    fallback_provider.endpoint = fallback.uri();
    registry.providers.push(fallback_provider);
    let mut config = gateway_config(PROVIDER);
    config.routes[0].fallback_provider = Some(ProviderId::new("anthropic-both-fail-fallback"));

    let dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = dispatch.ctx.ai_request_id.clone();
    let error = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), dispatch)
        .await
        .expect_err("the fallback error is returned when both providers fail");
    assert_recorded_status(error, 500);
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("primary request history")
            .len(),
        1
    );
    assert_eq!(
        fallback
            .received_requests()
            .await
            .expect("fallback request history")
            .len(),
        1
    );
    let database = pool.pool_arc().expect("read pool");
    let served: Option<String> =
        sqlx::query_scalar("SELECT served_provider FROM ai_requests WHERE id = $1")
            .bind(request_id.as_str())
            .fetch_optional(database.as_ref())
            .await?;
    assert_eq!(served.as_deref(), Some("anthropic-both-fail-fallback"));
    Ok(())
}

#[tokio::test]
async fn gateway_without_a_fallback_records_primary_failure_once() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "primary-only-failure@example.invalid").await?;
    let primary = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(503).set_body_string("maintenance"))
        .mount(&primary)
        .await;
    let registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let config = gateway_config(PROVIDER);
    assert!(config.routes[0].fallback_provider.is_none());
    let dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = dispatch.ctx.ai_request_id.clone();

    let error = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), dispatch)
        .await
        .expect_err("a primary-only route returns its upstream failure");
    assert_recorded_status(error, 503);
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("recorded primary requests")
            .len(),
        4,
        "the primary uses its configured bounded retry budget"
    );

    let database = pool.pool_arc().expect("read pool");
    let row: (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM ai_requests WHERE id = $1")
            .bind(request_id.as_str())
            .fetch_one(database.as_ref())
            .await?;
    assert_eq!(row.0, "failed");
    assert!(row.1.as_deref().is_some_and(|error| error.contains("503")));
    Ok(())
}

#[tokio::test]
async fn gateway_without_a_fallback_returns_and_attributes_primary_success() -> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "primary-only-success@example.invalid").await?;
    let primary = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response("primary served")))
        .mount(&primary)
        .await;
    let registry = provider_registry(
        &primary.uri(),
        PROVIDER,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let config = gateway_config(PROVIDER);
    assert!(config.routes[0].fallback_provider.is_none());
    let dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = dispatch.ctx.ai_request_id.clone();

    let response = GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), dispatch)
        .await
        .expect("primary-only response succeeds");
    assert_eq!(response.status(), http::StatusCode::OK);
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("primary served"));
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("recorded primary requests")
            .len(),
        1
    );

    let database = pool.pool_arc().expect("read pool");
    let identity: (String, Option<String>) =
        sqlx::query_as("SELECT provider, served_provider FROM ai_requests WHERE id = $1")
            .bind(request_id.as_str())
            .fetch_one(database.as_ref())
            .await?;
    assert_eq!(identity.0, PROVIDER);
    assert_eq!(
        identity.1, None,
        "served_provider records an override only when failover rebinds the request"
    );
    Ok(())
}

#[tokio::test]
async fn open_primary_circuit_routes_directly_to_fallback_and_records_the_served_provider()
-> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "primary-circuit@example.invalid").await?;
    let primary = MockServer::start().await;
    let fallback = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("primary unavailable"))
        .mount(&primary)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response("fallback")))
        .mount(&fallback)
        .await;
    let primary_name = format!("primary-circuit-{}", uuid::Uuid::new_v4().simple());
    let fallback_name = format!("fallback-circuit-{}", uuid::Uuid::new_v4().simple());
    let mut registry = provider_registry(
        &primary.uri(),
        &primary_name,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut fallback_provider = registry.providers[0].clone();
    fallback_provider.name = ProviderId::new(&fallback_name);
    fallback_provider.endpoint = fallback.uri();
    registry.providers.push(fallback_provider);
    let mut config = gateway_config(&primary_name);
    config.routes[0].fallback_provider = Some(ProviderId::new(&fallback_name));

    for _ in 0..5 {
        let response = GatewayService::dispatch(
            &config,
            &registry,
            &pool,
            &gw_repos(&pool),
            inputs(&credential, canonical_request(MODEL, false), false),
        )
        .await?;
        to_bytes(response.into_body(), 1024 * 1024).await?;
    }
    let primary_before = primary
        .received_requests()
        .await
        .expect("primary history")
        .len();
    let direct = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = direct.ctx.ai_request_id.clone();
    let response =
        GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), direct).await?;
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("fallback"));
    assert_eq!(
        primary
            .received_requests()
            .await
            .expect("primary history")
            .len(),
        primary_before,
        "an open primary circuit is not probed before its recovery interval"
    );
    assert_eq!(
        fallback
            .received_requests()
            .await
            .expect("fallback history")
            .len(),
        6
    );
    let database = pool.pool_arc().expect("read pool");
    let served: Option<String> =
        sqlx::query_scalar("SELECT served_provider FROM ai_requests WHERE id=$1")
            .bind(request_id.as_str())
            .fetch_one(database.as_ref())
            .await?;
    assert_eq!(served.as_deref(), Some(fallback_name.as_str()));
    Ok(())
}

#[tokio::test]
async fn open_fallback_circuit_keeps_a_healthy_primary_on_the_primary_only_path()
-> anyhow::Result<()> {
    install_provider_api_key();
    let (pool, _) = setup_ctx().await?;
    let credential = seed_admin_credential(&pool, "fallback-circuit@example.invalid").await?;
    let failing = MockServer::start().await;
    let training_fallback = MockServer::start().await;
    let healthy_primary = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("fallback unavailable"))
        .mount(&failing)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response("training")))
        .mount(&training_fallback)
        .await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(upstream_response("primary kept")))
        .mount(&healthy_primary)
        .await;
    let failing_name = format!("open-fallback-{}", uuid::Uuid::new_v4().simple());
    let training_name = format!("training-fallback-{}", uuid::Uuid::new_v4().simple());
    let mut training_registry = provider_registry(
        &failing.uri(),
        &failing_name,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut training_provider = training_registry.providers[0].clone();
    training_provider.name = ProviderId::new(&training_name);
    training_provider.endpoint = training_fallback.uri();
    training_registry.providers.push(training_provider);
    let mut training_config = gateway_config(&failing_name);
    training_config.routes[0].fallback_provider = Some(ProviderId::new(&training_name));
    for _ in 0..5 {
        let response = GatewayService::dispatch(
            &training_config,
            &training_registry,
            &pool,
            &gw_repos(&pool),
            inputs(&credential, canonical_request(MODEL, false), false),
        )
        .await?;
        to_bytes(response.into_body(), 1024 * 1024).await?;
    }
    assert!(
        !failing
            .received_requests()
            .await
            .expect("training primary history")
            .is_empty(),
        "the provider whose breaker is reused was exercised as the training primary"
    );

    let healthy_name = format!("healthy-primary-{}", uuid::Uuid::new_v4().simple());
    let mut registry = provider_registry(
        &healthy_primary.uri(),
        &healthy_name,
        WireProtocol::Anthropic,
        ApiSurface::Anthropic,
    );
    let mut open_fallback = registry.providers[0].clone();
    open_fallback.name = ProviderId::new(&failing_name);
    open_fallback.endpoint = failing.uri();
    registry.providers.push(open_fallback);
    let mut config = gateway_config(&healthy_name);
    config.routes[0].fallback_provider = Some(ProviderId::new(&failing_name));
    let failing_before = failing
        .received_requests()
        .await
        .expect("failure history")
        .len();
    let dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let request_id = dispatch.ctx.ai_request_id.clone();
    let response =
        GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), dispatch).await?;
    let body = to_bytes(response.into_body(), 1024 * 1024).await?;
    assert!(String::from_utf8_lossy(&body).contains("primary kept"));
    assert_eq!(
        failing
            .received_requests()
            .await
            .expect("failure history")
            .len(),
        failing_before,
        "an open fallback circuit is not contacted after a healthy primary response"
    );
    let database = pool.pool_arc().expect("read pool");
    let served: Option<String> =
        sqlx::query_scalar("SELECT served_provider FROM ai_requests WHERE id=$1")
            .bind(request_id.as_str())
            .fetch_one(database.as_ref())
            .await?;
    assert!(
        served.is_none(),
        "primary-only routing keeps provider attribution"
    );

    healthy_primary.reset().await;
    Mock::given(method("POST"))
        .and(path("/messages"))
        .respond_with(ResponseTemplate::new(500).set_body_string("healthy primary now failed"))
        .mount(&healthy_primary)
        .await;
    let failed_dispatch = inputs(&credential, canonical_request(MODEL, false), false);
    let failed_id = failed_dispatch.ctx.ai_request_id.clone();
    let error =
        GatewayService::dispatch(&config, &registry, &pool, &gw_repos(&pool), failed_dispatch)
            .await
            .expect_err("an open fallback cannot rescue a later primary failure");
    assert_recorded_status(error, 500);
    assert_eq!(
        failing
            .received_requests()
            .await
            .expect("failure history")
            .len(),
        failing_before,
        "the already-open fallback remains skipped when primary fails"
    );
    let failed: (String, Option<String>) =
        sqlx::query_as("SELECT status, error_message FROM ai_requests WHERE id=$1")
            .bind(failed_id.as_str())
            .fetch_one(database.as_ref())
            .await?;
    assert_eq!(failed.0, "failed");
    assert!(
        failed
            .1
            .as_deref()
            .is_some_and(|message| message.contains("500"))
    );
    Ok(())
}
