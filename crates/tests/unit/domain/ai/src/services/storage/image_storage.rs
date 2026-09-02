//! Tests for ImageStorage and StorageConfig over a local FileStorage backend.

use std::path::PathBuf;
use std::sync::Arc;

use systemprompt_ai::services::storage::{ImageStorage, StorageConfig};
use systemprompt_models::profile::StorageBackend;
use systemprompt_storage::build_file_storage;
use systemprompt_test_mocks::MockFileStorage;
use systemprompt_traits::{FileStorage, StoredFileId};
use tempfile::TempDir;

fn temp_backend() -> (TempDir, Arc<dyn FileStorage>) {
    let dir = TempDir::new().unwrap();
    let backend = build_file_storage(StorageBackend::Local, dir.path());
    (dir, backend)
}

fn config() -> StorageConfig {
    StorageConfig::new(
        PathBuf::from("files/images/generated"),
        "https://example.com/images".to_string(),
    )
}

fn create_temp_storage() -> (TempDir, ImageStorage) {
    let (dir, backend) = temp_backend();
    let storage = ImageStorage::new(config(), backend).unwrap();
    (dir, storage)
}

mod storage_config_tests {
    use super::*;

    #[test]
    fn new_creates_config() {
        let config = StorageConfig::new(
            PathBuf::from("images"),
            "https://cdn.example.com".to_string(),
        );

        assert_eq!(config.base_path, PathBuf::from("images"));
        assert_eq!(config.url_prefix, "https://cdn.example.com");
        assert_eq!(config.max_file_size_bytes, 10 * 1024 * 1024);
        assert!(config.organize_by_date);
    }

    #[test]
    fn validate_accepts_valid_config() {
        config().validate().expect("validation should succeed");
    }

    #[test]
    fn validate_rejects_empty_url_prefix() {
        let mut config = config();
        config.url_prefix = String::new();
        let err = config.validate().unwrap_err();
        assert!(err.contains("url_prefix"));
    }

    #[test]
    fn validate_rejects_zero_max_file_size() {
        let mut config = config();
        config.max_file_size_bytes = 0;
        let err = config.validate().unwrap_err();
        assert!(err.contains("max_file_size"));
    }

    #[test]
    fn config_is_debug_and_serializes() {
        let config = StorageConfig::new(
            PathBuf::from("data/images"),
            "https://cdn.test.com".to_string(),
        );
        let debug = format!("{:?}", config);
        assert!(debug.contains("StorageConfig"));
        assert!(debug.contains("data/images"));

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("data/images"));
        assert!(json.contains("cdn.test.com"));
    }
}

mod image_storage_tests {
    use super::*;

    #[test]
    fn new_rejects_invalid_config() {
        let (_dir, backend) = temp_backend();
        let mut config = config();
        config.url_prefix = String::new();
        ImageStorage::new(config, backend).unwrap_err();
    }

    #[test]
    fn debug_hides_the_backend() {
        let (_dir, storage) = create_temp_storage();
        let debug = format!("{storage:?}");
        assert!(debug.contains("ImageStorage"));
        assert!(debug.contains("files/images/generated"));
    }

    #[tokio::test]
    async fn save_image_bytes_writes_under_the_base_path_and_date_partition() {
        let (dir, storage) = create_temp_storage();

        let image_bytes = vec![0x89, 0x50, 0x4E, 0x47];
        let (id, url) = storage
            .save_image_bytes(&image_bytes, "image/png")
            .await
            .unwrap();

        let now = chrono::Utc::now();
        let partition = format!(
            "{:04}/{:02}/{:02}",
            chrono::Datelike::year(&now),
            chrono::Datelike::month(&now),
            chrono::Datelike::day(&now)
        );
        assert!(
            id.as_str()
                .starts_with(&format!("files/images/generated/{partition}/")),
            "{id}"
        );
        assert!(id.as_str().ends_with(".png"));
        assert!(url.starts_with(&format!("https://example.com/images/{partition}/")));
        assert_eq!(
            std::fs::read(dir.path().join(id.as_str())).unwrap(),
            image_bytes
        );
    }

    #[tokio::test]
    async fn save_image_bytes_respects_mime_type() {
        let (_dir, storage) = create_temp_storage();
        let bytes = vec![0xFF, 0xD8, 0xFF];

        for (mime, ext) in [
            ("image/jpeg", ".jpg"),
            ("image/webp", ".webp"),
            ("image/gif", ".gif"),
            ("image/unknown", ".png"),
        ] {
            let (id, _) = storage.save_image_bytes(&bytes, mime).await.unwrap();
            assert!(id.as_str().ends_with(ext), "{mime} -> {id}");
        }
    }

    #[tokio::test]
    async fn save_image_bytes_rejects_too_large_without_writing() {
        let backend = Arc::new(MockFileStorage::new());
        let mut config = config();
        config.max_file_size_bytes = 100;
        let storage =
            ImageStorage::new(config, Arc::clone(&backend) as Arc<dyn FileStorage>).unwrap();

        storage
            .save_image_bytes(&vec![0u8; 200], "image/png")
            .await
            .unwrap_err();
        assert!(backend.stored_files().await.is_empty());
    }

    #[tokio::test]
    async fn save_base64_image_decodes_and_saves() {
        let (dir, storage) = create_temp_storage();

        let (id, _) = storage
            .save_base64_image("dGVzdA==", "image/png")
            .await
            .unwrap();

        assert_eq!(
            std::fs::read(dir.path().join(id.as_str())).unwrap(),
            b"test"
        );
    }

    #[tokio::test]
    async fn save_base64_image_rejects_invalid_base64() {
        let (_dir, storage) = create_temp_storage();
        storage
            .save_base64_image("not valid base64!!!", "image/png")
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn delete_image_removes_file() {
        let (dir, storage) = create_temp_storage();

        let (id, _) = storage
            .save_image_bytes(&[1, 2, 3, 4], "image/png")
            .await
            .unwrap();
        assert!(storage.exists(&id).await.unwrap());

        storage.delete_image(&id).await.unwrap();

        assert!(!storage.exists(&id).await.unwrap());
        assert!(!dir.path().join(id.as_str()).exists());
    }

    #[tokio::test]
    async fn delete_image_fails_for_nonexistent() {
        let (_dir, storage) = create_temp_storage();
        storage
            .delete_image(&StoredFileId::new("files/images/generated/missing.png"))
            .await
            .unwrap_err();
    }

    #[tokio::test]
    async fn absolute_ids_are_rejected_by_the_backend() {
        let (dir, storage) = create_temp_storage();
        let outside = dir.path().join("outside.png");
        std::fs::write(&outside, b"keep").unwrap();

        let err = storage
            .delete_image(&StoredFileId::new(outside.to_string_lossy().into_owned()))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("relative"), "{err}");
        assert!(outside.exists(), "an absolute id must not delete anything");
    }

    #[test]
    fn get_full_path_joins_the_base_path() {
        let (_dir, storage) = create_temp_storage();
        assert_eq!(
            storage.get_full_path("subdir/image.png"),
            PathBuf::from("files/images/generated/subdir/image.png")
        );
    }

    #[tokio::test]
    async fn flat_storage_when_date_disabled() {
        let (_dir, backend) = temp_backend();
        let mut config = config();
        config.organize_by_date = false;
        let storage = ImageStorage::new(config, backend).unwrap();

        let (id, url) = storage
            .save_image_bytes(&[1, 2, 3], "image/png")
            .await
            .unwrap();

        assert_eq!(
            PathBuf::from(id.as_str()).parent().unwrap(),
            PathBuf::from("files/images/generated")
        );
        assert_eq!(
            url.matches('/').count(),
            "https://example.com/images/x".matches('/').count()
        );
    }

    #[tokio::test]
    async fn a_backend_write_failure_is_a_storage_error() {
        let backend: Arc<dyn FileStorage> =
            Arc::new(MockFileStorage::new().with_store_error("disk full"));
        let storage = ImageStorage::new(config(), backend).unwrap();

        let err = storage
            .save_image_bytes(b"\x89PNG\r\n\x1a\n", "image/png")
            .await
            .expect_err("a failing backend must fail the save");
        assert!(err.to_string().contains("disk full"), "{err}");
    }
}

mod from_trait_config {
    use super::*;
    use systemprompt_traits::ImageStorageConfig;

    #[test]
    fn the_trait_config_maps_onto_the_storage_config_defaults() {
        let config = StorageConfig::from_image_storage_config(ImageStorageConfig {
            base_path: PathBuf::from("files/images/generated"),
            url_prefix: "/media".to_owned(),
        });

        assert_eq!(config.base_path, PathBuf::from("files/images/generated"));
        assert_eq!(config.url_prefix, "/media");
        assert_eq!(
            config.max_file_size_bytes,
            StorageConfig::new(PathBuf::from("x"), "/y".to_owned()).max_file_size_bytes,
            "the trait-level config carries no size budget, so it must adopt the same default \
             as the direct constructor"
        );
        assert!(config.organize_by_date);
        config.validate().expect("valid");
    }
}
