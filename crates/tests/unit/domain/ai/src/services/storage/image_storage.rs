//! Tests for ImageStorage and StorageConfig.

use std::path::PathBuf;
use systemprompt_ai::services::storage::{ImageStorage, StorageConfig};
use tempfile::TempDir;

fn create_temp_storage() -> (TempDir, StorageConfig) {
    let temp_dir = TempDir::new().unwrap();
    let config = StorageConfig::new(
        temp_dir.path().to_path_buf(),
        "https://example.com/images".to_string(),
    );
    (temp_dir, config)
}

mod storage_config_tests {
    use super::*;

    #[test]
    fn new_creates_config() {
        let config = StorageConfig::new(
            PathBuf::from("/images"),
            "https://cdn.example.com".to_string(),
        );

        assert_eq!(config.base_path, PathBuf::from("/images"));
        assert_eq!(config.url_prefix, "https://cdn.example.com");
        assert_eq!(config.max_file_size_bytes, 10 * 1024 * 1024);
        assert!(config.organize_by_date);
    }

    #[test]
    fn validate_accepts_valid_config() {
        let config = StorageConfig::new(PathBuf::from("/tmp"), "https://example.com".to_string());

        config.validate().expect("validation should succeed");
    }

    #[test]
    fn validate_rejects_empty_url_prefix() {
        let mut config =
            StorageConfig::new(PathBuf::from("/tmp"), "https://example.com".to_string());
        config.url_prefix = String::new();

        let result = config.validate();
        let err = result.unwrap_err();
        assert!(err.contains("url_prefix"));
    }

    #[test]
    fn validate_rejects_zero_max_file_size() {
        let mut config =
            StorageConfig::new(PathBuf::from("/tmp"), "https://example.com".to_string());
        config.max_file_size_bytes = 0;

        let result = config.validate();
        let err = result.unwrap_err();
        assert!(err.contains("max_file_size"));
    }

    #[test]
    fn config_is_debug() {
        let config =
            StorageConfig::new(PathBuf::from("/images"), "https://example.com".to_string());
        let debug = format!("{:?}", config);

        assert!(debug.contains("StorageConfig"));
        assert!(debug.contains("/images"));
    }

    #[test]
    fn config_serialization() {
        let config = StorageConfig::new(
            PathBuf::from("/data/images"),
            "https://cdn.test.com".to_string(),
        );

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("/data/images"));
        assert!(json.contains("cdn.test.com"));
    }
}

mod image_storage_tests {
    use super::*;

    #[test]
    fn new_creates_directory_if_missing() {
        let temp_dir = TempDir::new().unwrap();
        let subdir = temp_dir.path().join("new_subdir");

        let config = StorageConfig::new(subdir.clone(), "https://example.com".to_string());
        let _storage = ImageStorage::new(config).unwrap();

        assert!(subdir.exists());
    }

    #[test]
    fn new_rejects_invalid_config() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = StorageConfig::new(
            temp_dir.path().to_path_buf(),
            "https://example.com".to_string(),
        );
        config.url_prefix = String::new();

        let result = ImageStorage::new(config);
        result.unwrap_err();
    }

    #[test]
    fn save_image_bytes_creates_file() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let image_bytes = vec![0x89, 0x50, 0x4E, 0x47]; // PNG magic bytes
        let (path, url) = storage.save_image_bytes(&image_bytes, "image/png").unwrap();

        assert!(path.exists());
        assert!(url.starts_with("https://example.com"));
        assert!(path.to_string_lossy().contains(".png"));
    }

    #[test]
    fn save_image_bytes_respects_mime_type() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let bytes = vec![0xFF, 0xD8, 0xFF]; // JPEG magic bytes

        let (path, _) = storage.save_image_bytes(&bytes, "image/jpeg").unwrap();
        assert!(path.to_string_lossy().contains(".jpg"));

        let (path, _) = storage.save_image_bytes(&bytes, "image/webp").unwrap();
        assert!(path.to_string_lossy().contains(".webp"));

        let (path, _) = storage.save_image_bytes(&bytes, "image/gif").unwrap();
        assert!(path.to_string_lossy().contains(".gif"));

        let (path, _) = storage.save_image_bytes(&bytes, "image/unknown").unwrap();
        assert!(path.to_string_lossy().contains(".png"));
    }

    #[test]
    fn save_image_bytes_rejects_too_large() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = StorageConfig::new(
            temp_dir.path().to_path_buf(),
            "https://example.com".to_string(),
        );
        config.max_file_size_bytes = 100; // Very small limit

        let storage = ImageStorage::new(config).unwrap();
        let large_bytes = vec![0u8; 200]; // Exceeds limit

        let result = storage.save_image_bytes(&large_bytes, "image/png");
        result.unwrap_err();
    }

    #[test]
    fn save_base64_image_decodes_and_saves() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let base64_data = "dGVzdA==";
        let (path, _) = storage.save_base64_image(base64_data, "image/png").unwrap();

        assert!(path.exists());
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, b"test");
    }

    #[test]
    fn save_base64_image_rejects_invalid_base64() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let result = storage.save_base64_image("not valid base64!!!", "image/png");
        result.unwrap_err();
    }

    #[test]
    fn delete_image_removes_file() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let bytes = vec![1, 2, 3, 4];
        let (path, _) = storage.save_image_bytes(&bytes, "image/png").unwrap();

        assert!(path.exists());
        storage.delete_image(&path).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn delete_image_fails_for_nonexistent() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let fake_path = PathBuf::from("/nonexistent/image.png");
        let result = storage.delete_image(&fake_path);

        result.unwrap_err();
    }

    #[test]
    fn exists_returns_correct_status() {
        let (_temp_dir, config) = create_temp_storage();
        let storage = ImageStorage::new(config).unwrap();

        let bytes = vec![1, 2, 3];
        let (path, _) = storage.save_image_bytes(&bytes, "image/png").unwrap();

        assert!(ImageStorage::exists(&path));
        assert!(!ImageStorage::exists(&PathBuf::from("/nonexistent")));
    }

    #[test]
    fn get_full_path_joins_correctly() {
        let (_temp_dir, config) = create_temp_storage();
        let base_path = config.base_path.clone();
        let storage = ImageStorage::new(config).unwrap();

        let full = storage.get_full_path("subdir/image.png");
        assert_eq!(full, base_path.join("subdir/image.png"));
    }

    #[test]
    fn organizes_by_date_when_enabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = StorageConfig::new(
            temp_dir.path().to_path_buf(),
            "https://example.com".to_string(),
        );
        config.organize_by_date = true;

        let storage = ImageStorage::new(config).unwrap();
        let bytes = vec![1, 2, 3];
        let (path, url) = storage.save_image_bytes(&bytes, "image/png").unwrap();

        let path_str = path.to_string_lossy();
        assert!(path_str.contains('/'));

        assert!(url.contains('/'));
    }

    #[test]
    fn flat_storage_when_date_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = StorageConfig::new(
            temp_dir.path().to_path_buf(),
            "https://example.com".to_string(),
        );
        config.organize_by_date = false;

        let storage = ImageStorage::new(config.clone()).unwrap();
        let bytes = vec![1, 2, 3];
        let (path, _) = storage.save_image_bytes(&bytes, "image/png").unwrap();

        assert_eq!(path.parent().unwrap(), config.base_path);
    }
}

mod from_trait_config {
    use super::*;
    use systemprompt_traits::ImageStorageConfig;

    #[test]
    fn the_trait_config_maps_onto_the_storage_config_defaults() {
        let config = StorageConfig::from_image_storage_config(ImageStorageConfig {
            base_path: std::path::PathBuf::from("/srv/images"),
            url_prefix: "/media".to_owned(),
        });

        assert_eq!(config.base_path, std::path::PathBuf::from("/srv/images"));
        assert_eq!(config.url_prefix, "/media");
        assert_eq!(
            config.max_file_size_bytes,
            StorageConfig::new(std::path::PathBuf::from("/x"), "/y".to_owned()).max_file_size_bytes,
            "the trait-level config carries no size budget, so it must adopt the same default \
             as the direct constructor"
        );
        assert!(
            config.organize_by_date,
            "date organisation is the default layout"
        );
        config
            .validate()
            .expect("a config mapped from the trait form must be valid");
    }
}

mod filesystem_failure_arms {
    use super::*;

    #[test]
    fn a_write_whose_parent_cannot_be_created_is_a_storage_error() {
        let temp = TempDir::new().unwrap();
        let base = temp.path().join("images");
        std::fs::create_dir_all(&base).unwrap();

        let storage = ImageStorage::new(StorageConfig::new(
            base.clone(),
            "https://example.com/images".to_string(),
        ))
        .expect("storage builds");

        // Occupy the date-partition directory the writer wants to create with a
        // regular file, so `create_dir_all` cannot succeed.
        let now = chrono::Utc::now();
        let year_dir = base.join(format!("{:04}", chrono::Datelike::year(&now)));
        std::fs::write(&year_dir, b"not a directory").expect("plant a blocking file");

        let err = storage
            .save_image_bytes(b"\x89PNG\r\n\x1a\n", "image/png")
            .expect_err("a directory that cannot be created must fail the save");
        let message = err.to_string();
        assert!(
            message.contains("directory") || message.contains("file"),
            "the error must name the filesystem operation that failed, got {message}"
        );
    }

    #[test]
    fn deleting_a_file_leaves_the_storage_usable_even_when_the_directory_cannot_be_pruned() {
        let temp = TempDir::new().unwrap();
        let base = temp.path().join("images");
        let storage = ImageStorage::new(StorageConfig::new(
            base,
            "https://example.com/images".to_string(),
        ))
        .expect("storage builds");

        let (path, _url) = storage
            .save_image_bytes(b"\x89PNG\r\n\x1a\n", "image/png")
            .expect("save");

        // A sibling file keeps the parent directory non-empty, so the cleanup
        // pass has to leave it alone rather than fail the delete.
        let sibling = path.parent().unwrap().join("keep-me.txt");
        std::fs::write(&sibling, b"keep").expect("plant a sibling");

        storage
            .delete_image(&path)
            .expect("the delete succeeds even though the directory survives");

        assert!(!path.exists(), "the target file must be gone");
        assert!(
            sibling.exists(),
            "a non-empty directory must not be pruned out from under its other files"
        );
    }

    #[test]
    fn deleting_the_last_file_prunes_the_empty_partition_directories() {
        let temp = TempDir::new().unwrap();
        let base = temp.path().join("images");
        let storage = ImageStorage::new(StorageConfig::new(
            base.clone(),
            "https://example.com/images".to_string(),
        ))
        .expect("storage builds");

        let (path, _url) = storage
            .save_image_bytes(b"\x89PNG\r\n\x1a\n", "image/png")
            .expect("save");
        let day_dir = path.parent().unwrap().to_path_buf();
        assert!(day_dir.exists());

        storage.delete_image(&path).expect("delete");

        assert!(
            !day_dir.exists(),
            "the now-empty date partition must be cleaned up"
        );
        assert!(base.exists(), "the storage root itself must survive");
    }
}
