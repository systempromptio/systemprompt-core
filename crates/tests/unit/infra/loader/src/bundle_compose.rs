//! Overlaying several bundles into one services root.
//!
//! The interesting cases are the refusals: composition must never silently
//! pick a winner when two bundles claim the same id, because that would make
//! the active access rules depend on the order the profile happens to list
//! its sources in.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;
use std::path::Path;

use systemprompt_loader::bundle::pack::build_manifest;
use systemprompt_loader::bundle::{BundleCache, BundleMember, compose, composed_hash};
use systemprompt_models::services::bundle::{BundleSourceInfo, ServicesBundleManifest};

use crate::bundle_support::{base_tree, marketplace_tree, write};

fn stage(cache: &BundleCache, name: &str, build: impl Fn(&Path)) -> ServicesBundleManifest {
    let tree = tempfile::tempdir().expect("tempdir");
    build(tree.path());
    let manifest = build_manifest(tree.path(), "1.0.0", ">=0.1", BundleSourceInfo::default())
        .expect("manifest");
    let target = cache.bundle_dir(name, &manifest.content_hash);
    fs::create_dir_all(target.parent().expect("parent")).expect("mkdir");
    copy_dir(tree.path(), &target);
    manifest
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for entry in fs::read_dir(from).expect("read_dir") {
        let path = entry.expect("entry").path();
        let target = to.join(path.file_name().expect("name"));
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            fs::copy(&path, &target).expect("copy");
        }
    }
}

#[test]
fn two_marketplace_bundles_compose_into_one_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let base = stage(&cache, "base", base_tree);
    let uk = stage(&cache, "uk", |root| marketplace_tree(root, "uk", "sales"));

    let members = vec![
        BundleMember {
            name: "base".to_owned(),
            content_hash: base.content_hash.clone(),
            manifest: &base,
        },
        BundleMember {
            name: "uk".to_owned(),
            content_hash: uk.content_hash.clone(),
            manifest: &uk,
        },
    ];
    let (root, hash) = compose(&cache, &members).expect("compose");

    assert!(
        root.join("config/config.yaml").is_file(),
        "base content is present"
    );
    assert!(
        root.join("marketplaces/uk/config.yaml").is_file(),
        "overlay content is present"
    );
    assert_eq!(
        hash,
        composed_hash(&members),
        "the root is named by its members"
    );
}

#[test]
fn recomposing_the_same_members_reuses_the_existing_root() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let base = stage(&cache, "base", base_tree);
    let members = vec![BundleMember {
        name: "base".to_owned(),
        content_hash: base.content_hash.clone(),
        manifest: &base,
    }];

    let (first, first_hash) = compose(&cache, &members).expect("compose");
    write(&first, "config/marker.yaml", "kept\n");
    let (second, second_hash) = compose(&cache, &members).expect("recompose");

    assert_eq!(first, second);
    assert_eq!(first_hash, second_hash);
    assert!(
        second.join("config/marker.yaml").is_file(),
        "an unchanged composition is not rebuilt"
    );
}

#[test]
fn a_plugin_claimed_by_two_bundles_is_refused_naming_both() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let first = stage(&cache, "uk", |root| marketplace_tree(root, "uk", "sales"));
    let second = stage(&cache, "us", |root| marketplace_tree(root, "us", "sales"));

    let members = vec![
        BundleMember {
            name: "uk".to_owned(),
            content_hash: first.content_hash.clone(),
            manifest: &first,
        },
        BundleMember {
            name: "us".to_owned(),
            content_hash: second.content_hash.clone(),
            manifest: &second,
        },
    ];
    let err = compose(&cache, &members).expect_err("must be refused");
    let text = err.to_string();

    assert!(text.contains("sales"), "the refusal names the id: {text}");
    assert!(
        text.contains("uk") && text.contains("us"),
        "the refusal names both sources: {text}"
    );
}

#[test]
fn an_overlay_bundle_carrying_a_base_only_directory_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let base = stage(&cache, "base", base_tree);
    let overlay = stage(&cache, "uk", |root| {
        marketplace_tree(root, "uk", "sales");
        write(root, "access-control/rules.yaml", "rules: []\n");
    });

    let members = vec![
        BundleMember {
            name: "base".to_owned(),
            content_hash: base.content_hash.clone(),
            manifest: &base,
        },
        BundleMember {
            name: "uk".to_owned(),
            content_hash: overlay.content_hash.clone(),
            manifest: &overlay,
        },
    ];
    let err = compose(&cache, &members).expect_err("must be refused");

    assert!(
        err.to_string().contains("access-control"),
        "the refusal names the directory an overlay may not ship: {err}"
    );
}

#[test]
fn a_failed_composition_leaves_no_partial_root_behind() {
    let dir = tempfile::tempdir().expect("tempdir");
    let cache = BundleCache::new(dir.path());
    let first = stage(&cache, "uk", |root| marketplace_tree(root, "uk", "sales"));
    let second = stage(&cache, "us", |root| marketplace_tree(root, "us", "sales"));

    let members = vec![
        BundleMember {
            name: "uk".to_owned(),
            content_hash: first.content_hash.clone(),
            manifest: &first,
        },
        BundleMember {
            name: "us".to_owned(),
            content_hash: second.content_hash.clone(),
            manifest: &second,
        },
    ];
    drop(compose(&cache, &members).expect_err("must be refused"));

    let composed = dir.path().join("composed");
    let leftovers: Vec<_> = fs::read_dir(&composed)
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    assert!(
        leftovers.is_empty(),
        "a refused composition must publish nothing: {leftovers:?}"
    );
}
