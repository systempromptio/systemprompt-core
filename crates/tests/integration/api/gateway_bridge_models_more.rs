//! Integration tests (coverage campaign 2026-07).
//!
//! Covers the bridge/model surface of the gateway router plus the pure helpers
//! reached through the `services::gateway` seams: `canonicalize_org_uuid`, the
//! `x-inference-protocol` surface parser, model-id humanisation, the outbound
//! and safety-scanner registries, canonical-message flattening, the
//! `GatewayAudit` mutators, and the bridge credential-exchange handlers.

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode, header};
use systemprompt_api::routes::gateway::bridge::canonicalize_org_uuid;
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_api::routes::gateway::models::{humanize_model_id, surfaces_from_header};
use systemprompt_api::services::gateway::GatewayRequestContext;
use systemprompt_api::services::gateway::audit::GatewayAudit;
use systemprompt_api::services::gateway::audit::test_api::flatten_message_content;
use systemprompt_api::services::gateway::protocol::{CanonicalContent, ImageSource};
use systemprompt_api::services::gateway::registry::{
    GatewayUpstreamRegistry, SafetyScannerRegistry,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::INFERENCE_PROTOCOL;
use systemprompt_identifiers::{
    AiRequestId, ContextId, GatewayConversationId, TenantId, TraceId, UserId,
};
use systemprompt_models::profile::ApiSurface;
use systemprompt_test_fixtures::{install_test_signing_key, seed_admin_credential};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router_and_pool() -> Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let router = gateway_router(&ctx).expect("gateway router available");
    Ok((router, pool))
}

fn header_map(pairs: &[(&str, &str)]) -> HeaderMap {
    use axum::http::HeaderName;
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        let name = HeaderName::from_bytes(k.as_bytes()).expect("header name");
        h.insert(name, HeaderValue::from_str(v).expect("header value"));
    }
    h
}

#[test]
fn canonicalize_org_uuid_strips_local_prefix_of_valid_uuid() {
    let raw = "550e8400-e29b-41d4-a716-446655440000";
    let tenant = TenantId::new(format!("local_{raw}"));
    assert_eq!(canonicalize_org_uuid(&tenant), raw);
}

#[test]
fn canonicalize_org_uuid_passes_through_bare_uuid() {
    let raw = "550e8400-e29b-41d4-a716-446655440000";
    let tenant = TenantId::new(raw.to_owned());
    assert_eq!(canonicalize_org_uuid(&tenant), raw);
}

#[test]
fn canonicalize_org_uuid_hashes_non_uuid_deterministically() {
    let tenant = TenantId::new("acme-corp".to_owned());
    let a = canonicalize_org_uuid(&tenant);
    let b = canonicalize_org_uuid(&tenant);
    assert_eq!(a, b, "hash is stable");
    assert!(uuid::Uuid::parse_str(&a).is_ok(), "produces a valid uuid");
}

#[test]
fn surfaces_from_header_absent_is_empty() {
    let surfaces = surfaces_from_header(&HeaderMap::new()).expect("ok");
    assert!(surfaces.is_empty());
}

#[test]
fn surfaces_from_header_parses_known_tags() {
    let h = header_map(&[(INFERENCE_PROTOCOL, "anthropic")]);
    let surfaces = surfaces_from_header(&h).expect("ok");
    assert_eq!(surfaces, vec![ApiSurface::Anthropic]);
}

#[test]
fn surfaces_from_header_rejects_unknown_tag() {
    let h = header_map(&[(INFERENCE_PROTOCOL, "quantum")]);
    let (status, msg) = surfaces_from_header(&h).expect_err("unknown must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(msg.contains("unknown"), "{msg}");
}

#[test]
fn surfaces_from_header_rejects_backend_surface() {
    let h = header_map(&[(INFERENCE_PROTOCOL, "backend")]);
    let (status, _msg) = surfaces_from_header(&h).expect_err("backend is not a client surface");
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[test]
fn humanize_model_id_title_cases_hyphen_segments() {
    assert_eq!(humanize_model_id("claude-sonnet-4"), "Claude Sonnet 4");
    assert_eq!(humanize_model_id("gpt-5-mini"), "Gpt 5 Mini");
    assert_eq!(humanize_model_id(""), "");
}

#[test]
fn flatten_message_content_covers_every_arm() {
    let parts = vec![
        CanonicalContent::Text("first".to_owned()),
        CanonicalContent::Thinking {
            text: "pondering".to_owned(),
            signature: None,
        },
        CanonicalContent::ToolUse {
            id: "call-1".to_owned(),
            name: "search".to_owned(),
            input: serde_json::json!({"q": "rust"}),
            signature: None,
        },
        CanonicalContent::ToolResult {
            tool_use_id: "call-1".to_owned(),
            content: vec![CanonicalContent::Text("nested result".to_owned())],
            is_error: false,
            structured_content: None,
            meta: None,
        },
        CanonicalContent::Image(ImageSource::Url {
            url: "https://example.invalid/x.png".to_owned(),
            detail: None,
        }),
        CanonicalContent::Text(String::new()),
    ];
    let flat = flatten_message_content(&parts);
    assert!(flat.contains("first"));
    assert!(flat.contains("pondering"));
    assert!(flat.contains("[tool_use:search"));
    assert!(flat.contains("nested result"));
    assert!(!flat.ends_with('\n'), "empty text does not append a blank line");
}

#[test]
fn flatten_message_content_empty_is_empty_string() {
    assert_eq!(flatten_message_content(&[]), "");
}

#[test]
fn upstream_registry_serves_builtin_wire_adapters() {
    let registry = GatewayUpstreamRegistry::global();
    assert!(registry.get("anthropic").is_some());
    assert!(registry.get("openai-chat").is_some());
    assert!(registry.get("nonexistent-wire").is_none());
    assert!(registry.tags().contains(&"anthropic"));
}

#[test]
fn safety_scanner_registry_has_heuristic_scanner() {
    let registry = SafetyScannerRegistry::global();
    assert!(registry.get("heuristic").is_some());
    assert!(registry.get("missing-scanner").is_none());
    assert!(registry.names().contains(&"heuristic"));
}

fn gateway_ctx(id: &AiRequestId, user: &UserId, upstream_model: &str) -> GatewayRequestContext {
    GatewayRequestContext {
        ai_request_id: id.clone(),
        user_id: user.clone(),
        session_id: None,
        context_id: ContextId::generate(),
        gateway_conversation_id: Some(
            GatewayConversationId::try_new("ctx_00112233aabbccdd".to_owned()).expect("id"),
        ),
        trace_id: Some(TraceId::generate()),
        provider: "anthropic".to_owned(),
        requested_model: Some("claude-test".to_owned()),
        model: upstream_model.to_owned(),
        max_tokens: Some(64),
        is_streaming: false,
        wire_protocol: "anthropic-messages".to_owned(),
    }
}

async fn seed_ai_request(pool: &DbPool, id: &AiRequestId, user: &UserId, model: &str) -> Result<()> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    sqlx::query(
        "INSERT INTO ai_requests (id, request_id, user_id, provider, model, cost_microdollars, \
         cache_hit, is_streaming, status, actor_kind, actor_id) VALUES ($1, $1, $2, 'anthropic', \
         $3, 0, false, false, 'pending', 'user', $2)",
    )
    .bind(id.as_str())
    .bind(user.as_str())
    .bind(model)
    .execute(pg.as_ref())
    .await?;
    Ok(())
}

async fn model_column(pool: &DbPool, id: &AiRequestId) -> Result<String> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    let row: (String,) = sqlx::query_as("SELECT model FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_one(pg.as_ref())
        .await?;
    Ok(row.0)
}

async fn system_prompt_override_column(pool: &DbPool, id: &AiRequestId) -> Result<Option<String>> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    let row: (Option<String>,) =
        sqlx::query_as("SELECT system_prompt_override FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pg.as_ref())
            .await?;
    Ok(row.0)
}

async fn route_match_column(pool: &DbPool, id: &AiRequestId) -> Result<Option<String>> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    let row: (Option<String>,) =
        sqlx::query_as("SELECT route_match FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_one(pg.as_ref())
            .await?;
    Ok(row.0)
}

#[tokio::test]
async fn gateway_audit_mutators_write_their_columns() -> Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "audit-mut@example.invalid").await?;
    let id = AiRequestId::generate();
    seed_ai_request(&pool, &id, &cred.user_id, "upstream-model").await?;

    let audit = GatewayAudit::new(&pool, gateway_ctx(&id, &cred.user_id, "upstream-model"))
        .expect("audit opens");

    audit.set_served_model("served-model").await;
    assert_eq!(model_column(&pool, &id).await?, "served-model");

    audit.set_system_prompt_override("override-desc").await;
    assert_eq!(
        system_prompt_override_column(&pool, &id).await?,
        Some("override-desc".to_owned())
    );

    audit.set_route_match("when:tier").await;
    assert_eq!(
        route_match_column(&pool, &id).await?,
        Some("when:tier".to_owned())
    );

    audit.fail("boom").await.expect("fail records");
    Ok(())
}

#[tokio::test]
async fn gateway_audit_set_served_model_noop_when_same() -> Result<()> {
    let (pool, _ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "audit-noop@example.invalid").await?;
    let id = AiRequestId::generate();
    seed_ai_request(&pool, &id, &cred.user_id, "same-model").await?;
    let audit =
        GatewayAudit::new(&pool, gateway_ctx(&id, &cred.user_id, "same-model")).expect("audit");
    audit.set_served_model("same-model").await;
    audit.set_served_model("").await;
    assert_eq!(model_column(&pool, &id).await?, "same-model");
    Ok(())
}

fn authed_post(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}

#[tokio::test]
async fn bridge_pat_with_bad_api_key_is_unauthorized() -> Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(authed_post(
            "/auth/bridge/pat",
            "sp-live-not-a-real-key",
            serde_json::json!({}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn bridge_session_pat_bad_code_is_unauthorized() -> Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/session-pat",
            serde_json::json!({ "code": "definitely-not-a-code" }),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn bridge_mtls_missing_fingerprint_is_bad_request() -> Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/mtls",
            serde_json::json!({ "device_cert_fingerprint": "" }),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn bridge_mtls_unenrolled_fingerprint_is_unauthorized() -> Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/mtls",
            serde_json::json!({
                "device_cert_fingerprint":
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            }),
        ))
        .await?;
    assert!(
        resp.status().is_client_error(),
        "unenrolled fingerprint must be a client error, got {}",
        resp.status()
    );
    Ok(())
}
