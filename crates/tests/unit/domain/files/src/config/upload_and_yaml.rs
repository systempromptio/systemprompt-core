//! Unit tests for FileUploadConfig and FilesConfigYaml

use systemprompt_files::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};

#[test]
fn test_file_upload_config_default() {
    let config = FileUploadConfig::default();
    assert!(config.enabled);
    assert_eq!(config.max_file_size_bytes, 50 * 1024 * 1024);
    assert_eq!(config.persistence_mode, FilePersistenceMode::ContextScoped);
    assert!(config.allowed_types.images);
    assert!(config.allowed_types.documents);
    assert!(config.allowed_types.audio);
    assert!(!config.allowed_types.video);
}

#[test]
fn test_file_upload_config_clone() {
    let config = FileUploadConfig::default();
    let cloned = config;
    assert_eq!(config.enabled, cloned.enabled);
    assert_eq!(config.max_file_size_bytes, cloned.max_file_size_bytes);
}

#[test]
fn test_file_upload_config_debug() {
    let config = FileUploadConfig::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("FileUploadConfig"));
    assert!(debug_str.contains("enabled"));
    assert!(debug_str.contains("max_file_size_bytes"));
}

#[test]
fn test_file_upload_config_serialize() {
    let config = FileUploadConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"enabled\":true"));
    assert!(json.contains("\"max_file_size_bytes\":"));
    assert!(json.contains("\"persistence_mode\":"));
    assert!(json.contains("\"allowed_types\":"));
}

#[test]
fn test_file_upload_config_max_size_zero() {
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 0,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes::default(),
    };
    assert_eq!(config.max_file_size_bytes, 0);
}

#[test]
fn test_file_upload_config_large_max_size() {
    let config = FileUploadConfig {
        enabled: true,
        max_file_size_bytes: 10 * 1024 * 1024 * 1024,
        persistence_mode: FilePersistenceMode::ContextScoped,
        allowed_types: AllowedFileTypes::default(),
    };
    assert_eq!(config.max_file_size_bytes, 10 * 1024 * 1024 * 1024);
}

#[test]
fn test_files_config_yaml_default() {
    let config = FilesConfigYaml::default();
    assert_eq!(config.url_prefix, "/files");
    assert!(config.upload.enabled);
}

#[test]
fn test_files_config_yaml_debug() {
    let config = FilesConfigYaml::default();
    let debug_str = format!("{:?}", config);
    assert!(debug_str.contains("FilesConfigYaml"));
    assert!(debug_str.contains("url_prefix"));
}

#[test]
fn test_files_config_yaml_serialize() {
    let config = FilesConfigYaml::default();
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("\"urlPrefix\""));
    assert!(json.contains("\"upload\""));
}

#[test]
fn test_files_config_yaml_custom_url_prefix() {
    let config = FilesConfigYaml {
        url_prefix: "/storage/v1".to_string(),
        upload: FileUploadConfig::default(),
    };
    assert_eq!(config.url_prefix, "/storage/v1");
}

#[test]
fn test_files_config_yaml_empty_url_prefix() {
    let config = FilesConfigYaml {
        url_prefix: "".to_string(),
        upload: FileUploadConfig::default(),
    };
    assert_eq!(config.url_prefix, "");
}
