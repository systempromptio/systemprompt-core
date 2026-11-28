//! The workspace snapshot store as a session sees it: a worktree is captured
//! and restored whatever its directory is called, and a worktree reached
//! through a symbolic link shares the store of the directory it points to.

use std::fs;
use std::path::Path;

use systemprompt_engine::workspace::{SnapshotOptions, SnapshotStore};

fn put(root: &Path, name: &str, text: &str) {
    fs::write(root.join(name), text).unwrap();
}

#[test]
fn a_worktree_whose_name_starts_with_two_dots_is_captured_and_restored() {
    let base = tempfile::tempdir().unwrap();
    let root = base.path().join("..work");
    fs::create_dir(&root).unwrap();
    let data = base.path().join("data");
    put(&root, "notes.txt", "before\n");
    let store = SnapshotStore::open(&root, &data, SnapshotOptions::default()).unwrap();
    let captured = store.track().unwrap();

    put(&root, "notes.txt", "after\n");
    put(&root, "extra.txt", "new\n");
    store.restore(&captured).unwrap();

    assert_eq!(
        fs::read_to_string(root.join("notes.txt")).unwrap(),
        "before\n"
    );
    assert!(!root.join("extra.txt").exists());
}

#[cfg(unix)]
#[test]
fn a_worktree_opened_through_a_symbolic_link_shares_the_store_of_its_real_path() {
    let base = tempfile::tempdir().unwrap();
    let real = base.path().join("real");
    fs::create_dir(&real).unwrap();
    let link = base.path().join("link");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let data = base.path().join("data");
    put(&real, "notes.txt", "before\n");

    let through_link = SnapshotStore::open(&link, &data, SnapshotOptions::default()).unwrap();
    let direct = SnapshotStore::open(&real, &data, SnapshotOptions::default()).unwrap();
    assert_eq!(through_link.store_dir(), direct.store_dir());

    let captured = through_link.track().unwrap();
    put(&real, "notes.txt", "after\n");
    direct.restore(&captured).unwrap();
    assert_eq!(
        fs::read_to_string(real.join("notes.txt")).unwrap(),
        "before\n"
    );
}
