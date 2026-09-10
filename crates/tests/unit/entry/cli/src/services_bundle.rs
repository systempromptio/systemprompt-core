//! Tests for `core services bundle`, `keygen` and `inspect`.
//!
//! The round trip is the contract: what `bundle` writes, `inspect` must read
//! back, including the signature identity and the derived ownership. The
//! `--marketplace-only` case proves the refusal happens before an archive is
//! written, not after.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::core::services::bundle::{BundleArgs, pack_bundle, requires_core};
use systemprompt_cli::core::services::inspect::describe_bundle;
use systemprompt_cli::core::services::keygen::{KeygenArgs, execute as keygen};
use systemprompt_cli::core::services::signing::{BundleSigningKey, load_signing_key};

fn write(root: &Path, relative: &str, body: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("relative path has a parent")).expect("mkdir");
    std::fs::write(path, body).expect("write");
}

fn marketplace_tree(root: &Path) {
    write(
        root,
        "plugins/alpha/config.yaml",
        "plugin:\n  id: alpha\n  version: 1.0.0\n",
    );
    write(root, "skills/alpha_report/config.yaml", "skill:\n  id: x\n");
}

fn args(root: PathBuf, out: PathBuf) -> BundleArgs {
    BundleArgs {
        root,
        out,
        version: "1.4.0".to_owned(),
        sign_key: None,
        source_repo: Some("org/services".to_owned()),
        source_commit: Some("deadbeef".to_owned()),
        workflow_run: Some("42".to_owned()),
        marketplace_only: false,
    }
}

#[test]
fn requires_core_pins_the_current_major_minor() {
    assert_eq!(requires_core("0.49.3").expect("semver"), ">=0.49");
    assert!(requires_core("not-a-version").is_err());
}

#[test]
fn bundle_and_inspect_round_trip_an_unsigned_archive() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("tree");
    marketplace_tree(&root);
    let out = dir.path().join("bundle.tar.gz");

    let outcome = pack_bundle(&args(root, out.clone()), None).expect("pack succeeds");
    assert_eq!(outcome.version, "1.4.0");
    assert!(outcome.signed_by.is_none());
    assert!(out.is_file(), "archive was not written");

    let signed = systemprompt_loader::bundle::verify::read_manifest(&out).expect("manifest reads");
    let report = describe_bundle(&signed, None);
    assert_eq!(report.version, "1.4.0");
    assert_eq!(report.source_repo.as_deref(), Some("org/services"));
    assert_eq!(report.source_commit.as_deref(), Some("deadbeef"));
    assert_eq!(report.workflow_run.as_deref(), Some("42"));
    assert_eq!(report.signature_status, "unsigned");
    assert_eq!(report.content_hash, outcome.content_hash);
    assert!(report.owns.contains("1 plugin(s)"), "owns: {}", report.owns);
}

#[test]
fn a_signed_bundle_carries_the_key_id_it_was_signed_with() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("tree");
    marketplace_tree(&root);
    let out = dir.path().join("bundle.tar.gz");

    let key = BundleSigningKey::generate();
    let outcome = pack_bundle(&args(root, out.clone()), Some(&key)).expect("pack succeeds");
    assert_eq!(outcome.signed_by.as_deref(), Some(key.key_id.as_str()));

    let signed = systemprompt_loader::bundle::verify::read_manifest(&out).expect("manifest reads");
    let signature = signed.signature.as_ref().expect("archive is signed");
    assert_eq!(signature.key_id, key.key_id);
    assert_eq!(signature.alg, "ed25519");
}

#[test]
fn marketplace_only_refuses_a_platform_directory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().join("tree");
    marketplace_tree(&root);
    write(&root, "mcp/knowledge-bank.yaml", "server: {}\n");
    let out = dir.path().join("bundle.tar.gz");

    let mut args = args(root, out.clone());
    args.marketplace_only = true;
    let err = pack_bundle(&args, None).expect_err("a platform directory must be refused");
    assert!(
        err.to_string().contains("--marketplace-only"),
        "unexpected error: {err}"
    );
    assert!(!out.exists(), "the archive was written despite the refusal");
}

#[test]
fn keygen_writes_a_seed_that_load_signing_key_reads_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let seed_path = dir.path().join("keys/bundle.key");
    let output = keygen(&KeygenArgs {
        out: Some(seed_path.clone()),
    })
    .expect("keygen succeeds");

    let rendered = serde_json::to_string(output.artifact()).expect("artifact serialises");
    assert!(rendered.contains("key_id"));
    assert!(
        !rendered.contains(&std::fs::read_to_string(&seed_path).expect("seed file")),
        "the seed leaked into the rendered output"
    );

    let loaded = load_signing_key(seed_path.to_str().expect("utf-8 path")).expect("seed reloads");
    assert!(
        rendered.contains(&loaded.public_key),
        "public key not reported"
    );
    assert!(rendered.contains(&loaded.key_id), "key id not reported");
}

#[test]
fn a_signing_key_of_the_wrong_length_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("short.key");
    std::fs::write(&path, "c2hvcnQ=").expect("write");
    let err = load_signing_key(path.to_str().expect("utf-8 path"))
        .expect_err("a 5-byte seed must be refused");
    assert!(err.to_string().contains("32 bytes"), "unexpected: {err}");
}
