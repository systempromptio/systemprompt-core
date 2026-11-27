use std::path::PathBuf;

use systemprompt_engine::workspace::SnapshotError;

use crate::workspace::support::{fixture, put, read};

#[test]
fn revert_files_touches_only_the_requested_paths() {
    let f = fixture();
    put(&f.root, "a.txt", "a\n");
    put(&f.root, "b.txt", "b\n");
    put(&f.root, "dir/c.txt", "c\n");
    let base = f.store.track().unwrap();

    put(&f.root, "a.txt", "A\n");
    put(&f.root, "b.txt", "B\n");
    put(&f.root, "dir/c.txt", "C\n");
    put(&f.root, "dir/new.txt", "new\n");

    let touched = f
        .store
        .revert_files(&base, &[PathBuf::from("a.txt"), f.root.join("dir")])
        .unwrap();
    assert_eq!(touched.len(), 3);
    assert_eq!(read(&f.root, "a.txt").as_deref(), Some("a\n"));
    assert_eq!(read(&f.root, "b.txt").as_deref(), Some("B\n"));
    assert_eq!(read(&f.root, "dir/c.txt").as_deref(), Some("c\n"));
    assert!(!f.root.join("dir/new.txt").exists());
}

#[test]
fn revert_rejects_paths_outside_the_worktree() {
    let f = fixture();
    put(&f.root, "a.txt", "a\n");
    let base = f.store.track().unwrap();
    let outside = std::env::temp_dir().join("definitely-elsewhere.txt");
    assert!(matches!(
        f.store.revert_files(&base, &[outside]),
        Err(SnapshotError::OutsideWorktree { .. })
    ));
}

#[test]
fn revert_of_a_directory_leaves_similarly_named_siblings() {
    let f = fixture();
    put(&f.root, "src/a.rs", "a\n");
    put(&f.root, "srcx/b.rs", "b\n");
    let base = f.store.track().unwrap();

    put(&f.root, "src/a.rs", "A\n");
    put(&f.root, "srcx/b.rs", "B\n");

    let touched = f
        .store
        .revert_files(&base, &[PathBuf::from("src")])
        .unwrap();
    assert_eq!(touched, vec![f.root.join("src").join("a.rs")]);
    assert_eq!(read(&f.root, "src/a.rs").as_deref(), Some("a\n"));
    assert_eq!(read(&f.root, "srcx/b.rs").as_deref(), Some("B\n"));
}

#[test]
fn revert_rejects_parent_components() {
    let f = fixture();
    put(&f.root, "a.txt", "a\n");
    let base = f.store.track().unwrap();
    assert!(matches!(
        f.store.revert_files(&base, &[PathBuf::from("a/../b")]),
        Err(SnapshotError::OutsideWorktree { .. })
    ));
}
