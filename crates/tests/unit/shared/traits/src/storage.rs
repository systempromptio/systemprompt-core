use systemprompt_traits::storage::{FileStorageError, StoredFileId};

#[test]
fn stored_file_id_new_and_as_str() {
    let id = StoredFileId::new("file-1");
    assert_eq!(id.as_str(), "file-1");
    assert_eq!(id.0, "file-1");
}

#[test]
fn stored_file_id_display_matches_inner() {
    let id = StoredFileId::new("abc");
    assert_eq!(format!("{id}"), "abc");
}

#[test]
fn stored_file_id_from_string_and_str_conversions() {
    let id: StoredFileId = String::from("x").into();
    assert_eq!(id.as_str(), "x");
    let id: StoredFileId = "y".into();
    assert_eq!(id.as_str(), "y");
}

#[test]
fn stored_file_id_equality_and_hash() {
    use std::collections::HashSet;
    let a = StoredFileId::new("k");
    let b = StoredFileId::new("k");
    let c = StoredFileId::new("other");
    assert_eq!(a, b);
    assert_ne!(a, c);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
    assert!(!set.contains(&c));
}

#[test]
fn file_storage_error_variants_display_useful_messages() {
    let e = FileStorageError::NotFound("foo".to_owned());
    assert!(format!("{e}").contains("foo"));
    let e = FileStorageError::Validation("size".to_owned());
    assert!(format!("{e}").contains("size"));
    let e = FileStorageError::Backend("network".to_owned());
    assert!(format!("{e}").contains("network"));
}

#[test]
fn file_storage_error_from_io_error_preserves_display() {
    let io = std::io::Error::other("disk full");
    let e: FileStorageError = io.into();
    let s = format!("{e}");
    assert!(s.contains("io error"));
    assert!(s.contains("disk full"));
}

#[test]
fn file_storage_error_from_serde_json_error() {
    let json: serde_json::Error = serde_json::from_str::<i64>("not-a-number").unwrap_err();
    let e: FileStorageError = json.into();
    let s = format!("{e}");
    assert!(s.contains("serialization error"));
}

mod file_storage_default_method {
    use async_trait::async_trait;
    use std::path::Path;
    use systemprompt_traits::storage::{
        FileStorage, FileStorageResult, StoredFileId, StoredFileMetadata,
    };

    struct MinimalStorage;

    #[async_trait]
    impl FileStorage for MinimalStorage {
        async fn store(&self, _path: &Path, _content: &[u8]) -> FileStorageResult<StoredFileId> {
            Ok(StoredFileId::new("stored"))
        }

        async fn retrieve(&self, _id: &StoredFileId) -> FileStorageResult<Vec<u8>> {
            Ok(Vec::new())
        }

        async fn delete(&self, _id: &StoredFileId) -> FileStorageResult<()> {
            Ok(())
        }

        async fn metadata(&self, _id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
            Ok(StoredFileMetadata {
                id: StoredFileId::new("x"),
                path: "p".to_string(),
                mime_type: "text/plain".to_string(),
                size_bytes: None,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
        }

        async fn exists(&self, _id: &StoredFileId) -> FileStorageResult<bool> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn public_url_default_is_none() {
        let storage = MinimalStorage;
        assert!(storage.public_url(&StoredFileId::new("any")).is_none());
    }

    #[tokio::test]
    async fn provided_methods_dispatch_through_dyn() {
        let storage: Box<dyn FileStorage> = Box::new(MinimalStorage);
        assert_eq!(
            storage.store(Path::new("p"), b"data").await.unwrap(),
            StoredFileId::new("stored")
        );
        assert!(storage.exists(&StoredFileId::new("a")).await.unwrap());
        assert!(storage.public_url(&StoredFileId::new("a")).is_none());
    }
}
