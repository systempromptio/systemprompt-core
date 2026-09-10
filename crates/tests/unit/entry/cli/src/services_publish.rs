//! `core services publish` — reference shaping and credential handling.
//!
//! The pin is what a profile ends up quoting, so a tag must be replaced by
//! the digest rather than appended to. The credential rules matter for the
//! same reason `--auth` is restricted to `env:VAR`: a registry password must
//! never become a shell argument.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use clap::Parser;
use systemprompt_cli::core::services::bundle::{BundleArgs, pack_bundle};
use systemprompt_cli::core::services::publish::{PublishArgs, execute, pin_reference};

#[derive(Debug, Parser)]
struct Harness {
    #[command(flatten)]
    args: PublishArgs,
}

fn parse(args: &[&str]) -> PublishArgs {
    Harness::try_parse_from(std::iter::once("publish").chain(args.iter().copied()))
        .expect("args parse")
        .args
}

fn bundle(dir: &Path) -> PathBuf {
    let tree = dir.join("tree");
    let config = tree.join("plugins/alpha/config.yaml");
    std::fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    std::fs::write(config, "plugin:\n  id: alpha\n  version: 1.0.0\n").expect("write");
    let out = dir.join("bundle.tar.gz");
    pack_bundle(
        &BundleArgs {
            root: tree,
            out: out.clone(),
            version: "1.4.0".to_owned(),
            sign_key: None,
            source_repo: None,
            source_commit: None,
            workflow_run: None,
            marketplace_only: false,
        },
        None,
    )
    .expect("pack succeeds");
    out
}

fn args(bundle: PathBuf, auth: Option<&str>) -> PublishArgs {
    PublishArgs {
        bundle,
        to: "oci://ghcr.io/org/services:1.4.0".to_owned(),
        auth_secret: None,
        auth: auth.map(str::to_owned),
    }
}

#[test]
fn a_tag_is_replaced_by_the_digest_not_appended() {
    assert_eq!(
        pin_reference("ghcr.io/org/services:1.4.0", "sha256:abc"),
        "ghcr.io/org/services@sha256:abc"
    );
}

#[test]
fn a_registry_port_is_not_mistaken_for_a_tag() {
    assert_eq!(
        pin_reference("registry.test:5000/org/services", "sha256:abc"),
        "registry.test:5000/org/services@sha256:abc"
    );
}

#[test]
fn an_untagged_reference_gains_the_digest() {
    assert_eq!(
        pin_reference("ghcr.io/org/services", "sha256:abc"),
        "ghcr.io/org/services@sha256:abc"
    );
}

#[test]
fn an_already_pinned_reference_is_repinned_to_the_pushed_digest() {
    assert_eq!(
        pin_reference("ghcr.io/org/services@sha256:old", "sha256:new"),
        "ghcr.io/org/services@sha256:new"
    );
}

#[test]
fn the_oci_scheme_and_credentials_parse_off_the_command_line() {
    let parsed = parse(&[
        "--bundle",
        "b.tar.gz",
        "--to",
        "oci://ghcr.io/org/services:1.4.0",
        "--auth",
        "env:GHCR_TOKEN",
    ]);
    assert_eq!(parsed.bundle, PathBuf::from("b.tar.gz"));
    assert_eq!(parsed.to, "oci://ghcr.io/org/services:1.4.0");
    assert_eq!(parsed.auth.as_deref(), Some("env:GHCR_TOKEN"));
    assert!(parsed.auth_secret.is_none());
}

#[tokio::test]
async fn an_unreadable_archive_is_refused_before_the_registry_is_contacted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("nope.tar.gz");
    let error = execute(&args(missing.clone(), None))
        .await
        .expect_err("a missing archive cannot be published");
    assert!(format!("{error:#}").contains("nope.tar.gz"), "{error:#}");
}

#[tokio::test]
async fn an_inline_credential_must_name_an_environment_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = bundle(dir.path());
    let error = execute(&args(archive, Some("ghp_secret_value")))
        .await
        .expect_err("a literal credential is refused");
    let rendered = format!("{error:#}");
    assert!(rendered.contains("env:VAR"), "{rendered}");
    assert!(
        !rendered.contains("ghp_secret_value"),
        "the credential leaked into the error: {rendered}"
    );
}

#[tokio::test]
async fn an_unset_credential_variable_names_the_variable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = bundle(dir.path());
    let error = execute(&args(archive, Some("env:SP_TEST_PUBLISH_UNSET")))
        .await
        .expect_err("an unset variable is an error");
    assert!(
        format!("{error:#}").contains("SP_TEST_PUBLISH_UNSET"),
        "{error:#}"
    );
}

#[tokio::test]
async fn an_auth_secret_without_a_profile_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = bundle(dir.path());
    let error = execute(&PublishArgs {
        bundle: archive,
        to: "oci://ghcr.io/org/services:1.4.0".to_owned(),
        auth_secret: Some("ghcr_token".to_owned()),
        auth: None,
    })
    .await
    .expect_err("no profile means no secrets");
    assert!(format!("{error:#}").contains("profile"), "{error:#}");
}

#[tokio::test]
async fn a_registry_that_rejects_the_upload_names_the_destination() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::any())
        .respond_with(wiremock::ResponseTemplate::new(500))
        .mount(&server)
        .await;
    unsafe { std::env::set_var("SYSTEMPROMPT_TRUSTED_HTTP_HOSTS", "127.0.0.1,localhost") };

    let dir = tempfile::tempdir().expect("tempdir");
    let archive = bundle(dir.path());
    let host = server.uri().replace("http://", "");
    let error = execute(&PublishArgs {
        bundle: archive,
        to: format!("oci://{host}/org/services:1.4.0"),
        auth_secret: None,
        auth: None,
    })
    .await
    .expect_err("a failing registry is an error");
    assert!(format!("{error:#}").contains("org/services"), "{error:#}");
}
