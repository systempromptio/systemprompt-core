//! A person's UI session over the real router: the code + PKCE grant of a
//! client the operator declared `person_ui` mints a short `ui`-audience token
//! carrying `ui_session` and the sign-in time, the same grant for any other
//! client never does, and refreshing it keeps the sign-in time and stops at the
//! absolute cap. The cap counts from the session start (the code's issue), the
//! `auth_time` is when the person signed in, and a refresh re-stamps neither,
//! for the `sp_web` client as for a UI session. The lifetimes are the profile's
//! `security.ui_session` block.

use std::net::SocketAddr;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ConnectInfo;
use axum::http::{Method, Request, StatusCode, header};
use base64::Engine;
use serde_json::Value;
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt_manifest::profile::{RateLimitsConfig, UiSessionConfig};
use systemprompt_oauth::repository::{AuthCodeParams, ClientRepository, CreateClientParams};
use systemprompt_test_fixtures::{
    OAuthClientFixture, TEST_CLIENT_SECRET, TEST_CLIENT_SECRET_HASH, TEST_REDIRECT_URI,
    ensure_test_bootstrap, fixture_config,
    oauth_fixture_repository, pkce_pair, seed_oauth_client, test_db_pool,
};
use tower::ServiceExt;
use uuid::Uuid;

pub(super) struct Person {
    pub(super) pool: DbPool,
    pub(super) app: Router,
    pub(super) user: UserId,
    pub(super) client: OAuthClientFixture,
    pub(super) ui_session: UiSessionConfig,
}

pub(super) async fn person(person_ui: bool) -> anyhow::Result<Person> {
    person_with(person_ui, UiSessionConfig::default()).await
}

pub(super) async fn person_with(
    person_ui: bool,
    ui_session: UiSessionConfig,
) -> anyhow::Result<Person> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    config.ui_session = ui_session;
    let ctx = systemprompt_test_fixtures::fixture_app_context_with_extensions(
        config,
        super::common::reference_admitted(&[])?,
    )
    .await?;
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
    let client = client_of(&pool, &user, person_ui).await?;
    Ok(Person {
        pool,
        app,
        user,
        client,
        ui_session,
    })
}

async fn client_of(
    pool: &DbPool,
    user: &UserId,
    person_ui: bool,
) -> anyhow::Result<OAuthClientFixture> {
    let client = seed_oauth_client(pool, user).await?;
    if person_ui {
        sqlx::query(
            "UPDATE oauth_clients SET person_ui = true, declared_source = 'identity/desk.yaml' \
             WHERE client_id = $1",
        )
        .bind(client.client_id.as_str())
        .execute(pool.write_pool().as_ref())
        .await?;
    }
    Ok(client)
}

pub(super) async fn same_person_through_another_client(
    p: &Person,
    person_ui: bool,
) -> anyhow::Result<Person> {
    Ok(Person {
        pool: p.pool.clone(),
        app: p.app.clone(),
        user: p.user.clone(),
        client: client_of(&p.pool, &p.user, person_ui).await?,
        ui_session: p.ui_session,
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

#[derive(Clone, Copy)]
pub(super) struct SignIn {
    pub(super) code_issued_ago_secs: i64,
    pub(super) authenticated_ago_secs: i64,
}

pub(super) async fn sign_in(
    p: &Person,
    signed_in_ago_secs: i64,
) -> anyhow::Result<(StatusCode, Value)> {
    sign_in_at(
        p,
        SignIn {
            code_issued_ago_secs: signed_in_ago_secs,
            authenticated_ago_secs: signed_in_ago_secs,
        },
    )
    .await
}

pub(super) async fn sign_in_at(p: &Person, times: SignIn) -> anyhow::Result<(StatusCode, Value)> {
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
            authenticated_at: chrono::Utc::now()
                - chrono::Duration::seconds(times.authenticated_ago_secs),
        })
        .await?;
    sqlx::query(
        "UPDATE oauth_auth_codes SET created_at = now() - make_interval(secs => $2) \
         WHERE client_id = $1 AND used_at IS NULL",
    )
    .bind(p.client.client_id.as_str())
    .bind(times.code_issued_ago_secs as f64)
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
    assert_eq!(p.ui_session.access_ttl_secs, 300);
    assert_eq!(
        v["expires_in"].as_i64(),
        Some(p.ui_session.access_ttl_secs),
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
        "SELECT bool_and(expires_at <= session_started_at + make_interval(secs => $2)) \
         FROM oauth_refresh_tokens WHERE client_id = $1",
    )
    .bind(p.client.client_id.as_str())
    .bind(p.ui_session.absolute_secs as f64)
    .fetch_one(p.pool.write_pool().as_ref())
    .await?;
    assert!(capped);

    let (status, second) = refresh(&p, first["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::OK, "{second}");
    let c = claims(second["access_token"].as_str().unwrap_or_default());
    assert_eq!(c["auth_time"], auth_time, "{c}");
    assert_eq!(c["ui_session"], Value::Bool(true), "{c}");

    sqlx::query(
        "UPDATE oauth_refresh_tokens SET session_started_at = now() - make_interval(secs => $2) \
         WHERE client_id = $1 AND consumed_at IS NULL",
    )
    .bind(p.client.client_id.as_str())
    .bind((p.ui_session.absolute_secs + 3600) as f64)
    .execute(p.pool.write_pool().as_ref())
    .await?;
    let (status, third) = refresh(&p, second["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{third}");
    Ok(())
}

#[tokio::test]
async fn a_sign_in_older_than_the_absolute_cap_mints_nothing() -> anyhow::Result<()> {
    let p = person(true).await?;
    let (status, v) = sign_in(&p, p.ui_session.absolute_secs + 3600).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{v}");
    assert!(v.get("access_token").is_none(), "{v}");
    Ok(())
}

#[tokio::test]
async fn the_configured_lifetimes_bound_the_minted_session() -> anyhow::Result<()> {
    let p = person_with(
        true,
        UiSessionConfig {
            access_ttl_secs: 120,
            absolute_secs: 600,
            step_up_max_age_secs: 60,
        },
    )
    .await?;
    let (status, v) = sign_in(&p, 30).await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(v["expires_in"].as_i64(), Some(120), "{v}");
    let c = claims(v["access_token"].as_str().unwrap_or_default());
    assert_eq!(
        c["exp"].as_i64().unwrap_or_default() - c["iat"].as_i64().unwrap_or_default(),
        120,
        "{c}"
    );
    let refresh_lifetime: f64 = sqlx::query_scalar(
        "SELECT extract(epoch FROM max(expires_at - session_started_at))::float8 \
         FROM oauth_refresh_tokens WHERE client_id = $1",
    )
    .bind(p.client.client_id.as_str())
    .fetch_one(p.pool.write_pool().as_ref())
    .await?;
    assert!(
        (599.0..=600.0).contains(&refresh_lifetime),
        "{refresh_lifetime}"
    );

    let (status, near_the_end) = sign_in(&p, 540).await?;
    assert_eq!(status, StatusCode::OK, "{near_the_end}");
    let left = near_the_end["expires_in"].as_i64().unwrap_or_default();
    assert!((1..=60).contains(&left), "{near_the_end}");

    let (status, past_the_end) = sign_in(&p, 700).await?;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{past_the_end}");
    assert!(past_the_end.get("access_token").is_none(), "{past_the_end}");
    Ok(())
}

const TEN_DAYS_SECS: i64 = 864_000;

fn signed_in_for(c: &Value) -> i64 {
    c["iat"].as_i64().unwrap_or_default() - c["auth_time"].as_i64().unwrap_or_default()
}

#[tokio::test]
async fn an_old_sign_in_still_mints_but_carries_its_real_auth_time() -> anyhow::Result<()> {
    let p = person(true).await?;
    assert!(TEN_DAYS_SECS > p.ui_session.absolute_secs);
    let (status, v) = sign_in_at(
        &p,
        SignIn {
            code_issued_ago_secs: 0,
            authenticated_ago_secs: TEN_DAYS_SECS,
        },
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{v}");
    assert_eq!(
        v["expires_in"].as_i64(),
        Some(p.ui_session.access_ttl_secs),
        "{v}"
    );
    let c = claims(v["access_token"].as_str().unwrap_or_default());
    assert_eq!(c["ui_session"], Value::Bool(true), "{c}");
    assert!(
        (TEN_DAYS_SECS..=TEN_DAYS_SECS + 60).contains(&signed_in_for(&c)),
        "{c}"
    );
    let (session_left, signed_in_before_start): (f64, f64) = sqlx::query_as(
        "SELECT extract(epoch FROM expires_at - session_started_at)::float8, \
                extract(epoch FROM session_started_at - authenticated_at)::float8 \
         FROM oauth_refresh_tokens WHERE client_id = $1",
    )
    .bind(p.client.client_id.as_str())
    .fetch_one(p.pool.write_pool().as_ref())
    .await?;
    let cap = p.ui_session.absolute_secs as f64;
    assert!((cap - 1.0..=cap).contains(&session_left), "{session_left}");
    let ten_days = TEN_DAYS_SECS as f64;
    assert!(
        (ten_days - 1.0..=ten_days + 60.0).contains(&signed_in_before_start),
        "{signed_in_before_start}"
    );
    Ok(())
}

#[tokio::test]
async fn refresh_keeps_both_times() -> anyhow::Result<()> {
    let p = person(true).await?;
    let (status, first) = sign_in_at(
        &p,
        SignIn {
            code_issued_ago_secs: 120,
            authenticated_ago_secs: 3600,
        },
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{first}");
    let auth_time = claims(first["access_token"].as_str().unwrap_or_default())["auth_time"].clone();

    let (status, second) = refresh(&p, first["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::OK, "{second}");
    let c = claims(second["access_token"].as_str().unwrap_or_default());
    assert_eq!(c["auth_time"], auth_time, "{c}");
    assert!((3600..=3660).contains(&signed_in_for(&c)), "{c}");

    let (tokens, distinct_times, apart): (i64, i64, f64) = sqlx::query_as(
        "SELECT count(*), count(DISTINCT (session_started_at, authenticated_at)), \
                extract(epoch FROM max(session_started_at - authenticated_at))::float8 \
         FROM oauth_refresh_tokens WHERE client_id = $1",
    )
    .bind(p.client.client_id.as_str())
    .fetch_one(p.pool.write_pool().as_ref())
    .await?;
    assert_eq!((tokens, distinct_times), (2, 1));
    assert!((3470.0..=3490.0).contains(&apart), "{apart}");
    Ok(())
}

async fn the_web_client(p: &Person) -> anyhow::Result<Person> {
    let client_id = ClientId::web();
    sqlx::query("DELETE FROM oauth_clients WHERE client_id = $1")
        .bind(client_id.as_str())
        .execute(p.pool.write_pool().as_ref())
        .await?;
    ClientRepository::new(&p.pool)
        .create(CreateClientParams {
            client_id: client_id.clone(),
            owner_user_id: p.user.clone(),
            client_secret_hash: Some(TEST_CLIENT_SECRET_HASH.to_owned()),
            registration_token_hash: None,
            client_name: "web".to_owned(),
            redirect_uris: vec![TEST_REDIRECT_URI.to_owned()],
            grant_types: Some(vec![
                "authorization_code".to_owned(),
                "refresh_token".to_owned(),
            ]),
            response_types: Some(vec!["code".to_owned()]),
            scopes: vec!["user".to_owned()],
            token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
            application_type: "web".to_owned(),
            client_uri: None,
            logo_uri: None,
            contacts: None,
        })
        .await
        .map_err(|e| anyhow::anyhow!("create the web client: {e}"))?;
    Ok(Person {
        pool: p.pool.clone(),
        app: p.app.clone(),
        user: p.user.clone(),
        client: OAuthClientFixture {
            client_id,
            client_secret: TEST_CLIENT_SECRET.to_owned(),
            redirect_uri: TEST_REDIRECT_URI.to_owned(),
        },
        ui_session: p.ui_session,
    })
}

#[tokio::test]
async fn a_web_client_refresh_keeps_the_original_sign_in_time() -> anyhow::Result<()> {
    let web = the_web_client(&person(false).await?).await?;
    let (status, first) = sign_in_at(
        &web,
        SignIn {
            code_issued_ago_secs: 0,
            authenticated_ago_secs: TEN_DAYS_SECS,
        },
    )
    .await?;
    assert_eq!(status, StatusCode::OK, "{first}");
    let minted = claims(first["access_token"].as_str().unwrap_or_default());
    assert_eq!(minted["browser_session"], Value::Bool(true), "{minted}");
    assert!(minted.get("ui_session").is_none(), "{minted}");
    assert!(
        (TEN_DAYS_SECS..=TEN_DAYS_SECS + 60).contains(&signed_in_for(&minted)),
        "{minted}"
    );

    let (status, second) =
        refresh(&web, first["refresh_token"].as_str().unwrap_or_default()).await?;
    assert_eq!(status, StatusCode::OK, "{second}");
    let refreshed = claims(second["access_token"].as_str().unwrap_or_default());
    assert_eq!(
        refreshed["browser_session"],
        Value::Bool(true),
        "{refreshed}"
    );
    assert_eq!(refreshed["auth_time"], minted["auth_time"], "{refreshed}");

    sqlx::query("DELETE FROM oauth_clients WHERE client_id = $1")
        .bind(web.client.client_id.as_str())
        .execute(web.pool.write_pool().as_ref())
        .await?;
    Ok(())
}
