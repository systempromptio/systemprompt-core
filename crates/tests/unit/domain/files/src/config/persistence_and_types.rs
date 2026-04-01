//! Unit tests for FilePersistenceMode and AllowedFileTypes

use systemprompt_files::{AllowedFileTypes, FilePersistenceMode};

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
