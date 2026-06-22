//! Tests for [`DeployArtifacts::resolve`].
//!
//! `resolve` validates the Docker build context before a cloud deploy. With a
//! bare temp project root the release binary is absent, so resolution fails
//! fast with a [`SyncError::BuildArtifacts`] error naming the missing binary —
//! this exercises path construction and the first validation branch without
//! requiring a built workspace.

use systemprompt_sync::SyncError;
use systemprompt_sync::deploy::DeployArtifacts;
use tempfile::TempDir;

#[test]
fn resolve_fails_when_release_binary_missing() {
    let root = TempDir::new().expect("tempdir");

    let err = DeployArtifacts::resolve(root.path(), "local")
        .expect_err("resolution must fail without a release binary");

    match err {
        SyncError::BuildArtifacts(message) => {
            assert!(
                message.contains("Release binary not found"),
                "unexpected message: {message}"
            );
            assert!(
                message.contains("cargo build --release"),
                "message should hint at the build command: {message}"
            );
        },
        other => panic!("expected BuildArtifacts error, got {other:?}"),
    }
}

#[test]
fn resolve_error_references_the_target_release_path() {
    let root = TempDir::new().expect("tempdir");

    let err = DeployArtifacts::resolve(root.path(), "prod")
        .expect_err("resolution must fail without a release binary");

    let SyncError::BuildArtifacts(message) = err else {
        panic!("expected BuildArtifacts error");
    };
    assert!(
        message.contains("release"),
        "binary path should sit under target/release: {message}"
    );
    assert!(
        message.contains(&root.path().display().to_string()),
        "message should reference the project root: {message}"
    );
}
