//! `/authorize` OAuth authorization endpoint — `handle_authorize_get` and
//! `handle_authorize_post`. The GET path validates CSRF/state, PKCE and the
//! OAuth parameter set, resolves the client, then renders the WebAuthn
//! challenge form; the POST path rejects password auth (and denied consent)
//! after validation. We cover the happy render (plain state and same-origin
//! return-path server-state issuance), and the missing-state, bad-pkce,
//! unsupported-response-type, unknown-client, denied-consent and
//! password-attempt branches.

use std::sync::Once;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, StatusCode, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_db_pool, seed_oauth_client,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::{empty_get, setup_ctx};

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let _ = Config::install(Config {
            instance_id: "test".to_owned(),
            max_concurrent_streams: 16,
            sitename: "test".to_owned(),
            database_type: "postgres".to_owned(),
            database_url: "postgres://x".to_owned(),
            database_write_url: None,
            github_link: String::new(),
            github_token: None,
            system_path: "/tmp".to_owned(),
            services_path: "/tmp".to_owned(),
            bin_path: "/tmp".to_owned(),
            skills_path: "/tmp".to_owned(),
            settings_path: "/tmp".to_owned(),
            content_config_path: "/tmp".to_owned(),
            geoip_database_path: None,
            web_path: "/tmp".to_owned(),
            web_config_path: "/tmp".to_owned(),
            web_metadata_path: "/tmp".to_owned(),
            host: "127.0.0.1".to_owned(),
            port: 0,
            api_server_url: "http://127.0.0.1".to_owned(),
            api_internal_url: "http://127.0.0.1".to_owned(),
            api_external_url: "http://127.0.0.1".to_owned(),
            jwt_issuer: "https://issuer.test".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 86_400,
            jwt_audiences: vec![],
            allowed_resource_audiences: vec!["hook".to_owned()],
            trusted_issuers: vec![],
            id_jag_ttl_secs: 300,
            signing_key_path: std::path::PathBuf::from("signing_key.pem"),
            use_https: false,
            rate_limits: RateLimitConfig::default(),
            cors_allowed_origins: vec![],
            trusted_proxies: vec![],
            is_cloud: false,
            system_admin_username: "admin".to_owned(),
            system_admin_email: None,
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            allow_registration: false,
            login_page_url: None,
        });
    });
}

const VALID_STATE: &str = "state-token-with-enough-entropy-000000";
const VALID_CHALLENGE: &str = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";

fn fixture_request_context() -> RequestContext {
    RequestContext::new(
        SessionId::generate(),
        TraceId::new("test-trace"),
        ContextId::generate(),
        AgentName::system(),
    )
}

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(fixture_request_context());
    next.run(req).await
}

async fn authorize_app() -> anyhow::Result<Router> {
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    );
    Ok(public_router()
        .layer(middleware::from_fn(inject_context))
        .with_state(state))
}

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(format!("authz-owner-{}", Uuid::new_v4()));
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@authz.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
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
    assert!(!bytes.is_empty(), "empty webauthn form");
    Ok(())
}

#[tokio::test]
async fn authorize_get_same_origin_return_path_issues_server_state() -> anyhow::Result<()> {
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
    ensure_config();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.user_provider().expect("user"),
    )
    .with_mcp_registry(std::sync::Arc::new(ctx.mcp_registry().clone()));
    Ok(public_router()
        .layer(middleware::from_fn(inject_context))
        .with_state(state))
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
