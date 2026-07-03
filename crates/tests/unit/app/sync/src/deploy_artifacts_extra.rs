//! Build-context validation branches for [`DeployArtifacts::resolve`].
//!
//! Extends `deploy_artifacts` past the missing-binary case: with a stub release
//! binary in place, `resolve` walks the storage, templates, and Dockerfile
//! checks in turn. Each test stages exactly enough of the tree to reach — and
//! assert on — the next validation branch, then a final test stages the whole
//! Docker build context (binary, `storage/files`, `services/web/templates`, and
//! the profile Dockerfile) to drive the success path. The test binary registers
//! no asset extensions, so the extension-asset check passes cleanly.

use std::fs;
use std::path::Path;

use systemprompt_cloud::ProjectContext;
use systemprompt_models::paths::constants::build;
use systemprompt_sync::SyncError;
use systemprompt_sync::deploy::DeployArtifacts;
use tempfile::TempDir;

fn stub_binary(root: &Path) {
    let bindir = root.join(build::CARGO_TARGET).join("release");
    fs::create_dir_all(&bindir).expect("bindir");
    fs::write(bindir.join(build::BINARY_NAME), b"#!stub").expect("binary");
}

fn build_message(err: SyncError) -> String {
    match err {
        SyncError::BuildArtifacts(message) => message,
        other => panic!("expected BuildArtifacts error, got {other:?}"),
    }
}

#[test]
fn missing_storage_directory_is_reported_after_binary() {
    let root = TempDir::new().expect("tempdir");
    stub_binary(root.path());

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("no storage dir");
    let message = build_message(err);
    assert!(
        message.contains("Storage directory not found"),
        "unexpected message: {message}"
    );
}

#[test]
fn missing_storage_files_directory_is_reported() {
    let root = TempDir::new().expect("tempdir");
    stub_binary(root.path());
    fs::create_dir_all(root.path().join("storage")).expect("storage");

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("no storage/files");
    let message = build_message(err);
    assert!(
        message.contains("Storage files directory not found"),
        "unexpected message: {message}"
    );
}

#[test]
fn missing_templates_directory_is_reported() {
    let root = TempDir::new().expect("tempdir");
    stub_binary(root.path());
    fs::create_dir_all(root.path().join("storage/files")).expect("storage/files");

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("no templates");
    let message = build_message(err);
    assert!(
        message.contains("Templates directory not found"),
        "unexpected message: {message}"
    );
}

#[test]
fn missing_dockerfile_is_reported_last() {
    let root = TempDir::new().expect("tempdir");
    stub_binary(root.path());
    fs::create_dir_all(root.path().join("storage/files")).expect("storage/files");
    fs::create_dir_all(root.path().join("services/web/templates")).expect("templates");

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("no dockerfile");
    let message = build_message(err);
    assert!(
        message.contains("Dockerfile not found"),
        "unexpected message: {message}"
    );
}

#[test]
fn fully_staged_context_resolves() {
    let root = TempDir::new().expect("tempdir");
    stub_binary(root.path());
    fs::create_dir_all(root.path().join("storage/files")).expect("storage/files");
    fs::create_dir_all(root.path().join("services/web/templates")).expect("templates");

    let dockerfile = ProjectContext::new(root.path().to_path_buf()).profile_dockerfile("local");
    fs::create_dir_all(dockerfile.parent().expect("dockerfile parent")).expect("docker dir");
    fs::write(&dockerfile, b"FROM scratch\n").expect("dockerfile");

    let artifacts = DeployArtifacts::resolve(root.path(), "local").expect("fully staged context");
    assert_eq!(artifacts.dockerfile, dockerfile);
    assert!(
        artifacts.binary.ends_with(build::BINARY_NAME),
        "binary path should end with the binary name: {}",
        artifacts.binary.display()
    );
    assert!(
        format!("{artifacts:?}").contains("DeployArtifacts"),
        "debug should render the struct name"
    );
}
