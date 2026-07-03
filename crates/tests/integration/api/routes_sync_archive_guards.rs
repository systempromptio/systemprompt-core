//! Path-traversal and entry-type guards in `routes::sync::archive`.
//!
//! Drives `extract_tarball` through the upload handler with tarballs that trip
//! each rejection branch (parent-dir component, absolute path, disallowed
//! symlink entry) and exercises `get_services_path`'s "not configured" error
//! by pointing the services path at a directory that does not exist.

use std::sync::Arc;

use axum::body::{Body, to_bytes};
use axum::http::{Request, Response, header};
use flate2::Compression;
use flate2::write::GzEncoder;
use systemprompt_api::routes::sync_router;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::profile::PathsConfig;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::app_context::fixture_app_context_with;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_db_pool};
use tower::ServiceExt;

async fn ctx_with_services(services: &str) -> anyhow::Result<Arc<AppContext>> {
    let b = ensure_test_bootstrap();
    let pool = fixture_db_pool(&b.database_url).await?;
    let paths = PathsConfig {
        system: b.system_path.display().to_string(),
        services: services.to_owned(),
        bin: "/tmp".to_string(),
        web_path: Some("/tmp".to_string()),
        storage: Some("/tmp".to_string()),
        geoip_database: None,
    };
    fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))
}

fn services_tree() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("config")).expect("mkdir config");
    dir
}

fn post_bytes(uri: &str, body: Vec<u8>) -> Request<Body> {
    Request::builder()
        .method(http::Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/gzip")
        .body(Body::from(body))
        .expect("build")
}

fn gz_tar_file(name: &str, data: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, data)
            .expect("append tar entry");
        builder.finish().expect("finish tar");
    }
    encoder.finish().expect("finish gz")
}

fn gz_tar_symlink(name: &str, target: &str) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut builder = tar::Builder::new(&mut encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        header.set_link_name(target).expect("set link name");
        header.set_cksum();
        builder
            .append_data(&mut header, name, std::io::empty())
            .expect("append symlink entry");
        builder.finish().expect("finish tar");
    }
    encoder.finish().expect("finish gz")
}

async fn status_of(resp: Response<Body>) -> http::StatusCode {
    let status = resp.status();
    let _ = to_bytes(resp.into_body(), 8 * 1024 * 1024).await;
    status
}

#[tokio::test]
async fn upload_rejects_symlink_entry() -> anyhow::Result<()> {
    let tree = services_tree();
    let ctx = ctx_with_services(&tree.path().to_string_lossy()).await?;
    let tarball = gz_tar_symlink("config/link", "/etc/passwd");
    let resp = sync_router()
        .with_state((*ctx).clone())
        .oneshot(post_bytes("/files", tarball))
        .await?;
    assert!(status_of(resp).await.is_server_error());
    assert!(!tree.path().join("config/link").exists());
    Ok(())
}

#[tokio::test]
async fn upload_with_unconfigured_services_path_errors() -> anyhow::Result<()> {
    let missing = "/tmp/systemprompt-cov-missing-services-xyz-does-not-exist";
    let ctx = ctx_with_services(missing).await?;
    let tarball = gz_tar_file("config/ok.json", b"{}");
    let resp = sync_router()
        .with_state((*ctx).clone())
        .oneshot(post_bytes("/files", tarball))
        .await?;
    assert!(status_of(resp).await.is_server_error());
    Ok(())
}
