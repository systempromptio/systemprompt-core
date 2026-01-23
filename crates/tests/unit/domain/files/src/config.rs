//! Unit tests for config types
//!
//! Tests cover:
//! - FilePersistenceMode enum (default, serialization, variants)
//! - AllowedFileTypes (default values, serialization)
//! - FileUploadConfig (default values, serialization)
//! - FilesConfigYaml (default values, serialization)

use systemprompt_files::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};

#[test]
fn test_file_persistence_mode_default() {
    let mode = FilePersistenceMode::default();
    assert_eq!(mode, FilePersistenceMode::ContextScoped);
}

#[test]
fn test_file_persistence_mode_context_scoped() {
    let mode = FilePersistenceMode::ContextScoped;
    assert_eq!(mode, FilePersistenceMode::ContextScoped);
}

#[test]
fn test_file_persistence_mode_user_library() {
    let mode = FilePersistenceMode::UserLibrary;
    assert_eq!(mode, FilePersistenceMode::UserLibrary);
}

#[test]
fn test_file_persistence_mode_disabled() {
    let mode = FilePersistenceMode::Disabled;
    assert_eq!(mode, FilePersistenceMode::Disabled);
}

#[test]
fn test_file_persistence_mode_clone() {
    let mode = FilePersistenceMode::ContextScoped;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_file_persistence_mode_copy() {
    let mode = FilePersistenceMode::UserLibrary;
    let copied: FilePersistenceMode = mode;
    assert_eq!(mode, copied);
}

#[test]
fn test_file_persistence_mode_debug() {
    let mode = FilePersistenceMode::ContextScoped;
    let debug_str = format!("{:?}", mode);
    assert!(debug_str.contains("ContextScoped"));
}

#[test]
fn test_file_persistence_mode_serialize_context_scoped() {
    let mode = FilePersistenceMode::ContextScoped;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"context_scoped\"");
}

#[test]
fn test_file_persistence_mode_serialize_user_library() {
    let mode = FilePersistenceMode::UserLibrary;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"user_library\"");
}

#[test]
fn test_file_persistence_mode_serialize_disabled() {
    let mode = FilePersistenceMode::Disabled;
    let json = serde_json::to_string(&mode).unwrap();
    assert_eq!(json, "\"disabled\"");
}

#[test]
fn test_file_persistence_mode_deserialize_context_scoped() {
    let mode: FilePersistenceMode = serde_json::from_str("\"context_scoped\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::ContextScoped);
}

#[test]
fn test_file_persistence_mode_deserialize_user_library() {
    let mode: FilePersistenceMode = serde_json::from_str("\"user_library\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::UserLibrary);
}

#[test]
fn test_file_persistence_mode_deserialize_disabled() {
    let mode: FilePersistenceMode = serde_json::from_str("\"disabled\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::Disabled);
}

#[test]
fn test_file_persistence_mode_roundtrip() {
    for mode in [
        FilePersistenceMode::ContextScoped,
        FilePersistenceMode::UserLibrary,
        FilePersistenceMode::Disabled,
    ] {
        let json = serde_json::to_string(&mode).unwrap();
        let restored: FilePersistenceMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, restored);
    }
}

#[test]
fn test_file_persistence_mode_equality() {
    assert_eq!(
        FilePersistenceMode::ContextScoped,
        FilePersistenceMode::ContextScoped
    );
    assert_ne!(
        FilePersistenceMode::ContextScoped,
        FilePersistenceMode::UserLibrary
    );
    assert_ne!(
        FilePersistenceMode::ContextScoped,
        FilePersistenceMode::Disabled
    );
}

#[test]
fn test_allowed_file_types_default() {
    let types = AllowedFileTypes::default();
    assert!(types.images);
    assert!(types.documents);
    assert!(types.audio);
    assert!(!types.video);
}

#[test]
fn test_allowed_file_types_clone() {
    let types = AllowedFileTypes::default();
    let cloned = types;
    assert_eq!(types.images, cloned.images);
    assert_eq!(types.documents, cloned.documents);
    assert_eq!(types.audio, cloned.audio);
    assert_eq!(types.video, cloned.video);
}

#[test]
fn test_allowed_file_types_copy() {
    let types = AllowedFileTypes {
        images: true,
        documents: false,
        audio: true,
        video: false,
    };
    let copied: AllowedFileTypes = types;
    assert_eq!(types.images, copied.images);
}

#[test]
fn test_allowed_file_types_debug() {
    let types = AllowedFileTypes::default();
    let debug_str = format!("{:?}", types);
    assert!(debug_str.contains("AllowedFileTypes"));
    assert!(debug_str.contains("images"));
    assert!(debug_str.contains("documents"));
}

#[test]
fn test_allowed_file_types_serialize() {
    let types = AllowedFileTypes::default();
    let json = serde_json::to_string(&types).unwrap();
    assert!(json.contains("\"images\":true"));
    assert!(json.contains("\"documents\":true"));
    assert!(json.contains("\"audio\":true"));
    assert!(json.contains("\"video\":false"));
}

#[test]
fn test_allowed_file_types_deserialize() {
    let json = r#"{"images":false,"documents":true,"audio":false,"video":true}"#;
    let types: AllowedFileTypes = serde_json::from_str(json).unwrap();
    assert!(!types.images);
    assert!(types.documents);
    assert!(!types.audio);
    assert!(types.video);
}

#[test]
fn test_allowed_file_types_roundtrip() {
    let types = AllowedFileTypes {
        images: true,
        documents: false,
        audio: true,
        video: true,
    };
    let json = serde_json::to_string(&types).unwrap();
    let restored: AllowedFileTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(types.images, restored.images);
    assert_eq!(types.documents, restored.documents);
    assert_eq!(types.audio, restored.audio);
    assert_eq!(types.video, restored.video);
}

#[test]
fn test_allowed_file_types_all_enabled() {
    let types = AllowedFileTypes {
        images: true,
        documents: true,
        audio: true,
        video: true,
    };
    assert!(types.images);
    assert!(types.documents);
    assert!(types.audio);
    assert!(types.video);
}

#[test]
fn test_allowed_file_types_all_disabled() {
    let types = AllowedFileTypes {
        images: false,
        documents: false,
        audio: false,
        video: false,
    };
    assert!(!types.images);
    assert!(!types.documents);
    assert!(!types.audio);
    assert!(!types.video);
}

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
fn test_file_upload_config_copy() {
    let config = FileUploadConfig {
        enabled: false,
        max_file_size_bytes: 1024,
        persistence_mode: FilePersistenceMode::UserLibrary,
        allowed_types: AllowedFileTypes::default(),
    };
    let copied: FileUploadConfig = config;
    assert_eq!(config.enabled, copied.enabled);
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
fn test_file_upload_config_deserialize() {
    let json = r#"{
        "enabled": false,
        "max_file_size_bytes": 10485760,
        "persistence_mode": "user_library",
        "allowed_types": {
            "images": true,
            "documents": false,
            "audio": false,
            "video": false
        }
    }"#;
    let config: FileUploadConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled);
    assert_eq!(config.max_file_size_bytes, 10 * 1024 * 1024);
    assert_eq!(config.persistence_mode, FilePersistenceMode::UserLibrary);
    assert!(config.allowed_types.images);
    assert!(!config.allowed_types.documents);
}

#[test]
fn test_file_upload_config_deserialize_with_defaults() {
    let json = r#"{}"#;
    let config: FileUploadConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert_eq!(config.max_file_size_bytes, 50 * 1024 * 1024);
}

#[test]
fn test_file_upload_config_roundtrip() {
    let config = FileUploadConfig {
        enabled: false,
        max_file_size_bytes: 100_000_000,
        persistence_mode: FilePersistenceMode::Disabled,
        allowed_types: AllowedFileTypes {
            images: false,
            documents: true,
            audio: true,
            video: true,
        },
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: FileUploadConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(config.enabled, restored.enabled);
    assert_eq!(config.max_file_size_bytes, restored.max_file_size_bytes);
    assert_eq!(config.persistence_mode, restored.persistence_mode);
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
fn test_files_config_yaml_clone() {
    let config = FilesConfigYaml::default();
    let cloned = config.clone();
    assert_eq!(config.url_prefix, cloned.url_prefix);
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
fn test_files_config_yaml_deserialize() {
    let json = r#"{
        "urlPrefix": "/custom/files",
        "upload": {
            "enabled": false,
            "max_file_size_bytes": 1048576,
            "persistence_mode": "disabled",
            "allowed_types": {
                "images": true,
                "documents": false,
                "audio": false,
                "video": false
            }
        }
    }"#;
    let config: FilesConfigYaml = serde_json::from_str(json).unwrap();
    assert_eq!(config.url_prefix, "/custom/files");
    assert!(!config.upload.enabled);
    assert_eq!(config.upload.max_file_size_bytes, 1024 * 1024);
    assert_eq!(
        config.upload.persistence_mode,
        FilePersistenceMode::Disabled
    );
}

#[test]
fn test_files_config_yaml_deserialize_with_defaults() {
    let json = r#"{}"#;
    let config: FilesConfigYaml = serde_json::from_str(json).unwrap();
    assert_eq!(config.url_prefix, "/files");
    assert!(config.upload.enabled);
}

#[test]
fn test_files_config_yaml_roundtrip() {
    let config = FilesConfigYaml {
        url_prefix: "/api/files".to_string(),
        upload: FileUploadConfig {
            enabled: true,
            max_file_size_bytes: 25 * 1024 * 1024,
            persistence_mode: FilePersistenceMode::UserLibrary,
            allowed_types: AllowedFileTypes {
                images: true,
                documents: true,
                audio: false,
                video: false,
            },
        },
    };
    let json = serde_json::to_string(&config).unwrap();
    let restored: FilesConfigYaml = serde_json::from_str(&json).unwrap();
    assert_eq!(config.url_prefix, restored.url_prefix);
    assert_eq!(config.upload.enabled, restored.upload.enabled);
    assert_eq!(
        config.upload.max_file_size_bytes,
        restored.upload.max_file_size_bytes
    );
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
