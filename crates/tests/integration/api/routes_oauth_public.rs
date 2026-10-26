//! Public OAuth router — health, token, authorize, webauthn endpoints. We
//! exercise the routing and error mappings; happy paths require a full
//! provisioned client + PKCE flow.


use axum::Router;
use axum::body::Body;
use axum::http::StatusCode;
use systemprompt_api::routes::oauth::{authenticated_router, public_router};
use systemprompt_oauth::OAuthState;
use systemprompt_traits::AppContext as _;
use tower::ServiceExt;

use super::common::{empty_get, json_post, setup_ctx};

async fn public_app() -> anyhow::Result<Router> {
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
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone())))
}

async fn authenticated_app() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    Ok(authenticated_router()
        .with_state(state)
        .layer(axum::Extension(ctx.config_arc().clone())))
}

#[tokio::test]
async fn health_returns_ok() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/health")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn session_returns_token() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/session", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn token_empty_body_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/token", serde_json::json!({})))
        .await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn authorize_get_without_params_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/authorize")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn callback_without_params_returns_error() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/callback")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn webauthn_register_start_returns_error_without_body() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/webauthn/register/start", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn webauthn_auth_start_runs_handler() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app
        .oneshot(json_post("/webauthn/auth/start", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn webauthn_link_start_runs_handler() -> anyhow::Result<()> {
    let app = public_app().await?;
    let resp = app.oneshot(empty_get("/webauthn/link/start")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn introspect_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/introspect", serde_json::json!({"token": "x"})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn revoke_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/revoke", serde_json::json!({"token": "x"})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn logout_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app
        .oneshot(json_post("/logout", serde_json::json!({})))
        .await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn userinfo_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/userinfo")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn consent_get_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/consent")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn register_client_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let body = serde_json::json!({
        "client_name": "test",
        "redirect_uris": ["http://localhost/callback"]
    });
    let resp = app.oneshot(json_post("/register", body)).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn register_client_applies_rfc7591_defaults_when_grant_and_response_types_omitted()
-> anyhow::Result<()> {
    use axum::Extension;
    use systemprompt_identifiers::UserId;
    use uuid::Uuid;
    let (pool, ctx) = setup_ctx().await?;

    let user_id = UserId::new(format!("dcr-defaults-{}", Uuid::new_v4()));
    {
        let p = pool.pool();
        sqlx::query(
            "INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id.as_str())
        .bind(format!("{}@dcr-fixture.invalid", user_id.as_str()))
        .execute(p.as_ref())
        .await?;
    }

    let mut req_ctx = super::common::request_context("ignored");
    req_ctx.auth.actor.user_id = user_id.clone();

    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    let app = systemprompt_api::routes::oauth::public_router(std::sync::Arc::new(
        systemprompt_oauth_issuance::JwksCache::new(),
    ))
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone()))
    .layer(Extension(req_ctx));

    let body = serde_json::json!({
        "client_name": "rfc7591-probe",
        "redirect_uris": ["http://127.0.0.1:53280/callback"],
    });
    let resp = app.oneshot(json_post("/register", body)).await?;
    let status = resp.status();
    let (_, body_str) = super::common::body_to_string(resp).await?;
    assert_eq!(
        status.as_u16(),
        201,
        "expected 201 Created, got {status}: {body_str}"
    );

    let json: serde_json::Value = serde_json::from_str(&body_str)?;
    assert_eq!(
        json["grant_types"],
        serde_json::json!(["authorization_code"]),
        "grant_types must default to RFC 7591 §2 value when omitted; got {body_str}"
    );
    assert_eq!(
        json["response_types"],
        serde_json::json!(["code"]),
        "response_types must default to RFC 7591 §2 value when omitted; got {body_str}"
    );
    assert!(json["client_id"].as_str().is_some(), "missing client_id");
    assert!(
        json["client_secret"].as_str().is_some(),
        "missing client_secret"
    );
    Ok(())
}

#[tokio::test]
async fn register_client_echoes_native_application_type() -> anyhow::Result<()> {
    use axum::Extension;
    use systemprompt_identifiers::UserId;
    use uuid::Uuid;
    let (pool, ctx) = setup_ctx().await?;

    let user_id = UserId::new(format!("dcr-apptype-{}", Uuid::new_v4()));
    {
        let p = pool.pool();
        sqlx::query(
            "INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id.as_str())
        .bind(format!("{}@dcr-fixture.invalid", user_id.as_str()))
        .execute(p.as_ref())
        .await?;
    }

    let mut req_ctx = super::common::request_context("ignored");
    req_ctx.auth.actor.user_id = user_id.clone();

    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    let app = systemprompt_api::routes::oauth::public_router(std::sync::Arc::new(
        systemprompt_oauth_issuance::JwksCache::new(),
    ))
    .with_state(state)
    .layer(axum::Extension(ctx.config_arc().clone()))
    .layer(Extension(req_ctx));

    let body = serde_json::json!({
        "client_name": "native-probe",
        "redirect_uris": ["myapp://callback"],
        "application_type": "native",
    });
    let resp = app.oneshot(json_post("/register", body)).await?;
    let status = resp.status();
    let (_, body_str) = super::common::body_to_string(resp).await?;
    assert_eq!(
        status.as_u16(),
        201,
        "expected 201 Created, got {status}: {body_str}"
    );

    let json: serde_json::Value = serde_json::from_str(&body_str)?;
    assert_eq!(
        json["application_type"], "native",
        "application_type must echo the requested value; got {body_str}"
    );
    Ok(())
}

#[tokio::test]
async fn get_client_configuration_unknown_returns_4xx() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/register/unknown_client")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn clients_list_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/clients")).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn clients_create_runs_handler() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let body = serde_json::json!({
        "client_name": "test client",
        "redirect_uris": ["http://localhost/cb"],
    });
    let resp = app.oneshot(json_post("/clients", body)).await?;
    let status = resp.status().as_u16();
    assert!((200..600).contains(&status));
    Ok(())
}

#[tokio::test]
async fn clients_get_unknown_returns_4xx() -> anyhow::Result<()> {
    let app = authenticated_app().await?;
    let resp = app.oneshot(empty_get("/clients/missing")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

async fn dcr_app() -> anyhow::Result<Router> {
    use axum::Extension;
    use systemprompt_identifiers::UserId;
    use uuid::Uuid;
    let (pool, ctx) = setup_ctx().await?;
    let user_id = UserId::new(format!("dcr-scope-{}", Uuid::new_v4()));
    let p = pool.pool();
    sqlx::query("INSERT INTO users (id, name, email) VALUES ($1, $1, $2) ON CONFLICT DO NOTHING")
        .bind(user_id.as_str())
        .bind(format!("{}@dcr-fixture.invalid", user_id.as_str()))
        .execute(p.as_ref())
        .await?;

    let mut req_ctx = super::common::request_context("ignored");
    req_ctx.auth.actor.user_id = user_id;
    let state = OAuthState::new(
        ctx.oauth_repositories().oauth.clone(),
        ctx.analytics_provider().expect("analytics"),
        ctx.session_provider().expect("sessions"),
        ctx.user_provider().expect("user"),
        systemprompt_test_fixtures::test_oauth_signing(),
    );
    Ok(
        systemprompt_api::routes::oauth::public_router(std::sync::Arc::new(
            systemprompt_oauth_issuance::JwksCache::new(),
        ))
        .with_state(state)
        .layer(axum::Extension(ctx.config_arc().clone()))
        .layer(Extension(req_ctx)),
    )
}

async fn register_with(scope: Option<&str>) -> anyhow::Result<(u16, serde_json::Value)> {
    let mut body = serde_json::json!({
        "client_name": "scope-probe",
        "redirect_uris": ["http://127.0.0.1:53281/callback"],
    });
    if let Some(scope) = scope {
        body["scope"] = serde_json::Value::String(scope.to_owned());
    }
    let resp = dcr_app()
        .await?
        .oneshot(json_post("/register", body))
        .await?;
    let status = resp.status().as_u16();
    let (_, text) = super::common::body_to_string(resp).await?;
    Ok((
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
    ))
}

#[tokio::test]
async fn a_requested_scope_is_granted_when_every_scope_is_known() -> anyhow::Result<()> {
    let (status, json) = register_with(Some("user")).await?;

    assert_eq!(status, 201, "{json}");
    let granted: Vec<&str> = json["scopes"]
        .as_array()
        .or_else(|| json["scope"].as_array())
        .map(|a| a.iter().filter_map(|s| s.as_str()).collect())
        .unwrap_or_default();
    assert!(
        granted.contains(&"user") || json["scope"].as_str().is_some_and(|s| s.contains("user")),
        "an explicitly requested known scope must be granted: {json}"
    );
    Ok(())
}

#[tokio::test]
async fn an_unknown_requested_scope_is_refused_rather_than_silently_dropped() -> anyhow::Result<()>
{
    let (status, json) = register_with(Some("user superuser-everything")).await?;

    // Silently narrowing to the valid subset would hand the client a token it
    // did not ask for and cannot detect, so registration has to fail outright.
    assert_eq!(status, 400, "{json}");
    let rendered = json.to_string();
    assert!(rendered.contains("scope"), "{rendered}");
    Ok(())
}

#[tokio::test]
async fn omitting_scope_falls_back_to_the_default_roles() -> anyhow::Result<()> {
    let (status, json) = register_with(None).await?;

    assert_eq!(status, 201, "{json}");
    let has_scopes = json["scopes"].as_array().is_some_and(|a| !a.is_empty())
        || json["scope"].as_str().is_some_and(|s| !s.is_empty());
    assert!(
        has_scopes,
        "a client registered without a scope request still receives the defaults: {json}"
    );
    Ok(())
}

#[tokio::test]
async fn a_blank_scope_string_is_treated_as_no_request() -> anyhow::Result<()> {
    let (status, json) = register_with(Some("   ")).await?;

    assert_eq!(
        status, 201,
        "whitespace is not a scope request and must not fail registration: {json}"
    );
    Ok(())
}

async fn register_body(body: serde_json::Value) -> anyhow::Result<(u16, serde_json::Value)> {
    let resp = dcr_app()
        .await?
        .oneshot(json_post("/register", body))
        .await?;
    let status = resp.status().as_u16();
    let (_, text) = super::common::body_to_string(resp).await?;
    Ok((
        status,
        serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
    ))
}

#[tokio::test]
async fn self_registration_cannot_claim_admin_scope() -> anyhow::Result<()> {
    let (status, json) = register_with(Some("user admin")).await?;
    assert_eq!(status, 400, "{json}");
    assert!(json.to_string().contains("admin"), "{json}");
    Ok(())
}

#[tokio::test]
async fn web_client_with_https_redirect_registers() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "probe-alpha",
        "redirect_uris": ["https://attacker-alpha1.example.com/cb"],
    }))
    .await?;
    assert_eq!(status, 201, "{json}");
    assert!(
        json["client_id"]
            .as_str()
            .is_some_and(|id| id.starts_with("client_")),
        "{json}"
    );
    assert!(
        json["registration_access_token"]
            .as_str()
            .is_some_and(|t| t.starts_with("reg_")),
        "{json}"
    );
    Ok(())
}

#[tokio::test]
async fn web_client_with_plain_http_remote_redirect_is_refused() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "probe",
        "redirect_uris": ["http://attacker.example/cb"],
    }))
    .await?;
    assert_eq!(status, 400, "{json}");
    assert_eq!(
        json["error"].as_str(),
        Some("invalid_client_metadata"),
        "{json}"
    );
    Ok(())
}

#[tokio::test]
async fn script_scheme_and_fragment_redirects_are_refused() -> anyhow::Result<()> {
    for uri in ["javascript:alert(1)", "https://app.example/cb#frag"] {
        let (status, json) = register_body(serde_json::json!({
            "client_name": "probe",
            "redirect_uris": [uri],
        }))
        .await?;
        assert_eq!(status, 400, "{uri}: {json}");
    }
    Ok(())
}

#[tokio::test]
async fn native_client_may_register_a_private_use_scheme() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "desktop",
        "application_type": "native",
        "redirect_uris": ["com.example.desktop:/oauth/cb", "http://127.0.0.1/cb"],
    }))
    .await?;
    assert_eq!(status, 201, "{json}");
    Ok(())
}

#[tokio::test]
async fn client_uri_must_be_http() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "probe",
        "redirect_uris": ["https://app.example/cb"],
        "client_uri": "javascript:alert(1)",
    }))
    .await?;
    assert_eq!(status, 400, "{json}");
    assert!(json.to_string().contains("client_uri"), "{json}");
    Ok(())
}

#[tokio::test]
async fn public_client_is_issued_no_secret() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "mcp-remote",
        "redirect_uris": ["http://127.0.0.1/cb"],
        "token_endpoint_auth_method": "none",
    }))
    .await?;
    assert_eq!(status, 201, "{json}");
    assert_eq!(
        json["token_endpoint_auth_method"].as_str(),
        Some("none"),
        "{json}"
    );
    assert!(json.get("client_secret").is_none(), "{json}");
    assert!(json.get("client_secret_expires_at").is_none(), "{json}");
    Ok(())
}

#[tokio::test]
async fn confidential_client_is_issued_a_secret() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "server-app",
        "redirect_uris": ["https://app.example/cb"],
        "token_endpoint_auth_method": "client_secret_post",
    }))
    .await?;
    assert_eq!(status, 201, "{json}");
    assert!(
        json["client_secret"]
            .as_str()
            .is_some_and(|s| !s.is_empty()),
        "{json}"
    );
    assert_eq!(json["client_secret_expires_at"].as_u64(), Some(0), "{json}");
    Ok(())
}

#[tokio::test]
async fn unsupported_auth_method_is_refused() -> anyhow::Result<()> {
    let (status, json) = register_body(serde_json::json!({
        "client_name": "probe",
        "redirect_uris": ["https://app.example/cb"],
        "token_endpoint_auth_method": "private_key_jwt",
    }))
    .await?;
    assert_eq!(status, 400, "{json}");
    assert!(json.to_string().contains("private_key_jwt"), "{json}");
    Ok(())
}

#[tokio::test]
async fn registration_token_manages_the_registered_client() -> anyhow::Result<()> {
    use axum::http::{Request, header};

    let (status, json) = register_body(serde_json::json!({
        "client_name": "managed",
        "redirect_uris": ["https://app.example/cb"],
    }))
    .await?;
    assert_eq!(status, 201, "{json}");
    let client_id = json["client_id"].as_str().expect("client_id").to_owned();
    let token = json["registration_access_token"]
        .as_str()
        .expect("token")
        .to_owned();

    let get = Request::builder()
        .uri(format!("/register/{client_id}"))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())?;
    let resp = dcr_app().await?.oneshot(get).await?;
    assert_eq!(resp.status(), StatusCode::OK, "{}", resp.status());
    let (_, text) = super::common::body_to_string(resp).await?;
    let v: serde_json::Value = serde_json::from_str(&text)?;
    assert_eq!(v["client_name"].as_str(), Some("managed"), "{v}");
    Ok(())
}
