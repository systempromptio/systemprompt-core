//! RFC 8693 token exchange, federated-issuer arms — the paths that consult a
//! configured `trusted_issuers` entry (`subject.rs` federated verification and
//! `oidc.rs` ID-JAG issuance).
//!
//! The process installs a Config carrying two trusted issuers, then presents
//! subject tokens signed by the local test authority but bearing a *federated*
//! `iss`. This drives the pre-JWKS validation arms deterministically: `typ`
//! allowlist rejection, missing `kid`, the non-ID-JAG-issuer rejection, and the
//! JWKS-resolution failure (the JWKS URI points at a closed port so the fetch
//! fails fast). The successful federated decode is a declared residual: it
//! needs the `test-jwks-insecure-scheme` feature on `systemprompt-security`
//! (JWKS is HTTPS-only otherwise) plus a live signing JWKS server, and this
//! crate cannot enable that feature.

use std::sync::{Arc, Once};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use axum::middleware::{self, Next};
use jsonwebtoken::{Algorithm, Header, encode};
use systemprompt_api::routes::oauth::public_router;
use systemprompt_identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt_models::Config;
use systemprompt_models::execution::context::RequestContext;
use systemprompt_models::profile::TrustedIssuer;
use systemprompt_oauth::OAuthState;
use systemprompt_security::keys::authority::{active_kid, encoding_key};
use systemprompt_test_fixtures::{
    OAuthClientFixture, ensure_test_bootstrap, fixture_config, fixture_db_pool,
    install_test_signing_key, seed_oauth_client,
};
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;
use uuid::Uuid;

use super::common::setup_ctx;

const TOKEN_EXCHANGE_GRANT: &str = "urn:ietf:params:oauth:grant-type:token-exchange";
const ACCESS_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:access_token";
const ID_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id_token";
const ID_JAG_TOKEN_TYPE: &str = "urn:ietf:params:oauth:token-type:id-jag";

const ISSUER_IDJAG: &str = "https://idp-idjag.example";
const ISSUER_PLAIN: &str = "https://idp-plain.example";
const IDJAG_TYP: &str = "oauth-id-jag+jwt";
const DEAD_JWKS: &str = "https://127.0.0.1:1/jwks";

static CONFIG_INSTALL: Once = Once::new();

fn ensure_config() {
    CONFIG_INSTALL.call_once(|| {
        let mut config = fixture_config("postgres://x");
        config.allowed_resource_audiences = vec!["hook".to_owned()];
        config.trusted_issuers = vec![
            TrustedIssuer {
                issuer: ISSUER_IDJAG.to_owned(),
                jwks_uri: DEAD_JWKS.to_owned(),
                audience: format!("{ISSUER_IDJAG}/aud"),
                typ_allowlist: vec![IDJAG_TYP.to_owned()],
                allowed_client_ids: vec![],
                can_issue_id_jag: true,
            },
            TrustedIssuer {
                issuer: ISSUER_PLAIN.to_owned(),
                jwks_uri: DEAD_JWKS.to_owned(),
                audience: format!("{ISSUER_PLAIN}/aud"),
                typ_allowlist: vec![],
                allowed_client_ids: vec![],
                can_issue_id_jag: false,
            },
        ];
        let _ = Config::install(config);
    });
}

async fn inject_context(mut req: Request<Body>, next: Next) -> Response<Body> {
    req.extensions_mut().insert(RequestContext::new(
        SessionId::generate(),
        TraceId::new("token-exchange-oidc"),
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

async fn seeded_client() -> anyhow::Result<OAuthClientFixture> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let user = UserId::new(Uuid::new_v4().to_string());
    let p = pool.pool_arc().expect("read pool");
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user.as_str())
        .bind(format!("{}@tx-oidc.invalid", user.as_str()))
        .execute(p.as_ref())
        .await?;
    seed_oauth_client(&pool, &user).await
}

fn mint_federated(iss: &str, aud: &str, typ: Option<&str>, with_kid: bool) -> String {
    install_test_signing_key();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64;
    let claims = serde_json::json!({
        "iss": iss,
        "aud": aud,
        "sub": "federated-subject",
        "email": "federated@idp.invalid",
        "iat": now,
        "nbf": now,
        "exp": now + 3600,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.typ = typ.map(str::to_owned);
    if with_kid {
        header.kid = Some(active_kid().expect("active kid").to_owned());
    }
    encode(&header, &claims, encoding_key().expect("encoding key")).expect("encode federated jwt")
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

async fn exchange(
    subject_token: &str,
    subject_token_type: &str,
    requested_token_type: Option<&str>,
) -> anyhow::Result<(http::StatusCode, serde_json::Value)> {
    ensure_config();
    let client = seeded_client().await?;
    let app = token_app().await?;
    let mut pairs = vec![
        ("grant_type", TOKEN_EXCHANGE_GRANT),
        ("subject_token", subject_token),
        ("subject_token_type", subject_token_type),
        ("client_id", client.client_id.as_str()),
        ("client_secret", client.client_secret.as_str()),
    ];
    if let Some(rt) = requested_token_type {
        pairs.push(("requested_token_type", rt));
    }
    let resp = app.oneshot(form_post(urlencode(&pairs))).await?;
    let status = resp.status();
    let v = read_json(resp).await?;
    Ok((status, v))
}

#[tokio::test]
async fn oidc_issue_typ_not_in_allowlist_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_IDJAG,
        &format!("{ISSUER_IDJAG}/aud"),
        Some("JWT"),
        true,
    );
    let (status, v) = exchange(&subject, ID_TOKEN_TYPE, Some(ID_JAG_TOKEN_TYPE)).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn oidc_issue_missing_kid_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_IDJAG,
        &format!("{ISSUER_IDJAG}/aud"),
        Some(IDJAG_TYP),
        false,
    );
    let (status, v) = exchange(&subject, ID_TOKEN_TYPE, Some(ID_JAG_TOKEN_TYPE)).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn oidc_issue_jwks_fetch_failure_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_IDJAG,
        &format!("{ISSUER_IDJAG}/aud"),
        Some(IDJAG_TYP),
        true,
    );
    let (status, v) = exchange(&subject, ID_TOKEN_TYPE, Some(ID_JAG_TOKEN_TYPE)).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    assert!(
        v["error_description"]
            .as_str()
            .unwrap_or_default()
            .contains("JWKS"),
        "expected a JWKS-resolution failure, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn oidc_issue_from_non_idjag_issuer_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_PLAIN,
        &format!("{ISSUER_PLAIN}/aud"),
        Some(IDJAG_TYP),
        true,
    );
    let (status, v) = exchange(&subject, ID_TOKEN_TYPE, Some(ID_JAG_TOKEN_TYPE)).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn oidc_issue_wrong_subject_token_type_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_IDJAG,
        &format!("{ISSUER_IDJAG}/aud"),
        Some(IDJAG_TYP),
        true,
    );
    let (status, v) = exchange(&subject, ACCESS_TOKEN_TYPE, Some(ID_JAG_TOKEN_TYPE)).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn federated_subject_missing_kid_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_PLAIN,
        &format!("{ISSUER_PLAIN}/aud"),
        Some("JWT"),
        false,
    );
    let (status, v) = exchange(&subject, ACCESS_TOKEN_TYPE, None).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}

#[tokio::test]
async fn federated_subject_jwks_fetch_failure_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        ISSUER_PLAIN,
        &format!("{ISSUER_PLAIN}/aud"),
        Some("JWT"),
        true,
    );
    let (status, v) = exchange(&subject, ACCESS_TOKEN_TYPE, None).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    assert!(
        v["error_description"]
            .as_str()
            .unwrap_or_default()
            .contains("JWKS"),
        "expected a JWKS-resolution failure, got {v}"
    );
    Ok(())
}

#[tokio::test]
async fn federated_subject_untrusted_issuer_is_rejected() -> anyhow::Result<()> {
    let subject = mint_federated(
        "https://not-configured.example",
        "https://not-configured.example/aud",
        Some("JWT"),
        true,
    );
    let (status, v) = exchange(&subject, ACCESS_TOKEN_TYPE, None).await?;
    assert!(status.is_client_error(), "{status} {v}");
    assert_eq!(v["error"].as_str(), Some("invalid_request"), "{v}");
    Ok(())
}
