//! End-to-end push/pull tests for `FileSyncService` that exercise the
//! `collect_files`, `create_tarball`, `peek_manifest`, and
//! `compare_tarball_with_local` bundler helpers via the public service
//! surface.

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::time::Duration;
use systemprompt_identifiers::TenantId;
use systemprompt_sync::api_client::RetryConfig;
use systemprompt_sync::{FileSyncService, SyncApiClient, SyncConfig, SyncDirection};
use tar::Builder;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn fast_retry() -> RetryConfig {
    RetryConfig {
        max_attempts: 2,
        initial_delay: Duration::from_millis(1),
        max_delay: Duration::from_millis(2),
        exponential_base: 2,
    }
}

fn build_tarball(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    {
        let mut tar = Builder::new(&mut encoder);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, name, *content)
                .expect("append");
        }
        tar.finish().expect("finish");
    }
    encoder.finish().expect("encode")
}

#[tokio::test]
async fn push_dry_run_collects_files_without_calling_api() {
    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();
    fs::create_dir_all(services.join("agents")).expect("mkdir");
    fs::write(services.join("agents/a.yaml"), "agent: a\n").expect("write");
    fs::create_dir_all(services.join("skills")).expect("mkdir");
    fs::write(services.join("skills/s.md"), "# s\n").expect("write");

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        "http://unused",
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Push)
    .with_dry_run(true)
    .build();

    let api = SyncApiClient::new("http://unused", "tok").expect("client");
    let svc = FileSyncService::new(config, api);

    let result = svc.sync().await.expect("dry-run push");
    assert_eq!(result.operation, "files_push");
    assert!(result.success);
    assert!(
        result.details.is_some(),
        "dry-run push should include manifest"
    );
}

#[tokio::test]
async fn pull_dry_run_uses_peek_manifest_from_downloaded_tarball() {
    let server = MockServer::start().await;
    let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1"), ("skills/s.md", b"# s")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Pull)
    .with_dry_run(true)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let result = svc.sync().await.expect("dry-run pull");
    assert_eq!(result.operation, "files_pull");
    assert!(result.success);
}

#[tokio::test]
async fn pull_live_extracts_tarball_to_services_path() {
    let server = MockServer::start().await;
    let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Pull)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let result = svc.sync().await.expect("live pull");
    assert_eq!(result.operation, "files_pull");
    assert!(services.join("agents/a.yaml").exists());
}

#[tokio::test]
async fn download_and_diff_against_empty_local_reports_all_added() {
    let server = MockServer::start().await;
    let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1"), ("skills/s.md", b"# skill")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();
    // Empty services dir — every remote file should be "added".

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Pull)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let pull = svc.download_and_diff().await.expect("download_and_diff");
    assert!(pull.diff.has_changes());
    assert_eq!(pull.diff.added, 2);
    assert_eq!(pull.diff.modified, 0);
    assert_eq!(pull.diff.unchanged, 0);
}

#[tokio::test]
async fn download_and_diff_with_matching_local_reports_unchanged() {
    let server = MockServer::start().await;
    let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();
    fs::create_dir_all(services.join("agents")).expect("mkdir");
    fs::write(services.join("agents/a.yaml"), "agent: 1").expect("write");

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Pull)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let pull = svc.download_and_diff().await.expect("download_and_diff");
    assert_eq!(pull.diff.unchanged, 1);
    assert_eq!(pull.diff.added, 0);
}

#[tokio::test]
async fn download_and_diff_local_only_file_reports_deleted() {
    let server = MockServer::start().await;
    let tarball = build_tarball(&[("agents/a.yaml", b"agent: 1")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(tarball))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();
    // Local has an extra file not on remote.
    fs::create_dir_all(services.join("agents")).expect("mkdir");
    fs::write(services.join("agents/a.yaml"), "agent: 1").expect("write");
    fs::write(services.join("agents/local_only.yaml"), "extra").expect("write");

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Pull)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let pull = svc.download_and_diff().await.expect("download_and_diff");
    assert_eq!(pull.diff.deleted, 1);
}

#[tokio::test]
async fn push_live_uploads_tarball_with_count() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/cloud/tenants/t1/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files_uploaded": 1
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().expect("tmp");
    let services = tmp.path().to_path_buf();
    fs::create_dir_all(services.join("agents")).expect("mkdir");
    fs::write(services.join("agents/a.yaml"), "agent: a\n").expect("write");

    let config = SyncConfig::builder(
        TenantId::new("t1"),
        &server.uri(),
        "tok",
        services.to_string_lossy().as_ref(),
    )
    .with_direction(SyncDirection::Push)
    .build();

    let api = SyncApiClient::new(&server.uri(), "tok")
        .expect("client")
        .with_retry_config(fast_retry());
    let svc = FileSyncService::new(config, api);

    let result = svc.sync().await.expect("live push");
    assert_eq!(result.operation, "files_push");
    assert!(result.success);
}
