use systemprompt_identifiers::{DbValue, ToDbValue, UserId};

const UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

#[test]
fn display_format() {
    let id = UserId::new(UUID);
    assert_eq!(format!("{}", id), UUID);
}

#[test]
fn serde_transparent_json() {
    let id = UserId::new(UUID);
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, format!("\"{UUID}\""));
    let deserialized: UserId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn deserialize_rejects_a_non_uuid() {
    let result: Result<UserId, _> = serde_json::from_str("\"serde-user\"");
    assert!(result.is_err());
}

#[test]
fn try_new_accepts_a_uuid() {
    let id = UserId::try_new(UUID).unwrap();
    assert_eq!(id.as_str(), UUID);
    assert_eq!(id.to_uuid().unwrap().to_string(), UUID);
}

#[test]
fn try_new_rejects_sentinels_and_garbage() {
    for raw in ["", "unset", "unknown", "user@example.com", "test-user"] {
        assert!(UserId::try_new(raw).is_err(), "{raw:?} must be rejected");
    }
}

#[test]
fn from_str_validates() {
    assert!(UUID.parse::<UserId>().is_ok());
    assert!("not-a-uuid".parse::<UserId>().is_err());
}

#[test]
fn generate_and_from_uuid_round_trip() {
    let id = UserId::generate();
    let uuid = id.to_uuid().unwrap();
    assert_eq!(UserId::from_uuid(uuid), id);
}

#[test]
fn into_string_conversion() {
    let id = UserId::new(UUID);
    let s: String = id.into();
    assert_eq!(s, UUID);
}

#[test]
fn partial_eq_str() {
    let id = UserId::new(UUID);
    assert!(id == UUID);
    assert!(UUID == id);
}

#[test]
fn to_db_value_owned_and_ref() {
    let id = UserId::new(UUID);
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == UUID));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == UUID));
}
