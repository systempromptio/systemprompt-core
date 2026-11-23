//! The route groups declared through the one mount, on the real router of an
//! all-role node. The user-gated groups (documents, uploads, folders, brain)
//! refuse a caller with no session; the admin brain group refuses a user
//! token at the admin gate. Every handler-verified group is mounted with no
//! context middleware or gate, so each one's own verifier is exercised here
//! and must refuse: the gateway credential, the managed device credential,
//! the MCP hop token and the upload token. Each assertion names a status or
//! body only that verifier produces, so it fails if the verification is
//! removed. The Slack and Teams groups are declared by their entry crates and
//! refused in `systemprompt-messaging-entry-integration-tests`.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::Router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use systemprompt_api::services::server::{ReadinessState, setup_api_server};
use systemprompt_database::DbPool;
use systemprompt_identifiers::{DocumentUploadId, JwtToken};
use systemprompt_manifest::profile::{NodeRole, RateLimitsConfig};
use systemprompt_models::modules::ApiPaths;
use systemprompt_runtime::documents::links::LinkTarget;
use systemprompt_runtime::documents::upload_links::UPLOAD_TOKEN_HEADER;
use systemprompt_test_fixtures::{
    ensure_test_bootstrap, fixture_app_context_with_config, fixture_config, test_db_pool,
};
use tower::ServiceExt;

async fn all_role() -> Result<(DbPool, Router)> {
    let bootstrap = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let mut config = fixture_config(&bootstrap.database_url);
    config.role = NodeRole::All;
    config.rate_limits = RateLimitsConfig::disabled();
    config.trusted_proxies = Vec::new();
    let ctx = fixture_app_context_with_config(config).await?;
    let router = setup_api_server(
        &ctx,
        systemprompt_api::services::server::routes::Composed {
            admitted: ctx.admitted(),
            kernel: ctx.kernel(),
        },
        None,
        &Arc::new(ReadinessState::new()),
    )
    .map_err(|e| anyhow::anyhow!("setup_api_server failed: {e}"))?;
    Ok((pool, router))
}

async fn send(
    app: &Router,
    method: &str,
    uri: &str,
    extra: &[(&str, &str)],
    body: String,
) -> Result<(StatusCode, String)> {
    let mut builder = Request::builder()
        .method(method)
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

async fn status(
    app: &Router,
    method: &str,
    uri: &str,
    bearer: Option<&JwtToken>,
) -> Result<StatusCode> {
    let authorization = bearer.map(|token| format!("Bearer {}", token.as_str()));
    let extra: Vec<(&str, &str)> = authorization
        .as_deref()
        .map(|value| vec![(header::AUTHORIZATION.as_str(), value)])
        .unwrap_or_default();
    Ok(send(app, method, uri, &extra, "{}".to_owned()).await?.0)
}

#[tokio::test]
async fn the_document_groups_refuse_a_caller_without_a_session_at_the_user_gate() -> Result<()> {
    let (_pool, app) = all_role().await?;
    for (method, uri) in [
        ("GET", ApiPaths::DOCUMENTS.to_owned()),
        ("GET", ApiPaths::DOCUMENT_FOLDERS.to_owned()),
        ("POST", ApiPaths::UPLOADS.to_owned()),
    ] {
        assert_eq!(
            status(&app, method, &uri, None).await?,
            StatusCode::UNAUTHORIZED,
            "{method} {uri}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn the_handler_verified_gateway_refuses_a_caller_without_a_credential() -> Result<()> {
    let (_pool, app) = all_role().await?;
    let uri = format!("{}/otel", ApiPaths::GATEWAY_BASE);
    assert_eq!(
        status(&app, "POST", &uri, None).await?,
        StatusCode::UNAUTHORIZED
    );
    Ok(())
}

#[tokio::test]
async fn the_managed_consumer_refuses_a_caller_without_a_device_credential() -> Result<()> {
    let (_pool, app) = all_role().await?;
    let uri = format!(
        "/api/v1/consumer/resources/{}/publications/{}/bundle?host=claude-code",
        uuid::Uuid::new_v4(),
        uuid::Uuid::new_v4()
    );
    for extra in [
        vec![],
        vec![("authorization", "Bearer not-a-device-credential")],
    ] {
        let (status, body) = send(&app, "GET", &uri, &extra, String::new()).await?;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{extra:?}: {body}");
        assert!(
            body.contains("enrolled-device authentication"),
            "{extra:?}: {body}"
        );
    }
    Ok(())
}

#[tokio::test]
async fn internal_links_refuse_a_request_without_a_hop_token() -> Result<()> {
    let (_pool, app) = all_role().await?;
    let body = serde_json::to_string(&serde_json::json!({
        "server": "declared-groups-srv",
        "target": LinkTarget::Upload { upload_id: DocumentUploadId::generate() },
    }))?;
    let (status, text) = send(&app, "POST", ApiPaths::INTERNAL_LINKS, &[], body).await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{text}");
    assert!(text.contains("hop token"), "{text}");
    Ok(())
}

#[tokio::test]
async fn the_upload_token_gate_refuses_an_invalid_token() -> Result<()> {
    let (_pool, app) = all_role().await?;
    let uri = format!(
        "{}/{}/content",
        ApiPaths::UPLOADS,
        DocumentUploadId::generate()
    );
    let (status, body) = send(
        &app,
        "GET",
        &uri,
        &[(UPLOAD_TOKEN_HEADER, "not-an-upload-token")],
        String::new(),
    )
    .await?;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert!(body.contains("upload link is invalid"), "{body}");
    Ok(())
}
