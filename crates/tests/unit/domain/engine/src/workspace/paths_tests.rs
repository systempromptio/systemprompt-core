use std::path::{Path, PathBuf};

use systemprompt_engine::workspace::SnapshotError;

use crate::workspace::support::{fixture, put};

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
fn a_verbatim_or_forward_slashed_path_under_the_worktree_is_inside_it() {
    let f = fixture();
    put(&f.root, "draft.txt", "kept\n");
    let base = f.store.track().unwrap();
    let slashed = f
        .root
        .join("draft.txt")
        .to_string_lossy()
        .replace('\\', "/");
    for written in [format!("//?/{slashed}"), slashed] {
        assert_eq!(
            f.store.file_at(&base, Path::new(&written)).unwrap(),
            Some(b"kept\n".to_vec()),
            "{written}"
        );
    }
}

#[test]
fn a_verbatim_path_beside_the_worktree_is_outside_it() {
    let f = fixture();
    put(&f.root, "x", "x\n");
    let base = f.store.track().unwrap();
    let beside = f.root.parent().unwrap().join("other").join("x");
    let written = format!("//?/{}", beside.to_string_lossy().replace('\\', "/"));
    assert!(matches!(
        f.store.file_at(&base, Path::new(&written)),
        Err(SnapshotError::OutsideWorktree { .. })
    ));
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
        .diff(&base, &systemprompt_engine::workspace::DiffTarget::Working)
        .unwrap();
    let expected: PathBuf = ["a", "b", "c.txt"].iter().collect();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].path, expected);
}
