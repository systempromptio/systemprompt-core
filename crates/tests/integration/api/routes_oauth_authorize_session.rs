//! `GET|POST /authorize` for a person already signed in to the browser, over
//! the real router: the session cookie the web login mints is recognised (and
//! nothing else in that cookie is), remembered consent makes issuance silent
//! only where the silence policy allows it, the consent page's Allow is bound
//! to the page it was rendered on, `prompt` and `max_age` are honoured, and
//! the way to the login page comes back exactly once.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{HeaderMap, Method, Request, StatusCode, header};
use base64::Engine;
use serde_json::Value;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{ClientId, PluginId, SessionId, UserId};
use systemprompt_manifest::Config;
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::auth::{JwtAudience, Permission, TokenInteraction, UserJwtRequest};
use systemprompt_oauth::repository::{
    ClientRepository, ConsentBinding, DeclaredClientWrite, ReauthBinding,
};
use systemprompt_test_fixtures::{
    OAuthClientFixture, PkcePair, browser_session_cookie, ensure_test_bootstrap,
    fixture_app_context_with_config, fixture_config, oauth_fixture_repository, pkce_pair,
    seed_oauth_client, session_cookie_with, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

const STATE: &str = "state-token-with-enough-entropy-000000";
const ORIGIN: &str = "http://127.0.0.1";
const AUTHORIZE: &str = "/api/v1/core/oauth/authorize";
const WEB_REDIRECT: &str = "https://desk.example.com/callback";
const LOGIN_PAGE: &str = "https://login.example.com/signin";

struct World {
    pool: DbPool,
    app: Router,
    user: UserId,
    email: String,
    client: OAuthClientFixture,
    pkce: PkcePair,
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: String,
}

impl Reply {
    fn location(&self) -> &str {
        self.headers
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
    }

    fn query(&self) -> BTreeMap<String, String> {
        url::Url::parse(self.location())
            .map(|url| url.query_pairs().into_owned().collect())
            .unwrap_or_default()
    }

    fn is_passkey_form(&self) -> bool {
        self.status == StatusCode::OK
            && self.location().is_empty()
            && self.body.contains("Sign in with Passkey")
    }

    fn is_consent_page(&self) -> bool {
        self.status == StatusCode::OK
            && self.location().is_empty()
            && self.body.contains("name=\"consent_token\"")
    }

    fn consent_token(&self) -> String {
        let marker = "name=\"consent_token\" value=\"";
        self.body
            .split_once(marker)
            .and_then(|(_, rest)| rest.split_once('"'))
            .map(|(token, _)| token.to_owned())
            .unwrap_or_default()
    }
}

async fn world() -> anyhow::Result<World> {
    world_with(|_| {}).await
}

async fn world_with(customize: impl FnOnce(&mut Config)) -> anyhow::Result<World> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    customize(&mut config);
    let ctx = fixture_app_context_with_config(config).await?;
    let app = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let email = format!("{}@authorize-session.invalid", user.as_str());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user.as_str())
        .bind(&email)
        .execute(pool.write_pool().as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;
    Ok(World {
        pool,
        app,
        user,
        email,
        client,
        pkce: pkce_pair(),
    })
}

async fn declared(mut w: World, redirect: &str) -> anyhow::Result<World> {
    sqlx::query(
        "UPDATE oauth_clients SET declared_source = 'identity/desk.yaml' WHERE client_id = $1",
    )
    .bind(w.client.client_id.as_str())
    .execute(w.pool.write_pool().as_ref())
    .await?;
    sqlx::query("UPDATE oauth_client_redirect_uris SET redirect_uri = $2 WHERE client_id = $1")
        .bind(w.client.client_id.as_str())
        .bind(redirect)
        .execute(w.pool.write_pool().as_ref())
        .await?;
    w.client.redirect_uri = redirect.to_owned();
    Ok(w)
}

async fn remember(w: &World, scopes: &[&str]) -> anyhow::Result<()> {
    let scopes: Vec<String> = scopes.iter().map(|s| (*s).to_owned()).collect();
    sqlx::query("INSERT INTO oauth_consents (user_id, client_id, scopes) VALUES ($1, $2, $3)")
        .bind(w.user.as_str())
        .bind(w.client.client_id.as_str())
        .bind(scopes)
        .execute(w.pool.write_pool().as_ref())
        .await?;
    Ok(())
}

async fn remembered(w: &World) -> anyhow::Result<Option<Vec<String>>> {
    Ok(sqlx::query_scalar(
        "SELECT scopes FROM oauth_consents WHERE user_id = $1 AND client_id = $2",
    )
    .bind(w.user.as_str())
    .bind(w.client.client_id.as_str())
    .fetch_optional(w.pool.write_pool().as_ref())
    .await?)
}

async fn codes_minted(w: &World) -> anyhow::Result<i64> {
    Ok(
        sqlx::query_scalar("SELECT count(*) FROM oauth_auth_codes WHERE client_id = $1")
            .bind(w.client.client_id.as_str())
            .fetch_one(w.pool.write_pool().as_ref())
            .await?,
    )
}

fn request_pairs<'a>(w: &'a World, extra: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
    let base = [
        ("response_type", "code"),
        ("client_id", w.client.client_id.as_str()),
        ("redirect_uri", w.client.redirect_uri.as_str()),
        ("scope", "user"),
        ("state", STATE),
        ("code_challenge", w.pkce.challenge.as_str()),
        ("code_challenge_method", "S256"),
    ];
    let mut pairs: Vec<(&str, &str)> = base
        .into_iter()
        .filter(|(key, _)| !extra.iter().any(|(overridden, _)| overridden == key))
        .collect();
    pairs.extend_from_slice(extra);
    pairs
}

fn encode(pairs: &[(&str, &str)]) -> String {
    url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

async fn send(app: &Router, mut req: Request<Body>) -> anyhow::Result<Reply> {
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.7:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok(Reply {
        status,
        headers,
        body: String::from_utf8_lossy(&body).into_owned(),
    })
}

async fn get_uri(w: &World, uri: &str, cookie: Option<&str>) -> anyhow::Result<Reply> {
    let mut req = Request::builder()
        .method(Method::GET)
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::ACCEPT, "text/html");
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    send(&w.app, req.body(Body::empty())?).await
}

async fn authorize(
    w: &World,
    extra: &[(&str, &str)],
    cookie: Option<&str>,
) -> anyhow::Result<Reply> {
    let uri = format!("{AUTHORIZE}?{}", encode(&request_pairs(w, extra)));
    get_uri(w, &uri, cookie).await
}

async fn decide(
    w: &World,
    extra: &[(&str, &str)],
    cookie: Option<&str>,
    origin: Option<&str>,
) -> anyhow::Result<Reply> {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri(AUTHORIZE)
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        req = req.header(header::COOKIE, cookie);
    }
    if let Some(origin) = origin {
        req = req.header(header::ORIGIN, origin);
    }
    let body = encode(&request_pairs(w, extra));
    send(&w.app, req.body(Body::from(body))?).await
}

fn claims_of(token: &str) -> Value {
    let payload = token.split('.').nth(1).unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

fn cookie_claims(cookie: &str) -> Value {
    claims_of(cookie.trim_start_matches("access_token="))
}

async fn redeem(w: &World, code: &str) -> anyhow::Result<(StatusCode, Value)> {
    let body = encode(&[
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", w.client.client_id.as_str()),
        ("client_secret", w.client.client_secret.as_str()),
        ("redirect_uri", w.client.redirect_uri.as_str()),
        ("code_verifier", w.pkce.verifier.as_str()),
    ]);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/core/oauth/token")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))?;
    let reply = send(&w.app, req).await?;
    Ok((
        reply.status,
        serde_json::from_str(&reply.body).unwrap_or(Value::Null),
    ))
}

fn assert_code_redirect(w: &World, reply: &Reply) -> String {
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    assert!(
        reply.location().starts_with(&w.client.redirect_uri),
        "{}",
        reply.location()
    );
    let query = reply.query();
    assert_eq!(query.get("state").map(String::as_str), Some(STATE));
    assert_eq!(query.get("iss").map(String::as_str), Some(ORIGIN));
    assert!(!query.contains_key("error"), "{}", reply.location());
    query.get("code").cloned().expect("a code")
}

fn assert_error_redirect(w: &World, reply: &Reply, error: &str) {
    assert_eq!(reply.status, StatusCode::SEE_OTHER, "{}", reply.body);
    assert!(
        reply.location().starts_with(&w.client.redirect_uri),
        "{}",
        reply.location()
    );
    let query = reply.query();
    assert_eq!(query.get("error").map(String::as_str), Some(error));
    assert_eq!(query.get("state").map(String::as_str), Some(STATE));
    assert_eq!(query.get("iss").map(String::as_str), Some(ORIGIN));
    assert!(!query.contains_key("code"), "{}", reply.location());
}

#[tokio::test]
async fn remembered_consent_for_a_declared_web_client_issues_silently() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 900).await?;
    let signed_in_at = cookie_claims(&cookie)["auth_time"]
        .as_i64()
        .expect("auth_time");

    let reply = authorize(&w, &[], Some(&cookie)).await?;
    let code = assert_code_redirect(&w, &reply);
    assert_eq!(
        reply
            .headers
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-store")
    );

    let (status, tokens) = redeem(&w, &code).await?;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let claims = claims_of(tokens["access_token"].as_str().unwrap_or_default());
    assert_eq!(claims["sub"].as_str(), Some(w.user.as_str()), "{claims}");
    assert_eq!(claims["auth_time"].as_i64(), Some(signed_in_at), "{claims}");
    assert!(
        claims["iat"].as_i64().unwrap_or_default() - signed_in_at >= 900,
        "the code carries the sign-in, not the mint: {claims}"
    );
    Ok(())
}

#[tokio::test]
async fn consent_that_does_not_cover_the_scopes_is_asked_again() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["anonymous"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;

    let reply = authorize(&w, &[], Some(&cookie)).await?;
    assert!(
        reply.is_consent_page(),
        "{} {}",
        reply.status,
        reply.location()
    );
    assert_eq!(codes_minted(&w).await?, 0);
    Ok(())
}

#[tokio::test]
async fn a_native_redirect_gets_the_consent_page_unless_the_operator_opted_in() -> anyhow::Result<()>
{
    let w = declared(world().await?, "http://127.0.0.1/callback").await?;
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;

    let reply = authorize(&w, &[], Some(&cookie)).await?;
    assert!(
        reply.is_consent_page(),
        "{} {}",
        reply.status,
        reply.location()
    );
    assert_eq!(codes_minted(&w).await?, 0);

    let any_port = with_redirect(&w, "http://127.0.0.1:53211/callback");
    let reply = authorize(&any_port, &[], Some(&cookie)).await?;
    assert!(
        reply.is_consent_page(),
        "{} {}",
        reply.status,
        reply.location()
    );

    let opted = World {
        app: world_with(|c| c.authorize.silent_native_clients = true)
            .await?
            .app,
        ..w
    };
    let reply = authorize(&opted, &[], Some(&cookie)).await?;
    assert_code_redirect(&opted, &reply);
    Ok(())
}

#[tokio::test]
async fn the_consent_page_names_the_request_and_is_neither_framed_nor_cached() -> anyhow::Result<()>
{
    let w = declared(world().await?, WEB_REDIRECT).await?;
    sqlx::query("UPDATE oauth_clients SET client_name = $2 WHERE client_id = $1")
        .bind(w.client.client_id.as_str())
        .bind("Desk <script>alert(1)</script> {consent_token}")
        .execute(w.pool.write_pool().as_ref())
        .await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;

    let reply = authorize(&w, &[], Some(&cookie)).await?;
    assert!(
        reply.is_consent_page(),
        "{} {}",
        reply.status,
        reply.location()
    );
    assert!(
        reply.body.contains(&w.email),
        "the signed-in account is shown"
    );
    assert!(reply.body.contains("https://desk.example.com"));
    assert!(reply.body.contains("name=\"user_consent\" value=\"allow\""));
    assert!(reply.body.contains("name=\"user_consent\" value=\"deny\""));
    assert!(!reply.body.contains("<script"), "no script on the page");
    assert!(
        reply
            .body
            .contains("Desk &lt;script&gt;alert(1)&lt;/script&gt; &#123;consent_token&#125;")
    );
    assert!(!reply.body.contains("type=\"password\"") && !reply.body.contains("type=\"email\""));
    let header_of = |name: &str| {
        reply
            .headers
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    assert_eq!(header_of("x-frame-options"), "DENY");
    assert!(header_of("content-security-policy").contains("frame-ancestors 'none'"));
    assert_eq!(header_of("cache-control"), "no-store");
    assert_eq!(
        remembered(&w).await?,
        None,
        "showing the page stores nothing"
    );
    Ok(())
}

#[tokio::test]
async fn allow_stores_consent_and_returns_a_code_carrying_the_sign_in() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 300).await?;
    let signed_in_at = cookie_claims(&cookie)["auth_time"]
        .as_i64()
        .expect("auth_time");
    let page = authorize(&w, &[], Some(&cookie)).await?;
    assert!(
        page.is_consent_page(),
        "{} {}",
        page.status,
        page.location()
    );
    let token = page.consent_token();

    let reply = decide(
        &w,
        &[
            ("user_consent", "allow"),
            ("consent_token", &token),
            ("resource", ""),
        ],
        Some(&cookie),
        Some(ORIGIN),
    )
    .await?;
    let code = assert_code_redirect(&w, &reply);
    assert_eq!(remembered(&w).await?, Some(vec!["user".to_owned()]));

    let (status, tokens) = redeem(&w, &code).await?;
    assert_eq!(status, StatusCode::OK, "{tokens}");
    let claims = claims_of(tokens["access_token"].as_str().unwrap_or_default());
    assert_eq!(claims["auth_time"].as_i64(), Some(signed_in_at), "{claims}");

    let silent = authorize(&w, &[], Some(&cookie)).await?;
    assert_code_redirect(&w, &silent);
    let asked = authorize(&w, &[("prompt", "consent")], Some(&cookie)).await?;
    assert!(asked.is_consent_page(), "prompt=consent always asks");
    Ok(())
}

#[tokio::test]
async fn a_later_allow_unions_its_scopes_into_the_remembered_consent() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["anonymous"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let page = authorize(&w, &[], Some(&cookie)).await?;

    let reply = decide(
        &w,
        &[
            ("user_consent", "allow"),
            ("consent_token", &page.consent_token()),
        ],
        Some(&cookie),
        Some(ORIGIN),
    )
    .await?;
    assert_code_redirect(&w, &reply);
    assert_eq!(
        remembered(&w).await?,
        Some(vec!["anonymous".to_owned(), "user".to_owned()])
    );
    Ok(())
}

fn allow(token: &str) -> [(&str, &str); 2] {
    [("user_consent", "allow"), ("consent_token", token)]
}

fn with_redirect(w: &World, redirect: &str) -> World {
    World {
        pool: w.pool.clone(),
        app: w.app.clone(),
        user: w.user.clone(),
        email: w.email.clone(),
        client: OAuthClientFixture {
            redirect_uri: redirect.to_owned(),
            ..w.client.clone()
        },
        pkce: pkce_pair(),
    }
}

fn binding<'a>(w: &'a World, user: &'a UserId, session: &'a SessionId) -> ConsentBinding<'a> {
    ConsentBinding {
        user_id: user,
        session_id: session,
        client_id: &w.client.client_id,
        redirect_uri: &w.client.redirect_uri,
        scope: "user",
        state: STATE,
        code_challenge: &w.pkce.challenge,
        code_challenge_method: "S256",
        response_type: "code",
        resource: None,
    }
}

fn reauth<'a>(w: &'a World, state: &'a str, client_id: &'a ClientId) -> ReauthBinding<'a> {
    ReauthBinding {
        client_id,
        response_type: "code",
        redirect_uri: Some(&w.client.redirect_uri),
        scope: Some("user"),
        state: Some(state),
        code_challenge: Some(&w.pkce.challenge),
        code_challenge_method: Some("S256"),
        resource: None,
    }
}

#[tokio::test]
async fn a_consent_token_binds_the_challenge_method_and_response_type() -> anyhow::Result<()> {
    let w = world().await?;
    let repo = oauth_fixture_repository(&w.pool);
    let session = SessionId::new("sess-consent-binding");
    let now = chrono::Utc::now().timestamp();
    let bound = binding(&w, &w.user, &session);
    let token = repo.consent_token(&bound, now + 300)?;
    assert!(repo.consent_token_matches(&bound, &token, now)?);
    for (name, other) in [
        (
            "plain challenge method",
            ConsentBinding {
                code_challenge_method: "plain",
                ..bound
            },
        ),
        (
            "token response type",
            ConsentBinding {
                response_type: "token",
                ..bound
            },
        ),
    ] {
        assert!(!repo.consent_token_matches(&other, &token, now)?, "{name}");
    }
    Ok(())
}

#[tokio::test]
async fn only_a_marker_minted_for_this_request_waives_a_new_sign_in() -> anyhow::Result<()> {
    let w = declared(
        world_with(|c| c.login_page_url = Some(LOGIN_PAGE.to_owned())).await?,
        WEB_REDIRECT,
    )
    .await?;
    remember(&w, &["user"]).await?;
    let repo = oauth_fixture_repository(&w.pool);
    let now = chrono::Utc::now().timestamp();
    let issued = now - 360;
    let five_minutes_old = browser_session_cookie(&w.pool, &w.user, 300).await?;
    let other_client = ClientId::new("sp_some_other_client");
    let unsigned = issued.to_string();
    let forged = format!("{issued}.{}", "0".repeat(64));
    let other_state = repo.reauth_marker(
        &reauth(
            &w,
            "another-state-with-enough-entropy-0000",
            &w.client.client_id,
        ),
        issued,
    )?;
    let other_client_marker = repo.reauth_marker(&reauth(&w, STATE, &other_client), issued)?;
    let genuine = repo.reauth_marker(&reauth(&w, STATE, &w.client.client_id), issued)?;
    let expired = repo.reauth_marker(&reauth(&w, STATE, &w.client.client_id), now - 3600)?;

    for marker in [
        unsigned.as_str(),
        forged.as_str(),
        other_state.as_str(),
        other_client_marker.as_str(),
        expired.as_str(),
        "not-a-marker",
    ] {
        for asked in [("prompt", "login"), ("max_age", "0")] {
            let reply = authorize(
                &w,
                &[asked, ("reauth_since", marker)],
                Some(&five_minutes_old),
            )
            .await?;
            assert_eq!(reply.status, StatusCode::SEE_OTHER, "{marker} {asked:?}");
            assert!(
                reply.location().starts_with(&format!("{LOGIN_PAGE}?")),
                "{marker} {asked:?}: {}",
                reply.location()
            );
        }
    }
    assert_eq!(codes_minted(&w).await?, 0);

    for asked in [("prompt", "login"), ("max_age", "0")] {
        let reply = authorize(
            &w,
            &[asked, ("reauth_since", genuine.as_str())],
            Some(&five_minutes_old),
        )
        .await?;
        assert_code_redirect(&w, &reply);
    }
    Ok(())
}

#[tokio::test]
async fn an_allow_that_is_not_the_pages_own_is_refused_and_leaves_nothing() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let other_session = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let page = authorize(&w, &[], Some(&cookie)).await?;
    let token = page.consent_token();
    assert!(!token.is_empty());
    let session = SessionId::new(
        cookie_claims(&cookie)["session_id"]
            .as_str()
            .expect("session_id")
            .to_owned(),
    );
    let repo = oauth_fixture_repository(&w.pool);
    let now = chrono::Utc::now().timestamp();
    let expired = repo.consent_token(&binding(&w, &w.user, &session), now - 1)?;
    let too_long = repo.consent_token(&binding(&w, &w.user, &session), now + 900)?;
    let tampered = format!("{token}0");
    let (expiry, mac) = token.split_once('.').expect("expiry.mac");
    let stretched = format!("{}.{mac}", expiry.parse::<i64>()? + 1);

    let refusals: Vec<(&str, Reply, StatusCode)> = vec![
        (
            "no token",
            decide(
                &w,
                &[("user_consent", "allow")],
                Some(&cookie),
                Some(ORIGIN),
            )
            .await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "garbage token",
            decide(&w, &allow("not-a-token"), Some(&cookie), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "tampered token",
            decide(&w, &allow(&tampered), Some(&cookie), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "expiry moved",
            decide(&w, &allow(&stretched), Some(&cookie), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "expired token",
            decide(&w, &allow(&expired), Some(&cookie), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "token living longer than the limit",
            decide(&w, &allow(&too_long), Some(&cookie), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "another session of the same person",
            decide(&w, &allow(&token), Some(&other_session), Some(ORIGIN)).await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "a different state",
            decide(
                &w,
                &[
                    ("user_consent", "allow"),
                    ("consent_token", &token),
                    ("state", "another-state-with-enough-entropy-11111"),
                ],
                Some(&cookie),
                Some(ORIGIN),
            )
            .await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "a wider scope",
            decide(
                &w,
                &[
                    ("user_consent", "allow"),
                    ("consent_token", &token),
                    ("scope", "user anonymous"),
                ],
                Some(&cookie),
                Some(ORIGIN),
            )
            .await?,
            StatusCode::BAD_REQUEST,
        ),
        (
            "foreign origin",
            decide(
                &w,
                &allow(&token),
                Some(&cookie),
                Some("https://evil.example"),
            )
            .await?,
            StatusCode::FORBIDDEN,
        ),
        (
            "null origin",
            decide(&w, &allow(&token), Some(&cookie), Some("null")).await?,
            StatusCode::FORBIDDEN,
        ),
        (
            "no origin",
            decide(&w, &allow(&token), Some(&cookie), None).await?,
            StatusCode::FORBIDDEN,
        ),
        (
            "no cookie",
            decide(&w, &allow(&token), None, Some(ORIGIN)).await?,
            StatusCode::FORBIDDEN,
        ),
        (
            "neither allow nor deny",
            decide(
                &w,
                &[("user_consent", "yes"), ("consent_token", &token)],
                Some(&cookie),
                Some(ORIGIN),
            )
            .await?,
            StatusCode::BAD_REQUEST,
        ),
    ];
    for (name, reply, status) in &refusals {
        assert_eq!(reply.status, *status, "{name}: {}", reply.body);
        assert!(reply.location().is_empty(), "{name}: {}", reply.location());
    }
    assert_eq!(codes_minted(&w).await?, 0, "nothing minted");
    assert_eq!(remembered(&w).await?, None, "nothing stored");

    let good = repo.consent_token(&binding(&w, &w.user, &session), now + 300)?;
    let reply = decide(&w, &allow(&good), Some(&cookie), Some(ORIGIN)).await?;
    assert_code_redirect(&w, &reply);
    Ok(())
}

#[tokio::test]
async fn deny_answers_access_denied_and_stores_nothing() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let page = authorize(&w, &[], Some(&cookie)).await?;

    let reply = decide(
        &w,
        &[
            ("user_consent", "deny"),
            ("consent_token", &page.consent_token()),
        ],
        Some(&cookie),
        Some(ORIGIN),
    )
    .await?;
    assert_error_redirect(&w, &reply, "access_denied");
    assert_eq!(codes_minted(&w).await?, 0);
    assert_eq!(remembered(&w).await?, None);

    let cross_site = decide(
        &w,
        &[("user_consent", "deny")],
        None,
        Some("https://evil.example"),
    )
    .await?;
    assert_eq!(cross_site.status, StatusCode::FORBIDDEN);
    assert!(cross_site.location().is_empty());
    Ok(())
}

#[tokio::test]
async fn prompt_none_answers_with_the_oidc_error_instead_of_a_page() -> anyhow::Result<()> {
    let web = declared(world().await?, WEB_REDIRECT).await?;
    let none = [("prompt", "none")];

    let reply = authorize(&web, &none, None).await?;
    assert_error_redirect(&web, &reply, "login_required");

    let cookie = browser_session_cookie(&web.pool, &web.user, 60).await?;
    let reply = authorize(&web, &none, Some(&cookie)).await?;
    assert_error_redirect(&web, &reply, "consent_required");

    let stale = authorize(
        &web,
        &[("prompt", "none"), ("max_age", "30")],
        Some(&cookie),
    )
    .await?;
    assert_error_redirect(&web, &stale, "login_required");

    remember(&web, &["user"]).await?;
    let reply = authorize(&web, &none, Some(&cookie)).await?;
    assert_code_redirect(&web, &reply);

    let native = declared(world().await?, "http://127.0.0.1/callback").await?;
    remember(&native, &["user"]).await?;
    let cookie = browser_session_cookie(&native.pool, &native.user, 60).await?;
    let reply = authorize(&native, &none, Some(&cookie)).await?;
    assert_error_redirect(&native, &reply, "interaction_required");
    assert_eq!(codes_minted(&native).await?, 0);

    let contradictory = authorize(&web, &[("prompt", "none login")], None).await?;
    assert_eq!(contradictory.status, StatusCode::SEE_OTHER);
    assert_eq!(
        contradictory.query().get("error").map(String::as_str),
        Some("invalid_request")
    );
    Ok(())
}

#[tokio::test]
async fn prompt_login_and_a_stale_max_age_go_to_the_login_page_with_the_way_back()
-> anyhow::Result<()> {
    let w = declared(
        world_with(|c| c.login_page_url = Some(LOGIN_PAGE.to_owned())).await?,
        WEB_REDIRECT,
    )
    .await?;
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 3600).await?;
    let before = chrono::Utc::now().timestamp();

    for extra in [
        vec![("prompt", "login")],
        vec![("prompt", "select_account")],
        vec![("max_age", "600")],
        vec![("max_age", "0")],
        vec![("prompt", "login"), ("login_hint", "ada@example.com")],
    ] {
        let reply = authorize(&w, &extra, Some(&cookie)).await?;
        assert_eq!(
            reply.status,
            StatusCode::SEE_OTHER,
            "{extra:?}: {}",
            reply.body
        );
        assert!(
            reply.location().starts_with(&format!("{LOGIN_PAGE}?")),
            "{}",
            reply.location()
        );
        let sent = reply.query();
        assert_eq!(
            sent.get("client_id").map(String::as_str),
            Some(w.client.client_id.as_str())
        );
        for (key, value) in &extra {
            assert_eq!(
                sent.get(*key).map(String::as_str),
                Some(*value),
                "{extra:?}"
            );
        }
        let way_back = sent.get("redirect").expect("the way back");
        let (path, marker) = way_back
            .rsplit_once("&reauth_since=")
            .expect("the reauth marker");
        assert!(
            path.starts_with(&format!("{AUTHORIZE}?response_type=code")),
            "{way_back}"
        );
        let (issued, _mac) = marker.split_once('.').expect("issued.mac");
        assert!(issued.parse::<i64>()? >= before, "{way_back}");

        let returned = get_uri(&w, way_back, Some(&cookie)).await?;
        assert_error_redirect(&w, &returned, "login_required");

        let fresh = browser_session_cookie(&w.pool, &w.user, 0).await?;
        let returned = get_uri(&w, way_back, Some(&fresh)).await?;
        assert_code_redirect(&w, &returned);
    }

    let signed_out = authorize(&w, &[], None).await?;
    assert!(signed_out.location().starts_with(&format!("{LOGIN_PAGE}?")));
    let way_back = signed_out
        .query()
        .get("redirect")
        .cloned()
        .expect("the way back");
    let returned = get_uri(&w, &way_back, None).await?;
    assert_error_redirect(&w, &returned, "login_required");

    let within = authorize(&w, &[("max_age", "7200")], Some(&cookie)).await?;
    assert_code_redirect(&w, &within);
    Ok(())
}

#[tokio::test]
async fn without_a_login_page_a_needed_sign_in_is_the_passkey_form() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 3600).await?;

    for (extra, cookie) in [
        (vec![], None),
        (vec![("prompt", "login")], Some(cookie.as_str())),
        (vec![("max_age", "60")], Some(cookie.as_str())),
        (vec![("prompt", "passkey")], Some(cookie.as_str())),
    ] {
        let reply = authorize(&w, &extra, cookie).await?;
        assert!(
            reply.is_passkey_form(),
            "{extra:?}: {} {}",
            reply.status,
            reply.location()
        );
    }
    assert_eq!(codes_minted(&w).await?, 0);
    Ok(())
}

async fn refused_as_a_session(w: &World, name: &str, cookie: &str) -> anyhow::Result<()> {
    let reply = authorize(w, &[], Some(cookie)).await?;
    assert!(
        reply.is_passkey_form(),
        "{name} must not be a session: {} {}",
        reply.status,
        reply.location()
    );
    let none = authorize(w, &[("prompt", "none")], Some(cookie)).await?;
    assert_error_redirect(w, &none, "login_required");
    Ok(())
}

#[tokio::test]
async fn only_the_web_logins_own_token_is_a_session() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["user"]).await?;
    let client = w.client.client_id.clone();
    type Edit = Box<dyn FnOnce(&mut UserJwtRequest)>;
    let edits: Vec<(&str, Edit)> = vec![
        (
            "a token /token issued to the web client",
            Box::new(|r| r.client_id = Some(ClientId::web())),
        ),
        ("a bearer token", Box::new(|r| r.browser_session = false)),
        (
            "a UI session",
            Box::new(move |r| {
                r.browser_session = false;
                r.ui_session = true;
                r.client_id = Some(client);
                r.audience = vec![JwtAudience::Ui];
            }),
        ),
        (
            "an anonymous visitor",
            Box::new(|r| r.permissions = vec![Permission::Anonymous]),
        ),
        (
            "a purpose-scoped token",
            Box::new(|r| r.purpose = Some("download".to_owned())),
        ),
        (
            "a plugin token",
            Box::new(|r| r.plugin_id = Some(PluginId::new("plugin"))),
        ),
        (
            "an unattended token",
            Box::new(|r| r.interaction = Some(TokenInteraction::NonInteractive)),
        ),
    ];
    for (name, edit) in edits {
        let cookie = session_cookie_with(&w.pool, &w.user, 60, edit).await?;
        refused_as_a_session(&w, name, &cookie).await?;
    }
    refused_as_a_session(&w, "a garbage cookie", "access_token=not.a.jwt").await?;

    let bearer_only = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let token = bearer_only.trim_start_matches("access_token=");
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("{AUTHORIZE}?{}", encode(&request_pairs(&w, &[]))))
        .header(header::HOST, "127.0.0.1")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    let reply = send(&w.app, req).await?;
    assert!(reply.is_passkey_form(), "a bearer header is never read");

    assert_eq!(codes_minted(&w).await?, 0);
    let real = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let reply = authorize(&w, &[], Some(&real)).await?;
    assert_code_redirect(&w, &reply);
    Ok(())
}

#[tokio::test]
async fn a_revoked_token_or_session_and_an_unusable_account_are_not_sessions() -> anyhow::Result<()>
{
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["user"]).await?;
    let pool = w.pool.write_pool();

    let revoked_jti = browser_session_cookie(&w.pool, &w.user, 60).await?;
    sqlx::query(
        "INSERT INTO oauth_jti_revocations (jti, user_id, exp) \
         VALUES ($1, $2, now() + interval '1 hour')",
    )
    .bind(cookie_claims(&revoked_jti)["jti"].as_str().expect("jti"))
    .bind(w.user.as_str())
    .execute(pool.as_ref())
    .await?;
    refused_as_a_session(&w, "a revoked jti", &revoked_jti).await?;

    let revoked_session = browser_session_cookie(&w.pool, &w.user, 60).await?;
    sqlx::query("UPDATE user_sessions SET revoked_at = now() WHERE session_id = $1")
        .bind(
            cookie_claims(&revoked_session)["session_id"]
                .as_str()
                .expect("session_id"),
        )
        .execute(pool.as_ref())
        .await?;
    refused_as_a_session(&w, "a revoked session", &revoked_session).await?;

    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let reply = authorize(&w, &[], Some(&cookie)).await?;
    assert_code_redirect(&w, &reply);
    let minted = codes_minted(&w).await?;

    sqlx::query("UPDATE users SET account_kind = 'service' WHERE id = $1")
        .bind(w.user.as_str())
        .execute(pool.as_ref())
        .await?;
    refused_as_a_session(&w, "a service account", &cookie).await?;
    sqlx::query("UPDATE users SET account_kind = 'person', status = 'suspended' WHERE id = $1")
        .bind(w.user.as_str())
        .execute(pool.as_ref())
        .await?;
    refused_as_a_session(&w, "an inactive user", &cookie).await?;
    assert_eq!(codes_minted(&w).await?, minted);
    Ok(())
}

#[tokio::test]
async fn a_dynamically_registered_client_is_never_silent_and_stores_no_consent()
-> anyhow::Result<()> {
    let w = world().await?;
    sqlx::query("UPDATE oauth_client_redirect_uris SET redirect_uri = $2 WHERE client_id = $1")
        .bind(w.client.client_id.as_str())
        .bind(WEB_REDIRECT)
        .execute(w.pool.write_pool().as_ref())
        .await?;
    let w = World {
        pkce: pkce_pair(),
        ..with_redirect(&w, WEB_REDIRECT)
    };
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;

    let page = authorize(&w, &[], Some(&cookie)).await?;
    assert!(
        page.is_consent_page(),
        "{} {}",
        page.status,
        page.location()
    );
    let none = authorize(&w, &[("prompt", "none")], Some(&cookie)).await?;
    assert_error_redirect(&w, &none, "consent_required");

    sqlx::query("DELETE FROM oauth_consents WHERE client_id = $1")
        .bind(w.client.client_id.as_str())
        .execute(w.pool.write_pool().as_ref())
        .await?;
    let reply = decide(
        &w,
        &[
            ("user_consent", "allow"),
            ("consent_token", &page.consent_token()),
        ],
        Some(&cookie),
        Some(ORIGIN),
    )
    .await?;
    assert_code_redirect(&w, &reply);
    assert_eq!(remembered(&w).await?, None, "nothing is remembered for it");

    let again = authorize(&w, &[], Some(&cookie)).await?;
    assert!(again.is_consent_page(), "it is asked every time");
    Ok(())
}

#[tokio::test]
async fn a_request_without_a_registered_redirect_never_uses_the_session() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    remember(&w, &["user"]).await?;
    let cookie = browser_session_cookie(&w.pool, &w.user, 60).await?;
    let pairs: Vec<(&str, &str)> = request_pairs(&w, &[])
        .into_iter()
        .filter(|(key, _)| *key != "redirect_uri")
        .collect();

    let reply = get_uri(
        &w,
        &format!("{AUTHORIZE}?{}", encode(&pairs)),
        Some(&cookie),
    )
    .await?;
    assert!(
        reply.is_passkey_form(),
        "{} {}",
        reply.status,
        reply.location()
    );

    let mut unregistered = pairs.clone();
    unregistered.push(("redirect_uri", "https://evil.example/phish"));
    let reply = get_uri(
        &w,
        &format!("{AUTHORIZE}?{}", encode(&unregistered)),
        Some(&cookie),
    )
    .await?;
    assert!(reply.status.is_client_error(), "{}", reply.status);
    assert!(reply.location().is_empty());
    assert_eq!(codes_minted(&w).await?, 0);
    Ok(())
}

#[tokio::test]
async fn a_person_ui_flip_and_a_release_clear_the_remembered_consent() -> anyhow::Result<()> {
    let w = declared(world().await?, WEB_REDIRECT).await?;
    let redirects = vec![WEB_REDIRECT.to_owned()];
    let grants = vec!["authorization_code".to_owned(), "refresh_token".to_owned()];
    let scopes = vec!["user".to_owned()];
    let write = |person_ui: bool| DeclaredClientWrite {
        client_id: &w.client.client_id,
        client_name: "Desk",
        owner_user_id: &w.user,
        is_active: true,
        secret_key: None,
        secret_hash: None,
        redirect_uris: &redirects,
        grant_types: &grants,
        scopes: &scopes,
        person_ui,
        source: "identity/desk.yaml",
    };
    let upsert = |person_ui: bool| {
        let pool = w.pool.write_pool();
        let write = write(person_ui);
        async move {
            let mut tx = pool.begin().await?;
            ClientRepository::upsert_declared_on(&mut tx, &write).await?;
            tx.commit().await?;
            anyhow::Ok(())
        }
    };

    remember(&w, &["user"]).await?;
    upsert(false).await?;
    assert!(
        remembered(&w).await?.is_some(),
        "an unchanged client keeps it"
    );

    upsert(true).await?;
    assert_eq!(remembered(&w).await?, None, "the flip clears it");

    remember(&w, &["user"]).await?;
    let mut conn = w.pool.write_pool().acquire().await?;
    ClientRepository::release_declared_on(&mut conn, &w.client.client_id).await?;
    assert_eq!(remembered(&w).await?, None, "the release clears it");
    assert!(
        !oauth_fixture_repository(&w.pool)
            .record_consent(&w.user, &w.client.client_id, &scopes)
            .await?,
        "a released client stores no consent"
    );
    assert_eq!(remembered(&w).await?, None);
    Ok(())
}
