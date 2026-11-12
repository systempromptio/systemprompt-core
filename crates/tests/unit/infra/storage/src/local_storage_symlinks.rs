use crate::support::ReadAll;
use std::path::Path;

use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};
use tempfile::TempDir;

struct Fixture {
    _root: TempDir,
    outside: TempDir,
    storage: LocalFileStorage,
}

fn fixture() -> Fixture {
    let root = TempDir::new().expect("root");
    let outside = TempDir::new().expect("outside");
    std::fs::write(outside.path().join("secret.txt"), b"outside the root").expect("seed");
    std::os::unix::fs::symlink(outside.path(), root.path().join("escape")).expect("dir link");
    std::os::unix::fs::symlink(
        outside.path().join("secret.txt"),
        root.path().join("leaf.txt"),
    )
    .expect("file link");
    let storage = LocalFileStorage::new(root.path().to_path_buf());
    Fixture {
        _root: root,
        outside,
        storage,
    }
}

fn is_validation<T>(result: &Result<T, FileStorageError>) -> bool {
    matches!(result, Err(FileStorageError::Validation(_)))
}

#[tokio::test]
async fn reads_through_a_symlinked_directory_are_refused() {
    let f = fixture();
    let id = StoredFileId::new("escape/secret.txt");
    assert!(is_validation(&f.storage.read_all(&id).await));
    assert!(is_validation(&f.storage.metadata(&id).await));
    assert!(is_validation(&f.storage.exists(&id).await));
}

#[tokio::test]
async fn a_symlinked_leaf_file_is_refused() {
    let f = fixture();
    let id = StoredFileId::new("leaf.txt");
    assert!(is_validation(&f.storage.read_all(&id).await));
    assert!(is_validation(&f.storage.delete(&id).await));
    assert!(f.outside.path().join("secret.txt").is_file());
}

#[tokio::test]
async fn writes_through_a_symlinked_directory_are_refused_and_create_nothing_outside() {
    let f = fixture();
    let stored = f.storage.store(Path::new("escape/planted.txt"), b"x").await;
    assert!(is_validation(&stored));
    assert!(!f.outside.path().join("planted.txt").exists());
}

#[tokio::test]
async fn ordinary_nested_paths_still_round_trip() {
    let f = fixture();
    let id = f
        .storage
        .store(Path::new("real/dir/a.txt"), b"inside")
        .await
        .expect("store");
    assert_eq!(f.storage.read_all(&id).await.expect("read"), b"inside");
}
