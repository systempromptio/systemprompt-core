//! Proxy MCP session-identity cache — drives `enrich_with_cached_identity` and
//! `handle_mcp_response` through the engine `test-api` seam, with wiremock
//! providing real backend responses. Also covers the external-MCP outbound
//! header filter and resolve-error mapping.

use axum::http::{HeaderMap, HeaderName, HeaderValue};
use systemprompt_api::services::proxy::engine_test_api::{
    ResponseArgs, TestSessionCache, enrich_with_cached_identity, handle_mcp_response,
    map_resolve_error, outbound_headers,
};
use systemprompt_identifiers::SessionId;
use systemprompt_mcp::McpDomainError;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use uuid::Uuid;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::request_context;

async fn backend_response(template: ResponseTemplate) -> reqwest::Response {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(template)
        .mount(&server)
        .await;
    reqwest::get(server.uri()).await.expect("backend response")
}

#[tokio::test]
async fn enrich_without_session_header_is_a_noop() {
    let cache = TestSessionCache::default();
    let rc = request_context("session-noop");
    let before = rc.user_id().to_string();
    let enriched = enrich_with_cached_identity(&cache, &HeaderMap::new(), rc, "svc").await;
    assert_eq!(enriched.user_id().to_string(), before);
}

#[tokio::test]
async fn enrich_with_unknown_session_leaves_context_unchanged() {
    let cache = TestSessionCache::default();
    let mut headers = HeaderMap::new();
    headers.insert("mcp-session-id", HeaderValue::from_static("unknown-sess"));
    let rc = request_context("session-unknown");
    let before = rc.user_id().to_string();
    let enriched = enrich_with_cached_identity(&cache, &headers, rc, "svc").await;
    assert_eq!(enriched.user_id().to_string(), before);
}

#[tokio::test]
async fn enrich_with_cached_session_adopts_cached_identity() {
    let cache = TestSessionCache::default();
    let user = Uuid::new_v4();
    cache
        .seed(
            &SessionId::new("sess-hit"),
            user,
            vec![Permission::User],
            "cached-token",
        )
        .await;
    let mut headers = HeaderMap::new();
    headers.insert("mcp-session-id", HeaderValue::from_static("sess-hit"));
    let rc = request_context("session-hit");
    let enriched = enrich_with_cached_identity(&cache, &headers, rc, "svc").await;
    assert_eq!(enriched.user_id().to_string(), user.to_string());
    assert_eq!(enriched.auth_token().as_str(), "cached-token");
}

#[tokio::test]
async fn successful_response_with_session_header_caches_identity() {
    let cache = TestSessionCache::default();
    let response =
        backend_response(ResponseTemplate::new(200).insert_header("mcp-session-id", "sess-new"))
            .await;
    let user_uuid = Uuid::new_v4();
    let user = AuthenticatedUser::new(
        user_uuid,
        "cache-user".to_owned(),
        "cache@test.invalid".to_owned(),
        vec![Permission::User],
    );
    let rc = request_context(&user_uuid.to_string());
    handle_mcp_response(ResponseArgs {
        cache: &cache,
        response: &response,
        request_headers: &HeaderMap::new(),
        req_context: &rc,
        authenticated_user: Some(&user),
        service_name: "svc",
        method_str: "POST",
    })
    .await;
    assert_eq!(
        cache.cached_user(&SessionId::new("sess-new")).await,
        Some(user_uuid)
    );
}

#[tokio::test]
async fn response_without_authenticated_user_does_not_cache() {
    let cache = TestSessionCache::default();
    let response =
        backend_response(ResponseTemplate::new(200).insert_header("mcp-session-id", "sess-anon"))
            .await;
    let rc = request_context("anon");
    handle_mcp_response(ResponseArgs {
        cache: &cache,
        response: &response,
        request_headers: &HeaderMap::new(),
        req_context: &rc,
        authenticated_user: None,
        service_name: "svc",
        method_str: "POST",
    })
    .await;
    assert_eq!(cache.cached_user(&SessionId::new("sess-anon")).await, None);
}

#[tokio::test]
async fn delete_request_evicts_cached_session() {
    let cache = TestSessionCache::default();
    let user = Uuid::new_v4();
    cache
        .seed(
            &SessionId::new("sess-del"),
            user,
            vec![Permission::User],
            "tok",
        )
        .await;
    let response = backend_response(ResponseTemplate::new(200)).await;
    let mut request_headers = HeaderMap::new();
    request_headers.insert("mcp-session-id", HeaderValue::from_static("sess-del"));
    let rc = request_context(&user.to_string());
    handle_mcp_response(ResponseArgs {
        cache: &cache,
        response: &response,
        request_headers: &request_headers,
        req_context: &rc,
        authenticated_user: None,
        service_name: "svc",
        method_str: "DELETE",
    })
    .await;
    assert_eq!(cache.cached_user(&SessionId::new("sess-del")).await, None);
}

#[tokio::test]
async fn stale_session_404_on_get_evicts_cache_entry() {
    let cache = TestSessionCache::default();
    let user = Uuid::new_v4();
    cache
        .seed(
            &SessionId::new("sess-stale"),
            user,
            vec![Permission::User],
            "tok",
        )
        .await;
    let response = backend_response(ResponseTemplate::new(404)).await;
    let mut request_headers = HeaderMap::new();
    request_headers.insert("mcp-session-id", HeaderValue::from_static("sess-stale"));
    let rc = request_context(&user.to_string());
    handle_mcp_response(ResponseArgs {
        cache: &cache,
        response: &response,
        request_headers: &request_headers,
        req_context: &rc,
        authenticated_user: None,
        service_name: "svc",
        method_str: "GET",
    })
    .await;
    assert_eq!(cache.cached_user(&SessionId::new("sess-stale")).await, None);
}

#[tokio::test]
async fn error_response_on_post_keeps_cache_entry() {
    let cache = TestSessionCache::default();
    let user = Uuid::new_v4();
    cache
        .seed(
            &SessionId::new("sess-keep"),
            user,
            vec![Permission::User],
            "tok",
        )
        .await;
    let response = backend_response(ResponseTemplate::new(500)).await;
    let mut request_headers = HeaderMap::new();
    request_headers.insert("mcp-session-id", HeaderValue::from_static("sess-keep"));
    let rc = request_context(&user.to_string());
    handle_mcp_response(ResponseArgs {
        cache: &cache,
        response: &response,
        request_headers: &request_headers,
        req_context: &rc,
        authenticated_user: None,
        service_name: "svc",
        method_str: "POST",
    })
    .await;
    assert_eq!(
        cache.cached_user(&SessionId::new("sess-keep")).await,
        Some(user)
    );
}

#[test]
fn outbound_headers_filters_to_mcp_passthrough_set() {
    let mut incoming = HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    incoming.insert("accept", HeaderValue::from_static("application/json"));
    incoming.insert("mcp-session-id", HeaderValue::from_static("sess"));
    incoming.insert(
        "mcp-protocol-version",
        HeaderValue::from_static("2025-01-01"),
    );
    incoming.insert("authorization", HeaderValue::from_static("Bearer leak-me"));
    incoming.insert("cookie", HeaderValue::from_static("secret=1"));

    let out = outbound_headers(&incoming, Vec::new());
    assert!(
        out.get("authorization").is_none(),
        "client bearer must not leak"
    );
    assert!(out.get("cookie").is_none());
    assert_eq!(out.len(), 4);
}

#[test]
fn outbound_headers_provider_credential_wins() {
    let mut incoming = HeaderMap::new();
    incoming.insert("content-type", HeaderValue::from_static("application/json"));
    let provider = vec![(
        HeaderName::from_static("authorization"),
        HeaderValue::from_static("Bearer provider-token"),
    )];
    let out = outbound_headers(&incoming, provider);
    assert_eq!(
        out.get("authorization").and_then(|v| v.to_str().ok()),
        Some("Bearer provider-token")
    );
}

#[test]
fn resolve_error_mapping_covers_auth_and_availability() {
    let auth = map_resolve_error("ext", McpDomainError::AuthRequired("login".to_owned()));
    assert!(auth.contains("Authentication required"), "{auth}");

    let unavailable = map_resolve_error(
        "ext",
        McpDomainError::ExternalAuthUnavailable {
            server: "ext".to_owned(),
            message: "vault down".to_owned(),
        },
    );
    assert!(unavailable.contains("vault down"), "{unavailable}");

    let other = map_resolve_error("ext", McpDomainError::Transport("boom".to_owned()));
    assert!(other.contains("Invalid response"), "{other}");
}
