use std::fs;
use std::path::{Path, PathBuf};

use harness_workspace::{FileDiff, FileStatus, SnapshotOptions, SnapshotStore};
use tempfile::TempDir;

pub(crate) struct Fixture {
    _dirs: (TempDir, TempDir),
    pub(crate) root: PathBuf,
    pub(crate) store: SnapshotStore,
}

pub(crate) fn fixture() -> Fixture {
    fixture_with(SnapshotOptions::default())
}

pub(crate) fn fixture_with(options: SnapshotOptions) -> Fixture {
    let worktree = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let root = worktree.path().to_path_buf();
    let store = SnapshotStore::open(&root, data.path(), options).unwrap();
    Fixture {
        _dirs: (worktree, data),
        root,
        store,
    }
}

pub(crate) fn put(root: &Path, rel: &str, body: &str) {
    let path = root.join(rel);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
}

pub(crate) fn read(root: &Path, rel: &str) -> Option<String> {
    fs::read_to_string(root.join(rel)).ok()
}

pub(crate) fn statuses(diffs: &[FileDiff]) -> Vec<(String, FileStatus)> {
    diffs
        .iter()
        .map(|d| (d.path.to_string_lossy().replace('\\', "/"), d.status))
        .collect()
}
