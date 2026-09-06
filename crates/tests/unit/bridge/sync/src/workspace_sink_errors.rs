//! The failure arms of the workspace artifact sink: unreadable and malformed
//! manifests, a staging directory that cannot be created, stale entries that
//! refuse to be removed, and ids that must never become a path segment.

use std::fs;
use std::path::PathBuf;

use systemprompt_bridge::gateway::manifest::ArtifactEntry;
use systemprompt_bridge::ids::{LibraryArtifactId, Sha256Digest};
use systemprompt_bridge::integration::cowork_artifacts::workspace_sink::{
    BUNDLE_MANIFEST_FILE, bundle_is_current, remove_bundle, write_bundle,
};

fn tempdir() -> PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bridge-workspace-sink-err-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&p).expect("tempdir");
    p
}

fn artifact(id: &str, version: &str) -> ArtifactEntry {
    ArtifactEntry {
        id: LibraryArtifactId::try_new(id).expect("id"),
        name: format!("name of {id}"),
        description: "desc".into(),
        version: version.to_owned(),
        mcp_tools: Vec::new(),
        content: format!("<html><body id=\"{id}\"></body></html>"),
        starred: false,
        sha256: Sha256Digest::try_new(
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect("digest"),
        plugins: Vec::new(),
    }
}

#[test]
fn a_bundle_whose_manifest_was_deleted_is_not_current_even_though_its_pages_remain() {
    let dir = tempdir();
    let artifacts = vec![artifact("dashboard", "1")];
    write_bundle(&dir, &artifacts).expect("write");
    assert!(bundle_is_current(&dir, &artifacts));

    fs::remove_file(dir.join(BUNDLE_MANIFEST_FILE)).expect("delete manifest");
    assert!(
        !bundle_is_current(&dir, &artifacts),
        "an unreadable manifest means the bundle must be rewritten"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_manifest_that_is_not_valid_json_makes_the_bundle_stale_rather_than_panicking() {
    let dir = tempdir();
    let artifacts = vec![artifact("dashboard", "1")];
    write_bundle(&dir, &artifacts).expect("write");

    fs::write(dir.join(BUNDLE_MANIFEST_FILE), b"{ this is not json").expect("corrupt");
    assert!(!bundle_is_current(&dir, &artifacts));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_manifest_that_is_valid_json_of_the_wrong_shape_also_makes_the_bundle_stale() {
    let dir = tempdir();
    let artifacts = vec![artifact("dashboard", "1")];
    write_bundle(&dir, &artifacts).expect("write");

    fs::write(dir.join(BUNDLE_MANIFEST_FILE), br#"{"artifacts": 7}"#).expect("wrong shape");
    assert!(!bundle_is_current(&dir, &artifacts));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn staging_into_a_path_that_is_already_a_file_reports_which_directory_could_not_be_created() {
    let parent = tempdir();
    let blocked = parent.join("not-a-dir");
    fs::write(&blocked, b"i am a file").expect("occupy the path");

    let err = write_bundle(&blocked, &[artifact("dashboard", "1")])
        .expect_err("a file cannot be turned into the bundle directory");
    let rendered = err.to_string();
    assert!(
        rendered.contains("not-a-dir"),
        "the error must name the path it failed on, got {rendered}"
    );

    fs::remove_dir_all(&parent).ok();
}

#[test]
fn a_stale_page_that_is_a_directory_rather_than_a_file_fails_the_write_by_name() {
    let dir = tempdir();
    write_bundle(&dir, &[artifact("stale", "1")]).expect("stage the first bundle");

    fs::remove_file(dir.join("stale.html")).expect("drop the staged page");
    fs::create_dir(dir.join("stale.html")).expect("replace it with a directory");

    let err = write_bundle(&dir, &[artifact("fresh", "1")])
        .expect_err("a stale entry that cannot be removed fails the write");
    let rendered = err.to_string();
    assert!(
        rendered.contains("stale.html"),
        "the error must name the stale entry, got {rendered}"
    );

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn an_id_that_would_escape_the_bundle_directory_is_skipped_rather_than_written() {
    let root = tempdir();
    let dir = root.join("bundle");
    let escaping = artifact("../escaped", "1");
    let safe = artifact("dashboard", "1");

    write_bundle(&dir, &[escaping, safe]).expect("the unsafe id is skipped, not fatal");

    assert!(dir.join("dashboard.html").is_file());
    assert!(
        !root.join("escaped.html").exists(),
        "an unsafe id must never be written outside the bundle directory"
    );

    let manifest = fs::read_to_string(dir.join(BUNDLE_MANIFEST_FILE)).expect("manifest");
    assert!(
        !manifest.contains("escaped"),
        "a skipped artifact must not be recorded as staged, got {manifest}"
    );

    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_bundle_holding_only_unsafe_ids_stages_nothing_but_still_writes_an_empty_manifest() {
    let dir = tempdir();
    write_bundle(&dir, &[artifact("../escaped", "1")]).expect("write");

    let manifest = fs::read_to_string(dir.join(BUNDLE_MANIFEST_FILE)).expect("manifest");
    assert!(manifest.contains("\"artifacts\""));
    assert!(!manifest.contains("escaped"));

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn removing_a_bundle_directory_that_was_never_staged_succeeds_silently() {
    let dir = tempdir();
    let never = dir.join("never-staged");
    remove_bundle(&never).expect("removing an absent bundle is not an error");
    assert!(!never.exists());

    fs::remove_dir_all(&dir).ok();
}

#[test]
fn removing_a_staged_bundle_takes_its_pages_and_manifest_with_it() {
    let dir = tempdir();
    write_bundle(&dir, &[artifact("dashboard", "1")]).expect("write");
    assert!(dir.join("dashboard.html").is_file());

    remove_bundle(&dir).expect("remove");
    assert!(!dir.exists());
}

#[test]
fn a_directory_that_was_never_staged_reads_as_empty_rather_than_erroring() {
    let dir = tempdir();
    let absent = dir.join("absent");
    assert!(
        !bundle_is_current(&absent, &[artifact("dashboard", "1")]),
        "an unreadable directory holds no staged ids"
    );
    assert!(
        !bundle_is_current(&absent, &[]),
        "with no manifest to read the bundle is never current, even for an empty set"
    );

    fs::remove_dir_all(&dir).ok();
}
