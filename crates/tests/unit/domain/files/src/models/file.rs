//! Unit tests for File model

use chrono::Utc;
use systemprompt_core_files::{File, FileMetadata, ImageMetadata};
use systemprompt_identifiers::{SessionId, TraceId, UserId};

// ============================================================================
// Helper Functions
// ============================================================================

fn create_test_file() -> File {
    let now = Utc::now();
    File {
        id: uuid::Uuid::new_v4(),
        path: "/storage/images/test.png".to_string(),
        public_url: "/files/images/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: Some(1024),
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

fn create_test_file_with_metadata(metadata: FileMetadata) -> File {
    let now = Utc::now();
    File {
        id: uuid::Uuid::new_v4(),
        path: "/storage/images/test.png".to_string(),
        public_url: "/files/images/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: Some(2048),
        ai_content: true,
        metadata: serde_json::to_value(metadata).unwrap(),
        user_id: Some(UserId::new("user_123")),
        session_id: Some(SessionId::new("sess_456")),
        trace_id: Some(TraceId::new("trace_789")),
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    }
}

// ============================================================================
// File::id() Tests
// ============================================================================

#[test]
fn test_file_id_returns_file_id() {
    let file = create_test_file();
    let file_id = file.id();

    assert_eq!(file_id.as_str(), file.id.to_string());
}

#[test]
fn test_file_id_format() {
    let uuid = uuid::Uuid::new_v4();
    let now = Utc::now();
    let file = File {
        id: uuid,
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: None,
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    let file_id = file.id();
    assert!(file_id
        .as_str()
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == '-'));
}

// ============================================================================
// File::metadata() Tests
// ============================================================================

#[test]
fn test_file_metadata_empty() {
    let file = create_test_file();
    let metadata = file.metadata().unwrap();

    assert!(metadata.checksums.is_none());
    assert!(metadata.type_specific.is_none());
}

#[test]
fn test_file_metadata_with_image() {
    let image_meta = ImageMetadata::new()
        .with_dimensions(1920, 1080)
        .with_alt_text("Test image");

    let file_meta = FileMetadata::new().with_image(image_meta);
    let file = create_test_file_with_metadata(file_meta);

    let metadata = file.metadata().unwrap();
    assert!(metadata.type_specific.is_some());
}

#[test]
fn test_file_metadata_with_checksums() {
    use systemprompt_core_files::FileChecksums;

    let checksums = FileChecksums::new()
        .with_md5("d41d8cd98f00b204e9800998ecf8427e")
        .with_sha256("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");

    let file_meta = FileMetadata::new().with_checksums(checksums);
    let file = create_test_file_with_metadata(file_meta);

    let metadata = file.metadata().unwrap();
    assert!(metadata.checksums.is_some());
    let cs = metadata.checksums.unwrap();
    assert_eq!(cs.md5, Some("d41d8cd98f00b204e9800998ecf8427e".to_string()));
    assert_eq!(
        cs.sha256,
        Some("e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_string())
    );
}

#[test]
fn test_file_metadata_invalid_json() {
    let now = Utc::now();
    let file = File {
        id: uuid::Uuid::new_v4(),
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: None,
        ai_content: false,
        metadata: serde_json::json!("not an object"),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    let result = file.metadata();
    assert!(result.is_err());
}

// ============================================================================
// File Field Tests
// ============================================================================

#[test]
fn test_file_with_all_optional_ids() {
    let now = Utc::now();
    let file = File {
        id: uuid::Uuid::new_v4(),
        path: "/storage/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: Some(4096),
        ai_content: true,
        metadata: serde_json::json!({}),
        user_id: Some(UserId::new("user_abc")),
        session_id: Some(SessionId::new("session_def")),
        trace_id: Some(TraceId::new("trace_ghi")),
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    assert!(file.user_id.is_some());
    assert!(file.session_id.is_some());
    assert!(file.trace_id.is_some());
    assert_eq!(file.user_id.as_ref().unwrap().as_str(), "user_abc");
    assert_eq!(file.session_id.as_ref().unwrap().as_str(), "session_def");
    assert_eq!(file.trace_id.as_ref().unwrap().as_str(), "trace_ghi");
}

#[test]
fn test_file_ai_content_flag() {
    let mut file = create_test_file();
    assert!(!file.ai_content);

    file.ai_content = true;
    assert!(file.ai_content);
}

#[test]
fn test_file_size_bytes() {
    let now = Utc::now();
    let file_with_size = File {
        id: uuid::Uuid::new_v4(),
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: Some(1_000_000),
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    assert_eq!(file_with_size.size_bytes, Some(1_000_000));

    let file_without_size = File {
        id: uuid::Uuid::new_v4(),
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: None,
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    assert!(file_without_size.size_bytes.is_none());
}

#[test]
fn test_file_deleted_at() {
    let now = Utc::now();
    let active_file = File {
        id: uuid::Uuid::new_v4(),
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: None,
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: None,
    };

    assert!(active_file.deleted_at.is_none());

    let deleted_file = File {
        id: uuid::Uuid::new_v4(),
        path: "/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        mime_type: "image/png".to_string(),
        size_bytes: None,
        ai_content: false,
        metadata: serde_json::json!({}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: now,
        updated_at: now,
        deleted_at: Some(now),
    };

    assert!(deleted_file.deleted_at.is_some());
}

// ============================================================================
// File Serialization Tests
// ============================================================================

#[test]
fn test_file_serialize_deserialize_roundtrip() {
    let file = create_test_file();
    let json = serde_json::to_string(&file).unwrap();
    let deserialized: File = serde_json::from_str(&json).unwrap();

    assert_eq!(file.id, deserialized.id);
    assert_eq!(file.path, deserialized.path);
    assert_eq!(file.public_url, deserialized.public_url);
    assert_eq!(file.mime_type, deserialized.mime_type);
    assert_eq!(file.size_bytes, deserialized.size_bytes);
    assert_eq!(file.ai_content, deserialized.ai_content);
}

#[test]
fn test_file_clone() {
    let file = create_test_file();
    let cloned = file.clone();

    assert_eq!(file.id, cloned.id);
    assert_eq!(file.path, cloned.path);
    assert_eq!(file.public_url, cloned.public_url);
}
