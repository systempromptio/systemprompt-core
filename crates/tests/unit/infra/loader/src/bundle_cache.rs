//! Reading a cached bundle's manifest back out of the cache.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;

use systemprompt_loader::bundle::BundleCache;
use systemprompt_models::services::bundle::BUNDLE_MANIFEST_FILE;

use crate::bundle_support::{base_tree, pack};

fn seed(cache: &BundleCache, name: &str) -> String {
    let tree = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    base_tree(tree.path());
    let packed = pack(tree.path(), &out.path().join("b.tar.gz"), "1.2.3", ">=0.1");

    let hash = packed.signed.manifest.content_hash.clone();
    let dir = cache.bundle_dir(name, &hash);
    fs::create_dir_all(&dir).expect("mkdir");
    fs::write(
        dir.join(BUNDLE_MANIFEST_FILE),
        serde_json::to_vec(&packed.signed).expect("serialise"),
    )
    .expect("write manifest");
    hash
}

#[test]
fn a_cached_manifest_reads_back_with_its_signature_intact() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let hash = seed(&cache, "base");

    let signed = cache.read_manifest("base", &hash).expect("read");

    assert_eq!(signed.manifest.version, "1.2.3");
    assert_eq!(signed.manifest.content_hash, hash);
    assert!(
        signed.signature.is_some(),
        "the signature must survive the cache round trip, or nothing downstream can re-check it"
    );
}

#[test]
fn a_missing_cache_entry_is_an_error_rather_than_an_empty_manifest() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());

    let err = cache
        .read_manifest("base", &"0".repeat(64))
        .expect_err("a missing entry must not read as a default manifest");

    assert!(
        err.to_string().contains("io error"),
        "the absence is reported, not swallowed: {err}"
    );
}

#[test]
fn a_corrupt_cached_manifest_names_the_source() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let hash = seed(&cache, "uk");
    fs::write(
        cache.bundle_dir("uk", &hash).join(BUNDLE_MANIFEST_FILE),
        b"{ truncated",
    )
    .expect("corrupt");

    let err = cache
        .read_manifest("uk", &hash)
        .expect_err("must not parse");

    assert!(
        err.to_string().contains("uk"),
        "the failure names which source's cache is bad: {err}"
    );
}
