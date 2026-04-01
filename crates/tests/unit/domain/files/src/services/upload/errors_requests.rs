//! Unit tests for error types, FileUploadRequest builder, and UploadedFile

use systemprompt_files::{
    FileUploadError, FileUploadRequest, FileUploadRequestBuilder, FileValidationError, UploadedFile,
};
use systemprompt_identifiers::{ContextId, FileId, SessionId, TraceId, UserId};

#[test]
fn test_file_validation_error_display_uploads_disabled() {
    let err = FileValidationError::UploadsDisabled;
    assert_eq!(format!("{}", err), "File uploads are disabled");
}

#[test]
fn test_file_validation_error_display_file_too_large() {
    let err = FileValidationError::FileTooLarge {
        size: 1000,
        max: 500,
    };
    assert_eq!(
        format!("{}", err),
        "File size 1000 bytes exceeds maximum allowed 500 bytes"
    );
}

#[test]
fn test_file_validation_error_display_type_not_allowed() {
    let err = FileValidationError::TypeNotAllowed {
        mime_type: "application/octet-stream".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File type 'application/octet-stream' is not allowed"
    );
}

#[test]
fn test_file_validation_error_display_type_blocked() {
    let err = FileValidationError::TypeBlocked {
        mime_type: "application/x-executable".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File type 'application/x-executable' is blocked for security reasons"
    );
}

#[test]
fn test_file_validation_error_display_category_disabled() {
    let err = FileValidationError::CategoryDisabled {
        category: "video".to_string(),
    };
    assert_eq!(
        format!("{}", err),
        "File category 'video' is disabled in configuration"
    );
}

#[test]
fn test_file_validation_error_debug() {
    let err = FileValidationError::UploadsDisabled;
    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("UploadsDisabled"));
}

#[test]
fn test_file_upload_error_display_persistence_disabled() {
    let err = FileUploadError::PersistenceDisabled;
    assert_eq!(format!("{}", err), "File persistence is disabled");
}

#[test]
fn test_file_upload_error_display_validation() {
    let err = FileUploadError::Validation(FileValidationError::UploadsDisabled);
    assert!(format!("{}", err).contains("Validation failed"));
}

#[test]
fn test_file_upload_error_display_database() {
    let err = FileUploadError::Database("connection failed".to_string());
    assert_eq!(format!("{}", err), "Database error: connection failed");
}

#[test]
fn test_file_upload_error_display_config() {
    let err = FileUploadError::Config("missing path".to_string());
    assert_eq!(format!("{}", err), "Configuration error: missing path");
}

#[test]
fn test_file_upload_error_display_base64_too_large() {
    let err = FileUploadError::Base64TooLarge {
        encoded_size: 100_000_000,
    };
    assert!(format!("{}", err).contains("Base64 input too large"));
}

#[test]
fn test_file_upload_error_display_path_validation() {
    let err = FileUploadError::PathValidation("invalid characters".to_string());
    assert_eq!(
        format!("{}", err),
        "Path validation failed: invalid characters"
    );
}

#[test]
fn test_file_upload_error_from_validation_error() {
    let validation_err = FileValidationError::UploadsDisabled;
    let upload_err: FileUploadError = validation_err.into();
    match upload_err {
        FileUploadError::Validation(_) => {}
        _ => panic!("Expected Validation variant"),
    }
}

#[test]
fn test_file_upload_request_builder_new() {
    let context_id = ContextId::new("ctx_123");
    let builder =
        FileUploadRequestBuilder::new("image/png", "base64data==", context_id);
    let debug_str = format!("{:?}", builder);
    assert!(debug_str.contains("FileUploadRequestBuilder"));
}

#[test]
fn test_file_upload_request_builder_build() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id).build();

    assert_eq!(request.mime_type, "image/png");
    assert_eq!(request.bytes_base64, "base64data==");
    assert_eq!(request.context_id.as_str(), "ctx_123");
    assert!(request.name.is_none());
    assert!(request.user_id.is_none());
    assert!(request.session_id.is_none());
    assert!(request.trace_id.is_none());
}

#[test]
fn test_file_upload_request_builder_with_name() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_name("myfile.png")
        .build();

    assert_eq!(request.name, Some("myfile.png".to_string()));
}

#[test]
fn test_file_upload_request_builder_with_user_id() {
    let context_id = ContextId::new("ctx_123");
    let user_id = UserId::new("user_abc");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_user_id(user_id)
        .build();

    assert!(request.user_id.is_some());
    assert_eq!(request.user_id.as_ref().unwrap().as_str(), "user_abc");
}

#[test]
fn test_file_upload_request_builder_with_session_id() {
    let context_id = ContextId::new("ctx_123");
    let session_id = SessionId::new("sess_xyz");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_session_id(session_id)
        .build();

    assert!(request.session_id.is_some());
    assert_eq!(request.session_id.as_ref().unwrap().as_str(), "sess_xyz");
}

#[test]
fn test_file_upload_request_builder_with_trace_id() {
    let context_id = ContextId::new("ctx_123");
    let trace_id = TraceId::new("trace_def");
    let request = FileUploadRequestBuilder::new("image/png", "base64data==", context_id)
        .with_trace_id(trace_id)
        .build();

    assert!(request.trace_id.is_some());
    assert_eq!(request.trace_id.as_ref().unwrap().as_str(), "trace_def");
}

#[test]
fn test_file_upload_request_builder_full_chain() {
    let context_id = ContextId::new("ctx_123");
    let user_id = UserId::new("user_abc");
    let session_id = SessionId::new("sess_xyz");
    let trace_id = TraceId::new("trace_def");

    let request = FileUploadRequestBuilder::new("application/pdf", "pdfdata==", context_id)
        .with_name("document.pdf")
        .with_user_id(user_id)
        .with_session_id(session_id)
        .with_trace_id(trace_id)
        .build();

    assert_eq!(request.mime_type, "application/pdf");
    assert_eq!(request.bytes_base64, "pdfdata==");
    assert_eq!(request.context_id.as_str(), "ctx_123");
    assert_eq!(request.name, Some("document.pdf".to_string()));
    assert_eq!(request.user_id.as_ref().unwrap().as_str(), "user_abc");
    assert_eq!(request.session_id.as_ref().unwrap().as_str(), "sess_xyz");
    assert_eq!(request.trace_id.as_ref().unwrap().as_str(), "trace_def");
}

#[test]
fn test_file_upload_request_builder_static_method() {
    let context_id = ContextId::new("ctx_456");
    let request = FileUploadRequest::builder("image/jpeg", "jpegdata==", context_id).build();

    assert_eq!(request.mime_type, "image/jpeg");
    assert_eq!(request.bytes_base64, "jpegdata==");
}

#[test]
fn test_file_upload_request_clone() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequest::builder("image/png", "data==", context_id)
        .with_name("test.png")
        .build();

    let cloned = request.clone();
    assert_eq!(request.mime_type, cloned.mime_type);
    assert_eq!(request.bytes_base64, cloned.bytes_base64);
    assert_eq!(request.name, cloned.name);
}

#[test]
fn test_file_upload_request_debug() {
    let context_id = ContextId::new("ctx_123");
    let request = FileUploadRequest::builder("image/png", "data==", context_id).build();

    let debug_str = format!("{:?}", request);
    assert!(debug_str.contains("FileUploadRequest"));
    assert!(debug_str.contains("image/png"));
}

#[test]
fn test_uploaded_file_struct() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id: file_id.clone(),
        path: "/storage/uploads/test.png".to_string(),
        public_url: "/files/uploads/test.png".to_string(),
        size_bytes: 4096,
    };

    assert_eq!(uploaded.file_id.as_str(), file_id.as_str());
    assert_eq!(uploaded.path, "/storage/uploads/test.png");
    assert_eq!(uploaded.public_url, "/files/uploads/test.png");
    assert_eq!(uploaded.size_bytes, 4096);
}

#[test]
fn test_uploaded_file_clone() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id,
        path: "/storage/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        size_bytes: 1024,
    };

    let cloned = uploaded.clone();
    assert_eq!(uploaded.path, cloned.path);
    assert_eq!(uploaded.public_url, cloned.public_url);
    assert_eq!(uploaded.size_bytes, cloned.size_bytes);
}

#[test]
fn test_uploaded_file_debug() {
    let file_id = FileId::new(uuid::Uuid::new_v4().to_string());
    let uploaded = UploadedFile {
        file_id,
        path: "/storage/test.png".to_string(),
        public_url: "/files/test.png".to_string(),
        size_bytes: 2048,
    };

    let debug_str = format!("{:?}", uploaded);
    assert!(debug_str.contains("UploadedFile"));
    assert!(debug_str.contains("/storage/test.png"));
}
