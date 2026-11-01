use chrono::Utc;
use systemprompt_files::uploads::{UploadError, UploadSession, UploadStatus};
use systemprompt_identifiers::{ContentHash, DocumentUploadId, TraceId, UserId};

fn session(blob: Option<ContentHash>) -> UploadSession {
    let now = Utc::now();
    UploadSession {
        id: DocumentUploadId::new("00000000-0000-0000-0000-000000000001"),
        owner_user_id: UserId::new("user"),
        file_name: "a.png".to_owned(),
        declared_mime: "image/png".to_owned(),
        declared_size: 4,
        declared_sha256: ContentHash::new("a"),
        part_size: 4,
        status: UploadStatus::Completed,
        blob_sha256: blob,
        detected_mime: None,
        checked_kind: None,
        refusal: None,
        trace_id: TraceId::new("trace"),
        created_at: now,
        expires_at: now,
        finished_at: None,
    }
}

#[test]
fn a_session_without_a_blob_hash_has_no_etag() {
    assert_eq!(session(None).etag(), None);
}

#[test]
fn a_session_with_a_blob_hash_quotes_it_as_its_etag() {
    let tag = session(Some(ContentHash::new("abc"))).etag();
    assert_eq!(tag.as_deref(), Some("\"abc\""));
}

#[test]
fn a_size_mismatch_carries_both_sizes_and_is_an_invalid_request() {
    let error = UploadError::SizeMismatch {
        declared: 5,
        actual: 7,
    };
    assert_eq!(error.code(), "invalid_request");
    assert_eq!(error.to_string(), "declared size 5 but the body is 7 bytes");
}

#[test]
fn an_invalid_content_type_names_the_value_given() {
    let error = UploadError::InvalidContentType {
        given: "nonsense".to_owned(),
    };
    assert_eq!(error.code(), "invalid_request");
    assert_eq!(error.to_string(), "nonsense is not a MIME type");
}
