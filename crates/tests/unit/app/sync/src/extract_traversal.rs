//! Tarball extraction hardening: symlink and parent-dir traversal entries
//! are rejected before touching disk, and `download_and_diff` classifies a
//! locally-edited file as modified.

use flate2::Compression;
use flate2::write::GzEncoder;
use systemprompt_identifiers::TenantId;
use systemprompt_sync::{FileDiffStatus, FileSyncService, SyncConfig, SyncError};
use tar::{Builder, EntryType, Header};
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn tarball_with_symlink() -> Vec<u8> {
    let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = Header::new_gnu();
    header.set_entry_type(EntryType::Symlink);
    header.set_size(0);
    header.set_cksum();
    builder
        .append_link(&mut header, "agents/evil-link", "/etc/passwd")
        .unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

fn tarball_with_entries(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (name, content) in entries {
        let mut header = Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append_data(&mut header, name, *content).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn apply_rejects_symlink_entries() {
    let target = TempDir::new().expect("tempdir");
    let err = FileSyncService::apply(&tarball_with_symlink(), target.path(), None)
        .expect_err("symlink must be rejected");
    match err {
        SyncError::TarballUnsafe(message) => {
            assert!(message.contains("disallowed entry type"), "{message}");
            assert!(message.contains("evil-link"), "{message}");
        },
        other => panic!("expected TarballUnsafe, got {other:?}"),
    }
    assert!(!target.path().join("agents").exists());
}

fn tarball_with_raw_name(name: &[u8], content: &[u8]) -> Vec<u8> {
    let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = Header::new_gnu();
    {
        let gnu = header.as_gnu_mut().expect("gnu header");
        gnu.name[..name.len()].copy_from_slice(name);
    }
    header.set_size(content.len() as u64);
    header.set_mode(0o644);
    header.set_entry_type(EntryType::Regular);
    header.set_cksum();
    builder.append(&header, content).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

#[test]
fn apply_rejects_parent_dir_traversal() {
    let target = TempDir::new().expect("tempdir");
    let data = tarball_with_raw_name(b"agents/../../escape.yaml", b"pwn");
    let err =
        FileSyncService::apply(&data, target.path(), None).expect_err("traversal must be rejected");
    match err {
        SyncError::TarballUnsafe(message) => {
            assert!(message.contains("invalid path in tarball"), "{message}");
        },
        other => panic!("expected TarballUnsafe, got {other:?}"),
    }
    assert!(!target.path().parent().unwrap().join("escape.yaml").exists());
}

#[tokio::test]
async fn download_and_diff_locally_edited_file_reports_modified() {
    let services = TempDir::new().expect("tempdir");
    let agents = services.path().join("agents");
    std::fs::create_dir_all(&agents).expect("agents dir");
    std::fs::write(agents.join("demo.yaml"), b"local edit").expect("local file");

    let server = MockServer::start().await;
    let data = tarball_with_entries(&[("agents/demo.yaml", b"remote version")]);
    Mock::given(method("GET"))
        .and(path("/api/v1/cloud/tenants/t-mod/files"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(data))
        .mount(&server)
        .await;

    let config = SyncConfig::builder(
        TenantId::new("t-mod"),
        server.uri(),
        "tok",
        services.path().to_string_lossy(),
    )
    .build();
    let client = systemprompt_sync::SyncApiClient::new(&server.uri(), "tok").expect("client");
    let download = FileSyncService::new(config, client)
        .download_and_diff()
        .await
        .expect("download");

    assert_eq!(download.diff.modified, 1);
    assert_eq!(download.diff.added, 0);
    assert_eq!(download.diff.deleted, 0);
    let entry = download
        .diff
        .entries
        .iter()
        .find(|e| e.path == "agents/demo.yaml")
        .expect("entry");
    assert_eq!(entry.status, FileDiffStatus::Modified);
}
