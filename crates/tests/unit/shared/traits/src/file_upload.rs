use systemprompt_identifiers::{ContextId, SessionId, TraceId, UserId};
use systemprompt_traits::file_upload::{FileUploadInput, FileUploadProviderError};

#[test]
fn file_upload_input_new_defaults() {
    let ctx = ContextId::generate();
    let i = FileUploadInput::new("image/png", "AAAA", Some(ctx.clone()));
    assert_eq!(i.mime_type, "image/png");
    assert_eq!(i.bytes_base64, "AAAA");
    assert_eq!(i.context_id, Some(ctx));
    assert!(i.name.is_none());
    assert!(i.user_id.is_none());
    assert!(i.session_id.is_none());
    assert!(i.trace_id.is_none());
}

#[test]
fn file_upload_input_builders_chain() {
    let i = FileUploadInput::new("text/plain", "", None)
        .with_name("file.txt")
        .with_user_id(UserId::new("u"))
        .with_session_id(SessionId::new("s"))
        .with_trace_id(TraceId::new("t"));
    assert_eq!(i.name.as_deref(), Some("file.txt"));
    assert_eq!(i.user_id.as_ref().unwrap().as_str(), "u");
    assert_eq!(i.session_id.as_ref().unwrap().as_str(), "s");
    assert_eq!(i.trace_id.as_ref().unwrap().as_str(), "t");
}

#[test]
fn file_upload_provider_error_displays_useful_text() {
    assert_eq!(FileUploadProviderError::Disabled.to_string(), "Upload disabled");
    let e = FileUploadProviderError::ValidationFailed("too large".to_owned());
    assert!(e.to_string().contains("too large"));
    let e = FileUploadProviderError::StorageError("disk full".to_owned());
    assert!(e.to_string().contains("disk full"));
    let e = FileUploadProviderError::Internal("boom".to_owned());
    assert!(e.to_string().contains("boom"));
}
