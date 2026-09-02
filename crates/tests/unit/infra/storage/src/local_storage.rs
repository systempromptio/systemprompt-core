use std::path::Path;

use systemprompt_models::profile::StorageBackend;
use systemprompt_storage::{LocalFileStorage, build_file_storage};
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};
use tempfile::TempDir;

fn storage() -> (TempDir, LocalFileStorage) {
    let dir = TempDir::new().expect("tempdir");
    let storage = LocalFileStorage::new(dir.path().to_path_buf());
    (dir, storage)
}

#[tokio::test]
async fn store_retrieve_metadata_delete_round_trip() {
    let (dir, storage) = storage();

    let id = storage
        .store(Path::new("files/uploads/ctx/images/a.png"), b"\x89PNG")
        .await
        .expect("store");
    assert_eq!(id.as_str(), "files/uploads/ctx/images/a.png");
    assert!(dir.path().join("files/uploads/ctx/images/a.png").is_file());

    assert!(storage.exists(&id).await.expect("exists"));
    assert_eq!(storage.retrieve(&id).await.expect("retrieve"), b"\x89PNG");

    let meta = storage.metadata(&id).await.expect("metadata");
    assert_eq!(meta.id, id);
    assert_eq!(meta.path, id.as_str());
    assert_eq!(meta.mime_type, "image/png");
    assert_eq!(meta.size_bytes, Some(4));

    storage.delete(&id).await.expect("delete");
    assert!(!storage.exists(&id).await.expect("exists after delete"));
    assert!(matches!(
        storage.retrieve(&id).await,
        Err(FileStorageError::NotFound(_))
    ));
    assert!(matches!(
        storage.delete(&id).await,
        Err(FileStorageError::NotFound(_))
    ));
}

#[tokio::test]
async fn store_strips_current_dir_components_from_the_id() {
    let (_dir, storage) = storage();
    let id = storage
        .store(Path::new("./files/./x.txt"), b"x")
        .await
        .expect("store");
    assert_eq!(id.as_str(), "files/x.txt");
}

#[tokio::test]
async fn parent_dir_and_absolute_paths_are_rejected_before_touching_disk() {
    let (dir, storage) = storage();

    for bad in ["../escape.txt", "files/../../escape.txt", "/etc/passwd", ""] {
        let err = storage.store(Path::new(bad), b"nope").await.expect_err(bad);
        assert!(
            matches!(err, FileStorageError::Validation(_)),
            "{bad}: {err}"
        );
        let resolve_err = storage
            .resolve(&StoredFileId::new(bad))
            .expect_err("resolve must reject too");
        assert!(matches!(resolve_err, FileStorageError::Validation(_)));
    }
    assert!(!dir.path().join("escape.txt").exists());
    assert!(!dir.path().parent().unwrap().join("escape.txt").exists());
}

#[tokio::test]
async fn resolve_joins_the_root() {
    let (dir, storage) = storage();
    let resolved = storage
        .resolve(&StoredFileId::new("files/a/b.txt"))
        .expect("resolve");
    assert_eq!(resolved, dir.path().join("files/a/b.txt"));
    assert_eq!(storage.root(), dir.path());
}

#[tokio::test]
async fn build_file_storage_local_writes_under_the_root() {
    let dir = TempDir::new().expect("tempdir");
    let storage = build_file_storage(StorageBackend::Local, dir.path());
    let id = storage
        .store(Path::new("files/built.txt"), b"built")
        .await
        .expect("store");
    assert_eq!(
        std::fs::read(dir.path().join("files/built.txt")).expect("read"),
        b"built"
    );
    assert!(storage.public_url(&id).is_none());
}
