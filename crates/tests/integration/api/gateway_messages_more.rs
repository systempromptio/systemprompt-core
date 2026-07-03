//! Integration tests (coverage campaign 2026-07).
//!
//! Drives the gateway message-extraction seams re-exported through
//! `routes::gateway::messages::test_api`: the header parsers, canonical-body
//! reader, conversation-id derivation, pre-dispatch authz enforcement, the
//! `authenticate` credential dispatcher (API-key + JWT), and the rejection
//! audit-record builder / persister — none of which are reachable through the
//! router because the fixture profile leaves the gateway unmounted.

use std::sync::Arc;

use anyhow::Result;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use systemprompt_api::routes::gateway::messages::test_api::{
    ApiKeyPrincipal, AuthedPrincipal, RejectionPartial, authenticate, build_rejection_record,
    derive_conversation, enforce_authz_pre_dispatch, optional_gateway_conversation_id,
    persist_rejection, read_gateway_body, require_session_id,
};
use systemprompt_api::services::gateway::protocol::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_api::services::gateway::protocol::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, InboundAdapter, Role,
};
use systemprompt_database::DbPool;
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_identifiers::{AiRequestId, TraceId, UserId};
use systemprompt_security::authz::{AllowAllHook, DenyAllHook, SharedAuthzHook};
use systemprompt_test_fixtures::{install_test_signing_key, seed_admin_credential};
use systemprompt_users::{ApiKeyService, IssueApiKeyParams};

use super::common::setup_ctx;

fn header_map(pairs: &[(&str, &str)]) -> HeaderMap {
    use axum::http::HeaderName;
    let mut h = HeaderMap::new();
    for (k, v) in pairs {
        let name = HeaderName::from_bytes(k.as_bytes()).expect("header name");
        h.insert(name, HeaderValue::from_str(v).expect("header value"));
    }
    h
}

fn inbound() -> Arc<dyn InboundAdapter> {
    Arc::new(AnthropicMessagesInbound)
}

fn canonical(messages: Vec<CanonicalMessage>) -> CanonicalRequest {
    CanonicalRequest {
        model: "claude-test".to_owned(),
        system: None,
        messages,
        max_tokens: 128,
        temperature: None,
        top_p: None,
        top_k: None,
        stop_sequences: Vec::new(),
        tools: Vec::new(),
        tool_choice: None,
        stream: false,
        thinking: None,
        metadata: None,
        response_format: None,
        reasoning_effort: None,
        search: None,
        code_execution: false,
        presence_penalty: None,
        frequency_penalty: None,
    }
}

fn user_message(text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::Text(text.to_owned())],
    }
}

#[test]
fn require_session_id_reads_present_header() {
    let h = header_map(&[(SESSION_ID, "sess-123")]);
    let id = require_session_id(&h).expect("session id parsed");
    assert_eq!(id.as_str(), "sess-123");
}

#[test]
fn require_session_id_trims_surrounding_whitespace() {
    let h = header_map(&[(SESSION_ID, "  sess-trim  ")]);
    let id = require_session_id(&h).expect("session id parsed");
    assert_eq!(id.as_str(), "sess-trim");
}

#[test]
fn require_session_id_missing_header_is_bad_request() {
    let (status, msg) = require_session_id(&HeaderMap::new()).expect_err("missing must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(msg.contains("missing"), "{msg}");
}

#[test]
fn require_session_id_empty_header_is_bad_request() {
    let h = header_map(&[(SESSION_ID, "   ")]);
    let (status, msg) = require_session_id(&h).expect_err("empty must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(msg.contains("empty"), "{msg}");
}

#[test]
fn optional_conversation_id_absent_is_none() {
    let result = optional_gateway_conversation_id(&HeaderMap::new()).expect("ok");
    assert!(result.is_none());
}

#[test]
fn optional_conversation_id_blank_is_none() {
    let h = header_map(&[(GATEWAY_CONVERSATION_ID, "   ")]);
    let result = optional_gateway_conversation_id(&h).expect("ok");
    assert!(result.is_none());
}

#[test]
fn optional_conversation_id_present_is_some() {
    let h = header_map(&[(GATEWAY_CONVERSATION_ID, "ctx_0123456789abcdef")]);
    let result = optional_gateway_conversation_id(&h).expect("ok");
    assert_eq!(result.expect("some").as_str(), "ctx_0123456789abcdef");
}

#[tokio::test]
async fn read_gateway_body_parses_canonical_and_populates_partial() -> Result<()> {
    let inbound = inbound();
    let body = serde_json::json!({
        "model": "claude-test",
        "max_tokens": 64,
        "messages": [{"role": "user", "content": "hi"}],
    })
    .to_string();
    let request = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .body(Body::from(body))
        .expect("request");
    let mut partial = RejectionPartial::default();
    let (bytes, canonical) = read_gateway_body(&inbound, request, &mut partial)
        .await
        .expect("body parses");
    assert!(!bytes.is_empty());
    assert_eq!(canonical.model, "claude-test");
    assert_eq!(partial.model.as_deref(), Some("claude-test"));
    assert_eq!(partial.max_tokens, Some(64));
    assert!(partial.body.is_some());
    Ok(())
}

#[tokio::test]
async fn read_gateway_body_rejects_unparseable_json() -> Result<()> {
    let inbound = inbound();
    let request = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .body(Body::from("not json at all"))
        .expect("request");
    let mut partial = RejectionPartial::default();
    let (status, _msg) = read_gateway_body(&inbound, request, &mut partial)
        .await
        .expect_err("garbage must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(partial.body.is_some(), "raw body captured before parse");
    Ok(())
}

#[test]
fn derive_conversation_prefers_header_value() {
    use systemprompt_identifiers::GatewayConversationId;
    let header = GatewayConversationId::try_new("ctx_00000000deadbeef".to_owned()).expect("id");
    let request = canonical(vec![user_message("hello")]);
    let mut partial = RejectionPartial::default();
    let (conv, ctx) =
        derive_conversation(Some(header), &request, &mut partial).expect("derived ok");
    assert_eq!(conv.as_str(), "ctx_00000000deadbeef");
    assert_eq!(
        partial.gateway_conversation_id.as_ref().expect("set"),
        &conv
    );
    assert_eq!(partial.context_id.as_ref().expect("set"), &ctx);
}

#[test]
fn derive_conversation_derives_from_messages_when_header_absent() {
    let request = canonical(vec![user_message("derive me")]);
    let mut partial = RejectionPartial::default();
    let (conv, _ctx) = derive_conversation(None, &request, &mut partial).expect("derived ok");
    assert!(!conv.as_str().is_empty());
}

#[test]
fn derive_conversation_without_messages_is_bad_request() {
    let request = canonical(vec![]);
    let mut partial = RejectionPartial::default();
    let (status, msg) =
        derive_conversation(None, &request, &mut partial).expect_err("no messages must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(msg.contains("cannot derive"), "{msg}");
}

fn api_key_principal(user: &str) -> AuthedPrincipal {
    AuthedPrincipal::ApiKey(ApiKeyPrincipal {
        user_id: UserId::new(user),
        trace_id: TraceId::generate(),
    })
}

fn gateway_route() -> systemprompt_models::profile::GatewayRoute {
    let mut route = systemprompt_models::profile::GatewayRoute {
        id: systemprompt_identifiers::RouteId::new(""),
        model_pattern: "claude-*".to_owned(),
        provider: systemprompt_identifiers::ProviderId::new("anthropic"),
        upstream_model: None,
        extra_headers: std::collections::HashMap::new(),
        pricing: None,
        when: None,
    };
    route.ensure_id();
    route
}

#[tokio::test]
async fn enforce_authz_allows_under_allow_all_hook() {
    let hook: SharedAuthzHook = Arc::new(AllowAllHook::null());
    let route = gateway_route();
    let principal = api_key_principal("authz-allow-user");
    enforce_authz_pre_dispatch(
        &principal,
        &route,
        "claude-test",
        &hook,
    )
    .await
    .expect("allow hook permits");
}

#[tokio::test]
async fn enforce_authz_denies_under_deny_all_hook() {
    let hook: SharedAuthzHook = Arc::new(DenyAllHook::null());
    let route = gateway_route();
    let principal = api_key_principal("authz-deny-user");
    let (status, msg) = enforce_authz_pre_dispatch(
        &principal,
        &route,
        "claude-test",
        &hook,
    )
    .await
    .expect_err("deny hook rejects");
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(msg.contains("authz denied"), "{msg}");
}

fn jwt_extractor(
    ctx: &systemprompt_runtime::AppContext,
) -> Result<systemprompt_api::services::middleware::JwtContextExtractor> {
    use systemprompt_api::services::middleware::{JtiRevocationChecker, JwtContextExtractor};
    use systemprompt_traits::{AnalyticsProvider, UserProvider};
    let concrete = Arc::clone(ctx.analytics_service());
    let analytics: Arc<dyn AnalyticsProvider> = concrete;
    let user_provider: Arc<dyn UserProvider> =
        Arc::new(systemprompt_users::UserService::new(ctx.db_pool())?);
    let jti = JtiRevocationChecker::from_pool(ctx.db_pool())?;
    Ok(JwtContextExtractor::new(analytics, user_provider, jti))
}

#[tokio::test]
async fn authenticate_accepts_seeded_api_key() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "auth-apikey@example.invalid").await?;
    let service = ApiKeyService::new(ctx.db_pool())?;
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &cred.user_id,
            name: "gateway-auth-test",
            expires_at: None,
        })
        .await?;

    let extractor = jwt_extractor(&ctx)?;
    let principal = authenticate(&issued.secret, &extractor, &ctx)
        .await
        .expect("api key authenticates");
    assert_eq!(principal.user_id().as_str(), cred.user_id.as_str());
    assert!(
        principal.attested_session().is_none(),
        "api-key has no session"
    );
    Ok(())
}

#[tokio::test]
async fn authenticate_rejects_unknown_api_key() -> Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let extractor = jwt_extractor(&ctx)?;
    let (status, _msg) = authenticate("sp-live-deadbeefdeadbeef", &extractor, &ctx)
        .await
        .expect_err("unknown api key must fail");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn authenticate_accepts_seeded_jwt() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    let cred = seed_admin_credential(&pool, "auth-jwt@example.invalid").await?;
    let extractor = jwt_extractor(&ctx)?;
    let principal = authenticate(cred.jwt.as_str(), &extractor, &ctx)
        .await
        .expect("jwt authenticates");
    assert_eq!(principal.user_id().as_str(), cred.user_id.as_str());
    assert_eq!(
        principal.attested_session().map(|s| s.as_str()),
        Some(cred.session_id.as_str()),
        "jwt principal carries its attested session"
    );
    Ok(())
}

#[test]
fn build_rejection_record_needs_user_id() {
    let partial = RejectionPartial::default();
    let id = AiRequestId::generate();
    assert!(
        build_rejection_record(&id, &partial).is_none(),
        "no user_id yields no record"
    );
}

#[test]
fn build_rejection_record_fills_defaults_for_missing_fields() {
    let mut partial = RejectionPartial::default();
    partial.user_id = Some(UserId::new("rej-user"));
    let id = AiRequestId::generate();
    let record = build_rejection_record(&id, &partial).expect("record built");
    assert_eq!(record.provider, "unknown");
    assert_eq!(record.model, "unknown");
}

#[tokio::test]
async fn persist_rejection_writes_audit_row() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "rej-persist@example.invalid").await?;
    let id = AiRequestId::generate();
    let mut partial = RejectionPartial::default();
    partial.user_id = Some(cred.user_id.clone());
    partial.provider = Some("anthropic".to_owned());
    partial.model = Some("claude-test".to_owned());
    partial.body = Some(bytes::Bytes::from_static(b"{\"model\":\"claude-test\"}"));

    persist_rejection(&ctx, &id, &partial, StatusCode::FORBIDDEN, "policy denied").await;

    assert!(
        audit_row_exists(&pool, &id).await?,
        "rejection persisted an ai_requests row"
    );
    Ok(())
}

async fn audit_row_exists(pool: &DbPool, id: &AiRequestId) -> Result<bool> {
    let pg = pool.pool_arc().map_err(|e| anyhow::anyhow!("pool: {e}"))?;
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_optional(pg.as_ref())
        .await?;
    Ok(row.is_some())
}
