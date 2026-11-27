use std::path::{Path, PathBuf};

use harness_workspace::SnapshotError;

use crate::support::{fixture, put};

#[test]
fn nested_files_are_found_by_relative_and_absolute_path() {
    let f = fixture();
    put(&f.root, "a/b/c.txt", "deep\n");
    let base = f.store.track().unwrap();
    let relative: PathBuf = ["a", "b", "c.txt"].iter().collect();
    assert_eq!(
        f.store.file_at(&base, &relative).unwrap(),
        Some(b"deep\n".to_vec())
    );
    let absolute = f.root.join("a").join("b").join("c.txt");
    assert_eq!(
        f.store.file_at(&base, &absolute).unwrap(),
        Some(b"deep\n".to_vec())
    );
}

#[test]
fn parent_components_are_outside_the_worktree() {
    let f = fixture();
    put(&f.root, "x", "x\n");
    let base = f.store.track().unwrap();
    assert!(matches!(
        f.store.file_at(&base, Path::new("../x")),
        Err(SnapshotError::OutsideWorktree { .. })
    ));
}

#[test]
fn absolute_paths_elsewhere_are_outside_the_worktree() {
    let f = fixture();
    put(&f.root, "x", "x\n");
    let base = f.store.track().unwrap();
    let elsewhere = f.root.parent().unwrap().join("other");
    assert!(matches!(
        f.store.file_at(&base, &elsewhere),
        Err(SnapshotError::OutsideWorktree { .. })
    ));
}

#[test]
fn diff_paths_use_one_separator_per_component() {
    let f = fixture();
    let base = f.store.track().unwrap();
    put(&f.root, "a/b/c.txt", "new\n");
    let diffs = f
        .store
        .diff(&base, &harness_workspace::DiffTarget::Working)
        .unwrap();
    let expected: PathBuf = ["a", "b", "c.txt"].iter().collect();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].path, expected);
}
