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
    let e = FileStorageError::Backend("network".into());
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
        ByteRange, FileStorage, FileStorageResult, StoredFileId, StoredFileMetadata, StoredFileRead,
    };

    struct MinimalStorage;

    #[async_trait]
    impl FileStorage for MinimalStorage {
        async fn store(&self, _path: &Path, _content: &[u8]) -> FileStorageResult<StoredFileId> {
            Ok(StoredFileId::new("stored"))
        }

        async fn open(
            &self,
            _id: &StoredFileId,
            _range: Option<ByteRange>,
        ) -> FileStorageResult<StoredFileRead> {
            Ok(StoredFileRead {
                metadata: self.metadata(&StoredFileId::new("x")).await?,
                range: None,
                body: Box::pin(futures::stream::empty()),
            })
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
                etag: None,
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

mod byte_range {
    use systemprompt_traits::{ByteRange, FileStorageError, ServedRange};

    #[test]
    fn parses_the_three_single_range_forms() {
        assert_eq!(
            ByteRange::parse("bytes=0-499"),
            Some(ByteRange::Bounded { start: 0, end: 499 })
        );
        assert_eq!(ByteRange::parse("bytes=500-"), Some(ByteRange::From(500)));
        assert_eq!(ByteRange::parse("bytes=-200"), Some(ByteRange::Suffix(200)));
    }

    #[test]
    fn rejects_multi_inverted_and_malformed_ranges() {
        for raw in [
            "bytes=0-1,4-5",
            "bytes=5-2",
            "bytes=-",
            "items=0-1",
            "bytes=a-b",
            "",
        ] {
            assert_eq!(ByteRange::parse(raw), None, "{raw}");
        }
    }

    #[test]
    fn resolves_against_the_object_size() {
        let r = ByteRange::Bounded {
            start: 90,
            end: 500,
        }
        .resolve(100)
        .unwrap();
        assert_eq!(
            r,
            ServedRange {
                start: 90,
                end: 99,
                size: 100
            }
        );
        assert_eq!(r.length(), 10);
        assert_eq!(r.content_range(), "bytes 90-99/100");
        assert!(matches!(
            ByteRange::From(100).resolve(100),
            Err(FileStorageError::RangeNotSatisfiable { size: 100 })
        ));
        assert!(ByteRange::From(0).resolve(0).is_err());
    }

    #[test]
    fn renders_the_request_header() {
        assert_eq!(ByteRange::Suffix(3).header_value(), "bytes=-3");
        assert_eq!(ByteRange::From(3).header_value(), "bytes=3-");
        assert_eq!(
            ByteRange::Bounded { start: 1, end: 2 }.header_value(),
            "bytes=1-2"
        );
    }

    #[test]
    fn suffix_ranges_resolve_to_the_tail_and_clamp_to_the_object() {
        assert_eq!(
            ByteRange::Suffix(3).resolve(10).unwrap(),
            ServedRange {
                start: 7,
                end: 9,
                size: 10
            }
        );
        assert_eq!(
            ByteRange::Suffix(50).resolve(10).unwrap(),
            ServedRange {
                start: 0,
                end: 9,
                size: 10
            }
        );
        assert!(matches!(
            ByteRange::Suffix(0).resolve(10),
            Err(FileStorageError::RangeNotSatisfiable { size: 10 })
        ));
        assert!(matches!(
            ByteRange::Suffix(4).resolve(0),
            Err(FileStorageError::RangeNotSatisfiable { size: 0 })
        ));
    }

    #[test]
    fn bounded_ranges_past_the_end_or_on_empty_objects_are_unsatisfiable() {
        assert!(matches!(
            ByteRange::Bounded { start: 10, end: 20 }.resolve(10),
            Err(FileStorageError::RangeNotSatisfiable { size: 10 })
        ));
        assert!(ByteRange::Bounded { start: 0, end: 0 }.resolve(0).is_err());
        assert_eq!(
            ByteRange::Bounded { start: 0, end: 0 }
                .resolve(1)
                .unwrap()
                .length(),
            1
        );
    }

    #[test]
    fn whitespace_around_the_spec_is_tolerated() {
        assert_eq!(
            ByteRange::parse("  bytes= 2 - 4 "),
            Some(ByteRange::Bounded { start: 2, end: 4 })
        );
        assert_eq!(ByteRange::parse("bytes=x-"), None);
        assert_eq!(ByteRange::parse("bytes=-x"), None);
        assert_eq!(ByteRange::parse("bytes=5"), None);
    }
}
