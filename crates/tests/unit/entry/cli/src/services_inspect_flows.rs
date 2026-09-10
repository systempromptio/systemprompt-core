//! `core services inspect` — signature verdicts and the active composition.
//!
//! The signature line is the only thing telling an operator whether the
//! archive in front of them was signed by a key their own profile trusts, so
//! each verdict is distinguished rather than collapsed into "signed".

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::{Path, PathBuf};

use systemprompt_cli::core::services::bundle::{BundleArgs, pack_bundle};
use systemprompt_cli::core::services::inspect::{InspectArgs, describe_bundle, execute};
use systemprompt_cli::core::services::signing::BundleSigningKey;
use systemprompt_models::Profile;
use systemprompt_models::services::bundle::SignedBundleManifest;

use crate::services_profile_fixture as fx;

fn tree(dir: &Path) -> PathBuf {
    let root = dir.join("tree");
    let config = root.join("plugins/alpha/config.yaml");
    std::fs::create_dir_all(config.parent().expect("parent")).expect("mkdir");
    std::fs::write(config, "plugin:\n  id: alpha\n  version: 1.0.0\n").expect("write");
    std::fs::create_dir_all(root.join("skills/alpha_report")).expect("mkdir");
    std::fs::write(
        root.join("skills/alpha_report/config.yaml"),
        "skill:\n  id: x\n",
    )
    .expect("write");
    root
}

fn packed(dir: &Path, key: Option<&BundleSigningKey>) -> SignedBundleManifest {
    let out = dir.join("bundle.tar.gz");
    pack_bundle(
        &BundleArgs {
            root: tree(dir),
            out: out.clone(),
            version: "1.4.0".to_owned(),
            sign_key: None,
            source_repo: None,
            source_commit: None,
            workflow_run: None,
            marketplace_only: false,
        },
        key,
    )
    .expect("pack succeeds");
    systemprompt_loader::bundle::verify::read_manifest(&out).expect("manifest reads")
}

fn profile_pinning(keys: &[String]) -> (fx::ProfileTree, Profile) {
    fx::loaded(&fx::https_source_with_keys(
        "base",
        "https://example.test/bundle.tar.gz",
        keys,
    ))
}

#[test]
fn a_signed_archive_read_without_a_profile_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let signed = packed(dir.path(), Some(&BundleSigningKey::generate()));
    let report = describe_bundle(&signed, None);
    assert_eq!(
        report.signature_status,
        "signed (no profile to check pinned keys against)"
    );
    assert!(report.signature_key_id.is_some());
}

#[test]
fn a_profile_that_pins_nothing_cannot_vouch_for_a_signature() {
    let dir = tempfile::tempdir().expect("tempdir");
    let signed = packed(dir.path(), Some(&BundleSigningKey::generate()));
    let (_tree, profile) = profile_pinning(&[]);
    let report = describe_bundle(&signed, Some(&profile));
    assert_eq!(report.signature_status, "signed (profile pins no keys)");
}

#[test]
fn a_signature_from_a_pinned_key_verifies() {
    let dir = tempfile::tempdir().expect("tempdir");
    let key = BundleSigningKey::generate();
    let signed = packed(dir.path(), Some(&key));
    let (_tree, profile) = profile_pinning(&[key.public_key.clone()]);
    let report = describe_bundle(&signed, Some(&profile));
    assert_eq!(report.signature_status, "verified against a pinned key");
}

#[test]
fn a_signature_from_an_unpinned_key_is_reported_as_untrusted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let signed = packed(dir.path(), Some(&BundleSigningKey::generate()));
    let other = BundleSigningKey::generate();
    let (_tree, profile) = profile_pinning(&[other.public_key]);
    let report = describe_bundle(&signed, Some(&profile));
    assert_eq!(
        report.signature_status,
        "signed by a key this profile does not pin"
    );
}

#[test]
fn the_ownership_summary_counts_what_the_bundle_declares() {
    let dir = tempfile::tempdir().expect("tempdir");
    let signed = packed(dir.path(), None);
    let report = describe_bundle(&signed, None);
    assert!(report.owns.contains("1 plugin(s)"), "{}", report.owns);
    assert!(report.owns.contains("skill(s)"), "{}", report.owns);
    assert!(report.files > 0);
    assert!(report.total_size > 0);
}

#[test]
fn inspect_without_a_target_explains_the_two_modes() {
    let error = execute(&InspectArgs {
        bundle: None,
        active: false,
    })
    .expect_err("one of the two flags is required");
    let rendered = format!("{error:#}");
    assert!(rendered.contains("--bundle"), "{rendered}");
    assert!(rendered.contains("--active"), "{rendered}");
}

#[test]
fn inspect_names_an_archive_it_cannot_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("absent.tar.gz");
    let error = execute(&InspectArgs {
        bundle: Some(missing),
        active: false,
    })
    .expect_err("a missing archive cannot be inspected");
    assert!(format!("{error:#}").contains("absent.tar.gz"), "{error:#}");
}
