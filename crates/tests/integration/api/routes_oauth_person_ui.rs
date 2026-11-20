//! A person's UI session over the real router: the code + PKCE grant of a
//! client the operator declared `person_ui` mints a short `ui`-audience token
//! carrying `ui_session` and the sign-in time, the same grant for any other
//! client never does, and refreshing it keeps the sign-in time and stops at the
//! absolute cap.

use std::net::SocketAddr;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use base64::Engine;
use serde_json::Value;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AuthorizationCode, UserId};
use systemprompt_manifest::profile::RateLimitsConfig;
use systemprompt_models::auth::ui_session::UI_SESSION_ACCESS_TTL_SECS;
use systemprompt_oauth::repository::AuthCodeParams;
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_app_context_with_config, fixture_config,
    oauth_fixture_repository, pkce_pair, seed_oauth_client, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

pub(super) struct Person {
    pub(super) pool: DbPool,
    pub(super) app: Router,
    pub(super) user: UserId,
    pub(super) client: OAuthClientFixture,
}

pub(super) async fn person(person_ui: bool) -> anyhow::Result<Person> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(config).await?;
    let app = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &std::sync::Arc::new(ReadinessState::new()),
    )
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    let user = UserId::new(Uuid::new_v4().to_string());
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2)")
        .bind(user.as_str())
        .bind(format!("{}@person-ui.invalid", user.as_str()))
        .execute(pool.write_pool().as_ref())
        .await?;
    let client = seed_oauth_client(&pool, &user).await?;
    if person_ui {
        sqlx::query(
            "UPDATE oauth_clients SET person_ui = true, declared_source = 'identity/desk.yaml' \
             WHERE client_id = $1",
        )
        .bind(client.client_id.as_str())
        .execute(pool.write_pool().as_ref())
        .await?;
    }
    Ok(Person {
        pool,
        app,
        user,
        client,
    })
}

pub(super) async fn send(
    app: &Router,
    mut req: Request<Body>,
) -> anyhow::Result<(StatusCode, String)> {
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.7:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let challenge = resp
        .headers()
        .get(header::WWW_AUTHENTICATE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let body = to_bytes(resp.into_body(), 1024 * 1024).await?;
    Ok((
        status,
        format!("{challenge}\n{}", String::from_utf8_lossy(&body)),
    ))
}

fn token_request(pairs: &[(&str, &str)]) -> anyhow::Result<Request<Body>> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish();
    Ok(Request::builder()
        .method(Method::POST)
        .uri("/api/v1/core/oauth/token")
        .header(header::HOST, "127.0.0.1")
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(Body::from(body))?)
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text.split_once('\n').map_or(text, |(_, body)| body))
        .unwrap_or(Value::Null)
}

pub(super) async fn sign_in(
    p: &Person,
    signed_in_ago_secs: i64,
) -> anyhow::Result<(StatusCode, Value)> {
    let code = AuthorizationCode::new(format!("code-{}", Uuid::new_v4().simple()));
    let pair = pkce_pair();
    oauth_fixture_repository(&p.pool)
        .store_authorization_code(AuthCodeParams {
            code: &code,
            client_id: &p.client.client_id,
            user_id: &p.user,
            redirect_uri: &p.client.redirect_uri,
            scope: "user",
            code_challenge: &pair.challenge,
            resource: None,
        })
        .await?;
    sqlx::query(
        "UPDATE oauth_auth_codes SET created_at = now() - make_interval(secs => $2) \
         WHERE client_id = $1 AND used_at IS NULL",
    )
    .bind(p.client.client_id.as_str())
    .bind(signed_in_ago_secs as f64)
    .execute(p.pool.write_pool().as_ref())
    .await?;
    let (status, text) = send(
        &p.app,
        token_request(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("client_id", p.client.client_id.as_str()),
            ("client_secret", p.client.client_secret.as_str()),
            ("redirect_uri", p.client.redirect_uri.as_str()),
            ("code_verifier", pair.verifier.as_str()),
        ])?,
    )
    .await?;
    Ok((status, json_of(&text)))
}

pub(super) async fn refresh(p: &Person, token: &str) -> anyhow::Result<(StatusCode, Value)> {
    let (status, text) = send(
        &p.app,
        token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", token),
            ("client_id", p.client.client_id.as_str()),
            ("client_secret", p.client.client_secret.as_str()),
        ])?,
    )
    .await?;
    Ok((status, json_of(&text)))
}

pub(super) fn claims(access_token: &str) -> Value {
    let payload = access_token.split('.').nth(1).unwrap_or_default();
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn the_code_grant_of_a_person_ui_client_mints_a_ui_session() -> anyhow::Result<()> {
    let p = person(true).await?;
    let (status, v) = sign_in(&p, 30).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v["expires_in"].as_i64(),
        Some(UI_SESSION_ACCESS_TTL_SECS),
        "{v}"
    );
    let c = claims(v["access_token"].as_str().unwrap_or_default());
    assert_eq!(c["aud"], serde_json::json!(["ui"]), "{c}");
    assert_eq!(c["ui_session"], Value::Bool(true), "{c}");
    assert_eq!(c["client_id"].as_str(), Some(p.client.client_id.as_str()));
    assert!(c.get("purpose").is_none() && c.get("act").is_none(), "{c}");
    let signed_in_for =
        c["iat"].as_i64().unwrap_or_default() - c["auth_time"].as_i64().unwrap_or_default();
    assert!((30..=120).contains(&signed_in_for), "{c}");
    Ok(())
}

#[tokio::test]
async fn the_same_grant_for_any_other_client_is_never_a_ui_session() -> anyhow::Result<()> {
    let p = person(false).await?;
    let (status, v) = sign_in(&p, 30).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    let c = claims(v["access_token"].as_str().unwrap_or_default());
    assert!(c.get("ui_session").is_none(), "{c}");
    assert!(
        !c["aud"]
            .as_array()
            .is_some_and(|a| a.iter().any(|x| x == "ui")),
        "{c}"
    );
    Ok(())
}

#[tokio::test]
async fn refresh_keeps_the_sign_in_time_and_stops_at_the_absolute_cap() -> anyhow::Result<()> {
    let p = person(true).await?;
    let (_, first) = sign_in(&p, 60).await?;
    let auth_time = claims(first["access_token"].as_str().unwrap_or_default())["auth_time"].clone();
    let capped: bool = sqlx::query_scalar(
        "SELECT bool_and(expires_at <= authenticated_at + interval '12 hours') \
         FROM oauth_refresh_tokens WHERE client_id = $1",
    )
    .bind(p.client.client_id.as_str())
    .fetch_one(p.pool.write_pool().as_ref())
    .await?;
    assert!(capped);

    let (status, second) = refresh(&p, first["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::OK, "{second}");
    let c = claims(second["access_token"].as_str().unwrap_or_default());
    assert_eq!(c["auth_time"], auth_time, "{c}");
    assert_eq!(c["ui_session"], Value::Bool(true), "{c}");

    sqlx::query(
        "UPDATE oauth_refresh_tokens SET authenticated_at = now() - interval '13 hours' \
         WHERE client_id = $1 AND consumed_at IS NULL",
    )
    .bind(p.client.client_id.as_str())
    .execute(p.pool.write_pool().as_ref())
    .await?;
    let (status, third) = refresh(&p, second["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{third}");
    Ok(())
}

#[tokio::test]
async fn a_sign_in_older_than_the_absolute_cap_mints_nothing() -> anyhow::Result<()> {
    let p = person(true).await?;
    let (status, v) = sign_in(&p, 13 * 3600).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.get("access_token").is_none(), "{v}");
    Ok(())
}
