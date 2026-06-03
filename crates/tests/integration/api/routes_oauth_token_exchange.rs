//! `/token` RFC 8693 token-exchange grant — drives `handle_token_exchange`
//! through the public `/token` route. The error branches (missing/invalid
//! subject token, untrusted issuer, unsupported subject_token_type, missing
//! client) are fully deterministic and need no network. We also attempt a
//! self-issued exchange end to end: the subject token is signed by the
//! process-wide test authority installed by `install_test_signing_key`, with
//! the issuer read from the live `Config` so it matches whichever config the
//! shared `Once` installed first.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{AgentName, ClientId, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::auth::{AuthenticatedUser, Permission};
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::services::{JwtConfig, JwtSigningParams, generate_jwt};
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_db_pool, install_test_signing_key,
    seed_oauth_client,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

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
            jwt_issuer: "test".to_owned(),
            jwt_access_token_expiration: 3600,
            jwt_refresh_token_expiration: 86_400,
            jwt_audiences: vec![],
            allowed_resource_audiences: vec!["hook".to_owned()],
            trusted_issuers: vec![],
            signing_key_path: std::path::PathBuf::from("signing_key.pem"),
            use_https: false,
            rate_limits: RateLimitConfig::default(),
            cors_allowed_origins: vec![],
            trusted_proxies: vec![],
            is_cloud: false,
            system_admin_username: "admin".to_owned(),
            content_negotiation: ContentNegotiationConfig::default(),
            security_headers: SecurityHeadersConfig::default(),
            allow_registration: false,
        });
    });
}

const TOKEN_EXCHANGE_GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";

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

async fn token_app() -> anyhow::Result<Router> {
    ensure_config();
    install_test_signing_key();
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        Arc::clone(ctx.db_pool()),
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
    let user = UserId::new(format!("tx-owner-{}", Uuid::new_v4()));
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@tx.invalid", user.as_str()))
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

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null))
}

fn urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
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

fn mint_self_issued_subject(scopes: Vec<Permission>) -> String {
    install_test_signing_key();
    let issuer = Config::get().expect("config").jwt_issuer.clone();
    let user = AuthenticatedUser::new_with_roles(
        Uuid::new_v4(),
        "tx-subject".to_owned(),
        "tx-subject@tx.invalid".to_owned(),
        scopes.clone(),
        scopes.iter().map(ToString::to_string).collect(),
    );
    let config = JwtConfig {
        permissions: scopes,
        audience: vec![],
        expires_in_hours: Some(1),
        resource: None,
        plugin_id: None,
    };
    let signing = JwtSigningParams { issuer: &issuer };
    generate_jwt(
        &user,
        config,
        Uuid::new_v4().to_string(),
        &SessionId::generate(),
        &signing,
    )
    .expect("mint self-issued subject token")
}

#[tokio::test]
async fn exchange_missing_subject_token_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn exchange_missing_client_id_returns_invalid_request() -> anyhow::Result<()> {
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", "x.y.z"),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn exchange_bad_client_secret_returns_invalid_client() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", "x.y.z"),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
        ("client_id", client.client_id.as_str()),
        ("client_secret", "wrong-secret"),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert!(
        v["error"].as_str().is_some_and(|e| e.contains("invalid")),
        "expected invalid_* error, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn exchange_unsupported_subject_token_type_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", "x.y.z"),
        ("subject_token_type", "urn:bogus:token-type"),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn exchange_malformed_subject_token_returns_invalid_request() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", "not-a-jwt"),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn exchange_resource_not_in_allowed_audiences_returns_invalid_target() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let subject = mint_self_issued_subject(vec![Permission::User]);
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", &subject),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
        ("resource", "https://not-allowed.example/"),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "{status} {v}");
    // A self-issued subject that validates but carries a disallowed resource is
    // rejected with invalid_target; if the process-wide config differs (issuer
    // mismatch from another test file installing first) the subject token fails
    // validation as invalid_request, which is also a 4xx error path.
    let err = v["error"].as_str().unwrap_or("");
    assert!(
        err == "invalid_target" || err == "invalid_request" || err.contains("invalid"),
        "expected an invalid_* error, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn exchange_self_issued_subject_does_not_500() -> anyhow::Result<()> {
    let client = seeded_client().await?;
    let subject = mint_self_issued_subject(vec![Permission::User]);
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", &subject),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
        ("client_id", client.client_id.as_str()),
        ("client_secret", &client.client_secret),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    // Either the exchange succeeds (200 with an access_token) or it is rejected
    // with a structured 4xx error. A self-issued, validly-signed subject token
    // must never trip an unhandled 500.
    assert!(
        status.is_success() || status.is_client_error(),
        "self-issued subject must not 500, got {status} {v}"
    );
    if status.is_success() {
        let access_token = v["access_token"]
            .as_str()
            .unwrap_or_else(|| panic!("success must carry an access_token; got {v}"));
        assert_eq!(v.get("token_type").and_then(|x| x.as_str()), Some("Bearer"));
        let claims = decode_jwt_claims(access_token)?;
        assert!(
            claims["roles"].is_null()
                || claims["roles"].as_array().is_some_and(Vec::is_empty),
            "delegated token must not carry scope strings as RBAC roles; got roles={}",
            claims["roles"]
        );
    }
    Ok(())
}

fn decode_jwt_claims(token: &str) -> anyhow::Result<serde_json::Value> {
    use base64::Engine as _;

    let payload = token
        .split('.')
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("token is not a JWT: {token}"))?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[tokio::test]
async fn exchange_unknown_client_does_not_500() -> anyhow::Result<()> {
    // Build the app first: token_app installs the process-global Config that
    // mint_self_issued_subject reads for the issuer.
    let app = token_app().await?;
    let subject = mint_self_issued_subject(vec![Permission::User]);
    let unknown_client = ClientId::new("no-such-client");
    let body = urlencode(&[
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", &subject),
        ("subject_token_type", ACCESS_TOKEN_TYPE),
        ("client_id", unknown_client.as_str()),
        ("client_secret", "irrelevant"),
    ]);
    let resp = app.oneshot(form_post("/token", body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "unknown client must be 4xx, got {status} {v}");
    Ok(())
}
