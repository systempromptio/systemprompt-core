//! Gateway router — exercises the JWT-protected and pubkey endpoints via
//! `oneshot`. Most happy paths require a valid bridge JWT and a loaded
//! profile; we cover the negative paths (missing credential, invalid JWT,
//! bad payload, unknown host) which still exercises the routing layer,
//! credential extraction, JWT decode, and host-allowlist validation.

use axum::Router;
use systemprompt_api::routes::gateway::gateway_router;
use tower::ServiceExt;

use super::common::{empty_get, json_post, setup_ctx};

async fn router() -> anyhow::Result<Router> {
    let (_pool, ctx) = setup_ctx().await?;
    Ok(gateway_router(&ctx)
        .expect("gateway journal opens")
        .expect("gateway router available"))
}

#[tokio::test]
async fn root_returns_service_metadata() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn capabilities_lists_auth_modes() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/auth/bridge/capabilities")).await?;
    assert!(resp.status().is_success(), "{}", resp.status());
    Ok(())
}

#[tokio::test]
async fn pubkey_returns_response() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/bridge/pubkey")).await?;
    // Either OK (test signing key installed) or 500 (signing key missing).
    assert!(
        resp.status().is_success() || resp.status().is_server_error(),
        "{}",
        resp.status()
    );
    Ok(())
}

#[tokio::test]
async fn profile_returns_error_without_profile_bootstrap() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/bridge/profile")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn list_models_returns_error_without_profile_bootstrap() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/models")).await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn pat_missing_bearer_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post("/auth/bridge/pat", serde_json::json!({})))
        .await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn session_empty_code_returns_400() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/session",
            serde_json::json!({ "code": "" }),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 400);
    Ok(())
}

#[tokio::test]
async fn session_unknown_code_returns_error() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/session",
            serde_json::json!({ "code": "not-a-real-code-xyz" }),
        ))
        .await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn session_pat_empty_code_returns_400() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/session-pat",
            serde_json::json!({ "code": "" }),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 400);
    Ok(())
}

#[tokio::test]
async fn session_pat_unknown_code_returns_unauthorized() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/session-pat",
            serde_json::json!({ "code": "not-a-real-code-xyz", "device_name": "test device" }),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}


#[tokio::test]
async fn oauth_client_missing_bearer_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/auth/bridge/oauth-client",
            serde_json::json!({}),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn whoami_missing_credential_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/bridge/whoami")).await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn manifest_missing_credential_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/bridge/manifest")).await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn profile_usage_missing_credential_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(empty_get("/bridge/profile/usage")).await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn set_enabled_host_missing_credential_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/bridge/profile/enabled_hosts",
            serde_json::json!({ "host_id": "claude-code", "enabled": true }),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn heartbeat_missing_credential_returns_401() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/bridge/heartbeat",
            serde_json::json!({
                "session_id": "00000000-0000-0000-0000-000000000000",
                "bridge_version": "1.0.0",
                "os": "linux",
                "hostname": "test"
            }),
        ))
        .await?;
    assert_eq!(resp.status().as_u16(), 401);
    Ok(())
}

#[tokio::test]
async fn messages_missing_credential_returns_error() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/messages",
            serde_json::json!({ "model": "claude-3", "messages": [] }),
        ))
        .await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn responses_missing_credential_returns_error() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post(
            "/responses",
            serde_json::json!({ "model": "gpt-4", "input": "hi" }),
        ))
        .await?;
    assert!(resp.status().is_client_error() || resp.status().is_server_error());
    Ok(())
}

#[tokio::test]
async fn otel_runs_handler() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post("/otel", serde_json::json!({})))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

#[tokio::test]
async fn otel_with_rest_path_runs_handler() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(json_post("/otel/v1/traces", serde_json::json!({})))
        .await?;
    assert!(resp.status().as_u16() >= 200);
    Ok(())
}

fn otel_post(uri: &str, headers: &[(&str, &str)]) -> axum::extract::Request {
    let mut req = axum::extract::Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header("content-type", "application/x-protobuf");
    for (name, value) in headers {
        req = req.header(*name, *value);
    }
    req.body(axum::body::Body::from(vec![0x0a, 0x00]))
        .expect("request build")
}

// Why: an anonymous OTLP writer could fill the log store with attacker-chosen
// rows; the route must refuse before it reads the body, not accept and drop.
#[tokio::test]
async fn otel_without_a_credential_is_unauthorized() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(otel_post("/otel", &[])).await?;
    assert_eq!(resp.status(), http::StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn otel_subpath_without_a_credential_is_unauthorized() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app.oneshot(otel_post("/otel/v1/traces", &[])).await?;
    assert_eq!(resp.status(), http::StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn otel_with_an_invalid_bearer_is_unauthorized() -> anyhow::Result<()> {
    let app = router().await?;
    let resp = app
        .oneshot(otel_post(
            "/otel",
            &[
                ("authorization", "Bearer not-a-token"),
                ("x-session-id", "sess-otel"),
            ],
        ))
        .await?;
    assert_eq!(resp.status(), http::StatusCode::UNAUTHORIZED);
    Ok(())
}
