use systemprompt_identifiers::{
    ContentHash, DocumentEditSessionId, DocumentFolderId, DocumentId, DocumentNodeId,
    DocumentRevisionId, DocumentTemplateId, DriveDeviceId, EditorId,
};

const HASH: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";

#[test]
fn content_hash_accepts_64_lowercase_hex() {
    assert_eq!(ContentHash::try_new(HASH).expect("valid").as_str(), HASH);
}

#[test]
fn content_hash_rejects_uppercase() {
    assert!(ContentHash::try_new(HASH.to_uppercase()).is_err());
}

#[test]
fn content_hash_rejects_wrong_lengths() {
    assert!(ContentHash::try_new(&HASH[..63]).is_err());
    assert!(ContentHash::try_new(format!("{HASH}0")).is_err());
    assert!(ContentHash::try_new("").is_err());
}

#[test]
fn content_hash_rejects_non_hex() {
    assert!(ContentHash::try_new(format!("{}g", &HASH[..63])).is_err());
}

#[test]
fn content_hash_deserialize_validates() {
    let bad = serde_json::json!(HASH.to_uppercase());
    assert!(serde_json::from_value::<ContentHash>(bad).is_err());
    let good: ContentHash = serde_json::from_value(serde_json::json!(HASH)).expect("valid");
    assert_eq!(good.as_str(), HASH);
}

#[test]
fn uuid_ids_round_trip() {
    let raw = uuid::Uuid::new_v4();
    assert_eq!(DocumentId::from_uuid(raw).to_uuid().expect("uuid"), raw);
    assert_eq!(
        DocumentFolderId::from_uuid(raw).to_uuid().expect("uuid"),
        raw
    );
    assert_eq!(
        DocumentRevisionId::from_uuid(raw).to_uuid().expect("uuid"),
        raw
    );
    assert_eq!(
        DocumentEditSessionId::from_uuid(raw)
            .to_uuid()
            .expect("uuid"),
        raw
    );
    let generated = DocumentId::generate();
    assert!(DocumentId::try_new(generated.as_str()).is_ok());
}

#[test]
fn uuid_ids_reject_non_uuid() {
    assert!(DocumentId::try_new("not-a-uuid").is_err());
    assert!(DocumentRevisionId::try_new("").is_err());
}

#[test]
fn checked_ids_reject_empty_and_whitespace() {
    assert!(DocumentNodeId::try_new("").is_err());
    assert!(DocumentNodeId::try_new("   ").is_err());
    assert!(DocumentTemplateId::try_new("").is_err());
    assert!(DriveDeviceId::try_new("\t").is_err());
    assert_eq!(
        DocumentNodeId::try_new("p12").expect("valid").as_str(),
        "p12"
    );
}

#[test]
fn editor_ids_reject_blank_values() {
    assert!(EditorId::try_new("\t").is_err());
    assert_eq!(EditorId::try_new("tab-1").expect("valid").as_str(), "tab-1");
}
