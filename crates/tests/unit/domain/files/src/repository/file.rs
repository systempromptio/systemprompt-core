//! Unit tests for InsertFileRequest
//!
//! Note: FileRepository methods require a database connection and are covered
//! in integration tests. These unit tests focus on InsertFileRequest builder.

use systemprompt_files::InsertFileRequest;
use systemprompt_identifiers::{FileId, SessionId, TraceId, UserId};

// ============================================================================
// InsertFileRequest Construction Tests
// ============================================================================

#[test]
fn test_insert_file_request_new() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(
        file_id.clone(),
        "/storage/test.png",
        "/files/test.png",
        "image/png",
    );

    assert_eq!(request.id.as_str(), file_id.as_str());
    assert_eq!(request.path, "/storage/test.png");
    assert_eq!(request.public_url, "/files/test.png");
    assert_eq!(request.mime_type, "image/png");
}

#[test]
fn test_insert_file_request_default_values() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(file_id, "/path", "/url", "application/octet-stream");

    assert!(request.size_bytes.is_none());
    assert!(!request.ai_content);
    assert!(request.metadata.is_object());
    assert!(request.user_id.is_none());
    assert!(request.session_id.is_none());
    assert!(request.trace_id.is_none());
}

// ============================================================================
// Builder Method Tests
// ============================================================================

#[test]
fn test_insert_file_request_with_size() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_size(4096);

    assert_eq!(request.size_bytes, Some(4096));
}

#[test]
fn test_insert_file_request_with_size_zero() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_size(0);

    assert_eq!(request.size_bytes, Some(0));
}

#[test]
fn test_insert_file_request_with_size_large() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let large_size: i64 = 10 * 1024 * 1024 * 1024; // 10 GB
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "video/mp4").with_size(large_size);

    assert_eq!(request.size_bytes, Some(large_size));
}

#[test]
fn test_insert_file_request_with_ai_content_true() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_ai_content(true);

    assert!(request.ai_content);
}

#[test]
fn test_insert_file_request_with_ai_content_false() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_ai_content(false);

    assert!(!request.ai_content);
}

#[test]
fn test_insert_file_request_with_metadata() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let metadata = serde_json::json!({
        "width": 1920,
        "height": 1080
    });

    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_metadata(metadata);

    assert!(request.metadata.is_object());
    assert_eq!(request.metadata["width"], 1920);
    assert_eq!(request.metadata["height"], 1080);
}

#[test]
fn test_insert_file_request_with_user_id() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let user_id = UserId::new("user_abc123");
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_user_id(user_id);

    assert!(request.user_id.is_some());
    assert_eq!(request.user_id.as_ref().unwrap().as_str(), "user_abc123");
}

#[test]
fn test_insert_file_request_with_session_id() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let session_id = SessionId::new("sess_xyz789");
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_session_id(session_id);

    assert!(request.session_id.is_some());
    assert_eq!(
        request.session_id.as_ref().unwrap().as_str(),
        "sess_xyz789"
    );
}

#[test]
fn test_insert_file_request_with_trace_id() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let trace_id = TraceId::new("trace_def456");
    let request =
        InsertFileRequest::new(file_id, "/path", "/url", "image/png").with_trace_id(trace_id);

    assert!(request.trace_id.is_some());
    assert_eq!(request.trace_id.as_ref().unwrap().as_str(), "trace_def456");
}

// ============================================================================
// Builder Chain Tests
// ============================================================================

#[test]
fn test_insert_file_request_builder_chain() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let user_id = UserId::new("user_123");
    let session_id = SessionId::new("sess_456");
    let trace_id = TraceId::new("trace_789");
    let metadata = serde_json::json!({"custom": "value"});

    let request = InsertFileRequest::new(
        file_id.clone(),
        "/storage/images/generated/art.png",
        "/files/images/generated/art.png",
        "image/png",
    )
    .with_size(2048)
    .with_ai_content(true)
    .with_metadata(metadata)
    .with_user_id(user_id)
    .with_session_id(session_id)
    .with_trace_id(trace_id);

    assert_eq!(request.id.as_str(), file_id.as_str());
    assert_eq!(request.path, "/storage/images/generated/art.png");
    assert_eq!(request.public_url, "/files/images/generated/art.png");
    assert_eq!(request.mime_type, "image/png");
    assert_eq!(request.size_bytes, Some(2048));
    assert!(request.ai_content);
    assert_eq!(request.metadata["custom"], "value");
    assert!(request.user_id.is_some());
    assert!(request.session_id.is_some());
    assert!(request.trace_id.is_some());
}

#[test]
fn test_insert_file_request_partial_builder() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());

    let request = InsertFileRequest::new(file_id, "/path", "/url", "application/pdf")
        .with_size(1024)
        .with_user_id(UserId::new("user"));

    assert_eq!(request.size_bytes, Some(1024));
    assert!(request.user_id.is_some());
    assert!(!request.ai_content);
    assert!(request.session_id.is_none());
    assert!(request.trace_id.is_none());
}

// ============================================================================
// Clone/Debug Tests
// ============================================================================

#[test]
fn test_insert_file_request_clone() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(file_id, "/path", "/url", "image/jpeg")
        .with_size(512)
        .with_ai_content(true);

    let cloned = request.clone();

    assert_eq!(request.id.as_str(), cloned.id.as_str());
    assert_eq!(request.path, cloned.path);
    assert_eq!(request.size_bytes, cloned.size_bytes);
    assert_eq!(request.ai_content, cloned.ai_content);
}

#[test]
fn test_insert_file_request_debug() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let request = InsertFileRequest::new(file_id, "/path", "/url", "image/gif");

    let debug_str = format!("{:?}", request);

    assert!(debug_str.contains("InsertFileRequest"));
    assert!(debug_str.contains("/path"));
    assert!(debug_str.contains("/url"));
    assert!(debug_str.contains("image/gif"));
}

// ============================================================================
// Various Mime Type Tests
// ============================================================================

#[test]
fn test_insert_file_request_image_mime_types() {
    let mime_types = [
        "image/png",
        "image/jpeg",
        "image/gif",
        "image/webp",
        "image/svg+xml",
        "image/x-icon",
    ];

    for mime_type in mime_types {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(file_id, "/path", "/url", mime_type);
        assert_eq!(request.mime_type, mime_type);
    }
}

#[test]
fn test_insert_file_request_document_mime_types() {
    let mime_types = [
        "application/pdf",
        "application/msword",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "text/plain",
    ];

    for mime_type in mime_types {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(file_id, "/path", "/url", mime_type);
        assert_eq!(request.mime_type, mime_type);
    }
}

#[test]
fn test_insert_file_request_video_mime_types() {
    let mime_types = ["video/mp4", "video/webm", "video/ogg"];

    for mime_type in mime_types {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(file_id, "/path", "/url", mime_type);
        assert_eq!(request.mime_type, mime_type);
    }
}

#[test]
fn test_insert_file_request_audio_mime_types() {
    let mime_types = ["audio/mpeg", "audio/wav", "audio/ogg", "audio/webm"];

    for mime_type in mime_types {
        let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
        let request = InsertFileRequest::new(file_id, "/path", "/url", mime_type);
        assert_eq!(request.mime_type, mime_type);
    }
}
