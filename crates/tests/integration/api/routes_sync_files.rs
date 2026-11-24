//! File download end to end through the `cloud` member's sync router.
//!
//! Seeds allow-listed directories into the test bootstrap's services tree,
//! which the `AppContext` reads as `paths.services`, then drives the manifest
//! and download handlers so the internal `collect_files` and `create_tarball`
//! helpers run.
//!
//! The sync state is the context's services root, so the test drives the
//! path purely through the context (no env mutation, which the workspace
//! forbids under `unsafe_code = "deny"`).

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use systemprompt_cloud_entry::http::SyncState;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, test_app_context};
use tower::ServiceExt;

async fn ctx_with_services() -> anyhow::Result<Arc<AppContext>> {
    let b = ensure_test_bootstrap();
    seed_services_tree(&b.services_path);
    Ok(test_app_context(&b.database_url).await)
}

fn app(ctx: &AppContext) -> axum::Router {
    systemprompt_cloud_entry::http::router(SyncState::new(
        ctx.app_paths().system().services().to_path_buf(),
    ))
}

fn seed_services_tree(root: &std::path::Path) {
    let agents = root.join("agents");
    std::fs::create_dir_all(&agents).expect("mkdir agents");
    std::fs::write(agents.join("hello.yaml"), b"name: hello\n").expect("write file");
    let config = root.join("config");
    std::fs::create_dir_all(&config).expect("mkdir config");
    std::fs::write(config.join("settings.json"), b"{}\n").expect("write file");
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
    let ctx = ctx_with_services().await?;

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
    let ctx = ctx_with_services().await?;

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
    let ctx = ctx_with_services().await?;

    let resp = app(&ctx).oneshot(get("/files?dry_run=true")).await?;
    let (status, body) = read_json(resp).await?;
    assert_eq!(status.as_u16(), 200, "{body}");
    assert!(body["files"].is_array(), "{body}");
    Ok(())
}

#[tokio::test]
async fn download_streams_gzip_tarball() -> anyhow::Result<()> {
    let ctx = ctx_with_services().await?;

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
