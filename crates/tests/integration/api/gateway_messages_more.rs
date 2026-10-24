//! Integration tests (coverage campaign 2026-07).
//!
//! Drives the gateway message-extraction helpers in
//! `routes::gateway::messages`: the header parsers, canonical-body
//! reader, conversation-id derivation, pre-dispatch authz enforcement, the
//! `authenticate` credential dispatcher (API-key + JWT), and the rejection
//! audit-record builder / persister — none of which are reachable through the
//! router because the fixture profile leaves the gateway unmounted.

use std::sync::Arc;
use systemprompt_api::routes::gateway::messages::error::RejectionError;

use anyhow::Result;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use systemprompt_ai::models::RequestStatus;
use systemprompt_api::routes::gateway::messages::auth::{AuthedPrincipal, KeyScopes, Principal};
use systemprompt_api::routes::gateway::messages::extract::authz::{
    DispatchTarget, enforce_authz_pre_dispatch,
};
use systemprompt_api::routes::gateway::messages::extract::headers::{
    optional_gateway_conversation_id, optional_session_id, read_gateway_body,
};
use systemprompt_api::routes::gateway::messages::extract::{RejectionPartial, derive_conversation};
use systemprompt_api::routes::gateway::messages::rejection::{
    build_rejection_record, persist_rejection,
};
use systemprompt_database::DbPool;
use systemprompt_gateway::protocol::anthropic_messages::AnthropicMessagesInbound;
use systemprompt_gateway::protocol::{
    CanonicalContent, CanonicalMessage, CanonicalReasoning, CanonicalRequest, CanonicalSampling,
    CanonicalTooling, InboundAdapter, Role,
};
use systemprompt_governance::authz::{AllowAllHook, DenyAllHook, SharedAuthzHook};
use systemprompt_identifiers::headers::{GATEWAY_CONVERSATION_ID, SESSION_ID};
use systemprompt_identifiers::{
    AiRequestId, ContextId, ModelId, ProviderId, SessionId, TraceId, UserId,
};
use systemprompt_test_fixtures::seed_admin_credential;
use systemprompt_users::{ApiKeyService, IssueApiKeyParams};

use super::common::setup_ctx;

use systemprompt_models::origin::{
    ClientAttestation, ClientKind, InboundWireProtocol, RequestOrigin,
};

pub(super) fn test_partial() -> RejectionPartial {
    RejectionPartial::new(RequestOrigin::gateway(
        ClientKind::Other,
        InboundWireProtocol::AnthropicMessages,
        ClientAttestation::None,
    ))
}

fn gateway_journal() -> systemprompt_gateway::audit::journal::GatewayJournal {
    systemprompt_gateway::audit::journal::GatewayJournal::open(
        systemprompt_test_fixtures::ensure_test_bootstrap()
            .app_paths
            .storage()
            .data(),
        systemprompt_config::SecretsBootstrap::encryption_master_key()
            .expect("encryption master key"),
    )
    .expect("gateway journal opens")
}


pub(super) fn gw_repos(
    db: &systemprompt_database::DbPool,
) -> systemprompt_gateway::GatewayRepositories {
    systemprompt_gateway::GatewayRepositories::new(
        db,
        gateway_journal(),
        std::sync::Arc::new(systemprompt_agent::services::ContextProviderService::new(
            systemprompt_agent::repository::ContextRepository::new(db),
        )),
        systemprompt_gateway::GatewayHandles {
            background: systemprompt_traits::BackgroundTasks::new(),
            cache_epochs: systemprompt_traits::CacheEpochs::new(),
            logging: systemprompt_test_fixtures::fixture_logging_runtime(),
        },
    )
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

fn inbound() -> Arc<dyn InboundAdapter> {
    Arc::new(AnthropicMessagesInbound)
}

fn canonical(messages: Vec<CanonicalMessage>) -> CanonicalRequest {
    CanonicalRequest {
        model: ModelId::new("claude-test"),
        cache_control: None,
        system: Vec::new(),
        messages,
        max_tokens: 128,
        stream: false,
        metadata: None,
        response_format: None,
        forwarded_surface: Default::default(),
        sampling: CanonicalSampling {
            temperature: None,
            top_p: None,
            top_k: None,
            stop_sequences: Vec::new(),
            presence_penalty: None,
            frequency_penalty: None,
        },
        tooling: CanonicalTooling {
            tools: Vec::new(),
            tool_choice: None,
            search: None,
            code_execution: false,
        },
        reasoning: CanonicalReasoning {
            thinking: None,
            reasoning_effort: None,
        },
    }
}

fn user_message(text: &str) -> CanonicalMessage {
    CanonicalMessage {
        role: Role::User,
        content: vec![CanonicalContent::text(text.to_owned())],
    }
}

#[test]
fn optional_session_id_reads_present_header() {
    let h = header_map(&[(SESSION_ID, "sess-123")]);
    let id = optional_session_id(&h).expect("session id parsed");
    assert_eq!(id.as_str(), "sess-123");
}

#[test]
fn optional_session_id_trims_surrounding_whitespace() {
    let h = header_map(&[(SESSION_ID, "  sess-trim  ")]);
    let id = optional_session_id(&h).expect("session id parsed");
    assert_eq!(id.as_str(), "sess-trim");
}

#[test]
fn optional_session_id_missing_or_empty_header_is_none_not_an_error() {
    assert!(optional_session_id(&HeaderMap::new()).is_none());
    assert!(optional_session_id(&header_map(&[(SESSION_ID, "   ")])).is_none());
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
    let mut partial = test_partial();
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
    let mut partial = test_partial();
    let RejectionError { status, .. } = read_gateway_body(&inbound, request, &mut partial)
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
    let mut partial = test_partial();
    let (conv, ctx, _client) = derive_conversation(
        &systemprompt_identifiers::UserId::new("owner-a"),
        Some(header),
        &request,
        &mut partial,
    )
    .expect("derived ok");
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
    let mut partial = test_partial();
    let (conv, _ctx, _client) = derive_conversation(
        &systemprompt_identifiers::UserId::new("owner-a"),
        None,
        &request,
        &mut partial,
    )
    .expect("derived ok");
    assert!(!conv.as_str().is_empty());
}

#[test]
fn derive_conversation_without_messages_is_bad_request() {
    let request = canonical(vec![]);
    let mut partial = test_partial();
    let RejectionError {
        status,
        message: msg,
        ..
    } = derive_conversation(
        &systemprompt_identifiers::UserId::new("owner-a"),
        None,
        &request,
        &mut partial,
    )
    .expect_err("no messages must fail");
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(msg.contains("cannot derive"), "{msg}");
}

fn api_key_principal(user: &str) -> AuthedPrincipal {
    AuthedPrincipal {
        principal: Principal {
            user_id: UserId::new(user),
            roles: Vec::new(),
            subject: systemprompt_governance::authz::SubjectAttributes::new(),
            attributes: std::collections::BTreeMap::new(),
            act_chain: Vec::new(),
            client_id: None,
            scopes: Some(KeyScopes {
                api_key_id: systemprompt_identifiers::ApiKeyId::generate(),
                limits: systemprompt_users::ApiKeyLimits::default(),
                scopes: Vec::new(),
            }),
        },
        trace_id: TraceId::generate(),
        session_id: SessionId::generate(),
        client_session: None,
    }
}

fn gateway_route() -> systemprompt_identifiers::RouteId {
    systemprompt_identifiers::RouteId::new("claude-route")
}

#[tokio::test]
async fn enforce_authz_allows_under_allow_all_hook() {
    let hook: SharedAuthzHook = Arc::new(AllowAllHook::null());
    let route = gateway_route();
    let principal = api_key_principal("authz-allow-user");
    enforce_authz_pre_dispatch(
        &principal,
        DispatchTarget::new(&route, "claude-test", &ProviderId::new("anthropic")),
        &ContextId::generate(),
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
    let RejectionError {
        status,
        message: msg,
        ..
    } = enforce_authz_pre_dispatch(
        &principal,
        DispatchTarget::new(&route, "claude-test", &ProviderId::new("anthropic")),
        &ContextId::generate(),
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
    use systemprompt_traits::UserProvider;
    let analytics = Arc::clone(&ctx.analytics_repositories().session_store);
    let user_provider: Arc<dyn UserProvider> = Arc::new(systemprompt_users::UserService::new(
        Arc::clone(ctx.user_repository()),
    ));
    let jti = JtiRevocationChecker::from_repository(
        ctx.oauth_repositories().oauth.clone(),
        systemprompt_traits::CacheEpochs::new(),
    );
    let issuer = ctx.config().jwt_issuer.clone();
    Ok(JwtContextExtractor::new(
        analytics,
        user_provider,
        jti,
        issuer,
        systemprompt_test_fixtures::test_signing_keys(),
    ))
}

#[tokio::test]
async fn authenticate_accepts_seeded_api_key() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "auth-apikey@example.invalid").await?;
    let service = ApiKeyService::new(
        Arc::clone(ctx.user_repository()),
        systemprompt_users::ApiKeyLifetime::unbounded(),
    );
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &cred.user_id,
            name: "gateway-auth-test",
            expires_at: None,
            limits: &systemprompt_users::ApiKeyLimits::default(),
            scopes: &[],
            purpose: None,
        })
        .await?;

    let extractor = jwt_extractor(&ctx)?;
    let principal = super::gateway_api_key_session_binding::sign_in(
        &ctx,
        &extractor,
        &issued.secret,
        Some(&cred.session_id),
        &systemprompt_governance::authz::SubjectProviderSet::default(),
    )
    .await
    .expect("api key authenticates");
    assert_eq!(principal.user_id().as_str(), cred.user_id.as_str());
    assert_eq!(
        principal.session_id().as_str(),
        cred.session_id.as_str(),
        "an issued session the caller presents is used as is"
    );
    assert!(principal.client_session().is_none());
    Ok(())
}

#[tokio::test]
async fn authenticate_rejects_unknown_api_key() -> Result<()> {
    let (_pool, ctx) = setup_ctx().await?;
    let extractor = jwt_extractor(&ctx)?;
    let RejectionError { status, .. } = super::gateway_api_key_session_binding::sign_in(
        &ctx,
        &extractor,
        "sp-live-deadbeefdeadbeef",
        Some(&SessionId::generate()),
        &systemprompt_governance::authz::SubjectProviderSet::default(),
    )
    .await
    .expect_err("unknown api key must fail");
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn authenticate_accepts_seeded_jwt() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "auth-jwt@example.invalid").await?;
    let extractor = jwt_extractor(&ctx)?;
    let principal = super::gateway_api_key_session_binding::sign_in(
        &ctx,
        &extractor,
        cred.jwt.as_str(),
        Some(&cred.session_id),
        &systemprompt_governance::authz::SubjectProviderSet::default(),
    )
    .await
    .expect("jwt authenticates");
    assert_eq!(principal.user_id().as_str(), cred.user_id.as_str());
    assert_eq!(
        principal.session_id().as_str(),
        cred.session_id.as_str(),
        "jwt principal carries its attested session"
    );
    Ok(())
}

#[test]
fn build_rejection_record_needs_user_id() {
    let partial = test_partial();
    let id = AiRequestId::generate();
    assert!(
        build_rejection_record(None, &id, &partial).is_none(),
        "no user_id yields no record"
    );
}

#[test]
fn build_rejection_record_leaves_unresolved_routing_absent() {
    let mut partial = test_partial();
    partial.user_id = Some(UserId::new("rej-user"));
    let id = AiRequestId::generate();
    let record = build_rejection_record(None, &id, &partial).expect("record built");
    assert_eq!(record.provider, None);
    assert_eq!(record.model, None);
    assert_eq!(record.status, RequestStatus::Rejected);
}

#[test]
fn build_rejection_record_keeps_routing_it_did_resolve() {
    let mut partial = test_partial();
    partial.user_id = Some(UserId::new("rej-user"));
    partial.model = Some("claude-test".to_owned());
    let id = AiRequestId::generate();
    let record = build_rejection_record(None, &id, &partial).expect("record built");
    assert_eq!(record.provider, None);
    assert_eq!(record.model.as_deref(), Some("claude-test"));
}

#[test]
fn build_rejection_record_without_a_context_gets_a_fresh_context_not_the_legacy_row() {
    let mut partial = test_partial();
    partial.user_id = Some(UserId::new("rej-user"));
    partial.context_id = None;
    partial.session_id = None;
    let first =
        build_rejection_record(None, &AiRequestId::generate(), &partial).expect("record built");
    let second =
        build_rejection_record(None, &AiRequestId::generate(), &partial).expect("record built");
    let legacy = systemprompt_identifiers::ContextId::legacy_context_row();
    assert_ne!(first.context_id, legacy);
    assert_ne!(second.context_id, legacy);
    assert_ne!(first.context_id, second.context_id);
}

#[tokio::test]
async fn persist_rejection_writes_audit_row() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "rej-persist@example.invalid").await?;
    let id = AiRequestId::generate();
    let mut partial = test_partial();
    partial.user_id = Some(cred.user_id.clone());
    partial.provider = Some("anthropic".to_owned());
    partial.model = Some("claude-test".to_owned());
    partial.body = Some(bytes::Bytes::from_static(b"{\"model\":\"claude-test\"}"));

    persist_rejection(
        &gw_repos(ctx.db_pool()),
        &id,
        &partial,
        StatusCode::FORBIDDEN,
        "policy denied",
    )
    .await;

    assert!(
        audit_row_exists(&pool, &id).await?,
        "rejection persisted an ai_requests row"
    );
    Ok(())
}

// The governance hole this whole path exists to close: before the routing
// columns became nullable, a request refused ahead of routing failed record
// construction and the caller swallowed the error, so the audit row was
// dropped entirely.
#[tokio::test]
async fn persist_rejection_writes_an_audit_row_when_routing_never_resolved() -> Result<()> {
    let (pool, ctx) = setup_ctx().await?;
    let cred = seed_admin_credential(&pool, "rej-unrouted@example.invalid").await?;
    let id = AiRequestId::generate();
    let mut partial = test_partial();
    partial.user_id = Some(cred.user_id.clone());

    persist_rejection(
        &gw_repos(ctx.db_pool()),
        &id,
        &partial,
        StatusCode::PAYMENT_REQUIRED,
        "credit exhausted",
    )
    .await;

    let row = audit_routing_row(&pool, &id)
        .await?
        .expect("a pre-routing rejection must still be audited");
    assert_eq!(row.0, None, "provider stays absent rather than faked");
    assert_eq!(row.1, None, "model stays absent rather than faked");
    assert_eq!(row.2, "rejected");
    Ok(())
}

async fn audit_routing_row(
    pool: &DbPool,
    id: &AiRequestId,
) -> Result<Option<(Option<String>, Option<String>, String)>> {
    let pg = pool.pool();
    let row: Option<(Option<String>, Option<String>, String)> =
        sqlx::query_as("SELECT provider, model, status FROM ai_requests WHERE id = $1")
            .bind(id.as_str())
            .fetch_optional(pg.as_ref())
            .await?;
    Ok(row)
}

async fn audit_row_exists(pool: &DbPool, id: &AiRequestId) -> Result<bool> {
    let pg = pool.pool();
    let row: Option<(String,)> = sqlx::query_as("SELECT id FROM ai_requests WHERE id = $1")
        .bind(id.as_str())
        .fetch_optional(pg.as_ref())
        .await?;
    Ok(row.is_some())
}
