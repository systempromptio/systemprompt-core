//! Asset-validation arms of [`DeployArtifacts::resolve`], driven through the
//! env-gated fixture extension in `ext_asset_fixtures`. Each test runs in its
//! own nextest process, so setting the mode env cannot leak into the rest of
//! the suite.

use std::fs;
use std::path::Path;

use systemprompt_cloud::ProjectContext;
use systemprompt_models::paths::constants::build;
use systemprompt_sync::SyncError;
use systemprompt_sync::deploy::DeployArtifacts;
use tempfile::TempDir;

use crate::ext_asset_fixtures::MODE_ENV;

fn stage_full_context(root: &Path) {
    let bindir = root.join(build::CARGO_TARGET).join("release");
    fs::create_dir_all(&bindir).expect("bindir");
    fs::write(bindir.join(build::BINARY_NAME), b"#!stub").expect("binary");
    fs::create_dir_all(root.join("storage/files")).expect("storage/files");
    fs::create_dir_all(root.join("services/web/templates")).expect("templates");
    let dockerfile = ProjectContext::new(root.to_path_buf()).profile_dockerfile("local");
    fs::create_dir_all(dockerfile.parent().expect("docker dir")).expect("docker dir");
    fs::write(&dockerfile, b"FROM scratch\n").expect("dockerfile");
}

fn build_message(err: SyncError) -> String {
    match err {
        SyncError::BuildArtifacts(message) => message,
        other => panic!("expected BuildArtifacts error, got {other:?}"),
    }
}

#[test]
fn missing_required_extension_asset_fails_resolution() {
    unsafe { std::env::set_var(MODE_ENV, "required") };
    let root = TempDir::new().expect("tempdir");
    stage_full_context(root.path());

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("required asset absent");
    let message = build_message(err);
    assert!(
        message.contains("Missing required extension assets"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("[ext:covsyncassets]") && message.contains("cov-required.css"),
        "message must name the extension and asset: {message}"
    );
    assert!(
        !message.contains("cov-optional.js"),
        "optional assets must not be reported: {message}"
    );
}

#[test]
fn present_required_extension_asset_resolves() {
    unsafe { std::env::set_var(MODE_ENV, "required") };
    let root = TempDir::new().expect("tempdir");
    stage_full_context(root.path());
    fs::write(root.path().join("storage/files/cov-required.css"), "body{}").expect("asset");

    DeployArtifacts::resolve(root.path(), "local")
        .expect("satisfied required asset must resolve; optional asset may stay absent");
}

#[test]
fn asset_outside_build_context_fails_resolution() {
    unsafe { std::env::set_var(MODE_ENV, "outside") };
    let outside = std::env::temp_dir().join("sync-cov-outside.html");
    fs::write(&outside, "<html></html>").expect("outside asset");
    let root = TempDir::new().expect("tempdir");
    stage_full_context(root.path());

    let err = DeployArtifacts::resolve(root.path(), "local").expect_err("asset outside context");
    let message = build_message(err);
    assert!(
        message.contains("outside Docker build context"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("sync-cov-outside.html"),
        "message must name the offending asset: {message}"
    );
}
