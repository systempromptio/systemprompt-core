use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use systemprompt_engine::workspace::VcsInfo;

use crate::workspace::vcs_index_tests::{entry_bytes, index_bytes};

pub(crate) const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

pub(crate) fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let git = dir.path().join(".git");
    fs::create_dir_all(git.join("refs").join("heads")).unwrap();
    fs::write(git.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        git.join("refs").join("heads").join("main"),
        format!("{COMMIT}\n"),
    )
    .unwrap();
    dir
}

pub(crate) fn stage(root: &Path, rel: &str) -> Vec<u8> {
    let metadata = fs::metadata(root.join(rel)).unwrap();
    let mtime = metadata
        .modified()
        .unwrap()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    entry_bytes(
        rel,
        u32::try_from(mtime).unwrap(),
        u32::try_from(metadata.len()).unwrap(),
        0o100_644,
    )
}

#[test]
fn reads_branch_and_head_from_nested_dir() {
    let dir = repo();
    let nested = dir.path().join("a").join("b");
    fs::create_dir_all(&nested).unwrap();
    let info = VcsInfo::discover(&nested).unwrap().unwrap();
    assert_eq!(info.root, dir.path());
    assert_eq!(info.branch.as_deref(), Some("main"));
    assert_eq!(info.head.as_deref(), Some(COMMIT));
    assert!(!info.is_detached());
}

#[test]
fn packed_refs_and_detached_heads() {
    let dir = repo();
    let git = dir.path().join(".git");
    fs::remove_file(git.join("refs").join("heads").join("main")).unwrap();
    fs::write(
        git.join("packed-refs"),
        format!("# pack-refs with: peeled\n{COMMIT} refs/heads/main\n"),
    )
    .unwrap();
    let info = VcsInfo::discover(dir.path()).unwrap().unwrap();
    assert_eq!(info.head.as_deref(), Some(COMMIT));

    fs::write(git.join("HEAD"), format!("{COMMIT}\n")).unwrap();
    let info = VcsInfo::discover(dir.path()).unwrap().unwrap();
    assert!(info.is_detached());
    assert_eq!(info.branch, None);
}

#[test]
fn unborn_branch_has_no_head() {
    let dir = repo();
    fs::remove_file(
        dir.path()
            .join(".git")
            .join("refs")
            .join("heads")
            .join("main"),
    )
    .unwrap();
    let info = VcsInfo::discover(dir.path()).unwrap().unwrap();
    assert_eq!(info.branch.as_deref(), Some("main"));
    assert_eq!(info.head, None);
}

#[test]
fn worktree_gitdir_file_is_followed() {
    let main = repo();
    let common = main.path().join(".git");
    let linked_git = common.join("worktrees").join("feature");
    fs::create_dir_all(&linked_git).unwrap();
    fs::write(linked_git.join("HEAD"), "ref: refs/heads/feature\n").unwrap();
    fs::write(linked_git.join("commondir"), "../..\n").unwrap();
    fs::write(
        common.join("refs").join("heads").join("feature"),
        format!("{COMMIT}\n"),
    )
    .unwrap();

    let checkout = tempfile::tempdir().unwrap();
    fs::write(
        checkout.path().join(".git"),
        format!("gitdir: {}\n", linked_git.display()),
    )
    .unwrap();
    let info = VcsInfo::discover(checkout.path()).unwrap().unwrap();
    assert_eq!(info.branch.as_deref(), Some("feature"));
    assert_eq!(info.head.as_deref(), Some(COMMIT));
}

#[test]
fn dirty_flag_tracks_changes_and_untracked_files() {
    let dir = repo();
    let root = dir.path();
    fs::write(root.join(".gitignore"), "*.log\n").unwrap();
    fs::write(root.join("a.txt"), "abc").unwrap();
    let index = index_bytes(&[stage(root, ".gitignore"), stage(root, "a.txt")]);
    fs::write(root.join(".git").join("index"), index).unwrap();
    let info = VcsInfo::discover(root).unwrap().unwrap();
    assert!(!info.is_dirty().unwrap());

    fs::write(root.join("ignored.log"), "x").unwrap();
    assert!(!info.is_dirty().unwrap());

    fs::write(root.join("new.txt"), "x").unwrap();
    assert!(info.is_dirty().unwrap());
    fs::remove_file(root.join("new.txt")).unwrap();

    fs::write(root.join("a.txt"), "abcd").unwrap();
    assert!(info.is_dirty().unwrap());
}

#[test]
fn outside_a_repository_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let plain = dir.path().join("plain");
    fs::create_dir_all(&plain).unwrap();
    let found = VcsInfo::discover(&plain).unwrap();
    assert!(found.is_none_or(|info| !info.root.starts_with(&plain)));
}
