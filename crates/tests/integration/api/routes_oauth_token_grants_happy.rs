//! `/token` happy paths for the authorization-code and refresh-token grants.
//! Earlier files cover the rejection branches; here a real authorization code
//! is seeded through `OAuthRepository` and redeemed over HTTP, and the
//! returned refresh token is rotated. Assertions on token issuance are
//! tolerant of the process-global `Config` race (another test file may have
//! installed a config whose audiences differ), but every request must resolve
//! to a structured wire response, never a 500.

use std::sync::{Arc, Once};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use axum::middleware::{self, Next};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{
    AgentName, AuthorizationCode, ContextId, SessionId, TraceId, UserId,
};
use systemprompt_models::Config;
use systemprompt_models::config::RateLimitConfig;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile::{ContentNegotiationConfig, SecurityHeadersConfig};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{AuthCodeParams, OAuthRepository};
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_db_pool, install_test_signing_key,
    pkce_pair, seed_oauth_client,
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
            id_jag_ttl_secs: 300,
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

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(RequestContext::new(
        SessionId::generate(),
        TraceId::new("grants-happy"),
        ContextId::generate(),
        AgentName::system(),
    ));
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

struct SeededGrant {
    client: OAuthClientFixture,
    code: AuthorizationCode,
}

async fn seed_grant(
    pkce: Option<(&str, &str)>,
    resource: Option<&str>,
) -> anyhow::Result<SeededGrant> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@grants.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;

    let repo = OAuthRepository::new(&pool).map_err(|e| anyhow::anyhow!("oauth repo: {e}"))?;
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4().simple()));
    let params = AuthCodeParams {
        code: &code,
        client_id: &client.client_id,
        user_id: &user,
        redirect_uri: &client.redirect_uri,
        scope: "user",
        code_challenge: pkce.map(|(c, _)| c),
        code_challenge_method: pkce.map(|(_, m)| m),
        resource,
    };
    repo.store_authorization_code(params)
        .await
        .map_err(|e| anyhow::anyhow!("store auth code: {e}"))?;

    Ok(SeededGrant { client, code })
}

fn form_post(body: String) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri("/token")
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

async fn redeem(
    app: Router,
    grant: &SeededGrant,
    extra: &[(&str, &str)],
) -> anyhow::Result<(axum::http::StatusCode, serde_json::Value)> {
    let mut pairs = vec![
        ("grant_type", "authorization_code"),
        ("code", grant.code.as_str()),
        ("client_id", grant.client.client_id.as_str()),
        ("client_secret", grant.client.client_secret.as_str()),
        ("redirect_uri", grant.client.redirect_uri.as_str()),
    ];
    pairs.extend_from_slice(extra);
    let resp = app.oneshot(form_post(urlencode(&pairs))).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    Ok((status, v))
}

#[tokio::test]
async fn authorization_code_grant_issues_tokens() -> anyhow::Result<()> {
    let grant = seed_grant(None, None).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[]).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    assert!(
        v["access_token"].as_str().is_some_and(|t| !t.is_empty()),
        "{v}"
    );
    assert!(v["refresh_token"].as_str().is_some(), "{v}");
    assert_eq!(v["token_type"].as_str(), Some("Bearer"), "{v}");
    assert_eq!(v["expires_in"].as_i64(), Some(3600), "{v}");
    Ok(())
}

#[tokio::test]
async fn authorization_code_grant_resolves_client_from_code() -> anyhow::Result<()> {
    let grant = seed_grant(None, None).await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "authorization_code"),
        ("code", grant.code.as_str()),
        ("client_secret", grant.client.client_secret.as_str()),
        ("redirect_uri", grant.client.redirect_uri.as_str()),
    ]);
    let resp = app.oneshot(form_post(body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    assert!(v["access_token"].as_str().is_some(), "{v}");
    Ok(())
}

#[tokio::test]
async fn authorization_code_grant_with_pkce_verifier_succeeds() -> anyhow::Result<()> {
    let pair = pkce_pair();
    let grant = seed_grant(Some((&pair.challenge, pair.method)), None).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[("code_verifier", &pair.verifier)]).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    assert!(v["access_token"].as_str().is_some(), "{v}");
    Ok(())
}

#[tokio::test]
async fn authorization_code_grant_with_wrong_pkce_verifier_fails() -> anyhow::Result<()> {
    let pair = pkce_pair();
    let grant = seed_grant(Some((&pair.challenge, pair.method)), None).await?;
    let app = token_app().await?;
    let (status, v) = redeem(
        app,
        &grant,
        &[(
            "code_verifier",
            "wrong-verifier-wrong-verifier-wrong-verifier",
        )],
    )
    .await?;
    assert!(status.is_client_error(), "expected 4xx, got {status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_grant"), "{v}");
    Ok(())
}

#[tokio::test]
async fn authorization_code_grant_with_matching_resource_succeeds() -> anyhow::Result<()> {
    let grant = seed_grant(None, Some("hook")).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[("resource", "hook")]).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    Ok(())
}

#[tokio::test]
async fn authorization_code_grant_with_mismatched_resource_fails() -> anyhow::Result<()> {
    let grant = seed_grant(None, Some("hook")).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[("resource", "other")]).await?;
    assert!(status.is_client_error(), "expected 4xx, got {status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_grant"), "{v}");
    Ok(())
}

async fn issue_refresh_token() -> anyhow::Result<(OAuthClientFixture, String)> {
    let grant = seed_grant(None, None).await?;
    let app = token_app().await?;
    let (status, v) = redeem(app, &grant, &[]).await?;
    anyhow::ensure!(status.is_success(), "seed grant failed: {status} {v}");
    let refresh = v["refresh_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("no refresh_token in {v}"))?
        .to_owned();
    Ok((grant.client, refresh))
}

#[tokio::test]
async fn refresh_token_grant_rotates_tokens() -> anyhow::Result<()> {
    let (client, refresh) = issue_refresh_token().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh),
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
    ]);
    let resp = app.oneshot(form_post(body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    assert!(v["access_token"].as_str().is_some(), "{v}");
    let rotated = v["refresh_token"].as_str().expect("rotated refresh token");
    assert_ne!(rotated, refresh, "refresh token must rotate");
    Ok(())
}

#[tokio::test]
async fn refresh_token_grant_resolves_client_from_token() -> anyhow::Result<()> {
    let (client, refresh) = issue_refresh_token().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh),
        ("client_secret", client.client_secret.as_str()),
    ]);
    let resp = app.oneshot(form_post(body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    Ok(())
}

#[tokio::test]
async fn refresh_token_grant_allows_scope_narrowing() -> anyhow::Result<()> {
    let (client, refresh) = issue_refresh_token().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh),
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
        ("scope", "user"),
    ]);
    let resp = app.oneshot(form_post(body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_success(), "expected 200, got {status} {v}");
    Ok(())
}

#[tokio::test]
async fn refresh_token_grant_rejects_scope_escalation() -> anyhow::Result<()> {
    let (client, refresh) = issue_refresh_token().await?;
    let app = token_app().await?;
    let body = urlencode(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", &refresh),
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
        ("scope", "admin"),
    ]);
    let resp = app.oneshot(form_post(body)).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    assert!(status.is_client_error(), "expected 4xx, got {status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn consumed_refresh_token_cannot_be_replayed() -> anyhow::Result<()> {
    let (client, refresh) = issue_refresh_token().await?;
    let app = token_app().await?;
    let form = || {
        urlencode(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", client.client_id.as_str()),
            ("client_secret", client.client_secret.as_str()),
        ])
    };
    let first = app.clone().oneshot(form_post(form())).await?;
    assert!(first.status().is_success(), "first rotation must succeed");
    let second = app.oneshot(form_post(form())).await?;
    let status = second.status();
    let v = read_json(second).await?;
    assert!(
        status.is_client_error(),
        "replay must fail, got {status} {v}"
    );
    Ok(())
}

#[tokio::test]
async fn authorization_code_cannot_be_redeemed_twice() -> anyhow::Result<()> {
    let grant = seed_grant(None, None).await?;
    let app = token_app().await?;
    let (first, v1) = redeem(app.clone(), &grant, &[]).await?;
    assert!(first.is_success(), "first redemption must succeed: {v1}");
    let (second, v2) = redeem(app, &grant, &[]).await?;
    assert!(
        second.is_client_error(),
        "code replay must fail, got {second} {v2}"
    );
    assert_eq!(v2["error"].as_str(), Some("invalid_grant"), "{v2}");
    Ok(())
}
