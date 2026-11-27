use std::fs;
use std::path::Path;
use std::time::Duration;

use harness_workspace::{
    DiffSummary, DiffTarget, FileStatus, PrunePolicy, SnapshotError, SnapshotId, SnapshotOptions,
};

use crate::support::{fixture, fixture_with, put, read, statuses};

#[test]
fn same_tree_gives_same_id() {
    let f = fixture();
    put(&f.root, "a.txt", "one\n");
    let first = f.store.track().unwrap();
    let second = f.store.track().unwrap();
    assert_eq!(first, second);
    put(&f.root, "a.txt", "two\n");
    assert_ne!(first, f.store.track().unwrap());
}

#[test]
fn diff_against_working_reports_add_modify_delete() {
    let f = fixture();
    put(&f.root, "keep.txt", "same\n");
    put(&f.root, "edit.txt", "a\nb\n");
    put(&f.root, "gone.txt", "x\ny\nz\n");
    let base = f.store.track().unwrap();

    put(&f.root, "edit.txt", "a\nc\nd\n");
    fs::remove_file(f.root.join("gone.txt")).unwrap();
    put(&f.root, "nested/deep/new.txt", "fresh\n");

    let diffs = f.store.diff(&base, &DiffTarget::Working).unwrap();
    assert_eq!(
        statuses(&diffs),
        vec![
            ("edit.txt".to_owned(), FileStatus::Modified),
            ("gone.txt".to_owned(), FileStatus::Deleted),
            ("nested/deep/new.txt".to_owned(), FileStatus::Added),
        ]
    );
    let summary = DiffSummary::from_diffs(&diffs);
    assert_eq!(
        summary,
        DiffSummary {
            additions: 3,
            deletions: 4,
            files: 3
        }
    );
    let edit = &diffs[0];
    assert_eq!((edit.additions, edit.deletions), (2, 1));
    assert!(edit.patch.contains("--- a/edit.txt"));
}

#[test]
fn diff_between_snapshots() {
    let f = fixture();
    put(&f.root, "a.txt", "1\n");
    let first = f.store.track().unwrap();
    put(&f.root, "b.txt", "2\n");
    let second = f.store.track().unwrap();
    let diffs = f
        .store
        .diff(&first, &DiffTarget::Snapshot(second.clone()))
        .unwrap();
    assert_eq!(
        statuses(&diffs),
        vec![("b.txt".to_owned(), FileStatus::Added)]
    );
    assert_eq!(
        f.store
            .diff(&second, &DiffTarget::Snapshot(second.clone()))
            .unwrap(),
        Vec::<harness_workspace::FileDiff>::new()
    );
}

#[test]
fn restore_rolls_back_everything() {
    let f = fixture();
    put(&f.root, "a.txt", "original\r\nline\r\n");
    put(&f.root, "dir/b.txt", "b\n");
    let base = f.store.track().unwrap();

    put(&f.root, "a.txt", "changed\n");
    fs::remove_file(f.root.join("dir/b.txt")).unwrap();
    put(&f.root, "added/deep/c.txt", "c\n");

    let touched = f.store.restore(&base).unwrap();
    assert_eq!(touched.len(), 3);
    assert_eq!(
        read(&f.root, "a.txt").as_deref(),
        Some("original\r\nline\r\n")
    );
    assert_eq!(read(&f.root, "dir/b.txt").as_deref(), Some("b\n"));
    assert!(!f.root.join("added").exists());
    assert_eq!(
        f.store.changed_files(&base).unwrap(),
        Vec::<std::path::PathBuf>::new()
    );
}

#[test]
fn ignored_files_are_left_alone() {
    let f = fixture();
    put(&f.root, ".gitignore", "build/\n*.log\n");
    put(&f.root, "src.txt", "s\n");
    let base = f.store.track().unwrap();

    put(&f.root, "build/out.bin", "artifact");
    put(&f.root, "run.log", "log");
    assert_eq!(
        f.store.changed_files(&base).unwrap(),
        Vec::<std::path::PathBuf>::new()
    );

    f.store.restore(&base).unwrap();
    assert!(f.root.join("build/out.bin").exists());
    assert!(f.root.join("run.log").exists());
}

#[test]
fn oversized_files_are_skipped() {
    let f = fixture_with(SnapshotOptions {
        max_file_size: 4,
        ..SnapshotOptions::default()
    });
    put(&f.root, "small.txt", "ok");
    put(&f.root, "large.txt", "too large");
    let base = f.store.track().unwrap();
    assert!(
        f.store
            .file_at(&base, Path::new("large.txt"))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        f.store.file_at(&base, Path::new("small.txt")).unwrap(),
        Some(b"ok".to_vec())
    );
    fs::remove_file(f.root.join("large.txt")).unwrap();
    f.store.restore(&base).unwrap();
    assert!(!f.root.join("large.txt").exists());
}

#[test]
fn read_only_files_are_restored() {
    let f = fixture();
    put(&f.root, "ro.txt", "first\n");
    let base = f.store.track().unwrap();
    put(&f.root, "ro.txt", "second\n");
    let path = f.root.join("ro.txt");
    let mut permissions = fs::metadata(&path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(&path, permissions).unwrap();

    f.store.restore(&base).unwrap();
    assert_eq!(read(&f.root, "ro.txt").as_deref(), Some("first\n"));
    assert!(fs::metadata(&path).unwrap().permissions().readonly());
}

#[cfg(unix)]
#[test]
fn symlinks_are_not_snapshotted() {
    let f = fixture();
    put(&f.root, "target.txt", "t\n");
    std::os::unix::fs::symlink(f.root.join("target.txt"), f.root.join("link.txt")).unwrap();
    let base = f.store.track().unwrap();
    assert!(
        f.store
            .file_at(&base, Path::new("link.txt"))
            .unwrap()
            .is_none()
    );
    f.store.restore(&base).unwrap();
    assert!(
        f.root
            .join("link.txt")
            .symlink_metadata()
            .unwrap()
            .file_type()
            .is_symlink()
    );
}

#[test]
fn unknown_snapshot_is_not_found() {
    let f = fixture();
    let id: SnapshotId = "0".repeat(64).parse().unwrap();
    assert!(matches!(
        f.store.restore(&id),
        Err(SnapshotError::NotFound(_))
    ));
    assert!("not-hex".parse::<SnapshotId>().is_err());
}

#[test]
fn prune_drops_old_snapshots_and_their_objects() {
    let f = fixture();
    put(&f.root, "a.txt", "first\n");
    let first = f.store.track().unwrap();
    put(&f.root, "a.txt", "second\n");
    let second = f.store.track().unwrap();
    std::thread::sleep(Duration::from_millis(20));

    let report = f
        .store
        .prune(PrunePolicy {
            max_age: Duration::ZERO,
            keep_latest: 1,
        })
        .unwrap();
    assert_eq!(report.snapshots_removed, 1);
    assert_eq!(report.objects_removed, 1);
    assert_eq!(f.store.list().unwrap().len(), 1);
    assert_ne!(f.store.exists(&second), f.store.exists(&first));
}
