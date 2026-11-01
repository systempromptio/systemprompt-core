use std::path::Path;

use bytes::Bytes;
use systemprompt_storage::LocalFileStorage;
use systemprompt_traits::{FileByteStream, FileStorage, FileStorageError, StoredFileId};
use tempfile::TempDir;

fn local() -> (TempDir, LocalFileStorage) {
    let dir = TempDir::new().expect("tempdir");
    let storage = LocalFileStorage::new(dir.path().to_path_buf());
    (dir, storage)
}

fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

fn stream(chunks: Vec<Result<Bytes, FileStorageError>>) -> FileByteStream {
    Box::pin(futures::stream::iter(chunks))
}

#[tokio::test]
async fn a_store_whose_target_is_a_non_empty_directory_fails_and_cleans_up() {
    let (dir, storage) = local();
    std::fs::create_dir_all(dir.path().join("files/taken")).expect("mkdir");
    std::fs::write(dir.path().join("files/taken/keep.txt"), b"keep").expect("seed");

    let err = storage
        .store(Path::new("files/taken"), b"payload")
        .await
        .expect_err("rename onto a directory");
    assert!(matches!(err, FileStorageError::Io(_)), "{err:?}");
    assert_eq!(entries(&dir.path().join("files")), vec!["taken".to_owned()]);
    assert_eq!(
        std::fs::read(dir.path().join("files/taken/keep.txt")).expect("kept"),
        b"keep"
    );
}

#[tokio::test]
async fn a_streamed_store_onto_a_directory_fails_and_cleans_up() {
    let (dir, storage) = local();
    std::fs::create_dir_all(dir.path().join("up/taken")).expect("mkdir");
    std::fs::write(dir.path().join("up/taken/keep.txt"), b"keep").expect("seed");

    storage
        .store_stream(
            Path::new("up/taken"),
            stream(vec![Ok(Bytes::from_static(b"abc"))]),
        )
        .await
        .expect_err("rename onto a directory");
    assert_eq!(entries(&dir.path().join("up")), vec!["taken".to_owned()]);
}

#[tokio::test]
async fn a_failing_body_stream_leaves_neither_target_nor_staging() {
    let (dir, storage) = local();
    let err = storage
        .store_stream(
            Path::new("up/broken.bin"),
            stream(vec![
                Ok(Bytes::from_static(b"first")),
                Err(FileStorageError::NotFound("upstream".to_owned())),
            ]),
        )
        .await
        .expect_err("body failed");
    assert!(matches!(err, FileStorageError::NotFound(ref what) if what == "upstream"));
    assert!(entries(&dir.path().join("up")).is_empty());
    assert!(
        !storage
            .exists(&StoredFileId::new("up/broken.bin"))
            .await
            .expect("exists")
    );
}

#[tokio::test]
async fn a_streamed_store_round_trips() {
    let (dir, storage) = local();
    let id = storage
        .store_stream(
            Path::new("up/ok.bin"),
            stream(vec![
                Ok(Bytes::from_static(b"ab")),
                Ok(Bytes::from_static(b"cd")),
            ]),
        )
        .await
        .expect("stored");
    assert_eq!(id.as_str(), "up/ok.bin");
    assert_eq!(
        std::fs::read(dir.path().join("up/ok.bin")).expect("read"),
        b"abcd"
    );
    assert_eq!(entries(&dir.path().join("up")), vec!["ok.bin".to_owned()]);
}

#[tokio::test]
async fn a_backend_error_keeps_the_io_cause_two_levels_down() {
    let (dir, storage) = local();
    std::fs::create_dir_all(dir.path().join("adir")).expect("mkdir");
    let err = storage
        .delete(&StoredFileId::new("adir"))
        .await
        .expect_err("cannot unlink a directory");
    let wrapper = std::error::Error::source(&err).expect("wrapper");
    assert_eq!(wrapper.to_string(), "stored file adir");
    let io = wrapper.source().expect("io cause");
    assert!(io.downcast_ref::<std::io::Error>().is_some(), "{io}");
}

#[tokio::test]
async fn metadata_of_a_directory_is_not_found() {
    let (dir, storage) = local();
    std::fs::create_dir_all(dir.path().join("bdir")).expect("mkdir");
    let err = storage
        .metadata(&StoredFileId::new("bdir"))
        .await
        .expect_err("not a file");
    assert!(matches!(err, FileStorageError::NotFound(ref id) if id == "bdir"));
}
