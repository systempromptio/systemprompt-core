//! Tests for `AiGeneratedFile`.

use chrono::Utc;
use systemprompt_identifiers::FileId;
use systemprompt_traits::AiGeneratedFile;

fn fixture_file(id: uuid::Uuid) -> AiGeneratedFile {
    AiGeneratedFile {
        id,
        path: "generated/report.png".to_owned(),
        public_url: "https://cdn.example.com/generated/report.png".to_owned(),
        mime_type: "image/png".to_owned(),
        size_bytes: Some(2048),
        ai_content: true,
        metadata: serde_json::json!({"prompt": "chart"}),
        user_id: None,
        session_id: None,
        trace_id: None,
        context_id: None,
        created_at: Utc::now(),
        updated_at: Utc::now(),
        deleted_at: None,
    }
}

#[test]
fn id_accessor_returns_typed_file_id_of_the_row_uuid() {
    let raw = uuid::Uuid::new_v4();
    let file = fixture_file(raw);

    assert_eq!(file.id(), FileId::new(raw.to_string()));
}

#[test]
fn serde_round_trip_preserves_identity_and_metadata() {
    let file = fixture_file(uuid::Uuid::new_v4());

    let back: AiGeneratedFile =
        serde_json::from_str(&serde_json::to_string(&file).unwrap()).unwrap();

    assert_eq!(back.id(), file.id());
    assert_eq!(back.metadata, file.metadata);
    assert_eq!(back.size_bytes, Some(2048));
}
