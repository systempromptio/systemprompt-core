//! `GET /bridge/manifest` against a seeded services catalog — signed happy
//! path, host-pref scoping, and the host model-filter round trip.

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use http::StatusCode;
use systemprompt_test_fixtures::seed_admin_credential;
use tower::ServiceExt;

use super::routes_bridge_plugin_file::bundle_router_and_pool;

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

fn authed_post(uri: &str, token: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .expect("request build")
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<serde_json::Value> {
    let bytes = to_bytes(resp.into_body(), 4 * 1024 * 1024).await?;
    Ok(serde_json::from_slice(&bytes)?)
}

async fn fetch_manifest(
    app: axum::Router,
    token: &str,
) -> anyhow::Result<(StatusCode, serde_json::Value)> {
    let resp = app.oneshot(authed_get("/bridge/manifest", token)).await?;
    let status = resp.status();
    let body = read_json(resp).await?;
    Ok((status, body))
}

#[tokio::test]
async fn manifest_with_seeded_catalog_is_signed_and_lists_plugin_and_skill() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest-catalog@example.invalid").await?;
    let (status, body) = fetch_manifest(app, cred.jwt.as_str()).await?;
    assert_eq!(status, StatusCode::OK, "{body}");

    assert_eq!(body["user_id"].as_str(), Some(cred.user_id.as_str()));
    assert_eq!(body["user"]["email"].as_str(), Some(cred.email.as_str()));
    assert!(
        body["signature"].as_str().is_some_and(|s| !s.is_empty()),
        "signature missing: {body}"
    );
    assert!(
        body["manifest_version"].as_str().is_some(),
        "manifest_version missing: {body}"
    );
    let issued_at = body["issued_at"].as_str().expect("issued_at");
    let not_before = body["not_before"].as_str().expect("not_before");
    assert!(not_before < issued_at, "{not_before} vs {issued_at}");

    let plugins = body["plugins"].as_array().expect("plugins array");
    let cov = plugins
        .iter()
        .find(|p| p["id"].as_str() == Some("cov-plugin"))
        .unwrap_or_else(|| panic!("cov-plugin missing from {body}"));
    assert_eq!(cov["version"].as_str(), Some("1.0.0"), "{cov}");
    assert!(
        cov["sha256"].as_str().is_some_and(|s| s.len() == 64),
        "bundle digest expected: {cov}"
    );
    assert!(
        cov["files"].as_array().is_some_and(|files| {
            files
                .iter()
                .any(|f| f["path"].as_str() == Some("skills/covskill/SKILL.md"))
        }),
        "skill file listing expected: {cov}"
    );

    let skills = body["skills"].as_array().expect("skills array");
    assert!(
        skills.iter().any(|s| s["id"].as_str() == Some("covskill")),
        "covskill missing from {body}"
    );

    let hosts = body["enabled_hosts"].as_array().expect("enabled_hosts");
    let host_names: Vec<&str> = hosts.iter().filter_map(|h| h.as_str()).collect();
    for expected in ["claude-code", "claude-desktop", "cowork", "codex-cli"] {
        assert!(host_names.contains(&expected), "{expected} not in {body}");
    }
    Ok(())
}

#[tokio::test]
async fn manifest_enabled_hosts_scope_to_stored_prefs() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest-hosts@example.invalid").await?;

    let resp = app
        .clone()
        .oneshot(authed_post(
            "/bridge/profile/enabled_hosts",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "claude-code", "enabled": true}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let resp = app
        .clone()
        .oneshot(authed_post(
            "/bridge/profile/enabled_hosts",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "cowork", "enabled": false}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);

    let (status, body) = fetch_manifest(app, cred.jwt.as_str()).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    let hosts: Vec<&str> = body["enabled_hosts"]
        .as_array()
        .expect("enabled_hosts")
        .iter()
        .filter_map(|h| h.as_str())
        .collect();
    assert_eq!(hosts, vec!["claude-code"], "{body}");
    Ok(())
}

#[tokio::test]
async fn set_enabled_host_rejects_unknown_host() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest-badhost@example.invalid").await?;
    let resp = app
        .oneshot(authed_post(
            "/bridge/profile/enabled_hosts",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "not-a-host", "enabled": true}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn host_model_filter_round_trips_into_manifest_and_clears() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest-filter@example.invalid").await?;

    let resp = app
        .clone()
        .oneshot(authed_post(
            "/bridge/profile/host-model-filter",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "claude-code", "model_protocols": ["anthropic", "openai"]}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let echoed = read_json(resp).await?;
    assert_eq!(echoed["host_id"].as_str(), Some("claude-code"));
    assert_eq!(
        echoed["model_protocols"],
        serde_json::json!(["anthropic", "openai"])
    );

    let (status, body) = fetch_manifest(app.clone(), cred.jwt.as_str()).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["host_model_protocols"]["claude-code"],
        serde_json::json!(["anthropic", "openai"]),
        "{body}"
    );

    let resp = app
        .clone()
        .oneshot(authed_post(
            "/bridge/profile/host-model-filter",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "claude-code", "model_protocols": null}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    let cleared = read_json(resp).await?;
    assert!(cleared["model_protocols"].is_null(), "{cleared}");

    let (status, body) = fetch_manifest(app, cred.jwt.as_str()).await?;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["host_model_protocols"]
            .as_object()
            .is_some_and(|m| !m.contains_key("claude-code")),
        "override should be cleared: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn host_model_filter_rejects_unknown_host_and_surface() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_admin_credential(&pool, "manifest-filter-bad@example.invalid").await?;

    let resp = app
        .clone()
        .oneshot(authed_post(
            "/bridge/profile/host-model-filter",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "not-a-host", "model_protocols": []}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let resp = app
        .oneshot(authed_post(
            "/bridge/profile/host-model-filter",
            cred.jwt.as_str(),
            serde_json::json!({"host_id": "claude-code", "model_protocols": ["cohere"]}),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn host_model_filter_without_credential_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = bundle_router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .method(http::Method::POST)
                .uri("/bridge/profile/host-model-filter")
                .header("content-type", "application/json")
                .body(Body::from(
                    serde_json::json!({"host_id": "claude-code"}).to_string(),
                ))?,
        )
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}
