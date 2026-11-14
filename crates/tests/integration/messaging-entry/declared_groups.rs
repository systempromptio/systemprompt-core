//! The Slack and Teams route groups on the real router of an all-role
//! messaging node, mounted from their entry crates' extension declarations.
//! Both groups are handler-verified: they carry no context middleware or
//! gate, so each handler's own verifier is exercised and must refuse an
//! unsigned or mis-signed Slack request and a Teams activity without a valid
//! Bot Connector token. Each assertion names a status or body only that
//! verifier produces, so it fails if the verification is removed. With both
//! extensions disabled in the profile the groups are not mounted at all.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_extension::ExtensionRegistry;
use systemprompt_identifiers::ExtensionId;
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_models::modules::ApiPaths;
use systemprompt_slack::signature::sign;
use systemprompt_test_fixtures::{
    TEST_TEAMS_TENANT_ID, ensure_messaging_bootstrap, fixture_app_context_with_extensions,
    fixture_config, test_app_context_on, test_db_pool,
};
use tower::ServiceExt;

async fn messaging_role() -> Result<Router> {
    let bootstrap = ensure_messaging_bootstrap();
    let pool = test_db_pool().await;
    let ctx = test_app_context_on(&bootstrap, &pool);
    setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn messaging_disabled() -> Result<Router> {
    let bootstrap = ensure_messaging_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
    config.rate_limits = RateLimitsConfig::disabled();
    let ctx = fixture_app_context_with_extensions(
        &pool,
        config,
        ExtensionRegistry::discover()?,
        &[ExtensionId::new("slack"), ExtensionId::new("teams")],
    )?;
    setup_api_server(&ctx, None, &Arc::new(ReadinessState::new()))
        .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))
}

async fn send(
    app: &Router,
    uri: &str,
    extra: &[(&str, &str)],
    body: String,
) -> Result<(StatusCode, String)> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::HOST, "127.0.0.1")
        .header(header::USER_AGENT, "declared-groups-test/1.0")
        .header(header::CONTENT_TYPE, "application/json");
    for (name, value) in extra {
        builder = builder.header(*name, *value);
    }
    let mut req = builder.body(Body::from(body))?;
    req.extensions_mut()
        .insert(ConnectInfo("203.0.113.9:40000".parse::<SocketAddr>()?));
    let resp = app.clone().oneshot(req).await?;
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024).await?;
    Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
}

#[tokio::test]
async fn slack_refuses_an_unsigned_or_mis_signed_event() -> Result<()> {
    let app = messaging_role().await?;
    let uri = format!("{}/events", ApiPaths::SLACK_BASE);
    let body = r#"{"type":"url_verification","challenge":"declared-chal"}"#;
    let (status, text) = send(&app, &uri, &[], body.to_owned()).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "unsigned: {text}");

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        .to_string();
    let signature = sign(b"the-wrong-secret", &timestamp, body.as_bytes()).expect("sign");
    let (status, text) = send(
        &app,
        &uri,
        &[
            ("x-slack-request-timestamp", timestamp.as_str()),
            ("x-slack-signature", signature.as_str()),
        ],
        body.to_owned(),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "mis-signed: {text}");
    assert!(!text.contains("declared-chal"), "{text}");
    Ok(())
}

#[tokio::test]
async fn teams_refuses_an_activity_without_a_valid_bot_token() -> Result<()> {
    let app = messaging_role().await?;
    let uri = format!("{}/messages", ApiPaths::TEAMS_BASE);
    let activity = serde_json::json!({
        "type": "message",
        "id": "declared-act",
        "serviceUrl": "https://smba.example",
        "text": "hello",
        "from": { "id": "29:user" },
        "conversation": { "id": "declared-conv", "tenantId": TEST_TEAMS_TENANT_ID },
    })
    .to_string();
    for extra in [vec![], vec![("authorization", "Bearer not-a-jwt")]] {
        let (status, text) = send(&app, &uri, &extra, activity.clone()).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{extra:?}: {text}");
    }
    Ok(())
}

#[tokio::test]
async fn disabled_messaging_extensions_are_not_mounted() -> Result<()> {
    let app = messaging_disabled().await?;
    for uri in [
        format!("{}/events", ApiPaths::SLACK_BASE),
        format!("{}/messages", ApiPaths::TEAMS_BASE),
    ] {
        let (status, text) = send(&app, &uri, &[], "{}".to_owned()).await?;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}: {text}");
    }
    Ok(())
}
