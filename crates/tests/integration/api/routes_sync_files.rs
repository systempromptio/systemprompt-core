//! File download end to end through `sync_router`.
//!
//! Builds an `AppContext` whose `paths.services` points at a temp tree
//! containing allow-listed directories, then drives the manifest and download
//! handlers so the internal `collect_files` and `create_tarball` helpers run.
//!
//! `get_services_path` honours `SYSTEMPROMPT_SERVICES_PATH` first, so the test
//! drives the path purely through the context's `app_paths` (no env mutation,
//! which the workspace forbids under `unsafe_code = "deny"`).

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use systemprompt_api::routes::sync_router;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::app_context::fixture_app_context_with;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use tower::ServiceExt;

async fn ctx_with_services(tree: &std::path::Path) -> anyhow::Result<Arc<AppContext>> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let services = tree.to_string_lossy().into_owned();
    let paths = PathsConfig {
        system: services.clone(),
        services,
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))
}

fn app(ctx: &AppContext) -> axum::Router {
    sync_router().with_state(ctx.clone())
}

fn make_services_tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let agents = dir.path().join("agents");
    std::fs::create_dir_all(&agents).expect("mkdir agents");
    std::fs::write(agents.join("hello.yaml"), b"name: hello\n").expect("write file");
    let config = dir.path().join("config");
    std::fs::create_dir_all(&config).expect("mkdir config");
    std::fs::write(config.join("settings.json"), b"{}\n").expect("write file");
    dir
}

async fn read_json(resp: Response<Body>) -> anyhow::Result<(http::StatusCode, serde_json::Value)> {
    let status = resp.status();
    let bytes = to_bytes(resp.into_body(), 8 * 1024 * 1024).await?;
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
    Ok((status, v))
}

fn get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("build")
}

#[tokio::test]
async fn manifest_lists_files_in_services_tree() -> anyhow::Result<()> {
    let tree = make_services_tree();
    let ctx = ctx_with_services(tree.path()).await?;

    let resp = app(&ctx).oneshot(get("/files/manifest")).await?;
    let (status, body) = read_json(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    let files = body["files"].as_array().expect("files array");
    assert!(
        files
            .iter()
            .any(|f| f["path"].as_str().is_some_and(|p| p.contains("hello.yaml"))),
        "manifest must list seeded file: {body}"
    );
    assert!(
        body["checksum"].as_str().is_some_and(|c| !c.is_empty()),
        "{body}"
    );
    Ok(())
}

#[tokio::test]
async fn manifest_with_filter_restricts_directories() -> anyhow::Result<()> {
    let tree = make_services_tree();
    let ctx = ctx_with_services(tree.path()).await?;

    let resp = app(&ctx)
        .oneshot(get("/files/manifest?filter=agents"))
        .await?;
    let (status, body) = read_json(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    let files = body["files"].as_array().expect("files array");
    assert!(
        files
            .iter()
            .all(|f| f["path"].as_str().is_some_and(|p| p.starts_with("agents"))),
        "filtered manifest must only contain agents/: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn download_dry_run_returns_manifest() -> anyhow::Result<()> {
    let tree = make_services_tree();
    let ctx = ctx_with_services(tree.path()).await?;

    let resp = app(&ctx).oneshot(get("/files?dry_run=true")).await?;
    let (status, body) = read_json(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body["files"].is_array(), "{body}");
    Ok(())
}

#[tokio::test]
async fn download_streams_gzip_tarball() -> anyhow::Result<()> {
    let tree = make_services_tree();
    let ctx = ctx_with_services(tree.path()).await?;

    let resp = app(&ctx).oneshot(get("/files")).await?;
    assert_eq!(resp.status().as_u16(), 200);
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(ct, "application/gzip");
    let bytes = to_bytes(resp.into_body(), 8 * 1024 * 1024).await?;
    assert!(!bytes.is_empty(), "tarball body must be non-empty");
    assert_eq!(&bytes[..2], &[0x1f, 0x8b], "gzip magic bytes");
    Ok(())
}

