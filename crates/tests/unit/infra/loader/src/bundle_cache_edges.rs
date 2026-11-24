//! The cache's own state file, current-symlink swap, and pruning.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_identifiers::ContentHash;
use systemprompt_loader::bundle::BundleCache;
use systemprompt_manifest::services::bundle::{BundleSourceState, ServicesBundleState};

fn state(hash: &str) -> ServicesBundleState {
    ServicesBundleState {
        composed_hash: hash.to_owned(),
        last_reconciled_hash: Some(hash.to_owned()),
        sources: std::collections::BTreeMap::from([(
            "base".to_owned(),
            BundleSourceState {
                digest: "sha256:aa".to_owned(),
                version: "1.2.3".to_owned(),
                content_hash: ContentHash::new(hash),
                fetched_at: chrono::Utc::now(),
            },
        )]),
    }
}

fn dir_with(root: &Path, name: &str) -> std::path::PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).expect("mkdir");
    path
}

#[test]
fn a_missing_state_file_reads_as_the_default_state() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());

    assert_eq!(cache.read_state(), ServicesBundleState::default());
    assert_eq!(cache.current_root(), None);
}

#[test]
fn a_corrupt_state_file_reads_as_the_default_rather_than_failing_boot() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    cache.prepare().expect("prepare");
    fs::write(cache.state_path(), "{ not json").expect("write state");

    assert_eq!(cache.read_state(), ServicesBundleState::default());
}

#[test]
fn state_written_to_the_cache_reads_back_unchanged() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());

    let written = state("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    cache.write_state(&written).expect("write state");

    assert_eq!(cache.read_state(), written);
    assert!(
        !root.path().join("state.json.tmp").exists(),
        "the temp file is renamed away, never left behind"
    );
    assert_eq!(cache.root(), root.path());
    assert_eq!(
        cache.bundle_dir("base", &ContentHash::new("abc")),
        root.path().join("bundles").join("base").join("abc")
    );
    assert_eq!(
        cache.composed_dir("abc"),
        root.path().join("composed").join("abc")
    );
}

#[test]
fn swapping_current_repoints_the_link_at_the_new_composition() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    let first = dir_with(root.path(), "composed/one");
    let second = dir_with(root.path(), "composed/two");
    fs::write(first.join("marker"), "one").expect("write");
    fs::write(second.join("marker"), "two").expect("write");

    cache.swap_current(&first).expect("first swap");
    assert_eq!(
        fs::read_to_string(cache.current_link().join("marker")).expect("read"),
        "one"
    );

    cache.swap_current(&second).expect("second swap");

    assert_eq!(
        fs::read_to_string(cache.current_link().join("marker")).expect("read"),
        "two",
        "a swap re-points the link rather than failing on the existing one"
    );
    assert_eq!(cache.current_root(), Some(cache.current_link()));
}

#[test]
fn pruning_keeps_the_two_newest_versions_of_each_bundle() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    cache.prepare().expect("prepare");
    for hash in ["v1", "v2", "v3"] {
        let dir = cache.bundle_dir("base", &ContentHash::new(hash));
        fs::create_dir_all(&dir).expect("mkdir");
        fs::write(dir.join("marker"), hash).expect("write");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    cache.gc(&ServicesBundleState::default()).expect("gc");

    assert!(
        !cache.bundle_dir("base", &ContentHash::new("v1")).exists(),
        "the oldest is pruned"
    );
    assert!(cache.bundle_dir("base", &ContentHash::new("v2")).exists());
    assert!(cache.bundle_dir("base", &ContentHash::new("v3")).exists());
}

#[test]
fn pruning_never_removes_the_bundle_a_source_is_held_on() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    cache.prepare().expect("prepare");
    for hash in ["held", "v2", "v3", "v4"] {
        fs::create_dir_all(cache.bundle_dir("base", &ContentHash::new(hash))).expect("mkdir");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    cache.gc(&state("held")).expect("gc");

    assert!(
        cache.bundle_dir("base", &ContentHash::new("held")).exists(),
        "the bundle the state records survives newer ones that were never served"
    );
    assert!(cache.bundle_dir("base", &ContentHash::new("v4")).exists());
    assert!(!cache.bundle_dir("base", &ContentHash::new("v2")).exists());
}

#[test]
fn a_refusal_record_that_cannot_be_read_is_an_error() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    assert!(
        cache.read_refusals().expect("no record").is_empty(),
        "an absent record means nothing was held back"
    );

    fs::write(root.path().join("refusals.json"), "[{\"source\":").expect("write");
    assert!(cache.read_refusals().is_err(), "a truncated record");

    fs::write(
        root.path().join("refusals.json"),
        r#"[{"source":"kit","kind":"a_later_kind","reason":"r","serving_version":"1.0.0","serving_digest":"sha256:aa","recorded_at":"2026-11-24T00:00:00Z"}]"#,
    )
    .expect("write");
    assert!(
        cache.read_refusals().is_err(),
        "a record a later core wrote is not read as empty"
    );
}

#[test]
fn pruning_never_removes_the_composition_currently_in_use() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());
    cache.prepare().expect("prepare");
    for hash in ["pinned", "c2", "c3", "c4"] {
        fs::create_dir_all(cache.composed_dir(hash)).expect("mkdir");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    cache.gc(&state("pinned")).expect("gc");

    assert!(
        cache.composed_dir("pinned").exists(),
        "the oldest composition survives because it is the live one"
    );
    assert!(cache.composed_dir("c4").exists());
    assert!(cache.composed_dir("c3").exists());
    assert!(!cache.composed_dir("c2").exists());
}

#[test]
fn pruning_an_empty_cache_is_a_no_op() {
    let root = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(root.path());

    cache
        .gc(&state("anything"))
        .expect("gc on an unprepared cache");

    assert!(!root.path().join("bundles").exists());
}
