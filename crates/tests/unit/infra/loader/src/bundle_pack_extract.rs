//! Packing a services tree and unpacking it again.
//!
//! The manifest is what an instance trusts, so these assert the two
//! properties that make it trustworthy: it is reproducible from the tree, and
//! its ownership claims are derived from what the tree actually contains
//! rather than declared by the publisher.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::fs;

use systemprompt_loader::bundle::pack::{build_manifest, collect_files, derive_ownership};
use systemprompt_loader::bundle::{ExtractOptions, TarLayout, extract_tarball};
use systemprompt_models::services::bundle::{BUNDLE_ALLOWED_DIRS, BundleSourceInfo};

use crate::bundle_support::{base_tree, marketplace_tree, pack, write};

fn opts() -> ExtractOptions<'static> {
    ExtractOptions {
        allowed_dirs: BUNDLE_ALLOWED_DIRS,
        max_bytes: 16 * 1024 * 1024,
        layout: TarLayout::Bundle,
    }
}

#[test]
fn the_content_hash_is_reproducible_for_the_same_tree() {
    let a = tempfile::tempdir().expect("tempdir");
    let b = tempfile::tempdir().expect("tempdir");
    base_tree(a.path());
    base_tree(b.path());
    write(b.path(), "config/config.yaml", "version: 1\n");

    let first = build_manifest(a.path(), "1.0.0", ">=0.1", BundleSourceInfo::default())
        .expect("manifest a");
    let second =
        build_manifest(b.path(), "9.9.9", ">=2", BundleSourceInfo::default()).expect("manifest b");

    assert_eq!(
        first.content_hash, second.content_hash,
        "the content hash covers the tree, not the version or the core range"
    );
}

#[test]
fn the_file_list_is_sorted_regardless_of_creation_order() {
    let dir = tempfile::tempdir().expect("tempdir");
    write(dir.path(), "skills/z/config.yaml", "z");
    write(dir.path(), "agents/a/config.yaml", "a");
    write(dir.path(), "config/config.yaml", "c");

    let files = collect_files(dir.path(), BUNDLE_ALLOWED_DIRS).expect("collect");
    let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();

    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(paths, sorted, "the walk must be order-independent");
}

#[test]
fn ownership_is_derived_from_the_directories_present() {
    let dir = tempfile::tempdir().expect("tempdir");
    marketplace_tree(dir.path(), "uk", "sales");
    write(dir.path(), "skills/pitch/config.yaml", "id: pitch\n");

    let owns = derive_ownership(dir.path()).expect("ownership");

    assert_eq!(owns.marketplaces, vec!["uk".to_owned()]);
    assert_eq!(owns.plugins, vec!["sales".to_owned()]);
    assert_eq!(owns.skills, vec!["pitch".to_owned()]);
    assert!(
        owns.dirs.contains(&"marketplaces".to_owned())
            && !owns.dirs.contains(&"access-control".to_owned()),
        "only directories that exist are claimed: {:?}",
        owns.dirs
    );
}

#[test]
fn a_packed_bundle_round_trips_through_extraction() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let dest = tempfile::tempdir().expect("tempdir");
    base_tree(src.path());

    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    let written = extract_tarball(&packed.archive, dest.path(), &opts()).expect("extract");

    assert!(
        written.iter().any(|p| p.ends_with("config/config.yaml")),
        "the tree lands at the destination root: {written:?}"
    );
    assert!(
        dest.path().join("bundle.json").is_file(),
        "bundle.json is kept beside the tree so the cache entry is self-describing"
    );
    assert_eq!(
        fs::read_to_string(dest.path().join("config/config.yaml")).expect("read"),
        "version: 1\n"
    );
}

#[test]
fn a_bundle_entry_outside_the_services_prefix_is_refused() {
    let dir = tempfile::tempdir().expect("tempdir");
    let dest = tempfile::tempdir().expect("tempdir");
    let archive = dir.path().join("hostile.tar.gz");
    let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
        fs::File::create(&archive).expect("create"),
        flate2::Compression::none(),
    ));
    let body = b"x".as_slice();
    let mut header = tar::Header::new_gnu();
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder
        .append_data(&mut header, "elsewhere/evil.yaml", body)
        .expect("append");
    builder.finish().expect("finish");
    drop(builder);

    let err = extract_tarball(&archive, dest.path(), &opts()).expect_err("must be refused");

    assert!(
        err.to_string().contains("bundle.json or under services/"),
        "the refusal names the layout rule: {err}"
    );
}

#[test]
fn an_archive_over_the_size_cap_is_refused() {
    let src = tempfile::tempdir().expect("tempdir");
    let out = tempfile::tempdir().expect("tempdir");
    let dest = tempfile::tempdir().expect("tempdir");
    write(src.path(), "config/config.yaml", &"a".repeat(4096));

    let packed = pack(src.path(), &out.path().join("b.tar.gz"), "1.0.0", ">=0.1");
    let capped = ExtractOptions {
        max_bytes: 512,
        ..opts()
    };

    let err = extract_tarball(&packed.archive, dest.path(), &capped).expect_err("must be refused");

    assert!(
        err.to_string().contains("byte limit"),
        "the refusal names the cap: {err}"
    );
}
