//! `GET /bridge/plugins/{id}/{*path}` — auth rejection, path-safety guard,
//! bundle lookup misses, and the content-type / path-safety helpers exposed
//! via `test-api`.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use http::StatusCode;
use systemprompt_api::routes::gateway::bridge_data::load_services_config;
use systemprompt_api::routes::gateway::bridge_plugin_file::test_api::{
    content_type, relative_path_is_safe,
};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{install_test_signing_key, seed_bridge_credential};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    Ok((gateway_router(&ctx).expect("gateway router"), pool))
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn plugin_file_without_credential_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/plugins/some-plugin/README.md")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn plugin_file_with_bad_token_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/some-plugin/README.md",
            "not-a-jwt",
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn plugin_file_traversal_path_is_rejected() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-traversal@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/some-plugin/a/../secret.txt",
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn plugin_file_unknown_plugin_misses_bundle_or_reports_unavailable() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-miss@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/no-such-plugin/SKILL.md",
            cred.jwt.as_str(),
        ))
        .await?;
    if load_services_config().is_ok() {
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    } else {
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(())
}

#[test]
fn relative_path_safety_covers_traversal_and_absolute_forms() {
    assert!(relative_path_is_safe("SKILL.md"));
    assert!(relative_path_is_safe("dir/nested/file.json"));
    assert!(relative_path_is_safe("./file.txt"));
    assert!(!relative_path_is_safe(""));
    assert!(!relative_path_is_safe("../escape.md"));
    assert!(!relative_path_is_safe("dir/../../escape.md"));
    assert!(!relative_path_is_safe("/etc/passwd"));
}

#[test]
fn content_type_maps_known_extensions_and_defaults_to_octet_stream() {
    assert_eq!(content_type("a.md"), "text/markdown; charset=utf-8");
    assert_eq!(content_type("a.txt"), "text/plain; charset=utf-8");
    assert_eq!(content_type("a.json"), "application/json");
    assert_eq!(content_type("a.yaml"), "application/yaml");
    assert_eq!(content_type("a.yml"), "application/yaml");
    assert_eq!(content_type("a.toml"), "application/toml");
    assert_eq!(content_type("a.html"), "text/html; charset=utf-8");
    assert_eq!(content_type("a.HTM"), "text/html; charset=utf-8");
    assert_eq!(content_type("a.css"), "text/css; charset=utf-8");
    assert_eq!(content_type("a.js"), "application/javascript");
    assert_eq!(content_type("a.png"), "image/png");
    assert_eq!(content_type("a.jpg"), "image/jpeg");
    assert_eq!(content_type("a.JPEG"), "image/jpeg");
    assert_eq!(content_type("a.svg"), "image/svg+xml");
    assert_eq!(content_type("a.wasm"), "application/wasm");
    assert_eq!(content_type("a.bin"), "application/octet-stream");
    assert_eq!(content_type("no-extension"), "application/octet-stream");
}
