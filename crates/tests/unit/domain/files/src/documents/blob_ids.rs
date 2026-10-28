use std::sync::Arc;

use systemprompt_files::documents::blob_store::storage_id_for;
use systemprompt_identifiers::ContentHash;
use systemprompt_provider_contracts::documents::content_hash;
use systemprompt_storage::{FileStorageBackend, build_file_storage};
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};

#[test]
fn blob_ids_live_outside_the_public_files_prefix() {
    let hash = content_hash(b"blob bytes");
    let id = storage_id_for(&hash);
    let hex = hash.as_str();
    assert_eq!(id.as_str(), format!("documents/blobs/{}/{hex}", &hex[..2]));
    assert!(!id.as_str().starts_with("files/"));
}

#[test]
fn short_hash_does_not_panic() {
    let id = storage_id_for(&ContentHash::new("a"));
    assert_eq!(id.as_str(), "documents/blobs/a/a");
}

#[tokio::test]
async fn traversal_from_files_prefix_into_blobs_is_refused() {
    let root = tempfile::tempdir().expect("tempdir");
    let storage: Arc<dyn FileStorage> = build_file_storage(FileStorageBackend::Local {
        root: root.path().to_path_buf(),
    });
    let hash = content_hash(b"secret");
    let blob_id = storage_id_for(&hash);
    storage
        .store(std::path::Path::new(blob_id.as_str()), b"secret")
        .await
        .expect("store blob");
    let escaped = StoredFileId::new(format!("files/../{}", blob_id.as_str()));
    let result = storage.open(&escaped, None).await;
    assert!(matches!(result, Err(FileStorageError::Validation(_))));
}
