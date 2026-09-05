//! Landing an admin-owned managed file: unchanged bodies are never rewritten,
//! and a write refused for permissions escalates rather than failing outright.

#![cfg(not(any(target_os = "macos", target_os = "windows")))]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use systemprompt_bridge::install::managed_file::test_api::{
    ManagedWrite, remove_managed_file, write_managed_file,
};
use tempfile::TempDir;

const PROMPT: &str = "systemprompt needs to update a managed configuration file";

fn sealed(dir: &Path) {
    let mut perms = std::fs::metadata(dir).expect("metadata").permissions();
    perms.set_mode(0o555);
    std::fs::set_permissions(dir, perms).expect("seal the directory");
}

fn unseal(dir: &Path) {
    let mut perms = std::fs::metadata(dir).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(dir, perms).expect("unseal the directory");
}

fn scratch() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    (dir, path)
}

#[test]
fn a_file_that_was_never_there_is_written_and_reported_as_written() {
    let (dir, path) = scratch();
    let outcome = write_managed_file(&path, b"{\"a\":1}", PROMPT).expect("write");
    assert_eq!(outcome, ManagedWrite::Written);
    assert_eq!(std::fs::read(&path).expect("read back"), b"{\"a\":1}");
    drop(dir);
}

#[test]
fn writing_the_same_body_again_is_reported_unchanged_and_does_not_touch_the_file() {
    let (dir, path) = scratch();
    write_managed_file(&path, b"{\"a\":1}", PROMPT).expect("first write");
    let before = std::fs::metadata(&path).expect("metadata").modified().ok();

    let outcome = write_managed_file(&path, b"{\"a\":1}", PROMPT).expect("second write");
    assert_eq!(
        outcome,
        ManagedWrite::Unchanged,
        "repeated installs and syncs must stay silent"
    );
    let after = std::fs::metadata(&path).expect("metadata").modified().ok();
    assert_eq!(before, after, "an unchanged body must not be rewritten");
    drop(dir);
}

#[test]
fn a_changed_body_replaces_the_previous_contents_and_reports_written() {
    let (dir, path) = scratch();
    write_managed_file(&path, b"old", PROMPT).expect("first write");

    let outcome = write_managed_file(&path, b"new", PROMPT).expect("second write");
    assert_eq!(outcome, ManagedWrite::Written);
    assert_eq!(std::fs::read(&path).expect("read back"), b"new");
    drop(dir);
}

#[test]
fn a_write_refused_for_permissions_reports_that_root_is_needed_and_names_the_path() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    sealed(dir.path());

    let err = write_managed_file(&path, b"{}", PROMPT)
        .expect_err("an unwritable directory cannot be escalated past on this platform");

    unseal(dir.path());

    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    let rendered = err.to_string();
    assert!(
        rendered.contains("managed-settings.json"),
        "the error must name the file, got {rendered}"
    );
    assert!(
        rendered.contains("re-run as root"),
        "there is no prompt to offer on this platform, so it must say what to re-run: {rendered}"
    );
    drop(dir);
}

#[test]
fn removing_a_file_that_is_not_there_reports_that_nothing_was_removed() {
    let (dir, path) = scratch();
    assert!(
        !remove_managed_file(&path, PROMPT).expect("an absent file is not an error"),
        "nothing was there to remove"
    );
    drop(dir);
}

#[test]
fn removing_a_file_that_is_there_reports_that_it_was_removed() {
    let (dir, path) = scratch();
    write_managed_file(&path, b"{}", PROMPT).expect("write");

    assert!(remove_managed_file(&path, PROMPT).expect("remove"));
    assert!(!path.exists());
    drop(dir);
}

#[test]
fn a_removal_refused_for_permissions_reports_that_root_is_needed() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("managed-settings.json");
    write_managed_file(&path, b"{}", PROMPT).expect("write while still writable");
    sealed(dir.path());

    let err = remove_managed_file(&path, PROMPT)
        .expect_err("an unwritable directory refuses the unlink");

    unseal(dir.path());

    assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(
        err.to_string().contains("re-run as root"),
        "got {err}"
    );
    drop(dir);
}

#[test]
fn a_failure_that_elevation_could_not_fix_is_returned_as_itself() {
    let (dir, path) = scratch();
    std::fs::create_dir(&path).expect("occupy the path with a directory");

    let err = write_managed_file(&path, b"{}", PROMPT)
        .expect_err("a directory in the way is not a permissions problem");
    assert_ne!(
        err.kind(),
        std::io::ErrorKind::PermissionDenied,
        "only permission-denied justifies escalating; this must surface as itself: {err}"
    );
    drop(dir);
}
