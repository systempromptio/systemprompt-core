use systemprompt_files::{AllowedFileTypes, FilePersistenceMode, FileUploadConfig, FilesConfigYaml};

#[test]
fn persistence_mode_deserialize_context_scoped() {
    let mode: FilePersistenceMode = serde_json::from_str("\"context_scoped\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::ContextScoped);
}

#[test]
fn persistence_mode_deserialize_user_library() {
    let mode: FilePersistenceMode = serde_json::from_str("\"user_library\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::UserLibrary);
}

#[test]
fn persistence_mode_deserialize_disabled() {
    let mode: FilePersistenceMode = serde_json::from_str("\"disabled\"").unwrap();
    assert_eq!(mode, FilePersistenceMode::Disabled);
}

#[test]
fn persistence_mode_roundtrip_all_variants() {
    for mode in [
        FilePersistenceMode::ContextScoped,
        FilePersistenceMode::UserLibrary,
        FilePersistenceMode::Disabled,
    ] {
        let json = serde_json::to_string(&mode).unwrap();
        let back: FilePersistenceMode = serde_json::from_str(&json).unwrap();
        assert_eq!(mode, back);
    }
}

#[test]
fn allowed_file_types_deserialize_all_true() {
    let json = r#"{"images":true,"documents":true,"audio":true,"video":true}"#;
    let types: AllowedFileTypes = serde_json::from_str(json).unwrap();
    assert!(types.images);
    assert!(types.documents);
    assert!(types.audio);
    assert!(types.video);
}

#[test]
fn allowed_file_types_deserialize_all_false() {
    let json = r#"{"images":false,"documents":false,"audio":false,"video":false}"#;
    let types: AllowedFileTypes = serde_json::from_str(json).unwrap();
    assert!(!types.images);
    assert!(!types.documents);
    assert!(!types.audio);
    assert!(!types.video);
}

#[test]
fn allowed_file_types_roundtrip() {
    let original = AllowedFileTypes {
        images: true,
        documents: false,
        audio: true,
        video: false,
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: AllowedFileTypes = serde_json::from_str(&json).unwrap();
    assert_eq!(original.images, back.images);
    assert_eq!(original.documents, back.documents);
    assert_eq!(original.audio, back.audio);
    assert_eq!(original.video, back.video);
}

#[test]
fn file_upload_config_deserialize_explicit_values() {
    let json = r#"{
        "enabled": false,
        "max_file_size_bytes": 1048576,
        "persistence_mode": "user_library",
        "allowed_types": {"images":true,"documents":false,"audio":false,"video":true}
    }"#;
    let cfg: FileUploadConfig = serde_json::from_str(json).unwrap();
    assert!(!cfg.enabled);
    assert_eq!(cfg.max_file_size_bytes, 1048576);
    assert_eq!(cfg.persistence_mode, FilePersistenceMode::UserLibrary);
    assert!(cfg.allowed_types.images);
    assert!(!cfg.allowed_types.documents);
    assert!(!cfg.allowed_types.audio);
    assert!(cfg.allowed_types.video);
}

#[test]
fn file_upload_config_deserialize_uses_defaults_for_missing() {
    let cfg: FileUploadConfig = serde_json::from_str("{}").unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.max_file_size_bytes, 50 * 1024 * 1024);
    assert_eq!(cfg.persistence_mode, FilePersistenceMode::ContextScoped);
}

#[test]
fn file_upload_config_roundtrip() {
    let original = FileUploadConfig {
        enabled: false,
        max_file_size_bytes: 10_000_000,
        persistence_mode: FilePersistenceMode::Disabled,
        allowed_types: AllowedFileTypes {
            images: false,
            documents: true,
            audio: false,
            video: false,
        },
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FileUploadConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(original.enabled, back.enabled);
    assert_eq!(original.max_file_size_bytes, back.max_file_size_bytes);
    assert_eq!(original.persistence_mode, back.persistence_mode);
    assert_eq!(original.allowed_types.documents, back.allowed_types.documents);
}

#[test]
fn files_config_yaml_deserialize_camel_case_url_prefix() {
    let yaml = r#"{"urlPrefix": "/media", "upload": {}}"#;
    let cfg: FilesConfigYaml = serde_json::from_str(yaml).unwrap();
    assert_eq!(cfg.url_prefix, "/media");
}

#[test]
fn files_config_yaml_deserialize_defaults_when_empty() {
    let cfg: FilesConfigYaml = serde_json::from_str("{}").unwrap();
    assert_eq!(cfg.url_prefix, "/files");
    assert!(cfg.upload.enabled);
}

#[test]
fn files_config_yaml_roundtrip() {
    let original = FilesConfigYaml {
        url_prefix: "/assets".to_owned(),
        upload: FileUploadConfig {
            enabled: false,
            max_file_size_bytes: 5_000_000,
            persistence_mode: FilePersistenceMode::UserLibrary,
            allowed_types: AllowedFileTypes::default(),
        },
    };
    let json = serde_json::to_string(&original).unwrap();
    let back: FilesConfigYaml = serde_json::from_str(&json).unwrap();
    assert_eq!(original.url_prefix, back.url_prefix);
    assert_eq!(original.upload.enabled, back.upload.enabled);
    assert_eq!(
        original.upload.max_file_size_bytes,
        back.upload.max_file_size_bytes
    );
}
