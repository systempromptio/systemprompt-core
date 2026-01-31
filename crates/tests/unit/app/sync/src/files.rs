//! Tests for file sync types

use chrono::Utc;
use systemprompt_sync::{FileBundle, FileEntry, FileManifest};

mod file_entry_tests {
    use super::*;

    fn test_entry() -> FileEntry {
        FileEntry {
            path: "content/blog/post.md".to_string(),
            checksum: "abc123".to_string(),
            size: 1024,
        }
    }

    #[test]
    fn entry_path_is_set() {
        let entry = test_entry();
        assert_eq!(entry.path, "content/blog/post.md");
    }

    #[test]
    fn entry_checksum_is_set() {
        let entry = test_entry();
        assert_eq!(entry.checksum, "abc123");
    }

    #[test]
    fn entry_size_is_set() {
        let entry = test_entry();
        assert_eq!(entry.size, 1024);
    }

    #[test]
    fn entry_is_serializable() {
        let entry = test_entry();
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("content/blog/post.md"));
        assert!(json.contains("abc123"));
        assert!(json.contains("1024"));
    }

    #[test]
    fn entry_is_deserializable() {
        let json = r#"{"path":"a.txt","checksum":"xyz","size":100}"#;
        let entry: FileEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.path, "a.txt");
        assert_eq!(entry.checksum, "xyz");
        assert_eq!(entry.size, 100);
    }

    #[test]
    fn entry_is_clone() {
        let entry = test_entry();
        let cloned = entry.clone();
        assert_eq!(cloned.path, entry.path);
    }

    #[test]
    fn entry_is_debug() {
        let entry = test_entry();
        let debug = format!("{:?}", entry);
        assert!(debug.contains("FileEntry"));
    }
}

mod file_manifest_tests {
    use super::*;

    fn test_manifest() -> FileManifest {
        FileManifest {
            files: vec![
                FileEntry {
                    path: "a.txt".to_string(),
                    checksum: "aa".to_string(),
                    size: 10,
                },
                FileEntry {
                    path: "b.txt".to_string(),
                    checksum: "bb".to_string(),
                    size: 20,
                },
            ],
            timestamp: Utc::now(),
            checksum: "manifest-checksum".to_string(),
        }
    }

    #[test]
    fn manifest_files_count() {
        let manifest = test_manifest();
        assert_eq!(manifest.files.len(), 2);
    }

    #[test]
    fn manifest_has_timestamp() {
        let manifest = test_manifest();
        assert!(manifest.timestamp <= Utc::now());
    }

    #[test]
    fn manifest_has_checksum() {
        let manifest = test_manifest();
        assert_eq!(manifest.checksum, "manifest-checksum");
    }

    #[test]
    fn manifest_is_serializable() {
        let manifest = test_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("a.txt"));
        assert!(json.contains("b.txt"));
        assert!(json.contains("manifest-checksum"));
    }

    #[test]
    fn manifest_is_deserializable() {
        let manifest = test_manifest();
        let json = serde_json::to_string(&manifest).unwrap();
        let deserialized: FileManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.files.len(), 2);
        assert_eq!(deserialized.checksum, manifest.checksum);
    }

    #[test]
    fn manifest_is_clone() {
        let manifest = test_manifest();
        let cloned = manifest.clone();
        assert_eq!(cloned.files.len(), manifest.files.len());
    }

    #[test]
    fn manifest_is_debug() {
        let manifest = test_manifest();
        let debug = format!("{:?}", manifest);
        assert!(debug.contains("FileManifest"));
    }

    #[test]
    fn empty_manifest() {
        let manifest = FileManifest {
            files: vec![],
            timestamp: Utc::now(),
            checksum: String::new(),
        };
        assert!(manifest.files.is_empty());
    }
}

mod file_bundle_tests {
    use super::*;

    fn test_bundle() -> FileBundle {
        FileBundle {
            manifest: FileManifest {
                files: vec![FileEntry {
                    path: "test.txt".to_string(),
                    checksum: "test-checksum".to_string(),
                    size: 100,
                }],
                timestamp: Utc::now(),
                checksum: "bundle-checksum".to_string(),
            },
            data: vec![1, 2, 3, 4, 5],
        }
    }

    #[test]
    fn bundle_has_manifest() {
        let bundle = test_bundle();
        assert_eq!(bundle.manifest.files.len(), 1);
    }

    #[test]
    fn bundle_has_data() {
        let bundle = test_bundle();
        assert_eq!(bundle.data.len(), 5);
    }

    #[test]
    fn bundle_serializes_manifest_only() {
        let bundle = test_bundle();
        let json = serde_json::to_string(&bundle).unwrap();
        assert!(json.contains("test.txt"));
        assert!(!json.contains("data"));
    }

    #[test]
    fn bundle_is_clone() {
        let bundle = test_bundle();
        let cloned = bundle.clone();
        assert_eq!(cloned.manifest.files.len(), bundle.manifest.files.len());
    }

    #[test]
    fn bundle_is_debug() {
        let bundle = test_bundle();
        let debug = format!("{:?}", bundle);
        assert!(debug.contains("FileBundle"));
    }
}
