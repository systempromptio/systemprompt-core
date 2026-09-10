//! The bundle trust chain.
//!
//! Each test breaks exactly one link and asserts the specific refusal, so a
//! future change that collapses two failures into one generic error — or that
//! lets an unsigned bundle through a pinned-key profile — fails here rather
//! than in production.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;

use systemprompt_loader::bundle::{
    ExtractOptions, TarLayout, extract_tarball, verify_bundle, verify_extracted,
};
use systemprompt_models::profile::BundleVerification;
use systemprompt_models::services::bundle::BUNDLE_ALLOWED_DIRS;
use systemprompt_security::manifest_signing::pubkey_b64_from_seed;

use crate::bundle_support::{OTHER_SEED, base_tree, pack, pack_with, pubkey, write};

fn pinned() -> BundleVerification {
    BundleVerification {
        sha256: None,
        ed25519_public_keys: vec![pubkey()],
    }
}

fn opts() -> ExtractOptions<'static> {
    ExtractOptions {
        allowed_dirs: BUNDLE_ALLOWED_DIRS,
        max_bytes: 16 * 1024 * 1024,
        layout: TarLayout::Bundle,
    }
}

#[test]
fn a_signed_bundle_verifies_against_the_pinned_key() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");

    let verified = verify_bundle(&packed.archive, &pinned(), "0.49.0").expect("verify");

    assert_eq!(
        verified.manifest.content_hash,
        packed.signed.manifest.content_hash
    );
}

#[test]
fn an_unsigned_bundle_is_refused_when_the_profile_pins_keys() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack_with(
        src.path(),
        &out.path().join("b.tar.gz"),
        "1.0.0",
        ">=0.1",
        None,
    );

    let err = verify_bundle(&packed.archive, &pinned(), "0.49.0").expect_err("must be refused");

    assert!(
        err.to_string().contains("unsigned"),
        "the refusal names the missing signature: {err}"
    );
}

#[test]
fn a_bundle_signed_by_an_unpinned_key_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack_with(
        src.path(),
        &out.path().join("b.tar.gz"),
        "1.0.0",
        ">=0.1",
        Some(&OTHER_SEED),
    );

    let err = verify_bundle(&packed.archive, &pinned(), "0.49.0").expect_err("must be refused");

    assert!(
        err.to_string().contains("unpinned key"),
        "the refusal names the key: {err}"
    );
    assert_ne!(pubkey(), pubkey_b64_from_seed(&OTHER_SEED));
}

#[test]
fn a_sha256_pin_that_does_not_match_the_archive_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");

    let verification = BundleVerification {
        sha256: Some("0".repeat(64)),
        ed25519_public_keys: vec![pubkey()],
    };
    let err = verify_bundle(&packed.archive, &verification, "0.49.0").expect_err("must be refused");

    assert!(
        err.to_string().contains("profile pins"),
        "the refusal names the digest pin: {err}"
    );
}

#[test]
fn a_bundle_requiring_a_newer_core_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.99");

    let err = verify_bundle(&packed.archive, &pinned(), "0.49.0").expect_err("must be refused");

    assert!(
        err.to_string().contains("requires core"),
        "the refusal names the core range: {err}"
    );
}

#[test]
fn a_tampered_file_fails_the_extracted_checksum() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let dest = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    extract_tarball(&packed.archive, dest.path(), &opts()).expect("extract");

    write(dest.path(), "config/config.yaml", "version: 666\n");
    let err = verify_extracted(dest.path(), &packed.signed.manifest).expect_err("must be refused");

    assert!(
        err.to_string().contains("config/config.yaml"),
        "the refusal names the file: {err}"
    );
}

#[test]
fn a_file_the_manifest_does_not_list_fails_verification() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let dest = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    extract_tarball(&packed.archive, dest.path(), &opts()).expect("extract");

    write(dest.path(), "config/extra.yaml", "smuggled: true\n");
    let err = verify_extracted(dest.path(), &packed.signed.manifest).expect_err("must be refused");

    assert!(
        err.to_string().contains("not listed in the manifest"),
        "the refusal names the surplus file: {err}"
    );
}

#[test]
fn a_tampered_manifest_no_longer_verifies() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let archive = out.path().join("b.tar.gz");
    let packed = pack(src.path(), &archive, "1.0.0", ">=0.1");

    let mut signed = packed.signed.clone();
    signed.manifest.version = "6.6.6".to_owned();
    let repacked = out.path().join("tampered.tar.gz");
    systemprompt_loader::bundle::pack::write_tarball(src.path(), &signed, &repacked)
        .expect("repack");

    let err = verify_bundle(&repacked, &pinned(), "0.49.0").expect_err("must be refused");

    assert!(
        err.to_string().contains("signature does not verify"),
        "editing the manifest must break the signature: {err}"
    );
    assert!(fs::metadata(&archive).is_ok());
}
