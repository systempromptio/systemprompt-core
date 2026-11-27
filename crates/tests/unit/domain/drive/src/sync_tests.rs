use std::path::Path;

use systemprompt_drive::sync::SyncProblemKind;
use systemprompt_drive::{DriveSync, SyncError, SyncPhase, account_root, default_drive_root};
use systemprompt_models::ApiPaths;
use url::Url;

use crate::support::{
    DeviceToken, FakeInstance, FakeResponse, account, options, start_sync, status_where,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_folder_mirrors_one_instance() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("Drive").join("sp.example.com");
    let sync = DriveSync::new(options());
    sync.start(account(
        "https://sp.example.com",
        &root,
        DeviceToken::Missing,
    ))
    .await
    .unwrap();
    assert_eq!(sync.root().as_deref(), Some(root.as_path()));
    sync.stop().await;
    assert_eq!(sync.root(), None);
    sync.start(account(
        "https://sp.example.com",
        &root,
        DeviceToken::Missing,
    ))
    .await
    .unwrap();
    let taken = sync
        .start(account(
            "https://other.example.com",
            &root,
            DeviceToken::Missing,
        ))
        .await;
    assert!(matches!(taken, Err(SyncError::RootTaken { .. })));
    let relative = sync
        .start(account(
            "https://sp.example.com",
            Path::new("relative/dir"),
            DeviceToken::Missing,
        ))
        .await;
    assert!(matches!(relative, Err(SyncError::InvalidRoot { .. })));
    assert_eq!(sync.status().phase, SyncPhase::Off);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn problems_name_the_missing_credential() {
    let cases = [
        (DeviceToken::Missing, SyncProblemKind::NoDeviceToken),
        (DeviceToken::SignedOut, SyncProblemKind::SignedOut),
    ];
    for (token, kind) in cases {
        let dir = tempfile::tempdir().unwrap();
        let sync = DriveSync::new(options());
        sync.start(account("https://sp.example.com", dir.path(), token))
            .await
            .unwrap();
        let status = status_where(&sync, "the cycle failed", |status| {
            status.phase == SyncPhase::Error
        })
        .await;
        assert_eq!(status.problem.map(|problem| problem.kind), Some(kind));
        sync.stop().await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_instance_without_a_document_store_waits() {
    let fake = FakeInstance::start(|_| FakeResponse::status(404)).await;
    let dir = tempfile::tempdir().unwrap();
    let sync = start_sync(&fake, dir.path()).await;
    let status = status_where(&sync, "the sync waited", |status| {
        status.phase == SyncPhase::Disabled
    })
    .await;
    assert!(status.problem.is_none());
    let requests = fake.requests();
    assert!(!requests.is_empty());
    assert!(
        requests
            .iter()
            .all(|request| request.path() == ApiPaths::BRIDGE_DRIVE_TOKEN)
    );
}

#[test]
fn account_folders_are_named_after_the_instance() {
    let base = Path::new("/home/u/Documents/Harness Drive");
    let local = Url::parse("http://localhost:8080").unwrap();
    assert_eq!(account_root(base, &local), base.join("localhost-8080"));
    let remote = Url::parse("https://sp.example.com/base").unwrap();
    assert_eq!(account_root(base, &remote), base.join("sp.example.com"));
    let documents = Path::new("/home/u/Documents");
    assert_eq!(
        default_drive_root(documents, "systemprompt Harness"),
        documents.join("systemprompt Harness Drive")
    );
    assert_eq!(
        default_drive_root(documents, "a/b:c"),
        documents.join("a-b-c Drive")
    );
}
