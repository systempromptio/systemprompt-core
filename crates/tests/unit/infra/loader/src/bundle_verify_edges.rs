//! Trust-chain refusals that sit off the signed happy path: an archive that
//! carries no manifest, a manifest from a future format, a signature in an
//! algorithm we do not implement, and the marketplace-only restriction.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use flate2::Compression;
use flate2::write::GzEncoder;
use systemprompt_loader::bundle::verify::{
    file_digest, read_manifest, require_marketplace_only, verify_bundle, verify_extracted,
};
use systemprompt_models::profile::BundleVerification;
use systemprompt_models::services::bundle::{BUNDLE_MANIFEST_FILE, SignedBundleManifest};

use crate::bundle_support::{SEED, base_tree, pack, pack_with, pubkey};

fn tar_gz_with(entries: &[(&str, &[u8])], out: &Path) {
    let file = fs::File::create(out).expect("create archive");
    let mut builder = tar::Builder::new(GzEncoder::new(file, Compression::default()));
    for (name, body) in entries {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, *body)
            .expect("append");
    }
    builder
        .into_inner()
        .expect("finish tar")
        .finish()
        .expect("finish gzip");
}

fn repack(signed: &SignedBundleManifest, out: &Path) {
    tar_gz_with(
        &[(
            BUNDLE_MANIFEST_FILE,
            serde_json::to_vec(signed).expect("serialise").as_slice(),
        )],
        out,
    );
}

fn pinned() -> BundleVerification {
    BundleVerification {
        sha256: None,
        ed25519_public_keys: vec![pubkey()],
    }
}

#[test]
fn an_archive_carrying_no_manifest_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("b.tar.gz");
    tar_gz_with(&[("config/config.yaml", b"version: 1\n")], &archive);

    let err = read_manifest(&archive).expect_err("there is no manifest to believe");

    assert!(
        err.to_string().contains("bundle.json"),
        "the refusal names the missing manifest: {err}"
    );
}

#[test]
fn a_manifest_that_does_not_parse_names_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("b.tar.gz");
    tar_gz_with(&[(BUNDLE_MANIFEST_FILE, b"{ not json")], &archive);

    let err = read_manifest(&archive).expect_err("a broken manifest is not a manifest");

    assert!(
        err.to_string().contains("bundle.json does not parse"),
        "the refusal names the file: {err}"
    );
}

#[test]
fn a_missing_archive_is_reported_as_an_io_failure() {
    let dir = tempfile::tempdir().expect("tempdir");

    let err = read_manifest(&dir.path().join("absent.tar.gz")).expect_err("nothing to read");

    assert!(
        err.to_string().to_lowercase().contains("no such file"),
        "the refusal names the missing archive: {err}"
    );
}

#[test]
fn a_bundle_from_an_unsupported_format_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let mut packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    packed.signed.manifest.format = 99;
    let archive = out.path().join("future.tar.gz");
    repack(&packed.signed, &archive);

    let err =
        verify_bundle(&archive, &pinned(), "1.0.0").expect_err("a future format cannot be trusted");

    assert!(
        err.to_string().contains("99"),
        "the refusal names the format it saw: {err}"
    );
}

#[test]
fn a_signature_in_an_unimplemented_algorithm_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let mut packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    packed.signed.signature.as_mut().expect("signature").alg = "rsa-pss".to_owned();
    let archive = out.path().join("rsa.tar.gz");
    repack(&packed.signed, &archive);

    let err = verify_bundle(&archive, &pinned(), "1.0.0")
        .expect_err("an unimplemented algorithm is not a verified signature");

    assert!(
        err.to_string().contains("rsa-pss"),
        "the refusal names the algorithm: {err}"
    );
}

#[test]
fn an_unsigned_bundle_is_accepted_when_the_profile_pins_no_keys() {
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

    let verified = verify_bundle(&packed.archive, &BundleVerification::default(), "1.0.0")
        .expect("an unpinned profile trusts the transport");

    assert_eq!(verified.manifest.version, "1.0.0");
    assert!(verified.signature.is_none());
}

#[test]
fn a_requires_core_that_is_not_a_semver_range_is_a_policy_error() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack_with(
        src.path(),
        &out.path().join("b.tar.gz"),
        "1.0.0",
        "not-a-range",
        Some(&SEED),
    );

    let err = verify_bundle(&packed.archive, &BundleVerification::default(), "1.0.0")
        .expect_err("an unparseable range cannot be satisfied");

    assert!(
        err.to_string().contains("requires_core"),
        "the error names the field: {err}"
    );
}

#[test]
fn a_matching_sha256_pin_lets_verification_continue() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "2.0.0", ">=0.1");
    let verification = BundleVerification {
        sha256: Some(file_digest(&packed.archive).expect("digest").to_uppercase()),
        ed25519_public_keys: vec![pubkey()],
    };

    let verified = verify_bundle(&packed.archive, &verification, "1.0.0").expect("verify");

    assert_eq!(verified.manifest.version, "2.0.0");
}

#[test]
fn a_bundle_owning_a_directory_outside_the_marketplace_set_is_not_marketplace_only() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");

    let err = require_marketplace_only(&packed.signed.manifest)
        .expect_err("config is not a marketplace directory");

    assert!(
        err.to_string().contains("access-control"),
        "the refusal names the offending directory: {err}"
    );
}

#[test]
fn a_bundle_owning_only_marketplace_directories_is_accepted() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(src.path().join("marketplaces/base")).expect("mkdir");
    fs::write(
        src.path().join("marketplaces/base/config.yaml"),
        "id: base\n",
    )
    .expect("write");
    fs::create_dir_all(src.path().join("plugins/demo")).expect("mkdir");
    fs::write(src.path().join("plugins/demo/config.yaml"), "id: demo\n").expect("write");
    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");

    require_marketplace_only(&packed.signed.manifest).expect("marketplace-only bundle");
}

#[test]
fn a_manifest_whose_content_hash_was_edited_no_longer_verifies_the_tree() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());
    let mut packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    packed.signed.manifest.content_hash = "0".repeat(64);

    let err = verify_extracted(src.path(), &packed.signed.manifest)
        .expect_err("the recomputed content hash does not match");

    assert!(
        err.to_string().to_lowercase().contains("content hash"),
        "the refusal names the content hash: {err}"
    );
}
