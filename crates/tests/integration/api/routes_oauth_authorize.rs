//! `/authorize` OAuth authorization endpoint — `handle_authorize_get` and
//! `handle_authorize_post`. The GET path validates CSRF/state, PKCE and the
//! OAuth parameter set, resolves the client, then renders the WebAuthn
//! challenge form; the POST path rejects password auth (and denied consent)
//! after validation. We cover the happy render (plain state and a path-shaped
//! state echoed unchanged), and the missing-state, bad-pkce,
//! unsupported-response-type, unknown-client, denied-consent and
//! password-attempt branches.


use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{Actor, AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::execution::context::RequestContext;
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, seed_oauth_client, test_db_pool,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, read_json, setup_ctx};

const VALID_STATE: &str = "state-token-with-enough-entropy-000000";
const VALID_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

fn fixture_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("test-trace"),
        ContextId::generate(),
        AgentName::system(),
        Actor::user(UserId::new("00000000-0000-4000-8000-000000000001")),
    )
}

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(fixture_request_context());
    next.run(req).await
}

async fn authorize_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    Ok(public_router(std::sync::Arc::new(
        systemprompt_oauth_issuance::JwksCache::new(),
    ))
    .layer(middleware::from_fn(inject_context))
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone())))
}

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let user = UserId::new(format!("authz-owner-{}", Uuid::new_v4()));
    let p = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@authz.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

fn form_post(uri: &str, body: String) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))
        .expect("build")
}

fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char);
            },
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

#[tokio::test]
async fn authorize_get_valid_request_renders_webauthn_form() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("http://127.0.0.1/callback"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains("<strong id=\"client-name\">test-client</strong>"),
        "consent page must name the registered client, not just its id: {html}"
    );
    assert!(
        html.contains(client.client_id.as_str()),
        "client id still shown for auditability"
    );
    assert!(
        html.contains("http://127.0.0.1"),
        "redirect destination must be shown"
    );
    assert!(
        html.contains("id=\"authorize-btn\"")
            && html.contains("name=\"user_consent\" value=\"deny\""),
        "explicit authorize/cancel step missing: {html}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_get_scope_outside_client_registration_is_refused() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=admin&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("http://127.0.0.1/callback"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    assert!(
        !status.is_success(),
        "a scope the client never registered must not reach consent: {status}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_get_loopback_redirect_matches_any_port() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("http://127.0.0.1:53281/callback"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn authorize_get_path_shaped_state_is_echoed_to_the_client_unchanged() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let return_path = "/dashboard/return/after/consent/padding0";
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("http://127.0.0.1/callback"),
        enc(return_path),
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    let html = String::from_utf8_lossy(&bytes);
    assert!(
        html.contains(&format!("name=\"state\" value=\"{return_path}\"")),
        "the client's own state must reach its redirect untouched: {html}"
    );
    assert!(
        !html.contains("state_"),
        "no server-issued state may replace the client's: {html}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_get_missing_state_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn authorize_get_missing_pkce_challenge_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&scope=openid&state={}",
        client.client_id.as_str(),
        VALID_STATE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    assert!(status.is_client_error(), "{status}");
    Ok(())
}

#[tokio::test]
async fn authorize_get_unsupported_response_type_is_rejected() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=token&client_id={}&redirect_uri={}&scope=openid&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("http://127.0.0.1/callback"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    assert!(
        status.is_client_error() || status.is_redirection(),
        "unsupported response_type must be a 4xx or a redirect carrying the error, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_get_unknown_client_is_rejected() -> anyhow::Result<()> {
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id=no-such-client&scope=openid&state={}&code_challenge={}&code_challenge_method=S256",
        VALID_STATE, VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    let status = resp.status();
    assert!(
        status.is_client_error() || status.is_redirection(),
        "unknown client must be rejected, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_post_denied_consent_returns_access_denied() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let body = urlencode(&[
        ("response_type", "code"),
        ("client_id", client.client_id.as_str()),
        ("redirect_uri", "http://127.0.0.1/callback"),
        ("scope", "user"),
        ("state", VALID_STATE),
        ("code_challenge", VALID_CHALLENGE),
        ("code_challenge_method", "S256"),
        ("user_consent", "deny"),
    ]);
    let resp = app.oneshot(form_post("/authorize", body)).await?;
    let status = resp.status();
    assert!(
        status.is_client_error() || status.is_redirection(),
        "denied consent must be rejected, got {status}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_post_password_attempt_is_unsupported() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let body = urlencode(&[
        ("response_type", "code"),
        ("client_id", client.client_id.as_str()),
        ("redirect_uri", "http://127.0.0.1/callback"),
        ("scope", "user"),
        ("state", VALID_STATE),
        ("code_challenge", VALID_CHALLENGE),
        ("code_challenge_method", "S256"),
        ("user_consent", "allow"),
        ("username", "someone"),
        ("password", "hunter2"),
    ]);
    let resp = app.oneshot(form_post("/authorize", body)).await?;
    let status = resp.status();
    assert!(
        status.is_client_error() || status.is_redirection(),
        "password auth must be rejected as unsupported, got {status}"
    );
    Ok(())
}

// `resolve_resource_scopes` (and the `get_mcp_server_scopes_from_resource`
// parser beneath it) only runs when the authorize request names a `resource`
// AND the OAuthState carries an MCP registry. The shared fixture supplies
// neither, so both stayed at zero calls; attaching the registry from the
// AppContext puts the resource-scoped authorization path on its real branch.
async fn authorize_app_with_mcp_registry() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    )
    .with_mcp_registry(std::sync::Arc::new(ctx.mcp_registry().clone()));
    Ok(public_router(std::sync::Arc::new(
        systemprompt_oauth_issuance::JwksCache::new(),
    ))
    .layer(middleware::from_fn(inject_context))
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone())))
}

fn authorize_uri(client_id: &str, resource: Option<&str>) -> String {
    let mut uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}&code_challenge={}&code_challenge_method=S256",
        client_id,
        enc("http://127.0.0.1/callback"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    if let Some(resource) = resource {
        uri.push_str(&format!("&resource={}", enc(resource)));
    }
    uri
}

#[tokio::test]
async fn authorize_with_an_unknown_mcp_resource_still_validates() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app_with_mcp_registry().await?;

    let resp = app
        .oneshot(empty_get(&authorize_uri(
            client.client_id.as_str(),
            Some("http://127.0.0.1/api/v1/mcp/no-such-server/mcp"),
        )))
        .await?;

    // An unregistered resource contributes no scopes; it must not fail the
    // request, and equally must not silently widen it.
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn authorize_rejects_an_https_resource_pointing_at_a_link_local_address() -> anyhow::Result<()>
{
    let client = seeded_client().await?;
    let app = authorize_app_with_mcp_registry().await?;

    let resp = app
        .oneshot(empty_get(&authorize_uri(
            client.client_id.as_str(),
            Some("https://169.254.169.254/api/v1/mcp/meta/mcp"),
        )))
        .await?;

    // The cloud metadata endpoint is the canonical SSRF target; an
    // authorization request naming it must not be honoured.
    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a resource naming a link-local address must be refused"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_rejects_an_internal_suffix_resource() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app_with_mcp_registry().await?;

    let resp = app
        .oneshot(empty_get(&authorize_uri(
            client.client_id.as_str(),
            Some("https://vault.internal/api/v1/mcp/secrets/mcp"),
        )))
        .await?;

    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "an `.internal` resource host is not externally routable and must be refused"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_with_a_malformed_resource_is_refused() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app_with_mcp_registry().await?;

    let resp = app
        .oneshot(empty_get(&authorize_uri(
            client.client_id.as_str(),
            Some("not-a-uri"),
        )))
        .await?;

    assert_ne!(
        resp.status(),
        StatusCode::OK,
        "a resource that is not a URI cannot be validated and must not be accepted"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_without_a_resource_is_unaffected_by_the_registry() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app_with_mcp_registry().await?;

    let resp = app
        .oneshot(empty_get(&authorize_uri(client.client_id.as_str(), None)))
        .await?;

    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    Ok(())
}

// RFC 6749 §4.1.2.1: the server must not redirect the user-agent to a
// `redirect_uri` it has not confirmed is registered for the client. Every
// validation failure below happens before (or because) that confirmation
// fails, so the error must render in place with no `Location` at all.
#[tokio::test]
async fn authorize_get_unknown_client_never_redirects_to_the_supplied_uri() -> anyhow::Result<()> {
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id=no-such-client&redirect_uri={}&scope=openid&state={}&code_challenge={}&code_challenge_method=S256",
        enc("https://evil.example/phish"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    assert!(
        resp.headers().get(header::LOCATION).is_none(),
        "an unregistered client must not be redirected: {:?}",
        resp.headers().get(header::LOCATION)
    );
    Ok(())
}

#[tokio::test]
async fn authorize_get_unregistered_redirect_uri_for_known_client_renders_in_place()
-> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}&code_challenge={}&code_challenge_method=S256",
        client.client_id.as_str(),
        enc("https://evil.example/phish"),
        VALID_STATE,
        VALID_CHALLENGE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    assert!(resp.headers().get(header::LOCATION).is_none());
    Ok(())
}

#[tokio::test]
async fn authorize_get_pkce_failure_with_unregistered_redirect_renders_in_place()
-> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}",
        client.client_id.as_str(),
        enc("https://evil.example/phish"),
        VALID_STATE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    assert!(resp.headers().get(header::LOCATION).is_none());
    Ok(())
}

#[tokio::test]
async fn authorize_get_pkce_failure_with_registered_redirect_redirects_with_error()
-> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let uri = format!(
        "/authorize?response_type=code&client_id={}&redirect_uri={}&scope=user&state={}",
        client.client_id.as_str(),
        enc("http://127.0.0.1/callback"),
        VALID_STATE,
    );
    let resp = app.oneshot(empty_get(&uri)).await?;
    assert!(resp.status().is_redirection(), "{}", resp.status());
    let location = resp
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.starts_with("http://127.0.0.1/callback?error=invalid_request"),
        "{location}"
    );
    assert!(
        location.contains(&format!("state={VALID_STATE}")),
        "{location}"
    );
    Ok(())
}

#[tokio::test]
async fn authorize_post_denied_consent_with_unregistered_redirect_renders_in_place()
-> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = authorize_app().await?;
    let body = urlencode(&[
        ("response_type", "code"),
        ("client_id", client.client_id.as_str()),
        ("redirect_uri", "https://evil.example/phish"),
        ("scope", "user"),
        ("state", VALID_STATE),
        ("code_challenge", VALID_CHALLENGE),
        ("code_challenge_method", "S256"),
        ("user_consent", "deny"),
    ]);
    let resp = app.oneshot(form_post("/authorize", body)).await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "{}", resp.status());
    assert!(resp.headers().get(header::LOCATION).is_none());
    Ok(())
}
